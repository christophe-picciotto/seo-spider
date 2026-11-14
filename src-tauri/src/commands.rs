//! Commandes Tauri v2 exposees au frontend.
//!
//! Contrat d'événements (doivent correspondre aux `listen(...)` du frontend) :
//!   - `crawl:progress` : payload `crawl_core::CrawlProgress` (avancement)
//!   - `crawl:page`     : payload `crawl_core::PageReport` (une page mesurée)
//!   - `crawl:done`     : payload `crawl_core::CrawlSummary` (resume final)
//!   - `crawl:error`    : payload `String` (message d'erreur global)
//!
//! Commandes (doivent correspondre aux `invoke("...")` du frontend) :
//!   - `start_crawl(config)` : DEMARRE un nouveau crawl, ou REPREND un crawl mis
//!                             en pause (même `seed_url` + file non vide).
//!   - `stop_crawl()`        : met le crawl en PAUSE (l'état est conserve, on peut
//!                             reprendre via un nouvel appel a `start_crawl`).
//!   - `clear_crawl()`       : VIDE l'état de crawl (après ca, `start_crawl` repart
//!                             de zero). Le bouton "Vider" de l'UI peut l'appeler.
//!   - `open_external(url)`  : ouvre une URL http(s) dans le navigateur par
//!                             defaut du système (impossible depuis la WebView).
//!
//! ## Pause / reprise (Vague 4)
//!
//! L'état du crawl ([`crawl_core::crawler::SharedState`]) est stocke dans l'état
//! applicatif Tauri ([`CrawlState`]) afin de SURVIVRE entre un `stop` (pause) et
//! un `start` (reprise). `stop_crawl` ne fait que lever un drapeau : les workers
//! finissent leur page courante puis sortent sans vider la file. Un `start_crawl`
//! ulterieur, si un état reprenable existe pour la même seed, RELANCE les workers
//! sur la file existante (reprise) ; sinon il initialise un état neuf.
//!
//! Ceci corrige aussi le bug du bouton "Démarrer" inerte après un "Stop" : comme
//! les workers s'arretent vraiment, `crawl:done` est emis rapidement et l'UI sort
//! de l'état "en cours" ; un nouvel appel a `start_crawl` repart (ou reprend).

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crawl_core::crawler::SharedState;
use crawl_core::report::{CrawlConfig, CrawlProgress, CrawlSummary, PageReport};
use tauri_plugin_dialog::DialogExt;

/// État partage gere par Tauri (via `.manage`). Porte l'état du crawl COURANT
/// (file + visited + agrégats + navigateur), conserve entre pause et reprise.
#[derive(Default)]
pub struct CrawlState {
    /// État du crawl courant, ou `None` si aucun crawl n'a (encore) été lance /
    /// si l'état a été vide via `clear_crawl`. Mutable derriere un `Mutex` car
    /// l'objet manage par Tauri est, lui, immuable.
    pub inner: Mutex<Option<Arc<SharedState>>>,
    /// Crawls dont la tâche tourne encore (workers pas tous sortis, `crawl:done`
    /// pas encore émis) : la fermeture différée de la fenêtre les attend.
    pub running: Arc<std::sync::atomic::AtomicUsize>,
    /// Pages remises à la fenêtre (`crawl:page`) depuis le lancement : avec le
    /// compte de celles qu'elle a lues, elle sait combien il lui en reste en
    /// file pendant une fermeture.
    pub pages_emitted: Arc<std::sync::atomic::AtomicU64>,
}

/// La fermeture de la fenêtre, différée le temps d'enregistrer (main.rs) :
/// demandée ? la fenêtre a-t-elle répondu qu'elle n'a plus rien à enregistrer ?
/// et le nombre d'échecs d'enregistrement au moment de la demande. Une seconde
/// demande ferme tout de suite.
#[derive(Default)]
pub struct ClosingState {
    pub requested: std::sync::atomic::AtomicBool,
    pub ready: std::sync::atomic::AtomicBool,
    /// La fermeture précédente a été annulée, et la fenêtre a promis que la
    /// suivante fermerait (« … la fermera SANS enregistrer le crawl. », « Vous
    /// pouvez fermer de nouveau la fenêtre. ») : un échec d'enregistrement pendant
    /// celle-ci ne la retient plus (`close_ready` avec `unsaved`).
    pub unsaved: std::sync::atomic::AtomicBool,
    /// La fenêtre a reçu l'annonce de la fermeture (`close_ack`).
    pub acked: std::sync::atomic::AtomicBool,
    /// Ses signes de vie (`alive`, au plus un par seconde pendant qu'elle lit
    /// des pages ; `close_ack`, `close_ready`) : compteur.
    pub life: std::sync::atomic::AtomicU64,
    pub failures_at: std::sync::atomic::AtomicU64,
    /// Texte ajouté au titre de la fenêtre dès la demande de fermeture, dans la
    /// langue de l'interface (`closing_title`) : SEUL signe visible tant que la
    /// fenêtre lit encore des pages en file, avant d'afficher son propre message.
    pub title_text: std::sync::Mutex<String>,
    /// Le titre d'avant, à remettre si la fermeture est annulée.
    pub title_before: std::sync::Mutex<Option<String>>,
}

impl ClosingState {
    /// Fermeture annulée (un enregistrement a échoué) : la prochaine demande
    /// repartira de zéro, et la fenêtre retrouve son titre.
    pub fn cancel(&self, window: &tauri::Window) {
        self.ready.store(false, std::sync::atomic::Ordering::SeqCst);
        self.requested.store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(t) = self.title_before.lock().unwrap().take() {
            let _ = window.set_title(&t);
        }
    }
}

/// Le texte, traduit, que la fenêtre veut voir ajouter à son titre pendant une
/// fermeture différée (envoyé au démarrage et à chaque changement de langue).
#[tauri::command]
pub fn closing_title(closing: State<'_, ClosingState>, text: String) {
    *closing.title_text.lock().unwrap() = text;
}

/// Signe de vie de la fenêtre pendant qu'elle lit des pages (au plus un par
/// seconde) : même avec des milliers de pages en file, la fermeture différée
/// sait qu'elle est vivante et l'attend.
#[tauri::command]
pub fn alive(closing: State<'_, ClosingState>, crawl: State<'_, CrawlState>) -> AliveReply {
    closing.life.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    AliveReply {
        closing: closing.requested.load(std::sync::atomic::Ordering::SeqCst),
        emitted: crawl.pages_emitted.load(std::sync::atomic::Ordering::SeqCst),
    }
}

/// Réponse au signe de vie : une fermeture est-elle demandée (la fenêtre cesse
/// alors ses rendus et ne fait plus que lire ses pages, enregistrer, fermer) ?
/// et combien de pages le moteur lui a remises en tout (pour dire combien il lui
/// en reste à lire).
#[derive(serde::Serialize)]
pub struct AliveReply {
    closing: bool,
    emitted: u64,
}

