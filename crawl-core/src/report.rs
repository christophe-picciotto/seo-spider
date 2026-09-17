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

/// Une couche de cache traversee par la reponse (cf. `cachestat`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheLayer {
    /// En-tete qui l'a revelee (ex: "cf-cache-status"), ou "html:wp-rocket"
    /// quand la couche a ete reconnue a sa signature dans le document.
    pub header: String,
    /// Nom lisible du moteur : "Cloudflare", "HiveCache", "WP Rocket"...
    pub engine: String,
    /// Valeur brute de l'en-tete, telle que servie.
    pub raw: String,
    /// Verdict normalise de CETTE couche. Vide quand le moteur est identifie
    /// mais que rien ne permet de dire s'il a servi la page.
    pub status: String,
}

/// Statut de cache de la reponse, lu dans ses en-tetes (cf. `cachestat`).
///
/// Renseigne des la sonde HTTP, donc present sur TOUTE page crawlee, y compris
/// celles mesurees ensuite au navigateur.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheData {
    /// Verdict normalise : "HIT" | "MISS" | "BYPASS" | "EXPIRED" |
    /// "REVALIDATED" | "STALE" | "UPDATING" | "PRERENDER" | "DYNAMIC" | "NONE",
    /// ou la valeur brute en majuscules si le serveur en a dit une autre.
    ///
    /// C'est le verdict de la couche la plus EXTERIEURE : ce qu'a vecu le
    /// visiteur.
    pub status: String,
    /// En-tete qui a tranche (ex: "cf-cache-status"), pour la tracabilite.
    pub source: String,
    /// Moteur derriere cet en-tete : "Cloudflare", "HiveCache"...
    #[serde(default)]
    pub engine: String,
    /// Qui a REELLEMENT servi la page : le premier cache a avoir repondu sans
    /// solliciter le serveur, ou "origine" si la requete est allee jusqu'a lui.
    ///
    /// Distinct de `status` : « MISS » cote Cloudflare avec `served_by` a
    /// "HiveCache" signifie que le CDN a laisse passer mais que le cache du
    /// serveur a fourni la page — le PHP n'a pas tourne.
    #[serde(default)]
    pub served_by: String,
    /// Valeur brute de cet en-tete, telle que servie.
    pub raw: String,
    /// `Age` en secondes : temps passe dans le cache. `None` si absent.
    pub age: Option<u64>,
    /// TOUTES les couches traversees, de la plus exterieure a la plus interne.
    ///
    /// Leur desaccord est l'information la plus parlante : Cloudflare en MISS
    /// avec HiveCache en HIT dit que la page etait en cache cote serveur mais
    /// que le CDN est alle la chercher quand meme.
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
    /// Statut de cache de la reponse (HIT/MISS/...). `None` quand aucun en-tete
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
    #[serde(default = "default_threads")]
    pub threads: usize,   // nb de workers paralleles
    /// Si true, AUCUN plafond de debit : `requests_per_second` est ignore et la
    /// vitesse n'est plus bornee que par `threads` et le temps de reponse du
    /// serveur.
    ///
    /// Sans ca, le rate-limiter est un bucket GLOBAL partage par tous les
    /// workers : le debit reste colle a `requests_per_second` quel que soit le
    /// nombre de threads (avec 5 rps et 80 ms de reponse, UN SEUL worker suffit
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
    /// Workers reellement en train de traiter une page a cet instant.
    ///
    /// Permet de VOIR si les threads configures servent : si ce nombre reste
    /// bien en dessous de `config.threads`, c'est que le goulot est ailleurs
    /// (plafond de debit, file vide, ou mesures Chrome serialisees).
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
