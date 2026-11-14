//! Orchestrateur du crawl — version CONCURRENTE (Vague 4).
//!
//! Pilote une exploration en largeur (BFS) à partir d'une URL seed, mais traite
//! desormais jusqu'à `config.threads` pages EN PARALLELE :
//!   1. Sonde HTTP légère de chaque page (`http_probe::probe`) -> statut, headers,
//!      HTML, liens, temps de réponse.
//!   2. Détection de techno (`techno::detect`).
//!   3. DECISION de mesure :
//!        - si `force_chrome` OU framework JS détecté -> mesure NAVIGATEUR réelle
//!          (`chrome_probe::measure_page`, bande passante CDP) ;
//!        - sinon -> mesure HTTP statique (`http_probe::measure_static_assets`).
//!   4. Notifications via callbacks `on_page` puis `on_progress`.
//!   5. Reinjection des liens internes (même host si `same_domain_only`) tant que
//!      `depth < max_depth`, dans la file PARTAGEE.
//!
//! ## Concurrence
//!
//! Tout l'état muable est porte par [`SharedState`], thread-safe :
//!   - `queue`   : file BFS `(url, depth)` derriere un `tokio::sync::Mutex` ;
//!   - `visited` : ensemble des URLs déjà prises en charge (`Mutex<HashSet>`) ;
//!   - `done` / `active` : compteurs atomiques (pages terminées / workers occupés) ;
//!   - `summary` : agrégats du resume derriere un `Mutex` ;
//!   - `browser` : navigateur lance en lazy UNE fois, derriere un
//!     `tokio::sync::Mutex<Option<Browser>>` — les sondes HTTP (majoritaires)
//!     restent paralleles, seules les mesures Chrome se serialisent.
//!
//! `config.threads` workers `tokio::spawn` tournent en boucle : ils tirent une URL
//! de la file, la traitent, et reinjectent ses liens. La fin est détectée quand la
//! file est vide ET plus aucun worker n'est actif.
//!
//! ## Pause / reprise
//!
//! [`SharedState`] est concu pour SURVIVRE entre deux appels : `stop()` leve un
//! drapeau `stop` (les workers finissent leur page courante puis sortent SANS
//! vider la file/visited/summary). Un appel ulterieur a [`run_shared`] sur le même
//! `SharedState` REPREND la ou on s'etait arrete (workers relances sur la file
//! existante). [`run`] reste l'entrée autonome historique (état local jetable).

use std::collections::{HashSet, VecDeque};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use governor::{Quota, RateLimiter};
use tokio::sync::Mutex;
use url::Url;

use crate::codes::PageError;
use crate::report::{
    AssetReport, CrawlConfig, CrawlProgress, CrawlSummary, PageReport, TypeStat,
};

/// User-agent par defaut (utilise quand la rotation aléatoire est désactivée).
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/145.0.0.0 Safari/537.36";

/// Débit utilise quand `config.no_rate_limit` est vrai : assez haut pour que le
/// bucket ne bloque jamais, sans valeur extreme qui ferait deborder les calculs
/// d'intervalle de `governor`.
const NO_LIMIT_RPS: u32 = 100_000;

/// Délai maximal pour ÉTABLIR la connexion (resolution DNS + TCP + TLS).
///
/// CRITIQUE en mode multi-sites : sans ce plafond, un domaine dont le DNS ne
/// resout pas bloque jusqu'au timeout du système (30-40 s), et ce TROIS fois
/// par site (sitemap, robots.txt, puis la seed). Sur une liste de 150 domaines
/// dont beaucoup sont morts, l'import passait ses heures a attendre.
///
/// 8 s est très large : un serveur vivant repond au connect en moins d'une
/// seconde, même charge.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Délai maximal pour une requête COMPLÈTE (connexion + réponse + corps).
///
/// Plus genereux que `CONNECT_TIMEOUT` : une page lourde sur un mutualise
/// fatigue peut legitimement mettre plusieurs secondes a rendre son HTML. On
/// coupe ce qui ne repondra jamais, pas ce qui est lent.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Plafond ABSOLU de pages, applique MÊME en mode "Tout le site" (`unlimited`).
///
/// Ce n'est PAS une limite de capacite : c'est un garde-fou contre les pieges a
/// crawler. Sur un site a facettes (`?couleur=X&taille=Y`) ou a calendrier, le
/// nombre d'URLs distinctes est quasi infini — sans plafond le crawl ne
/// s'arrete jamais de lui-même, l'app finit tuee par l'OOM killer, et le crawl
/// est INTÉGRALEMENT perdu puisque la sauvegarde n'a lieu qu'à la fin.
///
/// Le risque mémoire est réel car chaque page est gardée intégralement côté JS
/// (`S.pages`), un `PageReport` portant ses outlinks, ses H2, ses images et le
/// JSON-LD brut. (Jusqu'au 14/11/2026 elle l'était aussi côté Rust, dans
/// `summary.pages`, pour être renvoyée en bloc dans `crawl:done` : ce n'est plus
/// le cas, cf. `CrawlSummary::tally`.)
///
/// **Valeur calibree sur des crawls RÉELS** (et non sur une estimation) :
/// 11,5 a 15,0 Ko par page en JSON serialise, mesures sur trois projets
/// enregistres dont un de 4 035 pages (59 Mo). A 200 000 pages : ~3 Go de JSON,
/// soit plusieurs Go en mémoire dans la fenêtre une fois compte l'overhead des
/// objets. Aucun crawl legitime n'approche ce volume ; un piege a crawler, si.
///
/// Le vrai correctif reste a faire : sauvegarde incrementale (pour qu'un
/// plantage ne coute plus le crawl entier) puis stockage disque facon
/// "Database Storage Mode" de Screaming Frog. Cf. AUDIT-SCREAMING-FROG.md.
const HARD_MAX_PAGES: usize = 200_000;

