use std::collections::{HashMap, HashSet};

use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use url::Url;

use crate::report::{AssetReport, LinkInfo, TypeStat};

/// Number of asset downloads run concurrently.
const ASSET_CONCURRENCY: usize = 8;

/// Cache PARTAGE des poids d'assets déjà mesures : `url -> octets`.
///
/// Un site partage son CSS, ses polices et ses bundles JS entre TOUTES ses
/// pages. Sans ce cache, `measure_static_assets` retelechargeait chaque asset
/// une fois par page : sur 500 pages avec 10 assets communs, 5 000 requêtes et
/// des centaines de Mo pour toujours le même résultat — et le worker restait
/// bloque dessus avant de pouvoir prendre la page suivante.
///
/// Le poids reste compte dans CHAQUE page (le rapport « poids de la page »
/// mesure ce qu'un visiteur télécharge) : seule la requête réseau est evitee.
pub type AssetCache = tokio::sync::Mutex<HashMap<String, u64>>;

/// Maximum number of redirect hops followed before giving up.
const MAX_REDIRECTS: usize = 10;

/// Sauts de redirection suivis pour un ASSET (CDN, http->https, fingerprint).
const MAX_ASSET_REDIRECTS: usize = 5;

/// Résultat d'une mesure d'asset. La distinction stable / passager decide de ce
/// qui est CACHABLE, et elle seule :
///   - `Ok` et `Broken` sont des verdicts du serveur, stables sur la durée d'un
///     crawl -> mis en cache (mesurer 5 000 fois la même 404 est du gaspillage) ;
///   - `Transient` est un alea réseau -> jamais mis en cache, sinon un unique
///     timeout amputerait le poids de toutes les pages suivantes, en silence.
enum AssetMeasure {
    /// Poids mesure (`Content-Length` ou taille du corps).
    Ok(u64),
    /// Asset durablement inaccessible : 4xx/5xx, boucle de redirection,
    /// `Location` inexploitable, ou cible hors http(s) / locale.
    Broken,
    /// Échec passager : DNS, timeout, connexion coupee, corps tronque.
    Transient,
}

pub struct HttpProbeResult {
    /// Status of the FINAL response (after following redirects). The first 3xx
    /// code of a redirect chain is exposed separately via `first_status`.
    pub status: u16,
    pub headers: reqwest::header::HeaderMap,
    pub html: String,
    /// Deduplicated absolute http(s) links used to feed the crawl frontier.
    pub links: Vec<String>,
    /// Every `<a href>` with its anchor text and `rel`, in document order
    /// (used to populate `PageReport.outlinks`). May contain duplicates.
    pub links_detailed: Vec<LinkInfo>,
    /// URLs traversed during redirection, in order: the requested URL first,
    /// then every intermediate hop, then the final URL. Empty (or length 1)
    /// when there was no redirect.
    pub redirect_chain: Vec<String>,
    /// Status code of the FIRST response when it was a redirect (3xx), else
    /// `None` (the request landed directly on its final response).
    pub first_status: Option<u16>,
    /// Final URL reached after following all redirects.
    pub final_url: String,
    /// Temps total de la sonde HTTP en millisecondes : du début de la 1ere requête
    /// (chaîne de redirections incluse) jusqu'à la lecture complète du corps final.
    pub response_time_ms: u64,
}

/// Resolve an `href`/`src` value against a base URL, keeping only http(s) URLs.
///
/// Le fragment (`#...`) est CONSERVE : cette fonction alimente aussi les
/// RAPPORTS (outlinks, CSS, fonts), qui doivent refleter le `href` tel qu'il
/// est ecrit dans le HTML. Pour la file de crawl, utiliser
/// [`resolve_url_for_crawl`].
fn resolve_url(base: &Url, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let joined = base.join(raw).ok()?;
    match joined.scheme() {
        "http" | "https" => Some(joined.into()),
        _ => None,
    }
}

