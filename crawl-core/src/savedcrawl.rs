//! Lecture EN FLUX d'un crawl enregistré (`crawls/<site>.json`).
//!
//! Le fichier est écrit par l'interface (`saveCurrentCrawl` dans `main.js`) :
//! `JSON.stringify({ seed_url, date, pages, sitemap_urls })`.
//!
//! POURQUOI en flux. Jusqu'au 14/11/2026, le moteur lisait le fichier en entier
//! et le renvoyait d'un bloc à la fenêtre, comme UNE chaîne : Tauri la
//! ré-échappe en JSON (le fichier de 415 Mo de Chris devient 463 Mo, soit 460
//! millions de caractères), la fenêtre la décode, puis la relit avec
//! `JSON.parse`. La fenêtre tenait donc, en même temps, la réponse, le texte du
//! fichier et toutes les pages reconstruites ; et un fichier de plus de ~480 Mo
//! ne pouvait plus s'ouvrir du tout : la réponse dépassait la longueur maximale
//! d'une chaîne JavaScript (536 870 888 caractères), soit ~40 000 pages.
//!
//! Ici, le fichier est lu morceau par morceau et ses pages sont remises par
//! MESSAGES de quelques Mo (`{"pages":[…]}`), le texte de chaque page restant
//! EXACTEMENT celui du fichier (`RawValue`) : la fenêtre reçoit les mêmes
//! valeurs que si elle avait relu le fichier entier. Le moteur ne tient jamais
//! plus d'un message à la fois (mesures : `tests/memoire_ouverture.rs`).
//!
//! Le fichier n'est jamais modifié : ce module ne fait que lire.

use std::io::Read;

use serde::de::{self, DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Serialize;
use serde_json::value::RawValue;

/// Taille visée d'un message de pages, en octets de JSON. Un message part dès
/// qu'il atteint cette taille ; une page n'est jamais coupée.
pub const MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Ce qu'il faut savoir d'un crawl enregistré AVANT de l'ouvrir (le dialogue
/// « Nouveau crawl / Revoir l'enregistrement ») : sans construire ses pages.
///
/// `seed_url` et `date` sont repris TELS QU'ÉCRITS dans le fichier (`null`
/// s'ils manquent), comme l'interface les lisait auparavant après `JSON.parse`.
#[derive(Debug, Serialize)]
pub struct SavedCrawlInfo {
    pub seed_url: Option<Box<RawValue>>,
    pub date: Option<Box<RawValue>>,
    /// Nombre de pages enregistrées.
    pub count: usize,
}

/// Dernier message d'une lecture : tout ce qui n'est pas une page.
#[derive(Debug, Serialize)]
pub struct SavedCrawlEnd {
    /// Toujours `true` : distingue ce message de ceux qui portent des pages.
    pub done: bool,
    pub seed_url: Option<Box<RawValue>>,
    pub date: Option<Box<RawValue>>,
    pub sitemap_urls: Option<Box<RawValue>>,
    /// Nombre de pages remises dans les messages précédents.
    pub count: usize,
}

/// Message d'erreur quand la lecture s'arrête parce que le destinataire ne veut
/// plus de pages (fenêtre passée à autre chose).
pub const INTERRUPTED: &str = "lecture interrompue";

// ---------------------------------------------------------------------------
// Lecture des pages, en messages
// ---------------------------------------------------------------------------

struct Stream<'f, F: FnMut(String) -> bool> {
    on_message: &'f mut F,
    message_bytes: usize,
    /// Message en cours : `{"pages":[` puis les pages, séparées par des virgules.
    buffer: String,
    in_buffer: usize,
    pages: usize,
    pages_seen: bool,
    seed_url: Option<Box<RawValue>>,
    date: Option<Box<RawValue>>,
    sitemap_urls: Option<Box<RawValue>>,
    interrupted: bool,
}

const MESSAGE_HEAD: &str = "{\"pages\":[";

impl<F: FnMut(String) -> bool> Stream<'_, F> {
    fn push(&mut self, page: &RawValue) {
        if self.in_buffer > 0 {
            self.buffer.push(',');
        }
        self.buffer.push_str(page.get());
        self.in_buffer += 1;
        self.pages += 1;
        if self.buffer.len() >= self.message_bytes {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.in_buffer == 0 || self.interrupted {
            return;
        }
        let mut message = std::mem::replace(&mut self.buffer, String::with_capacity(self.message_bytes + 64 * 1024));
        self.buffer.push_str(MESSAGE_HEAD);
        message.push_str("]}");
        self.in_buffer = 0;
        if !(self.on_message)(message) {
            self.interrupted = true;
        }
    }
}

impl<'de, F: FnMut(String) -> bool> Visitor<'de> for &mut Stream<'_, F> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("un crawl enregistré (objet JSON)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "pages" => {
                    // `JSON.parse` garderait la DERNIÈRE liste : on refuse plutôt
                    // que d'afficher deux listes mélangées. (L'interface n'écrit
                    // jamais deux fois la même clé.)
                    if self.pages_seen {
                        return Err(de::Error::custom("deux listes de pages"));
                    }
                    self.pages_seen = true;
                    map.next_value_seed(Pages(&mut *self))?;
                }
                "seed_url" => self.seed_url = Some(map.next_value()?),
                "date" => self.date = Some(map.next_value()?),
                "sitemap_urls" => self.sitemap_urls = Some(map.next_value()?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

struct Pages<'s, 'f, F: FnMut(String) -> bool>(&'s mut Stream<'f, F>);

impl<'de, F: FnMut(String) -> bool> DeserializeSeed<'de> for Pages<'_, '_, F> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, F: FnMut(String) -> bool> Visitor<'de> for Pages<'_, '_, F> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("une liste de pages")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(page) = seq.next_element::<Box<RawValue>>()? {
            self.0.push(&page);
            if self.0.interrupted {
                return Err(de::Error::custom(INTERRUPTED));
            }
        }
        // Fin de la liste : le dernier message part sans attendre le reste du
        // fichier (les adresses du sitemap suivent).
        self.0.flush();
        if self.0.interrupted {
            return Err(de::Error::custom(INTERRUPTED));
        }
        Ok(())
    }
}

/// Lit un crawl enregistré depuis `reader` et en remet les pages par messages
/// JSON `{"pages":[…]}` d'environ `message_bytes` octets, puis rend le reste
/// (adresse de départ, date, sitemap, nombre de pages).
///
/// `on_message` reçoit chaque message ; s'il rend `false`, la lecture s'arrête
/// (erreur [`INTERRUPTED`]). Erreur aussi si le fichier n'est pas un crawl :
/// JSON invalide, pas un objet, pas de liste `pages`, texte après l'objet. Un
/// lecteur peut alors avoir reçu des messages : à lui de les jeter (l'interface
/// n'affiche le crawl qu'une fois le dernier message reçu).
pub fn read_pages<R: Read>(
    reader: R,
    message_bytes: usize,
    mut on_message: impl FnMut(String) -> bool,
) -> Result<SavedCrawlEnd, String> {
    let mut stream = Stream {
        on_message: &mut on_message,
        message_bytes: message_bytes.max(1),
        buffer: String::with_capacity(message_bytes.max(1) + 64 * 1024),
        in_buffer: 0,
        pages: 0,
        pages_seen: false,
        seed_url: None,
        date: None,
        sitemap_urls: None,
        interrupted: false,
    };
    stream.buffer.push_str(MESSAGE_HEAD);
    let mut de = serde_json::Deserializer::from_reader(reader);
    let lu = de.deserialize_map(&mut stream).and_then(|_| de.end());
    if stream.interrupted {
        return Err(INTERRUPTED.to_string());
    }
    lu.map_err(|e| e.to_string())?;
    if !stream.pages_seen {
        return Err("pas de liste de pages".to_string());
    }
    Ok(SavedCrawlEnd {
        done: true,
        seed_url: stream.seed_url,
        date: stream.date,
        sitemap_urls: stream.sitemap_urls,
        count: stream.pages,
    })
}

// ---------------------------------------------------------------------------
// Aperçu (dialogue « Nouveau / Revoir »)
// ---------------------------------------------------------------------------

struct Info {
    seed_url: Option<Box<RawValue>>,
    date: Option<Box<RawValue>>,
    pages: Option<usize>,
}

