//! Statut de CACHE d'une reponse : qui l'a servie, et depuis son cache ou non.
//!
//! Savoir si une page a ete servie depuis un cache (HIT) ou recalculee par
//! l'origine (MISS) dit deux choses a un audit :
//!
//!   - le temps de reponse mesure n'a pas le meme sens — un HIT flatte la
//!     vitesse, un MISS montre le vrai cout de l'origine ;
//!   - un site cense etre entierement cache qui repond MISS partout revele une
//!     regle de cache qui ne prend pas.
//!
//! Aucun en-tete n'est normalise entre CDN : chacun a le sien, avec son propre
//! vocabulaire (`HIT`, `TCP_MEM_HIT`, `Miss from cloudfront`...). `analyze` les
//! essaie du plus exterieur au plus interne et en tire trois informations :
//!
//!   - `status`   : le verdict de la couche la plus EXTERIEURE (ce qu'a vecu
//!                  le visiteur) ;
//!   - `served_by`: qui a REELLEMENT servi la page — le premier cache en HIT,
//!                  ou « origine » si la requete est allee jusqu'au serveur ;
//!   - `layers`   : toutes les couches traversees, pour voir ou ca coince.

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::HeaderMap;

use crate::report::{CacheData, CacheLayer};

/// En-tetes de statut de cache connus, du plus EXTERIEUR au plus interne, avec
/// le nom lisible du moteur derriere chacun.
///
/// L'ordre compte : une reponse en porte souvent plusieurs — un WordPress
/// derriere Cloudflare en expose deux — et c'est le cache le plus exterieur,
/// celui qui a reellement repondu au visiteur, qui donne le verdict. Un HIT
/// HiveCache sous un MISS Cloudflare reste un MISS pour le visiteur : la
/// requete est bel et bien allee jusqu'au serveur.
///
/// Un moteur vide signifie que l'en-tete est PARTAGE par plusieurs CDN et que
/// le nom doit etre devine sur d'autres indices (cf. `guess_cdn`).
const CACHE_HEADERS: &[(&str, &str)] = &[
    // --- Caches de peripherie (CDN) : ils repondent AVANT l'origine ---------
    ("cf-cache-status", "Cloudflare"),
    ("cf-apo-status", "Cloudflare APO"),
    ("x-vercel-cache", "Vercel"),
    ("x-nc", "Netlify"),
    ("x-cache", ""),          // CloudFront, Fastly, Akamai, Varnish...
    ("x-cdn-cache", ""),
    ("cdn-cache", ""),
    // --- Caches d'origine : ils n'ont servi que si le CDN a laisse passer ---
    ("x-hivecache", "HiveCache"),
    ("x-rocket-nginx-serving-static", "WP Rocket"),
    ("x-litespeed-cache", "LiteSpeed"),
    ("x-fastcgi-cache", "Nginx FastCGI"),
    ("x-cache-status", "Nginx"),
    ("x-proxy-cache", "Nginx"),
    ("x-varnish-cache", "Varnish"),
    ("x-drupal-cache", "Drupal"),
];

/// En-tetes portant un AGE de cache en secondes, par ordre de preference.
///
/// `age` est le standard (RFC 9111) ; les caches d'origine exposent souvent le
/// leur en plus, seule source disponible sur un site sans CDN devant.
const AGE_HEADERS: &[&str] = &["age", "x-hivecache-age", "x-cache-age"];

/// Statuts signifiant que le cache a repondu SANS solliciter l'origine.
///
/// `REVALIDATED` et `EXPIRED` n'en font pas partie : dans les deux cas le cache
/// a contacte le serveur pour verifier sa copie. Pour la question « est-ce
/// qu'on est alle taper le site ? », la reponse y est oui.
const SERVED_FROM_CACHE: &[&str] = &["HIT", "STALE", "UPDATING"];

/// Nom retenu quand aucun cache n'a servi : la page vient du serveur.
const ORIGIN: &str = "origine";

