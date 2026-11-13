//! Codes STABLES des textes que le moteur écrit dans les rapports.
//!
//! Le moteur écrit des phrases françaises dans les données qu'il envoie à
//! l'interface : problèmes SEO (`Issue::message`), erreurs de page
//! (`PageReport::error`). Ces phrases sont ENREGISTRÉES dans les crawls
//! (`crawls/<site>.json`). Pour que l'interface les affiche dans la langue de
//! l'utilisateur, chacune porte désormais, À CÔTÉ d'elle, un code stable en ASCII
//! et ses paramètres (nombres, détail technique) : `ui/engine-texts.js` en tire
//! la clé de traduction — `issue.<code>`, `err.<code>`.
//!
//! RÈGLES, à ne jamais enfreindre :
//!   - un code publié ne change plus : des crawls enregistrés le portent ;
//!   - la phrase française ne change pas non plus : une ancienne version de
//!     l'application relit un nouveau crawl, et `category` sert d'identifiant aux
//!     filtres de l'interface. On AJOUTE des champs, on ne modifie aucune valeur ;
//!   - un nouveau code a sa clé de traduction (`issue.<code>` / `err.<code>`) et
//!     son cas dans `ui/engine-texts.js`, sinon l'interface retombe sur la phrase
//!     française brute.
//!
//! Les autres textes du moteur passent par une reconnaissance de leur valeur
//! dans `ui/engine-texts.js`, car leur valeur EST déjà un identifiant figé :
//! les catégories (`Issue::category`), les erreurs de données structurées
//! (`SeoData::structured_data_errors`, cf. `seo::validate_ld_block`) et les textes
//! de cache (`cachestat`). Leurs valeurs sont verrouillées par des tests.

use crate::report::TextParams;

