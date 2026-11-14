//! Point 36 du chantier « après les langues » : ENREGISTRER un très gros crawl.
//!
//! FORME D'ORIGINE (jusqu'au 14/11/2026), reproduite ici à l'identique côté
//! moteur : la fenêtre fabriquait la chaîne du crawl entier
//! (`JSON.stringify({ seed_url, date, pages, sitemap_urls })`), puis
//! `invoke("save_crawl", { id, json, dir })` ; Tauri reçoit le corps de la
//! requête (les arguments en JSON, la chaîne `json` ÉCHAPPÉE), le décode en
//! entier (`serde_json::from_slice::<Value>`), en copie la chaîne dans
//! l'argument `json: String`, et `save_crawl` l'écrivait d'un coup
//! (`std::fs::write`), sur le fil principal.
//!
//! FORME ACTUELLE : la fenêtre fabrique le même texte page par page et le remet
//! par morceaux d'au plus 4 194 304 caractères (`saveCurrentCrawl`, reproduit
//! ici à l'identique : `morceaux_comme_la_fenetre`) ; le moteur les écrit avec
//! la vraie `savedcrawl::SaveSessions` (temporaire puis renommage).
//!
//!   cargo test -p crawl-core --test memoire_enregistrement -- --nocapture
//!   cargo test -p crawl-core --test memoire_enregistrement -- --ignored --nocapture   (35 000, 50 000, 200 000 pages)

mod commun;

use std::time::{Duration, Instant};

use commun::{fichier_de_crawl, mesure, mo, page_json, url_page, PAGES_POUR_415_MO};
use crawl_core::savedcrawl::SaveSessions;

/// Taille d'un morceau, en caractères (unités UTF-16 de JavaScript) : la même
/// que `SAVE_PART_CHARS` dans main.js.
const MORCEAU: usize = 4_194_304;

/// Le texte du fichier, page par page, coupé en morceaux d'au plus `MORCEAU`
/// caractères UTF-16 — EXACTEMENT l'algorithme de `saveCurrentCrawl`
/// (remplir le tampon jusqu'à un morceau plein, envoyer les morceaux pleins,
/// tout le reste à la fin, jamais une paire de substitution coupée).
fn morceaux_comme_la_fenetre(n: usize, date: &str, mut envoyer: impl FnMut(String)) {
    let mut buf = format!(
        "{{\"seed_url\":{},\"date\":{},\"pages\":[",
        serde_json::to_string("https://forum-synthetique.example/").unwrap(),
        serde_json::to_string(date).unwrap()
    );
    let mut unites = buf.encode_utf16().count();
    let (mut i, mut j, mut phase) = (0usize, 0usize, 0u8);
    loop {
        // fill()
        while unites < MORCEAU && phase < 2 {
            let ajout = if phase == 0 {
                if i < n {
                    let s = (if i > 0 { "," } else { "" }).to_string() + &page_json(i);
                    i += 1;
                    s
                } else {
                    phase = 1;
                    "],\"sitemap_urls\":[".to_string()
                }
            } else if j < n {
                let s = (if j > 0 { "," } else { "" }).to_string() + &serde_json::to_string(&url_page(j)).unwrap();
                j += 1;
                s
            } else {
                phase = 2;
                "]}".to_string()
            };
            unites += ajout.encode_utf16().count();
            buf.push_str(&ajout);
        }
        // flush()
        while unites >= MORCEAU || (phase == 2 && unites > 0) {
            let (mut coupe, mut u) = (buf.len(), 0usize);
            for (k, c) in buf.char_indices() {
                if u + c.len_utf16() > MORCEAU {
                    coupe = k; // (un caractère hors BMP n'est jamais coupé en deux)
                    break;
                }
                u += c.len_utf16();
            }
            let reste = buf.split_off(coupe);
            envoyer(std::mem::replace(&mut buf, reste));
            unites -= u;
        }
        if phase == 2 {
            return;
        }
    }
}

struct Actuelle {
    octets: u64,
    caracteres: usize,
    morceaux: usize,
    plus_long: usize,
    plus_lourd: usize,
    pic_ecriture: isize,
    temps_moteur: Duration,
}