/// La fenêtre a reçu l'annonce de la fermeture (donc lu tout ce qui la
/// précédait : pages en file, `crawl:done`).
#[tauri::command]
pub fn close_ack(closing: State<'_, ClosingState>) {
    closing.life.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    closing.acked.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Réponse de la fenêtre à `app:close_check` : elle n'a plus rien à enregistrer.
/// `unsaved` : elle tient la promesse de l'annulation précédente — un échec
/// d'enregistrement pendant cette fermeture ne la retient pas (main.rs).
#[tauri::command]
pub fn close_ready(closing: State<'_, ClosingState>, unsaved: Option<bool>) {
    closing.life.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    closing.unsaved.store(unsaved.unwrap_or(false), std::sync::atomic::Ordering::SeqCst);
    closing.acked.store(true, std::sync::atomic::Ordering::SeqCst);
    closing.ready.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// La fenêtre renonce à la fermeture différée (un enregistrement a échoué :
/// son message doit rester à l'écran) ; la prochaine demande repartira de zéro.
#[tauri::command]
pub fn close_cancel(window: tauri::Window, closing: State<'_, ClosingState>) {
    closing.cancel(&window);
}

/// Décompte d'un crawl en cours, rendu à la fin de sa tâche, même si elle panique.
struct RunningGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Demarre un nouveau crawl, ou REPREND un crawl mis en pause.
///
/// Heuristique de decision :
///   - s'il existe un état reprenable (`SharedState` déjà initialise, file non
///     vide) pour la MÊME `seed_url` -> on REPREND sur cet état ;
///   - sinon -> on crée un état NEUF à partir de la seed et on l'enregistre.
///
/// Dans les deux cas, on spawn une task qui appelle `crawler::run_shared` avec
/// deux closures relayant l'avancement (`crawl:progress`) et chaque page
/// (`crawl:page`) vers le frontend, puis emet `crawl:done` avec le resume.
#[tauri::command]
pub async fn start_crawl(
    app: AppHandle,
    state: State<'_, CrawlState>,
    closing: State<'_, ClosingState>,
    config: CrawlConfig,
) -> Result<(), String> {
    // La fenêtre se ferme (elle enregistre d'abord ce qu'elle a) : aucun crawl
    // ne démarre plus, sinon la fermeture l'attendrait.
    if closing.requested.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("fermeture de la fenêtre en cours".to_string());
    }
    // Seed nettoyee de son fragment (`#...`) AVANT tout : `SharedState::new` fait
    // le même nettoyage, donc sans ca la comparaison de reprise ligne ~73
    // (`existing.seed_url == config.seed_url`) opposerait une seed normalisée a
    // une seed brute et ne matcherait jamais.
    let config = CrawlConfig {
        seed_url: crawl_core::crawler::strip_fragment(&config.seed_url),
        ..config
    };

    // --- Choix : reprise d'un état existant, ou nouvel état. ---
    let is_resume: bool;
    let shared: Arc<SharedState> = {
        let mut slot = state.inner.lock().await;

        // Reprise possible ? Même seed + état encore reprenable (file non vide).
        let resume = match slot.as_ref() {
            Some(existing) => {
                existing.seed_url == config.seed_url && existing.is_resumable().await
            }
            None => false,
        };
        is_resume = resume;

        if resume {
            // REPREND : on reutilise l'état partage existant tel quel.
            Arc::clone(slot.as_ref().unwrap())
        } else {
            // NOUVEAU : on crée un état neuf amorce avec la seed et on l'enregistre.
            let fresh = Arc::new(SharedState::new(&config.seed_url));
            *slot = Some(Arc::clone(&fresh));
            fresh
        }
    };

    // Sitemap : fetch au DÉBUT (comme robots.txt), uniquement pour un NOUVEAU crawl.
    let seed = config.seed_url.clone();
    let do_sitemap = !is_resume;
    let app_sitemap = app.clone();

    // Clones de l'AppHandle (cheap, ref-compte) pour les emissions.
    let app_progress = app.clone();
    let app_page = app.clone();
    let app_done = app.clone();

    // Compté jusqu'à la fin de la tâche (APRÈS l'émission de `crawl:done`).
    state.running.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let en_cours = RunningGuard(Arc::clone(&state.running));
    let emitted = Arc::clone(&state.pages_emitted);

    tauri::async_runtime::spawn(async move {
        let _en_cours = en_cours;
        // Sitemap recupere EN PARALLELE du crawl (et non avant lui).
        //
        // Il etait `await` ici, donc le crawl ne demarrait qu'une fois le
        // sitemap recupere ou en timeout. Sur un domaine dont le DNS ne resout
        // pas, cela retardait de plusieurs secondes le simple constat que le
        // site est mort — et, en mode multi-sites, le passage au suivant. Les
        // workers restaient inactifs pendant ce temps.
        //
        // Le résultat est recupere juste avant d'emettre `crawl:done` : il a
        // largement eu le temps d'arriver, et l'onglet Sitemap se remplit des
        // reception via l'événement `crawl:sitemap`.
        let sitemap_task = if do_sitemap {
            Some(tauri::async_runtime::spawn(async move {
                let urls = crawl_core::sitemap::fetch_default(&seed).await;
                let _ = app_sitemap.emit("crawl:sitemap", urls.clone());
                urls
            }))
        } else {
            None
        };

        // Closure progression -> événement "crawl:progress".
        let on_progress = move |progress: CrawlProgress| {
            let _ = app_progress.emit("crawl:progress", progress);
        };

        // Closure page -> événement "crawl:page" (compté : cf. CrawlState::pages_emitted).
        let on_page = move |page: PageReport| {
            emitted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = app_page.emit("crawl:page", page);
        };

        // Lancement (ou reprise) du crawl sur l'état partage.
        let crawl_result = crawl_core::crawler::run_shared(shared, config, on_progress, on_page).await;

        // Le sitemap tournait en parallele : on le recupere maintenant. Une
        // task qui panique ou qui est annulee ne doit pas faire echouer le
        // crawl, d'ou le repli sur une liste vide.
        let sitemap_urls: Vec<String> = match sitemap_task {
            Some(h) => h.await.unwrap_or_default(),
            None => Vec::new(),
        };

        match crawl_result {
            Ok(mut summary) => {
                // On emet toujours le resume (fin naturelle OU pause) afin que l'UI
                // sorte de l'état "en cours". En cas de pause, le resume est partiel
                // et l'état reste reprenable via un nouvel appel a start_crawl.
                if !sitemap_urls.is_empty() {
                    summary.sitemap_urls = sitemap_urls;
                }
                let _ = app_done.emit("crawl:done", summary);
            }
            Err(e) => {
                // Échec global : on remonte un resume vide marque comme termine,
                // et on emet aussi un événement d'erreur dédié.
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

/// Met le crawl en cours en PAUSE : leve le drapeau d'arrêt de l'état partage.
/// Les workers finissent leur page courante puis sortent SANS vider l'état ;
/// un `start_crawl` ulterieur (même seed) reprendra la ou on s'etait arrete.
#[tauri::command]
pub async fn stop_crawl(state: State<'_, CrawlState>) -> Result<(), String> {
    if let Some(shared) = state.inner.lock().await.as_ref() {
        shared.request_stop();
    }
    Ok(())
}

/// VIDE l'état de crawl : leve le drapeau d'arrêt du crawl courant (s'il existe)
/// puis oublié l'état partage. Après cet appel, `start_crawl` repart de zero.
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

/// Renvoie l'IP publique sortante + le reverse DNS (affiche dans l'en-tête).
#[tauri::command]
pub async fn network_info() -> Result<crawl_core::netinfo::NetworkInfo, String> {
    crawl_core::netinfo::fetch().await.map_err(|e| e.to_string())
}

/// Chemin du fichier d'historique, dans le dossier de données de l'application.
fn history_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("history.json"))
}

/// Entrée d'historique ALLÉGÉE : toutes ses clés, sauf `pagesData`.
///
/// Jusqu'à la 0.1.0, chaque entrée embarquait la copie intégrale des pages du
/// crawl (`pagesData`, jusqu'à 5 000 pages). Sur 100 entrées, `history.json`
/// atteignait des centaines de Mo, que la WebView devait décoder au démarrage
/// puis re-sérialiser EN ENTIER à chaque fin de crawl : plusieurs Go de
/// mémoire, et le processus de rendu finissait par planter (« Out of Memory »).
///
/// La désérialisation est écrite à la main pour SAUTER `pagesData` sans le
/// construire ([`serde::de::IgnoredAny`] n'alloue rien) : un fichier de 450 Mo
/// se réduit à quelques Ko sans jamais matérialiser ses pages en mémoire.
struct LeanEntry {
    fields: serde_json::Map<String, serde_json::Value>,
    /// L'entrée d'origine portait un `pagesData` (donc : fichier à migrer).
    had_pages: bool,
}

impl<'de> serde::Deserialize<'de> for LeanEntry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntryVisitor;

        impl<'de> serde::de::Visitor<'de> for EntryVisitor {
            type Value = LeanEntry;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une entrée d'historique (objet JSON)")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<LeanEntry, A::Error> {
                let mut fields = serde_json::Map::new();
                let mut had_pages = false;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "pagesData" {
                        map.next_value::<serde::de::IgnoredAny>()?;
                        had_pages = true;
                    } else {
                        fields.insert(key, map.next_value()?);
                    }
                }
                Ok(LeanEntry { fields, had_pages })
            }
        }

        deserializer.deserialize_map(EntryVisitor)
    }
}

