//! Récupération des URLs déclarées dans le(s) sitemap(s) d'un site.
//!
//! Stratégie (sans dependance XML) :
//!   1. GET `{scheme}://{host}/sitemap.xml`.
//!   2. Si échec : GET `/sitemap_index.xml`.
//!   3. Si échec : lire `robots.txt` et suivre chaque ligne `Sitemap:`.
//! Le parsing extrait le contenu de chaque balise `<loc>...</loc>` par
//! recherche de sous-chaînes (pas de parser XML), decode les entites basiques
//! (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`/`&#39;`), et descend dans les
//! sitemap index (`<sitemapindex>`) en fetchant chaque sitemap enfant
//! (max 20). Le résultat est dedoublonne, limite a 50000 URLs http(s).
//! Toute erreur réseau / parsing renvoie une liste vide ; jamais de panic.

use std::collections::HashSet;

/// Nombre maximal de sitemaps enfants suivis depuis un sitemap index.
const MAX_CHILD_SITEMAPS: usize = 20;

/// Nombre maximal d'URLs de pages retournees.
const MAX_URLS: usize = 50_000;

/// Recupere les URLs listees dans le(s) sitemap(s) du site cible.
///
/// `client`   : client HTTP reutilise pour le crawl.
/// `seed_url` : URL de depart (sert a deriver l'origine du site).
pub async fn fetch(client: &reqwest::Client, seed_url: &str) -> Vec<String> {
    let parsed = match url::Url::parse(seed_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return Vec::new(),
    };
    let scheme = parsed.scheme();
    let origin = match parsed.port() {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    };

    // Liste ordonnee des points d'entrée sitemap a tenter.
    let mut entry_points: Vec<String> = vec![
        format!("{origin}/sitemap.xml"),
        format!("{origin}/sitemap_index.xml"),
    ];
    // Ajoute les sitemaps declares dans robots.txt (peut être vide).
    for sm in sitemaps_from_robots(client, &origin).await {
        if !entry_points.contains(&sm) {
            entry_points.push(sm);
        }
    }

    let mut pages: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Évite de re-fetcher deux fois la même URL de sitemap (index -> enfants).
    let mut fetched_sitemaps: HashSet<String> = HashSet::new();

    for entry in entry_points {
        if pages.len() >= MAX_URLS {
            break;
        }
        collect_from_sitemap(
            client,
            &entry,
            &mut pages,
            &mut seen,
            &mut fetched_sitemaps,
        )
        .await;
    }

    if pages.len() > MAX_URLS {
        pages.truncate(MAX_URLS);
    }
    pages
}

/// Télécharge un sitemap, et selon son type :
///   - sitemap index (`<sitemapindex>`) : suit chaque `<loc>` enfant (max 20) ;
///   - urlset normal : ajoute chaque `<loc>` (URL de page).
///
/// Mutualise `pages`/`seen` (dedup global) et `fetched_sitemaps` (anti-boucle).
async fn collect_from_sitemap(
    client: &reqwest::Client,
    sitemap_url: &str,
    pages: &mut Vec<String>,
    seen: &mut HashSet<String>,
    fetched_sitemaps: &mut HashSet<String>,
) {
    if pages.len() >= MAX_URLS {
        return;
    }
    if !fetched_sitemaps.insert(sitemap_url.to_string()) {
        return; // déjà traite
    }

    let body = match fetch_text(client, sitemap_url).await {
        Some(b) => b,
        None => return,
    };

    let locs = extract_locs(&body);

    if is_sitemap_index(&body) {
        // Chaque <loc> pointe vers un sitemap enfant.
        let mut children = 0usize;
        for child in locs {
            if children >= MAX_CHILD_SITEMAPS || pages.len() >= MAX_URLS {
                break;
            }
            if !is_http_url(&child) {
                continue;
            }
            children += 1;
            Box::pin(collect_from_sitemap(
                client,
                &child,
                pages,
                seen,
                fetched_sitemaps,
            ))
            .await;
        }
    } else {
        // urlset : chaque <loc> est une page.
        for loc in locs {
            if pages.len() >= MAX_URLS {
                break;
            }
            if !is_http_url(&loc) {
                continue;
            }
            if seen.insert(loc.clone()) {
                pages.push(loc);
            }
        }
    }
}