/// Frameworks dont la présence déclenché la mesure NAVIGATEUR (rendu/hydratation
/// côté client : la mesure HTTP statique sous-estimerait la bande passante réelle).
///
/// **React et Vue en ont été RETIRES volontairement.** Leur détection
/// (`techno.rs`) se déclenché sur un simple `data-reactroot` / `#root` + bundle,
/// ce qui matche un WordPress avec Gutenberg ou un Astro avec une seule ile
/// React. Or la mesure Chrome est SERIALISEE (un seul navigateur derriere un
/// Mutex) : un seul faux positif faisait tomber tout le crawl à une page à la
/// fois, threads et débit ignores.
///
/// Restent les meta-frameworks, ou le rendu client est structurel. Pour une SPA
/// React/Vue pure, utiliser la case "Forcer Chrome" (`config.force_chrome`).
const JS_FRAMEWORKS: [&str; 5] = ["Next.js", "Nuxt", "Gatsby", "SvelteKit", "Remix"];

/// Vrai si l'une des technos détectées correspond à un framework JS connu.
fn needs_chrome(tech: &[String]) -> bool {
    tech.iter().any(|t| JS_FRAMEWORKS.contains(&t.as_str()))
}

/// Vrai si la réponse est un DEFI anti-bot (Cloudflare et equivalents).
///
/// Un defi n'est pas une panne du site : c'est une page intermediaire qui exige
/// l'exécution d'un JavaScript pour delivrer un cookie de passage. Une sonde HTTP
/// pure ne peut pas la franchir et enregistre un 403 trompeur — le site repond
/// parfaitement 200 des qu'on l'interroge en direct.
///
/// Trois signaux, du plus fiable au plus general :
///   1. l'en-tête `cf-mitigated: challenge`, que Cloudflare pose explicitement ;
///   2. le titre « Just a moment... » de sa page d'attente ;
///   3. un 403 servi par `server: cloudflare` sur un corps minuscule.
///
/// Quand c'est vrai, on bascule sur Chrome : un vrai navigateur exécute le defi,
/// obtient le cookie, et le magasin de cookies du client le reutilise ensuite pour
/// tout le reste du crawl.
fn is_bot_challenge(status: u16, headers: &reqwest::header::HeaderMap, html: &str) -> bool {
    // 1. Signal explicite de Cloudflare.
    if let Some(v) = header_lower(headers, "cf-mitigated") {
        if v.contains("challenge") {
            return true;
        }
    }

    // 2. Page d'attente reconnaissable.
    if status == 403 || status == 503 {
        // `len()` compte des OCTETS, pas des caractères : couper à l'aveugle au
        // 2048e ferait paniquer le worker si la borne tombe au milieu d'un
        // caractère multi-octets — un seul accent dans une page 403 francaise
        // suffit. On recule jusqu'à la dernière frontière valide (3 tours au
        // pire, un caractère UTF-8 pesant au plus 4 octets).
        let mut cut = html.len().min(2048);
        while cut > 0 && !html.is_char_boundary(cut) {
            cut -= 1;
        }
        let head = html[..cut].to_ascii_lowercase();
        if head.contains("just a moment")
            || head.contains("cf-browser-vérification")
            || head.contains("challenge-platform")
        {
            return true;
        }
    }

    // 3. 403 très court servi par Cloudflare : signature d'une interstitielle.
    if status == 403 && html.len() < 8192 {
        if let Some(server) = header_lower(headers, "server") {
            if server.contains("cloudflare") {
                return true;
            }
        }
    }

    false
}

/// En-têtes par defaut d'une NAVIGATION, ajoutes a tous les clients HTTP.
///
/// Un agent utilisateur realiste ne suffit pas : Cloudflare juge une requête sur
/// son `Accept`. Sans en-tête `Accept` de document, il traite l'appel comme une
/// sonde et OMET son `server-timing` — celui qui expose `cfOrigin;dur=0`, seule
/// facon de savoir si l'origine a travaille sur un hebergement de peripherie
/// (Pages, Workers), ou aucun `cf-cache-status` n'est emis.
///
/// Vérifié le 24/08/2026 sur www.fuiterecherche.com : a UA identique, la
/// présence du seul `Accept` fait apparaitre `server-timing`. Ni
/// `Accept-Encoding`, ni `Accept-Language`, ni les `Sec-Fetch-*` n'ont cet effet.
///
/// La valeur est celle de Chrome pour un document. Le `*/*;q=0.8` final laisse
/// passer images, scripts et feuilles de style, que le même client télécharge
/// pour mesurer le poids des pages.
fn browser_headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    h.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        reqwest::header::HeaderValue::from_static("fr-FR,fr;q=0.9,en;q=0.8"),
    );
    h
}

/// Petit generateur xorshift64 (pour les délais aléatoires "humains").
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

/// Lit la valeur d'un header (insensible à la casse) en minuscules.
fn header_lower(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())
}

// =============================================================================
// ÉTAT PARTAGE (survit aux pauses, partage entre workers)
// =============================================================================

