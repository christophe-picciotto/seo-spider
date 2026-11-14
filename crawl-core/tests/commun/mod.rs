//! Outils partagés des mesures de mémoire (tests d'intégration du moteur).
//!
//! - Un allocateur qui COMPTE, par thread : octets vivants, pic, nombre
//!   d'allocations. Chaque binaire de test qui inclut ce module (`mod commun;`)
//!   l'installe pour lui seul : la bibliothèque et ses autres tests n'en voient
//!   rien. Compteurs par thread : seul compte ce que le thread du test alloue.
//! - Des pages SYNTHÉTIQUES calibrées sur les crawls réels de Chris
//!   (statistiques du 14/11/2026 sur 105 fichiers, 49 325 pages : en moyenne
//!   50 liens sortants, 6,7 H2, 2,3 blocs JSON-LD de ~870 caractères, 9,8 og,
//!   5,8 twitter, 3,3 problèmes, 2,4 images ; 12,2 Ko de JSON par page, 11,3 Ko
//!   dans le plus gros fichier, un forum : 36 679 pages, 415 Mo).
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;

use crawl_core::report::{
    AssetReport, CacheData, CacheLayer, Hreflang, LinkInfo, PageReport, SecurityData, SeoData, TypeStat,
    UrlFlags,
};

// ---------------------------------------------------------------------------
// Allocateur compteur
// ---------------------------------------------------------------------------

pub struct Compteur;

thread_local! {
    static VIVANTS: Cell<isize> = const { Cell::new(0) };
    static PIC: Cell<isize> = const { Cell::new(0) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn ajoute(delta: isize) {
    let _ = VIVANTS.try_with(|c| {
        let v = c.get() + delta;
        c.set(v);
        let _ = PIC.try_with(|p| {
            if v > p.get() {
                p.set(v)
            }
        });
    });
}

unsafe impl GlobalAlloc for Compteur {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ajoute(l.size() as isize);
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        ajoute(-(l.size() as isize));
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        ajoute(new_size as isize - l.size() as isize);
        System.realloc(p, l, new_size)
    }
}

#[global_allocator]
static ALLOCATEUR: Compteur = Compteur;

/// Octets vivants sur ce thread.
pub fn vivants() -> isize {
    VIVANTS.with(|c| c.get())
}

/// Résultat d'une mesure : ce que la valeur rendue GARDE, et le PIC atteint
/// pendant le calcul (au-dessus de l'état de départ), en octets.
pub struct Mesure<T> {
    pub gardes: isize,
    pub pic: isize,
    pub allocations: usize,
    pub valeur: T,
}

/// Mesure `f` : octets gardés par sa valeur de retour, et pic pendant son exécution.
pub fn mesure<T>(f: impl FnOnce() -> T) -> Mesure<T> {
    let depart = vivants();
    PIC.with(|p| p.set(depart));
    let a0 = ALLOCS.with(|c| c.get());
    let valeur = f();
    Mesure {
        gardes: vivants() - depart,
        pic: PIC.with(|p| p.get()) - depart,
        allocations: ALLOCS.with(|c| c.get()) - a0,
        valeur,
    }
}

/// Mo (millions d'octets) avec une décimale, pour les sorties.
pub fn mo(octets: isize) -> String {
    format!("{:.1} Mo", octets as f64 / 1e6)
}

// ---------------------------------------------------------------------------
// Pages synthétiques
// ---------------------------------------------------------------------------

/// Nombre de pages SYNTHÉTIQUES qui font un fichier de crawl de 415 Mo, la
/// taille du plus gros crawl réel de Chris (un forum : 415 061 804 octets,
/// 36 679 pages de 11,3 Ko). Les pages synthétiques pèsent 11,8 Ko.
pub const PAGES_POUR_415_MO: usize = 35_000;

const MOTS: [&str; 16] = [
    "thermographie", "infrarouge", "caméra", "isolation", "fuite", "toiture", "électricité", "diagnostic",
    "pont thermique", "humidité", "façade", "chauffage", "plancher", "menuiserie", "expertise", "contrôle",
];

fn mot(k: usize) -> &'static str {
    MOTS[k % MOTS.len()]
}

/// Adresse de la page `i` du site synthétique.
pub fn url_page(i: usize) -> String {
    format!("https://forum-synthetique.example/forum/{}/sujet-{:06}-{}/", mot(i / 7), i, mot(i).replace(' ', "-"))
}

