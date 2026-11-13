use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Serialize, Deserialize};
use std::borrow::Cow;
use std::collections::HashMap;

/// Paramètres d'un texte écrit par le moteur (nombre, longueur, nom d'en-tête,
/// détail technique), indexés par leur nom dans la phrase : `{"len": 72}` pour
/// « Title trop long (72 car.) ». L'interface les réinsère dans la traduction
/// (cf. `codes`).
///
/// Écrit en OBJET JSON, exactement comme l'était le `BTreeMap<String, Value>`
/// d'origine : mêmes clés, dans le même ordre (alphabétique), et pour une clé
/// donnée deux fois, la dernière valeur. Mais tenu dans un `Vec` de paires :
/// un `BTreeMap` non vide réserve d'emblée un nœud de 11 places, soit ~650
/// octets pour UN paramètre — et presque chaque problème en porte un, gardé
/// dans `summary.pages` pendant tout le crawl. Ici : ~70 octets. Les noms de
/// paramètres du moteur sont des constantes (`Cow::Borrowed`, sans allocation).
/// Mesure : `crawl-core/tests/memoire_params.rs`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextParams(Vec<(Cow<'static, str>, serde_json::Value)>);

impl TextParams {
    pub fn new() -> Self {
        TextParams(Vec::new())
    }

    pub fn with_capacity(n: usize) -> Self {
        TextParams(Vec::with_capacity(n))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Ajoute un paramètre, ou remplace la valeur d'un nom déjà présent (comme
    /// `BTreeMap::insert`) ; les noms restent triés.
    pub fn insert(&mut self, key: impl Into<Cow<'static, str>>, value: serde_json::Value) {
        let key = key.into();
        match self.0.binary_search_by(|(k, _)| k.as_ref().cmp(key.as_ref())) {
            Ok(i) => self.0[i].1 = value,
            Err(i) => self.0.insert(i, (key, value)),
        }
    }

    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.0
            .binary_search_by(|(k, _)| k.as_ref().cmp(key))
            .ok()
            .map(|i| &self.0[i].1)
    }

    /// Les paramètres, par nom croissant.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.0.iter().map(|(k, v)| (k.as_ref(), v))
    }
}

impl std::ops::Index<&str> for TextParams {
    type Output = serde_json::Value;

    /// Comme pour un `BTreeMap` : panique si le paramètre n'existe pas.
    fn index(&self, key: &str) -> &serde_json::Value {
        self.get(key).expect("paramètre absent")
    }
}

impl Serialize for TextParams {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k.as_ref(), v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for TextParams {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ParamsVisitor;

        impl<'de> Visitor<'de> for ParamsVisitor {
            type Value = TextParams;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                // Même attente que le `BTreeMap` d'origine (messages d'erreur identiques).
                f.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<TextParams, A::Error> {
                let mut p = TextParams::with_capacity(map.size_hint().unwrap_or(0).min(16));
                while let Some((k, v)) = map.next_entry::<String, serde_json::Value>()? {
                    p.insert(k, v);
                }
                p.0.shrink_to_fit();
                Ok(p)
            }
        }

        deserializer.deserialize_map(ParamsVisitor)
    }
}

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
    /// Phrase française, INCHANGÉE (cf. `codes`) : l'interface s'en sert de repli.
    pub message: String,
    /// Code stable du problème (`codes::issue`, ex. "title_too_long") : clé de
    /// traduction `issue.<code>`. Vide dans les crawls enregistrés avant son
    /// ajout — l'interface reconnaît alors `message`. `Cow` : les codes émis
    /// sont des constantes (`Cow::Borrowed`, aucune allocation par problème) ;
    /// seule une relecture JSON en fait une chaîne possédée.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub code: Cow<'static, str>,
    /// Paramètres de `message` (`{"len": 72}`), omis quand il n'y en a pas.
    #[serde(default, skip_serializing_if = "TextParams::is_empty")]
    pub params: TextParams,
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
    /// Phrase française de l'erreur, INCHANGÉE (cf. `codes::PageError::text`).
    pub error: Option<String>,
    /// Code stable de `error` (`codes::error`, ex. "robots_blocked") : clé de
    /// traduction `err.<code>`. Absent des crawls enregistrés avant son ajout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Paramètres de `error` : `{"detail": "<message du système>"}`.
    #[serde(default, skip_serializing_if = "TextParams::is_empty")]
    pub error_params: TextParams,
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

