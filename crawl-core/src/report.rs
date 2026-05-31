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
}