/// `true` si l'IPv4 appartient à un réseau non routable sur Internet.
///
/// Couvre le prive RFC 1918 (10/8, 172.16/12, 192.168/16), le loopback 127/8,
/// le lien-local 169.254/16 (dont 169.254.169.254, les métadonnées cloud),
/// `0.0.0.0`, et le CGNAT 100.64/10 — cette dernière plage est celle de
/// **Tailscale**, très courante sur un poste de dev et non couverte par
/// `is_private()`.
fn is_local_v4(v4: std::net::Ipv4Addr) -> bool {
    let o = v4.octets();
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_unspecified()
        || (o[0] == 100 && (64..128).contains(&o[1]))
}

/// `true` si l'URL vise la machine locale ou un réseau prive/lien-local.
///
/// Sert de garde-fou sur les REDIRECTIONS, que `same_domain_only` ne couvre
/// pas : sans ca, un redirecteur ouvert sur le site crawle amenerait la sonde a
/// interroger les services de dev, la box ou le NAS de l'utilisateur.
fn is_local_host(u: &Url) -> bool {
    // `Url::host()` distingue déjà Ipv4 / Ipv6 / Domain. NE PAS passer par
    // `host_str()` : il rend l'IPv6 sous sa forme URL, crochets compris
    // (`[::1]`), qui ne se parse pas en `IpAddr`.
    match u.host() {
        Some(url::Host::Ipv4(v4)) => is_local_v4(v4),
        Some(url::Host::Ipv6(v6)) => {
            // Un IPv4-mapped (`::ffff:127.0.0.1`) se juge sur son IPv4 :
            // `Ipv6Addr::is_loopback()` ne reconnait QUE `::1`, donc
            // `http://[::ffff:127.0.0.1]/` contournait le filtre d'une seule
            // ligne d'URL.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local_v4(v4);
            }
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || (s[0] & 0xfe00) == 0xfc00  // ULA fc00::/7
                || (s[0] & 0xffc0) == 0xfe80  // lien-local fe80::/10
        }
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            d == "localhost"
                || d.ends_with(".local")
                || d.ends_with(".localhost")
                || d.ends_with(".internal")
        }
        None => false,
    }
}

/// Idem [`resolve_url`], mais SANS le fragment.
///
/// Le `#...` n'est jamais transmis au serveur (RFC 3986 : le fragment est
/// purement côté client), donc deux URLs qui ne diffèrent que par leur fragment
/// designent la MÊME ressource et renvoient le MÊME HTML. Sans ce nettoyage,
/// chaque ancre du HTML entre dans la file comme une page distincte :
///   - `href="#top"` présent sur toutes les pages -> le crawl DOUBLE ;
///   - la page fantome porte les mêmes title/meta/H1/contenu que la vraie ->
///     faux positifs en masse dans les rapports "doublons" et "near-duplicates".
///
/// C'est le comportement par defaut de Screaming Frog (option
/// "Crawl Fragment Identifiers" decochee).
fn resolve_url_for_crawl(base: &Url, raw: &str) -> Option<String> {
    let mut joined = Url::parse(&resolve_url(base, raw)?).ok()?;
    joined.set_fragment(None);
    Some(joined.into())
}

