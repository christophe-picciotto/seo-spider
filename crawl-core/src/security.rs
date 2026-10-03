//! Analyse sécurité d'une page (HTTPS, mixed content, en-têtes manquants).
//!
//! `analyze` inspecte l'URL, les en-têtes HTTP de la réponse et le HTML pour
//! produire une `SecurityData` : statut HTTPS, ressources http:// chargees sur
//! une page https (mixed content) et en-têtes de sécurité recommandes absents.

use scraper::{Html, Selector};

use crate::report::SecurityData;

/// En-têtes de sécurité recommandes verifies par `analyze`. HSTS
/// (`Strict-Transport-Security`) n'est signale que sur une page HTTPS.
const SECURITY_HEADERS: &[&str] = &[
    "Strict-Transport-Security",
    "Content-Security-Policy",
    "X-Content-Type-Options",
    "X-Frame-Options",
    "Referrer-Policy",
];

/// Parse un selecteur CSS constant (toujours valide ici).
fn sel(s: &str) -> Option<Selector> {
    Selector::parse(s).ok()
}

/// Une valeur d'attribut est-elle une ressource http:// non sécurisée ?
/// (insensible à la casse, tolere les espaces de tête).
fn is_insecure_resource(value: &str) -> bool {
    let v = value.trim();
    let lower = v.to_ascii_lowercase();
    lower.starts_with("http://")
}

/// Collecte les ressources http:// referencees par un selecteur/attribut donne.
fn collect_insecure(
    document: &Html,
    selector: &str,
    attr: &str,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    let Some(s) = sel(selector) else { return };
    for el in document.select(&s) {
        if let Some(val) = el.value().attr(attr) {
            if is_insecure_resource(val) {
                let url = val.trim().to_string();
                if seen.insert(url.clone()) {
                    out.push(url);
                }
            }
        }
    }
}

/// Analyse la sécurité d'une page à partir de son URL, ses en-têtes HTTP
/// et son HTML.
///
/// - `https` : l'URL commence par `https://` (insensible à la casse) ;
/// - `mixed_content` : si la page est en HTTPS, liste dédupliquée des
///   ressources `http://` referencees dans le HTML (src/href des img, script,
///   link). Vide si la page est servie en HTTP ;
/// - `missing_headers` : parmi les en-têtes de sécurité recommandes, ceux
///   absents de la réponse (comparaison insensible à la casse). HSTS n'est
///   signale que sur une page HTTPS.
pub fn analyze(
    url: &str,
    headers: &reqwest::header::HeaderMap,
    html: &str,
) -> SecurityData {
    let https = url.trim().to_ascii_lowercase().starts_with("https://");

    // --- mixed content (uniquement si la page elle-même est en HTTPS) ------
    let mut mixed_content: Vec<String> = Vec::new();
    if https {
        let document = Html::parse_document(html);
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Ressources actives et passives susceptibles de déclencher du mixed content.
        collect_insecure(&document, "img[src]", "src", &mut seen, &mut mixed_content);
        collect_insecure(&document, "script[src]", "src", &mut seen, &mut mixed_content);
        collect_insecure(&document, "link[href]", "href", &mut seen, &mut mixed_content);
        collect_insecure(&document, "iframe[src]", "src", &mut seen, &mut mixed_content);
        collect_insecure(&document, "audio[src]", "src", &mut seen, &mut mixed_content);
        collect_insecure(&document, "video[src]", "src", &mut seen, &mut mixed_content);
        collect_insecure(&document, "source[src]", "src", &mut seen, &mut mixed_content);
    }

    // --- en-têtes de sécurité manquants -----------------------------------
    let mut missing_headers: Vec<String> = Vec::new();
    for header in SECURITY_HEADERS {
        // HSTS n'a de sens qu'en HTTPS : on ne le signale pas sur une page HTTP.
        if header.eq_ignore_ascii_case("Strict-Transport-Security") && !https {
            continue;
        }
        // reqwest::HeaderMap::contains_key est insensible à la casse.
        if !headers.contains_key(*header) {
            missing_headers.push((*header).to_string());
        }
    }

    SecurityData {
        https,
        mixed_content,
        missing_headers,
    }
}