impl<'de> Visitor<'de> for &mut Info {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("un crawl enregistré (objet JSON)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "pages" => {
                    if self.pages.is_some() {
                        return Err(de::Error::custom("deux listes de pages"));
                    }
                    self.pages = Some(map.next_value_seed(CountPages)?);
                }
                "seed_url" => self.seed_url = Some(map.next_value()?),
                "date" => self.date = Some(map.next_value()?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

struct CountPages;

impl<'de> DeserializeSeed<'de> for CountPages {
    type Value = usize;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<usize, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for CountPages {
    type Value = usize;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("une liste de pages")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<usize, A::Error> {
        let mut n = 0;
        while seq.next_element::<IgnoredAny>()?.is_some() {
            n += 1;
        }
        Ok(n)
    }
}

/// Aperçu d'un crawl enregistré : adresse, date et NOMBRE de pages, sans
/// construire ni garder aucune page (le fichier est parcouru, pas chargé).
/// Mêmes refus que [`read_pages`].
pub fn read_info<R: Read>(reader: R) -> Result<SavedCrawlInfo, String> {
    let mut info = Info { seed_url: None, date: None, pages: None };
    let mut de = serde_json::Deserializer::from_reader(reader);
    de.deserialize_map(&mut info).and_then(|_| de.end()).map_err(|e| e.to_string())?;
    let count = info.pages.ok_or_else(|| "pas de liste de pages".to_string())?;
    Ok(SavedCrawlInfo { seed_url: info.seed_url, date: info.date, count })
}

// ---------------------------------------------------------------------------
// Écriture EN MORCEAUX (point 36 du chantier « après les langues »)
// ---------------------------------------------------------------------------
//
// Jusqu'au 14/11/2026, la fenêtre fabriquait la chaîne du crawl ENTIER
// (`JSON.stringify`) et la remettait d'un bloc à `save_crawl` : au-delà de
// ~42 000 pages, cette chaîne (puis l'argument échappé) dépassait la longueur
// maximale d'une chaîne JavaScript, et rien n'était enregistré ; côté moteur,
// le corps de la requête, sa valeur JSON et la copie de l'argument tenaient
// ~1,7 Go pour un crawl de 415 Mo ; et `std::fs::write` écrivait directement
// le fichier final (un plantage en cours d'écriture le laissait tronqué).
//
// Désormais la fenêtre fabrique le MÊME texte, page par page, et le remet par
// morceaux (des octets, chacun avec sa position) ; le moteur les ajoute à un
// fichier TEMPORAIRE du même dossier, puis le renomme par-dessus l'ancien une
// fois tout reçu : l'ancien fichier reste intact jusqu'au bout. Un temporaire
// INCOMPLET (envoi interrompu, morceau en erreur) est retiré ; un temporaire
// COMPLET qui ne peut pas remplacer l'ancien fichier (tenu par un autre
// programme) est gardé à côté, jamais retiré (relecture du 14/11/2026, P1-1).
// Le moteur ne tient jamais qu'un morceau à la fois.

/// Erreur rendue aux morceaux d'un enregistrement REMPLACÉ par un plus récent
/// du même fichier (la fenêtre l'abandonne alors sans rien afficher).
pub const SAVE_SUPERSEDED: &str = "save_superseded";

/// Un temporaire orphelin (laissé par un plantage) plus ancien que ce délai est
/// retiré au prochain enregistrement du même site, s'il n'est ouvert nulle part
/// (`open_elsewhere`). Le délai protège l'enregistrement en cours d'une AUTRE
/// instance de l'application.
const ORPHAN_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Attentes entre deux essais du renommage final : un antivirus, un indexeur ou
/// un outil de synchronisation tient souvent le fichier du crawl quelques
/// centaines de millisecondes. 3,15 s au plus en tout.
const RENAME_WAITS_MS: [u64; 6] = [50, 100, 200, 400, 800, 1600];

/// L'échec d'un enregistrement. `kept` : le texte du crawl était COMPLET mais
/// n'a pas pu remplacer le fichier du site (renommage refusé, écriture forcée
/// sur le disque en échec) — le fichier où il a été gardé. Jamais retiré :
/// c'est alors le seul exemplaire du crawl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveError {
    pub reason: String,
    pub kept: Option<std::path::PathBuf>,
}

impl SaveError {
    fn new(reason: impl Into<String>) -> Self {
        SaveError { reason: reason.into(), kept: None }
    }
}

struct OpenSave {
    file: std::io::BufWriter<std::fs::File>,
    bytes: u64,
    /// Le dernier morceau écrit (position, octets) : sa répétition EXACTE est
    /// ignorée (repli postMessage de Tauri qui rejoue un appel déjà traité).
    last: Option<(u64, Vec<u8>)>,
}

struct SaveEntry {
    path: std::path::PathBuf,
    tmp: std::path::PathBuf,
    open: std::sync::Arc<std::sync::Mutex<Option<OpenSave>>>,
    /// `end()` en cours : plus aucun morceau accepté, mais l'enregistrement
    /// compte encore comme « en cours » (fermeture de la fenêtre différée).
    ending: bool,
}

#[derive(Default)]
struct SaveState {
    next_id: u32,
    active: std::collections::HashMap<u32, SaveEntry>,
    superseded: std::collections::HashSet<u32>,
    /// Pour chaque fichier, le numéro du plus récent enregistrement qui l'a
    /// RENOMMÉ : un enregistrement plus ancien ne renomme plus après lui.
    renamed: std::collections::HashMap<std::path::PathBuf, u32>,
    /// Enregistrements qui ont ÉCHOUÉ depuis le lancement (impossible à
    /// commencer, morceau en erreur, abandon par la fenêtre, fin en échec —
    /// crawl gardé à côté compris ; un enregistrement remplacé par un plus
    /// récent n'est pas un échec) : une fermeture différée n'a pas lieu si l'un
    /// d'eux a échoué depuis la demande (le message reste à l'écran).
    failures: u64,
}

/// Ce que la fermeture différée de la fenêtre a fini par décider (`wait_close`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    /// Tout est enregistré : fermer.
    Close,
    /// Un enregistrement a échoué depuis la demande : ne pas fermer, le message reste à l'écran.
    Stay,
    /// Délai de sûreté atteint : fermer quand même.
    Forced,
    /// La fermeture a été annulée entre-temps : rien à faire.
    Cancelled,
}

/// Les enregistrements en cours (plusieurs à la fois : un par site d'une liste).
/// Clonable à bas coût : les clones partagent le même état.
#[derive(Clone, Default)]
pub struct SaveSessions {
    state: std::sync::Arc<std::sync::Mutex<SaveState>>,
    /// Les renommages finaux, un à la fois : le contrôle « un plus récent a-t-il
    /// déjà renommé ? » et le renommage ne laissent passer personne entre eux.
    renaming: std::sync::Arc<std::sync::Mutex<()>>,
}

/// Nom du temporaire d'un enregistrement : `<fichier>.tmp-<processus>-<numéro>`.
fn tmp_prefix(path: &std::path::Path) -> Option<String> {
    path.file_name().map(|n| format!("{}.tmp-", n.to_string_lossy()))
}

fn discard(entry: SaveEntry) {
    // Fermer le fichier AVANT de le retirer (Windows refuserait sinon).
    drop(entry.open.lock().unwrap().take());
    let _ = std::fs::remove_file(&entry.tmp);
}

/// Un fichier encore ouvert par un AUTRE programme ou une autre instance de
/// l'application : sous Windows, l'ouvrir SANS aucun partage échoue alors.
#[cfg(windows)]
fn open_elsewhere(path: &std::path::Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().read(true).share_mode(0).open(path).is_err()
}

#[cfg(not(windows))]
fn open_elsewhere(_path: &std::path::Path) -> bool {
    false
}

/// `std::fs::rename`, réessayé (`RENAME_WAITS_MS`) ; aucun essai de plus si la
/// source n'existe plus.
fn rename_retrying(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    let mut waits = RENAME_WAITS_MS.iter();
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(e),
            Err(e) => match waits.next() {
                Some(ms) => std::thread::sleep(std::time::Duration::from_millis(*ms)),
                None => return Err(e),
            },
        }
    }
}

/// Date et heure UTC, au format compact d'ISO 8601 : `AAAAMMJJTHHMMSSZ`.
fn utc_stamp(t: std::time::SystemTime) -> String {
    let s = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (jours, sec) = ((s / 86_400) as i64, s % 86_400);
    // Jours depuis le 1er janvier 1970 → date civile (algorithme de H. Hinnant).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let jde = z.rem_euclid(146_097);
    let ade = (jde - jde / 1_460 + jde / 36_524 - jde / 146_096) / 365;
    let jda = jde - (365 * ade + ade / 4 - ade / 100);
    let mp = (5 * jda + 2) / 153;
    let jour = jda - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let an = ade + ere * 400 + i64::from(mois <= 2);
    format!("{an:04}{mois:02}{jour:02}T{:02}{:02}{:02}Z", sec / 3600, sec % 3600 / 60, sec % 60)
}

/// Où garder un crawl complet qui n'a pas pu remplacer `path` :
/// `<site>.<AAAAMMJJTHHMMSSZ>.json` à côté. « Ouvrir un projet » l'ouvre ; le
/// nettoyage des orphelins (`<fichier>.tmp-*` seulement) n'y touche pas. Le nom
/// est RÉSERVÉ (fichier vide créé sans pouvoir en remplacer un autre) : deux
/// crawls gardés dans la même seconde, même par deux instances de
/// l'application, ne prennent jamais le même nom.
fn kept_path(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "crawl".to_string());
    let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "json".to_string());
    let stamp = utc_stamp(std::time::SystemTime::now());
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    for n in 1u32..10_000 {
        let name = if n == 1 { format!("{stem}.{stamp}.{ext}") } else { format!("{stem}.{stamp}-{n}.{ext}") };
        let candidate = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "aucun nom libre"))
}

/// Range un temporaire COMPLET `tmp` sous un nom gardé (`kept_path`) : renommé
/// par-dessus le fichier réservé, ou copié s'il est lui-même tenu par un autre
/// programme. Rend le chemin gardé ; en cas d'échec, seul le fichier RÉSERVÉ
/// (le nôtre) est retiré, et `tmp` reste intact.
fn stash(path: &std::path::Path, tmp: &std::path::Path) -> Option<std::path::PathBuf> {
    let kept = kept_path(path).ok()?;
    if std::fs::rename(tmp, &kept).is_ok() {
        return Some(kept);
    }
    if std::fs::copy(tmp, &kept).is_ok() {
        let _ = std::fs::remove_file(tmp);
        return Some(kept);
    }
    let _ = std::fs::remove_file(&kept); // le nom réservé, vide ou copie partielle
    None
}

/// Le temporaire COMPLET `tmp` n'a pas pu devenir `path` : il est gardé sous un
/// nom gardé (`stash`) et, à défaut, reste où il est — le prochain
/// enregistrement du site le rangera (orphelin complet). Jamais retiré.
fn keep(path: &std::path::Path, tmp: &std::path::Path, reason: String) -> SaveError {
    let kept = stash(path, tmp).unwrap_or_else(|| tmp.to_path_buf());
    SaveError { reason, kept: Some(kept) }
}

/// Un temporaire COMPLET : il finit par les octets `]}` que la fenêtre écrit
/// toujours en dernier (fin de la liste du sitemap, puis de l'objet).
fn looks_complete(path: &std::path::Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let mut fin = [0u8; 2];
    std::fs::File::open(path)
        .and_then(|mut f| {
            f.seek(SeekFrom::End(-2))?;
            f.read_exact(&mut fin)
        })
        .is_ok()
        && &fin == b"]}"
}

impl SaveSessions {
    /// Commence l'enregistrement de `path` : crée le dossier au besoin et un
    /// fichier temporaire à côté. Un enregistrement du MÊME fichier dont les
    /// morceaux arrivent encore est remplacé (le plus récent gagne) ; celui qui
    /// est déjà dans son `end()` finit, mais ne renommera pas après un plus
    /// récent. Les temporaires orphelins de ce fichier (plantage) de plus de 10
    /// minutes, ouverts nulle part, sont rangés s'ils sont complets (cf.
    /// `begin_inner`), retirés sinon. Rend le numéro de l'enregistrement ; un
    /// échec compte parmi les échecs (`failures`).
    pub fn begin(&self, path: &std::path::Path) -> Result<u32, String> {
        let r = self.begin_inner(path);
        if r.is_err() {
            self.state.lock().unwrap().failures += 1;
        }
        r
    }