/// État complet et thread-safe d'un crawl. Concu pour être stocke dans l'état
/// applicatif Tauri afin de SURVIVRE entre un `stop` (pause) et un `start`
/// (reprise). Cree par [`SharedState::new`], pilote par [`run_shared`].
pub struct SharedState {
    /// URL de départ : sert d'identite pour decider reprise vs nouveau crawl.
    pub seed_url: String,
    /// File BFS des URLs a traiter, avec leur profondeur.
    pub queue: Mutex<VecDeque<(String, usize)>>,
    /// URLs déjà prises en charge (marquees AVANT traitement, anti-boucle).
    pub visited: Mutex<HashSet<String>>,
    /// URLs déjà ENFILEES au moins une fois (seed comprise) — dedoublonnage de
    /// la file contre elle-même.
    ///
    /// `visited` ne se remplit qu'au DEPILAGE : entre le moment ou une URL entre
    /// dans la file et celui ou un worker la traite, chaque page qui la
    /// référence la reenfilait. Sur un site de 5 000 pages a menu commun, la
    /// file croissait en O(pages x liens) au lieu de O(URLs distinctes) et la
    /// même page etait re-sondee plusieurs fois.
    ///
    /// C'est un SUR-ENSEMBLE de `visited` (toute URL depilee a forcement été
    /// enfilee), donc il suffit seul a decider de l'enfilage : voir la
    /// reinjection dans `process_one_page`.
    pub queued_set: Mutex<HashSet<String>>,
    /// Nombre de pages terminées (emises).
    pub done: AtomicUsize,
    /// Nombre de workers en train de traiter une page (détection de fin).
    pub active: AtomicUsize,
    /// Drapeau de pause : `true` => les workers finissent leur page puis sortent.
    pub stop: AtomicBool,
    /// `true` une fois la seed enfilee (= état initialise / non vierge).
    pub started: AtomicBool,
    /// Agrégats du resume (pages, totaux, technos) accumules au fil de l'eau.
    pub summary: Mutex<CrawlSummary>,
    /// Navigateur lance en lazy (une seule fois), partage entre workers.
    pub browser: Mutex<Option<chromiumoxide::Browser>>,
    /// `JoinHandle` de la task du navigateur (pour l'abandonner en fin de crawl).
    pub browser_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Poids des assets statiques déjà mesures (`url -> octets`), partage par
    /// tous les workers : le CSS/JS commun a tout le site n'est télécharge
    /// qu'UNE fois pour l'ensemble du crawl. Cf. [`crate::http_probe::AssetCache`].
    pub asset_cache: crate::http_probe::AssetCache,
    /// Compteur pour la rotation de user-agents (partage entre workers).
    pub ua_counter: AtomicU64,
    /// Compteur pour les délais aléatoires (partage entre workers).
    pub delay_counter: AtomicU64,
    /// Graine de base pour la rotation UA / les délais.
    pub ua_seed_base: u64,
}

/// Retire le fragment (`#...`) d'une URL de départ.
///
/// Le fragment n'est jamais envoye au serveur : le garder dans la seed la
/// rendrait différente (au sens de `visited`) des liens extraits du HTML, qui
/// eux sont nettoyes par `http_probe`. Coller `https://site.fr/#accueil` dans
/// la barre d'URL crawlerait donc l'accueil DEUX fois.
pub fn strip_fragment(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut u) => {
            u.set_fragment(None);
            u.into()
        }
        // URL non parsable : on la laisse telle quelle (le crawl echouera plus
        // loin avec un message explicite, comme avant).
        Err(_) => raw.to_string(),
    }
}

impl SharedState {
    /// Cree un état NEUF amorce avec la seed (file = [seed @ depth 0]).
    pub fn new(seed_url: &str) -> Self {
        let seed_url = strip_fragment(seed_url);
        let seed_url = seed_url.as_str();

        let ua_seed_base: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x1234_5678_9ABC_DEF0);

        let mut queue = VecDeque::new();
        queue.push_back((seed_url.to_string(), 0usize));

        // La seed entre dans la file SANS passer par la reinjection : elle doit
        // être inscrite ici, sinon un lien vers l'accueil la reenfilerait.
        let mut queued = HashSet::new();
        queued.insert(seed_url.to_string());

        SharedState {
            seed_url: seed_url.to_string(),
            queue: Mutex::new(queue),
            visited: Mutex::new(HashSet::new()),
            queued_set: Mutex::new(queued),
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
            asset_cache: Mutex::new(std::collections::HashMap::new()),
            ua_counter: AtomicU64::new(0),
            delay_counter: AtomicU64::new(0),
            ua_seed_base,
        }
    }

    /// Demande la PAUSE : les workers terminent leur page courante puis sortent,
    /// sans toucher à la file / visited / summary (l'état reste reprenable).
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Vrai si l'état contient encore du travail (crawl pause au milieu) :
    /// la seed a été amorcee ET il reste des URLs dans la file.
    pub async fn is_resumable(&self) -> bool {
        if !self.started.load(Ordering::SeqCst) {
            return false;
        }
        !self.queue.lock().await.is_empty()
    }

    /// Nombre de pages déjà terminées (pour reprise / affichage).
    pub fn done_count(&self) -> usize {
        self.done.load(Ordering::SeqCst)
    }
}

