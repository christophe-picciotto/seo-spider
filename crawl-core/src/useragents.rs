//! Pool de profils navigateur (user-agent + viewport) pour la rotation aleatoire.
//!
//! Un "profil" associe un user-agent realiste a des dimensions de fenetre
//! coherentes (desktop large, laptop, tablette, mobile). La sonde HTTP n'utilise
//! que le champ `ua` ; la sonde navigateur applique aussi width/height/mobile
//! (emulation de l'appareil via le CDP).
//!
//! >= 50 profils varies (Chrome / Firefox / Safari / Edge / Samsung Internet,
//! sur Windows / macOS / Linux / Android / iOS).

/// Profil navigateur : user-agent + viewport associe.
#[derive(Debug, Clone, Copy)]
pub struct UaProfile {
    pub ua: &'static str,
    pub width: u32,
    pub height: u32,
    pub mobile: bool,
}

/// Liste des profils disponibles (>= 50). Couvre desktop, tablette et mobile.
pub const PROFILES: &[UaProfile] = &[
    // ---- Desktop : Chrome ----
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36", width: 1920, height: 1080, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36", width: 1536, height: 864, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36", width: 1366, height: 768, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36", width: 1440, height: 900, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36", width: 1680, height: 1050, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36", width: 1920, height: 1080, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36", width: 1600, height: 900, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 11.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36", width: 2560, height: 1440, mobile: false },

    // ---- Desktop : Edge ----
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0", width: 1920, height: 1080, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0", width: 1366, height: 768, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36 Edg/144.0.0.0", width: 1512, height: 982, mobile: false },

    // ---- Desktop : Firefox ----
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0", width: 1920, height: 1080, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0", width: 1366, height: 768, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:133.0) Gecko/20100101 Firefox/133.0", width: 1440, height: 900, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0", width: 1920, height: 1080, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0", width: 1600, height: 900, mobile: false },

    // ---- Desktop : Safari ----
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15", width: 1440, height: 900, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15", width: 1680, height: 1050, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15", width: 2560, height: 1440, mobile: false },

    // ---- Mobile : iPhone (Safari iOS) ----
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Mobile/15E148 Safari/604.1", width: 393, height: 852, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1", width: 390, height: 844, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1", width: 375, height: 812, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1", width: 414, height: 896, mobile: true },

    // ---- Mobile : iPhone (Chrome iOS) ----
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/145.0.0.0 Mobile/15E148 Safari/604.1", width: 393, height: 852, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/144.0.0.0 Mobile/15E148 Safari/604.1", width: 390, height: 844, mobile: true },

    // ---- Tablette : iPad ----
    UaProfile { ua: "Mozilla/5.0 (iPad; CPU OS 18_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Mobile/15E148 Safari/604.1", width: 820, height: 1180, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1", width: 768, height: 1024, mobile: true },

    // ---- Mobile : Android Chrome (Pixel / Samsung / autres) ----
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36", width: 412, height: 915, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36", width: 412, height: 915, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; SM-S921B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36", width: 360, height: 800, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36", width: 360, height: 780, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; 22120RN86G) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Mobile Safari/537.36", width: 393, height: 873, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 13; moto g(60)) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36", width: 412, height: 892, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36", width: 448, height: 998, mobile: true },

    // ---- Mobile : Samsung Internet ----
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; SM-S918B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/27.0 Chrome/142.0.0.0 Mobile Safari/537.36", width: 384, height: 854, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 13; SM-A536B) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/26.0 Chrome/140.0.0.0 Mobile Safari/537.36", width: 360, height: 800, mobile: true },

    // ---- Mobile : Firefox Android ----
    UaProfile { ua: "Mozilla/5.0 (Android 14; Mobile; rv:133.0) Gecko/133.0 Firefox/133.0", width: 412, height: 915, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Android 13; Mobile; rv:132.0) Gecko/132.0 Firefox/132.0", width: 360, height: 800, mobile: true },

    // ---- Tablette : Android ----
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36", width: 800, height: 1280, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 13; Lenovo TB320FU) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36", width: 1200, height: 2000, mobile: true },

    // ---- Desktop : Chrome (versions/resolutions supplementaires) ----
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36", width: 1280, height: 720, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 12_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36", width: 1280, height: 800, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (X11; Fedora; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36", width: 1920, height: 1200, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", width: 1360, height: 768, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15", width: 1728, height: 1117, mobile: false },

    // ---- Desktop : Firefox (supplementaires) ----
    UaProfile { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0", width: 1536, height: 864, mobile: false },
    UaProfile { ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 13.6; rv:132.0) Gecko/20100101 Firefox/132.0", width: 1512, height: 982, mobile: false },

    // ---- Mobile : iPhone (supplementaires) ----
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 16_7 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1", width: 390, height: 844, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1", width: 430, height: 932, mobile: true },

    // ---- Mobile : Android (supplementaires) ----
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 12; SM-A325F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36", width: 360, height: 800, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 15; Pixel 8a) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Mobile Safari/537.36", width: 412, height: 915, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 14; CPH2451) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Mobile Safari/537.36", width: 360, height: 804, mobile: true },
    UaProfile { ua: "Mozilla/5.0 (Linux; Android 13; V2312A) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36", width: 392, height: 873, mobile: true },
];

/// Generateur pseudo-aleatoire xorshift64 (evite d'ajouter la crate `rand`).
fn xorshift64(mut x: u64) -> u64 {
    if x == 0 {
        x = 0x9E3779B97F4A7C15; // graine de repli non nulle
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// Choisit un profil a partir d'une graine (ex: compteur de requetes melange a
/// une graine de depart). Deterministe pour une meme graine.
pub fn pick(seed: u64) -> &'static UaProfile {
    let idx = (xorshift64(seed) % PROFILES.len() as u64) as usize;
    &PROFILES[idx]
}
