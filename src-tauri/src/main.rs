// Pas de console Windows en release (fenêtre GUI uniquement).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::sync::atomic::Ordering;
use std::time::Duration;

use commands::{ClosingState, CrawlState, SavedCrawlReads};
use crawl_core::savedcrawl::{CloseOutcome, SaveSessions, CLOSE_REASK, CLOSE_SILENCE};
use tauri::{Emitter, Manager};

/// Délai de sûreté de la fermeture différée : plus aucun octet reçu depuis
/// CLOSE_STALL alors qu'un enregistrement envoie ses morceaux (jamais pendant
/// la fin d'un enregistrement), ou CLOSE_CAP d'attente en tout — la fenêtre se
/// ferme quand même.
const CLOSE_STALL: Duration = Duration::from_secs(30);
const CLOSE_CAP: Duration = Duration::from_secs(300);
// Délai de SILENCE (CLOSE_SILENCE, 120 s) et redemande (CLOSE_REASK, 2 s) :
// définis et justifiés dans crawl_core::savedcrawl. Une fenêtre vivante qui lit
// encore des milliers de pages en file envoie un signe de vie au plus chaque
// seconde (`alive`), et elle est attendue aussi longtemps qu'il le faut (le
// crawl étant arrêté dès la demande, sa file ne grossit plus ; CLOSE_CAP ne
// court qu'une fois l'annonce lue). Une seconde demande ferme, toujours.

fn main() {
    tauri::Builder::default()
        // État partage : drapeau d'arrêt du crawl.
        .manage(CrawlState::default())
        // Lecture en flux d'un crawl enregistré (une à la fois).
        .manage(SavedCrawlReads::default())
        // Enregistrements de crawls en cours (en morceaux, un par site).
        .manage(SaveSessions::default())
        .manage(ClosingState::default())
        // Fermer la fenêtre : la fermeture est TOUJOURS d'abord soumise à la
        // fenêtre — elle seule sait si elle a des pages non enregistrées (à
        // pleine vitesse, elle peut avoir des milliers de pages de retard sur le
        // moteur : crawl fini pour lui, pas encore enregistré par elle). Elle en
        // reçoit l'annonce (`app:closing_after_save`) APRÈS tout ce qui la
        // précède, répond aussitôt par un signe de vie (`close_ack`), met un
        // crawl en cours en pause (ce qui l'enregistre) ou enregistre ce qui ne
        // l'est pas. Le moteur attend que le crawl soit fini et que plus rien ne
        // s'enregistre, lui demande si elle a encore quelque chose à enregistrer
        // (`app:close_check`, réponse `close_ready`), puis ferme — sauf si un
        // enregistrement a échoué entre-temps : la fenêtre reste alors ouverte
        // avec son message (`app:closing_cancelled`). Rien à enregistrer : un
        // aller-retour de quelques millisecondes. Sans signe de vie dans le
        // délai de silence (CLOSE_SILENCE) : on ferme. Une SECONDE demande ferme tout de suite.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let closing = window.state::<ClosingState>();
                if closing.requested.load(Ordering::SeqCst) {
                    return; // seconde demande : fermer tout de suite
                }
                let saves = window.state::<SaveSessions>().inner().clone();
                let running = std::sync::Arc::clone(&window.state::<CrawlState>().running);
                api.prevent_close();
                closing.ready.store(false, Ordering::SeqCst);
                closing.acked.store(false, Ordering::SeqCst);
                closing.unsaved.store(false, Ordering::SeqCst);
                closing.failures_at.store(saves.failures(), Ordering::SeqCst);
                closing.requested.store(true, Ordering::SeqCst);
                // Un crawl qui tourne s'arrête tout de suite (comme un Stop) : sans
                // cela, une fenêtre en retard ne lirait l'annonce qu'après des pages
                // qui, elles, continueraient d'arriver.
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(s) = app.state::<CrawlState>().inner.lock().await.as_ref() {
                        s.request_stop();
                    }
                });
                // Signe IMMÉDIAT, dans le titre : la fenêtre peut avoir encore des
                // milliers de pages en file à lire avant d'afficher son message.
                let texte = closing.title_text.lock().unwrap().clone();
                if !texte.is_empty() {
                    if let Ok(avant) = window.title() {
                        let _ = window.set_title(&format!("{avant} — {texte}"));
                        *closing.title_before.lock().unwrap() = Some(avant);
                    }
                }
                let _ = window.emit("app:closing_after_save", ());
                let window = window.clone();
                std::thread::spawn(move || {
                    let closing = window.state::<ClosingState>();
                    let issue = saves.wait_close(
                        closing.failures_at.load(Ordering::SeqCst),
                        &closing.ready,
                        &closing.acked,
                        &closing.life,
                        || {
                            let _ = window.emit("app:close_check", ());
                        },
                        || running.load(Ordering::SeqCst) > 0,
                        || !closing.requested.load(Ordering::SeqCst),
                        CLOSE_SILENCE,
                        CLOSE_REASK,
                        CLOSE_STALL,
                        CLOSE_CAP,
                    );
                    match issue {
                        CloseOutcome::Close | CloseOutcome::Forced => {
                            let _ = window.destroy();
                        }
                        // Un échec pendant une fermeture qui suit une annulation (la fenêtre
                        // a promis que la suivante fermerait, crawl gardé ou non) : on ferme,
                        // comme annoncé — l'échec ne la retient plus.
                        CloseOutcome::Stay if closing.unsaved.load(Ordering::SeqCst) => {
                            let _ = window.destroy();
                        }
                        // Un enregistrement a échoué : la fenêtre reste, avec son message.
                        CloseOutcome::Stay => {
                            closing.cancel(&window);
                            let _ = window.emit("app:closing_cancelled", ());
                        }
                        CloseOutcome::Cancelled => {}
                    }
                });
            }
        })
        .plugin(tauri_plugin_dialog::init())
        // Commandes exposees au frontend.
        .invoke_handler(tauri::generate_handler![
            commands::start_crawl,
            commands::stop_crawl,
            commands::clear_crawl,
            commands::network_info,
            commands::load_history,
            commands::save_history,
            commands::save_crawl_begin,
            commands::save_crawl_part,
            commands::save_crawl_end,
            commands::save_crawl_abort,
            commands::saved_crawl_info,
            commands::saved_crawl_open,
            commands::saved_crawl_next,
            commands::saved_crawl_close,
            commands::open_crawls_folder,
            commands::open_external,
            commands::pick_folder,
            commands::pick_open_project,
            commands::pick_url_list,
            commands::history_location,
            commands::open_history_folder,
            commands::alive,
            commands::closing_title,
            commands::close_ack,
            commands::close_ready,
            commands::close_cancel
        ])
        .build(tauri::generate_context!())
        .expect("erreur au demarrage de l'application Tauri")
        .run(|app, event| {
            // Fermeture FORCÉE pendant un enregistrement (seconde demande, ou
            // délai de sûreté) : la fenêtre ne remettra plus aucun morceau. Le
            // temporaire incomplet est retiré, l'ancien fichier du site reste.
            if let tauri::RunEvent::Exit = event {
                app.state::<SaveSessions>().abort_all();
            }
        });
}
