//! Coût mémoire des champs que le chantier i18n a ajoutés aux problèmes SEO
//! (`Issue::code`, `Issue::params`), mesuré sur les VRAIS types du moteur.
//!
//! Quand ce test a été écrit (13/11/2026), le moteur gardait chaque page en
//! entier dans `summary.pages` pendant tout le crawl, puis la clonait à la fin :
//! quelques centaines d'octets de plus par problème faisaient des centaines de
//! Mo à 200 000 pages. Depuis le 14/11/2026, il ne garde plus les pages (cf.
//! `CrawlSummary::tally`, `tests/memoire_fin_de_crawl.rs`) : un problème ne vit
//! plus que le temps de sa page (au plus une page par thread du crawl). Le test
//! reste un garde-fou sur la taille d'un problème.
//!
//! Test d'intégration = binaire À PART : son allocateur compteur ne touche ni la
//! bibliothèque ni ses autres tests. Compteurs par thread (les tests tournent en
//! parallèle) : seul compte ce que le thread du test alloue et garde.
//!
//!   cargo test -p crawl-core --test memoire_params -- --nocapture

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use crawl_core::codes::params;
use crawl_core::report::{Issue, PageReport, SecurityData, SeoData, TextParams};
use serde_json::Value;

struct Compteur;

thread_local! {
    static OCTETS: Cell<isize> = const { Cell::new(0) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Compteur {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = OCTETS.try_with(|c| c.set(c.get() + l.size() as isize));
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let _ = OCTETS.try_with(|c| c.set(c.get() - l.size() as isize));
        System.dealloc(p, l)
    }
}

#[global_allocator]
static ALLOCATEUR: Compteur = Compteur;

/// Octets ALLOUÉS ET GARDÉS par `f` (ce que retient sa valeur de retour), et
/// nombre d'allocations faites.
fn mesure<T>(f: impl FnOnce() -> T) -> (isize, usize, T) {
    let o0 = OCTETS.with(|c| c.get());
    let a0 = ALLOCS.with(|c| c.get());
    let v = f();
    (OCTETS.with(|c| c.get()) - o0, ALLOCS.with(|c| c.get()) - a0, v)
}

/// Page TYPE des crawls de Chris (3,3 problèmes par page, presque tous à un
/// paramètre) : trois en-têtes de sécurité manquants et un title trop long.
fn page_type() -> PageReport {
    PageReport {
        url: "https://www.exemple-de-site-vitrine.fr/une-page-de-service-a-lyon/".to_string(),
        status: 200,
        seo: Some(SeoData {
            title: Some("x".repeat(72)),
            title_len: 72,
            meta_description: Some("y".repeat(120)),
            meta_description_len: 120,
            h1_count: 1,
            canonical: Some("https://www.exemple-de-site-vitrine.fr/une-page-de-service-a-lyon/".to_string()),
            structured_data_valid: true,
            ..Default::default()
        }),
        security: Some(SecurityData {
            https: true,
            mixed_content: Vec::new(),
            missing_headers: vec![
                "Content-Security-Policy".to_string(),
                "Referrer-Policy".to_string(),
                "X-Frame-Options".to_string(),
            ],
        }),
        ..Default::default()
    }
}

/// Octets du tampon d'un Vec (sa capacité × la taille d'un élément).
fn tampon<T>(v: &Vec<T>) -> isize {
    (v.capacity() * std::mem::size_of::<T>()) as isize
}

/// Octets que retient UN champ de tous les problèmes (le clone d'un champ retient
/// exactement ce que le champ retient ; le Vec qui reçoit les clones est retiré).
fn champ<T: Clone>(toutes: &[Vec<Issue>], f: impl Fn(&Issue) -> &T) -> isize {
    let (o, _, copies) = mesure(|| toutes.iter().flatten().map(|i| f(i).clone()).collect::<Vec<T>>());
    o - tampon(&copies)
}

struct Mesures {
    params_header: (isize, usize),
    params_len: (isize, usize),
    par_page_issues: isize,
    par_page_code: isize,
    par_page_params: isize,
    problemes_par_page: usize,
    /// Pour information : champs d'AVANT le chantier (url, severity, category, message).
    par_page_anciens: [isize; 4],
}

fn mesurer() -> Mesures {
    let (o_h, a_h, p_h) = mesure(|| params(&[("header", Value::from("X-Frame-Options"))]));
    let (o_l, a_l, p_l) = mesure(|| params(&[("len", Value::from(72))]));
    drop((p_h, p_l));

    // 1 000 pages type : ce que les problèmes retiennent, au total puis champ par champ.
    // (Le clone d'un champ retient exactement ce que le champ retient.)
    let page = page_type();
    let (o_issues, _, toutes) = mesure(|| {
        (0..1000).map(|_| crawl_core::seo::analyze(&page)).collect::<Vec<Vec<Issue>>>()
    });
    let n_problemes: usize = toutes.iter().map(|v| v.len()).sum();
    let o_issues = o_issues - tampon(&toutes); // le Vec des 1 000 pages n'est pas un coût
    let o_code = champ(&toutes, |i| &i.code);
    let o_params = champ(&toutes, |i| &i.params);
    let anciens = [
        champ(&toutes, |i| &i.url) / 1000,
        champ(&toutes, |i| &i.severity) / 1000,
        champ(&toutes, |i| &i.category) / 1000,
        champ(&toutes, |i| &i.message) / 1000,
    ];

    Mesures {
        params_header: (o_h, a_h),
        params_len: (o_l, a_l),
        par_page_issues: o_issues / 1000,
        par_page_code: o_code / 1000,
        par_page_params: o_params / 1000,
        problemes_par_page: n_problemes / 1000,
        par_page_anciens: anciens,
    }
}

#[test]
fn cout_memoire_des_parametres_et_des_codes() {
    let m = mesurer();
    println!();
    println!("size_of::<Issue>()      = {} octets", std::mem::size_of::<Issue>());
    println!("size_of::<TextParams>() = {} octets", std::mem::size_of::<TextParams>());
    println!("params {{header: \"X-Frame-Options\"}} : {} octets gardés, {} allocations", m.params_header.0, m.params_header.1);
    println!("params {{len: 72}}                   : {} octets gardés, {} allocations", m.params_len.0, m.params_len.1);
    println!(
        "page type ({} problèmes) : {} octets gardés par ses problèmes, dont code {} et params {}",
        m.problemes_par_page, m.par_page_issues, m.par_page_code, m.par_page_params
    );
    println!(
        "à 200 000 pages : problèmes {:.0} Mo, dont code {:.0} Mo et params {:.0} Mo",
        m.par_page_issues as f64 * 200_000.0 / 1e6,
        m.par_page_code as f64 * 200_000.0 / 1e6,
        m.par_page_params as f64 * 200_000.0 / 1e6
    );
    let [url, severite, categorie, message] = m.par_page_anciens;
    println!(
        "pour information, champs d'avant le chantier, par page : url {url}, severity {severite}, category {categorie}, message {message} ; reste (tableau des problèmes, marge de capacité des phrases) {}",
        m.par_page_issues - m.par_page_code - m.par_page_params - url - severite - categorie - message
    );

    // Garde-fous (13/11/2026, constat P2-1 de la relecture) : avec un BTreeMap,
    // {header} gardait 653 octets et {len} 635 ; le code d'un problème, sa
    // longueur en octets. Aujourd'hui : une paire (56 octets) + la valeur.
    assert!(m.params_header.0 <= 96, "params {{header}} : {} octets", m.params_header.0);
    assert!(m.params_len.0 <= 64, "params {{len}} : {} octets", m.params_len.0);
    assert_eq!(m.par_page_code, 0, "le code d'un problème ne doit rien allouer");
    assert!(
        m.par_page_params <= 100 * m.problemes_par_page as isize,
        "params : {} octets pour {} problèmes",
        m.par_page_params,
        m.problemes_par_page
    );
}