/// Construit un `PageReport` à partir d'une sonde HTTP (mesure statique).
///
/// - `total_bytes` = octets du document (HTML) + octets de tous les assets statiques.
/// - `request_count` = 1 (document) + nombre d'assets telecharges.
/// - `dynamic_requests` = 1 si la page elle-même est "dynamique" (route /api, RSC,
///   ou cache Vercel MISS/STALE/PRERENDER), 0 sinon.
/// - compteurs cache Vercel lus sur le header `x-vercel-cache` du document.
async fn build_http_report(
    client: &reqwest::Client,
    url: &str,
    status: u16,
    headers: &reqwest::header::HeaderMap,
    html: &str,
    tech: Vec<String>,
    asset_cache: &crate::http_probe::AssetCache,
) -> PageReport {
    // Octets du document principal : Content-Length si annonce, sinon taille du HTML.
    let doc_bytes: u64 = header_lower(headers, "content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| html.len() as u64);

    // Assets statiques (images, scripts, CSS, fonts) telecharges en parallele.
    let (images_all, mut by_type, assets_bytes): (Vec<AssetReport>, _, u64) =
        crate::http_probe::measure_static_assets(client, url, html, asset_cache).await;

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

    // Dynamique ? (la page elle-même : /api, RSC, ou cache manqué).
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
// ENTRÉES PUBLIQUES
// =============================================================================

/// Lance un crawl AUTONOME (état local jetable) — entrée historique.
///
/// Cree un [`SharedState`] interne et delegue a [`run_shared`]. A utiliser quand
/// on n'a pas besoin de pause/reprise (tests, usage simple). Le navigateur est
/// ferme et l'état detruit à la fin.
///
/// `on_progress` est appele après chaque page traitée, `on_page` avec le
/// `PageReport` complet de chaque page. Les deux closures sont `Send + Sync +
/// 'static`. Retourne le `CrawlSummary` agrégé : totaux et technos, SANS les
/// pages — chacune n'a été remise qu'une fois, à `on_page` (cf.
/// `CrawlSummary::tally`).
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
/// - État NEUF (file = seed) -> nouveau crawl.
/// - État PAUSE non vide (file restante) -> REPREND la ou on s'etait arrete.
///
/// Spawn `config.threads` workers concurrents qui se partagent la file, sondent
/// les pages, emettent les callbacks et reinjectent les liens internes. Termine :
///   - NATURELLEMENT (file vide + 0 worker actif) -> finalise (ferme le navigateur,
///     dedup technos, recupere les sitemaps) et renvoie le resume complet ;
///   - sur PAUSE (`state.stop` leve) -> renvoie le resume PARTIEL accumule SANS
///     fermer le navigateur ni vider l'état (reprise possible plus tard).
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
    // On (re)part d'un drapeau d'arrêt a false pour cette session de travail.
    state.stop.store(false, Ordering::SeqCst);
    state.started.store(true, Ordering::SeqCst);

    // --- Client HTTP par defaut : compression negociee + UA Chrome realiste.
    //
    //     COOKIE STORE ACTIVE (auparavant : aucun, chaque requête partait vierge).
    //
    //     Motif : les defis anti-bot fonctionnent en DEUX temps. La première
    //     requête recoit un 403 accompagne d'un cookie de passage — `cf_clearance`
    //     chez Cloudflare — et les requêtes suivantes doivent le presenter. Sans
    //     magasin de cookies, ce jeton etait jete aussitot reçu : chaque URL du
    //     crawl repartait de zero et recoltait un 403, y compris après que Chrome
    //     eut résolu le defi.
    //
    //     Mesure du 08/08/2026 sur un parc de 7 sites derriere Cloudflare :
    //     200 depuis l'origine sur les 7, mais 403 via Cloudflare sur 5 d'entre
    //     eux, tous avec l'en-tête `cf-mitigated: challenge`. L'agent utilisateur
    //     n'y etait pour rien : Chrome/145, Screaming Frog et un UA vide donnaient
    //     le même résultat.
    //
    //     Le magasin est propre au client, donc au crawl : rien ne persiste d'une
    //     exécution à l'autre. ---
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .default_headers(browser_headers())
        // Redirects are followed manually in `http_probe::probe` so the full
        // chain (and the first 3xx code) is observable.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    // --- Rate-limiter (governor 0.7) PARTAGE. On clamp a >= 1 req/s. ---
    //
    // ATTENTION : c'est un bucket GLOBAL (`RateLimiter::direct`), pas un quota
    // par worker. Tant qu'il est actif, le débit du crawl entier est plafonne a
    // `requests_per_second` et le réglage "Threads" n'a plus AUCUN effet des que
    // `threads > rps * temps_de_reponse` (avec 5 rps et 80 ms de réponse, il
    // suffit de 0,4 worker pour saturer le quota).
    //
    // `no_rate_limit` leve ce plafond : on garde la même mecanique (aucune
    // signature ne change) mais avec un quota assez grand pour que
    // `until_ready()` rende la main immédiatement. La vitesse est alors bornée
    // par `threads` et le serveur cible, comme dans Screaming Frog.
    // --- Host de référence pour le filtre same_domain_only. ---
    let seed_host = host_of(&config.seed_url);

    // --- robots.txt : télécharge + parse une fois pour tout le crawl si demande.
    //     (Échec réseau / fichier absent -> Robots::empty() = tout autorise.)
    //     Recupere AVANT de construire le limiteur : son `Crawl-delay` en fixe
    //     la cadence. ---
    let robots = if config.respect_robots {
        crate::robots::fetch(&client, &config.seed_url, USER_AGENT).await
    } else {
        crate::robots::Robots::empty()
    };

    // Le `Crawl-delay` du serveur PRIME sur les réglages, "Débit illimité"
    // compris : cocher "Respecter robots.txt" doit vouloir dire respecter
    // robots.txt en entier, pas seulement ses `Disallow`. Sans ca, un site
    // mutualise declarant `Crawl-delay: 10` recevrait jusqu'à `threads`
    // requêtes simultanees — saturation, bannissement d'IP ou WAF, sur le site
    // d'un client, alors que l'utilisateur croit le menager.
    // Le `Crawl-delay` est un PLAFOND, jamais un plancher : on prend le `min`
    // avec le réglage utilisateur.
    //
    // Le prendre seul ACCELERERAIT un utilisateur prudent : règle a 2 rps sur un
    // mutualise fragile, un `Crawl-delay: 0.1` (= 10 rps tolerees) l'aurait fait
    // passer a 10 rps — le correctif anti-nuisance aurait créé la nuisance qu'il
    // visait. Le `min` borne aussi le cast `as u32` : un `Crawl-delay`
    // minuscule saturerait sinon vers `u32::MAX`.
    let user_rps = if config.no_rate_limit {
        NO_LIMIT_RPS
    } else {
        config.requests_per_second.max(1)
    };
    let rps = match robots.crawl_delay {
        Some(d) if d > 0.0 => user_rps.min(((1.0 / d).ceil() as u32).max(1)),
        _ => user_rps,
    };
    let quota = Quota::per_second(NonZeroU32::new(rps).unwrap());
    let limiter = Arc::new(RateLimiter::direct(quota));

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

    // --- Si on s'est arrete sur une PAUSE : ne pas finaliser, garder l'état. ---
    if state.stop.load(Ordering::SeqCst) {
        // Renvoie le resume PARTIEL accumule (l'état reste reprenable).
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
        // sitemap deplace au DÉBUT du crawl (src-tauri/commands.rs + event crawl:sitemap)
    }

    // Clone LÉGER : le résumé ne porte que des totaux et les technos, jamais les
    // pages (cf. `CrawlSummary::tally`). Il clonait autrefois toutes les pages
    // du crawl, juste avant de les renvoyer en bloc dans `crawl:done`.
    let summary = state.summary.lock().await.clone();
    Ok(summary)
}

// =============================================================================
// BOUCLE WORKER
// =============================================================================

/// Boucle d'un worker : tire des URLs de la file partagee et les traite jusqu'à
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
        // Pause demandee : on sort proprement (sans vider l'état).
        if state.stop.load(Ordering::SeqCst) {
            return;
        }

        // Limite globale de pages atteinte : plus rien a faire.
        // `HARD_MAX_PAGES` s'applique MÊME en mode illimité (garde-fou mémoire).
        let done_now = state.done.load(Ordering::SeqCst);
        if done_now >= HARD_MAX_PAGES || (!config.unlimited && done_now >= config.max_pages) {
            return;
        }

        // --- Tirage d'une URL dans la file partagee. ---
        //
        // INVARIANT ANTI-RACE : on incremente `active` SOUS LE MÊME LOCK que le
        // `pop_front` réussi. Ainsi un worker qui observe une file vide voit
        // forcement `active >= 1` tant qu'un autre worker, ayant tout juste
        // depile la dernière URL, n'a pas encore reinjecte ses liens et decremente
        // `active`. Sans ce couplage, un worker pourrait conclure a tort que le
        // crawl est fini (file vide + active==0 vus dans une fenêtre de course).
        let next = {
            let mut q = state.queue.lock().await;
            // Borne stricte de pages : `done + active` = pages déjà terminées +
            // pages en cours de traitement (réservées). Tant que ce total atteint
            // `max_pages`, on ne depile plus rien (évite le sur-crawl en parallele).
            let reserved = state.done.load(Ordering::SeqCst) + state.active.load(Ordering::SeqCst);
            if reserved >= HARD_MAX_PAGES || (!config.unlimited && reserved >= config.max_pages) {
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
                // File vide. Si plus aucun worker n'est actif -> fin réelle.
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
        // On dereference les Arc (`&*`) pour passer des références aux valeurs
        // sous-jacentes : `&*on_progress` est `&P` (pas `&Arc<P>`), ce que la
        // signature generique de `process_one_page` attend.
        process_one_page(
            &state, &*client, &*limiter, &*robots, &*config, &*seed_host,
            &*on_progress, &*on_page, url, depth,
        )
        .await;

        // Page terminée : ce worker n'est plus actif.
        state.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Emet UNE page terminée : totaux du résumé, page remise à l'interface,
/// progression.
///
/// La page n'est NI copiée NI gardée par le moteur : seuls ses totaux entrent
/// dans le résumé (`CrawlSummary::tally`), puis elle part telle quelle à
/// `on_page`, qui la remet à l'interface (`crawl:page`) — c'est l'interface qui
/// garde les pages. Jusqu'au 14/11/2026, le moteur en gardait une copie
/// intégrale dans `summary.pages` pendant et après le crawl, la clonait en fin
/// de crawl et la renvoyait en bloc dans `crawl:done`, que l'interface ignorait :
/// 722 Mo gardés, 536 Mo de clone et un script de 812 Mo pour un crawl de 415 Mo
/// (cf. `tests/memoire_fin_de_crawl.rs`).
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
    let (total_bytes, queued) = {
        let mut summary = state.summary.lock().await;
        summary.tally(&report);
        let tb = summary.total_bytes;
        drop(summary);
        let queued = state.queue.lock().await.len();
        (tb, queued)
    };
    on_page(report);
    let done = state.done.fetch_add(1, Ordering::SeqCst) + 1;
    on_progress(CrawlProgress {
        done,
        queued,
        current_url: url.to_string(),
        total_bytes_so_far: total_bytes,
        // Le worker courant est encore compte actif (il ne se decremente
        // qu'au retour de `process_one_page`), ce qui est bien ce qu'on veut
        // afficher : le nombre de workers en train de travailler.
        active_workers: state.active.load(Ordering::SeqCst),
    });
}

/// Traite UNE page : robots, rate-limit/délai, sonde HTTP, decision Chrome vs
/// HTTP, enrichissement SEO/sécurité/issues, emission des callbacks, et
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
    // Déjà vu ? On marque AVANT TOUT traitement, contrôle robots inclus.
    //
    // Cette garde etait AUTREFOIS placee après le contrôle robots : une URL
    // interdite enfilee N fois (la file n'est pas dedoublonnee contre
    // elle-même) etait re-emise N fois, car le contrôle robots ne consulte pas
    // `visited`. `done` etant incremente à chaque emission et pilotant la borne
    // `max_pages`, le crawl s'arretait prematurement en croyant avoir traite
    // des pages fantomes.
    if !state.visited.lock().await.insert(url.clone()) {
        return;
    }

    // Respect de robots.txt : si le path est interdit, on emet un rapport
    // "bloqué" et on s'arrête la pour cette URL (sans la sonder ni l'enfiler).
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
            // (déjà marque visite ci-dessus : une seule emission par URL)
            let mut report = PageReport {
                url: url.clone(),
                status: 0,
                measured_with: "http".to_string(),
                ..Default::default()
            };
            // « Bloque par robots.txt » + son code (cf. `codes::PageError`).
            report.set_error(&PageError::RobotsBlocked);
            emit_report(state, on_page, on_progress, report, &url).await;
            return;
        }
    }

    // Respect du rythme : délai aléatoire "humain" si demande, sinon rate-limit fixe.
    if config.random_delay {
        let c = state.delay_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let r = rng_next(
            state
                .ua_seed_base
                .wrapping_add(c.wrapping_mul(0xD1B5_4A32_D192_ED03)),
        );
        let ms = 500 + (r % 4000); // 0,5 s a 4,5 s entre requêtes
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
    // Le quota s'applique TOUJOURS, délai aléatoire compris : c'est lui qui
    // porte le `Crawl-delay` du serveur. Le mettre en `else` le rendait
    // contournable — 5 workers en délai aléatoire montaient a ~2 req/s sur un
    // site demandant 0,1 req/s. Les deux se cumulent : le plus contraignant
    // gagne, et hors `Crawl-delay` le quota reste celui choisi par l'utilisateur.
    limiter.until_ready().await;

    // Profil UA pour cette page (rotation pseudo-aléatoire si demandee).
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

    // Client HTTP : un client dédié portant l'UA du profil, sinon le client global.
    let page_client = match &profile {
        Some(p) => reqwest::Client::builder()
            .user_agent(p.ua)
            // Mêmes en-têtes de navigation que le client global : sans eux,
            // cocher "UA aléatoire" ferait disparaitre le `server-timing` de
            // Cloudflare, donc la colonne Cache sur tout hebergement Pages.
            .default_headers(browser_headers())
            // Same manual-redirect policy AND same timeouts as the global
            // client : sans ca, la rotation d'UA reintroduirait l'attente
            // illimitée sur un hote injoignable.
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| client.clone()),
        None => client.clone(),
    };

    // 1. Sonde HTTP légère.
    let probe = match crate::http_probe::probe(&page_client, &url).await {
        Ok(p) => p,
        Err(e) => {
            // Page en erreur : on emet un rapport minimal et on continue.
            let mut report = PageReport {
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
                ..Default::default()
            };
            // « sonde HTTP echouee: {e} » + son code ; `e` (reqwest) reste tel quel.
            report.set_error(&PageError::HttpProbeFailed(e.to_string()));
            emit_report(state, on_page, on_progress, report, &url).await;
            return;
        }
    };

    // 2. Détection de techno (headers + HTML).
    let tech = crate::techno::detect(&probe.headers, &probe.html);

    // 3. DECISION : navigateur (framework JS / defi anti-bot / force_chrome)
    //    ou HTTP statique.
    //
    //    Le defi anti-bot est la troisieme raison de passer par Chrome, et la
    //    seule qui ne depende pas de la techno du site : sans lui, la sonde HTTP
    //    enregistrerait un 403 alors que la page est parfaitement servie. Chrome
    //    exécute le defi, et le cookie obtenu profite ensuite a tout le crawl
    //    grace au magasin de cookies du client.
    let challenged = is_bot_challenge(probe.status, &probe.headers, &probe.html);
    let use_chrome = config.force_chrome || needs_chrome(&tech) || challenged;

    // On tente d'abord la mesure navigateur si demandee/nécessaire, en notant
    // toute raison d'échec pour basculer proprement sur le repli HTTP statique.
    let mut report: Option<PageReport> = None;
    let mut chrome_error: Option<PageError> = None;

    if use_chrome {
        // Accès au navigateur partage : on serialise UNIQUEMENT les mesures Chrome.
        // Les sondes HTTP (majoritaires) restent paralleles.
        let mut browser_guard = state.browser.lock().await;

        // Lance le navigateur à la demande (lazy), une seule fois pour tout le crawl.
        if browser_guard.is_none() {
            match crate::chrome_manager::launch_browser().await {
                Ok((b, h)) => {
                    *browser_guard = Some(b);
                    *state.browser_handle.lock().await = Some(h);
                }
                Err(e) => {
                    chrome_error = Some(PageError::ChromeLaunchFailed(e.to_string()));
                }
            }
        }

        // Si le navigateur est dispo, on mesure réellement la page.
        if let Some(b) = browser_guard.as_ref() {
            match crate::chrome_probe::measure_page(b, &url, profile).await {
                Ok(r) => report = Some(r),
                Err(e) => {
                    chrome_error = Some(PageError::ChromeMeasureFailed(e.to_string()));
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
                &state.asset_cache,
            )
            .await;
            // Si on visait Chrome mais qu'on a du se replier, on documente pourquoi
            // (« lancement Chrome impossible, repli HTTP statique: {e} » ou
            // « mesure Chrome echouee, repli HTTP statique: {e} », + leur code).
            if let Some(err) = chrome_error {
                http_report.set_error(&err);
            }
            http_report
        }
    };

    if report.seo.is_none() {
        report.seo = Some(crate::seo::extract_seo(&probe.html, &url, seed_host.as_deref()));
    }

    // Temps de réponse de la sonde HTTP (Vague 4 : mesure de vitesse).
    report.response_time_ms = probe.response_time_ms;

    // Liens sortants (avec ancre + rel) detectes sur la page.
    report.outlinks = probe.links_detailed.clone();

    // Chaîne de redirection (si la sonde HTTP en a traverse une).
    //   - redirect_chain : URLs traversees (vide si pas de redirection) ;
    //   - redirect_to    : URL finale, si différente de l'URL demandée ;
    //   - status         : code de la 1ere réponse 3xx en cas de redirection,
    //                       sinon le statut de la réponse finale.
    if !probe.redirect_chain.is_empty() {
        report.redirect_chain = probe.redirect_chain.clone();
        if probe.final_url != url {
            report.redirect_to = Some(probe.final_url.clone());
        }
        if let Some(first) = probe.first_status {
            report.status = first;
        }
    }

    // Indicateurs URL + sécurité (Vague 2). Calcules AVANT seo::analyze
    // pour que la génération d'issues puisse en tenir compte plus tard.
    report.url_flags = Some(crate::urlcheck::analyze(&url));
    report.security = Some(crate::security::analyze(&url, &probe.headers, &probe.html));

    // Statut de cache (HIT/MISS/...) lu sur les en-têtes de la sonde HTTP.
    //
    // Pose ICI, dans la zone commune aux deux chemins de mesure : la sonde HTTP
    // precede TOUJOURS la mesure navigateur, donc la colonne est renseignee des
    // la première URL, y compris sur les pages finalement mesurées par Chrome.
    //
    // Sur une chaîne de redirection, ce sont les en-têtes de la réponse FINALE
    // (cf. `http_probe::probe`) : le statut decrit la page réellement servie,
    // pas le 301 qui y menait.
    //
    // Le HTML est passe en second : WP Rocket seul (sans son module serveur)
    // n'emet aucun en-tête et ne se reconnait qu'à sa signature en fin de
    // document.
    report.cache = crate::cachestat::analyze(&probe.headers, &probe.html);
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
        // ORDRE DES VERROUS : `queue` puis `queued_set`, jamais l'inverse.
        // C'est le seul imbriquement de verrous du crawler — le conserver tel
        // quel garantit l'absence de cycle, donc de deadlock.
        //
        // `visited` n'est PLUS consulte ici : `queued_set` en est un
        // sur-ensemble (toute URL depilee a d'abord été enfilee), donc son
        // `insert` decide seul.
        let mut q = state.queue.lock().await;
        let mut queued = state.queued_set.lock().await;

        // La DESTINATION d'une redirection est enfilee explicitement.
        //
        // Ne pas compter sur les ancres de la page pour l'y amener : une page
        // sans lien vers elle-même (one-pager sans ancre, page de contenu pur)
        // ne serait alors JAMAIS crawlée en 200 et le rapport n'en garderait
        // qu'une ligne 301 — l'onglet Sitemap la marquerait même "orpheline".
        // `queued_set` garantit une seule sonde pour tout le crawl, donc aucun
        // trafic double. Profondeur INCHANGEE : c'est la même ressource
        // logique, une redirection ne coute pas un niveau.
        if probe.final_url != url {
            let same_host = !config.same_domain_only
                || matches!(
                    (host_of(&probe.final_url), seed_host.as_deref()),
                    (Some(h), Some(seed)) if h == seed
                );
            if same_host && queued.insert(probe.final_url.clone()) {
                q.push_back((probe.final_url.clone(), depth));
            }
        }

        for link in &probe.links {
            // Une page ne se REENFILE JAMAIS elle-même, sous sa propre URL.
            //
            // Les liens sont resolus contre l'URL FINALE, donc sans redirection
            // (`url == final_url`) toute ancre `#...` resout vers `url` et est
            // filtree ici.
            //
            // NE PAS filtrer aussi `final_url` : quand une redirection a été
            // traversee, c'est une URL DISTINCTE, qui doit être enfilee une fois
            // pour être rapportee en 200. La filtrer supprimerait non pas un
            // reenfilage redondant (`queued_set` s'en charge déjà) mais le SEUL
            // enfilage — sur un site dont les liens internes omettent le slash
            // final (`/x` -> 301 -> `/x/`), le rapport ne contiendrait alors QUE
            // des 301 et pas une seule page canonique.
            //
            // `/contact` (301) et `/contact/` (200) doivent rester deux lignes
            // distinctes : c'est ce qu'attend un audit de redirections, et c'est
            // ce que fait Screaming Frog. Une fois `/contact/` depilee, ses
            // propres ancres resolvent vers elle-même et retombent sur le filtre
            // ci-dessus : aucune boucle possible.
            if *link == url {
                continue;
            }
            // Filtre same-domain : même host que le seed.
            if config.same_domain_only {
                match (host_of(link), seed_host.as_deref()) {
                    (Some(h), Some(seed)) if h == seed => {}
                    // Hote different, ou impossible a comparer : on saute.
                    _ => continue,
                }
            }
            // `insert` renvoie false si l'URL a DÉJÀ été enfilee (par cette page
            // ou par n'importe quelle autre) : une URL n'entre dans la file
            // qu'UNE seule fois pour tout le crawl.
            if queued.insert(link.clone()) {
                q.push_back((link.clone(), depth + 1));
            }
        }
    }

    // 5. Emission du rapport + progression.
    emit_report(state, on_page, on_progress, report, &url).await;
}

