//! Orchestrateur du crawl — version CONCURRENTE (Vague 4).
//!
//! Pilote une exploration en largeur (BFS) a partir d'une URL seed, mais traite
//! desormais jusqu'a `config.threads` pages EN PARALLELE :
//!   1. Sonde HTTP legere de chaque page (`http_probe::probe`) -> statut, headers,
//!      HTML, liens, temps de reponse.
//!   2. Detection de techno (`techno::detect`).
//!   3. DECISION de mesure :
//!        - si `force_chrome` OU framework JS detecte -> mesure NAVIGATEUR reelle
//!          (`chrome_probe::measure_page`, bande passante CDP) ;
//!        - sinon -> mesure HTTP statique (`http_probe::measure_static_assets`).
//!   4. Notifications via callbacks `on_page` puis `on_progress`.
//!   5. Reinjection des liens internes (meme host si `same_domain_only`) tant que
//!      `depth < max_depth`, dans la file PARTAGEE.
//!
//! ## Concurrence
//!
//! Tout l'etat muable est porte par [`SharedState`], thread-safe :
//!   - `queue`   : file BFS `(url, depth)` derriere un `tokio::sync::Mutex` ;
//!   - `visited` : ensemble des URLs deja prises en charge (`Mutex<HashSet>`) ;
//!   - `done` / `active` : compteurs atomiques (pages terminees / workers occupes) ;
//!   - `summary` : agregats du resume derriere un `Mutex` ;
//!   - `browser` : navigateur lance en lazy UNE fois, derriere un
//!     `tokio::sync::Mutex<Option<Browser>>` — les sondes HTTP (majoritaires)
//!     restent paralleles, seules les mesures Chrome se serialisent.
//!
//! `config.threads` workers `tokio::spawn` tournent en boucle : ils tirent une URL
//! de la file, la traitent, et reinjectent ses liens. La fin est detectee quand la
//! file est vide ET plus aucun worker n'est actif.
//!
//! ## Pause / reprise
//!
//! [`SharedState`] est concu pour SURVIVRE entre deux appels : `stop()` leve un
//! drapeau `stop` (les workers finissent leur page courante puis sortent SANS
//! vider la file/visited/summary). Un appel ulterieur a [`run_shared`] sur le meme
//! `SharedState` REPREND la ou on s'etait arrete (workers relances sur la file
//! existante). [`run`] reste l'entree autonome historique (etat local jetable).

use std::collections::{HashSet, VecDeque};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use governor::{Quota, RateLimiter};
use tokio::sync::Mutex;
use url::Url;

use crate::report::{
    AssetReport, CrawlConfig, CrawlProgress, CrawlSummary, PageReport, TypeStat,
};

/// User-agent par defaut (utilise quand la rotation aleatoire est desactivee).
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/145.0.0.0 Safari/537.36";

/// Frameworks dont la presence declenche la mesure NAVIGATEUR (rendu/hydratation
/// cote client : la mesure HTTP statique sous-estimerait la bande passante reelle).
const JS_FRAMEWORKS: [&str; 7] = [
    "Next.js", "Nuxt", "Gatsby", "SvelteKit", "Remix", "React", "Vue",
];

/// Vrai si l'une des technos detectees correspond a un framework JS connu.
fn needs_chrome(tech: &[String]) -> bool {
    tech.iter().any(|t| JS_FRAMEWORKS.contains(&t.as_str()))
}

/// Petit generateur xorshift64 (pour les delais aleatoires "humains").
fn rng_next(mut x: u64) -> u64 {
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Host (domaine) d'une URL, en minuscules. `None` si l'URL est invalide ou sans host.
fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
}

/// Lit la valeur d'un header (insensible a la casse) en minuscules.
fn header_lower(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())
}

// =============================================================================
// ETAT PARTAGE (survit aux pauses, partage entre workers)
// =============================================================================

