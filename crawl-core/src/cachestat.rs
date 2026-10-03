//! Statut de CACHE d'une réponse : qui l'a servie, et depuis son cache ou non.
//!
//! Savoir si une page a été servie depuis un cache (HIT) ou recalculee par
//! l'origine (MISS) dit deux choses à un audit :
//!
//!   - le temps de réponse mesure n'a pas le même sens — un HIT flatte la
//!     vitesse, un MISS montre le vrai cout de l'origine ;
//!   - un site cense être entierement cache qui repond MISS partout revele une
//!     règle de cache qui ne prend pas.
//!
//! Aucun en-tête n'est normalisé entre CDN : chacun a le sien, avec son propre
//! vocabulaire (`HIT`, `TCP_MEM_HIT`, `Miss from cloudfront`...). `analyze` les
//! essaie du plus extérieur au plus interne et en tire trois informations :
//!
//!   - `status`   : le verdict de la couche la plus EXTÉRIEURE (ce qu'a vecu
//!                  le visiteur) ;
//!   - `served_by`: qui a RÉELLEMENT servi la page — le premier cache en HIT,
//!                  ou « origine » si la requête est allée jusqu'au serveur ;
//!   - `layers`   : toutes les couches traversees, pour voir ou ca coince.

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::header::HeaderMap;

use crate::report::{CacheData, CacheLayer};

/// En-têtes de statut de cache connus, du plus EXTÉRIEUR au plus interne, avec
/// le nom lisible du moteur derriere chacun.
///
/// L'ordre compte : une réponse en porte souvent plusieurs — un WordPress
/// derriere Cloudflare en expose deux — et c'est le cache le plus extérieur,
/// celui qui a réellement repondu au visiteur, qui donne le verdict. Un HIT
/// HiveCache sous un MISS Cloudflare reste un MISS pour le visiteur : la
/// requête est bel et bien allée jusqu'au serveur.
///
/// Un moteur vide signifie que l'en-tête est PARTAGE par plusieurs CDN et que
/// le nom doit être devine sur d'autres indices (cf. `guess_cdn`).
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

/// En-têtes portant un AGE de cache en secondes, par ordre de préférence.
///
/// `age` est le standard (RFC 9111) ; les caches d'origine exposent souvent le
/// leur en plus, seule source disponible sur un site sans CDN devant.
const AGE_HEADERS: &[&str] = &["age", "x-hivecache-age", "x-cache-age"];

/// Statuts signifiant que le cache a repondu SANS solliciter l'origine.
///
/// `REVALIDATED` et `EXPIRED` n'en font pas partie : dans les deux cas le cache
/// a contacte le serveur pour verifier sa copie. Pour la question « est-ce
/// qu'on est alle taper le site ? », la réponse y est oui.
///
/// `EDGE` en fait partie pour une raison plus radicale : sur un hebergement de
/// peripherie, il n'y a pas d'origine du tout (cf. `cloudflare_edge`).
const SERVED_FROM_CACHE: &[&str] = &["HIT", "STALE", "UPDATING", "EDGE"];

/// Nom retenu quand aucun cache n'a servi : la page vient du serveur.
const ORIGIN: &str = "origine";

/// Ramene la valeur brute d'un en-tête de cache à un verdict court.
///
/// La valeur est découpée en mots : `TCP_MEM_HIT` (Akamai), `Miss from
/// cloudfront` (CloudFront) et `hit` (Cloudflare) portent tous leur verdict
/// dans un mot isole. Le PREMIER mot reconnu tranche — Fastly repond
/// « HIT, MISS » quand deux couches se repondent, et c'est la plus externe,
/// donc la première, qui a servi le visiteur.
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
            // règle ne la couvre). Ce n'est PAS un échec de cache, c'est
            // l'absence de règle — distinction utile en audit.
            "dynamic" => return Some("DYNAMIC"),
            "none" | "unknown" => return Some("NONE"),
            _ => {}
        }
    }
    None
}