/// Light HTTP probe: GET the target FOLLOWING redirects manually so the full
/// chain is observable, capture the final status/headers/body and extract every
/// absolute http(s) link found in `a[href]`.
///
/// Redirects are followed by hand (the supplied `client` must use
/// `redirect::Policy::none()`): every hop URL is recorded in
/// `redirect_chain`, the first 3xx code in `first_status`, and the final
/// response body is the one analysed for SEO. Up to `MAX_REDIRECTS` hops.
///
/// `scraper::Html` is not `Send`, so the HTML is parsed and every URL is
/// collected into owned `String`s in a scope that ends before any `.await`.
pub async fn probe(
    client: &reqwest::Client,
    target: &str,
) -> anyhow::Result<HttpProbeResult> {
    // Chrono : mesure le temps total de la sonde (toute la chaîne de redirections
    // + lecture du corps final), pour alimenter PageReport.response_time_ms.
    let probe_start = std::time::Instant::now();

    // Chain of URLs traversed, starting with the requested one.
    let mut chain: Vec<String> = vec![target.to_string()];
    let mut first_status: Option<u16> = None;

    let mut current = target.to_string();
    let mut hops = 0usize;

    // Loop: follow Location while we get a 3xx with a resolvable target.
    let (final_status, headers, html, final_url) = loop {
        let resp = client.get(&current).send().await?;
        let status = resp.status();
        let code = status.as_u16();

        // Not a redirect (or a 3xx without usable Location) -> terminal response.
        let location = if status.is_redirection() {
            resp.headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        } else {
            None
        };

        match location {
            Some(loc) if hops < MAX_REDIRECTS => {
                // Resolve the (possibly relative) Location against the current URL.
                let next = match Url::parse(&current).and_then(|b| b.join(&loc)) {
                    Ok(u) => u,
                    Err(_) => {
                        // Unresolvable redirect target: treat current as terminal.
                        let headers = resp.headers().clone();
                        let html = resp.text().await?;
                        break (code, headers, html, current.clone());
                    }
                };
                // Only follow http(s) redirects.
                match next.scheme() {
                    "http" | "https" => {}
                    _ => {
                        let headers = resp.headers().clone();
                        let html = resp.text().await?;
                        break (code, headers, html, current.clone());
                    }
                }

                // On ne suit JAMAIS une redirection vers le réseau local.
                //
                // Le filtre `same_domain_only` ne s'applique qu'aux liens
                // EXTRAITS, jamais à la cible d'une redirection. Un redirecteur
                // ouvert sur le site crawle (`/go/?url=`, plugin de tracking)
                // suffirait donc a faire requeter le réseau local de la
                // machine — et la réponse serait stockee dans le fichier projet.
                if is_local_host(&next) {
                    let headers = resp.headers().clone();
                    let _ = resp.bytes().await;
                    break (code, headers, String::new(), current.clone());
                }

                if first_status.is_none() {
                    first_status = Some(code);
                }
                // Drain the body so the connection can be reused.
                let _ = resp.bytes().await;

                let next_str: String = next.into();
                chain.push(next_str.clone());
                current = next_str;
                hops += 1;
            }
            _ => {
                // Terminal: capture final headers/body.
                let headers = resp.headers().clone();
                let html = resp.text().await?;
                break (code, headers, html, current.clone());
            }
        }
    };

    // Links are resolved against the FINAL URL (where the document actually lives).
    let links = extract_links(&final_url, &html);
    let links_detailed = extract_links_detailed(&final_url, &html);

    // If there was no redirect, expose an empty chain (length-1 chains are noise).
    let redirect_chain = if chain.len() > 1 { chain } else { Vec::new() };

    // Temps total ecoule depuis le début de la sonde.
    let response_time_ms = probe_start.elapsed().as_millis() as u64;

    Ok(HttpProbeResult {
        status: final_status,
        headers,
        html,
        links,
        links_detailed,
        redirect_chain,
        first_status,
        final_url,
        response_time_ms,
    })
}

/// Parse the HTML and return the deduplicated list of absolute http(s) links.
///
/// Alimente la FILE de crawl : les fragments sont retires (cf.
/// [`resolve_url_for_crawl`]), donc les N ancres d'un menu s'effondrent en une
/// seule entrée via le `HashSet` ci-dessous.
///
/// Entirely synchronous: no `scraper` type escapes this function, so nothing
/// non-`Send` is held across an `.await` in the caller.
fn extract_links(base_url: &str, html: &str) -> Vec<String> {
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };

    let document = Html::parse_document(html);
    let selector = match Selector::parse("a[href]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut links: Vec<String> = Vec::new();
    for el in document.select(&selector) {
        if let Some(href) = el.value().attr("href") {
            if let Some(abs) = resolve_url_for_crawl(&base, href) {
                if seen.insert(abs.clone()) {
                    links.push(abs);
                }
            }
        }
    }
    links
}

