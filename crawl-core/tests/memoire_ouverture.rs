//! Point 9 du chantier « après les langues » : rouvrir un crawl enregistré
//! très gros (le plus gros de Chris : un forum, 415 Mo, 36 679 pages).
//!
//! FORME D'ORIGINE (jusqu'au 14/11/2026), mesurée ici à l'identique :
//! `load_crawl` lisait le fichier en entier (`read_to_string`) et le rendait
//! comme une `String` ; Tauri la sérialise en JSON (une chaîne ÉCHAPPÉE :
//! chaque `"` et chaque `\` doublés), wry la recopie dans un flux mémoire pour
//! WebView2 (`SHCreateMemStream`), et la fenêtre la décode (`response.json()`)
//! puis relit le texte obtenu avec `JSON.parse`. L'interface l'appelait À
//! CHAQUE démarrage d'une adresse déjà crawlée, juste pour proposer
//! « Nouveau crawl / Revoir l'enregistrement ».
//!
//! FORME ACTUELLE : `savedcrawl::read_info` (aperçu, sans charger les pages)
//! et `savedcrawl::read_pages` (pages en messages de 4 Mo).
//!
//!   cargo test -p crawl-core --test memoire_ouverture -- --nocapture
//!   cargo test -p crawl-core --test memoire_ouverture -- --ignored --nocapture   (50 000 pages et 415 Mo)
//!
//! Deux tests ignorés de plus, lancés à la main avec une variable d'environnement :
//!   SEO_SPIDER_VRAI_CRAWL=<chemin d'un vrai crawl, LU seulement>  → vrai_crawl_enregistre
//!   SEO_SPIDER_BANC=<dossier>  → ecrire_les_reponses_pour_la_fenetre (essai dans Chromium)

mod commun;

use std::io::{BufReader, Read};

use commun::{fichier_de_crawl, mesure, mo, FichierEnFlux, PAGES_POUR_415_MO};
use crawl_core::savedcrawl::{read_info, read_pages, MESSAGE_BYTES};

/// Longueur maximale d'une chaîne V8 64 bits (2^29 - 24 caractères).
const V8_LONGUEUR_MAX: usize = 536_870_888;

fn forme_d_origine(n: usize) -> (usize, usize) {
    // `read_to_string` alloue le fichier entier, à sa taille exacte (il réserve
    // d'après la taille du fichier) : `contenu.len()` octets.
    let contenu = fichier_de_crawl(n);
    // Ce que Tauri fait d'une `String` rendue par une commande (`IpcResponse` :
    // `serde_json::to_string`), mesuré tel quel, croissance du tampon comprise :
    let reponse = mesure(|| serde_json::to_string(&contenu).unwrap());
    let f = contenu.len();
    let f_echappe = reponse.valeur.len();
    let car_fichier = contenu.encode_utf16().count();
    let car_reponse = reponse.valeur.encode_utf16().count();
    let hors_latin1 = contenu.chars().any(|c| c as u32 > 0xff);
    println!("FORME D'ORIGINE (load_crawl) — {n} pages");
    println!("   fichier lu en entier (read_to_string)            : {}", mo(f as isize));
    println!(
        "   réponse sérialisée par Tauri (chaîne échappée)    : {} ({} caractères), {} alloués",
        mo(f_echappe as isize),
        car_reponse,
        mo(reponse.gardes)
    );
    println!("   copie de wry pour WebView2 (SHCreateMemStream)   : {}", mo(f_echappe as isize));
    println!("   pic du moteur, au moins : fichier + réponse        = {}", mo(f as isize + reponse.gardes));
    let octets = if hors_latin1 { 2 } else { 1 };
    println!(
        "   fenêtre : texte de la réponse {} + chaîne du fichier {} (V8, {} octet(s) par caractère) + toutes les pages en objets",
        mo((octets * car_reponse) as isize),
        mo((octets * car_fichier) as isize),
        octets
    );
    println!(
        "   longueur de la réponse : {:.0} % de la longueur maximale d'une chaîne V8 ; {}",
        100.0 * car_reponse as f64 / V8_LONGUEUR_MAX as f64,
        if car_reponse > V8_LONGUEUR_MAX { "AU-DELÀ : le crawl ne peut PAS s'ouvrir" } else { "en deçà" }
    );
    (f, f_echappe)
}