impl PageReport {
    /// Pose l'erreur de la page : sa phrase historique (`error`), son code
    /// stable (`error_code`) et son détail (`error_params`), toujours ensemble.
    pub fn set_error(&mut self, e: &crate::codes::PageError) {
        self.error = Some(e.text());
        self.error_code = Some(e.code().to_string());
        self.error_params = e.params();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes::PageError;

    /// Un problème d'un crawl enregistré AVANT les codes se relit (code vide).
    #[test]
    fn ancien_probleme_sans_code_se_relit() {
        let json = r#"{"url":"https://a.fr/","severity":"info","category":"Securite","message":"En-tete securite manquant: Referrer-Policy"}"#;
        let it: Issue = serde_json::from_str(json).unwrap();
        assert_eq!(it.message, "En-tete securite manquant: Referrer-Policy");
        assert!(it.code.is_empty());
        assert!(it.params.is_empty());
    }

    /// Un nouveau problème garde ses quatre champs historiques, intacts, et
    /// AJOUTE `code` (+ `params` s'il y en a) : une ancienne version les ignore.
    #[test]
    fn nouveau_probleme_ajoute_code_et_params_sans_toucher_au_reste() {
        let it = Issue {
            url: "https://a.fr/".to_string(),
            severity: "warning".to_string(),
            category: "Titres".to_string(),
            message: "Title trop long (72 car.)".to_string(),
            code: "title_too_long".into(),
            params: crate::codes::params(&[("len", 72.into())]),
        };
        let v = serde_json::to_value(&it).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "url": "https://a.fr/", "severity": "warning", "category": "Titres",
                "message": "Title trop long (72 car.)",
                "code": "title_too_long", "params": {"len": 72}
            })
        );
        // Sans paramètre : pas de `params` du tout (les crawls restent légers).
        let sans = Issue { params: TextParams::new(), code: "title_missing".into(), message: "Title manquant".to_string(), ..it };
        let v = serde_json::to_value(&sans).unwrap();
        assert!(v.get("params").is_none());
        assert_eq!(v["code"], "title_missing");
    }

    /// Le JSON d'un problème, AU CARACTÈRE PRÈS (ordre des clés compris) : c'est
    /// ce texte que l'interface reçoit et enregistre dans les crawls.
    #[test]
    fn json_d_un_probleme_au_caractere_pres() {
        let it = Issue {
            url: "https://a.fr/".to_string(),
            severity: "info".to_string(),
            category: "Sécurité".to_string(),
            message: "En-tête sécurité manquant: X-Frame-Options".to_string(),
            code: "security_header_missing".into(),
            params: crate::codes::params(&[("header", "X-Frame-Options".into())]),
        };
        assert_eq!(
            serde_json::to_string(&it).unwrap(),
            r#"{"url":"https://a.fr/","severity":"info","category":"Sécurité","message":"En-tête sécurité manquant: X-Frame-Options","code":"security_header_missing","params":{"header":"X-Frame-Options"}}"#
        );
        let ancien = Issue { code: "".into(), params: TextParams::new(), ..it };
        assert_eq!(
            serde_json::to_string(&ancien).unwrap(),
            r#"{"url":"https://a.fr/","severity":"info","category":"Sécurité","message":"En-tête sécurité manquant: X-Frame-Options"}"#
        );
    }

    /// `TextParams` écrit EXACTEMENT le JSON du `BTreeMap<String, Value>` qu'il
    /// remplace — clés triées, dernière valeur gardée pour une clé en double,
    /// objet vide — et relit ce JSON à l'identique.
    #[test]
    fn texte_params_meme_json_que_le_btreemap_d_origine() {
        use std::collections::BTreeMap;
        let cas: Vec<Vec<(&'static str, serde_json::Value)>> = vec![
            vec![],
            vec![("len", 72.into())],
            vec![("header", "X-Frame-Options".into())],
            vec![("detail", "Bloc 1: @type manquant".into())],
            // Plusieurs noms, dans le désordre, dont un en double et des valeurs variées.
            vec![
                ("n", 3.into()),
                ("detail", "é « guillemets » \"échappés\"\n".into()),
                ("a", serde_json::json!({"x": [1, 2.5, null, true]})),
                ("n", 4.into()),
                ("Z", (-1).into()),
            ],
        ];
        for paires in cas {
            let mut ancien: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            let mut nouveau = TextParams::new();
            for (k, v) in &paires {
                ancien.insert((*k).to_string(), v.clone());
                nouveau.insert(*k, v.clone());
            }
            let json_ancien = serde_json::to_string(&ancien).unwrap();
            assert_eq!(serde_json::to_string(&nouveau).unwrap(), json_ancien, "écrit");
            assert_eq!(serde_json::to_string(&crate::codes::params(&paires)).unwrap(), json_ancien, "codes::params");
            let relu: TextParams = serde_json::from_str(&json_ancien).unwrap();
            assert_eq!(relu, nouveau, "relu depuis {json_ancien}");
            assert_eq!(relu.len(), ancien.len());
            for (k, v) in &ancien {
                assert_eq!(relu.get(k), Some(v));
            }
        }
        // Un objet écrit dans le désordre, avec un doublon : relu comme le BTreeMap le relit.
        let brut = r#"{"b":1,"a":"x","b":2}"#;
        let ancien: BTreeMap<String, serde_json::Value> = serde_json::from_str(brut).unwrap();
        let nouveau: TextParams = serde_json::from_str(brut).unwrap();
        assert_eq!(serde_json::to_string(&nouveau).unwrap(), serde_json::to_string(&ancien).unwrap());
        // Ce qui n'est pas un objet est refusé, comme avant (même message d'erreur).
        for pas_un_objet in ["null", "[]", "3", "\"x\""] {
            let e_ancien = serde_json::from_str::<BTreeMap<String, serde_json::Value>>(pas_un_objet).unwrap_err().to_string();
            let e_nouveau = serde_json::from_str::<TextParams>(pas_un_objet).unwrap_err().to_string();
            assert_eq!(e_nouveau, e_ancien);
        }
    }

    /// Une page d'un crawl réel d'avant les codes (copie d'un enregistrement de
    /// Chris, juillet 2026) se relit, sans code ni paramètres d'erreur.
    #[test]
    fn ancienne_page_en_erreur_se_relit() {
        let json = r#"{"url":"https://site-exemple.fr/","status":0,"tech":[],"total_bytes":0,"by_type":{},"request_count":0,"dynamic_requests":0,"vercel_cache_hits":0,"vercel_cache_misses":0,"images":[],"measured_with":"http","error":"sonde HTTP echouee: error sending request for url (https://site-exemple.fr/)","seo":null,"issues":[],"content_type":null,"response_time_ms":0,"redirect_to":null,"redirect_chain":[],"outlinks":[],"security":null,"url_flags":null}"#;
        let p: PageReport = serde_json::from_str(json).unwrap();
        assert_eq!(p.error.as_deref(), Some("sonde HTTP echouee: error sending request for url (https://site-exemple.fr/)"));
        assert!(p.error_code.is_none());
        assert!(p.error_params.is_empty());
    }

    /// `set_error` pose la phrase historique ET son code ; une page sans erreur
    /// n'écrit aucun des deux nouveaux champs.
    #[test]
    fn erreur_de_page_ecrit_phrase_code_et_detail() {
        let mut p = PageReport { url: "https://a.fr/".to_string(), ..Default::default() };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("error_code").is_none() && v.get("error_params").is_none());
        assert!(v["error"].is_null());

        p.set_error(&PageError::HttpProbeFailed("error decoding response body".to_string()));
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["error"], "sonde HTTP echouee: error decoding response body");
        assert_eq!(v["error_code"], "http_probe_failed");
        assert_eq!(v["error_params"], serde_json::json!({"detail": "error decoding response body"}));

        p.set_error(&PageError::RobotsBlocked);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["error"], "Bloque par robots.txt");
        assert_eq!(v["error_code"], "robots_blocked");
        assert!(v.get("error_params").is_none());

        // Au caractère près, comme la première forme (BTreeMap) l'écrivait.
        let detail = "error sending request for url (https://a.fr/?q=\"é\")";
        for e in [
            PageError::HttpProbeFailed(detail.to_string()),
            PageError::ChromeLaunchFailed(detail.to_string()),
            PageError::ChromeMeasureFailed(detail.to_string()),
            PageError::ChromeNavigationFailed(detail.to_string()),
        ] {
            let mut premiere_forme = std::collections::BTreeMap::new();
            premiere_forme.insert("detail".to_string(), serde_json::Value::from(detail));
            assert_eq!(
                serde_json::to_string(&e.params()).unwrap(),
                serde_json::to_string(&premiere_forme).unwrap()
            );
        }
    }
}