/// Parse the HTML and return every `<a href>` as a `LinkInfo`: absolute target,
/// trimmed anchor text, `rel` attribute (if any) and whether the target is on
/// the same host as the page.
///
/// Unlike `extract_links`, this keeps duplicates and preserves document order
/// (one entry per anchor). Fully synchronous so no `scraper` type escapes.
fn extract_links_detailed(base_url: &str, html: &str) -> Vec<LinkInfo> {
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };
    let base_host = base.host_str().map(|h| h.to_ascii_lowercase());

    let document = Html::parse_document(html);
    let selector = match Selector::parse("a[href]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<LinkInfo> = Vec::new();
    for el in document.select(&selector) {
        let href = match el.value().attr("href") {
            Some(h) => h,
            None => continue,
        };
        let target = match resolve_url(&base, href) {
            Some(t) => t,
            None => continue,
        };

        // Anchor text: concatenated text nodes, whitespace-collapsed and trimmed.
        let anchor: String = el.text().collect::<Vec<_>>().join(" ");
        let anchor = anchor.split_whitespace().collect::<Vec<_>>().join(" ");

        let rel = el
            .value()
            .attr("rel")
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());

        // internal == same host as the page (case-insensitive).
        let internal = match (&base_host, Url::parse(&target).ok().and_then(|u| {
            u.host_str().map(|h| h.to_ascii_lowercase())
        })) {
            (Some(bh), Some(th)) => *bh == th,
            _ => false,
        };

        out.push(LinkInfo {
            target,
            anchor,
            rel,
            internal,
        });
    }
    out
}

/// A static asset URL with the resource kind inferred from the tag it came from.
struct AssetUrl {
    url: String,
    kind: &'static str,
}

/// Parse the HTML and collect every static asset URL (images, scripts,
/// stylesheets, preloaded fonts) resolved to an absolute http(s) URL.
///
/// Fully synchronous so the non-`Send` `scraper` types are dropped before the
/// caller awaits any download.
fn extract_static_asset_urls(base_url: &str, html: &str) -> Vec<AssetUrl> {
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Vec::new(),
    };

    let document = Html::parse_document(html);

    let img_sel = Selector::parse("img[src]").ok();
    let script_sel = Selector::parse("script[src]").ok();
    let css_sel = Selector::parse("link[rel=stylesheet][href]").ok();
    let preload_sel = Selector::parse("link[rel=preload][href]").ok();

    let mut seen: HashSet<String> = HashSet::new();
    let mut assets: Vec<AssetUrl> = Vec::new();

    if let Some(sel) = &img_sel {
        for el in document.select(sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(abs) = resolve_url(&base, src) {
                    push_asset(&mut assets, &mut seen, abs, "Image");
                }
            }
        }
    }

    if let Some(sel) = &script_sel {
        for el in document.select(sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(abs) = resolve_url(&base, src) {
                    push_asset(&mut assets, &mut seen, abs, "Script");
                }
            }
        }
    }

    if let Some(sel) = &css_sel {
        for el in document.select(sel) {
            if let Some(href) = el.value().attr("href") {
                if let Some(abs) = resolve_url(&base, href) {
                    push_asset(&mut assets, &mut seen, abs, "Stylesheet");
                }
            }
        }
    }

    // Preloaded fonts: <link rel="preload" as="font" href="...">
    if let Some(sel) = &preload_sel {
        for el in document.select(sel) {
            let is_font = el
                .value()
                .attr("as")
                .map(|a| a.eq_ignore_ascii_case("font"))
                .unwrap_or(false);
            if !is_font {
                continue;
            }
            if let Some(href) = el.value().attr("href") {
                if let Some(abs) = resolve_url(&base, href) {
                    push_asset(&mut assets, &mut seen, abs, "Font");
                }
            }
        }
    }

    assets
}