/// La forme actuelle, côté moteur : la vraie SaveSessions reçoit les morceaux.
fn forme_actuelle(n: usize, dossier: &std::path::Path, date: &str) -> (Actuelle, std::path::PathBuf) {
    let chemin = dossier.join("https___forum-synthetique.example_.json");
    let sessions = SaveSessions::default();
    let mut a = Actuelle { octets: 0, caracteres: 0, morceaux: 0, plus_long: 0, plus_lourd: 0, pic_ecriture: 0, temps_moteur: Duration::ZERO };
    let t = Instant::now();
    let id = sessions.begin(&chemin).unwrap();
    a.temps_moteur += t.elapsed();
    morceaux_comme_la_fenetre(n, date, |m| {
        let unites = m.encode_utf16().count();
        a.caracteres += unites;
        a.morceaux += 1;
        a.plus_long = a.plus_long.max(unites);
        a.plus_lourd = a.plus_lourd.max(m.len());
        let position = a.octets;
        a.octets += m.len() as u64;
        // Ce que le moteur alloue pour écrire ce morceau (hors corps de la
        // requête : le morceau arrive déjà en octets, pris tel quel).
        let octets = m.into_bytes();
        let t = Instant::now();
        let w = mesure(|| sessions.write(id, position, octets).unwrap());
        a.temps_moteur += t.elapsed();
        a.pic_ecriture = a.pic_ecriture.max(w.pic);
    });
    let t = Instant::now();
    sessions.end(id, a.octets).unwrap();
    a.temps_moteur += t.elapsed();
    (a, chemin)
}

fn afficher_actuelle(n: usize, a: &Actuelle) {
    println!("FORME ACTUELLE (en morceaux) — {n} pages, fichier de {}", mo(a.octets as isize));
    println!(
        "   fenêtre : texte du crawl {} caractères ({:.0} % de la limite V8 s'il était d'un bloc : {}), jamais tenu entier",
        a.caracteres,
        100.0 * a.caracteres as f64 / V8_LONGUEUR_MAX as f64,
        if a.caracteres > V8_LONGUEUR_MAX { "la forme d'origine ne pouvait PAS l'enregistrer" } else { "la forme d'origine passait" }
    );
    println!(
        "   messages : {} morceaux, le plus long {} caractères ({:.2} % de la limite V8), le plus lourd {} octets",
        a.morceaux,
        a.plus_long,
        100.0 * a.plus_long as f64 / V8_LONGUEUR_MAX as f64,
        a.plus_lourd
    );
    println!(
        "   moteur : écriture d'un morceau, allocations {} au plus ; + le morceau reçu (corps de la requête) et sa copie pour l'écriture : pic ≤ {} ; temps du moteur {:.1} s (écriture, sync_all, renommage ; profil debug)",
        mo(a.pic_ecriture),
        mo(a.pic_ecriture + 2 * a.plus_lourd as isize),
        a.temps_moteur.as_secs_f64()
    );
}

/// Longueur maximale d'une chaîne V8 64 bits (2^29 - 24 caractères).
const V8_LONGUEUR_MAX: usize = 536_870_888;