/// Construit les paramètres d'un texte : `params(&[("len", 72.into())])`.
/// Les noms sont des constantes (aucune allocation pour eux) ; capacité exacte.
pub fn params(pairs: &[(&'static str, serde_json::Value)]) -> TextParams {
    let mut p = TextParams::with_capacity(pairs.len());
    for (k, v) in pairs {
        p.insert(*k, v.clone());
    }
    p
}

/// Codes des problèmes SEO (`Issue::code`), produits par `seo::analyze`.
/// Clé de traduction : `issue.<code>`. Paramètres entre accolades.
pub mod issue {
    /// « Code réponse {status} » (statut HTTP >= 400).
    pub const STATUS_ERROR: &str = "status_error";
    /// « Redirection {status} » (statut HTTP 3xx).
    pub const REDIRECT: &str = "redirect";
    /// « Title manquant ».
    pub const TITLE_MISSING: &str = "title_missing";
    /// « Title trop long ({len} car.) ».
    pub const TITLE_TOO_LONG: &str = "title_too_long";
    /// « Title court ({len}) ».
    pub const TITLE_TOO_SHORT: &str = "title_too_short";
    /// « Meta description manquante ».
    pub const META_DESC_MISSING: &str = "meta_desc_missing";
    /// « Meta description trop longue ({len}) ».
    pub const META_DESC_TOO_LONG: &str = "meta_desc_too_long";
    /// « Meta description courte ({len}) ».
    pub const META_DESC_TOO_SHORT: &str = "meta_desc_too_short";
    /// « H1 manquant ».
    pub const H1_MISSING: &str = "h1_missing";
    /// « {n} H1 (devrait être unique) ».
    pub const H1_MULTIPLE: &str = "h1_multiple";
    /// « {n} image(s) sans attribut alt ».
    pub const IMAGES_MISSING_ALT: &str = "images_missing_alt";
    /// « Page en noindex ».
    pub const NOINDEX: &str = "noindex";
    /// « Canonical manquant ».
    pub const CANONICAL_MISSING: &str = "canonical_missing";
    /// « Page canonicalisée (non-indexable) ».
    pub const CANONICALISED: &str = "canonicalised";
    /// « X-Robots-Tag noindex ».
    pub const X_ROBOTS_NOINDEX: &str = "x_robots_noindex";
    /// « Page paginee ».
    pub const PAGINATED: &str = "paginated";
    /// « Données structurees invalides ({detail}) » — `detail` = la première
    /// erreur de `structured_data_errors`, elle-même traduite par l'interface.
    pub const SD_INVALID: &str = "sd_invalid";
    /// « Contenu mixte ({n} ressources http) ».
    pub const MIXED_CONTENT: &str = "mixed_content";
    /// « En-tête sécurité manquant: {header} » — `header` = nom d'en-tête HTTP.
    pub const SECURITY_HEADER_MISSING: &str = "security_header_missing";
    /// « URL avec caractères non-ASCII ».
    pub const URL_NON_ASCII: &str = "url_non_ascii";
    /// « URL longue ({len}) ».
    pub const URL_TOO_LONG: &str = "url_too_long";
    /// « URL avec majuscules ».
    pub const URL_UPPERCASE: &str = "url_uppercase";
    /// « URL avec underscores ».
    pub const URL_UNDERSCORES: &str = "url_underscores";

    /// Tous les codes de problème que le moteur peut émettre.
    pub const ALL: &[&str] = &[
        STATUS_ERROR,
        REDIRECT,
        TITLE_MISSING,
        TITLE_TOO_LONG,
        TITLE_TOO_SHORT,
        META_DESC_MISSING,
        META_DESC_TOO_LONG,
        META_DESC_TOO_SHORT,
        H1_MISSING,
        H1_MULTIPLE,
        IMAGES_MISSING_ALT,
        NOINDEX,
        CANONICAL_MISSING,
        CANONICALISED,
        X_ROBOTS_NOINDEX,
        PAGINATED,
        SD_INVALID,
        MIXED_CONTENT,
        SECURITY_HEADER_MISSING,
        URL_NON_ASCII,
        URL_TOO_LONG,
        URL_UPPERCASE,
        URL_UNDERSCORES,
    ];
}

/// Erreur d'une page (`PageReport::error`), avec son code stable
/// (`PageReport::error_code`, clé de traduction `err.<code>`) et son détail
/// technique (`PageReport::error_params.detail`).
///
/// Le détail vient du système ou d'une bibliothèque (reqwest, chromiumoxide,
/// contextes `anyhow` de `chrome_manager` / `chrome_probe`) : il est conservé tel
/// quel, seule la partie fixe de la phrase se traduit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageError {
    /// URL interdite par robots.txt : la page n'est pas sondée.
    RobotsBlocked,
    /// La sonde HTTP n'a obtenu aucune réponse exploitable.
    HttpProbeFailed(String),
    /// Chrome n'a pas pu être lancé : la page est mesurée en HTTP statique.
    ChromeLaunchFailed(String),
    /// La mesure Chrome a échoué : la page est mesurée en HTTP statique.
    ChromeMeasureFailed(String),
    /// Chrome a mesuré la page, mais sa navigation a échoué.
    ChromeNavigationFailed(String),
}

/// Codes des erreurs de page (`PageReport::error_code`).
pub mod error {
    /// « Bloque par robots.txt ».
    pub const ROBOTS_BLOCKED: &str = "robots_blocked";
    /// « sonde HTTP echouee: {detail} ».
    pub const HTTP_PROBE_FAILED: &str = "http_probe_failed";
    /// « lancement Chrome impossible, repli HTTP statique: {detail} ».
    pub const CHROME_LAUNCH_FAILED: &str = "chrome_launch_failed";
    /// « mesure Chrome echouee, repli HTTP statique: {detail} ».
    pub const CHROME_MEASURE_FAILED: &str = "chrome_measure_failed";
    /// « navigation echouee: {detail} ».
    pub const CHROME_NAVIGATION_FAILED: &str = "chrome_navigation_failed";

    /// Tous les codes d'erreur de page que le moteur peut émettre.
    pub const ALL: &[&str] = &[
        ROBOTS_BLOCKED,
        HTTP_PROBE_FAILED,
        CHROME_LAUNCH_FAILED,
        CHROME_MEASURE_FAILED,
        CHROME_NAVIGATION_FAILED,
    ];
}

impl PageError {
    /// Code stable de l'erreur (`PageReport::error_code`).
    pub fn code(&self) -> &'static str {
        match self {
            PageError::RobotsBlocked => error::ROBOTS_BLOCKED,
            PageError::HttpProbeFailed(_) => error::HTTP_PROBE_FAILED,
            PageError::ChromeLaunchFailed(_) => error::CHROME_LAUNCH_FAILED,
            PageError::ChromeMeasureFailed(_) => error::CHROME_MEASURE_FAILED,
            PageError::ChromeNavigationFailed(_) => error::CHROME_NAVIGATION_FAILED,
        }
    }

    /// Détail technique de l'erreur, tel que fourni par le système.
    pub fn detail(&self) -> Option<&str> {
        match self {
            PageError::RobotsBlocked => None,
            PageError::HttpProbeFailed(d)
            | PageError::ChromeLaunchFailed(d)
            | PageError::ChromeMeasureFailed(d)
            | PageError::ChromeNavigationFailed(d) => Some(d.as_str()),
        }
    }

    /// Phrase française HISTORIQUE (`PageReport::error`) : NE PAS MODIFIER,
    /// accents manquants compris — elle est enregistrée dans les crawls et
    /// relue par les anciennes versions. L'interface affiche la traduction de
    /// `err.<code>`, où l'orthographe est corrigée.
    pub fn text(&self) -> String {
        match self {
            PageError::RobotsBlocked => "Bloque par robots.txt".to_string(),
            PageError::HttpProbeFailed(e) => format!("sonde HTTP echouee: {e}"),
            PageError::ChromeLaunchFailed(e) => {
                format!("lancement Chrome impossible, repli HTTP statique: {e}")
            }
            PageError::ChromeMeasureFailed(e) => {
                format!("mesure Chrome echouee, repli HTTP statique: {e}")
            }
            PageError::ChromeNavigationFailed(e) => format!("navigation echouee: {e}"),
        }
    }

    /// Paramètres de la phrase (`PageReport::error_params`) : `detail`, s'il y en a un.
    pub fn params(&self) -> TextParams {
        match self.detail() {
            Some(d) => params(&[("detail", d.into())]),
            None => TextParams::new(),
        }
    }
}

