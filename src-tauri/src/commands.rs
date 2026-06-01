//! Commandes Tauri v2 exposees au frontend.
//!
//! Contrat d'evenements (doivent correspondre aux `listen(...)` du frontend) :
//!   - `crawl:progress` : payload `crawl_core::CrawlProgress` (avancement)
//!   - `crawl:page`     : payload `crawl_core::PageReport` (une page mesuree)
//!   - `crawl:done`     : payload `crawl_core::CrawlSummary` (resume final)
//!   - `crawl:error`    : payload `String` (message d'erreur global)
//!
//! Commandes (doivent correspondre aux `invoke("...")` du frontend) :
//!   - `start_crawl(config)` : DEMARRE un nouveau crawl, ou REPREND un crawl mis
//!                             en pause (meme `seed_url` + file non vide).
//!   - `stop_crawl()`        : met le crawl en PAUSE (l'etat est conserve, on peut
//!                             reprendre via un nouvel appel a `start_crawl`).
//!   - `clear_crawl()`       : VIDE l'etat de crawl (apres ca, `start_crawl` repart
//!                             de zero). Le bouton "Vider" de l'UI peut l'appeler.
//!
//! ## Pause / reprise (Vague 4)
//!
//! L'etat du crawl ([`crawl_core::crawler::SharedState`]) est stocke dans l'etat
//! applicatif Tauri ([`CrawlState`]) afin de SURVIVRE entre un `stop` (pause) et
//! un `start` (reprise). `stop_crawl` ne fait que lever un drapeau : les workers
//! finissent leur page courante puis sortent sans vider la file. Un `start_crawl`
//! ulterieur, si un etat reprenable existe pour la meme seed, RELANCE les workers
//! sur la file existante (reprise) ; sinon il initialise un etat neuf.
//!
//! Ceci corrige aussi le bug du bouton "Demarrer" inerte apres un "Stop" : comme
//! les workers s'arretent vraiment, `crawl:done` est emis rapidement et l'UI sort
//! de l'etat "en cours" ; un nouvel appel a `start_crawl` repart (ou reprend).

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crawl_core::crawler::SharedState;
use crawl_core::report::{CrawlConfig, CrawlProgress, CrawlSummary, PageReport};

/// Etat partage gere par Tauri (via `.manage`). Porte l'etat du crawl COURANT
/// (file + visited + agregats + navigateur), conserve entre pause et reprise.
#[derive(Default)]
pub struct CrawlState {
    /// Etat du crawl courant, ou `None` si aucun crawl n'a (encore) ete lance /
    /// si l'etat a ete vide via `clear_crawl`. Mutable derriere un `Mutex` car
    /// l'objet manage par Tauri est, lui, immuable.
    pub inner: Mutex<Option<Arc<SharedState>>>,
}

/// Demarre un nouveau crawl, ou REPREND un crawl mis en pause.
///
/// Heuristique de decision :
///   - s'il existe un etat reprenable (`SharedState` deja initialise, file non
///     vide) pour la MEME `seed_url` -> on REPREND sur cet etat ;
///   - sinon -> on cree un etat NEUF a partir de la seed et on l'enregistre.
///
/// Dans les deux cas, on spawn une task qui appelle `crawler::run_shared` avec
/// deux closures relayant l'avancement (`crawl:progress`) et chaque page
/// (`crawl:page`) vers le frontend, puis emet `crawl:done` avec le resume.
#[tauri::command]
pub async fn start_crawl(
    app: AppHandle,
    state: State<'_, CrawlState>,
    config: CrawlConfig,
) -> Result<(), String> {
    // --- Choix : reprise d'un etat existant, ou nouvel etat. ---
    let is_resume: bool;
    let shared: Arc<SharedState> = {
        let mut slot = state.inner.lock().await;

        // Reprise possible ? Meme seed + etat encore reprenable (file non vide).
        let resume = match slot.as_ref() {
            Some(existing) => {
                existing.seed_url == config.seed_url && existing.is_resumable().await
            }
            None => false,
        };
        is_resume = resume;

        if resume {
            // REPREND : on reutilise l'etat partage existant tel quel.
            Arc::clone(slot.as_ref().unwrap())
        } else {
            // NOUVEAU : on cree un etat neuf amorce avec la seed et on l'enregistre.
            let fresh = Arc::new(SharedState::new(&config.seed_url));
            *slot = Some(Arc::clone(&fresh));
            fresh
        }
    };

    // Sitemap : fetch au DEBUT (comme robots.txt), uniquement pour un NOUVEAU crawl.
    let seed = config.seed_url.clone();
    let do_sitemap = !is_resume;
    let app_sitemap = app.clone();

    // Clones de l'AppHandle (cheap, ref-compte) pour les emissions.
    let app_progress = app.clone();
    let app_page = app.clone();
    let app_done = app.clone();

    tauri::async_runtime::spawn(async move {
        // Sitemap recupere AU DEBUT (robots.txt + /sitemap.xml) et emis tout de
        // suite, pour que l'onglet Sitemap se remplisse sans attendre la fin.
        let sitemap_urls: Vec<String> = if do_sitemap {
            let urls = crawl_core::sitemap::fetch_default(&seed).await;
            let _ = app_sitemap.emit("crawl:sitemap", urls.clone());
            urls
        } else {
            Vec::new()
        };

        // Closure progression -> evenement "crawl:progress".
        let on_progress = move |progress: CrawlProgress| {
            let _ = app_progress.emit("crawl:progress", progress);
        };

        // Closure page -> evenement "crawl:page".
        let on_page = move |page: PageReport| {
            let _ = app_page.emit("crawl:page", page);
        };

        // Lancement (ou reprise) du crawl sur l'etat partage.
        match crawl_core::crawler::run_shared(shared, config, on_progress, on_page).await {
            Ok(mut summary) => {
                // On emet toujours le resume (fin naturelle OU pause) afin que l'UI
                // sorte de l'etat "en cours". En cas de pause, le resume est partiel
                // et l'etat reste reprenable via un nouvel appel a start_crawl.
                if !sitemap_urls.is_empty() {
                    summary.sitemap_urls = sitemap_urls;
                }
                let _ = app_done.emit("crawl:done", summary);
            }
            Err(e) => {
                // Echec global : on remonte un resume vide marque comme termine,
                // et on emet aussi un evenement d'erreur dedie.
                let empty = CrawlSummary {
                    pages: Vec::new(),
                    total_bytes: 0,
                    total_requests: 0,
                    total_dynamic_requests: 0,
                    tech_detected: Vec::new(),
                    sitemap_urls: Vec::new(),
                };
                let _ = app_done.emit("crawl:error", e.to_string());
                let _ = app_done.emit("crawl:done", empty);
            }
        }
    });

    Ok(())
}