/// Append a deduplicated asset URL to `assets`, recording it in `seen`.
fn push_asset(
    assets: &mut Vec<AssetUrl>,
    seen: &mut HashSet<String>,
    url: String,
    kind: &'static str,
) {
    if seen.insert(url.clone()) {
        assets.push(AssetUrl { url, kind });
    }
}

/// Measure the transferred size of every static asset referenced by `html`.
///
/// Downloads each asset in parallel (`buffer_unordered`, ~8 at a time). For
/// each asset the byte size is the `Content-Length` header when présent,
/// otherwise the length of the downloaded body. Returns the per-asset reports,
/// the aggregated `by_type` map and the total number of bytes.
pub async fn measure_static_assets(
    client: &reqwest::Client,
    base_url: &str,
    html: &str,
    cache: &AssetCache,
) -> (
    Vec<AssetReport>,
    std::collections::HashMap<String, TypeStat>,
    u64,
) {
    // Extract every URL before any await: nothing from `scraper` is held here.
    let asset_urls = extract_static_asset_urls(base_url, html);

    // Download each asset concurrently and measure its transferred size.
    //
    // Un asset déjà mesure lors d'une page précédente est repris du cache sans
    // requête réseau (cf. [`AssetCache`]).
    let reports: Vec<AssetReport> = stream::iter(asset_urls.into_iter())
        .map(|asset| {
            let client = client.clone();
            async move {
                let known = cache.lock().await.get(&asset.url).copied();
                let bytes = match known {
                    Some(b) => b,
                    None => {
                        // Lock relache pendant le téléchargement : deux workers
                        // peuvent demander le même asset en même temps (au pire
                        // il est mesure deux fois, jamais N fois), mais aucun ne
                        // bloque les autres sur une E/S réseau.
                        match fetch_asset_bytes(&client, &asset.url).await {
                            // Succès : mesure fiable, on la partage.
                            AssetMeasure::Ok(b) => {
                                cache.lock().await.insert(asset.url.clone(), b);
                                b
                            }
                            // CASSE (statut non-2xx après redirections) : c'est
                            // un verdict STABLE du serveur, pas un alea. On le
                            // met en cache pour ne pas redemander 5 000 fois la
                            // même 404 (un logo supprime reste dans le header de
                            // toutes les pages du template).
                            AssetMeasure::Broken => {
                                cache.lock().await.insert(asset.url.clone(), 0);
                                0
                            }
                            // TRANSITOIRE (timeout, DNS, coupure) : on reporte 0
                            // pour CETTE page et on ne met RIEN en cache — la
                            // page suivante retentera.
                            //
                            // Mettre un échec passager en cache figerait une
                            // mesure fausse pour tout le reste du crawl : un
                            // seul timeout sur le CSS principal amputerait le
                            // poids de TOUTES les pages, en silence. Un cache
                            // change la portee d'une erreur — locale et
                            // passagere sans lui, globale et definitive avec.
                            // D'ou la distinction stable / passager.
                            AssetMeasure::Transient => 0,
                        }
                    }
                };
                AssetReport {
                    url: asset.url,
                    bytes,
                    kind: asset.kind.to_string(),
                }
            }
        })
        .buffer_unordered(ASSET_CONCURRENCY)
        .collect()
        .await;

    let mut by_type: HashMap<String, TypeStat> = HashMap::new();
    let mut total: u64 = 0;
    for r in &reports {
        total = total.saturating_add(r.bytes);
        let entry = by_type.entry(r.kind.clone()).or_default();
        entry.bytes = entry.bytes.saturating_add(r.bytes);
        entry.count += 1;
    }

    (reports, by_type, total)
}

