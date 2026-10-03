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
    /// `src` de chaque image SANS attribut alt, en URL absolue quand la
    /// resolution est possible. Le compteur seul ne permet pas de retrouver les
    /// images en defaut, donc pas de les corriger. Dedoublonne et plafonne
    /// (cf. `seo::MAX_MISSING_ALT_URLS`). `default` : les projets enregistres
    /// avant ce champ se rechargent sans lui.
    #[serde(default)]
    pub images_missing_alt_urls: Vec<String>,
    pub internal_links: u32,
    pub external_links: u32,
    pub hreflang: Vec<Hreflang>,
    pub structured_data_types: Vec<String>,
    #[serde(default)]
    pub structured_data_raw: Vec<String>,   // blocs JSON-LD bruts (texte du script ld+json), pour affichage detaille
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

/// Une couche de cache traversee par la réponse (cf. `cachestat`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheLayer {
    /// En-tête qui l'a revelee (ex: "cf-cache-status"), ou "html:wp-rocket"
    /// quand la couche a été reconnue à sa signature dans le document.
    pub header: String,
    /// Nom lisible du moteur : "Cloudflare", "HiveCache", "WP Rocket"...
    pub engine: String,
    /// Valeur brute de l'en-tête, telle que servie.
    pub raw: String,
    /// Verdict normalisé de CETTE couche. Vide quand le moteur est identifie
    /// mais que rien ne permet de dire s'il a servi la page.
    pub status: String,
}

/// Statut de cache de la réponse, lu dans ses en-têtes (cf. `cachestat`).
///
/// Renseigne des la sonde HTTP, donc présent sur TOUTE page crawlée, y compris
/// celles mesurées ensuite au navigateur.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheData {
    /// Verdict normalisé : "HIT" | "MISS" | "BYPASS" | "EXPIRED" |
    /// "REVALIDATED" | "STALE" | "UPDATING" | "PRERENDER" | "DYNAMIC" | "NONE",
    /// ou la valeur brute en majuscules si le serveur en a dit une autre.
    ///
    /// C'est le verdict de la couche la plus EXTÉRIEURE : ce qu'a vecu le
    /// visiteur.
    pub status: String,
    /// En-tête qui a tranche (ex: "cf-cache-status"), pour la tracabilite.
    pub source: String,
    /// Moteur derriere cet en-tête : "Cloudflare", "HiveCache"...
    #[serde(default)]
    pub engine: String,
    /// Qui a RÉELLEMENT servi la page : le premier cache a avoir repondu sans
    /// solliciter le serveur, ou "origine" si la requête est allée jusqu'à lui.
    ///
    /// Distinct de `status` : « MISS » côté Cloudflare avec `served_by` a
    /// "HiveCache" signifie que le CDN a laisse passer mais que le cache du
    /// serveur a fourni la page — le PHP n'a pas tourne.
    #[serde(default)]
    pub served_by: String,
    /// Valeur brute de cet en-tête, telle que servie.
    pub raw: String,
    /// `Age` en secondes : temps passe dans le cache. `None` si absent.
    pub age: Option<u64>,
    /// TOUTES les couches traversees, de la plus extérieure à la plus interne.
    ///
    /// Leur desaccord est l'information la plus parlante : Cloudflare en MISS
    /// avec HiveCache en HIT dit que la page etait en cache côté serveur mais
    /// que le CDN est alle la chercher quand même.
    #[serde(default)]
    pub layers: Vec<CacheLayer>,
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
    pub category: String,  // "Titres" | "Meta description" | "H1" | "Images" | "Canonical" | "Directives" | "Hreflang" | "Réponse"
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
    pub total_bytes: u64,            // bande passante réelle (octets transférés)
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
    /// Statut de cache de la réponse (HIT/MISS/...). `None` quand aucun en-tête
    /// ne permet de conclure — l'absence d'information n'est pas un MISS.
    #[serde(default)]
    pub cache: Option<CacheData>,
}

/// Default value helper for serde (`threads` defaults to 5).
fn default_threads() -> usize {
    5
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
    /// Si true, chaque page est crawlée avec un user-agent + viewport tires du pool.
    #[serde(default)]
    pub random_user_agent: bool,
    /// Si true, on attend un délai ALÉATOIRE entre chaque requête (comportement
    /// "humain") au lieu du débit fixe `requests_per_second`.
    #[serde(default)]
    pub random_delay: bool,
    #[serde(default)]
    pub unlimited: bool,
    #[serde(default = "default_true")]
    pub respect_robots: bool,
    #[serde(default = "default_threads")]
    pub threads: usize,   // nb de workers paralleles
    /// Si true, AUCUN plafond de débit : `requests_per_second` est ignore et la
    /// vitesse n'est plus bornée que par `threads` et le temps de réponse du
    /// serveur.
    ///
    /// Sans ca, le rate-limiter est un bucket GLOBAL partage par tous les
    /// workers : le débit reste colle a `requests_per_second` quel que soit le
    /// nombre de threads (avec 5 rps et 80 ms de réponse, UN SEUL worker suffit
    /// a saturer le quota — les autres attendent).
    #[serde(default)]
    pub no_rate_limit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlProgress {
    pub done: usize,
    pub queued: usize,
    pub current_url: String,
    pub total_bytes_so_far: u64,
    /// Workers réellement en train de traiter une page à cet instant.
    ///
    /// Permet de VOIR si les threads configurés servent : si ce nombre reste
    /// bien en dessous de `config.threads`, c'est que le goulot est ailleurs
    /// (plafond de débit, file vide, ou mesures Chrome serialisees).
    #[serde(default)]
    pub active_workers: usize,
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
