//! Localisation et lancement de Chrome/Chromium pour chromiumoxide 0.7.
//!
//! Deux responsabilites :
//! - `find_chrome` : trouver un binaire Chrome (ou Edge en repli sous Windows)
//!   sur le système hote, sans dependance externe.
//! - `launch_browser` : démarrer un navigateur headless via chromiumoxide et
//!   drainer son `Handler` dans une task tokio dédiée (obligatoire : sans cela
//!   le navigateur ne traite jamais les messages CDP et se bloque).

use std::path::PathBuf;

use anyhow::Context;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;

/// Localise un binaire Chrome (ou Edge en repli sous Windows) utilisable.
///
/// Renvoie le premier chemin existant trouve, sinon `None` (chromiumoxide
/// tentera alors sa propre detection au lancement).
pub fn find_chrome() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Chemins absolus standards d'installation de Chrome puis d'Edge.
        let mut candidates: Vec<PathBuf> = vec![
            PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
            PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
        ];

        // Installation per-user de Chrome via %LOCALAPPDATA%.
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let mut p = PathBuf::from(&local_app_data);
            p.push(r"Google\Chrome\Application\chrome.exe");
            candidates.push(p);
        }

        // Repli sur Microsoft Edge (moteur Chromium, compatible CDP).
        candidates.push(PathBuf::from(
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ));
        candidates.push(PathBuf::from(
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ));
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let mut p = PathBuf::from(&local_app_data);
            p.push(r"Microsoft\Edge\Application\msedge.exe");
            candidates.push(p);
        }

        candidates.into_iter().find(|p| p.exists())
    }

    #[cfg(not(windows))]
    {
        // Sous Linux : chercher les binaires connus dans le PATH.
        const NAMES: [&str; 4] = [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ];
        NAMES.iter().find_map(|name| find_in_path(name))
    }
}

/// Cherche `name` dans chaque répertoire du `PATH` et renvoie le 1er existant.
#[cfg(not(windows))]
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(name);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    })
}

/// Lance un navigateur headless et renvoie `(browser, join_handle)`.
///
/// Le `Handler` retourne par `Browser::launch` DOIT être draine en continu :
/// on spawn une task tokio qui boucle `while let Some(_) = handler.next().await {}`.
/// Son `JoinHandle` est renvoye pour que l'appelant puisse l'abandonner/attendre.
pub async fn launch_browser() -> anyhow::Result<(Browser, tokio::task::JoinHandle<()>)> {
    // Headless est le mode par defaut du builder ; on n'appelle donc PAS
    // `with_head()`. Si un binaire est localise, on le fixe explicitement,
    // sinon on laisse chromiumoxide tenter sa propre detection.
    let mut builder = BrowserConfig::builder();
    if let Some(exe) = find_chrome() {
        builder = builder.chrome_executable(exe);
    }

    // `build()` renvoie `Result<BrowserConfig, String>` : on convertit l'erreur
    // String en `anyhow::Error` via `map_err`.
    let config = builder
        .build()
        .map_err(|e| anyhow::anyhow!("impossible de construire BrowserConfig: {e}"))?;

    // `launch` est async et renvoie `Result<(Browser, Handler)>`.
    let (browser, mut handler) = Browser::launch(config)
        .await
        .context("échec du lancement du navigateur (Chrome/Chromium introuvable ?)")?;

    // Le `Handler` implemente `futures::Stream` (Item = Result<(), CdpError>) ;
    // on le draine jusqu'à sa fin (lorsque le navigateur se ferme).
    let join_handle = tokio::spawn(async move {
        while let Some(_event) = handler.next().await {}
    });

    Ok((browser, join_handle))
}