/// Modèles FRANÇAIS des phrases, tels que le moteur les écrit aujourd'hui, avec
/// leurs paramètres entre accolades. Servent aux tests : chaque phrase produite
/// doit être exactement son modèle rempli avec ses paramètres — preuve que
/// l'interface, qui ne voit que le code et les paramètres, peut la reconstruire.
#[cfg(test)]
pub(crate) const ISSUE_TEXTS_FR: &[(&str, &str)] = &[
    (issue::STATUS_ERROR, "Code réponse {status}"),
    (issue::REDIRECT, "Redirection {status}"),
    (issue::TITLE_MISSING, "Title manquant"),
    (issue::TITLE_TOO_LONG, "Title trop long ({len} car.)"),
    (issue::TITLE_TOO_SHORT, "Title court ({len})"),
    (issue::META_DESC_MISSING, "Meta description manquante"),
    (issue::META_DESC_TOO_LONG, "Meta description trop longue ({len})"),
    (issue::META_DESC_TOO_SHORT, "Meta description courte ({len})"),
    (issue::H1_MISSING, "H1 manquant"),
    (issue::H1_MULTIPLE, "{n} H1 (devrait être unique)"),
    (issue::IMAGES_MISSING_ALT, "{n} image(s) sans attribut alt"),
    (issue::NOINDEX, "Page en noindex"),
    (issue::CANONICAL_MISSING, "Canonical manquant"),
    (issue::CANONICALISED, "Page canonicalisée (non-indexable)"),
    (issue::X_ROBOTS_NOINDEX, "X-Robots-Tag noindex"),
    (issue::PAGINATED, "Page paginee"),
    (issue::SD_INVALID, "Données structurees invalides ({detail})"),
    (issue::MIXED_CONTENT, "Contenu mixte ({n} ressources http)"),
    (issue::SECURITY_HEADER_MISSING, "En-tête sécurité manquant: {header}"),
    (issue::URL_NON_ASCII, "URL avec caractères non-ASCII"),
    (issue::URL_TOO_LONG, "URL longue ({len})"),
    (issue::URL_UPPERCASE, "URL avec majuscules"),
    (issue::URL_UNDERSCORES, "URL avec underscores"),
];

