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
    config: CrawlConfig,
) -> Result<(), String> {
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

    tauri::async_runtime::spawn(async move {
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

        // Closure page -> événement "crawl:page".
        let on_page = move |page: PageReport| {
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

/// Enregistre un crawl complet (JSON) sous `crawls/<id>.json`. Renvoie le chemin du fichier.
#[tauri::command]
pub fn save_crawl(app: AppHandle, id: String, json: String, dir: Option<String>) -> Result<String, String> {
    let d = crawls_dir(&app, &dir)?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let path = d.join(format!("{}.json", sanitize_id(&id)));
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Charge un crawl enregistré par son identifiant (URL de départ). Renvoie "" si absent.
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

/// Ouvre un selecteur de FICHIER (.json) et renvoie le CONTENU du projet choisi,
/// ou None si annule. Le frontend parse ce JSON et l'affiche (consultation).
///
/// `filter_name` (facultatif, `filterName` côté JS) : libellé du filtre dans la
/// langue de l'interface (clé `engine.dialog_project`) ; « Projet de crawl » sinon.
#[tauri::command]
pub async fn pick_open_project(app: AppHandle, filter_name: Option<String>) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(dialog_label(filter_name, "Projet de crawl"), &["json"])
        .pick_file(move |file| {
            let content = file
                .and_then(|f| f.into_path().ok())
                .and_then(|p| std::fs::read_to_string(p).ok());
            let _ = tx.send(content);
        });
    rx.await.map_err(|e| e.to_string())
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
