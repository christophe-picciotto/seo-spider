//! Commandes Tauri v2 exposees au frontend.
//!
//! Contrat d'evenements (doivent correspondre aux `listen(...)` du frontend) :
//!   - `crawl:progress` : payload `crawl_core::CrawlProgress` (avancement)
//!   - `crawl:page`     : payload `crawl_core::PageReport` (une page mesuree)
//!   - `crawl:done`     : payload `crawl_core::CrawlSummary` (resume final)
//!
//! Commandes (doivent correspondre aux `invoke("...")` du frontend) :
//!   - `start_crawl(config)` : demarre un crawl en tache de fond.
//!   - `stop_crawl()`        : demande l'arret (les emissions se taisent).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use crawl_core::report::{CrawlConfig, CrawlProgress, CrawlSummary, PageReport};

/// Etat partage gere par Tauri (via `.manage`). Porte le drapeau d'arret du crawl.
#[derive(Default)]
pub struct CrawlState {
    /// `true` => arret demande : les closures cessent d'emettre vers le frontend.
    pub stop: Arc<AtomicBool>,
}

/// Demarre un crawl en arriere-plan.
///
/// - Remet le drapeau d'arret a `false`.
/// - Spawn une task qui appelle `crawl_core::crawler::run` avec deux closures qui
///   relaient l'avancement (`crawl:progress`) et chaque page (`crawl:page`) vers
///   le frontend via `AppHandle::emit`.
/// - A la fin, emet `crawl:done` avec le resume.
///
/// Les closures consultent le drapeau d'arret (clone de l'`Arc`) : si l'arret est
/// demande, elles n'emettent plus rien (le crawler n'expose pas de signal d'arret,
/// donc on stoppe au minimum l'emission cote UI).
#[tauri::command]
pub async fn start_crawl(
    app: AppHandle,
    state: State<'_, CrawlState>,
    config: CrawlConfig,
) -> Result<(), String> {
    // Reinitialise le drapeau d'arret pour ce nouveau crawl.
    state.stop.store(false, Ordering::SeqCst);

    // Clones de l'Arc pour chaque closure (gate d'emission).
    let stop_progress = Arc::clone(&state.stop);
    let stop_page = Arc::clone(&state.stop);

    // Clones de l'AppHandle (cheap, ref-compte) pour les emissions.
    let app_progress = app.clone();
    let app_page = app.clone();
    let app_done = app.clone();

    tauri::async_runtime::spawn(async move {
        // Closure progression -> evenement "crawl:progress".
        let on_progress = move |progress: CrawlProgress| {
            if stop_progress.load(Ordering::SeqCst) {
                return; // Arret demande : on n'emet plus.
            }
            let _ = app_progress.emit("crawl:progress", progress);
        };

        // Closure page -> evenement "crawl:page".
        let on_page = move |page: PageReport| {
            if stop_page.load(Ordering::SeqCst) {
                return; // Arret demande : on n'emet plus.
            }
            let _ = app_page.emit("crawl:page", page);
        };

        // Lancement du crawl.
        match crawl_core::crawler::run(config, on_progress, on_page).await {
            Ok(summary) => {
                // On emet toujours le resume final (meme apres un stop) afin que
                // l'UI puisse cloturer proprement l'etat "en cours".
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

/// Demande l'arret du crawl en cours : passe le drapeau a `true`.
/// Les closures d'emission cesseront de relayer les evenements vers le frontend.
#[tauri::command]
pub fn stop_crawl(state: State<'_, CrawlState>) {
    state.stop.store(true, Ordering::SeqCst);
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
