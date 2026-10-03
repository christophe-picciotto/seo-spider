// Pas de console Windows en release (fenêtre GUI uniquement).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::CrawlState;

fn main() {
    tauri::Builder::default()
        // État partage : drapeau d'arrêt du crawl.
        .manage(CrawlState::default())
        .plugin(tauri_plugin_dialog::init())
        // Commandes exposees au frontend.
        .invoke_handler(tauri::generate_handler![
            commands::start_crawl,
            commands::stop_crawl,
            commands::clear_crawl,
            commands::network_info,
            commands::load_history,
            commands::save_history,
            commands::save_crawl,
            commands::load_crawl,
            commands::open_crawls_folder,
            commands::open_external,
            commands::pick_folder,
            commands::pick_open_project,
            commands::pick_url_list,
            commands::history_location,
            commands::open_history_folder
        ])
        .run(tauri::generate_context!())
        .expect("erreur au demarrage de l'application Tauri");
}