/// Idem pour les erreurs de page.
#[cfg(test)]
pub(crate) const ERROR_TEXTS_FR: &[(&str, &str)] = &[
    (error::ROBOTS_BLOCKED, "Bloque par robots.txt"),
    (error::HTTP_PROBE_FAILED, "sonde HTTP echouee: {detail}"),
    (error::CHROME_LAUNCH_FAILED, "lancement Chrome impossible, repli HTTP statique: {detail}"),
    (error::CHROME_MEASURE_FAILED, "mesure Chrome echouee, repli HTTP statique: {detail}"),
    (error::CHROME_NAVIGATION_FAILED, "navigation echouee: {detail}"),
];

/// Remplit un modèle : `{nom}` est remplacé par le paramètre `nom` (nombre ou
/// texte, sans guillemets). Même règle que `I18N.t` côté interface.
#[cfg(test)]
pub(crate) fn render(template: &str, params: &TextParams) -> String {
    let mut out = template.to_string();
    for (k, v) in params.iter() {
        let value = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out = out.replace(&format!("{{{k}}}"), &value);
    }
    out
}

/// Modèle français d'un code, ou panique (test).
#[cfg(test)]
pub(crate) fn template_of<'a>(table: &'a [(&'a str, &'a str)], code: &str) -> &'a str {
    table
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, t)| *t)
        .unwrap_or_else(|| panic!("aucun modèle pour le code « {code} »"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Codes uniques, en ASCII minuscule (ils deviennent des clés de traduction).
    #[test]
    fn codes_uniques_et_en_ascii() {
        for all in [issue::ALL, error::ALL] {
            let vus: HashSet<&str> = all.iter().copied().collect();
            assert_eq!(vus.len(), all.len(), "code en double");
            for c in all {
                assert!(
                    !c.is_empty() && c.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                    "code non conforme : {c}"
                );
            }
        }
    }

    /// Chaque code a son modèle, et chaque modèle son code.
    #[test]
    fn chaque_code_a_son_modele() {
        assert_eq!(ISSUE_TEXTS_FR.len(), issue::ALL.len());
        for c in issue::ALL {
            template_of(ISSUE_TEXTS_FR, c);
        }
        assert_eq!(ERROR_TEXTS_FR.len(), error::ALL.len());
        for c in error::ALL {
            template_of(ERROR_TEXTS_FR, c);
        }
    }

    /// Les phrases d'erreur n'ont pas bougé d'un caractère, et le code + le
    /// détail suffisent à les reconstruire.
    #[test]
    fn erreurs_de_page_phrases_historiques_et_codes() {
        let cas = [
            (PageError::RobotsBlocked, "Bloque par robots.txt"),
            (
                PageError::HttpProbeFailed("error sending request for url (https://a.fr/)".into()),
                "sonde HTTP echouee: error sending request for url (https://a.fr/)",
            ),
            (
                PageError::ChromeLaunchFailed(
                    "échec du lancement du navigateur (Chrome/Chromium introuvable ?)".into(),
                ),
                "lancement Chrome impossible, repli HTTP statique: échec du lancement du navigateur (Chrome/Chromium introuvable ?)",
            ),
            (
                PageError::ChromeMeasureFailed("ouverture d'une nouvelle page Chrome".into()),
                "mesure Chrome echouee, repli HTTP statique: ouverture d'une nouvelle page Chrome",
            ),
            (
                PageError::ChromeNavigationFailed("Request timed out.".into()),
                "navigation echouee: Request timed out.",
            ),
        ];
        let mut vus = HashSet::new();
        for (e, attendu) in &cas {
            assert_eq!(e.text(), *attendu);
            assert!(error::ALL.contains(&e.code()));
            assert_eq!(render(template_of(ERROR_TEXTS_FR, e.code()), &e.params()), *attendu);
            vus.insert(e.code());
        }
        assert_eq!(vus.len(), error::ALL.len(), "un code d'erreur n'est pas couvert");
        assert!(PageError::RobotsBlocked.params().is_empty());
    }
}
