use std::collections::{HashMap, HashSet};

use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use url::Url;

use crate::report::{AssetReport, TypeStat};

/// Number of asset downloads run concurrently.
const ASSET_CONCURRENCY: usize = 8;

pub struct HttpProbeResult {
    pub status: u16,
    pub headers: reqwest::header::HeaderMap,
    pub html: String,
    pub links: Vec<String>,
}

/// Resolve an `href`/`src` value against a base URL, keeping only http(s) URLs.
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

/// Light HTTP probe: GET the target, capture status/headers/body and extract
/// every absolute http(s) link found in `a[href]`.
///
/// `scraper::Html` is not `Send`, so the HTML is parsed and every URL is
/// collected into owned `String`s in a scope that ends before any `.await`.
pub async fn probe(
    client: &reqwest::Client,
    target: &str,
) -> anyhow::Result<HttpProbeResult> {
    let resp = client.get(target).send().await?;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    let html = resp.text().await?;

    let links = extract_links(target, &html);

    Ok(HttpProbeResult {
        status,
        headers,
        html,
        links,
    })
}

/// Parse the HTML and return the deduplicated list of absolute http(s) links.
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
            if let Some(abs) = resolve_url(&base, href) {
                if seen.insert(abs.clone()) {
                    links.push(abs);
                }
            }
        }
    }
    links
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
/// each asset the byte size is the `Content-Length` header when present,
/// otherwise the length of the downloaded body. Returns the per-asset reports,
/// the aggregated `by_type` map and the total number of bytes.
pub async fn measure_static_assets(
    client: &reqwest::Client,
    base_url: &str,
    html: &str,
) -> (
    Vec<AssetReport>,
    std::collections::HashMap<String, TypeStat>,
    u64,
) {
    // Extract every URL before any await: nothing from `scraper` is held here.
    let asset_urls = extract_static_asset_urls(base_url, html);

    // Download each asset concurrently and measure its transferred size.
    let reports: Vec<AssetReport> = stream::iter(asset_urls.into_iter())
        .map(|asset| {
            let client = client.clone();
            async move {
                let bytes = fetch_asset_bytes(&client, &asset.url).await;
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
async fn fetch_asset_bytes(client: &reqwest::Client, url: &str) -> u64 {
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return 0,
    };

    let content_length = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok());

    if let Some(len) = content_length {
        // Drain the body so the connection can be reused by the pool.
        let _ = resp.bytes().await;
        return len;
    }

    match resp.bytes().await {
        Ok(b) => b.len() as u64,
        Err(_) => 0,
    }
}