/// Etat complet et thread-safe d'un crawl. Concu pour etre stocke dans l'etat
/// applicatif Tauri afin de SURVIVRE entre un `stop` (pause) et un `start`
/// (reprise). Cree par [`SharedState::new`], pilote par [`run_shared`].
pub struct SharedState {
    /// URL de depart : sert d'identite pour decider reprise vs nouveau crawl.
    pub seed_url: String,
    /// File BFS des URLs a traiter, avec leur profondeur.
    pub queue: Mutex<VecDeque<(String, usize)>>,
    /// URLs deja prises en charge (marquees AVANT traitement, anti-boucle).
    pub visited: Mutex<HashSet<String>>,
    /// Nombre de pages terminees (emises).
    pub done: AtomicUsize,
    /// Nombre de workers en train de traiter une page (detection de fin).
    pub active: AtomicUsize,
    /// Drapeau de pause : `true` => les workers finissent leur page puis sortent.
    pub stop: AtomicBool,
    /// `true` une fois la seed enfilee (= etat initialise / non vierge).
    pub started: AtomicBool,
    /// Agregats du resume (pages, totaux, technos) accumules au fil de l'eau.
    pub summary: Mutex<CrawlSummary>,
    /// Navigateur lance en lazy (une seule fois), partage entre workers.
    pub browser: Mutex<Option<chromiumoxide::Browser>>,
    /// `JoinHandle` de la task du navigateur (pour l'abandonner en fin de crawl).
    pub browser_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Compteur pour la rotation de user-agents (partage entre workers).
    pub ua_counter: AtomicU64,
    /// Compteur pour les delais aleatoires (partage entre workers).
    pub delay_counter: AtomicU64,
    /// Graine de base pour la rotation UA / les delais.
    pub ua_seed_base: u64,
}

impl SharedState {
    /// Cree un etat NEUF amorce avec la seed (file = [seed @ depth 0]).
    pub fn new(seed_url: &str) -> Self {
        let ua_seed_base: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678_9ABC_DEF0);

        let mut queue = VecDeque::new();
        queue.push_back((seed_url.to_string(), 0usize));

        SharedState {
            seed_url: seed_url.to_string(),
            queue: Mutex::new(queue),
            visited: Mutex::new(HashSet::new()),
            done: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            started: AtomicBool::new(true),
            summary: Mutex::new(CrawlSummary {
                pages: Vec::new(),
                total_bytes: 0,
                total_requests: 0,
                total_dynamic_requests: 0,
                tech_detected: Vec::new(),
                sitemap_urls: Vec::new(),
            }),
            browser: Mutex::new(None),
            browser_handle: Mutex::new(None),
            ua_counter: AtomicU64::new(0),
            delay_counter: AtomicU64::new(0),
            ua_seed_base,
        }
    }

    /// Demande la PAUSE : les workers terminent leur page courante puis sortent,
    /// sans toucher a la file / visited / summary (l'etat reste reprenable).
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Vrai si l'etat contient encore du travail (crawl pause au milieu) :
    /// la seed a ete amorcee ET il reste des URLs dans la file.
    pub async fn is_resumable(&self) -> bool {
        if !self.started.load(Ordering::SeqCst) {
            return false;
        }
        !self.queue.lock().await.is_empty()
    }

    /// Nombre de pages deja terminees (pour reprise / affichage).
    pub fn done_count(&self) -> usize {
        self.done.load(Ordering::SeqCst)
    }
}

