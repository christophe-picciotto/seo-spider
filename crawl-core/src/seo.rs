//! SEO analysis: extract on-page SEO signals from HTML and flag issues.
//!
//! This module is the heart of the crawler's value. `extract_seo` pulls
//! structured data out of raw HTML; `analyze` applies Screaming-Frog-style
//! rules to produce a list of actionable issues.

use crate::report::SeoData;
use crate::report::{Hreflang, Issue, PageReport};
use scraper::{ElementRef, Html, Selector};
use url::Url;

/// Nombre maximal de `src` d'images sans alt conserves par page.
///
/// Une galerie ou une page de listing peut en aligner des centaines ; au-dela
/// de ce plafond, la liste ne sert plus a corriger et ne fait que gonfler les
/// rapports enregistres. Le COMPTEUR (`images_missing_alt`) reste exact quoi
/// qu'il arrive — seule la liste est bornee.
const MAX_MISSING_ALT_URLS: usize = 200;

/// Parse a constant selector string. The argument is always a hard-coded,
/// known-valid CSS selector, so `expect` here can never fire on external input.
fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("constant selector must be valid")
}

/// Collapse any whitespace run into a single space and trim the ends.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cleaned text content of an element (all descendant text nodes, ws-collapsed).
fn element_text(el: &ElementRef) -> String {
    let raw: String = el.text().collect::<Vec<_>>().join(" ");
    normalize_ws(&raw)
}

/// Resolve a possibly-relative href against the page URL. Returns the absolute
/// URL string, or `None` if the join fails.
fn resolve_url(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    base.join(href).ok().map(|u| u.to_string())
}

/// Normalize a URL for canonical comparison: drop the fragment and any single
/// trailing slash so that `https://x/` and `https://x` compare equal.
fn normalize_for_compare(u: &Url) -> String {
    let mut u = u.clone();
    u.set_fragment(None);
    let mut s = u.to_string();
    if s.ends_with('/') && !s.ends_with("://") {
        s.pop();
    }
    s
}

/// Extract a charset from a `content-type` value like `text/html; charset=utf-8`.
fn parse_charset_from_content_type(ct: &str) -> Option<String> {
    for part in ct.split(';') {
        let part = part.trim();
        let lower = part.to_ascii_lowercase();
        if let Some(idx) = lower.find("charset") {
            let after = part[idx + "charset".len()..].trim_start();
            if let Some(eq) = after.strip_prefix('=') {
                let cs = eq.trim().trim_matches('"').trim();
                if !cs.is_empty() {
                    return Some(cs.to_string());
                }
            }
        }
    }
    None
}

/// Heuristic: does this href carry an explicit authority (scheme://host or
/// protocol-relative //host)? Used only when no seed host is provided.
fn url_has_explicit_authority(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    lower.starts_with("//") || lower.starts_with("http://") || lower.starts_with("https://")
}