struct Flux {
    messages: usize,
    plus_gros: usize,
    pages: usize,
    pic: isize,
    pic_apercu: isize,
}

fn forme_actuelle<R: Read>(lecteur: impl Fn() -> R, n_attendu: usize) -> Flux {
    let apercu = mesure(|| read_info(BufReader::with_capacity(1 << 20, lecteur())).unwrap());
    assert_eq!(apercu.valeur.count, n_attendu);
    let mut messages = 0;
    let mut plus_gros = 0;
    let lu = mesure(|| {
        read_pages(BufReader::with_capacity(1 << 20, lecteur()), MESSAGE_BYTES, |m| {
            messages += 1;
            plus_gros = plus_gros.max(m.len());
            true // le message part à la fenêtre (ici : il est libéré)
        })
        .unwrap()
    });
    assert_eq!(lu.valeur.count, n_attendu);
    Flux { messages, plus_gros, pages: lu.valeur.count, pic: lu.pic, pic_apercu: apercu.pic }
}

fn afficher(f: &Flux, titre: &str) {
    println!("FORME ACTUELLE (lecture en flux) — {titre}");
    println!("   aperçu pour le dialogue « Nouveau / Revoir » : pic {} (aucune page chargée)", mo(f.pic_apercu));
    println!("   pages : {} messages de {} au plus, {} pages", f.messages, mo(f.plus_gros as isize), f.pages);
    println!("   pic du lecteur : {} ; pic du moteur, au plus : lecteur + 2 messages en attente = {}", mo(f.pic), mo(f.pic + 2 * f.plus_gros as isize));
    println!("   fenêtre : un message à la fois ({}), puis ses pages en objets", mo(f.plus_gros as isize));
}

#[test]
fn ouverture_2000_pages() {
    let (f, f_echappe) = forme_d_origine(2000);
    assert!(f_echappe > f + f / 20, "l'échappement ajoute plus de 5 % : {f} → {f_echappe}");
    let flux = forme_actuelle(|| FichierEnFlux::new(2000), 2000);
    afficher(&flux, "2000 pages");
    // Garde-fous : le lecteur ne tient jamais le fichier ; un message reste sous
    // sa taille visée + une page ; l'aperçu ne coûte presque rien.
    assert!(flux.pic < (MESSAGE_BYTES as isize) * 3, "pic du lecteur : {}", flux.pic);
    assert!(flux.plus_gros < MESSAGE_BYTES + 100_000);
    assert!(flux.pic_apercu < 3 << 20, "pic de l'aperçu : {}", flux.pic_apercu);
    assert!(flux.messages >= f / MESSAGE_BYTES);
}

#[test]
#[ignore = "lourd : 50 000 pages puis 35 000 pages (un fichier de 415 Mo)"]
fn ouverture_50_000_pages_et_415_mo() {
    for n in [50_000, PAGES_POUR_415_MO] {
        forme_d_origine(n);
        let flux = forme_actuelle(|| FichierEnFlux::new(n), n);
        afficher(&flux, &format!("{n} pages"));
        assert!(flux.pic < (MESSAGE_BYTES as isize) * 3);
    }
}