/// Ramene la valeur brute d'un en-tete de cache a un verdict court.
///
/// La valeur est decoupee en mots : `TCP_MEM_HIT` (Akamai), `Miss from
/// cloudfront` (CloudFront) et `hit` (Cloudflare) portent tous leur verdict
/// dans un mot isole. Le PREMIER mot reconnu tranche — Fastly repond
/// « HIT, MISS » quand deux couches se repondent, et c'est la plus externe,
/// donc la premiere, qui a servi le visiteur.
///
/// `None` si aucun mot n'est reconnu : l'appelant conserve alors la valeur
/// brute plutot que d'inventer un verdict.
fn normalize(raw: &str) -> Option<&'static str> {
    for token in raw.split(|c: char| !c.is_ascii_alphanumeric()) {
        let t = token.to_ascii_lowercase();
        match t.as_str() {
            "hit" | "cached" | "fresh" => return Some("HIT"),
            "miss" | "uncached" => return Some("MISS"),
            "bypass" | "ignored" | "nocache" => return Some("BYPASS"),
            "expired" => return Some("EXPIRED"),
            "revalidated" => return Some("REVALIDATED"),
            "stale" => return Some("STALE"),
            "updating" => return Some("UPDATING"),
            "prerender" => return Some("PRERENDER"),
            // Cloudflare : la ressource n'est pas eligible au cache (aucune
            // regle ne la couvre). Ce n'est PAS un echec de cache, c'est
            // l'absence de regle — distinction utile en audit.
            "dynamic" => return Some("DYNAMIC"),
            "none" | "unknown" => return Some("NONE"),
            _ => {}
        }
    }
    None
}

/// Vrai si un nom d'en-tete ressemble a un statut de cache proprietaire.
///
/// Filet pour les caches maison et les plugins qui posent leur propre en-tete
/// (`x-wp-super-cache`...) : on ne peut pas tous les lister, mais leur nom
/// contient « cache » et leur valeur dit hit/miss.
///
/// `cache-control` est exclu explicitement : son nom correspond mais sa valeur
/// est une DIRECTIVE (`max-age=600, public`), jamais un statut. Idem pour les
/// en-tetes de cle, de duree ou d'age.
fn looks_like_cache_header(name: &str) -> bool {
    if !name.contains("cache") {
        return false;
    }
    !matches!(
        name,
        "cache-control"
            | "x-cache-key"
            | "x-cache-ttl"
            | "x-cache-expires"
            | "x-cache-hits"
            | "x-cache-age"
            | "x-hivecache-age"
            | "x-hivecache-preload"
    )
}

/// Nom lisible deduit d'un en-tete inconnu : `x-wp-super-cache` -> « Wp Super ».
///
/// Approximatif par nature, mais toujours plus parlant que le nom brut, et
/// l'en-tete d'origine reste disponible dans `CacheLayer::header`.
fn engine_from_header(name: &str) -> String {
    let cleaned = name
        .trim_start_matches("x-")
        .replace("-cache", "")
        .replace("cache-", "")
        .replace('-', " ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return name.to_string();
    }
    // Majuscule initiale de chaque mot.
    cleaned
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Devine le CDN derriere un en-tete partage (`x-cache` et consorts).
///
/// `x-cache` est utilise par CloudFront, Fastly, Akamai et Varnish sans qu'on
/// puisse les distinguer sur sa seule valeur. On s'appuie donc sur les en-tetes
/// signature que chacun ajoute par ailleurs.
fn guess_cdn(headers: &HeaderMap) -> String {
    if headers.contains_key("x-amz-cf-id") || headers.contains_key("x-amz-cf-pop") {
        return "CloudFront".to_string();
    }
    if headers.contains_key("x-served-by") || headers.contains_key("x-fastly-request-id") {
        return "Fastly".to_string();
    }
    if headers.contains_key("x-akamai-transformed") || headers.contains_key("akamai-grn") {
        return "Akamai".to_string();
    }
    if let Some(v) = header_lower(headers, "via") {
        if v.contains("varnish") {
            return "Varnish".to_string();
        }
        if v.contains("cloudfront") {
            return "CloudFront".to_string();
        }
    }
    if let Some(s) = header_lower(headers, "server") {
        if s.contains("cloudflare") {
            return "Cloudflare".to_string();
        }
        if s.contains("cloudfront") {
            return "CloudFront".to_string();
        }
        if s.contains("varnish") {
            return "Varnish".to_string();
        }
    }
    "CDN".to_string()
}

/// Lit la valeur d'un header (insensible a la casse) en minuscules.
fn header_lower(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())
}