/// Dédupliqué un `Vec<String>` en conservant l'ordre de première apparition.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// La seed doit être nettoyee de son fragment, sinon elle ne correspond plus
    /// aux liens extraits du HTML (eux aussi nettoyes) et l'accueil serait
    /// crawle deux fois. Invariant dont depend aussi la reprise de crawl, qui
    /// compare `existing.seed_url == config.seed_url` (`commands.rs`).
    #[test]
    fn strip_fragment_nettoie_la_seed() {
        assert_eq!(strip_fragment("https://site.fr/#accueil"), "https://site.fr/");
        assert_eq!(
            strip_fragment("https://site.fr/page/?a=1#bas"),
            "https://site.fr/page/?a=1"
        );
        // Déjà propre : inchangee (au détail pres de la normalisation `Url`).
        assert_eq!(strip_fragment("https://site.fr/page/"), "https://site.fr/page/");
        // Non parsable : passthrough, l'erreur sera signalee plus loin.
        assert_eq!(strip_fragment("pas une url"), "pas une url");
    }

    /// `SharedState::new` doit appliquer la MÊME normalisation que
    /// `commands.rs`, sinon la comparaison de reprise ne matcherait jamais.
    #[test]
    fn shared_state_normalise_la_seed() {
        let seed = "https://site.fr/#accueil";
        let state = SharedState::new(seed);
        assert_eq!(state.seed_url, strip_fragment(seed));
        assert_eq!(state.seed_url, "https://site.fr/");
    }

    /// Verrouille le choix volontaire : React/Vue seuls ne doivent PLUS
    /// déclencher la mesure Chrome (qui est serialisee et effondrait le crawl
    /// sur un WordPress+Gutenberg ou un Astro a ile React), mais les
    /// meta-frameworks SSR, si.
    #[test]
    fn needs_chrome_ignore_react_et_vue() {
        assert!(!needs_chrome(&["React".to_string()]));
        assert!(!needs_chrome(&["Vue".to_string()]));
        assert!(!needs_chrome(&["WordPress".to_string(), "React".to_string()]));

        assert!(needs_chrome(&["Next.js".to_string()]));
        assert!(needs_chrome(&["Nuxt".to_string()]));
        assert!(needs_chrome(&["Gatsby".to_string()]));
        assert!(needs_chrome(&["SvelteKit".to_string()]));
        assert!(needs_chrome(&["Remix".to_string()]));

        // Un meta-framework accompagne de React déclenché quand même Chrome.
        assert!(needs_chrome(&["React".to_string(), "Next.js".to_string()]));
        assert!(!needs_chrome(&[]));
    }

    // --- Point 8 (14/11/2026) : le moteur ne garde plus les pages ------------

    /// `emit_report` remet chaque page UNE fois à l'interface et n'en garde que
    /// les totaux : le résumé n'a plus de pages, et ses totaux sont exacts.
    #[tokio::test]
    async fn emit_report_remet_la_page_sans_la_garder() {
        let state = Arc::new(SharedState::new("https://site.fr/"));
        let recues = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let progres = Arc::new(std::sync::Mutex::new(Vec::<(usize, u64)>::new()));
        let r = Arc::clone(&recues);
        let on_page = move |p: PageReport| r.lock().unwrap().push(p.url);
        let g = Arc::clone(&progres);
        let on_progress = move |p: CrawlProgress| g.lock().unwrap().push((p.done, p.total_bytes_so_far));
        for i in 0..3u64 {
            let url = format!("https://site.fr/{i}/");
            let report = PageReport {
                url: url.clone(),
                total_bytes: 100 * (i + 1),
                request_count: 2,
                dynamic_requests: 1,
                tech: vec!["WordPress".to_string(), if i == 2 { "Cloudflare" } else { "WordPress" }.to_string()],
                ..Default::default()
            };
            emit_report(&state, &on_page, &on_progress, report, &url).await;
        }
        let s = state.summary.lock().await;
        assert!(s.pages.is_empty(), "le moteur garde des pages : {}", s.pages.len());
        assert_eq!(s.total_bytes, 600);
        assert_eq!(s.total_requests, 6);
        assert_eq!(s.total_dynamic_requests, 3);
        assert_eq!(s.tech_detected, vec!["WordPress", "Cloudflare"]);
        assert_eq!(*recues.lock().unwrap(), vec!["https://site.fr/0/", "https://site.fr/1/", "https://site.fr/2/"]);
        // Progression inchangée : page par page, totaux cumulés page comprise.
        assert_eq!(*progres.lock().unwrap(), vec![(1, 100), (2, 300), (3, 600)]);
        assert_eq!(state.done_count(), 3);
    }

    /// Bout en bout, sur un petit site servi en local : `run` remet les trois
    /// pages à `on_page` et rend un résumé SANS pages (celui de `crawl:done`),
    /// dont la sérialisation ne grossit plus avec le crawl.
    #[tokio::test]
    async fn run_rend_un_resume_sans_pages() {
        use std::io::{BufRead, BufReader, Write};
        let ecoute = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ecoute.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for flux in ecoute.incoming().take(16) {
                let Ok(mut flux) = flux else { continue };
                let mut chemin = String::new();
                let mut lecteur = BufReader::new(flux.try_clone().unwrap());
                let mut ligne = String::new();
                while lecteur.read_line(&mut ligne).map(|n| n > 0).unwrap_or(false) {
                    if chemin.is_empty() {
                        chemin = ligne.split_whitespace().nth(1).unwrap_or("").to_string();
                    }
                    if ligne == "\r\n" {
                        break;
                    }
                    ligne.clear();
                }
                let corps = match chemin.as_str() {
                    "/" => r#"<html><head><title>Accueil du site de test local</title></head><body><h1>Accueil</h1><a href="/a/">A</a> <a href="/b/">B</a></body></html>"#,
                    "/a/" | "/b/" => r#"<html><head><title>Une page du site de test local</title></head><body><h1>Page</h1></body></html>"#,
                    _ => "",
                };
                let statut = if corps.is_empty() { "404 Not Found" } else { "200 OK" };
                let _ = write!(
                    flux,
                    "HTTP/1.1 {statut}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corps}",
                    corps.len()
                );
            }
        });
        let config = CrawlConfig {
            seed_url: format!("http://127.0.0.1:{port}/"),
            max_pages: 10,
            max_depth: 3,
            same_domain_only: true,
            requests_per_second: 100,
            force_chrome: false,
            random_user_agent: false,
            random_delay: false,
            unlimited: false,
            respect_robots: true,
            threads: 2,
            no_rate_limit: true,
        };
        let recues = Arc::new(std::sync::Mutex::new(Vec::<PageReport>::new()));
        let r = Arc::clone(&recues);
        let summary = run(config, |_| {}, move |p| r.lock().unwrap().push(p)).await.unwrap();

        let mut urls: Vec<String> = recues.lock().unwrap().iter().map(|p| p.url.clone()).collect();
        urls.sort();
        assert_eq!(urls, vec![format!("http://127.0.0.1:{port}/"), format!("http://127.0.0.1:{port}/a/"), format!("http://127.0.0.1:{port}/b/")]);
        assert!(recues.lock().unwrap().iter().all(|p| p.status == 200 && p.seo.is_some()));
        assert!(summary.pages.is_empty(), "crawl:done porterait {} pages", summary.pages.len());
        let octets_pages: u64 = recues.lock().unwrap().iter().map(|p| p.total_bytes).sum();
        assert_eq!(summary.total_bytes, octets_pages);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.len() < 300, "crawl:done = {} octets : {json}", json.len());
        assert!(json.contains("\"pages\":[]"), "{json}");
    }
}
