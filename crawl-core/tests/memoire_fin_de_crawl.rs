//! Point 8 du chantier « après les langues » : ce que coûte, à la fin d'un
//! crawl, le résumé envoyé à la fenêtre (`crawl:done`).
//!
//! FORME D'ORIGINE (jusqu'au 14/11/2026), reproduite ici à l'identique :
//!   - pendant tout le crawl, chaque page était gardée EN ENTIER dans
//!     `summary.pages` (`merge_into_summary`), et elle y restait APRÈS le crawl,
//!     l'état partagé étant conservé pour une reprise (`CrawlState`) ;
//!   - à la fin, `run_shared` CLONAIT ce résumé, pages comprises ;
//!   - `commands.rs` l'envoyait dans `crawl:done` : Tauri le sérialise en JSON,
//!     l'enveloppe dans un script, puis WebView2 le reçoit en UTF-16 ;
//!   - l'interface n'en lisait que `sitemap_urls` : elle avait déjà reçu chaque
//!     page, une par une, par `crawl:page`.
//!
//!   cargo test -p crawl-core --test memoire_fin_de_crawl -- --nocapture
//!   cargo test -p crawl-core --test memoire_fin_de_crawl -- --ignored --nocapture   (50 000 pages et 415 Mo)

mod commun;

use commun::{mesure, mo, page_synthetique, url_page, PAGES_POUR_415_MO};
use crawl_core::report::CrawlSummary;

/// Enveloppe que Tauri 2.11 met autour d'un événement (`event::emit_js_script`),
/// hors charge utile : ~110 caractères, sans effet sur les ordres de grandeur.
const ENVELOPPE_SCRIPT: usize = 110;

/// Longueur maximale d'une chaîne V8 64 bits (2^29 - 24 caractères) : WebView2
/// ne peut pas tenir un script plus long.
const V8_LONGUEUR_MAX: usize = 536_870_888;

struct Chiffres {
    pages: usize,
    json_par_page: usize,
    /// Octets que le moteur garde pendant (et après) le crawl, pour le résumé.
    garde: isize,
    /// Octets alloués par le clone de fin de crawl.
    clone: isize,
    /// Taille de la charge utile de `crawl:done` (JSON, octets).
    message: usize,
    /// Caractères UTF-16 du script que WebView2 doit exécuter.
    script_utf16: usize,
}

impl Chiffres {
    fn afficher(&self, titre: &str) {
        println!("{titre} — {} pages ({} octets de JSON par page en moyenne)", self.pages, self.json_par_page);
        println!("   gardé par le moteur pour le résumé, pendant et après le crawl : {}", mo(self.garde));
        println!("   clone du résumé en fin de crawl                              : {}", mo(self.clone));
        println!("   message crawl:done (JSON)                                    : {}", mo(self.message as isize));
        println!(
            "   script à exécuter par WebView2 : {} caractères UTF-16 = {} ; {}",
            self.script_utf16,
            mo(2 * self.script_utf16 as isize),
            if self.script_utf16 > V8_LONGUEUR_MAX {
                "AU-DELÀ de la longueur maximale d'une chaîne V8 : le script ne peut PAS être exécuté".to_string()
            } else {
                format!("{:.0} % de la longueur maximale d'une chaîne V8", 100.0 * self.script_utf16 as f64 / V8_LONGUEUR_MAX as f64)
            }
        );
        println!(
            "   pic du moteur en fin de crawl, au moins : résumé + clone + JSON + script + UTF-16 = {}",
            mo(self.garde + self.clone + 2 * self.message as isize + 2 * self.script_utf16 as isize)
        );
    }
}

