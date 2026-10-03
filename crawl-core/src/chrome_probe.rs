//! Sonde NAVIGATEUR via chromiumoxide 0.7 (Chrome DevTools Protocol, domaine Network).
//!
//! C'est la mesure de référence : `encoded_data_length` du CDP donne la bande
//! passante RÉELLE transferee sur le fil (compression incluse, telle que vue par
//! le navigateur), contrairement à une simple lecture HTTP de l'asset.
//!
//! Stratégie :
//!   1. Ouvrir une page vierge.
//!   2. Activer + ECOUTER le domaine Network AVANT de naviguer (sinon on rate les
//!      premieres réponses).
//!   3. Deux flux d'événements alimentent un état partage `Arc<Mutex<NetState>>` :
//!        - `EventResponseReceived` : type de ressource + headers (x-vercel-cache) + URL.
//!        - `EventLoadingFinished`  : `encoded_data_length` (octets réels) -> agregation.
//!   4. Naviguer, attendre la fin de navigation + une tempo d'inactivite réseau.
//!   5. Recuperer le HTML final pour la detection de techno.
//!   6. Construire le `PageReport`.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context as _;
use futures::StreamExt;
use tokio::sync::Mutex;

use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams as NetworkEnableParams, EventLoadingFinished, EventResponseReceived, ResourceType,
};

use crate::report::{AssetReport, PageReport, TypeStat};

/// État partage entre les deux tâches d'ecoute et le thread principal.
#[derive(Default)]
struct NetState {
    /// Infos memorisees a `ResponseReceived`, indexees par `requestId` (chaîne CDP).
    responses: HashMap<String, ResponseMeta>,
    /// Agregation des octets réels par type de ressource (clé = libellé `kind`).
    by_type: HashMap<String, TypeStat>,
    /// Assets de type Image, finalises (un par `LoadingFinished` correspondant).
    images: Vec<AssetReport>,
    /// Somme de tous les `encoded_data_length`.
    total_bytes: u64,
    /// Nombre total de requêtes finalisees.
    request_count: u32,
    /// Requêtes dynamiques (cache MISS + /api + RSC) — invocations serverless.
    dynamic_requests: u32,
    /// Cache Vercel : HIT.
    vercel_cache_hits: u32,
    /// Cache Vercel : MISS / STALE / PRERENDER.
    vercel_cache_misses: u32,
}

/// Données retenues depuis `EventResponseReceived`, consommees a `LoadingFinished`.
struct ResponseMeta {
    url: String,
    kind: String,
    /// Valeur du header `x-vercel-cache` (déjà en minuscules), si présente.
    vercel_cache: Option<String>,
    /// Tous les headers de réponse (objet JSON brut du CDP). Conserve pour le
    /// document principal afin d'alimenter `techno::detect` (server, x-vercel-id,
    /// x-powered-by, cf-ray, x-nf-request-id...).
    headers_json: serde_json::Value,
    /// Statut HTTP du document principal (sert a `PageReport.status`).
    status: u16,
    /// Type CDP brut (pour distinguer le Document principal).
    resource_type: ResourceType,
    /// `true` si déjà comptabilise par un `LoadingFinished` (anti double-comptage).
    finalized: bool,
}

/// Traduit le `ResourceType` du CDP vers le libellé `kind` du contrat report.rs.
/// Domaine de valeurs cible : "Image" | "Script" | "Stylesheet" | "Font" |
/// "Document" | "Fetch" | "Other".
fn resource_kind(rt: &ResourceType) -> String {
    match rt {
        ResourceType::Document => "Document",
        ResourceType::Stylesheet => "Stylesheet",
        ResourceType::Image => "Image",
        ResourceType::Font => "Font",
        ResourceType::Script => "Script",
        // XHR et Fetch sont regroupes sous "Fetch" (requêtes de données).
        ResourceType::Xhr | ResourceType::Fetch => "Fetch",
        // Tout le reste (Media, Manifest, Ping, WebSocket, Other, etc.).
        _ => "Other",
    }
    .to_string()
}

/// Extrait la valeur d'un header (insensible à la casse) depuis l'objet JSON
/// `Headers` du CDP. En chromiumoxide 0.7, `network::Headers` enveloppe un
/// `serde_json::Value` (objet) ; on passe par `serde_json::to_value` pour rester
/// robuste aux variations exactes d'API (deref / inner()).
fn header_value(headers_json: &serde_json::Value, name: &str) -> Option<String> {
    let obj = headers_json.as_object()?;
    let want = name.to_ascii_lowercase();
    for (k, v) in obj {
        if k.to_ascii_lowercase() == want {
            // Les valeurs de header peuvent être des chaînes ; on gere aussi le
            // cas (rare) d'une valeur déjà typee différemment.
            return match v {
                serde_json::Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            };
        }
    }
    None
}