/// Une page crawlée réaliste (tailles calibrées sur les crawls réels), avec ses
/// problèmes SEO calculés par le VRAI `seo::analyze`. Déterministe.
pub fn page_synthetique(i: usize) -> PageReport {
    let url = url_page(i);
    // Liens sortants : menu + fil de discussion (50 en moyenne dans les vrais
    // crawls, ~65 caractères de texte chacun).
    let outlinks: Vec<LinkInfo> = (0..50)
        .map(|k| {
            let cible = (i * 31 + k * 17) % 40_000;
            LinkInfo {
                target: format!("https://forum-synthetique.example/s/{cible:06}/"),
                anchor: format!("{} — l’avis {}", mot(k + 3), k),
                rel: if k % 9 == 0 { Some("nofollow".to_string()) } else { None },
                internal: k % 11 != 0,
            }
        })
        .collect();
    let images: Vec<AssetReport> = (0..2)
        .map(|k| AssetReport {
            url: format!("https://forum-synthetique.example/wp-content/uploads/2026/07/{}-{:06}-{k}.webp", mot(i + k).replace(' ', "-"), i),
            bytes: 40_000 + ((i * 13 + k) % 90_000) as u64,
            kind: "Image".to_string(),
        })
        .collect();
    let mut by_type = HashMap::new();
    for (k, nom) in ["Document", "Image", "Script", "Stylesheet"].iter().enumerate() {
        by_type.insert(nom.to_string(), TypeStat { bytes: 10_000 * (k as u64 + 1) + i as u64 % 5000, count: k as u32 + 1 });
    }
    let jsonld = |k: usize| {
        format!(
            "{{\"@context\":\"https://schema.org\",\"@type\":\"DiscussionForumPosting\",\"headline\":\"Question n°{i} sur la {} et la {}\",\
             \"author\":{{\"@type\":\"Person\",\"name\":\"Membre {k}\"}},\"datePublished\":\"2026-07-{:02}T10:00:00+02:00\",\
             \"text\":\"Bonjour à tous, j’ai une question à propos de la {} : la caméra montre une zone froide près de la {}. \
             Est-ce un pont thermique ou de l’humidité ? Merci d’avance pour vos réponses et vos conseils d’expert.\",\
             \"interactionStatistic\":{{\"@type\":\"InteractionCounter\",\"interactionType\":\"https://schema.org/CommentAction\",\"userInteractionCount\":{}}},\
             \"url\":\"{}\"}}",
            mot(i), mot(i + 5), 1 + i % 28, mot(i + 2), mot(i + 9), i % 40, url
        )
    };
    let og: HashMap<String, String> = [
        ("title", format!("Sujet {i} : {} — Forum de la thermographie", mot(i))),
        ("description", format!("Discussion sur la {} et la {} : réponses d’experts.", mot(i), mot(i + 1))),
        ("url", url.clone()),
        ("type", "article".to_string()),
        ("site_name", "Forum synthétique de la thermographie".to_string()),
        ("locale", "fr_FR".to_string()),
        ("image", format!("https://forum-synthetique.example/wp-content/uploads/2026/07/og-{i:06}.jpg")),
        ("image:width", "1200".to_string()),
        ("image:height", "630".to_string()),
        ("updated_time", "2026-07-30T12:59:00+02:00".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let twitter: HashMap<String, String> = [
        ("card", "summary_large_image".to_string()),
        ("title", format!("Sujet {i} : {}", mot(i))),
        ("description", format!("Discussion sur la {}.", mot(i))),
        ("image", format!("https://forum-synthetique.example/wp-content/uploads/2026/07/og-{i:06}.jpg")),
        ("label1", "Temps de lecture".to_string()),
        ("data1", "3 minutes".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let title = format!("Sujet {i} : {} — Forum de la thermographie", mot(i));
    let meta = format!(
        "Question sur la {} : la caméra thermique montre une zone froide. Réponses de membres et d’experts du forum, conseils et diagnostic.",
        mot(i)
    );
    // Une page sur trois a une image sans alt : 3,3 problèmes par page en
    // moyenne (les trois en-têtes de sécurité manquants, plus l'image).
    let sans_alt = if i % 3 == 0 { 1 } else { 0 };

    let mut page = PageReport {
        url: url.clone(),
        status: 200,
        tech: vec!["WordPress".to_string(), "Cloudflare".to_string()],
        total_bytes: 250_000 + (i % 100_000) as u64,
        by_type,
        request_count: 18,
        dynamic_requests: 0,
        vercel_cache_hits: 0,
        vercel_cache_misses: 0,
        images,
        measured_with: "http".to_string(),
        seo: Some(SeoData {
            title_len: title.chars().count(),
            title: Some(title),
            meta_description_len: meta.chars().count(),
            meta_description: Some(meta),
            h1: vec![format!("Sujet {i} : {} et {}", mot(i), mot(i + 1))],
            h1_count: 1,
            h2: (0..7).map(|k| format!("Réponse {k} : la {} en question", mot(i + k))).collect(),
            canonical: Some(url.clone()),
            meta_robots: Some("index, follow, max-image-preview:large".to_string()),
            noindex: false,
            nofollow: false,
            indexable: true,
            lang: Some("fr-FR".to_string()),
            viewport: Some("width=device-width, initial-scale=1".to_string()),
            charset: Some("UTF-8".to_string()),
            word_count: 900 + i % 400,
            text_ratio: 0.1234,
            og,
            twitter,
            images_total: 3,
            images_missing_alt: sans_alt,
            images_missing_alt_urls: (0..sans_alt)
                .map(|_| format!("https://forum-synthetique.example/wp-content/uploads/2026/07/avatar-{:04}.png", i % 3000))
                .collect(),
            internal_links: 42,
            external_links: 4,
            hreflang: vec![
                Hreflang { lang: "fr".to_string(), href: url.clone() },
                Hreflang { lang: "x-default".to_string(), href: url.clone() },
            ],
            structured_data_types: vec!["DiscussionForumPosting".to_string(), "InteractionCounter".to_string(), "Person".to_string()],
            structured_data_raw: vec![jsonld(0), jsonld(1)],
            x_robots_tag: None,
            pagination_next: None,
            pagination_prev: None,
            canonicalised: false,
            structured_data_valid: true,
            structured_data_errors: Vec::new(),
            simhash: format!("{:016x}", (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
        }),
        content_type: Some("text/html; charset=UTF-8".to_string()),
        response_time_ms: 80 + (i % 300) as u64,
        outlinks,
        security: Some(SecurityData {
            https: true,
            mixed_content: Vec::new(),
            missing_headers: vec![
                "Content-Security-Policy".to_string(),
                "Referrer-Policy".to_string(),
                "X-Frame-Options".to_string(),
            ],
        }),
        url_flags: Some(UrlFlags { length: url.len(), ..Default::default() }),
        // 14 % des pages réelles ont un statut de cache.
        cache: if i % 7 == 0 {
            Some(CacheData {
                status: "HIT".to_string(),
                source: "cf-cache-status".to_string(),
                engine: "Cloudflare".to_string(),
                served_by: "Cloudflare".to_string(),
                raw: "HIT".to_string(),
                age: Some(3600),
                layers: vec![CacheLayer {
                    header: "cf-cache-status".to_string(),
                    engine: "Cloudflare".to_string(),
                    raw: "HIT".to_string(),
                    status: "HIT".to_string(),
                }],
            })
        } else {
            None
        },
        ..Default::default()
    };
    page.issues = crawl_core::seo::analyze(&page);
    page
}

/// Le JSON de la page synthétique `i`, IDENTIQUE d'un appel à l'autre.
///
/// `serde_json::to_string(&page_synthetique(i))` ne l'est pas : `by_type`, `og`
/// et `twitter` sont des `HashMap`, dont l'ordre de parcours change d'une
/// instance à l'autre. On passe donc par une valeur JSON (clés triées). Dans
/// l'application, la question ne se pose pas : les deux chemins d'enregistrement
/// sérialisent les MÊMES objets de la fenêtre.
pub fn page_json(i: usize) -> String {
    serde_json::to_string(&serde_json::to_value(page_synthetique(i)).unwrap()).unwrap()
}

/// Le fichier d'un crawl enregistré, tel que l'interface l'écrit
/// (`saveCurrentCrawl` : `JSON.stringify({ seed_url, date, pages, sitemap_urls })`).
pub fn fichier_de_crawl(n: usize) -> String {
    let mut s = String::from("{\"seed_url\":\"https://forum-synthetique.example/\",\"date\":\"2026-07-30T10:59:00.000Z\",\"pages\":[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&page_json(i));
    }
    s.push_str("],\"sitemap_urls\":[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&serde_json::to_string(&url_page(i)).unwrap());
    }
    s.push_str("]}");
    s
}

/// Le même fichier, produit à la volée (sans jamais le tenir en mémoire) : pour
/// mesurer un lecteur en flux sur un fichier de plusieurs centaines de Mo.
pub struct FichierEnFlux {
    n: usize,
    /// Prochain morceau à produire : 0 = en-tête, 1..=n = pages, n+1 = transition,
    /// n+2..=2n+1 = adresses du sitemap, 2n+2 = fin, au-delà = épuisé.
    etape: usize,
    tampon: Vec<u8>,
    pos: usize,
}

impl FichierEnFlux {
    pub fn new(n: usize) -> Self {
        FichierEnFlux { n, etape: 0, tampon: Vec::new(), pos: 0 }
    }

    fn morceau(&mut self) -> bool {
        let n = self.n;
        self.tampon.clear();
        self.pos = 0;
        let e = self.etape;
        self.etape += 1;
        if e == 0 {
            self.tampon.extend_from_slice(b"{\"seed_url\":\"https://forum-synthetique.example/\",\"date\":\"2026-07-30T10:59:00.000Z\",\"pages\":[");
        } else if e <= n {
            if e > 1 {
                self.tampon.push(b',');
            }
            self.tampon.extend_from_slice(page_json(e - 1).as_bytes());
        } else if e == n + 1 {
            self.tampon.extend_from_slice(b"],\"sitemap_urls\":[");
        } else if e <= 2 * n + 1 {
            if e > n + 2 {
                self.tampon.push(b',');
            }
            serde_json::to_writer(&mut self.tampon, &url_page(e - n - 2)).unwrap();
        } else if e == 2 * n + 2 {
            self.tampon.extend_from_slice(b"]}");
        } else {
            return false;
        }
        true
    }
}

impl std::io::Read for FichierEnFlux {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.tampon.len() {
            if !self.morceau() {
                return Ok(0);
            }
        }
        let k = (self.tampon.len() - self.pos).min(buf.len());
        buf[..k].copy_from_slice(&self.tampon[self.pos..self.pos + k]);
        self.pos += k;
        Ok(k)
    }
}
