//! Orchestrateur du crawl.
//!
//! Pilote une exploration en largeur (BFS) a partir d'une URL seed :
//!   1. Sonde HTTP legere de chaque page (`http_probe::probe`) -> statut, headers,
//!      HTML, liens.
//!   2. Detection de techno (`techno::detect`).
//!   3. DECISION de mesure :
//!        - si `force_chrome` OU framework JS detecte -> mesure NAVIGATEUR reelle
//!          (`chrome_probe::measure_page`, bande passante CDP) ;
//!        - sinon -> mesure HTTP statique (`http_probe::measure_static_assets`).
//!   4. Notifications via callbacks `on_page` puis `on_progress`.
//!   5. Enfilement des liens internes (meme host si `same_domain_only`) tant que
//!      `depth < max_depth`.
//!
//! Le navigateur n'est lance qu'a la PREMIERE page qui en a besoin (lazy), une
//! seule fois pour tout le crawl, et ferme proprement a la fin.
//!
//! Rotation de user-agents : si `config.random_user_agent`, chaque page est
//! crawlee avec un profil (UA + viewport) tire du pool `useragents`. La sonde
//! HTTP utilise alors un client reqwest dedie portant cet UA ; la sonde
//! navigateur applique l'emulation correspondante.

use std::collections::{HashSet, VecDeque};
use std::num::NonZeroU32;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use governor::{Quota, RateLimiter};
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