/// Convertit l'objet JSON `Headers` du CDP en `reqwest::header::HeaderMap`
/// (pour `techno::detect`). Les entrées invalides (nom/valeur non ASCII-token)
/// sont ignorées silencieusement.
fn headers_json_to_map(headers_json: &serde_json::Value) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    let mut map = HeaderMap::new();
    if let Some(obj) = headers_json.as_object() {
        for (k, v) in obj {
            let val_str = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(&val_str),
            ) {
                map.insert(name, value);
            }
        }
    }
    map
}

pub async fn measure_page(
    browser: &chromiumoxide::Browser,
    target: &str,
    profile: Option<crate::useragents::UaProfile>,
) -> anyhow::Result<PageReport> {
    // 1. Nouvelle page vierge.
    let page = browser
        .new_page("about:blank")
        .await
        .context("ouverture d'une nouvelle page Chrome")?;

    // Emulation du profil (user-agent + viewport) si demande.
    if let Some(p) = profile {
        use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
        use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
        let _ = page
            .execute(SetUserAgentOverrideParams::new(p.ua.to_string()))
            .await;
        let _ = page
            .execute(SetDeviceMetricsOverrideParams::new(
                p.width as i64,
                p.height as i64,
                1.0,
                p.mobile,
            ))
            .await;
    }

    // Session vierge : on efface cookies + cache avant chaque page (le profil
    // Chrome est déjà temporaire et isole de celui de l'utilisateur).
    {
        use chromiumoxide::cdp::browser_protocol::network::{
            ClearBrowserCacheParams, ClearBrowserCookiesParams,
        };
        let _ = page.execute(ClearBrowserCookiesParams::default()).await;
        let _ = page.execute(ClearBrowserCacheParams::default()).await;
    }

    let state: Arc<Mutex<NetState>> = Arc::new(Mutex::new(NetState::default()));

    // 2. ECOUTER le domaine Network AVANT de naviguer.
    //    On enregistre les listeners d'événements puis on active le domaine.
    let mut response_events = page
        .event_listener::<EventResponseReceived>()
        .await
        .context("abonnement EventResponseReceived")?;
    let mut finished_events = page
        .event_listener::<EventLoadingFinished>()
        .await
        .context("abonnement EventLoadingFinished")?;

    // Active la capture réseau (equivaut a Network.enable du CDP).
    page.execute(NetworkEnableParams::default())
        .await
        .context("Network.enable")?;

    // --- Tâche : ResponseReceived -> memorise type/headers/url par request_id. ---
    let state_resp = Arc::clone(&state);
    let resp_task = tokio::spawn(async move {
        while let Some(ev) = response_events.next().await {
            // `ev` est un Arc<EventResponseReceived>.
            let request_id = ev.request_id.inner().to_string();
            let resource_type = ev.r#type.clone();
            let kind = resource_kind(&resource_type);

            let response = &ev.response;
            let url = response.url.clone();
            // Le statut CDP est numérique (i64) ; on le ramene a u16.
            let status = response.status as u16;

            // Headers de réponse -> JSON pour lecture insensible à la casse.
            // `response.headers` est de type network::Headers (newtype JSON).
            let headers_json = serde_json::to_value(&response.headers).unwrap_or(serde_json::Value::Null);
            let vercel_cache =
                header_value(&headers_json, "x-vercel-cache").map(|v| v.to_ascii_lowercase());

            let mut st = state_resp.lock().await;
            st.responses.insert(
                request_id,
                ResponseMeta {
                    url,
                    kind,
                    vercel_cache,
                    headers_json,
                    status,
                    resource_type,
                    finalized: false,
                },
            );
        }
    });

    // --- Tâche : LoadingFinished -> octets réels + agregation par type. ---
    let state_fin = Arc::clone(&state);
    let fin_task = tokio::spawn(async move {
        while let Some(ev) = finished_events.next().await {
            // `ev` est un Arc<EventLoadingFinished>.
            let request_id = ev.request_id.inner().to_string();
            // `encoded_data_length` est un f64 (octets réels sur le fil).
            let bytes = ev.encoded_data_length.max(0.0) as u64;

            let mut st = state_fin.lock().await;

            // On a besoin des metas de la réponse correspondante.
            let (kind, url, vercel_cache) = {
                match st.responses.get_mut(&request_id) {
                    Some(meta) => {
                        if meta.finalized {
                            // Déjà comptabilise (ex: redirection chainee) — on saute.
                            continue;
                        }
                        meta.finalized = true;
                        (meta.kind.clone(), meta.url.clone(), meta.vercel_cache.clone())
                    }
                    None => {
                        // LoadingFinished sans ResponseReceived connu (preflight, data:,
                        // cache disque...). On compte quand même les octets dans "Other".
                        ("Other".to_string(), String::new(), None)
                    }
                }
            };

            // Totaux + agregation par type.
            st.total_bytes = st.total_bytes.saturating_add(bytes);
            st.request_count = st.request_count.saturating_add(1);
            let entry = st.by_type.entry(kind.clone()).or_default();
            entry.bytes = entry.bytes.saturating_add(bytes);
            entry.count = entry.count.saturating_add(1);

            // Assets Image.
            if kind == "Image" {
                st.images.push(AssetReport {
                    url: url.clone(),
                    bytes,
                    kind: "Image".to_string(),
                });
            }

            // Requêtes dynamiques (cache MISS + /api + RSC).
            if crate::techno::is_dynamic_request(&url, vercel_cache.as_deref()) {
                st.dynamic_requests = st.dynamic_requests.saturating_add(1);
            }

            // Compteurs cache Vercel.
            if let Some(vc) = vercel_cache.as_deref() {
                match vc {
                    "hit" => st.vercel_cache_hits = st.vercel_cache_hits.saturating_add(1),
                    "miss" | "stale" | "prerender" => {
                        st.vercel_cache_misses = st.vercel_cache_misses.saturating_add(1)
                    }
                    _ => {}
                }
            }
        }
    });

    // 3. Navigation + attente de fin de chargement + tempo d'inactivite réseau.
    //    On capture l'erreur eventuelle sans interrompre la collecte déjà faite.
    let nav_error: Option<String> = match page.goto(target).await {
        Ok(_) => {
            // Attendre l'événement de navigation (best-effort).
            let _ = page.wait_for_navigation().await;
            // Petite tempo pour laisser les dernières requêtes (images lazy, RSC,
            // XHR de fin de chargement) emettre leur LoadingFinished.
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            None
        }
        Err(e) => Some(format!("navigation echouee: {e}")),
    };

    // 4. HTML final + headers du document principal -> detection techno.
    //    Le HTML est recupere via le DOM (page.content), ce qui inclut le rendu JS.
    let html = page.content().await.unwrap_or_default();

    // 5. Construction du PageReport à partir de l'état partage.
    //    On prend un snapshot sous lock, puis on relache avant de fermer la page.
    let report = {
        let st = state.lock().await;

        // Statut + headers du document principal (première réponse de type Document).
        // On cherche la réponse Document dont l'URL correspond le mieux à la cible,
        // à défaut la première réponse Document rencontree.
        let mut status: u16 = 0;
        let mut doc_headers = reqwest::header::HeaderMap::new();
        let mut doc_meta: Option<&ResponseMeta> = None;
        for meta in st.responses.values() {
            if matches!(meta.resource_type, ResourceType::Document) {
                // Priorite à l'URL qui matche exactement la cible (après redirections,
                // l'URL finale peut differer ; on garde la première Document trouvee
                // et on remplace si on trouve un match exact de la cible demandee).
                match doc_meta {
                    None => doc_meta = Some(meta),
                    Some(_) if meta.url == target => doc_meta = Some(meta),
                    _ => {}
                }
            }
        }
        if let Some(meta) = doc_meta {
            status = meta.status;
            // Reconstruit le HeaderMap COMPLET du document pour que techno::detect
            // dispose de tous les signaux (server, x-vercel-id, x-powered-by,
            // cf-ray, x-nf-request-id, x-vercel-cache...).
            doc_headers = headers_json_to_map(&meta.headers_json);
        }

        // Detection techno : combine headers du document + HTML rendu.
        let tech = crate::techno::detect(&doc_headers, &html);

        PageReport {
            url: target.to_string(),
            status,
            tech,
            total_bytes: st.total_bytes,
            by_type: st.by_type.clone(),
            request_count: st.request_count,
            dynamic_requests: st.dynamic_requests,
            vercel_cache_hits: st.vercel_cache_hits,
            vercel_cache_misses: st.vercel_cache_misses,
            images: st.images.clone(),
            measured_with: "chrome".to_string(),
            error: nav_error,
            ..Default::default()
        }
    };

    // 6. Fermeture de la page + arrêt des tâches d'ecoute.
    //    Fermer la page termine les streams d'événements -> les boucles `while`
    //    se finissent, on annule par sécurité.
    let _ = page.close().await;
    resp_task.abort();
    fin_task.abort();

    Ok(report)
}
