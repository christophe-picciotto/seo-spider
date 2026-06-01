use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReport {
    pub url: String,
    pub bytes: u64,
    pub kind: String, // "Image" | "Script" | "Stylesheet" | "Font" | "Document" | "Fetch" | "Other"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeStat {
    pub bytes: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hreflang {
    pub lang: String,
    pub href: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeoData {
    pub title: Option<String>,
    pub title_len: usize,
    pub meta_description: Option<String>,
    pub meta_description_len: usize,
    pub h1: Vec<String>,
    pub h1_count: usize,
    pub h2: Vec<String>,
    pub canonical: Option<String>,
    pub meta_robots: Option<String>,
    pub noindex: bool,
    pub nofollow: bool,
    pub indexable: bool,
    pub lang: Option<String>,
    pub viewport: Option<String>,
    pub charset: Option<String>,
    pub word_count: usize,
    pub text_ratio: f32,
    pub og: HashMap<String, String>,
    pub twitter: HashMap<String, String>,
    pub images_total: u32,
    pub images_missing_alt: u32,
    pub internal_links: u32,
    pub external_links: u32,
    pub hreflang: Vec<Hreflang>,
    pub structured_data_types: Vec<String>,
    pub x_robots_tag: Option<String>,
    pub pagination_next: Option<String>,
    pub pagination_prev: Option<String>,
    pub canonicalised: bool,            // canonical pointe vers une AUTRE url (page non-indexable)
    pub structured_data_valid: bool,
    pub structured_data_errors: Vec<String>,
    #[serde(default)]
    pub simhash: String,                // empreinte 64 bits en HEX (16 chars) du contenu visible, pour near-duplicates
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityData {
    pub https: bool,
    pub mixed_content: Vec<String>,
    pub missing_headers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrlFlags {
    pub length: usize,
    pub non_ascii: bool,
    pub underscores: bool,
    pub uppercase: bool,
    pub multiple_slashes: bool,
    pub params: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub url: String,
    pub severity: String,  // "critical" | "warning" | "opportunity" | "info"
    pub category: String,  // "Titres" | "Meta description" | "H1" | "Images" | "Canonical" | "Directives" | "Hreflang" | "Reponse"
    pub message: String,
}

/// A single outgoing link discovered on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    pub target: String,
    pub anchor: String,
    pub rel: Option<String>,
    pub internal: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageReport {
    pub url: String,
    pub status: u16,
    pub tech: Vec<String>,
    pub total_bytes: u64,            // bande passante reelle (octets transferes)
    pub by_type: HashMap<String, TypeStat>,
    pub request_count: u32,
    pub dynamic_requests: u32,       // cache MISS + /api + RSC (proxy invocations serverless)
    pub vercel_cache_hits: u32,
    pub vercel_cache_misses: u32,
    pub images: Vec<AssetReport>,
    pub measured_with: String,       // "http" | "chrome"
    pub error: Option<String>,
    #[serde(default)]
    pub seo: Option<SeoData>,
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub response_time_ms: u64,
    #[serde(default)]
    pub redirect_to: Option<String>,
    #[serde(default)]
    pub redirect_chain: Vec<String>,
    #[serde(default)]
    pub outlinks: Vec<LinkInfo>,
    #[serde(default)]
    pub security: Option<SecurityData>,
    #[serde(default)]
    pub url_flags: Option<UrlFlags>,
}

/// Default value helper for serde (`respect_robots` defaults to true).
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    pub seed_url: String,
    pub max_pages: usize,
    pub max_depth: usize,
    pub same_domain_only: bool,
    pub requests_per_second: u32,
    pub force_chrome: bool,
    /// Si true, chaque page est crawlee avec un user-agent + viewport tires du pool.
    #[serde(default)]
    pub random_user_agent: bool,
    /// Si true, on attend un delai ALEATOIRE entre chaque requete (comportement
    /// "humain") au lieu du debit fixe `requests_per_second`.
    #[serde(default)]
    pub random_delay: bool,
    #[serde(default)]
    pub unlimited: bool,
    #[serde(default = "default_true")]
    pub respect_robots: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlProgress {
    pub done: usize,
    pub queued: usize,
    pub current_url: String,
    pub total_bytes_so_far: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlSummary {
    pub pages: Vec<PageReport>,
    pub total_bytes: u64,
    pub total_requests: u32,
    pub total_dynamic_requests: u32,
    pub tech_detected: Vec<String>,
    #[serde(default)]
    pub sitemap_urls: Vec<String>,
}
