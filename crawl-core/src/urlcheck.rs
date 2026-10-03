//! Analyse de la qualité d'une URL (longueur, non-ascii, underscores, etc.).
//!
//! `analyze` produit une `UrlFlags` decrivant des problèmes courants d'URL
//! reperes par les outils SEO (URL trop longue, caractères non-ASCII,
//! underscores dans le chemin, majuscules, doubles slashes, paramètres).

use url::Url;

use crate::report::UrlFlags;

/// Analyse une URL et renvoie des indicateurs de qualité.
///
/// - `length` : longueur (en octets) de l'URL complète ;
/// - `non_ascii` : l'URL complète contient au moins un caractère non-ASCII ;
/// - `underscores` : le PATH contient un underscore `_` ;
/// - `uppercase` : le PATH contient au moins une lettre majuscule ASCII ;
/// - `multiple_slashes` : le PATH contient une séquence `//` ;
/// - `params` : nombre de paramètres de la query string.
///
/// Le decompte sur le chemin se fait sur le path tel qu'il apparait dans
/// l'URL d'origine (avant tout decodage percent), afin de refleter ce que
/// voient les moteurs. Si l'URL n'est pas analysable, on retombe sur des
/// heuristiques sur la chaîne brute (length/non_ascii restent fiables).
pub fn analyze(url: &str) -> UrlFlags {
    let mut flags = UrlFlags::default();

    // Longueur et non-ASCII portent sur l'URL complète telle quelle.
    flags.length = url.len();
    flags.non_ascii = url.chars().any(|c| !c.is_ascii());

    match Url::parse(url) {
        Ok(parsed) => {
            let path = parsed.path();
            flags.underscores = path.contains('_');
            flags.uppercase = path.chars().any(|c| c.is_ascii_uppercase());
            flags.multiple_slashes = path.contains("//");
            flags.params = parsed.query_pairs().count();
        }
        Err(_) => {
            // URL non standard : on isole au mieux le chemin de la chaîne brute
            // (entre l'autorite et un eventuel '?' ou '#').
            let path = extract_path_fallback(url);
            flags.underscores = path.contains('_');
            flags.uppercase = path.chars().any(|c| c.is_ascii_uppercase());
            flags.multiple_slashes = path.contains("//");
            flags.params = count_params_fallback(url);
        }
    }

    flags
}

/// Extrait grossierement le chemin d'une URL non analysable : on retire le
/// schema://hote, puis on coupe à la query (`?`) ou au fragment (`#`).
fn extract_path_fallback(url: &str) -> String {
    // Retirer le schema (jusqu'au premier "://") si présent.
    let after_scheme = match url.find("://") {
        Some(idx) => &url[idx + 3..],
        None => url,
    };
    // Le chemin commence au premier '/', sinon il n'y a pas de chemin.
    let from_slash = match after_scheme.find('/') {
        Some(idx) => &after_scheme[idx..],
        None => return String::new(),
    };
    // Couper avant la query et le fragment.
    let end = from_slash
        .find(['?', '#'])
        .unwrap_or(from_slash.len());
    from_slash[..end].to_string()
}

/// Compte les paramètres de query d'une URL non analysable : portion après le
/// premier '?' (et avant un eventuel '#'), séparée par '&', en ignorant les
/// segments vides.
fn count_params_fallback(url: &str) -> usize {
    let after_q = match url.find('?') {
        Some(idx) => &url[idx + 1..],
        None => return 0,
    };
    let query = match after_q.find('#') {
        Some(idx) => &after_q[..idx],
        None => after_q,
    };
    query.split('&').filter(|p| !p.is_empty()).count()
}