/// Met le crawl en cours en PAUSE : leve le drapeau d'arret de l'etat partage.
/// Les workers finissent leur page courante puis sortent SANS vider l'etat ;
/// un `start_crawl` ulterieur (meme seed) reprendra la ou on s'etait arrete.
#[tauri::command]
pub async fn stop_crawl(state: State<'_, CrawlState>) -> Result<(), String> {
    if let Some(shared) = state.inner.lock().await.as_ref() {
        shared.request_stop();
    }
    Ok(())
}

/// VIDE l'etat de crawl : leve le drapeau d'arret du crawl courant (s'il existe)
/// puis oublie l'etat partage. Apres cet appel, `start_crawl` repart de zero.
/// (Le bouton "Vider" de l'UI peut appeler cette commande ; l'UI actuelle se
/// contente d'appeler `stop_crawl` + reset local, ce qui reste valable.)
#[tauri::command]
pub async fn clear_crawl(state: State<'_, CrawlState>) -> Result<(), String> {
    let mut slot = state.inner.lock().await;
    if let Some(shared) = slot.as_ref() {
        shared.request_stop();
    }
    *slot = None;
    Ok(())
}

/// Renvoie l'IP publique sortante + le reverse DNS (affiche dans l'en-tete).
#[tauri::command]
pub async fn network_info() -> Result<crawl_core::netinfo::NetworkInfo, String> {
    crawl_core::netinfo::fetch().await.map_err(|e| e.to_string())
}

/// Chemin du fichier d'historique, dans le dossier de donnees de l'application.
fn history_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("history.json"))
}

/// Charge l'historique (blob JSON) depuis le disque. Renvoie "[]" si absent.
#[tauri::command]
pub fn load_history(app: AppHandle) -> Result<String, String> {
    let path = history_path(&app)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(_) => Ok("[]".to_string()),
    }
}

/// Enregistre l'historique complet (blob JSON) sur le disque (dossier de l'app).
#[tauri::command]
pub fn save_history(app: AppHandle, json: String) -> Result<(), String> {
    let path = history_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Renvoie le chemin complet (texte) du fichier d'historique, pour affichage.
#[tauri::command]
pub fn history_location(app: AppHandle) -> Result<String, String> {
    Ok(history_path(&app)?.to_string_lossy().to_string())
}

/// Ouvre, dans l'explorateur de fichiers, le dossier contenant l'historique.
#[tauri::command]
pub fn open_history_folder(app: AppHandle) -> Result<(), String> {
    let path = history_path(&app)?;
    let dir = path.parent().ok_or_else(|| "dossier introuvable".to_string())?;
    // Cree le dossier si besoin (sinon l'explorateur ouvrirait dans le vide).
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        // .spawn() : explorer.exe renvoie souvent un code != 0 meme en succes.
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}


/// Dossier des crawls enregistres (sous le dossier de donnees de l'application).
fn crawls_dir(app: &AppHandle, custom: &Option<String>) -> Result<std::path::PathBuf, String> {
    if let Some(d) = custom {
        if !d.trim().is_empty() {
            return Ok(std::path::PathBuf::from(d.trim()));
        }
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("crawls"))
}

/// Transforme un identifiant (URL de depart) en nom de fichier sur (sans separateurs).
fn sanitize_id(id: &str) -> String {
    let mut s: String = id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' { ch } else { '_' })
        .collect();
    if s.len() > 120 {
        s.truncate(120);
    }
    if s.is_empty() {
        s.push_str("crawl");
    }
    s
}

/// Enregistre un crawl complet (JSON) sous `crawls/<id>.json`. Renvoie le chemin du fichier.
#[tauri::command]
pub fn save_crawl(app: AppHandle, id: String, json: String, dir: Option<String>) -> Result<String, String> {
    let d = crawls_dir(&app, &dir)?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(format!("{}.json", sanitize_id(&id)));
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Charge un crawl enregistre par son identifiant (URL de depart). Renvoie "" si absent.
#[tauri::command]
pub fn load_crawl(app: AppHandle, id: String, dir: Option<String>) -> Result<String, String> {
    let d = crawls_dir(&app, &dir)?;
    let path = d.join(format!("{}.json", sanitize_id(&id)));
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(_) => Ok(String::new()),
    }
}

/// Ouvre, dans l'explorateur, le dossier des crawls enregistres (custom ou defaut).
#[tauri::command]
pub fn open_crawls_folder(app: AppHandle, dir: Option<String>) -> Result<(), String> {
    let d = crawls_dir(&app, &dir)?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(&d).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(&d).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}