fn dossier_de_test(nom: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("seo-spider-enregistrement-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// La forme d'origine, côté moteur, pour un crawl dont le fichier est `texte`.
/// Rend (pic du moteur, durée, longueur de l'argument échappé en caractères).
fn forme_d_origine(texte: &str, dossier: &std::path::Path) -> (isize, f64, usize) {
    // Ce que la fenêtre envoyait : JSON.stringify({ id, json, dir }) (même échappement).
    let corps = serde_json::to_vec(&serde_json::json!({ "id": "https://forum-synthetique.example/", "json": texte, "dir": null })).unwrap();
    let caracteres = String::from_utf8_lossy(&corps).encode_utf16().count();
    let chemin = dossier.join("https___forum-synthetique.example_.json");
    let debut = Instant::now();
    let m = mesure(|| {
        // Tauri : le corps entier décodé en valeur JSON…
        let valeur: serde_json::Value = serde_json::from_slice(&corps).unwrap();
        // … puis l'argument `json: String` désérialisé depuis cette valeur (copie).
        let json: String = serde_json::from_value(valeur["json"].clone()).unwrap();
        drop(valeur);
        // save_crawl : écriture d'un bloc.
        std::fs::write(&chemin, json).unwrap();
    });
    let duree = debut.elapsed().as_secs_f64();
    assert_eq!(std::fs::read(&chemin).unwrap().len(), texte.len());
    // Le corps de la requête, reçu par le moteur, vit pendant tout ce temps.
    (m.pic + corps.len() as isize, duree, caracteres)
}

fn afficher_origine(n: usize, texte: &str, pic: isize, duree: f64, caracteres: usize) {
    let car_texte = texte.encode_utf16().count();
    println!("FORME D'ORIGINE (save_crawl) — {n} pages, fichier de {}", mo(texte.len() as isize));
    println!(
        "   fenêtre : chaîne du crawl {car_texte} caractères ({:.0} % de la limite V8), argument d'invoke échappé {caracteres} caractères ({:.0} %) ; {}",
        100.0 * car_texte as f64 / V8_LONGUEUR_MAX as f64,
        100.0 * caracteres as f64 / V8_LONGUEUR_MAX as f64,
        if car_texte > V8_LONGUEUR_MAX || caracteres > V8_LONGUEUR_MAX { "AU-DELÀ : rien n'est enregistré" } else { "en deçà" }
    );
    println!("   moteur : corps reçu + valeur JSON + copie de l'argument + écriture : pic {}, {:.1} s (profil debug)", mo(pic), duree);
}

#[test]
fn enregistrement_forme_d_origine_2000_pages() {
    let dossier = dossier_de_test("origine-2000");
    let texte = fichier_de_crawl(2000);
    let (pic, duree, caracteres) = forme_d_origine(&texte, &dossier);
    afficher_origine(2000, &texte, pic, duree, caracteres);
    assert!(pic as usize > 2 * texte.len(), "la forme d'origine tenait au moins deux fois le crawl");
    std::fs::remove_dir_all(&dossier).unwrap();
}

#[test]
#[ignore = "lourd : 35 000 pages (un fichier de 415 Mo)"]
fn enregistrement_forme_d_origine_415_mo() {
    let dossier = dossier_de_test("origine-415");
    let texte = fichier_de_crawl(PAGES_POUR_415_MO);
    let (pic, duree, caracteres) = forme_d_origine(&texte, &dossier);
    afficher_origine(PAGES_POUR_415_MO, &texte, pic, duree, caracteres);
    std::fs::remove_dir_all(&dossier).unwrap();
}

/// Date des fichiers synthétiques (`commun::fichier_de_crawl`) : la même ici,
/// pour comparer les deux chemins au caractère près.
const DATE: &str = "2026-07-30T10:59:00.000Z";

/// Compare deux fichiers ; en cas d'écart, montre où (sans déverser des Mo).
fn compare(ecrit: &[u8], ancien: &[u8]) {
    if ecrit == ancien {
        return;
    }
    let i = ecrit.iter().zip(ancien).position(|(a, b)| a != b).unwrap_or(ecrit.len().min(ancien.len()));
    let autour = |v: &[u8]| String::from_utf8_lossy(&v[i.saturating_sub(80)..(i + 80).min(v.len())]).to_string();
    panic!(
        "fichiers différents : {} octets écrits contre {} ; premier écart à l'octet {i}\n  écrit : {}\n  ancien : {}",
        ecrit.len(),
        ancien.len(),
        autour(ecrit),
        autour(ancien)
    );
}

/// Le même crawl enregistré par l'ancien chemin (une chaîne, `std::fs::write`)
/// et par le nouveau (morceaux de la fenêtre, SaveSessions) : fichiers
/// IDENTIQUES octet pour octet ; garde-fous de taille et de mémoire.
#[test]
fn enregistrement_en_morceaux_2000_pages_meme_fichier() {
    let dossier = dossier_de_test("actuelle-2000");
    let (a, chemin) = forme_actuelle(2000, &dossier, DATE);
    afficher_actuelle(2000, &a);
    let ancien = fichier_de_crawl(2000);
    compare(&std::fs::read(&chemin).unwrap(), ancien.as_bytes());
    assert!(a.morceaux >= 5, "{} morceaux pour 23 Mo", a.morceaux);
    assert!(a.plus_long <= MORCEAU);
    // Moteur : rien de proportionnel au crawl (le tampon d'écriture de 1 Mo est pris au début).
    assert!(a.pic_ecriture < 1 << 20, "écriture d'un morceau : {} octets", a.pic_ecriture);
    // Plus aucun temporaire dans le dossier.
    let reste: Vec<_> = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(reste.len(), 1, "{reste:?}");
    std::fs::remove_dir_all(&dossier).unwrap();
}

/// 35 000 (415 Mo), 50 000 et 200 000 pages (la limite de l'application). Le
/// fichier de 415 Mo est comparé à celui de l'ancien chemin ; celui de 200 000
/// pages est relu en flux (`read_info`) : un crawl valide, toutes ses pages.
#[test]
#[ignore = "lourd : 35 000, 50 000 puis 200 000 pages (2,4 Go écrits sur le disque)"]
fn enregistrement_en_morceaux_35_000_50_000_200_000_pages() {
    for n in [PAGES_POUR_415_MO, 50_000, 200_000] {
        let dossier = dossier_de_test(&format!("actuelle-{n}"));
        let (a, chemin) = forme_actuelle(n, &dossier, DATE);
        afficher_actuelle(n, &a);
        assert!(a.plus_long <= MORCEAU && a.pic_ecriture < 1 << 20);
        if n == PAGES_POUR_415_MO {
            let ancien = fichier_de_crawl(n);
            let ecrit = std::fs::read(&chemin).unwrap();
            println!("   comparaison au fichier de l'ancien chemin : {}", if ecrit == ancien.as_bytes() { "IDENTIQUE" } else { "DIFFÉRENT" });
            compare(&ecrit, ancien.as_bytes());
        }
        let info = crawl_core::savedcrawl::read_info(std::io::BufReader::with_capacity(1 << 20, std::fs::File::open(&chemin).unwrap())).unwrap();
        println!("   relu par read_info : {} pages", info.count);
        assert_eq!(info.count, n);
        std::fs::remove_dir_all(&dossier).unwrap();
    }
}