/// Résumé tel que la forme d'origine le bâtissait (`merge_into_summary`).
fn resume_forme_d_origine(n: usize) -> CrawlSummary {
    let mut s = CrawlSummary {
        pages: Vec::new(),
        total_bytes: 0,
        total_requests: 0,
        total_dynamic_requests: 0,
        tech_detected: Vec::new(),
        sitemap_urls: Vec::new(),
    };
    for i in 0..n {
        let report = page_synthetique(i);
        s.total_bytes = s.total_bytes.saturating_add(report.total_bytes);
        s.total_requests = s.total_requests.saturating_add(report.request_count);
        s.total_dynamic_requests = s.total_dynamic_requests.saturating_add(report.dynamic_requests);
        for t in &report.tech {
            if !s.tech_detected.iter().any(|x| x == t) {
                s.tech_detected.push(t.clone());
            }
        }
        s.pages.push(report);
    }
    s
}

/// Résumé tel que le moteur le bâtit AUJOURD'HUI : la vraie méthode
/// `CrawlSummary::tally`, appelée par `crawler::emit_report` ; la page part
/// ensuite à l'interface (`on_page`) et le moteur ne la garde pas.
fn resume_forme_actuelle(n: usize) -> CrawlSummary {
    let mut s = CrawlSummary::default();
    for i in 0..n {
        let report = page_synthetique(i);
        s.tally(&report);
        drop(report); // remise à `on_page`, émise par `crawl:page`, puis libérée
    }
    s
}

/// Mesure un résumé, pour `n` pages, construit par `construire`.
fn chiffres(n: usize, construire: fn(usize) -> CrawlSummary) -> Chiffres {
    let construit = mesure(|| construire(n));
    let resume = construit.valeur;
    let clone = mesure(|| resume.clone());
    drop(clone.valeur);
    let mut resume = resume;
    // L'interface lit les adresses du sitemap dans crawl:done : elles y restent.
    resume.sitemap_urls = (0..n).map(url_page).collect();
    let json = serde_json::to_string(&resume).unwrap();
    let script_utf16 = json.encode_utf16().count() + ENVELOPPE_SCRIPT;
    // Taille moyenne d'une page en JSON (celle d'un message crawl:page).
    let echantillon = n.min(2000);
    let json_pages: usize = (0..echantillon).map(|i| serde_json::to_string(&page_synthetique(i)).unwrap().len()).sum();
    Chiffres {
        pages: n,
        json_par_page: json_pages / echantillon.max(1),
        garde: construit.gardes,
        clone: clone.gardes,
        message: json.len(),
        script_utf16,
    }
}

fn comparer(n: usize) -> (Chiffres, Chiffres) {
    let avant = chiffres(n, resume_forme_d_origine);
    avant.afficher("FORME D'ORIGINE");
    let apres = chiffres(n, resume_forme_actuelle);
    apres.afficher("FORME ACTUELLE ");
    (avant, apres)
}

#[test]
fn fin_de_crawl_2000_pages() {
    let (avant, apres) = comparer(2000);
    // Calibrage des pages synthétiques : 11,3 Ko de JSON par page dans le
    // fichier de 415 Mo, 12,2 Ko en moyenne sur tous les crawls réels.
    assert!((10_500..=12_500).contains(&avant.json_par_page), "{} octets par page", avant.json_par_page);
    // Garde-fous : le moteur ne garde plus rien de proportionnel aux pages, et
    // crawl:done ne porte plus que des totaux et les adresses du sitemap.
    assert!(avant.garde > 2000 * 10_000, "la forme d'origine gardait les pages : {}", avant.garde);
    assert!(apres.garde < 1000, "résumé : {} octets gardés", apres.garde);
    assert!(apres.clone < 1000, "clone : {} octets", apres.clone);
    let sitemap: usize = (0..2000).map(|i| url_page(i).len() + 3).sum();
    assert!(apres.message < sitemap + 300, "crawl:done : {} octets pour {} d'adresses", apres.message, sitemap);
}

#[test]
#[ignore = "lourd : 50 000 pages puis 35 000 pages (un fichier de 415 Mo), plusieurs Go de mémoire"]
fn fin_de_crawl_50_000_pages_et_415_mo() {
    for n in [50_000, PAGES_POUR_415_MO] {
        let (avant, apres) = comparer(n);
        assert!(apres.garde < 1000 && apres.clone < 1000);
        assert!(apres.script_utf16 < avant.script_utf16 / 50);
    }
}