/// Lance le crawl complet.
///
/// `on_progress` est appele apres chaque page traitee (avancement), `on_page`
/// avec le `PageReport` complet de chaque page. Les deux closures sont `Send +
/// Sync + 'static` pour pouvoir etre portees dans une task Tokio cote Tauri.
///
/// Retourne un `CrawlSummary` agrege (toutes les pages, totaux, technos sans
/// doublon dans l'ordre de premiere apparition).
pub async fn run<P, G>(
    config: CrawlConfig,
    on_progress: P,
    on_page: G,
) -> anyhow::Result<CrawlSummary>
where
    P: Fn(CrawlProgress) + Send + Sync + 'static,
    G: Fn(PageReport) + Send + Sync + 'static,
{
    // --- Client HTTP par defaut : compression negociee + UA Chrome realiste.
    //     (Aucun cookie store n'est active : chaque requete part d'un etat vierge.) ---
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        // Redirects are followed manually in `http_probe::probe` so the full
        // chain (and the first 3xx code) is observable.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    // --- Graine pour la rotation de user-agents (si activee). ---
    let ua_seed_base: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x1234_5678_9ABC_DEF0);
    let mut ua_counter: u64 = 0;
    let mut delay_counter: u64 = 0;

    // --- Rate-limiter (governor 0.7). On clamp a >= 1 req/s (le type est u32 ;
    //     une valeur 0 floorerait le quota a zero et bloquerait le crawl). ---
    let rps = config.requests_per_second.max(1);
    let quota = Quota::per_second(NonZeroU32::new(rps).unwrap());
    let limiter = RateLimiter::direct(quota);

    // --- Host de reference pour le filtre same_domain_only. ---
    let seed_host = host_of(&config.seed_url);

    // --- Etat de la BFS. ---
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((config.seed_url.clone(), 0));
    let mut visited: HashSet<String> = HashSet::new();

    // --- Navigateur lance en lazy (une seule fois pour tout le crawl). ---
    let mut browser: Option<chromiumoxide::Browser> = None;
    let mut browser_handle: Option<tokio::task::JoinHandle<()>> = None;

    // --- Agregats du resume. ---
    let mut summary = CrawlSummary {
        pages: Vec::new(),
        total_bytes: 0,
        total_requests: 0,
        total_dynamic_requests: 0,
        tech_detected: Vec::new(),
        sitemap_urls: Vec::new(),
    };
    let mut done: usize = 0;

    // --- robots.txt : telecharge + parse une fois pour tout le crawl si demande.
    //     (Echec reseau / fichier absent -> Robots::empty() = tout autorise.) ---
    let robots = if config.respect_robots {
        crate::robots::fetch(&client, &config.seed_url, USER_AGENT).await
    } else {
        crate::robots::Robots::empty()
    };

    while let Some((url, depth)) = queue.pop_front() {
        if !config.unlimited && done >= config.max_pages {
            break;
        }

        // Respect de robots.txt : si le path est interdit, on emet un rapport
        // "bloque" et on passe a l'URL suivante (sans la sonder ni l'enfiler).
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
                visited.insert(url.clone());
                let report = PageReport {
                    url: url.clone(),
                    status: 0,
                    measured_with: "http".to_string(),
                    error: Some("Bloque par robots.txt".to_string()),
                    ..Default::default()
                };
                on_page(report.clone());
                done += 1;
                merge_into_summary(&mut summary, &report);
                on_progress(CrawlProgress {
                    done,
                    queued: queue.len(),
                    current_url: url.clone(),
                    total_bytes_so_far: summary.total_bytes,
                });
                continue;
            }
        }

        // Deja vu ? On marque AVANT traitement pour ne pas re-enfiler en boucle.
        if !visited.insert(url.clone()) {
            continue;
        }

        // Respect du rythme : delai aleatoire "humain" si demande, sinon rate-limit fixe.
        if config.random_delay {
            delay_counter = delay_counter.wrapping_add(1);
            let r = rng_next(ua_seed_base.wrapping_add(delay_counter.wrapping_mul(0xD1B5_4A32_D192_ED03)));
            let ms = 500 + (r % 4000); // 0,5 s a 4,5 s entre requetes
            tokio::time::sleep(Duration::from_millis(ms)).await;
        } else {
            limiter.until_ready().await;
        }

        // Profil UA pour cette page (rotation pseudo-aleatoire si demandee).
        let profile = if config.random_user_agent {
            ua_counter = ua_counter.wrapping_add(1);
            Some(*crate::useragents::pick(
                ua_seed_base.wrapping_add(ua_counter.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
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
                on_page(report.clone());
                done += 1;
                merge_into_summary(&mut summary, &report);
                on_progress(CrawlProgress {
                    done,
                    queued: queue.len(),
                    current_url: url.clone(),
                    total_bytes_so_far: summary.total_bytes,
                });
                continue;
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
            // Lance le navigateur a la demande (lazy), une seule fois pour tout le crawl.
            if browser.is_none() {
                match crate::chrome_manager::launch_browser().await {
                    Ok((b, h)) => {
                        browser = Some(b);
                        browser_handle = Some(h);
                    }
                    Err(e) => {
                        chrome_error =
                            Some(format!("lancement Chrome impossible, repli HTTP statique: {e}"));
                    }
                }
            }

            // Si le navigateur est dispo, on mesure reellement la page.
            if let Some(b) = browser.as_ref() {
                match crate::chrome_probe::measure_page(b, &url, profile).await {
                    Ok(r) => report = Some(r),
                    Err(e) => {
                        chrome_error =
                            Some(format!("mesure Chrome echouee, repli HTTP statique: {e}"));
                    }
                }
            }
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

        // 4. Notifications + agregation.
        on_page(report.clone());
        done += 1;
        merge_into_summary(&mut summary, &report);
        on_progress(CrawlProgress {
            done,
            queued: queue.len(),
            current_url: url.clone(),
            total_bytes_so_far: summary.total_bytes,
        });

        // 5. Enfilement des liens internes (si profondeur restante).
        if depth < config.max_depth {
            for link in probe.links {
                // Filtre same-domain : meme host que le seed.
                if config.same_domain_only {
                    match (host_of(&link), seed_host.as_deref()) {
                        (Some(h), Some(seed)) if h == seed => {}
                        // Hote different, ou impossible a comparer : on saute.
                        _ => continue,
                    }
                }
                if !visited.contains(&link) {
                    queue.push_back((link, depth + 1));
                }
            }
        }
    }

    // --- Fermeture propre du navigateur (si lance). ---
    if let Some(mut b) = browser.take() {
        let _ = b.close().await;
        let _ = b.wait().await;
    }
    if let Some(h) = browser_handle.take() {
        h.abort();
    }

    // --- Dedup final des technos (ordre de premiere apparition). ---
    summary.tech_detected = dedup_keep_order(summary.tech_detected);

    // --- Sitemaps : recupere la liste des URLs declarees dans le(s) sitemap(s). ---
    summary.sitemap_urls = crate::sitemap::fetch(&client, &config.seed_url).await;

    Ok(summary)
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