/// Age de l'entree en cache, en secondes, lu sur le premier en-tete disponible.
fn read_age(headers: &HeaderMap) -> Option<u64> {
    for name in AGE_HEADERS {
        if let Some(v) = headers
            .get(*name)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
        {
            return Some(v);
        }
    }
    None
}

/// Les `n` derniers octets d'une chaine, coupes sur une frontiere de caractere.
///
/// `len()` compte des OCTETS : trancher a l'aveugle ferait paniquer des qu'un
/// caractere multi-octets chevauche la borne. On recule donc jusqu'a la
/// derniere frontiere valide (3 tours au pire).
fn tail(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut start = s.len() - n;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Detecte WP Rocket dans le HTML, faute d'en-tete HTTP.
///
/// POURQUOI c'est necessaire : WP Rocket seul (mode PHP) n'emet AUCUN en-tete
/// de statut. Seule son extension serveur Rocket-Nginx pose un
/// `x-rocket-nginx-serving-static`. Sans elle, une page parfaitement servie
/// depuis le cache WP Rocket serait indiscernable d'une page non cachee. Le
/// plugin laisse en revanche sa signature en fin de document :
///
/// ```html
/// <!-- ... Performance optimized by WP Rocket ... Debug: cached@1712345678 -->
/// ```
///
/// COMMENT on tranche HIT/MISS : le `cached@<epoch>` date la creation du
/// fichier de cache. Le comparer a l'heure courante donne l'age de l'entree —
/// une page servie depuis le cache a un age de plusieurs secondes au moins,
/// alors qu'une page qui vient d'etre generee (et qui PEUPLE le cache, en
/// portant deja le commentaire) a un age quasi nul.
///
/// LIMITE ASSUMEE : la comparaison se fait sur l'horloge LOCALE, pas sur celle
/// du serveur. Un decalage de quelques secondes est sans effet grace au seuil ;
/// une machine franchement deregle rendrait le verdict inexploitable, et l'age
/// aberrant est alors ecarte.
fn wp_rocket_from_html(html: &str) -> Option<CacheLayer> {
    // La signature est en FIN de document : inutile de scanner tout le HTML.
    let end = tail(html, 4096).to_ascii_lowercase();
    if !end.contains("wp rocket") && !end.contains("wp-rocket") {
        return None;
    }

    // Age de l'entree, si le commentaire de debogage est present.
    let age = end.find("cached@").and_then(|pos| {
        let rest = &end[pos + "cached@".len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let cached_at = digits.parse::<i64>().ok()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        let delta = now - cached_at;
        // Age negatif (horloge locale en retard) ou superieur a un an :
        // incoherent, on n'en tire rien.
        if (0..31_536_000).contains(&delta) {
            Some(delta as u64)
        } else {
            None
        }
    });

    let status = match age {
        // Entree deja ancienne : la page a bien ete servie depuis le cache.
        Some(a) if a > 10 => "HIT",
        // Entree creee a l'instant : c'est CETTE requete qui a peuple le cache.
        Some(_) => "MISS",
        // Signature presente mais pas de `cached@` : on sait que WP Rocket est
        // actif, pas s'il a servi. Mieux vaut ne rien affirmer.
        None => "",
    };

    Some(CacheLayer {
        header: "html:wp-rocket".to_string(),
        engine: "WP Rocket".to_string(),
        raw: match age {
            Some(a) => format!("cached@ il y a {a} s"),
            None => "signature HTML".to_string(),
        },
        status: status.to_string(),
    })
}

/// Determine le statut de cache d'une reponse a partir de ses en-tetes et,
/// en dernier recours, de son HTML.
///
/// Strategie en quatre passes, de la plus sure a la plus generale :
///   1. les en-tetes connus, dans l'ordre du plus exterieur au plus interne ;
///   2. tout en-tete dont le nom evoque un cache et dont la valeur dit hit/miss
///      (caches maison, plugins WordPress) ;
///   3. la signature HTML de WP Rocket, qui n'emet pas d'en-tete sans son
///      module serveur ;
///   4. `Age` (RFC 9111) : un age > 0 PROUVE qu'un intermediaire a stocke puis
///      servi la reponse, meme sans en-tete proprietaire. Un age de 0 ne prouve
///      rien — une entree fraichement stockee vaut 0 elle aussi.
///
/// `None` quand rien de tout cela n'aboutit : l'absence d'information n'est pas
/// un MISS, et l'afficher comme tel serait faux.
pub fn analyze(headers: &HeaderMap, html: &str) -> Option<CacheData> {
    let age = read_age(headers);
    let mut layers: Vec<CacheLayer> = Vec::new();

    // --- 1. En-tetes connus, dans l'ordre ---------------------------------
    for (name, engine) in CACHE_HEADERS {
        let Some(raw) = headers.get(*name).and_then(|v| v.to_str().ok()) else {
            continue;
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        layers.push(CacheLayer {
            header: (*name).to_string(),
            // Moteur vide dans la table = en-tete partage entre plusieurs CDN.
            engine: if engine.is_empty() { guess_cdn(headers) } else { (*engine).to_string() },
            raw: raw.to_string(),
            // Valeur non reconnue : on la remonte telle quelle plutot que de
            // trancher a tort. L'interface affiche ce qu'a dit le serveur.
            status: normalize(raw)
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw.to_uppercase()),
        });
    }

    // --- 2. Passe generique sur les en-tetes « *cache* » ------------------
    for (name, value) in headers.iter() {
        let n = name.as_str().to_ascii_lowercase();
        if !looks_like_cache_header(&n) || CACHE_HEADERS.iter().any(|(h, _)| *h == n) {
            continue; // deja traite a la passe 1
        }
        let Some(raw) = value.to_str().ok().map(str::trim) else {
            continue;
        };
        // Ici on EXIGE un verdict reconnu : sans liste blanche pour garantir
        // qu'il s'agit d'un statut, une valeur inconnue serait du bruit.
        if let Some(status) = normalize(raw) {
            layers.push(CacheLayer {
                header: n.clone(),
                engine: engine_from_header(&n),
                raw: raw.to_string(),
                status: status.to_string(),
            });
        }
    }

    // --- 3. WP Rocket sans en-tete (signature HTML) ------------------------
    // Ajoute en DERNIER : c'est un cache d'origine, le plus interne de tous.
    if !layers.iter().any(|l| l.engine == "WP Rocket") {
        if let Some(l) = wp_rocket_from_html(html) {
            layers.push(l);
        }
    }

    if !layers.is_empty() {
        // Verdict = couche la plus exterieure (la premiere collectee).
        let first = &layers[0];
        // Qui a REELLEMENT servi : le premier cache a avoir repondu sans
        // solliciter l'origine. Aucun -> la requete est allee au serveur.
        let served_by = layers
            .iter()
            .find(|l| SERVED_FROM_CACHE.contains(&l.status.as_str()))
            .map(|l| l.engine.clone())
            .unwrap_or_else(|| ORIGIN.to_string());
        return Some(CacheData {
            status: first.status.clone(),
            source: first.header.clone(),
            engine: first.engine.clone(),
            served_by,
            raw: first.raw.clone(),
            age,
            layers,
        });
    }

    // --- 4. Repli sur l'age ------------------------------------------------
    if let Some(a) = age {
        if a > 0 {
            return Some(CacheData {
                status: "HIT".to_string(),
                source: "age".to_string(),
                engine: "cache HTTP".to_string(),
                served_by: "cache HTTP".to_string(),
                raw: a.to_string(),
                age,
                layers: Vec::new(),
            });
        }
    }

    // --- 5. Aucune information ---------------------------------------------
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }
    /// Raccourci : analyse sans HTML.
    fn an(pairs: &[(&str, &str)]) -> Option<CacheData> {
        analyze(&hm(pairs), "")
    }

    #[test]
    fn cloudflare_hit() {
        let c = an(&[("cf-cache-status", "HIT"), ("age", "42")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.source, "cf-cache-status");
        assert_eq!(c.engine, "Cloudflare");
        assert_eq!(c.served_by, "Cloudflare");
        assert_eq!(c.age, Some(42));
    }

    #[test]
    fn cloudflare_dynamic_nest_pas_un_miss() {
        let c = an(&[("cf-cache-status", "DYNAMIC")]).unwrap();
        assert_eq!(c.status, "DYNAMIC");
        // Non eligible au cache : c'est bien le serveur qui a repondu.
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn cloudfront_phrase_complete() {
        let c = an(&[("x-cache", "Miss from cloudfront"), ("x-amz-cf-id", "abc")]).unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.engine, "CloudFront");
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn akamai_tcp_mem_hit() {
        let c = an(&[("x-cache", "TCP_MEM_HIT from a23-1-2-3"), ("akamai-grn", "0.1")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.engine, "Akamai");
    }

    #[test]
    fn fastly_reconnu_par_sa_signature() {
        let c = an(&[("x-cache", "HIT"), ("x-served-by", "cache-cdg123")]).unwrap();
        assert_eq!(c.engine, "Fastly");
    }

    #[test]
    fn cdn_indeterminable_reste_generique() {
        let c = an(&[("x-cache", "HIT")]).unwrap();
        assert_eq!(c.engine, "CDN");
    }

    #[test]
    fn le_cache_le_plus_exterieur_donne_le_verdict() {
        // HiveCache a servi depuis son cache, mais Cloudflare a du aller
        // jusqu'a l'origine : pour le visiteur, c'est un MISS.
        let c = an(&[("x-hivecache", "hit"), ("cf-cache-status", "MISS")]).unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.source, "cf-cache-status");
        // ... mais c'est bien HiveCache qui a fourni la page, pas le serveur.
        assert_eq!(c.served_by, "HiveCache");
        assert_eq!(c.layers.len(), 2);
    }

    #[test]
    fn tout_en_miss_c_est_l_origine_qui_a_repondu() {
        let c = an(&[("cf-cache-status", "MISS"), ("x-hivecache", "MISS")]).unwrap();
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn revalidated_a_bien_sollicite_l_origine() {
        let c = an(&[("cf-cache-status", "REVALIDATED")]).unwrap();
        assert_eq!(c.status, "REVALIDATED");
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn cache_control_n_est_pas_un_statut() {
        assert!(an(&[("cache-control", "public, max-age=600")]).is_none());
    }

    // --- HiveCache : les trois seules valeurs emises par le plugin ---------
    // (advanced-cache.php : "HIT", "HIT-304", "MISS", + X-HiveCache-Age)

    #[test]
    fn hivecache_hit_avec_son_propre_age() {
        // Sans CDN devant, `Age` n'existe pas : l'age vient du plugin.
        let c = an(&[("X-HiveCache", "HIT"), ("X-HiveCache-Age", "3600")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.engine, "HiveCache");
        assert_eq!(c.served_by, "HiveCache");
        assert_eq!(c.age, Some(3600));
    }

    #[test]
    fn hivecache_hit_304() {
        let c = an(&[("X-HiveCache", "HIT-304")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.raw, "HIT-304");
    }

    #[test]
    fn hivecache_miss() {
        let c = an(&[("X-HiveCache", "MISS")]).unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn hivecache_age_seul_n_est_pas_un_statut() {
        let c = an(&[("X-HiveCache-Age", "120")]).unwrap();
        assert_eq!(c.source, "age");
        assert_eq!(c.status, "HIT");
    }

    #[test]
    fn cloudflare_devant_hivecache_les_deux_couches_remontent() {
        let c = an(&[
            ("cf-cache-status", "MISS"),
            ("X-HiveCache", "HIT"),
            ("X-HiveCache-Age", "88"),
        ])
        .unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.served_by, "HiveCache");
        assert_eq!(c.age, Some(88));
        assert_eq!(c.layers[0].engine, "Cloudflare");
        assert_eq!(c.layers[1].engine, "HiveCache");
    }

    // --- WP Rocket ---------------------------------------------------------

    #[test]
    fn wp_rocket_avec_rocket_nginx() {
        // Le module serveur, lui, pose un vrai en-tete.
        let c = an(&[("x-rocket-nginx-serving-static", "HIT")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.engine, "WP Rocket");
        assert_eq!(c.served_by, "WP Rocket");
    }

    #[test]
    fn wp_rocket_signature_html_avec_entree_ancienne() {
        let vieux = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 7200;
        let html = format!(
            "<html><body>x</body></html>\n<!-- Performance optimized by WP Rocket. \
             Learn more: https://wp-rocket.me - Debug: cached@{vieux} -->"
        );
        let c = analyze(&HeaderMap::new(), &html).unwrap();
        assert_eq!(c.engine, "WP Rocket");
        assert_eq!(c.status, "HIT");
        assert_eq!(c.served_by, "WP Rocket");
    }

    #[test]
    fn wp_rocket_entree_creee_a_l_instant_est_un_miss() {
        // C'est CETTE requete qui vient de peupler le cache.
        let maintenant = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let html = format!("<!-- WP Rocket - Debug: cached@{maintenant} -->");
        let c = analyze(&HeaderMap::new(), &html).unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn wp_rocket_sans_horodatage_n_affirme_rien() {
        let html = "<!-- This website is like a Rocket, isn't it? Performance optimized by WP Rocket. -->";
        let c = analyze(&HeaderMap::new(), html).unwrap();
        assert_eq!(c.engine, "WP Rocket");
        assert_eq!(c.status, "");        // aucune affirmation
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn en_tete_rocket_nginx_prime_sur_la_signature_html() {
        let html = "<!-- Performance optimized by WP Rocket -->";
        let c = analyze(&hm(&[("x-rocket-nginx-serving-static", "HIT")]), html).unwrap();
        // Une seule couche WP Rocket, celle de l'en-tete (plus fiable).
        assert_eq!(c.layers.len(), 1);
        assert_eq!(c.layers[0].header, "x-rocket-nginx-serving-static");
    }

    #[test]
    fn html_sans_wp_rocket_ne_declenche_rien() {
        assert!(analyze(&HeaderMap::new(), "<html><body>rien</body></html>").is_none());
    }

    #[test]
    fn html_accentue_ne_fait_pas_paniquer_la_coupe() {
        // Le HTML depasse la fenetre de queue et regorge d'accents : la
        // troncature doit tomber sur une frontiere de caractere valide.
        let mut html = "é".repeat(5000);
        html.push_str("<!-- WP Rocket -->");
        let c = analyze(&HeaderMap::new(), &html).unwrap();
        assert_eq!(c.engine, "WP Rocket");
    }

    // --- Divers ------------------------------------------------------------

    #[test]
    fn plugin_inconnu_capte_par_la_passe_generique() {
        let c = an(&[("x-wp-super-cache", "HIT")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.source, "x-wp-super-cache");
        assert_eq!(c.engine, "Wp Super");
    }

    #[test]
    fn age_positif_prouve_un_cache() {
        let c = an(&[("age", "120")]).unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.source, "age");
    }

    #[test]
    fn age_zero_ne_prouve_rien() {
        assert!(an(&[("age", "0")]).is_none());
    }

    #[test]
    fn sans_en_tete_pas_de_verdict() {
        // L'absence d'information n'est PAS un MISS.
        assert!(an(&[("content-type", "text/html")]).is_none());
    }

    #[test]
    fn valeur_inconnue_remontee_telle_quelle() {
        let c = an(&[("cf-cache-status", "weird-value")]).unwrap();
        assert_eq!(c.status, "WEIRD-VALUE");
        assert_eq!(c.raw, "weird-value");
    }
}