/// Lit `robots.txt` et renvoie les URLs déclarées via des lignes `Sitemap:`.
/// Renvoie un vecteur vide en cas d'échec (jamais de panic).
async fn sitemaps_from_robots(client: &reqwest::Client, origin: &str) -> Vec<String> {
    let robots_url = format!("{origin}/robots.txt");
    let body = match fetch_text(client, &robots_url).await {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        // Forme attendue : `Sitemap: https://...` (insensible à la casse).
        if let Some(rest) = strip_prefix_ci(line, "sitemap:") {
            let url = rest.trim();
            if is_http_url(url) {
                out.push(url.to_string());
            }
        }
    }
    out
}

/// GET une URL et renvoie le corps texte si la réponse est un succès (2xx).
/// Toute erreur (URL, réseau, statut non-2xx, lecture du corps) -> `None`.
async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.text().await.ok()
}

/// Detecte un sitemap index à partir de la présence de la balise ouvrante
/// `<sitemapindex` (insensible à la casse).
fn is_sitemap_index(body: &str) -> bool {
    contains_ci(body, "<sitemapindex")
}

/// Extrait le contenu (trim + entites decodees) de chaque balise
/// `<loc>...</loc>` du document, par recherche de sous-chaînes. Tolere les
/// espaces et attributs eventuels (`<loc >`, `<loc xml:...>`).
fn extract_locs(body: &str) -> Vec<String> {
    let lower = body.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut search_from = 0usize;

    while let Some(rel_open) = lower[search_from..].find("<loc") {
        let open_tag = search_from + rel_open;
        // Trouve la fin de la balise ouvrante `>`.
        let after_open = match lower[open_tag..].find('>') {
            Some(gt) => open_tag + gt + 1,
            None => break, // balise ouvrante non terminée -> stop
        };
        // Trouve la balise fermante `</loc>`.
        let rel_close = match lower[after_open..].find("</loc>") {
            Some(c) => c,
            None => break,
        };
        let close_start = after_open + rel_close;
        let inner = &body[after_open..close_start];
        let decoded = decode_entities(inner.trim());
        if !decoded.is_empty() {
            out.push(decoded);
        }
        search_from = close_start + "</loc>".len();
    }
    out
}

/// Decode les entites XML/HTML basiques rencontrees dans une URL de sitemap.
/// Ordre important : `&amp;` doit être traite en dernier pour ne pas
/// re-decoder les `&` issus des autres remplacements.
fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

/// Vrai si l'URL commence par `http://` ou `https://` (insensible à la casse).
fn is_http_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// `str::contains` insensible à la casse (alloue une copie en minuscules).
fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_ascii_lowercase().contains(needle_lower)
}

/// `str::strip_prefix` insensible à la casse. `prefix_lower` doit être fourni
/// déjà en minuscules. Renvoie le reste de la chaîne (après le prefixe).
fn strip_prefix_ci<'a>(s: &'a str, prefix_lower: &str) -> Option<&'a str> {
    if s.len() < prefix_lower.len() {
        return None;
    }
    let head = &s[..prefix_lower.len()];
    if head.eq_ignore_ascii_case(prefix_lower) {
        Some(&s[prefix_lower.len()..])
    } else {
        None
    }
}

/// Variante autonome : crée son propre client HTTP. Pratique pour un appel
/// HORS crawl (ex. au demarrage côté Tauri, pour remplir l'onglet Sitemap tout
/// de suite). Suit jusqu'à 5 redirections.
///
/// `connect_timeout` court (8 s, aligne sur `crawler::CONNECT_TIMEOUT`) : ce
/// fetch precede le crawl, donc sur un domaine dont le DNS ne resout pas il
/// retarde d'autant le passage au site suivant en mode multi-sites.
pub async fn fetch_default(seed_url: &str) -> Vec<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();
    fetch(&client, seed_url).await
}