    fn begin_inner(&self, path: &std::path::Path) -> Result<u32, String> {
        let dir = path.parent().ok_or_else(|| "dossier introuvable".to_string())?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let prefix = tmp_prefix(path).ok_or_else(|| "nom de fichier introuvable".to_string())?;
        let mut st = self.state.lock().unwrap();
        // 1. Un enregistrement du même fichier encore en cours d'envoi : remplacé.
        let anciens: Vec<u32> = st.active.iter().filter(|(_, e)| e.path == path && !e.ending).map(|(k, _)| *k).collect();
        for k in anciens {
            if let Some(e) = st.active.remove(&k) {
                discard(e);
            }
            st.superseded.insert(k);
        }
        // 2. Temporaires orphelins de ce fichier : vieux ET ouverts nulle part
        //    (sous NTFS, la date d'un fichier qu'une autre instance écrit encore
        //    peut ne pas bouger dans le dossier avant sa fermeture). Un orphelin
        //    COMPLET peut être le dernier exemplaire d'un crawl (crawl gardé
        //    qu'on n'a pas pu renommer, sortie pendant une fin d'enregistrement) :
        //    il est RANGÉ sous un nom gardé, et retiré seulement s'il est plus
        //    ancien que le fichier du site (un enregistrement plus récent a
        //    réussi). Un orphelin incomplet ne vaut rien : retiré.
        let actifs: std::collections::HashSet<std::path::PathBuf> = st.active.values().map(|e| e.tmp.clone()).collect();
        let date_du_site = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if let Ok(entrees) = std::fs::read_dir(dir) {
            for e in entrees.flatten() {
                let nom = e.file_name().to_string_lossy().to_string();
                if !nom.starts_with(&prefix) || actifs.contains(&e.path()) {
                    continue;
                }
                let date = e.metadata().and_then(|m| m.modified()).ok();
                let vieux = date.and_then(|t| t.elapsed().ok()).is_some_and(|age| age > ORPHAN_AGE);
                if !vieux || open_elsewhere(&e.path()) {
                    continue;
                }
                let depasse = matches!((date, date_du_site), (Some(o), Some(s)) if o < s);
                if looks_complete(&e.path()) && !depasse {
                    let _ = stash(path, &e.path());
                } else {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        // 3. Le temporaire de cet enregistrement.
        st.next_id += 1;
        let id = st.next_id;
        let tmp = dir.join(format!("{prefix}{}-{id}", std::process::id()));
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        st.active.insert(
            id,
            SaveEntry {
                path: path.to_path_buf(),
                tmp,
                open: std::sync::Arc::new(std::sync::Mutex::new(Some(OpenSave {
                    file: std::io::BufWriter::with_capacity(1 << 20, file),
                    bytes: 0,
                    last: None,
                }))),
                ending: false,
            },
        );
        Ok(id)
    }

    /// Ajoute un morceau à l'enregistrement `id`. `offset` : sa position dans
    /// le fichier (les octets que la fenêtre a déjà envoyés). Un morceau hors
    /// position abandonne l'enregistrement (temporaire retiré, ancien fichier
    /// intact) ; la répétition exacte du dernier est ignorée. Une erreur
    /// d'écriture abandonne aussi l'enregistrement.
    pub fn write(&self, id: u32, offset: u64, bytes: Vec<u8>) -> Result<(), String> {
        use std::io::Write;
        let open = {
            let st = self.state.lock().unwrap();
            match st.active.get(&id) {
                Some(e) if e.ending => return Err("enregistrement déjà terminé".to_string()),
                Some(e) => std::sync::Arc::clone(&e.open),
                None if st.superseded.contains(&id) => return Err(SAVE_SUPERSEDED.to_string()),
                None => return Err("enregistrement inconnu".to_string()),
            }
        };
        let mut guard = open.lock().unwrap();
        let Some(o) = guard.as_mut() else {
            return Err(SAVE_SUPERSEDED.to_string());
        };
        if offset != o.bytes {
            if o.last.as_ref().is_some_and(|(at, dernier)| *at == offset && *dernier == bytes) {
                return Ok(());
            }
            let attendu = o.bytes;
            drop(guard);
            self.abort(id);
            return Err(format!("morceau hors position (octet {offset}, attendu {attendu})"));
        }
        o.last = None; // jamais deux morceaux gardés en mémoire
        match o.file.write_all(&bytes) {
            Ok(()) => {
                o.bytes += bytes.len() as u64;
                o.last = Some((offset, bytes));
                Ok(())
            }
            Err(e) => {
                drop(guard);
                self.abort(id);
                Err(e.to_string())
            }
        }
    }

    /// Termine l'enregistrement `id`. `total` : les octets que la fenêtre a
    /// envoyés en tout ; s'il diffère de ce qui a été écrit (morceau perdu ou
    /// rejoué), le temporaire est retiré et l'ancien fichier reste. Sinon :
    /// dernier vidage, écriture forcée sur le disque, puis RENOMMAGE par-dessus
    /// le fichier final (jamais à moitié écrit), réessayé ~3 s. S'il reste
    /// refusé (fichier tenu par un autre programme), ou si l'écriture forcée
    /// échoue, le crawl COMPLET est gardé à côté (`SaveError::kept`) et jamais
    /// retiré. Un enregistrement plus ancien que le dernier à avoir renommé ce
    /// fichier ne renomme pas (SAVE_SUPERSEDED). Rend le chemin du fichier.
    pub fn end(&self, id: u32, total: u64) -> Result<std::path::PathBuf, SaveError> {
        let prepared = self.end_prepare(id, total)?;
        self.end_commit(id, prepared)
    }

    /// 1re moitié de `end` : plus aucun morceau accepté (l'enregistrement
    /// reste « en cours »), total contrôlé, dernier vidage, écriture forcée sur
    /// le disque. Rend (fichier final, temporaire COMPLET).
    fn end_prepare(&self, id: u32, total: u64) -> Result<(std::path::PathBuf, std::path::PathBuf), SaveError> {
        let (path, tmp, open) = {
            let mut st = self.state.lock().unwrap();
            let remplace = st.superseded.contains(&id);
            match st.active.get_mut(&id) {
                Some(e) if e.ending => return Err(SaveError::new("enregistrement déjà terminé")),
                Some(e) => {
                    e.ending = true;
                    (e.path.clone(), e.tmp.clone(), std::sync::Arc::clone(&e.open))
                }
                None if remplace => return Err(SaveError::new(SAVE_SUPERSEDED)),
                None => return Err(SaveError::new("enregistrement inconnu")),
            }
        };
        let taken = open.lock().unwrap().take();
        let outcome = match taken {
            None => Err(SaveError::new(SAVE_SUPERSEDED)),
            // Un morceau perdu ou rejoué : ce temporaire ne vaut rien.
            Some(o) if o.bytes != total => {
                let recu = o.bytes;
                drop(o);
                let _ = std::fs::remove_file(&tmp);
                Err(SaveError::new(format!("{recu} octets reçus pour {total} envoyés")))
            }
            Some(o) => match o.file.into_inner() {
                // Dernier vidage en échec : le temporaire est INCOMPLET, retiré.
                Err(e) => {
                    let raison = e.error().to_string();
                    drop(e);
                    let _ = std::fs::remove_file(&tmp);
                    Err(SaveError::new(raison))
                }
                // Désormais le temporaire est COMPLET : il ne sera jamais retiré.
                Ok(file) => {
                    let synced = file.sync_all();
                    drop(file);
                    synced.map_err(|e| keep(&path, &tmp, e.to_string()))
                }
            },
        };
        match outcome {
            Ok(()) => Ok((path, tmp)),
            Err(e) => {
                let mut st = self.state.lock().unwrap();
                st.active.remove(&id);
                if e.reason != SAVE_SUPERSEDED {
                    st.failures += 1;
                }
                Err(e)
            }
        }
    }

    /// 2e moitié de `end` : renommer le temporaire complet par-dessus le
    /// fichier final, sauf si un enregistrement PLUS RÉCENT du même fichier l'a
    /// déjà fait (son crawl est plus à jour : le temporaire est alors retiré).
    fn end_commit(&self, id: u32, (path, tmp): (std::path::PathBuf, std::path::PathBuf)) -> Result<std::path::PathBuf, SaveError> {
        let renamed = {
            let _un_a_la_fois = self.renaming.lock().unwrap();
            if self.state.lock().unwrap().renamed.get(&path).is_some_and(|&r| r > id) {
                None
            } else {
                let r = rename_retrying(&tmp, &path);
                if r.is_ok() {
                    self.state.lock().unwrap().renamed.insert(path.clone(), id);
                }
                Some(r)
            }
        };
        let result = match renamed {
            None => {
                let _ = std::fs::remove_file(&tmp);
                Err(SaveError::new(SAVE_SUPERSEDED))
            }
            Some(Ok(())) => Ok(path),
            Some(Err(e)) => Err(keep(&path, &tmp, e.to_string())),
        };
        let mut st = self.state.lock().unwrap();
        st.active.remove(&id);
        if matches!(&result, Err(e) if e.reason != SAVE_SUPERSEDED) {
            st.failures += 1;
        }
        result
    }

    /// Abandonne l'enregistrement `id` (erreur côté fenêtre, ou morceau en
    /// erreur) : temporaire retiré, ancien fichier intact ; compte parmi les
    /// échecs. Sans effet sur un enregistrement déjà dans son `end()` (son
    /// temporaire est peut-être complet), ni sur un numéro qui n'est plus en cours.
    pub fn abort(&self, id: u32) {
        let entry = {
            let mut st = self.state.lock().unwrap();
            if st.active.get(&id).is_some_and(|e| e.ending) {
                return;
            }
            let e = st.active.remove(&id);
            if e.is_some() {
                st.failures += 1;
            }
            e
        };
        if let Some(e) = entry {
            discard(e);
        }
    }

    /// Abandonne TOUS les enregistrements dont les morceaux arrivaient encore
    /// (fermeture forcée de l'application) ; ceux qui sont dans leur `end()`
    /// finissent s'ils en ont le temps.
    pub fn abort_all(&self) {
        let entries: Vec<SaveEntry> = {
            let mut st = self.state.lock().unwrap();
            let ids: Vec<u32> = st.active.iter().filter(|(_, e)| !e.ending).map(|(k, _)| *k).collect();
            ids.into_iter().filter_map(|k| st.active.remove(&k)).collect()
        };
        for e in entries {
            discard(e);
        }
    }

    /// Octets reçus par l'enregistrement `id` (None s'il n'est pas en cours).
    pub fn bytes(&self, id: u32) -> Option<u64> {
        let open = self.state.lock().unwrap().active.get(&id).map(|e| std::sync::Arc::clone(&e.open))?;
        let g = open.lock().unwrap();
        g.as_ref().map(|o| o.bytes)
    }

    /// Enregistrements en cours (fin comprise) et octets reçus par eux en tout :
    /// la fermeture de la fenêtre attend qu'il n'en reste aucun.
    pub fn pending(&self) -> (usize, u64) {
        let (n, _, octets) = self.pending_detail();
        (n, octets)
    }

    /// Comme `pending`, avec en plus le nombre d'enregistrements déjà dans leur
    /// `end()` : (en cours, dont en fin, octets reçus).
    fn pending_detail(&self) -> (usize, usize, u64) {
        let (opens, en_fin): (Vec<_>, usize) = {
            let st = self.state.lock().unwrap();
            (st.active.values().map(|e| std::sync::Arc::clone(&e.open)).collect(), st.active.values().filter(|e| e.ending).count())
        };
        let octets = opens.iter().map(|open| open.lock().unwrap().as_ref().map_or(0, |o| o.bytes)).sum();
        (opens.len(), en_fin, octets)
    }

    /// Enregistrements qui ont échoué depuis le lancement (cf. `SaveState::failures`).
    pub fn failures(&self) -> u64 {
        self.state.lock().unwrap().failures
    }

    /// La fermeture différée de la fenêtre. Attend qu'aucun crawl ne tourne
    /// plus (`crawling` : workers sortis, `crawl:done` émis) et qu'aucun
    /// enregistrement ne soit plus en cours ; alors seulement, DEMANDE à la
    /// fenêtre (`ask`) si elle a encore quelque chose à enregistrer, et attend
    /// sa réponse (`ready`, remis à faux à chaque demande). La demande part
    /// APRÈS le `crawl:done` : la fenêtre, qui traite les événements dans
    /// l'ordre, a déjà commencé le réenregistrement des pages en vol quand elle
    /// la reçoit — elle ne répond qu'une fois tout enregistré. Si quelque chose
    /// reprend entre-temps, la réponse ne vaut plus et une nouvelle demande
    /// suivra. Décide alors : `Stay` si un enregistrement a échoué depuis la
    /// demande de fermeture (`failures` > `failures_at`), `Close` sinon. Délai
    /// de sûreté (`Forced`) : `stall` sans aucun octet reçu alors qu'un
    /// enregistrement envoie encore ses morceaux — jamais pendant un `end()`,
    /// qui finit toujours (réessais bornés, écriture forcée) —, ou `cap` après
    /// que la fenêtre a lu l'annonce. `cancelled` : la fermeture a été annulée
    /// ailleurs.
    ///
    /// `acked` : la fenêtre a reçu l'annonce de la fermeture (elle a donc
    /// traité tout ce qui la précédait : pages en file, `crawl:done`). Avant,
    /// elle peut avoir des milliers de pages en file à lire d'abord.
    ///
    /// Délai de SILENCE (`silence_timeout`, CLOSE_SILENCE dans l'application),
    /// avant comme après l'annonce : aucun signe de vie (`life` : compteur que
    /// font avancer ses signaux, au plus un par seconde pendant qu'elle lit des
    /// pages, et ses réponses), aucun enregistrement qui avance, aucune fin
    /// d'enregistrement en cours, pendant ce temps → la fenêtre est bloquée ou
    /// rechargée (elle ne répondra pas) : la fermeture a lieu (`Forced`). Tant
    /// qu'elle n'a pas répondu, la demande lui est REFAITE toutes les `reask` :
    /// une page rechargée, qui a perdu la première, y répond aussitôt.
    #[allow(clippy::too_many_arguments)]
    pub fn wait_close(
        &self,
        failures_at: u64,
        ready: &std::sync::atomic::AtomicBool,
        acked: &std::sync::atomic::AtomicBool,
        life: &std::sync::atomic::AtomicU64,
        ask: impl Fn(),
        crawling: impl Fn() -> bool,
        cancelled: impl Fn() -> bool,
        silence_timeout: std::time::Duration,
        reask: std::time::Duration,
        stall: std::time::Duration,
        cap: std::time::Duration,
    ) -> CloseOutcome {
        use std::sync::atomic::Ordering;
        use std::time::Instant;
        let mut dernier = None;
        let mut progres = Instant::now();
        let mut demande: Option<Instant> = None; // dernière demande faite pendant ce calme
        let (mut vie, mut silence) = (life.load(Ordering::SeqCst), Instant::now());
        let mut activite = None;
        // `cap` court à partir de l'annonce LUE : avant, la fenêtre peut lire ses
        // pages en file pendant des minutes ; vivante, elle est attendue (le moteur
        // a arrêté le crawl à la demande : la file ne grossit plus).
        let mut annonce_lue: Option<Instant> = None;
        loop {
            if cancelled() {
                return CloseOutcome::Cancelled;
            }
            let (n, en_fin, octets) = self.pending_detail();
            // Signe de vie, enregistrement qui avance, fin d'enregistrement en cours :
            // la fenêtre (ou le disque) travaille. Sinon, le silence court.
            let l = life.load(Ordering::SeqCst);
            if l != vie || activite != Some((n, octets)) || en_fin > 0 {
                vie = l;
                activite = Some((n, octets));
                silence = Instant::now();
            }
            if silence.elapsed() >= silence_timeout {
                return CloseOutcome::Forced;
            }
            let lue = acked.load(Ordering::SeqCst);
            if lue && annonce_lue.is_none() {
                annonce_lue = Some(Instant::now());
            }
            if n == 0 && !crawling() {
                if demande.is_some() && ready.load(Ordering::SeqCst) {
                    return if self.failures() > failures_at { CloseOutcome::Stay } else { CloseOutcome::Close };
                }
                match demande {
                    None => {
                        ready.store(false, Ordering::SeqCst);
                        demande = Some(Instant::now());
                        ask();
                    }
                    // Pas de réponse : redemander (une page rechargée répond aussitôt).
                    Some(t) if t.elapsed() >= reask => {
                        demande = Some(Instant::now());
                        ask();
                    }
                    Some(_) => {}
                }
            } else if demande.is_some() {
                // Un enregistrement (ou un crawl) a repris : il faudra redemander.
                demande = None;
                ready.store(false, Ordering::SeqCst);
            }
            // Le délai sans octet ne court que pendant l'ENVOI des morceaux, une fois
            // l'annonce lue (avant, la fenêtre lit encore ses pages en file).
            if !lue || n == 0 || en_fin > 0 || dernier != Some(octets) {
                dernier = Some(octets);
                progres = Instant::now();
            }
            if progres.elapsed() >= stall || annonce_lue.is_some_and(|t| t.elapsed() >= cap) {
                return CloseOutcome::Forced;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// Délai de SILENCE de la fermeture différée (cf. `SaveSessions::wait_close`) :
/// ni signe de vie de la fenêtre, ni enregistrement qui avance, depuis ce temps
/// → bloquée ou rechargée, elle ne répondra pas : on ferme. Une tâche de rendu
/// d'un seul tenant ne lit aucune page, donc n'envoie rien : mesurée par le
/// relecteur jusqu'à ~17 s (183 395 pages, JavaScript seul), davantage dans
/// WebView2 (12,8 s vues pendant l'enregistrement d'un crawl de 20 001 pages,
/// plus de 60 s pendant un crawl à pleine vitesse) : 120 s laissent la marge.
/// La seconde demande de fermeture ferme toujours avant, et le titre le dit.
pub const CLOSE_SILENCE: std::time::Duration = std::time::Duration::from_secs(120);

/// Tant que la fenêtre n'a pas répondu à la demande, elle lui est refaite à cet
/// intervalle : une page rechargée (qui a perdu la première demande, même après
/// avoir lu l'annonce) y répond aussitôt.
pub const CLOSE_REASK: std::time::Duration = std::time::Duration::from_secs(2);

#[cfg(test)]
mod tests {
    use super::*;

    /// Lit `texte` en messages de `taille` octets ; rend les messages et la fin.
    fn lire(texte: &str, taille: usize) -> (Vec<String>, Result<SavedCrawlEnd, String>) {
        let mut messages = Vec::new();
        let fin = read_pages(texte.as_bytes(), taille, |m| {
            messages.push(m);
            true
        });
        (messages, fin)
    }

    /// Les pages, recollées depuis les messages, dans l'ordre.
    fn pages_recollees(messages: &[String]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .flat_map(|m| {
                let v: serde_json::Value = serde_json::from_str(m).unwrap();
                v["pages"].as_array().unwrap().clone()
            })
            .collect()
    }

    const CRAWL: &str = r#"{"seed_url":"https://a.fr/","date":"2026-07-30T10:59:00.000Z","pages":[{"url":"https://a.fr/","status":200,"seo":{"title":"Été « à » \"Lyon\"","h2":["un","deux"]}},{"url":"https://a.fr/b/","status":404,"issues":[]},null,{"url":"https://a.fr/c/","x":[1,2.5,-3e-7,true]}],"sitemap_urls":["https://a.fr/","https://a.fr/b/"]}"#;

    /// Chaque page arrive EXACTEMENT avec le texte du fichier, dans l'ordre, et
    /// le dernier message porte le reste ; la taille des messages ne change rien.
    #[test]
    fn pages_identiques_au_fichier_quelle_que_soit_la_taille_des_messages() {
        let attendu: serde_json::Value = serde_json::from_str(CRAWL).unwrap();
        for taille in [1, 50, 120, 10_000] {
            let (messages, fin) = lire(CRAWL, taille);
            let fin = fin.unwrap();
            assert_eq!(pages_recollees(&messages), attendu["pages"].as_array().unwrap().clone(), "taille {taille}");
            assert_eq!(fin.count, 4);
            assert!(fin.done);
            assert_eq!(fin.seed_url.unwrap().get(), "\"https://a.fr/\"");
            assert_eq!(fin.date.unwrap().get(), "\"2026-07-30T10:59:00.000Z\"");
            assert_eq!(fin.sitemap_urls.unwrap().get(), r#"["https://a.fr/","https://a.fr/b/"]"#);
            // Texte des pages : les octets mêmes du fichier.
            let brut: String = messages
                .iter()
                .map(|m| m.strip_prefix("{\"pages\":[").unwrap().strip_suffix("]}").unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            let debut = CRAWL.find("\"pages\":[").unwrap() + "\"pages\":[".len();
            let fin_liste = CRAWL.find("],\"sitemap_urls\"").unwrap();
            assert_eq!(brut, CRAWL[debut..fin_liste], "taille {taille}");
        }
        // Une page par message quand la taille est minimale ; tout d'un coup sinon.
        assert_eq!(lire(CRAWL, 1).0.len(), 4);
        assert_eq!(lire(CRAWL, 10_000).0.len(), 1);
    }

    /// Ordre des clés quelconque, clés inconnues ignorées, valeurs reprises telles
    /// quelles ; liste vide : aucun message de pages.
    #[test]
    fn ordre_des_cles_et_valeurs_inattendues() {
        let texte = r#" { "autre" : {"x":[1,{"y":2}]}, "sitemap_urls":null, "pages" : [ ] , "date": 12, "seed_url":"https://b.fr/" } "#;
        let (messages, fin) = lire(texte, 100);
        let fin = fin.unwrap();
        assert!(messages.is_empty());
        assert_eq!(fin.count, 0);
        assert_eq!(fin.date.unwrap().get(), "12");
        assert_eq!(fin.sitemap_urls.unwrap().get(), "null");
        // Sans adresse ni date : null, comme une clé absente après JSON.parse.
        let (_, fin) = lire(r#"{"pages":[{"url":"u"}]}"#, 100);
        let fin = fin.unwrap();
        assert!(fin.seed_url.is_none() && fin.date.is_none() && fin.sitemap_urls.is_none());
        assert_eq!(
            serde_json::to_string(&fin).unwrap(),
            r#"{"done":true,"seed_url":null,"date":null,"sitemap_urls":null,"count":1}"#
        );
    }

    /// Ce que `JSON.parse` + `Array.isArray(data.pages)` refusaient est refusé.
    #[test]
    fn fichiers_refuses() {
        for texte in [
            "",
            "   ",
            "null",
            "[]",
            "\"texte\"",
            "{}",
            r#"{"seed_url":"https://a.fr/"}"#,
            r#"{"pages":{}}"#,
            r#"{"pages":"x"}"#,
            r#"{"pages":[{"url":"a"}"#,
            r#"{"pages":[{"url":"a"}]} x"#,
            r#"{"pages":[{"url":"a"},]}"#,
            r#"{"pages":[],"pages":[]}"#,
        ] {
            assert!(lire(texte, 100).1.is_err(), "accepté à tort : {texte:?}");
            assert!(read_info(texte.as_bytes()).is_err(), "aperçu accepté à tort : {texte:?}");
        }
    }

    /// Le destinataire qui ne veut plus de pages arrête la lecture.
    #[test]
    fn lecture_interrompue_par_le_destinataire() {
        let mut recus = 0;
        let fin = read_pages(CRAWL.as_bytes(), 1, |_| {
            recus += 1;
            recus < 2
        });
        assert_eq!(fin.unwrap_err(), INTERRUPTED);
        assert_eq!(recus, 2);
    }

    /// L'aperçu compte les pages et reprend l'adresse et la date, sans les pages.
    #[test]
    fn apercu() {
        let info = read_info(CRAWL.as_bytes()).unwrap();
        assert_eq!(info.count, 4);
        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"seed_url":"https://a.fr/","date":"2026-07-30T10:59:00.000Z","count":4}"#
        );
        let vide = read_info(r#"{"pages":[]}"#.as_bytes()).unwrap();
        assert_eq!(serde_json::to_string(&vide).unwrap(), r#"{"seed_url":null,"date":null,"count":0}"#);
    }

    // --- Écriture en morceaux (point 36) -----------------------------------

    fn dossier(nom: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("seo-spider-savedcrawl-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Fichiers du dossier, triés (pour voir ce qui reste après coup).
    fn contenu(d: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        v.sort();
        v
    }

    /// Coupe `texte` en morceaux d'au plus `n` caractères (comme la fenêtre :
    /// jamais au milieu d'un caractère).
    fn morceaux(texte: &str, n: usize) -> Vec<&str> {
        let mut out = Vec::new();
        let mut reste = texte;
        while !reste.is_empty() {
            let coupe = reste.char_indices().nth(n).map(|(i, _)| i).unwrap_or(reste.len());
            out.push(&reste[..coupe]);
            reste = &reste[coupe..];
        }
        out
    }

    /// Le même crawl, écrit d'un bloc (forme d'origine : `std::fs::write`) et
    /// en morceaux : fichiers IDENTIQUES octet pour octet, quelle que soit la
    /// taille des morceaux ; l'ancien fichier reste intact jusqu'au renommage ;
    /// aucun temporaire ne reste.
    #[test]
    fn en_morceaux_meme_fichier_qu_ecrit_d_un_bloc() {
        let d = dossier("identique");
        // (CRAWL contient des caractères de 2 et 3 octets : « », é, —)
        let texte = CRAWL.to_string();
        let d_un_bloc = d.join("bloc.json");
        std::fs::write(&d_un_bloc, &texte).unwrap();
        let sessions = SaveSessions::default();
        for n in [1, 7, 64, 100_000] {
            let chemin = d.join("https___a.fr_.json");
            std::fs::write(&chemin, "ANCIEN").unwrap();
            let id = sessions.begin(&chemin).unwrap();
            let mut position = 0u64;
            for m in morceaux(&texte, n) {
                sessions.write(id, position, m.as_bytes().to_vec()).unwrap();
                position += m.len() as u64;
                assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "ANCIEN", "l'ancien fichier bouge avant la fin");
            }
            assert_eq!(sessions.bytes(id), Some(texte.len() as u64));
            assert_eq!(sessions.end(id, position).unwrap(), chemin);
            assert_eq!(std::fs::read(&chemin).unwrap(), std::fs::read(&d_un_bloc).unwrap(), "morceaux de {n}");
            assert_eq!(contenu(&d), vec!["bloc.json".to_string(), "https___a.fr_.json".to_string()], "temporaire resté");
        }
        // Le dossier est créé s'il n'existe pas (comme save_crawl le faisait).
        let neuf = d.join("sous-dossier").join("x.json");
        let id = sessions.begin(&neuf).unwrap();
        sessions.write(id, 0, b"{}".to_vec()).unwrap();
        sessions.end(id, 2).unwrap();
        assert_eq!(std::fs::read_to_string(&neuf).unwrap(), "{}");
        assert_eq!(sessions.pending(), (0, 0), "plus rien en cours");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Abandon (fenêtre fermée, morceau en erreur) : l'ancien fichier reste,
    /// le temporaire disparaît ; un numéro inconnu est refusé.
    #[test]
    fn abandon_garde_l_ancien_fichier() {
        let d = dossier("abandon");
        let chemin = d.join("site.json");
        std::fs::write(&chemin, "ANCIEN").unwrap();
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"{\"seed_url\":".to_vec()).unwrap();
        assert_eq!(contenu(&d).len(), 2, "le temporaire existe pendant l'enregistrement");
        sessions.abort(id);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "ANCIEN");
        assert_eq!(contenu(&d), vec!["site.json".to_string()]);
        assert_eq!(sessions.write(id, 12, b"x".to_vec()).unwrap_err(), "enregistrement inconnu");
        assert_eq!(sessions.end(id, 12).unwrap_err().reason, "enregistrement inconnu");
        // Fermeture forcée de l'application : tous les temporaires disparaissent.
        let a = sessions.begin(&d.join("a.json")).unwrap();
        let b = sessions.begin(&d.join("b.json")).unwrap();
        sessions.write(a, 0, b"x".to_vec()).unwrap();
        sessions.write(b, 0, b"y".to_vec()).unwrap();
        sessions.abort_all();
        assert_eq!(contenu(&d), vec!["site.json".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Deux enregistrements du MÊME site (pause puis fin de crawl) : le plus
    /// récent gagne, l'ancien est abandonné sans bruit (SAVE_SUPERSEDED) ; un
    /// autre site, lui, n'est pas touché.
    #[test]
    fn le_plus_recent_enregistrement_du_meme_site_gagne() {
        let d = dossier("remplace");
        let chemin = d.join("site.json");
        let autre = d.join("autre.json");
        let sessions = SaveSessions::default();
        let premier = sessions.begin(&chemin).unwrap();
        let tiers = sessions.begin(&autre).unwrap();
        sessions.write(premier, 0, b"PREMIER".to_vec()).unwrap();
        let second = sessions.begin(&chemin).unwrap();
        assert_eq!(sessions.write(premier, 7, b"suite".to_vec()).unwrap_err(), SAVE_SUPERSEDED);
        assert_eq!(sessions.end(premier, 12).unwrap_err().reason, SAVE_SUPERSEDED);
        sessions.write(second, 0, b"SECOND".to_vec()).unwrap();
        sessions.end(second, 6).unwrap();
        sessions.write(tiers, 0, b"AUTRE".to_vec()).unwrap();
        sessions.end(tiers, 5).unwrap();
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "SECOND");
        assert_eq!(std::fs::read_to_string(&autre).unwrap(), "AUTRE");
        assert_eq!(contenu(&d), vec!["autre.json".to_string(), "site.json".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Temporaires ORPHELINS (plantage) : ceux du même site de plus de 10
    /// minutes sont retirés au prochain enregistrement de ce site ; un récent
    /// (autre instance en train d'écrire) et ceux des autres sites restent.
    #[test]
    fn temporaires_orphelins() {
        let d = dossier("orphelins");
        let chemin = d.join("site.json");
        let vieux = d.join("site.json.tmp-999-1");
        let recent = d.join("site.json.tmp-999-2");
        let autre_site = d.join("autre.json.tmp-999-3");
        for f in [&vieux, &recent, &autre_site] {
            std::fs::write(f, "morceau").unwrap();
        }
        let il_y_a_une_heure = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for f in [&vieux, &autre_site] {
            std::fs::File::options().write(true).open(f).unwrap().set_modified(il_y_a_une_heure).unwrap();
        }
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        let reste = contenu(&d);
        assert!(!reste.contains(&"site.json.tmp-999-1".to_string()), "{reste:?}");
        assert!(reste.contains(&"site.json.tmp-999-2".to_string()), "{reste:?}");
        assert!(reste.contains(&"autre.json.tmp-999-3".to_string()), "{reste:?}");
        sessions.write(id, 0, b"{}".to_vec()).unwrap();
        sessions.end(id, 2).unwrap();
        std::fs::remove_dir_all(&d).unwrap();
    }

    // --- Corrections après relecture (14/11/2026) ----------------------------

    /// Le nom sous lequel un crawl est gardé porte la date UTC (ISO 8601 compact).
    #[test]
    fn date_utc_du_nom_garde() {
        let t = |s: u64| utc_stamp(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s));
        assert_eq!(t(0), "19700101T000000Z");
        assert_eq!(t(951_868_799), "20000229T235959Z"); // jour bissextile
        assert_eq!(t(1_794_622_507), "20261114T021507Z");
        assert_eq!(t(4_107_542_400), "21000301T000000Z"); // 2100 n'est pas bissextile
        let d = dossier("nom-garde");
        let chemin = d.join("https___site.fr_.json");
        let k = kept_path(&chemin).unwrap();
        let nom = k.file_name().unwrap().to_string_lossy().to_string();
        assert!(nom.starts_with("https___site.fr_.") && nom.ends_with("Z.json") && nom.len() == "https___site.fr_.".len() + 21, "{nom}");
        assert!(!nom.starts_with("https___site.fr_.json.tmp-"), "hors du motif des orphelins");
        // Un nom déjà pris n'est jamais écrasé.
        std::fs::write(&k, "DÉJÀ LÀ").unwrap();
        assert_ne!(kept_path(&chemin).unwrap(), k);
        assert_eq!(std::fs::read_to_string(&k).unwrap(), "DÉJÀ LÀ");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P3-4 : un morceau hors position abandonne l'enregistrement (ancien
    /// fichier intact, temporaire retiré) ; la répétition exacte du dernier est
    /// ignorée ; un total qui ne correspond pas empêche le renommage.
    #[test]
    fn morceau_hors_position_rejoue_ou_perdu() {
        let d = dossier("position");
        let chemin = d.join("site.json");
        std::fs::write(&chemin, "ANCIEN").unwrap();
        let sessions = SaveSessions::default();
        // Répétition exacte du dernier morceau : ignorée, le fichier est juste.
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"{\"a\":".to_vec()).unwrap();
        sessions.write(id, 0, b"{\"a\":".to_vec()).unwrap();
        sessions.write(id, 5, b"1}".to_vec()).unwrap();
        sessions.write(id, 5, b"1}".to_vec()).unwrap();
        assert_eq!(sessions.end(id, 7).unwrap(), chemin);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "{\"a\":1}");
        // Morceau sauté, ou même position avec d'autres octets : refusés.
        for (position, octets) in [(9u64, b"2}".to_vec()), (0, b"{\"b\":".to_vec())] {
            std::fs::write(&chemin, "ANCIEN").unwrap();
            let id = sessions.begin(&chemin).unwrap();
            sessions.write(id, 0, b"{\"a\":".to_vec()).unwrap();
            let e = sessions.write(id, position, octets).unwrap_err();
            assert!(e.starts_with("morceau hors position"), "{e}");
            assert_eq!(sessions.write(id, 5, b"1}".to_vec()).unwrap_err(), "enregistrement inconnu");
            assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "ANCIEN");
            assert_eq!(contenu(&d), vec!["site.json".to_string()], "temporaire resté");
        }
        // Total annoncé différent de ce qui a été écrit : pas de renommage.
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"{\"a\":1}".to_vec()).unwrap();
        let e = sessions.end(id, 9).unwrap_err();
        assert_eq!(e, SaveError::new("7 octets reçus pour 9 envoyés"));
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "ANCIEN");
        assert_eq!(contenu(&d), vec!["site.json".to_string()]);
        assert_eq!(sessions.pending().0, 0);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P3-2 : un enregistrement plus ANCIEN du même fichier, encore dans son
    /// end() quand un plus récent commence, ne renomme pas après lui ; s'il
    /// renomme avant, le plus récent passe ensuite. Le plus récent gagne.
    #[test]
    fn un_ancien_enregistrement_ne_renomme_pas_apres_un_plus_recent() {
        let d = dossier("generation");
        let chemin = d.join("site.json");
        std::fs::write(&chemin, "ANCIEN").unwrap();
        let sessions = SaveSessions::default();
        // A en fin d'enregistrement (vidé, écrit sur le disque, pas encore renommé)…
        let a = sessions.begin(&chemin).unwrap();
        sessions.write(a, 0, b"PAUSE".to_vec()).unwrap();
        let pret = sessions.end_prepare(a, 5).unwrap();
        // … B commence (A n'est pas remplacé : il est déjà dans son end()), finit et renomme.
        let b = sessions.begin(&chemin).unwrap();
        sessions.write(b, 0, b"FIN-DU-CRAWL".to_vec()).unwrap();
        assert_eq!(sessions.pending().0, 2, "A compte encore comme en cours");
        sessions.end(b, 12).unwrap();
        // A ne renomme plus par-dessus B : son temporaire est retiré.
        assert_eq!(sessions.end_commit(a, pret).unwrap_err().reason, SAVE_SUPERSEDED);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "FIN-DU-CRAWL");
        assert_eq!(contenu(&d), vec!["site.json".to_string()]);
        // Dans l'autre ordre : A renomme d'abord, puis B le remplace.
        let a = sessions.begin(&chemin).unwrap();
        sessions.write(a, 0, b"PAUSE".to_vec()).unwrap();
        let pret = sessions.end_prepare(a, 5).unwrap();
        let b = sessions.begin(&chemin).unwrap();
        sessions.write(b, 0, b"FIN-DU-CRAWL".to_vec()).unwrap();
        assert_eq!(sessions.end_commit(a, pret).unwrap(), chemin);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "PAUSE");
        sessions.end(b, 12).unwrap();
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "FIN-DU-CRAWL");
        // Le plus récent abandonne : l'ancien, complet, renomme quand même.
        let a = sessions.begin(&chemin).unwrap();
        sessions.write(a, 0, b"PAUSE".to_vec()).unwrap();
        let pret = sessions.end_prepare(a, 5).unwrap();
        let b = sessions.begin(&chemin).unwrap();
        sessions.abort(b);
        assert_eq!(sessions.end_commit(a, pret).unwrap(), chemin);
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "PAUSE");
        assert_eq!(contenu(&d), vec!["site.json".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P2-1 : `pending` compte les enregistrements en cours (fin comprise) ;
    /// `wait_idle` rend la main quand ils sont finis, ou au bout du délai de
    /// sûreté quand plus rien n'arrive.
    #[test]
    fn attente_de_la_fin_des_enregistrements() {
        let d = dossier("attente");
        let sessions = SaveSessions::default();
        let court = std::time::Duration::from_millis(300);
        let long = std::time::Duration::from_secs(20);
        // (Fenêtre prête dès qu'on le lui demande, aucun crawl : seule l'attente
        // des enregistrements joue ici.)
        let attendre = |s: &SaveSessions| fermer(s, s.failures(), || false, || false, court, long);
        assert_eq!(attendre(&sessions), CloseOutcome::Close, "rien en cours : tout de suite");
        // Un enregistrement qui avance (un morceau toutes les 20 ms) puis finit.
        let id = sessions.begin(&d.join("site.json")).unwrap();
        assert_eq!(sessions.pending(), (1, 0));
        let s = sessions.clone();
        let fil = std::thread::spawn(move || {
            for k in 0..25u64 {
                s.write(id, k, b"x".to_vec()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            s.end(id, 25).unwrap();
        });
        let debut = std::time::Instant::now();
        assert_eq!(attendre(&sessions), CloseOutcome::Close, "fini avant le délai de sûreté");
        assert!(debut.elapsed() >= std::time::Duration::from_millis(400), "attendu la fin réelle");
        fil.join().unwrap();
        // Un enregistrement bloqué (plus aucun morceau) : délai de sûreté.
        let id = sessions.begin(&d.join("autre.json")).unwrap();
        sessions.write(id, 0, b"x".to_vec()).unwrap();
        let debut = std::time::Instant::now();
        assert_eq!(attendre(&sessions), CloseOutcome::Forced);
        assert!(debut.elapsed() < std::time::Duration::from_secs(5));
        sessions.abort(id);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// La fenêtre de ces essais : prête dès que le moteur le lui demande.
    fn fermer(
        s: &SaveSessions,
        failures_at: u64,
        crawling: impl Fn() -> bool,
        cancelled: impl Fn() -> bool,
        stall: std::time::Duration,
        cap: std::time::Duration,
    ) -> CloseOutcome {
        let pret = std::sync::atomic::AtomicBool::new(false);
        let vivante = std::sync::atomic::AtomicBool::new(true);
        let vie = std::sync::atomic::AtomicU64::new(0);
        let silence = std::time::Duration::from_secs(10);
        let redemande = std::time::Duration::from_millis(200);
        s.wait_close(failures_at, &pret, &vivante, &vie, || pret.store(true, std::sync::atomic::Ordering::SeqCst), crawling, cancelled, silence, redemande, stall, cap)
    }

    /// TROISIÈME TOUR, N-7 / N-8 : une fenêtre SILENCIEUSE (bloquée, rechargée)
    /// ne retient la fermeture que le délai de silence ; une fenêtre qui lit
    /// encore des pages en file (signes de vie réguliers), bien plus longtemps
    /// que ce délai, est attendue — puis la fermeture suit sa réponse ; la
    /// limite totale (`cap`) ne court qu'une fois l'annonce lue.
    #[test]
    fn fenetre_sans_signe_de_vie_delai_court() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::Arc;
        let sessions = SaveSessions::default();
        let court = std::time::Duration::from_millis(400);
        let redemande = std::time::Duration::from_millis(100);
        let long = std::time::Duration::from_secs(20);
        // Jamais de signe de vie : fermée au bout du délai de silence, pas de la limite de 5 min.
        let (pret, vivante, vie) = (AtomicBool::new(false), AtomicBool::new(false), AtomicU64::new(0));
        let debut = std::time::Instant::now();
        let issue = sessions.wait_close(0, &pret, &vivante, &vie, || {}, || false, || false, court, redemande, long, long);
        assert_eq!(issue, CloseOutcome::Forced);
        assert!(debut.elapsed() >= court && debut.elapsed() < std::time::Duration::from_secs(3));
        // 1,5 s de pages en file (un signe de vie toutes les 100 ms : près de 4 fois le délai),
        // puis l'annonce lue et la réponse : fermée sur la réponse, jamais forcée.
        let (pret, vivante, vie) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
        let (p, v, l) = (Arc::clone(&pret), Arc::clone(&vivante), Arc::clone(&vie));
        let fenetre = std::thread::spawn(move || {
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                l.fetch_add(1, Ordering::SeqCst);
            }
            v.store(true, Ordering::SeqCst);
            l.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(150));
            p.store(true, Ordering::SeqCst);
            l.fetch_add(1, Ordering::SeqCst);
        });
        let debut = std::time::Instant::now();
        let issue = sessions.wait_close(0, &pret, &vivante, &vie, || {}, || false, || false, court, redemande, long, long);
        fenetre.join().unwrap();
        assert_eq!(issue, CloseOutcome::Close);
        assert!(debut.elapsed() >= std::time::Duration::from_millis(1600), "fermée sans attendre la réponse");
        // La limite totale (`cap`, ici 500 ms) ne court qu'une fois l'annonce lue : une fenêtre
        // vivante qui lit ses pages en file pendant 1,2 s n'est pas coupée pour autant.
        let (pret, vivante, vie) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
        let (p, v, l) = (Arc::clone(&pret), Arc::clone(&vivante), Arc::clone(&vie));
        let fenetre = std::thread::spawn(move || {
            for _ in 0..12 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                l.fetch_add(1, Ordering::SeqCst);
            }
            v.store(true, Ordering::SeqCst);
            p.store(true, Ordering::SeqCst);
            l.fetch_add(1, Ordering::SeqCst);
        });
        let issue = sessions.wait_close(0, &pret, &vivante, &vie, || {}, || false, || false, court, redemande, long, std::time::Duration::from_millis(500));
        fenetre.join().unwrap();
        assert_eq!(issue, CloseOutcome::Close, "coupée pendant la lecture de ses pages en file");
    }

    /// QUATRIÈME TOUR, O-1 : avec le VRAI délai de silence (CLOSE_SILENCE), une
    /// fenêtre muette 30 s (une tâche de rendu de 30 s, pas un seul signe de
    /// vie) puis vivante n'est PAS fermée : elle répond, la fermeture suit sa
    /// réponse.
    #[test]
    fn fenetre_muette_30_s_puis_vivante_pas_fermee() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
        use std::sync::Arc;
        assert!(CLOSE_SILENCE >= std::time::Duration::from_secs(120));
        let sessions = SaveSessions::default();
        let (pret, vivante, vie) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
        let (p, v, l) = (Arc::clone(&pret), Arc::clone(&vivante), Arc::clone(&vie));
        let fenetre = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(30)); // muette
            l.fetch_add(1, Ordering::SeqCst); // vivante : signe de vie, annonce lue, réponse
            v.store(true, Ordering::SeqCst);
            p.store(true, Ordering::SeqCst);
        });
        let debut = std::time::Instant::now();
        let issue = sessions.wait_close(
            0,
            &pret,
            &vivante,
            &vie,
            || {},
            || false,
            || false,
            CLOSE_SILENCE,
            CLOSE_REASK,
            std::time::Duration::from_secs(30),
            std::time::Duration::from_secs(300),
        );
        fenetre.join().unwrap();
        assert_eq!(issue, CloseOutcome::Close, "fenêtre vivante fermée de force");
        assert!(debut.elapsed() >= std::time::Duration::from_secs(30));
    }

    /// QUATRIÈME TOUR, O-1 : une fenêtre muette POUR DE BON est fermée au terme
    /// du délai de silence (ici réduit à 800 ms), avant comme après avoir lu
    /// l'annonce ; et O-5 : une page RECHARGÉE après avoir lu l'annonce (la
    /// première demande perdue) répond à une redemande, la fermeture n'attend
    /// pas les 5 min.
    #[test]
    fn fenetre_muette_pour_de_bon_et_page_rechargee() {
        use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
        use std::sync::Arc;
        let sessions = SaveSessions::default();
        let silence = std::time::Duration::from_millis(800);
        let redemande = std::time::Duration::from_millis(200);
        let long = std::time::Duration::from_secs(300);
        for annonce_lue in [false, true] {
            let (pret, vivante, vie) = (AtomicBool::new(false), AtomicBool::new(annonce_lue), AtomicU64::new(0));
            let debut = std::time::Instant::now();
            let issue = sessions.wait_close(0, &pret, &vivante, &vie, || {}, || false, || false, silence, redemande, long, long);
            assert_eq!(issue, CloseOutcome::Forced, "annonce lue : {annonce_lue}");
            assert!(debut.elapsed() >= silence && debut.elapsed() < std::time::Duration::from_secs(3));
        }
        // Rechargée après l'annonce : elle ne répond qu'à partir de la 3e demande.
        let (pret, vivante, vie) = (Arc::new(AtomicBool::new(false)), AtomicBool::new(true), AtomicU64::new(0));
        let demandes = AtomicUsize::new(0);
        let debut = std::time::Instant::now();
        let issue = sessions.wait_close(
            0,
            &pret,
            &vivante,
            &vie,
            || {
                if demandes.fetch_add(1, Ordering::SeqCst) + 1 >= 3 {
                    pret.store(true, Ordering::SeqCst);
                    vie.fetch_add(1, Ordering::SeqCst);
                }
            },
            || false,
            || false,
            std::time::Duration::from_secs(120),
            redemande,
            long,
            long,
        );
        assert_eq!(issue, CloseOutcome::Close);
        assert!(demandes.load(Ordering::SeqCst) >= 3);
        assert!(debut.elapsed() < std::time::Duration::from_secs(3), "attendu {:?}", debut.elapsed());
    }

    /// SECOND TOUR, N-4 : un enregistrement dans son end() (ici : préparé, pas
    /// encore renommé) ne déclenche JAMAIS le délai sans octet ; la fermeture
    /// l'attend, puis a lieu.
    #[test]
    fn fin_d_enregistrement_lente_sans_delai_de_surete() {
        let d = dossier("fin-lente");
        let chemin = d.join("site.json");
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"{}".to_vec()).unwrap();
        let pret = sessions.end_prepare(id, 2).unwrap();
        let s = sessions.clone();
        let fil = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(800)); // bien plus que le délai de 200 ms
            s.end_commit(id, pret).unwrap();
        });
        let debut = std::time::Instant::now();
        let court = std::time::Duration::from_millis(200);
        let issue = fermer(&sessions, 0, || false, || false, court, std::time::Duration::from_secs(20));
        assert_eq!(issue, CloseOutcome::Close);
        assert!(debut.elapsed() >= std::time::Duration::from_millis(700), "la fin a été attendue");
        fil.join().unwrap();
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "{}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// SECOND TOUR, N-2 : un enregistrement qui ÉCHOUE après la demande de
    /// fermeture → `Stay` (la fenêtre reste, le message aussi) ; un échec
    /// d'AVANT la demande ne retient pas la fenêtre ; un enregistrement
    /// remplacé par un plus récent n'est pas un échec.
    #[test]
    fn echec_pendant_la_fermeture_la_fenetre_reste() {
        let d = dossier("echec-fermeture");
        let sessions = SaveSessions::default();
        let court = std::time::Duration::from_millis(300);
        let long = std::time::Duration::from_secs(20);
        // Un échec AVANT la demande (total faux) …
        let id = sessions.begin(&d.join("a.json")).unwrap();
        sessions.write(id, 0, b"{}".to_vec()).unwrap();
        assert!(sessions.end(id, 9).is_err());
        assert_eq!(sessions.failures(), 1);
        // … puis la demande de fermeture, un enregistrement en cours, qui réussit : Close.
        let avant = sessions.failures();
        let id = sessions.begin(&d.join("b.json")).unwrap();
        let s = sessions.clone();
        let fil = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            s.write(id, 0, b"{}".to_vec()).unwrap();
            s.end(id, 2).unwrap();
        });
        assert_eq!(fermer(&sessions, avant, || false, || false, court, long), CloseOutcome::Close);
        fil.join().unwrap();
        // Un enregistrement remplacé par un plus récent (pause puis réenregistrement) : pas un échec.
        let avant = sessions.failures();
        let premier = sessions.begin(&d.join("c.json")).unwrap();
        let second = sessions.begin(&d.join("c.json")).unwrap();
        assert_eq!(sessions.write(premier, 0, b"x".to_vec()).unwrap_err(), SAVE_SUPERSEDED);
        sessions.abort(premier); // la fenêtre abandonne celui qui a été remplacé : sans effet
        sessions.write(second, 0, b"{}".to_vec()).unwrap();
        sessions.end(second, 2).unwrap();
        assert_eq!(sessions.failures(), avant);
        assert_eq!(fermer(&sessions, avant, || false, || false, court, long), CloseOutcome::Close);
        // Un échec APRÈS la demande : la fenêtre reste.
        let avant = sessions.failures();
        let id = sessions.begin(&d.join("e.json")).unwrap();
        let s = sessions.clone();
        let fil = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            s.write(id, 0, b"{}".to_vec()).unwrap();
            assert!(s.write(id, 9, b"?".to_vec()).is_err()); // morceau hors position
        });
        assert_eq!(fermer(&sessions, avant, || false, || false, court, long), CloseOutcome::Stay);
        fil.join().unwrap();
        // Une fermeture annulée ailleurs : rien à décider.
        let id = sessions.begin(&d.join("f.json")).unwrap();
        assert_eq!(fermer(&sessions, sessions.failures(), || false, || true, court, long), CloseOutcome::Cancelled);
        sessions.abort(id);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// SECOND TOUR, N-6 : fermeture demandée PENDANT un crawl. La fenêtre met le
    /// crawl en pause et l'enregistre ; la fermeture attend que le crawl soit
    /// vraiment fini (workers sortis), puis demande à la fenêtre, qui a alors
    /// commencé le réenregistrement des pages en vol (elle traite `crawl:done`
    /// AVANT la demande) et ne répond qu'une fois tout enregistré.
    #[test]
    fn fermeture_pendant_un_crawl() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        let d = dossier("fermeture-crawl");
        let chemin = d.join("site.json");
        let sessions = SaveSessions::default();
        let crawling = Arc::new(AtomicBool::new(true));
        let prete = Arc::new(AtomicBool::new(false));
        // La fenêtre : « occupée » dès qu'elle a pris un enregistrement en charge
        // (au crawl:done, avant même que le moteur le voie commencer).
        let occupee = Arc::new(AtomicBool::new(false));
        let reponse_due = Arc::new(AtomicBool::new(false));
        let (s, c, p, o, r, ch) = (
            sessions.clone(),
            Arc::clone(&crawling),
            Arc::clone(&prete),
            Arc::clone(&occupee),
            Arc::clone(&reponse_due),
            chemin.clone(),
        );
        let fenetre = std::thread::spawn(move || {
            // Les workers finissent leurs pages en vol (300 ms) : rien n'est encore enregistré.
            std::thread::sleep(std::time::Duration::from_millis(300));
            // crawl:done : la fenêtre prend le réenregistrement en charge, puis le crawl est fini.
            o.store(true, Ordering::SeqCst);
            c.store(false, Ordering::SeqCst);
            // Le réenregistrement commence un peu après (aller-retour vers le moteur).
            std::thread::sleep(std::time::Duration::from_millis(100));
            let id = s.begin(&ch).unwrap();
            for k in 0..10u64 {
                s.write(id, k, b"x".to_vec()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            s.end(id, 10).unwrap();
            // Plus rien à enregistrer : la fenêtre répond à la demande restée en attente.
            o.store(false, Ordering::SeqCst);
            if r.swap(false, Ordering::SeqCst) {
                p.store(true, Ordering::SeqCst);
            }
        });
        let debut = std::time::Instant::now();
        let vivante = AtomicBool::new(true); // la fenêtre a traité l'annonce de la fermeture
        let vie = std::sync::atomic::AtomicU64::new(0);
        let issue = sessions.wait_close(
            0,
            &prete,
            &vivante,
            &vie,
            || {
                if occupee.load(Ordering::SeqCst) {
                    reponse_due.store(true, Ordering::SeqCst);
                } else {
                    prete.store(true, Ordering::SeqCst);
                }
            },
            || crawling.load(Ordering::SeqCst),
            || false,
            std::time::Duration::from_secs(10),
            std::time::Duration::from_millis(200),
            std::time::Duration::from_millis(250), // plus court que l'attente des workers : sans effet
            std::time::Duration::from_secs(20),
        );
        assert_eq!(issue, CloseOutcome::Close);
        assert!(debut.elapsed() >= std::time::Duration::from_millis(550), "fermé avant la fin de l'enregistrement");
        fenetre.join().unwrap();
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "xxxxxxxxxx", "le crawl est enregistré avant la fermeture");
        // Rien à attendre (crawl déjà en pause et enregistré) : tout de suite.
        let debut = std::time::Instant::now();
        let issue = fermer(&sessions, 0, || false, || false, std::time::Duration::from_secs(1), std::time::Duration::from_secs(20));
        assert_eq!(issue, CloseOutcome::Close);
        assert!(debut.elapsed() < std::time::Duration::from_millis(300));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// SECOND TOUR, N-1 : un orphelin COMPLET (le dernier exemplaire d'un crawl)
    /// n'est jamais retiré par le nettoyage : il est rangé sous un nom gardé —
    /// sauf s'il est plus ancien que le fichier du site. Un incomplet est retiré.
    #[test]
    fn orphelin_complet_range_sous_un_nom_garde() {
        let d = dossier("orphelin-complet");
        let chemin = d.join("site.json");
        let heure = |h: u64| std::time::SystemTime::now() - std::time::Duration::from_secs(h * 3600);
        let date = |f: &std::path::Path, t: std::time::SystemTime| {
            std::fs::File::options().write(true).open(f).unwrap().set_modified(t).unwrap();
        };
        std::fs::write(&chemin, "{\"pages\":[]}").unwrap();
        date(&chemin, heure(3));
        let complet = d.join("site.json.tmp-999-1"); // plus récent que le fichier du site
        let complet_depasse = d.join("site.json.tmp-999-2"); // plus ancien que le fichier du site
        let incomplet = d.join("site.json.tmp-999-3");
        std::fs::write(&complet, "{\"seed_url\":\"a\",\"pages\":[],\"sitemap_urls\":[]}").unwrap();
        std::fs::write(&complet_depasse, "{\"seed_url\":\"b\",\"pages\":[],\"sitemap_urls\":[]}").unwrap();
        std::fs::write(&incomplet, "{\"seed_url\":\"c\",\"pages\":[{\"url\":\"x\"").unwrap();
        date(&complet, heure(1));
        date(&complet_depasse, heure(5));
        date(&incomplet, heure(1));
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        sessions.abort(id);
        let reste = contenu(&d);
        assert!(!reste.iter().any(|n| n.contains(".tmp-999")), "orphelins traités : {reste:?}");
        let gardes: Vec<&String> = reste.iter().filter(|n| n.starts_with("site.") && n.ends_with("Z.json")).collect();
        assert_eq!(gardes.len(), 1, "{reste:?}");
        assert_eq!(std::fs::read_to_string(d.join(gardes[0])).unwrap(), "{\"seed_url\":\"a\",\"pages\":[],\"sitemap_urls\":[]}");
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "{\"pages\":[]}", "le fichier du site n'est pas touché");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// SECOND TOUR, N-5 : le nom gardé est RÉSERVÉ (fichier créé sans pouvoir
    /// en remplacer un autre) ; deux réservations de la même seconde ont deux
    /// noms ; un fichier déjà là n'est jamais touché.
    #[test]
    fn nom_garde_reserve() {
        let d = dossier("nom-reserve");
        let chemin = d.join("https___site.fr_.json");
        let a = kept_path(&chemin).unwrap();
        let b = kept_path(&chemin).unwrap();
        assert_ne!(a, b);
        assert!(a.exists() && b.exists(), "noms réservés sur le disque");
        std::fs::write(&a, "GARDÉ PAR UNE AUTRE INSTANCE").unwrap();
        let c = kept_path(&chemin).unwrap();
        assert!(c != a && c != b);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "GARDÉ PAR UNE AUTRE INSTANCE");
        // Ranger un temporaire complet : renommé par-dessus SON nom réservé.
        let tmp = d.join("https___site.fr_.json.tmp-1-1");
        std::fs::write(&tmp, "{\"pages\":[]}").unwrap();
        let garde = stash(&chemin, &tmp).unwrap();
        assert!(!tmp.exists() && garde != a && garde != b && garde != c);
        assert_eq!(std::fs::read_to_string(&garde).unwrap(), "{\"pages\":[]}");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "GARDÉ PAR UNE AUTRE INSTANCE");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P1-1 : le fichier du crawl tenu par un autre programme (partage lecture
    /// et écriture SANS suppression, le plus courant sous Windows) pendant tout
    /// le end() : le renommage est réessayé puis refusé, et le crawl COMPLET est
    /// gardé à côté, ouvrable ; l'ancien fichier reste intact.
    #[cfg(windows)]
    #[test]
    fn renommage_refuse_le_crawl_est_garde() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = dossier("renommage-refuse");
        let chemin = d.join("https___site.fr_.json");
        std::fs::write(&chemin, "ANCIEN").unwrap();
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"NOUVEAU-CRAWL".to_vec()).unwrap();
        let tenu = std::fs::OpenOptions::new().read(true).share_mode(1 | 2).open(&chemin).unwrap();
        let debut = std::time::Instant::now();
        let e = sessions.end(id, 13).unwrap_err();
        assert!(debut.elapsed() >= std::time::Duration::from_secs(3), "renommage réessayé ~3 s");
        drop(tenu);
        let garde = e.kept.clone().expect("crawl gardé");
        assert_eq!(std::fs::read_to_string(&garde).unwrap(), "NOUVEAU-CRAWL", "{e:?}");
        assert_eq!(garde.parent(), chemin.parent());
        assert!(garde.extension().is_some_and(|x| x == "json"));
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "ANCIEN");
        assert!(contenu(&d).iter().all(|n| !n.contains(".tmp-")), "{:?}", contenu(&d));
        assert_eq!(sessions.pending().0, 0);
        // Le prochain enregistrement du site ne touche pas au crawl gardé.
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"SUIVANT".to_vec()).unwrap();
        sessions.end(id, 7).unwrap();
        assert_eq!(std::fs::read_to_string(&garde).unwrap(), "NOUVEAU-CRAWL");
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "SUIVANT");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P1-1 : le fichier du crawl tenu quelques centaines de millisecondes
    /// (antivirus, indexeur) : le renommage réessayé finit par passer.
    #[cfg(windows)]
    #[test]
    fn renommage_reessaye_quand_le_fichier_se_libere() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = dossier("renommage-libere");
        let chemin = d.join("https___site.fr_.json");
        std::fs::write(&chemin, "ANCIEN").unwrap();
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        sessions.write(id, 0, b"NOUVEAU-CRAWL".to_vec()).unwrap();
        let tenu = std::fs::OpenOptions::new().read(true).share_mode(1 | 2).open(&chemin).unwrap();
        let liberation = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            drop(tenu);
        });
        assert_eq!(sessions.end(id, 13).unwrap(), chemin);
        liberation.join().unwrap();
        assert_eq!(std::fs::read_to_string(&chemin).unwrap(), "NOUVEAU-CRAWL");
        assert_eq!(contenu(&d), vec!["https___site.fr_.json".to_string()], "ni temporaire ni copie gardée");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// P3-3 : un vieux temporaire encore OUVERT ailleurs (une autre instance
    /// qui écrit depuis plus de 10 minutes) n'est pas pris pour un orphelin ;
    /// une fois fermé, il l'est.
    #[cfg(windows)]
    #[test]
    fn orphelin_ouvert_ailleurs_garde() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = dossier("orphelin-ouvert");
        let chemin = d.join("site.json");
        let ailleurs = d.join("site.json.tmp-999-1");
        std::fs::write(&ailleurs, "morceau").unwrap();
        let il_y_a_une_heure = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&ailleurs).unwrap().set_modified(il_y_a_une_heure).unwrap();
        // Tenu avec le partage le plus large : reconnu quand même.
        let tenu = std::fs::OpenOptions::new().read(true).share_mode(1 | 2 | 4).open(&ailleurs).unwrap();
        let sessions = SaveSessions::default();
        let id = sessions.begin(&chemin).unwrap();
        assert!(ailleurs.exists(), "retiré alors qu'il est ouvert ailleurs");
        sessions.abort(id);
        drop(tenu);
        let id = sessions.begin(&chemin).unwrap();
        assert!(!ailleurs.exists(), "orphelin fermé : retiré");
        sessions.abort(id);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