/// Vrai si un nom d'en-tête ressemble à un statut de cache propriétaire.
///
/// Filet pour les caches maison et les plugins qui posent leur propre en-tête
/// (`x-wp-super-cache`...) : on ne peut pas tous les lister, mais leur nom
/// contient « cache » et leur valeur dit hit/miss.
///
/// `cache-control` est exclu explicitement : son nom correspond mais sa valeur
/// est une DIRECTIVE (`max-age=600, public`), jamais un statut. Idem pour les
/// en-têtes de clé, de durée ou d'age.
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

/// Nom lisible deduit d'un en-tête inconnu : `x-wp-super-cache` -> « Wp Super ».
///
/// Approximatif par nature, mais toujours plus parlant que le nom brut, et
/// l'en-tête d'origine reste disponible dans `CacheLayer::header`.
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

/// Devine le CDN derriere un en-tête partage (`x-cache` et consorts).
///
/// `x-cache` est utilise par CloudFront, Fastly, Akamai et Varnish sans qu'on
/// puisse les distinguer sur sa seule valeur. On s'appuie donc sur les en-têtes
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

/// Lit la valeur d'un header (insensible à la casse) en minuscules.
fn header_lower(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_ascii_lowercase())
}

/// Age de l'entrée en cache, en secondes, lu sur le premier en-tête disponible.
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

/// Reconnait un hebergement de PERIPHERIE Cloudflare (Pages, Workers avec
/// assets statiques), qui n'emet aucun statut de cache.
///
/// LE FAIT, vérifié sur le parc : Cloudflare Pages ne pose JAMAIS
/// `cf-cache-status` sur le HTML — ni à la première requête, ni aux suivantes.
/// Un site proxifie classique en porte toujours un, ne serait-ce que `DYNAMIC`.
/// L'ABSENCE de cet en-tête, conjuguée à la présence de `cf-ray` (l'identifiant
/// de requête que Cloudflare ajoute systématiquement), est donc discriminante.
///
/// POURQUOI ce n'est ni un HIT ni un MISS : sur ce type d'hebergement il n'y a
/// pas d'origine a joindre. Le document EST sur le réseau, servi par le noeud le
/// plus proche. La question « le serveur a-t-il travaille ? » n'a pas de MISS
/// possible — d'ou un verdict propre, `EDGE`, plutot qu'un HIT approximatif.
///
/// PRÉCISION DU NOM : les en-têtes ne permettent pas de distinguer Pages d'un
/// Worker servant des assets statiques — leurs réponses sont identiques. On s'en
/// tient donc a « Cloudflare edge », vrai dans les deux cas, plutot que de
/// nommer un produit au hasard.
fn cloudflare_edge(headers: &HeaderMap) -> Option<CacheLayer> {
    if !headers.contains_key("cf-ray") {
        return None;
    }

    // Piste PRECISE, quand elle est disponible : `server-timing`.
    //
    // Cloudflare y decompose le temps de traitement — `cfEdge` (peripherie),
    // `cfOrigin` (serveur d'origine), `cfWorker` (code exécute au bord) :
    //
    //     server-timing: cfEdge;dur=13,cfOrigin;dur=0,cfWorker;dur=58
    //
    // `cfOrigin;dur=0` DIT que l'origine n'a pas été sollicitée — l'équivalent
    // d'un HIT, exprime en temps plutot qu'en statut. Une valeur non nulle dit
    // l'inverse : le serveur a travaille, donc MISS.
    //
    // ATTENTION : cet en-tête n'est emis que si la requête ressemble à une
    // NAVIGATION, ce que Cloudflare juge sur l'en-tête `Accept`. Le crawler en
    // envoie donc un de navigateur (cf. `crawler.rs`) — sans lui, l'information
    // n'arrive jamais.
    if let Some(st) = header_lower(headers, "server-timing") {
        if let Some(origin_ms) = server_timing_dur(&st, "cforigin") {
            let worker = server_timing_dur(&st, "cfworker").unwrap_or(0.0);
            let engine = if worker > 0.0 { "Cloudflare Worker" } else { "Cloudflare edge" };
            return Some(CacheLayer {
                header: "server-timing".to_string(),
                engine: engine.to_string(),
                raw: st.clone(),
                status: if origin_ms > 0.0 {
                    // Le serveur d'origine a bel et bien repondu.
                    "MISS".to_string()
                } else {
                    "EDGE".to_string()
                },
            });
        }
    }

    // à défaut : la seule ABSENCE de statut malgre un `cf-ray`.
    Some(CacheLayer {
        header: "cf-ray".to_string(),
        engine: "Cloudflare edge".to_string(),
        raw: "servi par le réseau, sans origine (aucun cf-cache-status)".to_string(),
        status: "EDGE".to_string(),
    })
}

