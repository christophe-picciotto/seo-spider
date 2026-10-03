//! Detection de technologie par signatures (style Wappalyzer).
//!
//! Deux entrées publiques :
//! - [`detect`] : inspecte les en-têtes HTTP et le HTML pour deviner la stack.
//! - [`is_dynamic_request`] : heuristique "cette requête a-t-elle declenche du
//!   calcul serveur ?" (route API, RSC, ou cache CDN manque).

use reqwest::header::HeaderMap;

/// Recherche une valeur d'en-tête (par nom, insensible à la casse côté reqwest)
/// et renvoie sa valeur en minuscules pour une comparaison robuste.
fn header_lower(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase())
}

/// Vrai si l'en-tête `name` existe et que sa valeur contient `needle`
/// (comparaison insensible à la casse).
fn header_contains(headers: &HeaderMap, name: &str, needle: &str) -> bool {
    header_lower(headers, name)
        .map(|v| v.contains(&needle.to_ascii_lowercase()))
        .unwrap_or(false)
}

/// Vrai si l'en-tête `name` est présent (quelle que soit sa valeur).
fn header_present(headers: &HeaderMap, name: &str) -> bool {
    headers.contains_key(name)
}

/// Detecte les technologies à partir des en-têtes HTTP et du corps HTML.
///
/// Le HTML est compare tel quel (les signatures fournies sont sensibles à la
/// casse : `__NEXT_DATA__`, `wp-content`, `/_astro/`, ...). Les valeurs
/// d'en-têtes sont comparees sans tenir compte de la casse car les serveurs
/// varient (`Vercel` / `vercel`, `cloudflare`, ...).
///
/// La liste retournee est sans doublons et conserve l'ordre de detection.
pub fn detect(headers: &HeaderMap, html: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    // Pas de `mut` : cette closure ne capture rien de son environnement —
    // `found` lui est passe en ARGUMENT à chaque appel. C'est justement ce qui
    // évite qu'elle ne le verrouille en emprunt mutable pour toute sa durée de
    // vie, ce qui empecherait le reste de la fonction d'y toucher.
    let push = |tech: &str, found: &mut Vec<String>| {
        if !found.iter().any(|t| t == tech) {
            found.push(tech.to_string());
        }
    };

    // --- Frameworks JS / SSR ---

    // Next.js : marqueur de données, dossier d'assets, ou en-tête x-powered-by.
    if html.contains("__NEXT_DATA__")
        || html.contains("/_next/")
        || header_contains(headers, "x-powered-by", "Next.js")
    {
        push("Next.js", &mut found);
    }

    // Nuxt (Vue meta-framework).
    if html.contains("__NUXT__") {
        push("Nuxt", &mut found);
    }

    // Gatsby.
    if html.contains("___gatsby") {
        push("Gatsby", &mut found);
    }

    // SvelteKit.
    if html.contains("__sveltekit") {
        push("SvelteKit", &mut found);
    }

    // Remix.
    if html.contains("window.__remixContext") {
        push("Remix", &mut found);
    }

    // Astro : dossier d'assets hashes ou balise generator.
    if html.contains("/_astro/") || html.contains("name=\"generator\" content=\"Astro") {
        push("Astro", &mut found);
    }

    // --- CMS ---

    // WordPress.
    if html.contains("wp-content") || html.contains("wp-includes") {
        push("WordPress", &mut found);
    }

    // --- Librairies front (après les frameworks, plus spécifiques) ---

    // React : racine SSR explicite, ou conteneur racine + présence d'un bundle.
    if html.contains("data-reactroot")
        || (html.contains("id=\"root\"") && html.contains("bundle"))
    {
        push("React", &mut found);
    }

    // Vue : attribut de scope SFC ou conteneur racine conventionnel.
    if html.contains("data-v-") || html.contains("id=\"app\"") {
        push("Vue", &mut found);
    }

    // --- Hebergeurs / CDN (en-têtes) ---

    // Vercel : server = Vercel, ou en-têtes spécifiques.
    if header_contains(headers, "server", "Vercel")
        || header_present(headers, "x-vercel-id")
        || header_present(headers, "x-vercel-cache")
    {
        push("Vercel", &mut found);
    }

    // Netlify.
    if header_present(headers, "x-nf-request-id") || header_contains(headers, "server", "Netlify") {
        push("Netlify", &mut found);
    }

    // Cloudflare.
    if header_present(headers, "cf-ray") || header_contains(headers, "server", "cloudflare") {
        push("Cloudflare", &mut found);
    }

    found
}