/// Fetch a single asset and return its size in bytes (Content-Length when
/// advertised, otherwise the length of the downloaded body). Errors yield 0.
async fn fetch_asset_bytes(client: &reqwest::Client, url: &str) -> AssetMeasure {
    let mut current = url.to_string();

    for _ in 0..=MAX_ASSET_REDIRECTS {
        // Même garde-fou que sur les pages, applique des le PREMIER fetch : un
        // `<img src="http://192.168.1.1/x">` dans le HTML d'un site crawle
        // partait jusqu'ici sans contrôle (seuls les hops de redirection
        // etaient filtres).
        if Url::parse(&current).map(|u| is_local_host(&u)).unwrap_or(false) {
            return AssetMeasure::Broken;
        }

        let resp = match client.get(&current).send().await {
            Ok(r) => r,
            // Réseau : rien ne dit que ca se reproduira.
            Err(_) => return AssetMeasure::Transient,
        };
        let status = resp.status();

        // Redirection : le client de la sonde a `Policy::none()` (nécessaire
        // pour observer les chaînes de redirection des PAGES), donc c'est a nous
        // de suivre. Sans ca, un asset servi en 301 (CDN, http->https, version
        // fingerprintee) etait compte comme "casse" et son poids réel jamais
        // mesure.
        if status.is_redirection() {
            let loc = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let next = match loc.and_then(|l| Url::parse(&current).ok()?.join(&l).ok()) {
                Some(u) => u,
                // 3xx sans Location exploitable : verdict stable.
                None => return AssetMeasure::Broken,
            };
            if !matches!(next.scheme(), "http" | "https") || is_local_host(&next) {
                return AssetMeasure::Broken;
            }
            let _ = resp.bytes().await; // libere la connexion pour le pool
            current = next.into();
            continue;
        }

        // Statut non-2xx : le corps est une page d'erreur, pas l'asset. Le
        // compter donnerait un poids plausible mais FAUX (une 404 HTML fait
        // quelques Ko). C'est un verdict STABLE : cachable.
        if !status.is_success() {
            return AssetMeasure::Broken;
        }

        let content_length = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());

        if let Some(len) = content_length {
            // Drain the body so the connection can be reused by the pool.
            let _ = resp.bytes().await;
            return AssetMeasure::Ok(len);
        }

        return match resp.bytes().await {
            Ok(b) => AssetMeasure::Ok(b.len() as u64),
            // Corps tronque en cours de lecture : passager.
            Err(_) => AssetMeasure::Transient,
        };
    }

    // Trop de sauts : boucle de redirection, verdict stable.
    AssetMeasure::Broken
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://site.fr/services/").unwrap()
    }

    /// Les RAPPORTS (outlinks) doivent refleter le `href` du HTML, fragment inclus.
    #[test]
    fn resolve_url_garde_le_fragment() {
        assert_eq!(
            resolve_url(&base(), "#contact").unwrap(),
            "https://site.fr/services/#contact"
        );
        assert_eq!(
            resolve_url(&base(), "/tarifs/#pro").unwrap(),
            "https://site.fr/tarifs/#pro"
        );
    }

    /// La FILE de crawl, elle, ne doit jamais voir de fragment : `#...` n'est pas
    /// envoye au serveur, donc c'est la même ressource.
    #[test]
    fn resolve_url_for_crawl_retire_le_fragment() {
        assert_eq!(
            resolve_url_for_crawl(&base(), "#contact").unwrap(),
            "https://site.fr/services/"
        );
        assert_eq!(
            resolve_url_for_crawl(&base(), "/tarifs/#pro").unwrap(),
            "https://site.fr/tarifs/"
        );
        assert_eq!(
            resolve_url_for_crawl(&base(), "#").unwrap(),
            "https://site.fr/services/"
        );
        // La query, elle, DOIT survivre : elle change la ressource servie.
        assert_eq!(
            resolve_url_for_crawl(&base(), "?page=2#haut").unwrap(),
            "https://site.fr/services/?page=2"
        );
        // Les schemes non-http restent rejetes.
        assert!(resolve_url_for_crawl(&base(), "mailto:a@b.fr").is_none());
        assert!(resolve_url_for_crawl(&base(), "javascript:void(0)").is_none());
    }

    /// REGRESSION : un menu d'ancres ne doit plus produire qu'UNE entrée de file.
    /// Avant le correctif, ce HTML enfilait 4 URLs et faisait crawler la page
    /// courante 3 fois de plus (même title/H1/meta -> faux doublons partout).
    #[test]
    fn extract_links_effondre_les_ancres_d_un_menu() {
        // `r##"..."##` (double diese) obligatoire : `href="#haut"` contient la
        // séquence `"#` qui refermerait un raw string `r#"..."#`.
        let html = r##"
            <a href="#haut">Haut</a>
            <a href="#tarifs">Tarifs</a>
            <a href="#contact">Contact</a>
            <a href="/contact/">Nous contacter</a>
        "##;
        let links = extract_links("https://site.fr/services/", html);
        assert_eq!(
            links,
            vec![
                "https://site.fr/services/".to_string(),
                "https://site.fr/contact/".to_string(),
            ]
        );
    }

    /// Deux ancres vers la MÊME page cible = une seule entrée de file.
    #[test]
    fn extract_links_dedoublonne_entre_pages() {
        let html = r##"
            <a href="/tarifs/#pro">Pro</a>
            <a href="/tarifs/#particulier">Particulier</a>
            <a href="/tarifs/">Tous les tarifs</a>
        "##;
        let links = extract_links("https://site.fr/", html);
        assert_eq!(links, vec!["https://site.fr/tarifs/".to_string()]);
    }
}