/// Lit une durée dans un en-tête `server-timing` : `parse("cfEdge;dur=13,...",
/// "cfedge")` renvoie `Some(13.0)`.
///
/// Le nom recherche doit être donne en MINUSCULES (l'en-tête est déjà abaisse
/// par `header_lower`).
fn server_timing_dur(raw: &str, name: &str) -> Option<f64> {
    for entry in raw.split(',') {
        let entry = entry.trim();
        let mut parts = entry.split(';');
        let key = parts.next()?.trim();
        if key != name {
            continue;
        }
        for p in parts {
            let p = p.trim();
            if let Some(v) = p.strip_prefix("dur=") {
                return v.trim().parse::<f64>().ok();
            }
        }
    }
    None
}

/// Les `n` derniers octets d'une chaîne, coupes sur une frontière de caractère.
///
/// `len()` compte des OCTETS : trancher à l'aveugle ferait paniquer des qu'un
/// caractère multi-octets chevauche la borne. On recule donc jusqu'à la
/// dernière frontière valide (3 tours au pire).
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

/// Détecté WP Rocket dans le HTML, faute d'en-tête HTTP.
///
/// POURQUOI c'est nécessaire : WP Rocket seul (mode PHP) n'emet AUCUN en-tête
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
/// fichier de cache. Le comparer à l'heure courante donne l'age de l'entrée —
/// une page servie depuis le cache à un age de plusieurs secondes au moins,
/// alors qu'une page qui vient d'être générée (et qui PEUPLE le cache, en
/// portant déjà le commentaire) à un age quasi nul.
///
/// LIMITE ASSUMÉE : la comparaison se fait sur l'horloge LOCALE, pas sur celle
/// du serveur. Un decalage de quelques secondes est sans effet grace au seuil ;
/// une machine franchement deregle rendrait le verdict inexploitable, et l'age
/// aberrant est alors écarte.
fn wp_rocket_from_html(html: &str) -> Option<CacheLayer> {
    // La signature est en FIN de document : inutile de scanner tout le HTML.
    let end = tail(html, 4096).to_ascii_lowercase();
    if !end.contains("wp rocket") && !end.contains("wp-rocket") {
        return None;
    }

    // Age de l'entrée, si le commentaire de debogage est présent.
    let age = end.find("cached@").and_then(|pos| {
        let rest = &end[pos + "cached@".len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let cached_at = digits.parse::<i64>().ok()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        let delta = now - cached_at;
        // Age négatif (horloge locale en retard) ou supérieur à un an :
        // incoherent, on n'en tire rien.
        if (0..31_536_000).contains(&delta) {
            Some(delta as u64)
        } else {
            None
        }
    });

    let status = match age {
        // Entrée déjà ancienne : la page a bien été servie depuis le cache.
        Some(a) if a > 10 => "HIT",
        // Entrée créée à l'instant : c'est CETTE requête qui a peuple le cache.
        Some(_) => "MISS",
        // Signature présente mais pas de `cached@` : on sait que WP Rocket est
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

/// Determine le statut de cache d'une réponse à partir de ses en-têtes et,
/// en dernier recours, de son HTML.
///
/// Stratégie en quatre passes, de la plus sure à la plus generale :
///   1. les en-têtes connus, dans l'ordre du plus extérieur au plus interne ;
///   2. tout en-tête dont le nom evoque un cache et dont la valeur dit hit/miss
///      (caches maison, plugins WordPress) ;
///   3. l'hebergement de peripherie Cloudflare (Pages, Workers), reconnaissable
///      à l'ABSENCE de statut de cache malgre un `cf-ray` ;
///   4. la signature HTML de WP Rocket, qui n'emet pas d'en-tête sans son
///      module serveur ;
///   5. `Age` (RFC 9111) : un age > 0 PROUVE qu'un intermediaire a stocke puis
///      servi la réponse, même sans en-tête propriétaire. Un age de 0 ne prouve
///      rien — une entrée fraichement stockee vaut 0 elle aussi.
///
/// `None` quand rien de tout cela n'aboutit : l'absence d'information n'est pas
/// un MISS, et l'afficher comme tel serait faux.
pub fn analyze(headers: &HeaderMap, html: &str) -> Option<CacheData> {
    let age = read_age(headers);
    let mut layers: Vec<CacheLayer> = Vec::new();

    // --- 1. En-têtes connus, dans l'ordre ---------------------------------
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
            // Moteur vide dans la table = en-tête partage entre plusieurs CDN.
            engine: if engine.is_empty() { guess_cdn(headers) } else { (*engine).to_string() },
            raw: raw.to_string(),
            // Valeur non reconnue : on la remonte telle quelle plutot que de
            // trancher a tort. L'interface affiche ce qu'a dit le serveur.
            status: normalize(raw)
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw.to_uppercase()),
        });
    }

    // --- 2. Passe generique sur les en-têtes « *cache* » ------------------
    for (name, value) in headers.iter() {
        let n = name.as_str().to_ascii_lowercase();
        if !looks_like_cache_header(&n) || CACHE_HEADERS.iter().any(|(h, _)| *h == n) {
            continue; // déjà traite à la passe 1
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

    // --- 3. Hebergement de peripherie Cloudflare (Pages / Workers) ---------
    //
    // UNIQUEMENT si aucun en-tête de statut n'a parle : c'est précisément leur
    // absence qui signe ce mode d'hebergement. Des qu'un `cf-cache-status`
    // existe — même `DYNAMIC` —, le site est proxifie et c'est lui qui fait foi.
    //
    // Insere EN TÊTE : le réseau de peripherie est la couche la plus extérieure
    // de toutes, celle qui a repondu au visiteur.
    if layers.is_empty() {
        if let Some(l) = cloudflare_edge(headers) {
            layers.push(l);
        }
    }

    // --- 4. WP Rocket sans en-tête (signature HTML) ------------------------
    // Ajoute en DERNIER : c'est un cache d'origine, le plus interne de tous.
    if !layers.iter().any(|l| l.engine == "WP Rocket") {
        if let Some(l) = wp_rocket_from_html(html) {
            layers.push(l);
        }
    }

    if !layers.is_empty() {
        // Verdict = couche la plus extérieure (la première collectee).
        let first = &layers[0];
        // Qui a RÉELLEMENT servi : le premier cache a avoir repondu sans
        // solliciter l'origine. Aucun -> la requête est allée au serveur.
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
        // jusqu'à l'origine : pour le visiteur, c'est un MISS.
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
        // Le module serveur, lui, pose un vrai en-tête.
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
        // C'est CETTE requête qui vient de peupler le cache.
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
        // Une seule couche WP Rocket, celle de l'en-tête (plus fiable).
        assert_eq!(c.layers.len(), 1);
        assert_eq!(c.layers[0].header, "x-rocket-nginx-serving-static");
    }

    #[test]
    fn html_sans_wp_rocket_ne_declenche_rien() {
        assert!(analyze(&HeaderMap::new(), "<html><body>rien</body></html>").is_none());
    }

    #[test]
    fn html_accentue_ne_fait_pas_paniquer_la_coupe() {
        // Le HTML depasse la fenêtre de queue et regorge d'accents : la
        // troncature doit tomber sur une frontière de caractère valide.
        let mut html = "é".repeat(5000);
        html.push_str("<!-- WP Rocket -->");
        let c = analyze(&HeaderMap::new(), &html).unwrap();
        assert_eq!(c.engine, "WP Rocket");
    }

    // --- Hebergement de peripherie Cloudflare ------------------------------
    // En-têtes releves le 24/08/2026 sur amiante.guide et web-pulse.app.

    #[test]
    fn cloudflare_pages_reconnu_malgre_l_absence_de_statut() {
        let c = an(&[
            ("cf-ray", "a30423943f3300d0-CDG"),
            ("cache-control", "public, max-age=0, must-revalidate"),
            ("access-control-allow-origin", "*"),
            ("server", "cloudflare"),
        ])
        .unwrap();
        assert_eq!(c.status, "EDGE");
        assert_eq!(c.engine, "Cloudflare edge");
        // Il n'y a pas d'origine a joindre : le document EST sur le réseau.
        assert_eq!(c.served_by, "Cloudflare edge");
    }

    #[test]
    fn un_cf_cache_status_meme_dynamic_prime_sur_la_signature_edge() {
        // Site proxifie (Worker SSR) : l'en-tête existe, il fait foi, et la
        // détection de peripherie ne doit PAS se déclencher.
        let c = an(&[("cf-ray", "a30423d6ba98acc1-CDG"), ("cf-cache-status", "DYNAMIC")]).unwrap();
        assert_eq!(c.status, "DYNAMIC");
        assert_eq!(c.engine, "Cloudflare");
        assert_eq!(c.served_by, ORIGIN);
        assert_eq!(c.layers.len(), 1);
    }

    // --- server-timing : la decomposition edge / origine / worker ----------
    // Valeurs relevees le 24/08/2026 sur www.fuiterecherche.com.

    #[test]
    fn server_timing_origine_a_zero_vaut_edge() {
        let c = an(&[
            ("cf-ray", "a3046d370c2fbb4e-CDG"),
            ("server-timing", "cfEdge;dur=13,cfOrigin;dur=0,cfWorker;dur=58"),
        ])
        .unwrap();
        assert_eq!(c.status, "EDGE");
        // Un cfWorker non nul : du code a tourne au bord, pas sur une origine.
        assert_eq!(c.engine, "Cloudflare Worker");
        assert_eq!(c.served_by, "Cloudflare Worker");
    }

    #[test]
    fn server_timing_origine_non_nulle_vaut_miss() {
        // Le serveur d'origine a bel et bien travaille.
        let c = an(&[
            ("cf-ray", "abc-CDG"),
            ("server-timing", "cfEdge;dur=4,cfOrigin;dur=213,cfWorker;dur=0"),
        ])
        .unwrap();
        assert_eq!(c.status, "MISS");
        assert_eq!(c.engine, "Cloudflare edge");
        assert_eq!(c.served_by, ORIGIN);
    }

    #[test]
    fn server_timing_sans_worker_reste_edge() {
        let c = an(&[("cf-ray", "abc"), ("server-timing", "cfEdge;dur=9,cfOrigin;dur=0")]).unwrap();
        assert_eq!(c.status, "EDGE");
        assert_eq!(c.engine, "Cloudflare edge");
    }

    #[test]
    fn server_timing_d_un_autre_editeur_est_ignore() {
        // Beaucoup d'applications emettent leur propre `server-timing` : sans
        // clé `cfOrigin`, on retombe sur la détection par absence de statut.
        let c = an(&[("cf-ray", "abc"), ("server-timing", "db;dur=53, app;dur=47.2")]).unwrap();
        assert_eq!(c.status, "EDGE");
        assert_eq!(c.source, "cf-ray");
    }

    #[test]
    fn un_cf_cache_status_prime_sur_le_server_timing() {
        // Site proxifie : le statut explicite fait foi, on ne deduit rien.
        let c = an(&[
            ("cf-ray", "abc"),
            ("cf-cache-status", "HIT"),
            ("server-timing", "cfEdge;dur=1,cfOrigin;dur=99"),
        ])
        .unwrap();
        assert_eq!(c.status, "HIT");
        assert_eq!(c.source, "cf-cache-status");
    }

    #[test]
    fn lecture_d_une_duree_server_timing() {
        let raw = "cfedge;dur=13,cforigin;dur=0,cfworker;dur=58.5";
        assert_eq!(server_timing_dur(raw, "cfedge"), Some(13.0));
        assert_eq!(server_timing_dur(raw, "cforigin"), Some(0.0));
        assert_eq!(server_timing_dur(raw, "cfworker"), Some(58.5));
        assert_eq!(server_timing_dur(raw, "absent"), None);
    }

    #[test]
    fn sans_cf_ray_pas_de_verdict_de_peripherie() {
        // `server: cloudflare` seul ne suffit pas : il apparait aussi sur des
        // réponses ou Cloudflare n'a fait que relayer.
        assert!(an(&[("server", "cloudflare")]).is_none());
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