/// Construit un `PageReport` a partir d'une sonde HTTP (mesure statique).
///
/// - `total_bytes` = octets du document (HTML) + octets de tous les assets statiques.
/// - `request_count` = 1 (document) + nombre d'assets telecharges.
/// - `dynamic_requests` = 1 si la page elle-meme est "dynamique" (route /api, RSC,
///   ou cache Vercel MISS/STALE/PRERENDER), 0 sinon.
/// - compteurs cache Vercel lus sur le header `x-vercel-cache` du document.
async fn build_http_report(
    client: &reqwest::Client,
    url: &str,
    status: u16,
    headers: &reqwest::header::HeaderMap,
    html: &str,
    tech: Vec<String>,
) -> PageReport {
    // Octets du document principal : Content-Length si annonce, sinon taille du HTML.
    let doc_bytes: u64 = header_lower(headers, "content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| html.len() as u64);

    // Assets statiques (images, scripts, CSS, fonts) telecharges en parallele.
    let (images_all, mut by_type, assets_bytes): (Vec<AssetReport>, _, u64) =
        crate::http_probe::measure_static_assets(client, url, html).await;

    // Ne garder que les images pour le champ `images` du rapport.
    let images: Vec<AssetReport> = images_all
        .into_iter()
        .filter(|a| a.kind == "Image")
        .collect();

    // Comptabilise le document principal dans by_type / total.
    let total_bytes = doc_bytes.saturating_add(assets_bytes);
    {
        let entry = by_type.entry("Document".to_string()).or_insert_with(TypeStat::default);
        entry.bytes = entry.bytes.saturating_add(doc_bytes);
        entry.count = entry.count.saturating_add(1);
    }

    // request_count = somme des count de by_type (document + assets).
    let request_count: u32 = by_type.values().map(|s| s.count).sum();

    // Cache Vercel sur le document principal.
    let vercel_cache = header_lower(headers, "x-vercel-cache");
    let mut vercel_cache_hits = 0u32;
    let mut vercel_cache_misses = 0u32;
    if let Some(vc) = vercel_cache.as_deref() {
        match vc {
            "hit" => vercel_cache_hits += 1,
            "miss" | "stale" | "prerender" => vercel_cache_misses += 1,
            _ => {}
        }
    }

    // Dynamique ? (la page elle-meme : /api, RSC, ou cache manque).
    let dynamic_requests =
        if crate::techno::is_dynamic_request(url, vercel_cache.as_deref()) { 1 } else { 0 };

    PageReport {
        url: url.to_string(),
        status,
        tech,
        total_bytes,
        by_type,
        request_count,
        dynamic_requests,
        vercel_cache_hits,
        vercel_cache_misses,
        images,
        measured_with: "http".to_string(),
        error: None,
        ..Default::default()
    }
}

// =============================================================================
// ENTREES PUBLIQUES
// =============================================================================

/// Lance un crawl AUTONOME (etat local jetable) — entree historique.
///
/// Cree un [`SharedState`] interne et delegue a [`run_shared`]. A utiliser quand
/// on n'a pas besoin de pause/reprise (tests, usage simple). Le navigateur est
/// ferme et l'etat detruit a la fin.
///
/// `on_progress` est appele apres chaque page traitee, `on_page` avec le
/// `PageReport` complet de chaque page. Les deux closures sont `Send + Sync +
/// 'static`. Retourne le `CrawlSummary` agrege.
pub async fn run<P, G>(
    config: CrawlConfig,
    on_progress: P,
    on_page: G,
) -> anyhow::Result<CrawlSummary>
where
    P: Fn(CrawlProgress) + Send + Sync + 'static,
    G: Fn(PageReport) + Send + Sync + 'static,
{
    let state = Arc::new(SharedState::new(&config.seed_url));
    run_shared(state, config, on_progress, on_page).await
}

/// Lance (ou REPREND) un crawl sur un [`SharedState`] PARTAGE.
///
/// - Etat NEUF (file = seed) -> nouveau crawl.
/// - Etat PAUSE non vide (file restante) -> REPREND la ou on s'etait arrete.
///
/// Spawn `config.threads` workers concurrents qui se partagent la file, sondent
/// les pages, emettent les callbacks et reinjectent les liens internes. Termine :
///   - NATURELLEMENT (file vide + 0 worker actif) -> finalise (ferme le navigateur,
///     dedup technos, recupere les sitemaps) et renvoie le resume complet ;
///   - sur PAUSE (`state.stop` leve) -> renvoie le resume PARTIEL accumule SANS
///     fermer le navigateur ni vider l'etat (reprise possible plus tard).
pub async fn run_shared<P, G>(
    state: Arc<SharedState>,
    config: CrawlConfig,
    on_progress: P,
    on_page: G,
) -> anyhow::Result<CrawlSummary>
where
    P: Fn(CrawlProgress) + Send + Sync + 'static,
    G: Fn(PageReport) + Send + Sync + 'static,
{
    // On (re)part d'un drapeau d'arret a false pour cette session de travail.
    state.stop.store(false, Ordering::SeqCst);
    state.started.store(true, Ordering::SeqCst);

    // --- Client HTTP par defaut : compression negociee + UA Chrome realiste.
    //     (Aucun cookie store n'est active : chaque requete part d'un etat vierge.) ---
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        // Redirects are followed manually in `http_probe::probe` so the full
        // chain (and the first 3xx code) is observable.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // --- Rate-limiter (governor 0.7) PARTAGE. On clamp a >= 1 req/s. ---
    let rps = config.requests_per_second.max(1);
    let quota = Quota::per_second(NonZeroU32::new(rps).unwrap());
    let limiter = Arc::new(RateLimiter::direct(quota));

    // --- Host de reference pour le filtre same_domain_only. ---
    let seed_host = host_of(&config.seed_url);

    // --- robots.txt : telecharge + parse une fois pour tout le crawl si demande.
    //     (Echec reseau / fichier absent -> Robots::empty() = tout autorise.) ---
    let robots = if config.respect_robots {
        crate::robots::fetch(&client, &config.seed_url, USER_AGENT).await
    } else {
        crate::robots::Robots::empty()
    };

    // --- Conteneurs partages (Arc) clones dans chaque worker. ---
    let client = Arc::new(client);
    let robots = Arc::new(robots);
    let config = Arc::new(config);
    let seed_host = Arc::new(seed_host);
    let on_progress = Arc::new(on_progress);
    let on_page = Arc::new(on_page);

    // Nombre de workers : au moins 1.
    let threads = config.threads.max(1);

    // --- Lancement des workers. ---
    let mut workers: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(threads);
    for _ in 0..threads {
        let state = Arc::clone(&state);
        let client = Arc::clone(&client);
        let limiter = Arc::clone(&limiter);
        let robots = Arc::clone(&robots);
        let config = Arc::clone(&config);
        let seed_host = Arc::clone(&seed_host);
        let on_progress = Arc::clone(&on_progress);
        let on_page = Arc::clone(&on_page);

        workers.push(tokio::spawn(async move {
            worker_loop(
                state, client, limiter, robots, config, seed_host, on_progress, on_page,
            )
            .await;
        }));
    }

    // --- Attente de la fin de tous les workers. ---
    for w in workers {
        let _ = w.await;
    }

    // --- Si on s'est arrete sur une PAUSE : ne pas finaliser, garder l'etat. ---
    if state.stop.load(Ordering::SeqCst) {
        // Renvoie le resume PARTIEL accumule (l'etat reste reprenable).
        let summary = state.summary.lock().await.clone();
        return Ok(summary);
    }

    // --- Fin NATURELLE : finalisation (une seule fois). ---

    // Fermeture propre du navigateur (si lance).
    {
        let mut browser_guard = state.browser.lock().await;
        if let Some(mut b) = browser_guard.take() {
            let _ = b.close().await;
            let _ = b.wait().await;
        }
    }
    {
        let mut handle_guard = state.browser_handle.lock().await;
        if let Some(h) = handle_guard.take() {
            h.abort();
        }
    }

    // Dedup final des technos + sitemaps, dans le summary partage.
    {
        let mut summary = state.summary.lock().await;
        let techs = std::mem::take(&mut summary.tech_detected);
        summary.tech_detected = dedup_keep_order(techs);
        // sitemap deplace au DEBUT du crawl (src-tauri/commands.rs + event crawl:sitemap)
    }

    let summary = state.summary.lock().await.clone();
    Ok(summary)
}

// =============================================================================
// BOUCLE WORKER
// =============================================================================

/// Boucle d'un worker : tire des URLs de la file partagee et les traite jusqu'a
/// epuisement (file vide + aucun worker actif) ou pause (`state.stop`).
#[allow(clippy::too_many_arguments)]
async fn worker_loop<P, G>(
    state: Arc<SharedState>,
    client: Arc<reqwest::Client>,
    limiter: Arc<
        RateLimiter<
            governor::state::direct::NotKeyed,
            governor::state::InMemoryState,
            governor::clock::DefaultClock,
        >,
    >,
    robots: Arc<crate::robots::Robots>,
    config: Arc<CrawlConfig>,
    seed_host: Arc<Option<String>>,
    on_progress: Arc<P>,
    on_page: Arc<G>,
) where
    P: Fn(CrawlProgress) + Send + Sync + 'static,
    G: Fn(PageReport) + Send + Sync + 'static,
{
    loop {
        // Pause demandee : on sort proprement (sans vider l'etat).
        if state.stop.load(Ordering::SeqCst) {
            return;
        }

        // Limite globale de pages atteinte : plus rien a faire.
        if !config.unlimited && state.done.load(Ordering::SeqCst) >= config.max_pages {
            return;
        }

        // --- Tirage d'une URL dans la file partagee. ---
        //
        // INVARIANT ANTI-RACE : on incremente `active` SOUS LE MEME LOCK que le
        // `pop_front` reussi. Ainsi un worker qui observe une file vide voit
        // forcement `active >= 1` tant qu'un autre worker, ayant tout juste
        // depile la derniere URL, n'a pas encore reinjecte ses liens et decremente
        // `active`. Sans ce couplage, un worker pourrait conclure a tort que le
        // crawl est fini (file vide + active==0 vus dans une fenetre de course).
        let next = {
            let mut q = state.queue.lock().await;
            // Borne stricte de pages : `done + active` = pages deja terminees +
            // pages en cours de traitement (reservees). Tant que ce total atteint
            // `max_pages`, on ne depile plus rien (evite le sur-crawl en parallele).
            let reserved = state.done.load(Ordering::SeqCst) + state.active.load(Ordering::SeqCst);
            if !config.unlimited && reserved >= config.max_pages {
                None
            } else {
                match q.pop_front() {
                    Some(item) => {
                        state.active.fetch_add(1, Ordering::SeqCst);
                        Some(item)
                    }
                    None => None,
                }
            }
        };

        let (url, depth) = match next {
            Some(item) => item,
            None => {
                // File vide. Si plus aucun worker n'est actif -> fin reelle.
                if state.active.load(Ordering::SeqCst) == 0 {
                    return;
                }
                // Sinon, d'autres workers peuvent encore reinjecter des liens :
                // on patiente brievement puis on re-essaie.
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            }
        };

        // Traite la page (sonde, mesure, emission, reinjection des liens).
        // On dereference les Arc (`&*`) pour passer des references aux valeurs
        // sous-jacentes : `&*on_progress` est `&P` (pas `&Arc<P>`), ce que la
        // signature generique de `process_one_page` attend.
        process_one_page(
            &state, &*client, &*limiter, &*robots, &*config, &*seed_host,
            &*on_progress, &*on_page, url, depth,
        )
        .await;

        // Page terminee : ce worker n'est plus actif.
        state.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Traite UNE page : robots, rate-limit/delai, sonde HTTP, decision Chrome vs
/// HTTP, enrichissement SEO/securite/issues, emission des callbacks, et
/// reinjection des liens internes dans la file partagee.
#[allow(clippy::too_many_arguments)]
async fn process_one_page<P, G>(
    state: &Arc<SharedState>,
    client: &reqwest::Client,
    limiter: &RateLimiter<
        governor::state::direct::NotKeyed,
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
    robots: &crate::robots::Robots,
    config: &CrawlConfig,
    seed_host: &Option<String>,
    on_progress: &P,
    on_page: &G,
    url: String,
    depth: usize,
) where
    P: Fn(CrawlProgress) + Send + Sync + 'static,
    G: Fn(PageReport) + Send + Sync + 'static,
{
    // Helper local : emet un rapport + met a jour le summary + emet la progression.
    async fn emit_report<P, G>(
        state: &Arc<SharedState>,
        on_page: &G,
        on_progress: &P,
        report: PageReport,
        url: &str,
    ) where
        P: Fn(CrawlProgress) + Send + Sync + 'static,
        G: Fn(PageReport) + Send + Sync + 'static,
    {
        on_page(report.clone());
        let (total_bytes, queued) = {
            let mut summary = state.summary.lock().await;
            merge_into_summary(&mut summary, &report);
            let tb = summary.total_bytes;
            drop(summary);
            let queued = state.queue.lock().await.len();
            (tb, queued)
        };
        let done = state.done.fetch_add(1, Ordering::SeqCst) + 1;
        on_progress(CrawlProgress {
            done,
            queued,
            current_url: url.to_string(),
            total_bytes_so_far: total_bytes,
        });
    }

    // Respect de robots.txt : si le path est interdit, on emet un rapport
    // "bloque" et on s'arrete la pour cette URL (sans la sonder ni l'enfiler).
    if config.respect_robots {
        let path = Url::parse(&url)
            .ok()
            .map(|u| {
                let mut p = u.path().to_string();
                if let Some(q) = u.query() {
                    p.push('?');
                    p.push_str(q);
                }
                p
            })
            .unwrap_or_else(|| url.clone());
        if !robots.allowed(&path) {
            // Marque comme visite pour ne pas re-evaluer en boucle.
            state.visited.lock().await.insert(url.clone());
            let report = PageReport {
                url: url.clone(),
                status: 0,
                measured_with: "http".to_string(),
                error: Some("Bloque par robots.txt".to_string()),
                ..Default::default()
            };
            emit_report(state, on_page, on_progress, report, &url).await;
            return;
        }
    }

    // Deja vu ? On marque AVANT traitement pour ne pas re-traiter en boucle.
    if !state.visited.lock().await.insert(url.clone()) {
        return;
    }

    // Respect du rythme : delai aleatoire "humain" si demande, sinon rate-limit fixe.
    if config.random_delay {
        let c = state.delay_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let r = rng_next(
            state
                .ua_seed_base
                .wrapping_add(c.wrapping_mul(0xD1B5_4A32_D192_ED03)),
        );
        let ms = 500 + (r % 4000); // 0,5 s a 4,5 s entre requetes
        tokio::time::sleep(Duration::from_millis(ms)).await;
    } else {
        limiter.until_ready().await;
    }

    // Profil UA pour cette page (rotation pseudo-aleatoire si demandee).
    let profile = if config.random_user_agent {
        let c = state.ua_counter.fetch_add(1, Ordering::SeqCst) + 1;
        Some(*crate::useragents::pick(
            state
                .ua_seed_base
                .wrapping_add(c.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        ))
    } else {
        None
    };

    // Client HTTP : un client dedie portant l'UA du profil, sinon le client global.
    let page_client = match &profile {
        Some(p) => reqwest::Client::builder()
            .user_agent(p.ua)
            // Same manual-redirect policy as the global client.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| client.clone()),
        None => client.clone(),
    };

    // 1. Sonde HTTP legere.
    let probe = match crate::http_probe::probe(&page_client, &url).await {
        Ok(p) => p,
        Err(e) => {
            // Page en erreur : on emet un rapport minimal et on continue.
            let report = PageReport {
                url: url.clone(),
                status: 0,
                tech: Vec::new(),
                total_bytes: 0,
                by_type: std::collections::HashMap::new(),
                request_count: 0,
                dynamic_requests: 0,
                vercel_cache_hits: 0,
                vercel_cache_misses: 0,
                images: Vec::new(),
                measured_with: "http".to_string(),
                error: Some(format!("sonde HTTP echouee: {e}")),
                ..Default::default()
            };
            emit_report(state, on_page, on_progress, report, &url).await;
            return;
        }
    };

    // 2. Detection de techno (headers + HTML).
    let tech = crate::techno::detect(&probe.headers, &probe.html);

    // 3. DECISION : navigateur (framework JS / force_chrome) ou HTTP statique.
    let use_chrome = config.force_chrome || needs_chrome(&tech);

    // On tente d'abord la mesure navigateur si demandee/necessaire, en notant
    // toute raison d'echec pour basculer proprement sur le repli HTTP statique.
    let mut report: Option<PageReport> = None;
    let mut chrome_error: Option<String> = None;

    if use_chrome {
        // Acces au navigateur partage : on serialise UNIQUEMENT les mesures Chrome.
        // Les sondes HTTP (majoritaires) restent paralleles.
        let mut browser_guard = state.browser.lock().await;

        // Lance le navigateur a la demande (lazy), une seule fois pour tout le crawl.
        if browser_guard.is_none() {
            match crate::chrome_manager::launch_browser().await {
                Ok((b, h)) => {
                    *browser_guard = Some(b);
                    *state.browser_handle.lock().await = Some(h);
                }
                Err(e) => {
                    chrome_error =
                        Some(format!("lancement Chrome impossible, repli HTTP statique: {e}"));
                }
            }
        }

        // Si le navigateur est dispo, on mesure reellement la page.
        if let Some(b) = browser_guard.as_ref() {
            match crate::chrome_probe::measure_page(b, &url, profile).await {
                Ok(r) => report = Some(r),
                Err(e) => {
                    chrome_error =
                        Some(format!("mesure Chrome echouee, repli HTTP statique: {e}"));
                }
            }
        }
        // Le guard est relache ici : les autres workers peuvent acceder au browser.
    }

    // Repli HTTP statique : pas de mesure navigateur (non demandee, navigateur
    // indisponible, ou mesure echouee). Une seule sonde d'assets, jamais deux.
    let mut report: PageReport = match report {
        Some(r) => r,
        None => {
            let mut http_report = build_http_report(
                &page_client,
                &url,
                probe.status,
                &probe.headers,
                &probe.html,
                tech.clone(),
            )
            .await;
            // Si on visait Chrome mais qu'on a du se replier, on documente pourquoi.
            if let Some(err) = chrome_error {
                http_report.error = Some(err);
            }
            http_report
        }
    };

    if report.seo.is_none() {
        report.seo = Some(crate::seo::extract_seo(&probe.html, &url, seed_host.as_deref()));
    }

    // Temps de reponse de la sonde HTTP (Vague 4 : mesure de vitesse).
    report.response_time_ms = probe.response_time_ms;

    // Liens sortants (avec ancre + rel) detectes sur la page.
    report.outlinks = probe.links_detailed.clone();

    // Chaine de redirection (si la sonde HTTP en a traverse une).
    //   - redirect_chain : URLs traversees (vide si pas de redirection) ;
    //   - redirect_to    : URL finale, si differente de l'URL demandee ;
    //   - status         : code de la 1ere reponse 3xx en cas de redirection,
    //                       sinon le statut de la reponse finale.
    if !probe.redirect_chain.is_empty() {
        report.redirect_chain = probe.redirect_chain.clone();
        if probe.final_url != url {
            report.redirect_to = Some(probe.final_url.clone());
        }
        if let Some(first) = probe.first_status {
            report.status = first;
        }
    }

    // Indicateurs URL + securite (Vague 2). Calcules AVANT seo::analyze
    // pour que la generation d'issues puisse en tenir compte plus tard.
    report.url_flags = Some(crate::urlcheck::analyze(&url));
    report.security = Some(crate::security::analyze(&url, &probe.headers, &probe.html));
    if let Some(seo) = report.seo.as_mut() {
        seo.x_robots_tag = probe
            .headers
            .get("x-robots-tag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
    }

    report.issues = crate::seo::analyze(&report);

    // 4. Reinjection des liens internes AVANT d'emettre (pour que `queued` soit a
    //    jour dans la progression). On respecte la profondeur restante.
    if depth < config.max_depth {
        let mut q = state.queue.lock().await;
        let visited = state.visited.lock().await;
        for link in &probe.links {
            // Filtre same-domain : meme host que le seed.
            if config.same_domain_only {
                match (host_of(link), seed_host.as_deref()) {
                    (Some(h), Some(seed)) if h == seed => {}
                    // Hote different, ou impossible a comparer : on saute.
                    _ => continue,
                }
            }
            if !visited.contains(link) {
                q.push_back((link.clone(), depth + 1));
            }
        }
    }

    // 5. Emission du rapport + progression.
    emit_report(state, on_page, on_progress, report, &url).await;
}

/// Cumule un `PageReport` dans le resume (pages, totaux, technos).
fn merge_into_summary(summary: &mut CrawlSummary, report: &PageReport) {
    summary.total_bytes = summary.total_bytes.saturating_add(report.total_bytes);
    summary.total_requests = summary.total_requests.saturating_add(report.request_count);
    summary.total_dynamic_requests = summary
        .total_dynamic_requests
        .saturating_add(report.dynamic_requests);
    for t in &report.tech {
        if !summary.tech_detected.iter().any(|x| x == t) {
            summary.tech_detected.push(t.clone());
        }
    }
    summary.pages.push(report.clone());
}

/// Deduplique un `Vec<String>` en conservant l'ordre de premiere apparition.
fn dedup_keep_order(items: Vec<String>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::with_capacity(items.len());
    for it in items {
        if seen.insert(it.clone()) {
            out.push(it);
        }
    }
    out
}