#[cfg(test)]
mod tests_local_host {
    use super::*;

    fn local(u: &str) -> bool {
        is_local_host(&Url::parse(u).unwrap())
    }

    /// Une redirection vers le réseau local ne doit jamais être suivie :
    /// `same_domain_only` ne couvre pas les cibles de redirection, donc un
    /// redirecteur ouvert sur le site crawle suffirait a atteindre les services
    /// de dev, la box ou le NAS de la machine.
    #[test]
    fn detecte_les_hotes_locaux() {
        assert!(local("http://localhost:3000/"));
        assert!(local("http://127.0.0.1/"));
        assert!(local("http://127.0.0.53/"));
        assert!(local("http://192.168.1.1/"));
        assert!(local("http://10.0.0.5/"));
        assert!(local("http://172.16.0.1/"));
        assert!(local("http://169.254.169.254/latest/meta-data/"));
        assert!(local("http://0.0.0.0/"));
        assert!(local("http://nas.local/"));
        assert!(local("http://[::1]/"));
    }

    /// Bypass classiques du filtre : IPv4-mapped IPv6, CGNAT (Tailscale),
    /// ULA/lien-local IPv6, et notations decimale/hexa de l'IPv4.
    #[test]
    fn detecte_les_contournements() {
        // `Ipv6Addr::is_loopback()` ne reconnait QUE `::1` : sans traitement
        // explicite, cette URL atteignait le loopback.
        assert!(local("http://[::ffff:127.0.0.1]/"));
        assert!(local("http://[::ffff:192.168.1.1]/"));
        // CGNAT 100.64/10 = plage Tailscale.
        assert!(local("http://100.64.0.1/"));
        assert!(local("http://100.127.255.254/"));
        // IPv6 ULA et lien-local.
        assert!(local("http://[fd00::1]/"));
        assert!(local("http://[fe80::1]/"));
        // Notations alternatives : normalisées par le parser WHATWG du crate
        // `url` (c'est ce que `host_str()` n'aurait pas donne).
        assert!(local("http://2130706433/"));
        assert!(local("http://0x7f.0.0.1/"));
    }

    /// Les hotes publics restent évidemment suivis.
    #[test]
    fn laisse_passer_les_hotes_publics() {
        assert!(!local("https://site.fr/"));
        assert!(!local("https://172.32.0.1/"));   // hors plage privee 172.16/12
        assert!(!local("https://11.0.0.1/"));     // hors plage privee 10/8
        assert!(!local("https://192.169.1.1/"));  // hors plage privee 192.168/16
        assert!(!local("https://100.128.0.1/"));  // hors CGNAT 100.64/10
        assert!(!local("https://100.63.0.1/"));   // hors CGNAT 100.64/10
        assert!(!local("http://[2001:db8::1]/")); // IPv6 public
    }
}