/// Heuristique : la requête a-t-elle probablement declenche du calcul serveur
/// (invocation serverless) plutot qu'un simple hit de cache statique ?
///
/// Vrai si :
/// - l'URL contient `/api/` (route API),
/// - l'URL contient `?_rsc=` ou `_rsc=` (React Server Components / navigation),
/// - le cache Vercel (insensible à la casse) vaut `MISS`, `STALE` ou `PRERENDER`.
///
/// Faux sinon (typiquement un `HIT` de cache).
pub fn is_dynamic_request(url: &str, vercel_cache: Option<&str>) -> bool {
    if url.contains("/api/") || url.contains("?_rsc=") || url.contains("_rsc=") {
        return true;
    }

    if let Some(cache) = vercel_cache {
        let cache = cache.trim().to_ascii_uppercase();
        if cache == "MISS" || cache == "STALE" || cache == "PRERENDER" {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    fn headers_from(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn detect_nextjs_via_html_marker() {
        let h = HeaderMap::new();
        let techs = detect(&h, r#"<script id="__NEXT_DATA__">{}</script>"#);
        assert!(techs.contains(&"Next.js".to_string()));
    }

    #[test]
    fn detect_nextjs_via_assets_path() {
        let h = HeaderMap::new();
        let techs = detect(&h, r#"<link href="/_next/static/chunks/main.js">"#);
        assert!(techs.contains(&"Next.js".to_string()));
    }

    #[test]
    fn detect_nextjs_via_powered_by_header_case_insensitive() {
        let h = headers_from(&[("x-powered-by", "next.js")]);
        let techs = detect(&h, "");
        assert!(techs.contains(&"Next.js".to_string()));
    }

    #[test]
    fn detect_vercel_via_server_header() {
        let h = headers_from(&[("server", "Vercel")]);
        let techs = detect(&h, "");
        assert!(techs.contains(&"Vercel".to_string()));
    }

    #[test]
    fn detect_vercel_via_x_vercel_cache() {
        let h = headers_from(&[("x-vercel-cache", "HIT")]);
        let techs = detect(&h, "");
        assert!(techs.contains(&"Vercel".to_string()));
    }

    #[test]
    fn detect_netlify_and_cloudflare() {
        let nf = headers_from(&[("x-nf-request-id", "abc")]);
        assert!(detect(&nf, "").contains(&"Netlify".to_string()));

        let cf = headers_from(&[("cf-ray", "abc-CDG")]);
        assert!(detect(&cf, "").contains(&"Cloudflare".to_string()));

        let cf2 = headers_from(&[("server", "cloudflare")]);
        assert!(detect(&cf2, "").contains(&"Cloudflare".to_string()));
    }

    #[test]
    fn detect_astro_via_generator_meta() {
        let h = HeaderMap::new();
        let techs = detect(&h, r#"<meta name="generator" content="Astro v4.0">"#);
        assert!(techs.contains(&"Astro".to_string()));
    }

    #[test]
    fn detect_astro_via_assets_path() {
        let h = HeaderMap::new();
        assert!(detect(&h, r#"<link href="/_astro/index.abc123.css">"#).contains(&"Astro".to_string()));
    }

    #[test]
    fn detect_wordpress() {
        let h = HeaderMap::new();
        assert!(detect(&h, r#"<img src="/wp-content/uploads/x.png">"#).contains(&"WordPress".to_string()));
        assert!(detect(&h, r#"<link href="/wp-includes/css/x.css">"#).contains(&"WordPress".to_string()));
    }

    #[test]
    fn detect_react_requires_root_and_bundle() {
        let h = HeaderMap::new();
        assert!(detect(&h, r#"<div data-reactroot></div>"#).contains(&"React".to_string()));
        assert!(detect(&h, r#"<div id="root"></div><script src="/bundle.js">"#)
            .contains(&"React".to_string()));
        // id="root" seul sans bundle -> pas de React
        assert!(!detect(&h, r#"<div id="root"></div>"#).contains(&"React".to_string()));
    }

    #[test]
    fn detect_vue() {
        let h = HeaderMap::new();
        assert!(detect(&h, r#"<span data-v-7ba5bd90>x</span>"#).contains(&"Vue".to_string()));
        assert!(detect(&h, r#"<div id="app"></div>"#).contains(&"Vue".to_string()));
    }

    #[test]
    fn detect_nuxt_gatsby_sveltekit_remix() {
        let h = HeaderMap::new();
        assert!(detect(&h, "window.__NUXT__={}").contains(&"Nuxt".to_string()));
        assert!(detect(&h, r#"<div id="___gatsby"></div>"#).contains(&"Gatsby".to_string()));
        assert!(detect(&h, "__sveltekit_1abc = {}").contains(&"SvelteKit".to_string()));
        assert!(detect(&h, "window.__remixContext = {}").contains(&"Remix".to_string()));
    }

    #[test]
    fn detect_dedups_results() {
        // x-powered-by ET html declenchent tous deux Next.js : une seule entrée.
        let h = headers_from(&[("x-powered-by", "Next.js")]);
        let techs = detect(&h, "/_next/ __NEXT_DATA__");
        let count = techs.iter().filter(|t| *t == "Next.js").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn detect_empty_when_nothing_matches() {
        let h = HeaderMap::new();
        assert!(detect(&h, "<html><body>plain</body></html>").is_empty());
    }

    #[test]
    fn dynamic_api_route() {
        assert!(is_dynamic_request("https://x.com/api/users", None));
    }

    #[test]
    fn dynamic_rsc() {
        assert!(is_dynamic_request("https://x.com/page?_rsc=abcd", None));
        assert!(is_dynamic_request("https://x.com/page?foo=1&_rsc=abcd", None));
    }

    #[test]
    fn dynamic_cache_states_case_insensitive() {
        assert!(is_dynamic_request("https://x.com/", Some("MISS")));
        assert!(is_dynamic_request("https://x.com/", Some("miss")));
        assert!(is_dynamic_request("https://x.com/", Some("Stale")));
        assert!(is_dynamic_request("https://x.com/", Some("PRERENDER")));
    }

    #[test]
    fn static_hit_is_not_dynamic() {
        assert!(!is_dynamic_request("https://x.com/about", Some("HIT")));
        assert!(!is_dynamic_request("https://x.com/about", None));
    }
}