/// Push a `@type` JSON value (string or array of strings) into `out`.
fn push_type(t: &serde_json::Value, out: &mut Vec<String>) {
    match t {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if !s.is_empty() {
                out.push(s.to_string());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let serde_json::Value::String(s) = item {
                    let s = s.trim();
                    if !s.is_empty() {
                        out.push(s.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

/// Recursively collect `@type` values from a JSON-LD value, descending into
/// arrays and the `@graph` container.
fn collect_ld_types(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ld_types(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(t) = map.get("@type") {
                push_type(t, out);
            }
            if let Some(graph) = map.get("@graph") {
                collect_ld_types(graph, out);
            }
        }
        _ => {}
    }
}

/// Does this JSON-LD object carry a non-empty `@type`?
fn ld_obj_has_type(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    match map.get("@type") {
        Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
        Some(serde_json::Value::Array(items)) => items.iter().any(|v| {
            matches!(v, serde_json::Value::String(s) if !s.trim().is_empty())
        }),
        _ => false,
    }
}

/// Validate a single parsed JSON-LD `<script>` block (basic check).
///
/// Rules: each described entity must carry `@type`, and `@context` must be
/// present (either on the wrapper object or, for an array, on each entity).
/// Returns a human-readable error string on the first problem, or `None` when
/// the block is acceptable. The block may be a single object, an array of
/// objects, or an object wrapping a `@graph` array.
fn validate_ld_block(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            // @graph container: @context on the wrapper, @type on each node.
            if let Some(graph) = map.get("@graph") {
                if map.get("@context").is_none() {
                    return Some("@context manquant".to_string());
                }
                return match graph {
                    serde_json::Value::Array(items) => {
                        for item in items {
                            match item {
                                serde_json::Value::Object(node) => {
                                    if !ld_obj_has_type(node) {
                                        return Some("@type manquant".to_string());
                                    }
                                }
                                _ => return Some("@type manquant".to_string()),
                            }
                        }
                        None
                    }
                    // @graph present but not an array: still require a @type somewhere.
                    serde_json::Value::Object(node) => {
                        if ld_obj_has_type(node) {
                            None
                        } else {
                            Some("@type manquant".to_string())
                        }
                    }
                    _ => Some("@type manquant".to_string()),
                };
            }

            if !ld_obj_has_type(map) {
                return Some("@type manquant".to_string());
            }
            if map.get("@context").is_none() {
                return Some("@context manquant".to_string());
            }
            None
        }
        serde_json::Value::Array(items) => {
            // Top-level array: every entry must be a valid object on its own.
            for item in items {
                if let Some(err) = validate_ld_block(item) {
                    return Some(err);
                }
            }
            None
        }
        // A bare scalar / null is not a valid JSON-LD entity.
        _ => Some("@type manquant".to_string()),
    }
}

/// 64-bit FNV-1a hash of a single token. Deterministic and stable across
/// runs and platforms (no random seed), so two crawls of the same page yield
/// the same fingerprint.
fn fnv1a64(token: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in token.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Compute a 64-bit SimHash fingerprint of visible text, returned as a
/// zero-padded 16-char hex string. Classic Charikar SimHash: lowercase-tokenize
/// on whitespace, hash each token (FNV-1a 64), keep 64 signed accumulators
/// (+1 where the token-hash bit is 1, -1 where it is 0), then set the output
/// bit when its accumulator is positive. Near-identical pages produce hashes
/// with a small Hamming distance; the UI does the bit-distance comparison.
fn simhash64(text: &str) -> String {
    let mut acc = [0i64; 64];
    for token in text.split_whitespace() {
        let token = token.to_lowercase();
        if token.is_empty() {
            continue;
        }
        let h = fnv1a64(&token);
        for (i, slot) in acc.iter_mut().enumerate() {
            if (h >> i) & 1 == 1 {
                *slot += 1;
            } else {
                *slot -= 1;
            }
        }
    }
    let mut fingerprint: u64 = 0;
    for (i, slot) in acc.iter().enumerate() {
        if *slot > 0 {
            fingerprint |= 1u64 << i;
        }
    }
    format!("{:016x}", fingerprint)
}

/// Extract SEO-relevant data from an HTML document.
///
/// `page_url` is the absolute URL the HTML was fetched from (used to resolve
/// relative canonical/hreflang links). `seed_host` is the host of the crawl
/// seed, used to classify links as internal vs external.
pub fn extract_seo(html: &str, page_url: &str, seed_host: Option<&str>) -> SeoData {
    let document = Html::parse_document(html);
    let base = Url::parse(page_url).ok();

    let mut data = SeoData::default();

    // --- <title> ----------------------------------------------------------
    if let Some(el) = document.select(&sel("title")).next() {
        let t = element_text(&el);
        if !t.is_empty() {
            data.title_len = t.chars().count();
            data.title = Some(t);
        }
    }

    // --- <html lang> ------------------------------------------------------
    if let Some(html_el) = document.select(&sel("html")).next() {
        if let Some(lang) = html_el.value().attr("lang") {
            let lang = lang.trim();
            if !lang.is_empty() {
                data.lang = Some(lang.to_string());
            }
        }
    }

    // --- <meta ...> : description, robots, viewport, charset, og, twitter --
    for el in document.select(&sel("meta")) {
        let v = el.value();
        let name = v.attr("name").map(|s| s.trim().to_ascii_lowercase());
        let property = v.attr("property").map(|s| s.trim().to_ascii_lowercase());
        let content = v.attr("content").map(|s| s.trim());

        if let Some(cs) = v.attr("charset") {
            let cs = cs.trim();
            if !cs.is_empty() && data.charset.is_none() {
                data.charset = Some(cs.to_string());
            }
        }
        if let Some(eq) = v.attr("http-equiv") {
            if eq.trim().eq_ignore_ascii_case("content-type") {
                if let Some(c) = content {
                    if let Some(cs) = parse_charset_from_content_type(c) {
                        if data.charset.is_none() {
                            data.charset = Some(cs);
                        }
                    }
                }
            }
        }

        if let Some(name) = name.as_deref() {
            match name {
                "description" => {
                    if let Some(c) = content {
                        if !c.is_empty() && data.meta_description.is_none() {
                            let c = normalize_ws(c);
                            data.meta_description_len = c.chars().count();
                            data.meta_description = Some(c);
                        }
                    }
                }
                "robots" => {
                    if let Some(c) = content {
                        let lc = c.to_ascii_lowercase();
                        if data.meta_robots.is_none() {
                            data.meta_robots = Some(c.to_string());
                        }
                        if lc.contains("noindex") {
                            data.noindex = true;
                        }
                        if lc.contains("nofollow") {
                            data.nofollow = true;
                        }
                    }
                }
                "viewport" => {
                    if let Some(c) = content {
                        if !c.is_empty() && data.viewport.is_none() {
                            data.viewport = Some(c.to_string());
                        }
                    }
                }
                _ => {
                    if let Some(key) = name.strip_prefix("twitter:") {
                        if let Some(c) = content {
                            data.twitter
                                .entry(key.to_string())
                                .or_insert_with(|| c.to_string());
                        }
                    }
                }
            }
        }

        if let Some(property) = property.as_deref() {
            if let Some(key) = property.strip_prefix("og:") {
                if let Some(c) = content {
                    data.og
                        .entry(key.to_string())
                        .or_insert_with(|| c.to_string());
                }
            }
        }
    }

    // --- <link ...> : canonical + hreflang --------------------------------
    for el in document.select(&sel("link")) {
        let v = el.value();
        let rel = match v.attr("rel") {
            Some(r) => r.trim().to_ascii_lowercase(),
            None => continue,
        };
        let href = match v.attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        if href.is_empty() {
            continue;
        }
        let rels: Vec<&str> = rel.split_whitespace().collect();

        if rels.iter().any(|r| *r == "canonical") && data.canonical.is_none() {
            data.canonical = match base.as_ref() {
                Some(b) => resolve_url(b, href),
                None => Some(href.to_string()),
            };
        }

        // Pagination: <link rel="next"> / <link rel="prev"> (resolus en absolu).
        if rels.iter().any(|r| *r == "next") && data.pagination_next.is_none() {
            data.pagination_next = match base.as_ref() {
                Some(b) => resolve_url(b, href),
                None => Some(href.to_string()),
            };
        }
        if rels.iter().any(|r| *r == "prev") && data.pagination_prev.is_none() {
            data.pagination_prev = match base.as_ref() {
                Some(b) => resolve_url(b, href),
                None => Some(href.to_string()),
            };
        }

        if rels.iter().any(|r| *r == "alternate") {
            if let Some(lang) = v.attr("hreflang") {
                let lang = lang.trim();
                if !lang.is_empty() {
                    let abs = match base.as_ref() {
                        Some(b) => resolve_url(b, href).unwrap_or_else(|| href.to_string()),
                        None => href.to_string(),
                    };
                    data.hreflang.push(Hreflang {
                        lang: lang.to_string(),
                        href: abs,
                    });
                }
            }
        }
    }

    // --- indexable: not noindex AND (no canonical OR canonical == page) ----
    let canonical_ok = match (&data.canonical, &base) {
        (None, _) => true,
        (Some(_), None) => true,
        (Some(c), Some(b)) => match Url::parse(c) {
            Ok(cu) => normalize_for_compare(&cu) == normalize_for_compare(b),
            Err(_) => false,
        },
    };
    data.indexable = !data.noindex && canonical_ok;

    // canonicalised : un canonical est present ET pointe vers une AUTRE url
    // (normalisee) que la page courante -> la page se declare non-indexable.
    data.canonicalised = data.canonical.is_some() && !canonical_ok;

    // --- headings ---------------------------------------------------------
    for el in document.select(&sel("h1")) {
        let t = element_text(&el);
        if !t.is_empty() {
            data.h1.push(t);
        }
    }
    data.h1_count = data.h1.len();

    for el in document.select(&sel("h2")) {
        let t = element_text(&el);
        if !t.is_empty() {
            data.h2.push(t);
        }
    }

    // --- images -----------------------------------------------------------
    for el in document.select(&sel("img")) {
        data.images_total += 1;
        let alt_ok = match el.value().attr("alt") {
            Some(a) => !a.trim().is_empty(),
            None => false,
        };
        if !alt_ok {
            data.images_missing_alt += 1;

            // Le compteur ne suffit pas a corriger : on retient AUSSI le `src`.
            //
            // Les images en chargement paresseux (lazy) portent souvent un `src`
            // vide ou un pixel transparent en `data:`, et le vrai chemin dans
            // `data-src` : on prend le premier attribut exploitable. Resolu en
            // absolu contre l'URL de la page quand c'est possible, sinon garde
            // tel quel plutot que perdu.
            let src = ["src", "data-src", "data-lazy-src"]
                .iter()
                .filter_map(|a| el.value().attr(a))
                .map(str::trim)
                .find(|s| !s.is_empty() && !s.starts_with("data:"));
            if let Some(src) = src {
                if data.images_missing_alt_urls.len() < MAX_MISSING_ALT_URLS {
                    let abs = base
                        .as_ref()
                        .and_then(|b| resolve_url(b, src))
                        .unwrap_or_else(|| src.to_string());
                    // La meme image (logo, pictogramme) revient souvent plusieurs
                    // fois dans une page : une seule ligne suffit pour la corriger.
                    if !data.images_missing_alt_urls.contains(&abs) {
                        data.images_missing_alt_urls.push(abs);
                    }
                }
            }
        }
    }

    // --- links: internal vs external --------------------------------------
    let seed_host_norm = seed_host.map(|h| h.trim().to_ascii_lowercase());
    for el in document.select(&sel("a[href]")) {
        let href = match el.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        if href.is_empty() {
            continue;
        }
        let lower = href.to_ascii_lowercase();
        if lower.starts_with("mailto:")
            || lower.starts_with("tel:")
            || lower.starts_with("javascript:")
            || href.starts_with('#')
        {
            continue;
        }

        let abs = base.as_ref().and_then(|b| b.join(href).ok());
        match (&abs, &seed_host_norm) {
            (Some(u), Some(seed)) => match u.host_str().map(|h| h.to_ascii_lowercase()) {
                Some(h) if &h == seed => data.internal_links += 1,
                Some(_) => data.external_links += 1,
                None => data.internal_links += 1,
            },
            (Some(u), None) => {
                if u.host_str().is_some() && url_has_explicit_authority(href) {
                    data.external_links += 1;
                } else {
                    data.internal_links += 1;
                }
            }
            (None, _) => data.internal_links += 1,
        }
    }

    // --- structured data (JSON-LD) ----------------------------------------
    // Pour chaque bloc : on collecte les @type et on valide basiquement
    // (JSON parsable + @type/@context presents).
    let mut sd_types: Vec<String> = Vec::new();
    let mut sd_errors: Vec<String> = Vec::new();
    let mut sd_raw: Vec<String> = Vec::new();
    let mut sd_block_index = 0usize;
    for el in document.select(&sel("script[type=\"application/ld+json\"]")) {
        let raw = el.text().collect::<String>();
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        sd_block_index += 1;
        // Texte brut du bloc, tel quel (deja trim) : l'UI le formattera/affichera.
        sd_raw.push(raw.to_string());
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(json) => {
                collect_ld_types(&json, &mut sd_types);
                if let Some(err) = validate_ld_block(&json) {
                    sd_errors.push(format!("Bloc {}: {}", sd_block_index, err));
                }
            }
            Err(_) => {
                sd_errors.push(format!("Bloc {}: JSON invalide", sd_block_index));
            }
        }
    }
    sd_types.sort();
    sd_types.dedup();
    data.structured_data_types = sd_types;
    data.structured_data_raw = sd_raw;
    data.structured_data_valid = sd_errors.is_empty();
    data.structured_data_errors = sd_errors;

    // --- word count & text ratio ------------------------------------------
    let visible_text = match document.select(&sel("body")).next() {
        Some(body) => body.text().collect::<Vec<_>>().join(" "),
        None => document.root_element().text().collect::<Vec<_>>().join(" "),
    };
    let visible_text = normalize_ws(&visible_text);
    data.word_count = if visible_text.is_empty() {
        0
    } else {
        visible_text.split_whitespace().count()
    };

    // Near-duplicate fingerprint over the same visible text used for word_count.
    data.simhash = simhash64(&visible_text);

    let html_len = html.len();
    data.text_ratio = if html_len == 0 {
        0.0
    } else {
        (visible_text.len() as f32 / html_len as f32).clamp(0.0, 1.0)
    };

    data
}

/// Analyze a page report and produce a list of SEO issues.
pub fn analyze(report: &PageReport) -> Vec<Issue> {
    let seo = match &report.seo {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut issues: Vec<Issue> = Vec::new();
    let mut push = |severity: &str, category: &str, message: String| {
        issues.push(Issue {
            url: report.url.clone(),
            severity: severity.to_string(),
            category: category.to_string(),
            message,
        });
    };

    // --- Response code ----------------------------------------------------
    if report.status >= 400 {
        push("critical", "Reponse", format!("Code reponse {}", report.status));
    } else if report.status >= 300 {
        push("warning", "Reponse", format!("Redirection {}", report.status));
    }

    // --- Title ------------------------------------------------------------
    let has_title = seo.title.as_deref().map(|t| !t.is_empty()).unwrap_or(false);
    if !has_title {
        push("critical", "Titres", "Title manquant".to_string());
    } else if seo.title_len > 60 {
        push("warning", "Titres", format!("Title trop long ({} car.)", seo.title_len));
    } else if seo.title_len < 30 {
        push("opportunity", "Titres", format!("Title court ({})", seo.title_len));
    }

    // --- Meta description -------------------------------------------------
    let has_desc = seo.meta_description.as_deref().map(|d| !d.is_empty()).unwrap_or(false);
    if !has_desc {
        push("warning", "Meta description", "Meta description manquante".to_string());
    } else if seo.meta_description_len > 160 {
        push(
            "warning",
            "Meta description",
            format!("Meta description trop longue ({})", seo.meta_description_len),
        );
    } else if seo.meta_description_len < 70 {
        push(
            "opportunity",
            "Meta description",
            format!("Meta description courte ({})", seo.meta_description_len),
        );
    }

    // --- H1 ---------------------------------------------------------------
    if seo.h1_count == 0 {
        push("warning", "H1", "H1 manquant".to_string());
    } else if seo.h1_count > 1 {
        push("warning", "H1", format!("{} H1 (devrait etre unique)", seo.h1_count));
    }

    // --- Images -----------------------------------------------------------
    if seo.images_missing_alt > 0 {
        push(
            "opportunity",
            "Images",
            format!("{} image(s) sans attribut alt", seo.images_missing_alt),
        );
    }

    // --- Directives -------------------------------------------------------
    if seo.noindex {
        push("warning", "Directives", "Page en noindex".to_string());
    }

    // --- Canonical --------------------------------------------------------
    if seo.canonical.is_none() {
        push("opportunity", "Canonical", "Canonical manquant".to_string());
    }

    // --- Canonical : page canonicalisee (pointe vers une autre URL) -------
    if seo.canonicalised {
        push(
            "warning",
            "Canonical",
            "Page canonicalisee (non-indexable)".to_string(),
        );
    }

    // --- Directives : X-Robots-Tag noindex (en-tete HTTP) -----------------
    if let Some(xrt) = seo.x_robots_tag.as_deref() {
        if xrt.to_ascii_lowercase().contains("noindex") {
            push("warning", "Directives", "X-Robots-Tag noindex".to_string());
        }
    }

    // --- Pagination -------------------------------------------------------
    if seo.pagination_next.is_some() || seo.pagination_prev.is_some() {
        push("info", "Pagination", "Page paginee".to_string());
    }

    // --- Donnees structurees ----------------------------------------------
    if !seo.structured_data_valid {
        let first = seo
            .structured_data_errors
            .first()
            .cloned()
            .unwrap_or_else(|| "erreur".to_string());
        push(
            "warning",
            "Donnees structurees",
            format!("Donnees structurees invalides ({})", first),
        );
    }

    // --- Securite : mixed content + en-tetes manquants --------------------
    if let Some(security) = report.security.as_ref() {
        if !security.mixed_content.is_empty() {
            push(
                "warning",
                "Securite",
                format!(
                    "Contenu mixte ({} ressources http)",
                    security.mixed_content.len()
                ),
            );
        }
        for header in &security.missing_headers {
            push(
                "info",
                "Securite",
                format!("En-tete securite manquant: {}", header),
            );
        }
    }

    // --- URL : qualite de l'URL -------------------------------------------
    if let Some(flags) = report.url_flags.as_ref() {
        if flags.non_ascii {
            push("info", "URL", "URL avec caracteres non-ASCII".to_string());
        }
        if flags.length > 115 {
            push("opportunity", "URL", format!("URL longue ({})", flags.length));
        }
        if flags.uppercase {
            push("info", "URL", "URL avec majuscules".to_string());
        }
        if flags.underscores {
            push("info", "URL", "URL avec underscores".to_string());
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_sans_alt_remontees_en_url_absolue_et_dedoublonnees() {
        let html = r#"<html><body>
            <img src="/img/a.webp">
            <img src="/img/a.webp">
            <img src="/img/b.webp" alt="">
            <img src="/img/c.webp" alt="ok">
            <img src="data:image/gif;base64,R0lGOD" data-src="https://cdn.ex/lazy.webp">
            <img alt="">
        </body></html>"#;
        let seo = extract_seo(html, "https://exemple.fr/page/", Some("exemple.fr"));

        assert_eq!(seo.images_total, 6);
        // a, a, b (alt vide), lazy, sans src : seul c.webp a un alt.
        assert_eq!(seo.images_missing_alt, 5);
        assert_eq!(
            seo.images_missing_alt_urls,
            vec![
                "https://exemple.fr/img/a.webp",   // relatif -> absolu ; le doublon n'apparait qu'une fois
                "https://exemple.fr/img/b.webp",   // alt="" compte comme manquant
                "https://cdn.ex/lazy.webp",        // `src` en data: ignore, `data-src` retenu
            ]
        );
    }

    #[test]
    fn images_sans_alt_le_compteur_reste_exact_seule_la_liste_est_bornee() {
        let mut html = String::from("<html><body>");
        for i in 0..MAX_MISSING_ALT_URLS + 50 {
            html.push_str(&format!("<img src=\"/i/{i}.webp\">"));
        }
        html.push_str("</body></html>");
        let seo = extract_seo(&html, "https://exemple.fr/", None);

        assert_eq!(seo.images_missing_alt as usize, MAX_MISSING_ALT_URLS + 50);
        assert_eq!(seo.images_missing_alt_urls.len(), MAX_MISSING_ALT_URLS);
    }
}
