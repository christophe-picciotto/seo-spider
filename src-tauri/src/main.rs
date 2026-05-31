// Pas de console Windows en release (fenetre GUI uniquement).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::CrawlState;

fn main() {
    tauri::Builder::default()
        // Etat partage : drapeau d'arret du crawl.
        .manage(CrawlState::default())
        // Commandes exposees au frontend.
        .invoke_handler(tauri::generate_handler![
            commands::start_crawl,
            commands::stop_crawl,
            commands::network_info,
            commands::load_history,
            commands::save_history,
            commands::history_location,
            commands::open_history_folder
        ])
        .run(tauri::generate_context!())
        .expect("erreur au demarrage de l'application Tauri");
}