/// FNV-1a 64 bits, en continu.
struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }
    fn ajoute(&mut self, octets: &[u8]) {
        for b in octets {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

/// Un VRAI crawl enregistré (LU seulement, jamais écrit) : aperçu, lecture en
/// flux, et preuve que les pages remises sont, octet pour octet, celles du
/// fichier (empreinte de la liste `pages` du fichier = empreinte des messages
/// recollés).
#[test]
#[ignore = "lancé à la main : SEO_SPIDER_VRAI_CRAWL=<chemin du fichier>"]
fn vrai_crawl_enregistre() {
    let chemin = std::env::var("SEO_SPIDER_VRAI_CRAWL").expect("SEO_SPIDER_VRAI_CRAWL");
    let ouvrir = || std::fs::File::open(&chemin).unwrap();
    let debut = std::time::Instant::now();
    let apercu = mesure(|| read_info(BufReader::with_capacity(1 << 20, ouvrir())).unwrap());
    let duree_apercu = debut.elapsed();
    let mut empreinte = Fnv::new();
    let mut messages = 0usize;
    let mut plus_gros = 0usize;
    let debut = std::time::Instant::now();
    let lu = mesure(|| {
        read_pages(BufReader::with_capacity(1 << 20, ouvrir()), MESSAGE_BYTES, |m| {
            let pages = m.strip_prefix("{\"pages\":[").unwrap().strip_suffix("]}").unwrap();
            if messages > 0 {
                empreinte.ajoute(b",");
            }
            empreinte.ajoute(pages.as_bytes());
            messages += 1;
            plus_gros = plus_gros.max(m.len());
            true
        })
        .unwrap()
    });
    let duree = debut.elapsed();
    // Référence : la liste `pages` du fichier, prise telle quelle entre ses crochets.
    let brut = std::fs::read(&chemin).unwrap();
    let ouvre = brut.windows(9).position(|w| w == b"\"pages\":[").unwrap() + 9;
    let ferme = brut.windows(17).rposition(|w| w == b"],\"sitemap_urls\":").unwrap();
    let mut reference = Fnv::new();
    reference.ajoute(&brut[ouvre..ferme]);
    println!("VRAI CRAWL ENREGISTRÉ — {} ({} octets)", chemin, brut.len());
    println!("   aperçu : {} pages, pic {}, {:.1} s (profil debug)", apercu.valeur.count, mo(apercu.pic), duree_apercu.as_secs_f64());
    println!(
        "   lecture en flux : {} pages en {} messages de {} au plus, pic du lecteur {}, {:.1} s (profil debug)",
        lu.valeur.count, messages, mo(plus_gros as isize), mo(lu.pic), duree.as_secs_f64()
    );
    println!(
        "   empreinte des pages remises {:016x} / de la liste du fichier {:016x} → {}",
        empreinte.0,
        reference.0,
        if empreinte.0 == reference.0 { "IDENTIQUES" } else { "DIFFÉRENTES" }
    );
    assert_eq!(apercu.valeur.count, lu.valeur.count);
    assert_eq!(empreinte.0, reference.0, "les pages remises diffèrent du fichier");
    assert!(lu.pic < (MESSAGE_BYTES as isize) * 3);
}

/// Écrit, pour l'essai dans Chromium (`banc-moteur/memoire/`), les réponses que
/// la fenêtre recevait (forme d'origine) et celles qu'elle reçoit (forme
/// actuelle), pour 35 000 et 50 000 pages synthétiques.
#[test]
#[ignore = "lancé à la main : SEO_SPIDER_BANC=<dossier>"]
fn ecrire_les_reponses_pour_la_fenetre() {
    let dossier = std::path::PathBuf::from(std::env::var("SEO_SPIDER_BANC").expect("SEO_SPIDER_BANC"));
    std::fs::create_dir_all(&dossier).unwrap();
    for n in [PAGES_POUR_415_MO, 50_000] {
        let contenu = fichier_de_crawl(n);
        std::fs::write(dossier.join(format!("load_crawl-{n}.json")), serde_json::to_string(&contenu).unwrap()).unwrap();
        let info = read_info(contenu.as_bytes()).unwrap();
        std::fs::write(dossier.join(format!("info-{n}.json")), serde_json::to_string(&info).unwrap()).unwrap();
        let sous = dossier.join(format!("messages-{n}"));
        std::fs::create_dir_all(&sous).unwrap();
        let mut k = 0usize;
        let fin = read_pages(contenu.as_bytes(), MESSAGE_BYTES, |m| {
            std::fs::write(sous.join(format!("{k:04}.json")), m).unwrap();
            k += 1;
            true
        })
        .unwrap();
        std::fs::write(sous.join(format!("{k:04}.json")), serde_json::to_string(&fin).unwrap()).unwrap();
        println!("{n} pages : fichier {} octets, {} messages de pages + 1 message de fin dans {}", contenu.len(), k, sous.display());
    }
}
