//! Recuperation des URLs declarees dans le(s) sitemap(s) d'un site.
//!
//! Strategie (sans dependance XML) :
//!   1. GET `{scheme}://{host}/sitemap.xml`.
//!   2. Si echec : GET `/sitemap_index.xml`.
//!   3. Si echec : lire `robots.txt` et suivre chaque ligne `Sitemap:`.
//! Le parsing extrait le contenu de chaque balise `<loc>...</loc>` par
//! recherche de sous-chaines (pas de parser XML), decode les entites basiques
//! (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`/`&#39;`), et descend dans les
//! sitemap index (`<sitemapindex>`) en fetchant chaque sitemap enfant
//! (max 20). Le resultat est dedoublonne, limite a 50000 URLs http(s).
//! Toute erreur reseau / parsing renvoie une liste vide ; jamais de panic.

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

    // Liste ordonnee des points d'entree sitemap a tenter.
    let mut entry_points: Vec<String> = vec![
        format!("{origin}/sitemap.xml"),
        format!("{origin}/sitemap_index.xml"),
    ];
    // Ajoute les sitemaps declares dans robots.txt (peut etre vide).
    for sm in sitemaps_from_robots(client, &origin).await {
        if !entry_points.contains(&sm) {
            entry_points.push(sm);
        }
    }

    let mut pages: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Evite de re-fetcher deux fois la meme URL de sitemap (index -> enfants).
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

/// Telecharge un sitemap, et selon son type :
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
        return; // deja traite
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

/// Lit `robots.txt` et renvoie les URLs declarees via des lignes `Sitemap:`.
/// Renvoie un vecteur vide en cas d'echec (jamais de panic).
async fn sitemaps_from_robots(client: &reqwest::Client, origin: &str) -> Vec<String> {
    let robots_url = format!("{origin}/robots.txt");
    let body = match fetch_text(client, &robots_url).await {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        // Forme attendue : `Sitemap: https://...` (insensible a la casse).
        if let Some(rest) = strip_prefix_ci(line, "sitemap:") {
            let url = rest.trim();
            if is_http_url(url) {
                out.push(url.to_string());
            }
        }
    }
    out
}

/// GET une URL et renvoie le corps texte si la reponse est un succes (2xx).
/// Toute erreur (URL, reseau, statut non-2xx, lecture du corps) -> `None`.
async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.text().await.ok()
}

/// Detecte un sitemap index a partir de la presence de la balise ouvrante
/// `<sitemapindex` (insensible a la casse).
fn is_sitemap_index(body: &str) -> bool {
    contains_ci(body, "<sitemapindex")
}

/// Extrait le contenu (trim + entites decodees) de chaque balise
/// `<loc>...</loc>` du document, par recherche de sous-chaines. Tolere les
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
            None => break, // balise ouvrante non terminee -> stop
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
/// Ordre important : `&amp;` doit etre traite en dernier pour ne pas
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

/// Vrai si l'URL commence par `http://` ou `https://` (insensible a la casse).
fn is_http_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// `str::contains` insensible a la casse (alloue une copie en minuscules).
fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_ascii_lowercase().contains(needle_lower)
}

/// `str::strip_prefix` insensible a la casse. `prefix_lower` doit etre fourni
/// deja en minuscules. Renvoie le reste de la chaine (apres le prefixe).
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