/// Allège un historique brut : retire `pagesData` de chaque entrée. Renvoie le
/// JSON allégé, et `true` si au moins une entrée en portait un.
fn slim_history(raw: &[u8]) -> Result<(String, bool), serde_json::Error> {
    let entries: Vec<LeanEntry> = serde_json::from_slice(raw)?;
    let had_pages = entries.iter().any(|e| e.had_pages);
    let fields: Vec<&serde_json::Map<String, serde_json::Value>> =
        entries.iter().map(|e| &e.fields).collect();
    Ok((serde_json::to_string(&fields)?, had_pages))
}

/// Premier nom libre pour conserver un historique d'avant allègement :
/// `history.avant-allegement.json`, puis `…-2.json`, `…-3.json`. Une copie déjà
/// présente n'est JAMAIS écrasée.
fn history_backup_path(path: &std::path::Path) -> std::path::PathBuf {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut n = 1u32;
    loop {
        let name = if n == 1 {
            "history.avant-allegement.json".to_string()
        } else {
            format!("history.avant-allegement-{n}.json")
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Lit l'historique et renvoie sa forme ALLÉGÉE (sans `pagesData`) : c'est la
/// seule que la WebView doit recevoir. "[]" si le fichier est absent ou illisible.
///
/// MIGRATION (une seule fois) : si le fichier contenait des `pagesData`, il est
/// réécrit allégé. L'original n'est pas détruit : il est RENOMMÉ à côté (cf.
/// [`history_backup_path`]) — ses pages ne sont lues par aucun écran, mais ce
/// n'est pas à une mise à jour de les effacer. Le fichier peut être supprimé à
/// la main.
fn load_history_from(path: &std::path::Path) -> String {
    let raw = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return "[]".to_string(),
    };
    let (lean, had_pages) = match slim_history(&raw) {
        Ok(result) => result,
        // Illisible : le frontend repartait déjà d'une liste vide dans ce cas.
        // On ne lui envoie surtout pas le fichier brut (il peut être énorme).
        Err(_) => return "[]".to_string(),
    };
    drop(raw);

    if had_pages {
        // L'original est mis à l'abri AVANT toute réécriture. Si le renommage
        // échoue (fichier verrouillé par un autre programme…), on ne réécrit
        // rien ici : la WebView reçoit de toute façon la forme allégée, et
        // c'est le prochain `save_history` qui l'écrira par-dessus.
        let backup = history_backup_path(path);
        if std::fs::rename(path, &backup).is_ok() && std::fs::write(path, &lean).is_err() {
            // Réécriture impossible : on remet l'original à sa place plutôt
            // que de laisser l'application sans fichier d'historique.
            let _ = std::fs::rename(&backup, path);
        }
    }
    lean
}

/// Charge l'historique (blob JSON) depuis le disque. Renvoie "[]" si absent.
///
/// `async` : exécutée HORS du thread principal. La première lecture d'un ancien
/// historique volumineux (migration) prend de l'ordre de la seconde ; sur le
/// thread principal, la fenêtre serait figée pendant ce temps.
#[tauri::command(async)]
pub fn load_history(app: AppHandle) -> Result<String, String> {
    let path = history_path(&app)?;
    Ok(load_history_from(&path))
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
        // .spawn() : explorer.exe renvoie souvent un code != 0 même en succès.
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


/// Dossier des crawls enregistres (sous le dossier de données de l'application).
fn crawls_dir(app: &AppHandle, custom: &Option<String>) -> Result<std::path::PathBuf, String> {
    if let Some(d) = custom {
        if !d.trim().is_empty() {
            return Ok(std::path::PathBuf::from(d.trim()));
        }
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("crawls"))
}

/// Transforme un identifiant (URL de départ) en nom de fichier sur (sans separateurs).
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

// -----------------------------------------------------------------------------
// Enregistrer un crawl : EN MORCEAUX (cf. `crawl_core::savedcrawl::SaveSessions`)
// -----------------------------------------------------------------------------
//
// Jusqu'au 14/11/2026, `save_crawl(id, json, dir)` recevait la chaîne du crawl
// ENTIER et l'écrivait d'un coup, sur le fil principal (commande synchrone :
// la fenêtre figeait). Au-delà de ~42 000 pages, la fenêtre ne pouvait même pas
// fabriquer cette chaîne (longueur maximale d'une chaîne JavaScript) : rien
// n'était enregistré. Désormais (`saveCurrentCrawl` dans main.js) :
//   save_crawl_begin(id, dir)            → numéro de l'enregistrement
//   save_crawl_part(<octets>)            × n, un morceau ≤ 4 194 304 caractères,
//                                        numéro dans l'en-tête `Seo-Spider-Save`,
//                                        position dans `Seo-Spider-Offset`
//   save_crawl_end(save, total)          → chemin du fichier (renommage final) ;
//                                        en échec { error, kept } (cf. SaveFailure)
//   save_crawl_abort(save)               en cas d'échec côté fenêtre
// Le format du fichier ne change pas : la fenêtre fabrique le même texte que
// `JSON.stringify` du crawl entier, page par page.

/// En-tête HTTP qui porte le numéro de l'enregistrement dans `save_crawl_part`
/// (le corps, lui, est le morceau en octets : aucun échappement JSON).
const SAVE_HEADER: &str = "seo-spider-save";

/// En-tête HTTP qui porte la POSITION du morceau (octets déjà envoyés par la
/// fenêtre) : un morceau perdu ou rejoué ne peut pas corrompre le fichier.
const SAVE_OFFSET_HEADER: &str = "seo-spider-offset";

/// L'échec de `save_crawl_end`, tel que la fenêtre le reçoit : la raison et,
/// si le crawl complet a pu être gardé sur le disque malgré l'échec (fichier
/// du site tenu par un autre programme), le fichier où il se trouve.
#[derive(Debug, serde::Serialize)]
pub struct SaveFailure {
    error: String,
    kept: Option<String>,
}

impl From<crawl_core::savedcrawl::SaveError> for SaveFailure {
    fn from(e: crawl_core::savedcrawl::SaveError) -> Self {
        SaveFailure { error: e.reason, kept: e.kept.map(|p| p.to_string_lossy().to_string()) }
    }
}

/// Commence l'enregistrement du crawl de `id` sous `crawls/<id>.json` (ou le
/// dossier choisi) : fichier temporaire à côté, l'ancien fichier reste intact
/// jusqu'à `save_crawl_end`. Rend le numéro de l'enregistrement.
#[tauri::command]
pub async fn save_crawl_begin(
    app: AppHandle,
    saves: State<'_, crawl_core::savedcrawl::SaveSessions>,
    id: String,
    dir: Option<String>,
) -> Result<u32, String> {
    let path = crawls_dir(&app, &dir)?.join(format!("{}.json", sanitize_id(&id)));
    let saves = saves.inner().clone();
    tauri::async_runtime::spawn_blocking(move || saves.begin(&path))
        .await
        .map_err(|e| e.to_string())?
}

/// Le morceau, en octets. Par le protocole `ipc://` (le chemin normal), Tauri
/// remet le Uint8Array tel quel (`InvokeBody::Raw`). Si ce protocole a échoué
/// une seule fois, Tauri fait passer TOUT l'IPC de la fenêtre par `postMessage`
/// jusqu'à la fin de la session, et le Uint8Array y arrive en tableau de nombres
/// (`InvokeBody::Json`) : accepté aussi, l'enregistrement marche par les deux.
fn save_part_bytes(body: &tauri::ipc::InvokeBody) -> Result<Vec<u8>, String> {
    match body {
        tauri::ipc::InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        tauri::ipc::InvokeBody::Json(serde_json::Value::Array(values)) => values
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| "morceau illisible".to_string()),
        tauri::ipc::InvokeBody::Json(_) => Err("morceau attendu en octets".to_string()),
    }
}

/// Un nombre porté par un en-tête de `save_crawl_part`.
fn header_number<T: std::str::FromStr>(request: &tauri::ipc::Request<'_>, name: &str) -> Option<T> {
    request.headers().get(name).and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok())
}

/// Ajoute un morceau à un enregistrement : le CORPS de l'appel, en octets
/// (`invoke("save_crawl_part", Uint8Array, { headers: { "Seo-Spider-Save": n,
/// "Seo-Spider-Offset": position } })`), écrit hors du fil principal.
#[tauri::command]
pub async fn save_crawl_part(
    saves: State<'_, crawl_core::savedcrawl::SaveSessions>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let save: u32 = header_number(&request, SAVE_HEADER).ok_or_else(|| "numéro d'enregistrement absent".to_string())?;
    let offset: u64 = header_number(&request, SAVE_OFFSET_HEADER).ok_or_else(|| "position du morceau absente".to_string())?;
    let bytes = save_part_bytes(request.body())?;
    let saves = saves.inner().clone();
    tauri::async_runtime::spawn_blocking(move || saves.write(save, offset, bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// Termine un enregistrement : `total` = octets envoyés par la fenêtre
/// (contrôlés), écriture forcée sur le disque, puis le temporaire est RENOMMÉ
/// par-dessus l'ancien fichier. Rend le chemin du fichier ; en échec, la raison
/// et, s'il a pu être gardé, le fichier où se trouve le crawl.
#[tauri::command]
pub async fn save_crawl_end(
    saves: State<'_, crawl_core::savedcrawl::SaveSessions>,
    save: u32,
    total: u64,
) -> Result<String, SaveFailure> {
    let saves = saves.inner().clone();
    tauri::async_runtime::spawn_blocking(move || saves.end(save, total))
        .await
        .map_err(|e| SaveFailure { error: e.to_string(), kept: None })?
        .map(|p| p.to_string_lossy().to_string())
        .map_err(SaveFailure::from)
}

/// Abandonne un enregistrement (erreur côté fenêtre) : temporaire retiré,
/// ancien fichier intact.
#[tauri::command]
pub async fn save_crawl_abort(saves: State<'_, crawl_core::savedcrawl::SaveSessions>, save: u32) -> Result<(), String> {
    let saves = saves.inner().clone();
    tauri::async_runtime::spawn_blocking(move || saves.abort(save))
        .await
        .map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Exporter un CSV : écrit PAR LE MOTEUR dans le dossier des téléchargements
// -----------------------------------------------------------------------------
//
// Jusqu'au 14/11/2026, la fenêtre fabriquait un « téléchargement » (lien
// invisible cliqué par programme). WebView2 le traitait comme celui d'un site
// web : dès le deuxième export, il affichait SA demande « télécharger plusieurs
// fichiers : autoriser / bloquer », hors de l'application, et la fenêtre ne
// savait jamais si le fichier avait été écrit. Désormais (`downloadCsv` dans
// main.js), sur le modèle de l'enregistrement d'un crawl :
//   export_begin(name)          → numéro de l'export ; le fichier est créé dans
//                                 le dossier des téléchargements, sans jamais
//                                 remplacer un fichier existant (« nom (2).csv »)
//   export_part(<octets>)       × n, numéro dans l'en-tête `Seo-Spider-Export`,
//                                 position dans `Seo-Spider-Offset`
//   export_end(id, total)       → chemin complet du fichier écrit
//   export_abort(id)            en cas d'échec : le fichier incomplet est retiré

/// En-tête HTTP qui porte le numéro de l'export dans `export_part`.
const EXPORT_HEADER: &str = "seo-spider-export";

/// Un export en cours d'écriture.
struct ExportFile {
    file: std::fs::File,
    path: std::path::PathBuf,
    /// Octets écrits.
    bytes: u64,
    /// Position et longueur du dernier morceau écrit (sa répétition est ignorée).
    last: Option<(u64, usize)>,
}

impl ExportFile {
    /// Ferme et retire le fichier, incomplet.
    fn discard(self) {
        let ExportFile { file, path, .. } = self;
        drop(file);
        let _ = std::fs::remove_file(path);
    }
}

#[derive(Default)]
struct ExportTable {
    last_id: u32,
    open: std::collections::HashMap<u32, ExportFile>,
}

/// Les exports CSV en cours d'écriture (un fichier ouvert par numéro).
#[derive(Clone, Default)]
pub struct ExportSessions(Arc<std::sync::Mutex<ExportTable>>);

/// Nom de fichier d'un export, sans son extension : jamais un chemin (les
/// séparateurs et les caractères que Windows interdit dans un nom deviennent
/// « _ »), sans point ni espace aux deux bouts, 80 caractères au plus.
fn export_stem(name: &str) -> String {
    let name = name.trim();
    let name = match name.char_indices().rev().nth(3) {
        Some((at, _)) if name[at..].eq_ignore_ascii_case(".csv") => &name[..at],
        _ => name,
    };
    let stem: String = name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                ch
            }
        })
        .take(80)
        .collect();
    let stem = stem.trim_matches(|ch: char| ch == '.' || ch.is_whitespace());
    if stem.is_empty() {
        "export".to_string()
    } else {
        stem.to_string()
    }
}

impl ExportSessions {
    fn table(&self) -> std::sync::MutexGuard<'_, ExportTable> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Crée le fichier de l'export dans `dir` : « nom.csv », ou « nom (2).csv »…
    /// si ce nom est pris — un fichier existant n'est JAMAIS remplacé. Rend le
    /// numéro de l'export.
    pub fn begin(&self, dir: &std::path::Path, name: &str) -> Result<u32, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let stem = export_stem(name);
        for n in 1..=999u32 {
            let file_name = if n == 1 { format!("{stem}.csv") } else { format!("{stem} ({n}).csv") };
            let path = dir.join(file_name);
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    let mut table = self.table();
                    table.last_id += 1;
                    let id = table.last_id;
                    table.open.insert(id, ExportFile { file, path, bytes: 0, last: None });
                    return Ok(id);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("aucun nom de fichier libre pour cet export".to_string())
    }

    /// Ajoute un morceau à l'export `id`. `offset` : sa position dans le fichier
    /// (les octets que la fenêtre a déjà envoyés). La répétition du dernier
    /// morceau est ignorée ; un morceau hors position, ou une erreur d'écriture,
    /// abandonne l'export (fichier incomplet retiré).
    pub fn write(&self, id: u32, offset: u64, bytes: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let mut table = self.table();
        let Some(export) = table.open.get_mut(&id) else {
            return Err("export inconnu".to_string());
        };
        let echec = if offset != export.bytes {
            if export.last == Some((offset, bytes.len())) {
                return Ok(());
            }
            format!("morceau hors position (octet {offset}, attendu {})", export.bytes)
        } else {
            match export.file.write_all(bytes) {
                Ok(()) => {
                    export.bytes += bytes.len() as u64;
                    export.last = Some((offset, bytes.len()));
                    return Ok(());
                }
                Err(e) => e.to_string(),
            }
        };
        let abandonne = table.open.remove(&id);
        drop(table);
        if let Some(export) = abandonne {
            export.discard();
        }
        Err(echec)
    }

    /// Termine l'export `id`. `total` : les octets que la fenêtre a envoyés en
    /// tout ; s'il diffère de ce qui a été écrit, ou si le système refuse de
    /// confirmer l'écriture, le fichier est retiré. Rend le chemin du fichier.
    pub fn end(&self, id: u32, total: u64) -> Result<std::path::PathBuf, String> {
        let export = self.table().open.remove(&id).ok_or_else(|| "export inconnu".to_string())?;
        if export.bytes != total {
            let ecrits = export.bytes;
            export.discard();
            return Err(format!("export incomplet ({ecrits} octets écrits, {total} envoyés)"));
        }
        if let Err(e) = export.file.sync_all() {
            export.discard();
            return Err(e.to_string());
        }
        Ok(export.path)
    }

    /// Abandonne l'export `id` : son fichier, incomplet, est retiré.
    pub fn abort(&self, id: u32) {
        let abandonne = self.table().open.remove(&id);
        if let Some(export) = abandonne {
            export.discard();
        }
    }

    /// Abandonne tous les exports en cours (l'application se ferme : la fenêtre
    /// ne remettra plus aucun morceau).
    pub fn abort_all(&self) {
        let tous: Vec<ExportFile> = self.table().open.drain().map(|(_, e)| e).collect();
        for export in tous {
            export.discard();
        }
    }
}

/// Commence un export : crée « name » (extension .csv) dans le dossier des
/// téléchargements de l'utilisateur. Rend le numéro de l'export.
#[tauri::command]
pub async fn export_begin(app: AppHandle, exports: State<'_, ExportSessions>, name: String) -> Result<u32, String> {
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    let exports = exports.inner().clone();
    tauri::async_runtime::spawn_blocking(move || exports.begin(&dir, &name))
        .await
        .map_err(|e| e.to_string())?
}

/// Ajoute un morceau à un export : le CORPS de l'appel, en octets
/// (`invoke("export_part", Uint8Array, { headers: { "Seo-Spider-Export": n,
/// "Seo-Spider-Offset": position } })`), écrit hors du fil principal.
#[tauri::command]
pub async fn export_part(exports: State<'_, ExportSessions>, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let id: u32 = header_number(&request, EXPORT_HEADER).ok_or_else(|| "numéro d'export absent".to_string())?;
    let offset: u64 = header_number(&request, SAVE_OFFSET_HEADER).ok_or_else(|| "position du morceau absente".to_string())?;
    let bytes = save_part_bytes(request.body())?;
    let exports = exports.inner().clone();
    tauri::async_runtime::spawn_blocking(move || exports.write(id, offset, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// Termine un export : rend le chemin complet du fichier écrit.
#[tauri::command]
pub async fn export_end(exports: State<'_, ExportSessions>, id: u32, total: u64) -> Result<String, String> {
    let exports = exports.inner().clone();
    tauri::async_runtime::spawn_blocking(move || exports.end(id, total))
        .await
        .map_err(|e| e.to_string())?
        .map(|p| p.to_string_lossy().to_string())
}

/// Abandonne un export (erreur côté fenêtre) : le fichier incomplet est retiré.
#[tauri::command]
pub async fn export_abort(exports: State<'_, ExportSessions>, id: u32) -> Result<(), String> {
    let exports = exports.inner().clone();
    tauri::async_runtime::spawn_blocking(move || exports.abort(id))
        .await
        .map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Relire un crawl enregistré : EN FLUX (cf. `crawl_core::savedcrawl`)
// -----------------------------------------------------------------------------
//
// Jusqu'au 14/11/2026, `load_crawl` lisait le fichier en entier et le rendait
// comme une chaîne : Tauri la ré-échappait, la fenêtre la décodait puis la
// relisait. Pour le fichier de 415 Mo de Chris, la réponse faisait 460 millions
// de caractères (86 % de la longueur maximale d'une chaîne JavaScript) : au-delà
// de ~480 Mo, un crawl enregistré ne pouvait plus s'ouvrir du tout. Et
// l'interface l'appelait à chaque démarrage d'une adresse déjà crawlée, juste
// pour proposer « Nouveau crawl / Revoir l'enregistrement ».
//
// Désormais :
//   - `saved_crawl_info` : aperçu (adresse, date, NOMBRE de pages) pour ce
//     dialogue, sans charger aucune page ;
//   - `saved_crawl_open` / `pick_open_project` ouvrent une LECTURE ; la fenêtre
//     en tire les pages par `saved_crawl_next`, un message de ~4 Mo à la fois
//     (`{"pages":[…]}`), jusqu'au message de fin (`{"done":true,…}`) ;
//   - le lecteur n'a jamais plus d'un message d'avance : le moteur ne tient pas
//     le fichier, et la fenêtre ne reçoit jamais plus de ~4 Mo d'un coup.

/// Ce qu'une lecture remet à la fenêtre : un message de pages, ou le message de
/// fin (`savedcrawl::SavedCrawlEnd`), déjà en JSON.
enum SavedChunk {
    Pages(String),
    End(String),
}

struct SavedRead {
    id: u32,
    rx: tokio::sync::mpsc::Receiver<Result<SavedChunk, String>>,
}

/// La lecture en cours (UNE à la fois : en ouvrir une autre abandonne celle-ci,
/// dont le lecteur s'arrête dès qu'il veut remettre un message).
#[derive(Default)]
pub struct SavedCrawlReads {
    slot: std::sync::Mutex<Option<SavedRead>>,
    next_id: std::sync::atomic::AtomicU32,
}

/// Lance la lecture en flux de `file` dans un thread à part et l'enregistre
/// comme lecture en cours. Rend son numéro.
fn start_saved_read(reads: &SavedCrawlReads, file: std::fs::File) -> u32 {
    // Capacité 1 : le lecteur a au plus un message d'avance sur la fenêtre.
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<SavedChunk, String>>(1);
    std::thread::spawn(move || {
        let reader = std::io::BufReader::with_capacity(1 << 20, file);
        let end = crawl_core::savedcrawl::read_pages(reader, crawl_core::savedcrawl::MESSAGE_BYTES, |m| {
            tx.blocking_send(Ok(SavedChunk::Pages(m))).is_ok()
        });
        // Fenêtre partie (lecture abandonnée) : l'envoi échoue, rien à faire.
        let _ = tx.blocking_send(match end {
            Ok(end) => serde_json::to_string(&end).map(SavedChunk::End).map_err(|e| e.to_string()),
            Err(e) => Err(e),
        });
    });
    let id = reads.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    *reads.slot.lock().unwrap() = Some(SavedRead { id, rx });
    id
}

/// Message suivant de la lecture `read` : `Ok((json, fin))`.
async fn next_saved_chunk(reads: &SavedCrawlReads, read: u32) -> Result<(String, bool), String> {
    // Le récepteur est SORTI de l'emplacement pendant l'attente : une autre
    // lecture peut s'ouvrir entre-temps sans attendre celle-ci.
    let mut current = {
        let mut slot = reads.slot.lock().unwrap();
        match slot.take() {
            Some(r) if r.id == read => r,
            other => {
                *slot = other;
                return Err("lecture inconnue ou abandonnée".to_string());
            }
        }
    };
    match current.rx.recv().await {
        Some(Ok(SavedChunk::Pages(json))) => {
            let mut slot = reads.slot.lock().unwrap();
            // Une lecture plus récente a pris la place : celle-ci est abandonnée
            // (son récepteur, libéré ici, arrête son lecteur).
            if slot.is_none() {
                *slot = Some(current);
            }
            Ok((json, false))
        }
        Some(Ok(SavedChunk::End(json))) => Ok((json, true)),
        Some(Err(e)) => Err(e),
        None => Err(crawl_core::savedcrawl::INTERRUPTED.to_string()),
    }
}

/// Aperçu d'un crawl enregistré, pour le dialogue « Nouveau crawl / Revoir
/// l'enregistrement » : `{seed_url, date, count}` (nombre de pages), ou `None`
/// si aucun enregistrement n'est lisible pour cette adresse — comme avant, un
/// fichier absent ou illisible vaut « pas d'enregistrement ».
#[tauri::command]
pub async fn saved_crawl_info(
    app: AppHandle,
    id: String,
    dir: Option<String>,
) -> Result<Option<crawl_core::savedcrawl::SavedCrawlInfo>, String> {
    let path = crawls_dir(&app, &dir)?.join(format!("{}.json", sanitize_id(&id)));
    tauri::async_runtime::spawn_blocking(move || {
        let file = std::fs::File::open(&path).ok()?;
        crawl_core::savedcrawl::read_info(std::io::BufReader::with_capacity(1 << 20, file)).ok()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Ouvre la lecture en flux du crawl enregistré pour `id` ; rend son numéro, ou
/// `None` s'il n'y a pas d'enregistrement.
#[tauri::command]
pub fn saved_crawl_open(
    app: AppHandle,
    reads: State<'_, SavedCrawlReads>,
    id: String,
    dir: Option<String>,
) -> Result<Option<u32>, String> {
    let path = crawls_dir(&app, &dir)?.join(format!("{}.json", sanitize_id(&id)));
    Ok(std::fs::File::open(&path).ok().map(|file| start_saved_read(&reads, file)))
}

/// Message suivant d'une lecture : `{"pages":[…]}`, puis `{"done":true,
/// "seed_url":…,"date":…,"sitemap_urls":…,"count":<nombre de pages>}`. Erreur si le
/// fichier n'est pas un crawl (la fenêtre jette alors ce qu'elle a reçu).
/// Rendu tel quel, en JSON : la fenêtre reçoit des objets, sans double lecture.
#[tauri::command]
pub async fn saved_crawl_next(reads: State<'_, SavedCrawlReads>, read: u32) -> Result<tauri::ipc::Response, String> {
    next_saved_chunk(&reads, read).await.map(|(json, _)| tauri::ipc::Response::new(json))
}

/// Abandonne la lecture `read` (son lecteur s'arrête, le fichier se ferme).
#[tauri::command]
pub fn saved_crawl_close(reads: State<'_, SavedCrawlReads>, read: u32) {
    let mut slot = reads.slot.lock().unwrap();
    if slot.as_ref().map(|r| r.id) == Some(read) {
        *slot = None;
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

/// Ouvre une URL dans le NAVIGATEUR PAR DEFAUT du système.
///
/// POURQUOI une commande côté Rust : dans une WebView, un `<a target="_blank">`
/// ne fait rien — il n'y a pas d'onglet ou aller. Seul le système peut ouvrir le
/// navigateur, et seul le backend peut le lui demander.
///
/// SÉCURITÉ — le schéma est VÉRIFIÉ avant tout appel système. Ces URLs viennent
/// de pages crawlées, donc de sources non maitrisees : sans ce filtre, un lien
/// `file:///C:/...` ouvrirait un fichier local et un schema propriétaire
/// (`ms-...`) declencherait le handler correspondant. Seuls `http` et `https`
/// passent.
///
/// Aucun shell n'est invoque (pas de `cmd /c start`) : l'URL est passee comme
/// ARGUMENT au programme, jamais interprétée comme une ligne de commande, ce qui
/// écarte toute injection via `&`, `|` ou `^`.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(url.trim())
        .map_err(|e| format!("URL invalide : {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("schema refuse : {other}")),
    }
    // On repasse par la forme normalisée par le parseur, jamais par la chaîne
    // brute reçue du frontend.
    let target = parsed.as_str();

    #[cfg(windows)]
    {
        // `explorer <url>` delegue au navigateur par defaut. Il rend un code de
        // sortie non nul même en cas de succès : on ne l'attend pas (`spawn`).
        std::process::Command::new("explorer").arg(target).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(target).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ouvre un selecteur de DOSSIER natif. Renvoie le chemin choisi, ou None si annule.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let path = folder
            .and_then(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().to_string());
        let _ = tx.send(path);
    });
    rx.await.map_err(|e| e.to_string())
}

/// Libellé d'un filtre de dialogue natif : celui que l'interface envoie, déjà
/// traduit dans sa langue, sinon le libellé français historique (appel sans
/// argument, ou interface d'avant la traduction).
fn dialog_label(given: Option<String>, default_fr: &str) -> String {
    match given {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => default_fr.to_string(),
    }
}

/// Ouvre un selecteur de FICHIER (.json) et OUVRE LA LECTURE EN FLUX du projet
/// choisi (cf. `saved_crawl_next`) : rend son numéro, ou None si l'utilisateur
/// annule ou si le fichier ne s'ouvre pas (comme avant : rien ne se passe).
/// Il rendait autrefois le CONTENU du fichier, d'un bloc (cf. `saved_crawl_info`).
///
/// `filter_name` (facultatif, `filterName` côté JS) : libellé du filtre dans la
/// langue de l'interface (clé `engine.dialog_project`) ; « Projet de crawl » sinon.
#[tauri::command]
pub async fn pick_open_project(
    app: AppHandle,
    reads: State<'_, SavedCrawlReads>,
    filter_name: Option<String>,
) -> Result<Option<u32>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(dialog_label(filter_name, "Projet de crawl"), &["json"])
        .pick_file(move |file| {
            let _ = tx.send(file.and_then(|f| f.into_path().ok()));
        });
    let path = rx.await.map_err(|e| e.to_string())?;
    Ok(path
        .and_then(|p| std::fs::File::open(p).ok())
        .map(|file| start_saved_read(&reads, file)))
}

/// Ouvre un selecteur de fichier (.txt / .csv) et renvoie la LISTE DES URLs
/// qu'il contient — une par ligne. `None` si l'utilisateur annule.
///
/// Le parsing est fait côté Rust pour que le frontend recoive une liste déjà
/// propre et dedoublonnee :
///   - lignes vides et commentaires (`#`) ignores ;
///   - première colonne seulement si le fichier est un CSV (`,` ou `;`), ce qui
///     permet de deposer un export tel quel sans le nettoyer à la main ;
///   - `https://` ajoute quand le schema manqué (`example.com` est accepte) ;
///   - fragment retire et URL normalisée via le crate `url`, comme la seed d'un
///     crawl normal — sinon deux lignes designant la même ressource lanceraient
///     deux crawls complets ;
///   - doublons ecartes en conservant l'ordre du fichier.
///
/// `filter_name` / `all_files_name` (facultatifs, `filterName` / `allFilesName`
/// côté JS) : libellés des deux filtres dans la langue de l'interface (clés
/// `engine.dialog_url_list` et `engine.dialog_all_files`) ; « Liste d'URLs » et
/// « Tous les fichiers » sinon.
#[tauri::command]
pub async fn pick_url_list(
    app: AppHandle,
    filter_name: Option<String>,
    all_files_name: Option<String>,
) -> Result<Option<Vec<String>>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(dialog_label(filter_name, "Liste d'URLs"), &["txt", "csv", "list"])
        .add_filter(dialog_label(all_files_name, "Tous les fichiers"), &["*"])
        .pick_file(move |file| {
            let content = file
                .and_then(|f| f.into_path().ok())
                .and_then(|p| std::fs::read_to_string(p).ok());
            let _ = tx.send(content);
        });

    let content = match rx.await.map_err(|e| e.to_string())? {
        Some(c) => c,
        None => return Ok(None),
    };
    Ok(Some(parse_url_list(&content)))
}

/// Extrait les URLs d'un texte brut (une par ligne). Cf. [`pick_url_list`].
fn parse_url_list(content: &str) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();

    for raw in content.lines() {
        // Retire un BOM eventuel en tête de fichier (export Windows/Excel).
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // CSV : on ne garde que la première colonne.
        let first = line
            .split([',', ';'])
            .next()
            .unwrap_or(line)
            .trim()
            .trim_matches('"')
            .trim();
        if first.is_empty() {
            continue;
        }
        // Schema implicite : "example.com" -> "https://example.com".
        let candidate = if first.contains("://") {
            first.to_string()
        } else {
            format!("https://{first}")
        };
        // Normalisation identique a celle d'une seed (fragment retire).
        let normalized = match url::Url::parse(&candidate) {
            Ok(mut u) if matches!(u.scheme(), "http" | "https") && u.host().is_some() => {
                // Un hote SANS POINT n'est pas un domaine public : c'est presque
                // toujours un en-tête de colonne CSV ("URL", "Site", "Domaine"),
                // que le complement de schema transforme en `https://url/` —
                // une URL parfaitement valide, et un crawl fantome garanti.
                if !u.host_str().unwrap_or("").contains('.') {
                    continue;
                }
                u.set_fragment(None);
                let s: String = u.into();
                s
            }
            // Ligne inexploitable (texte libre, en-tête de colonne CSV...) :
            // on l'ignore plutot que de lancer un crawl voue à l'échec.
            _ => continue,
        };
        if seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{dialog_label, load_history_from, parse_url_list, slim_history};

    /// Sans libellé (ancienne interface, ou appel sans argument), le dialogue
    /// garde ses libellés français historiques ; sinon il prend celui reçu.
    #[test]
    fn libelles_de_dialogue_traduits_ou_historiques() {
        assert_eq!(dialog_label(None, "Projet de crawl"), "Projet de crawl");
        assert_eq!(dialog_label(Some("   ".to_string()), "Liste d'URLs"), "Liste d'URLs");
        assert_eq!(dialog_label(Some("Crawl project".to_string()), "Projet de crawl"), "Crawl project");
        assert_eq!(dialog_label(Some(" All files ".to_string()), "Tous les fichiers"), "All files");
    }

    /// Dossier temporaire vide, propre à un test (les tests tournent en parallèle).
    fn dossier_de_test(nom: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("seo-spider-test-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- Point 9 (14/11/2026) : relire un crawl enregistré en flux -----------

    use super::{next_saved_chunk, start_saved_read, SavedCrawlReads};

    /// Un crawl enregistré de `n` pages, tel que l'interface l'écrit.
    fn crawl_de_test(n: usize) -> String {
        let pages: Vec<String> = (0..n)
            .map(|i| format!(r#"{{"url":"https://a.fr/{i}/","status":200,"seo":{{"title":"Page « {i} » — été"}},"outlinks":[{{"target":"https://a.fr/","anchor":"{}"}}]}}"#, "x".repeat(i % 50)))
            .collect();
        format!(
            r#"{{"seed_url":"https://a.fr/","date":"2026-11-14T08:00:00.000Z","pages":[{}],"sitemap_urls":["https://a.fr/"]}}"#,
            pages.join(",")
        )
    }

    /// Tire tous les messages d'une lecture, comme la fenêtre.
    async fn tout_lire(reads: &SavedCrawlReads, id: u32) -> Result<(Vec<serde_json::Value>, serde_json::Value), String> {
        let mut pages = Vec::new();
        loop {
            let (json, fin) = next_saved_chunk(reads, id).await?;
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            if fin {
                return Ok((pages, v));
            }
            pages.extend(v["pages"].as_array().unwrap().iter().cloned());
        }
    }

    /// La fenêtre reçoit, message par message, EXACTEMENT les pages du fichier
    /// (mêmes valeurs que `JSON.parse` du fichier entier), puis le reste.
    #[tokio::test]
    async fn lecture_en_flux_rend_les_pages_du_fichier() {
        let dir = dossier_de_test("lecture-en-flux");
        let chemin = dir.join("crawl.json");
        // ~25 000 pages de ~180 octets : plusieurs messages de 4 Mo.
        let texte = crawl_de_test(25_000);
        std::fs::write(&chemin, &texte).unwrap();
        let reads = SavedCrawlReads::default();
        let id = start_saved_read(&reads, std::fs::File::open(&chemin).unwrap());
        let (pages, fin) = tout_lire(&reads, id).await.unwrap();
        let attendu: serde_json::Value = serde_json::from_str(&texte).unwrap();
        assert_eq!(serde_json::Value::Array(pages), attendu["pages"]);
        assert_eq!(fin["done"], true);
        assert_eq!(fin["count"], 25_000);
        assert_eq!(fin["seed_url"], attendu["seed_url"]);
        assert_eq!(fin["date"], attendu["date"]);
        assert_eq!(fin["sitemap_urls"], attendu["sitemap_urls"]);
        // Lecture terminée : elle n'est plus connue.
        assert!(next_saved_chunk(&reads, id).await.is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Un fichier qui n'est pas un crawl : erreur (la fenêtre jette tout).
    #[tokio::test]
    async fn lecture_en_flux_d_un_fichier_invalide() {
        let dir = dossier_de_test("lecture-invalide");
        let complet = crawl_de_test(30);
        // Fin du fichier coupée (ses derniers caractères sont en ASCII).
        let tronque = &complet[..complet.len() - 5];
        for (nom, texte) in [("tronque", tronque), ("pas-un-crawl", "[1,2,3]"), ("vide", "")] {
            let chemin = dir.join(format!("{nom}.json"));
            std::fs::write(&chemin, texte).unwrap();
            let reads = SavedCrawlReads::default();
            let id = start_saved_read(&reads, std::fs::File::open(&chemin).unwrap());
            assert!(tout_lire(&reads, id).await.is_err(), "{nom} accepté");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Ouvrir une lecture en abandonne une autre : l'ancienne n'est plus servie,
    /// la nouvelle va jusqu'au bout.
    #[tokio::test]
    async fn une_nouvelle_lecture_abandonne_la_precedente() {
        let dir = dossier_de_test("lecture-abandonnee");
        let chemin = dir.join("crawl.json");
        std::fs::write(&chemin, crawl_de_test(25_000)).unwrap();
        let reads = SavedCrawlReads::default();
        let premiere = start_saved_read(&reads, std::fs::File::open(&chemin).unwrap());
        let (_, fin) = next_saved_chunk(&reads, premiere).await.unwrap();
        assert!(!fin);
        let seconde = start_saved_read(&reads, std::fs::File::open(&chemin).unwrap());
        assert!(next_saved_chunk(&reads, premiere).await.is_err());
        let (pages, fin) = tout_lire(&reads, seconde).await.unwrap();
        assert_eq!(pages.len(), 25_000);
        assert_eq!(fin["count"], 25_000);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // --- Point 36 (14/11/2026) : enregistrer un crawl en morceaux -------------

    use super::save_part_bytes;

    /// Un morceau arrive en octets bruts (protocole ipc://) ou, si Tauri est
    /// passé en repli postMessage, en tableau de nombres : mêmes octets.
    #[test]
    fn morceau_d_enregistrement_par_les_deux_chemins_de_tauri() {
        use tauri::ipc::InvokeBody;
        let texte = r#"{"seed_url":"https://a.fr/","pages":[{"title":"Été « 2026 » 🕷"}"#;
        assert_eq!(save_part_bytes(&InvokeBody::Raw(texte.as_bytes().to_vec())).unwrap(), texte.as_bytes());
        let nombres = serde_json::Value::Array(texte.bytes().map(serde_json::Value::from).collect());
        assert_eq!(save_part_bytes(&InvokeBody::Json(nombres)).unwrap(), texte.as_bytes());
        // Ce qui n'est pas un morceau en octets est refusé (rien n'est écrit).
        assert!(save_part_bytes(&InvokeBody::Json(serde_json::json!([123, 256]))).is_err());
        assert!(save_part_bytes(&InvokeBody::Json(serde_json::json!([123, -1]))).is_err());
        assert!(save_part_bytes(&InvokeBody::Json(serde_json::json!({ "json": "{}" }))).is_err());
    }

    // --- Export CSV écrit par le moteur (14/11/2026) --------------------------

    use super::{export_stem, ExportSessions};

    /// Le nom reçu de la fenêtre ne peut jamais désigner un autre dossier ni
    /// porter une autre extension : `export_begin` ajoute « .csv » à ce radical.
    #[test]
    fn nom_d_export_jamais_un_chemin() {
        assert_eq!(export_stem("crawl-internal-2026-11-14-21-43-29.csv"), "crawl-internal-2026-11-14-21-43-29");
        assert_eq!(export_stem("  Rapport été.CSV "), "Rapport été");
        assert_eq!(export_stem("..\\..\\Windows\\win.ini"), "_.._Windows_win.ini");
        assert_eq!(export_stem("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(export_stem("C:\\a<b>c|d?e*f\"g\n.csv"), "C__a_b_c_d_e_f_g_");
        assert_eq!(export_stem(".."), "export");
        assert_eq!(export_stem(".csv"), "export");
        assert_eq!(export_stem(""), "export");
        assert_eq!(export_stem(&"é".repeat(300)).chars().count(), 80);
    }

    /// Un export : les morceaux à la suite, le chemin rendu à la fin ; un fichier
    /// existant n'est jamais remplacé (le suivant s'appelle « nom (2).csv »).
    #[test]
    fn export_ecrit_sans_jamais_remplacer() {
        let dir = dossier_de_test("export-ecrit");
        let exports = ExportSessions::default();
        let entete = "\u{feff}Adresse;Titre\r\n".as_bytes();
        let ligne = "https://a.fr/;\"Été ; hiver\"".as_bytes();
        let (milieu, total) = (entete.len() as u64, (entete.len() + ligne.len()) as u64);
        let a = exports.begin(&dir, "crawl-test.csv").unwrap();
        exports.write(a, 0, entete).unwrap();
        exports.write(a, milieu, ligne).unwrap();
        // La répétition du dernier morceau (repli de Tauri) est ignorée.
        exports.write(a, milieu, ligne).unwrap();
        let chemin = exports.end(a, total).unwrap();
        assert_eq!(chemin, dir.join("crawl-test.csv"));
        assert_eq!(std::fs::read(&chemin).unwrap(), [entete, ligne].concat());
        // Même nom une seconde fois : un autre fichier, le premier reste intact.
        let b = exports.begin(&dir, "crawl-test.csv").unwrap();
        exports.write(b, 0, b"x").unwrap();
        assert_eq!(exports.end(b, 1).unwrap(), dir.join("crawl-test (2).csv"));
        assert_eq!(std::fs::read(&chemin).unwrap().len() as u64, total);
        // Un export terminé ne reçoit plus rien.
        assert!(exports.write(a, total, b"y").is_err());
        assert!(exports.end(a, total).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Morceau hors position, total faux, abandon, fermeture de l'application :
    /// le fichier incomplet est retiré, il n'en reste jamais un morceau.
    #[test]
    fn export_incomplet_ne_laisse_aucun_fichier() {
        let dir = dossier_de_test("export-incomplet");
        let exports = ExportSessions::default();
        let restants = || std::fs::read_dir(&dir).unwrap().count();
        let a = exports.begin(&dir, "a").unwrap();
        exports.write(a, 0, b"abc").unwrap();
        assert!(exports.write(a, 7, b"def").is_err());   // un morceau a été perdu
        assert!(exports.write(a, 3, b"def").is_err());   // l'export n'existe plus
        assert_eq!(restants(), 0);
        let b = exports.begin(&dir, "b").unwrap();
        exports.write(b, 0, b"abc").unwrap();
        assert!(exports.end(b, 5).is_err());             // la fenêtre en a envoyé 5
        assert_eq!(restants(), 0);
        let c = exports.begin(&dir, "c").unwrap();
        exports.write(c, 0, b"abc").unwrap();
        exports.abort(c);
        exports.abort(c);                                // sans effet la seconde fois
        assert_eq!(restants(), 0);
        let d = exports.begin(&dir, "d").unwrap();
        let e = exports.begin(&dir, "e").unwrap();
        assert!(d != e);
        assert_eq!(restants(), 2);
        exports.abort_all();
        assert_eq!(restants(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `pagesData` disparaît ; toutes les autres clés ressortent telles quelles.
    #[test]
    fn historique_allege_retire_les_pages() {
        let raw = br#"[
            {"date":"2026-11-13","url":"https://a.fr","pages":2,"config":{"rps":5},
             "pagesData":[{"url":"https://a.fr/","seo":{"h1":["x"]}},{"url":"https://a.fr/b"}]},
            {"url":"https://b.fr","pages":0}
        ]"#;
        let (lean, had_pages) = slim_history(raw).unwrap();
        assert!(had_pages);
        let value: serde_json::Value = serde_json::from_str(&lean).unwrap();
        assert_eq!(
            value,
            serde_json::json!([
                {"date":"2026-11-13","url":"https://a.fr","pages":2,"config":{"rps":5}},
                {"url":"https://b.fr","pages":0}
            ])
        );
    }

    /// Un historique déjà léger ressort identique et n'est pas signalé « à migrer ».
    #[test]
    fn historique_deja_leger_inchange() {
        let raw = br#"[{"url":"https://a.fr","seeds":["https://a.fr","https://b.fr"],"techs":["WordPress"]}]"#;
        let (lean, had_pages) = slim_history(raw).unwrap();
        assert!(!had_pages);
        let avant: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let apres: serde_json::Value = serde_json::from_str(&lean).unwrap();
        assert_eq!(avant, apres);
    }

    /// Fichier tronqué, ou qui n'est pas une liste d'entrées : une erreur, pas de panique.
    #[test]
    fn historique_illisible_est_une_erreur() {
        assert!(slim_history(br#"[{"url":"https://a.fr","pagesData":[{"url":"#).is_err());
        assert!(slim_history(b"{}").is_err());
        assert!(slim_history(b"").is_err());
    }

    /// Migration de bout en bout, sur de vrais fichiers : le fichier est réécrit
    /// allégé, l'original est conservé octet pour octet, et une copie existante
    /// n'est jamais écrasée.
    #[test]
    fn migration_conserve_l_original() {
        let dir = dossier_de_test("migration");
        let path = dir.join("history.json");
        let copie = dir.join("history.avant-allegement.json");
        let lourd = r#"[{"url":"https://a.fr","pagesData":[{"url":"https://a.fr/"}]}]"#;
        std::fs::write(&path, lourd).unwrap();

        let lean = load_history_from(&path);
        assert_eq!(lean, r#"[{"url":"https://a.fr"}]"#);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), lean);
        assert_eq!(std::fs::read_to_string(&copie).unwrap(), lourd);

        // Deuxième lecture : plus rien à migrer, donc pas de nouvelle copie.
        assert_eq!(load_history_from(&path), lean);
        assert!(!dir.join("history.avant-allegement-2.json").exists());

        // Un historique lourd qui réapparaît part dans une copie NUMÉROTÉE.
        let lourd_2 = r#"[{"url":"https://b.fr","pagesData":[]}]"#;
        std::fs::write(&path, lourd_2).unwrap();
        assert_eq!(load_history_from(&path), r#"[{"url":"https://b.fr"}]"#);
        assert_eq!(std::fs::read_to_string(&copie).unwrap(), lourd);
        assert_eq!(
            std::fs::read_to_string(dir.join("history.avant-allegement-2.json")).unwrap(),
            lourd_2
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Fichier absent ou illisible : liste vide, et le fichier n'est pas touché.
    #[test]
    fn historique_absent_ou_illisible_donne_une_liste_vide() {
        let dir = dossier_de_test("illisible");
        let path = dir.join("history.json");
        assert_eq!(load_history_from(&path), "[]");

        let casse = r#"[{"url":"https://a.fr","pagesData":[{"#;
        std::fs::write(&path, casse).unwrap();
        assert_eq!(load_history_from(&path), "[]");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), casse);
        assert!(!dir.join("history.avant-allegement.json").exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_liste_simple() {
        let txt = "https://a.fr/\nhttps://b.fr/\n";
        assert_eq!(parse_url_list(txt), vec!["https://a.fr/", "https://b.fr/"]);
    }

    /// Lignes vides, commentaires, espaces parasites et BOM : ignores.
    #[test]
    fn parse_ignore_le_bruit() {
        let txt = "\u{feff}# mes sites\n\n  https://a.fr/  \n\n# commentaire\nhttps://b.fr/\n";
        assert_eq!(parse_url_list(txt), vec!["https://a.fr/", "https://b.fr/"]);
    }

    /// CSV : seule la première colonne est lue, guillemets retires.
    #[test]
    fn parse_csv_premiere_colonne() {
        let txt = "\"https://a.fr/\",Lyon,42\nhttps://b.fr/;Nice;7\n";
        assert_eq!(parse_url_list(txt), vec!["https://a.fr/", "https://b.fr/"]);
    }

    /// Schema manquant -> https ; doublons et fragments -> une seule entrée.
    #[test]
    fn parse_complete_et_dedoublonne() {
        let txt = "example.com\nhttps://example.com/\nhttps://example.com/#top\nhttps://b.fr/\n";
        assert_eq!(parse_url_list(txt), vec!["https://example.com/", "https://b.fr/"]);
    }

    /// Lignes inexploitables : ecartees sans faire echouer l'import.
    #[test]
    fn parse_ecarte_les_lignes_invalides() {
        let txt = "URL;Ville\nftp://a.fr/\nhttps://ok.fr/\n";
        assert_eq!(parse_url_list(txt), vec!["https://ok.fr/"]);
    }
}
