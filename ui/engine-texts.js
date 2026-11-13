/**
 * SEO Spider — textes écrits par le MOTEUR Rust → clés de traduction.
 *
 * Le moteur écrit des phrases françaises dans les données qu'il envoie
 * (problèmes SEO, erreurs de page, textes de cache, erreurs de données
 * structurées), et ces phrases sont ENREGISTRÉES dans les crawls. Ce script les
 * ramène à une clé de `ui/locales/*.js` et à ses paramètres :
 *
 *   EngineTexts.issue(it)              problème       → { key, params } | null
 *   EngineTexts.category(nom)          catégorie      → "cat.…" | null
 *   EngineTexts.pageError(page)        erreur de page → { key, params } | null
 *   EngineTexts.cacheText(texte)       texte de cache → "cache.…" | null
 *   EngineTexts.sdError(texte)         erreur JSON-LD → { key, params } | null
 *   EngineTexts.cacheTextParams(texte) texte de cache → { key, params } | null
 *       (extension : couvre AUSSI « cached@ il y a 12855 s », qui a un nombre ;
 *        cacheText renvoie null pour lui, une clé seule afficherait « {age} »)
 *
 * Usage : `var r = EngineTexts.issue(it); r ? I18N.t(r.key, r.params) : it.message`
 *
 * - Le CODE ajouté par le moteur (`it.code` + `it.params`, `page.error_code` +
 *   `page.error_params`, cf. crawl-core/src/codes.rs) est utilisé s'il est
 *   présent et complet ; sinon l'ANCIENNE phrase est reconnue, dans toutes les
 *   orthographes que le moteur a écrites (sans accents avant novembre 2026),
 *   pour que les crawls déjà enregistrés s'affichent eux aussi traduits.
 * - `params` est prêt pour I18N.t : nombres BRUTS, tels que le moteur les a
 *   écrits (le français reste identique), `count` ajouté pour les pluriels.
 *   Un détail qui est lui-même un texte du moteur (« Bloc 1: @type manquant »,
 *   « ouverture d'une nouvelle page Chrome »…) est rendu dans la langue
 *   COURANTE : appeler ces fonctions au moment du rendu.
 * - Aucune fonction ne lève d'exception : entrée inattendue → null, et
 *   l'appelant affiche la valeur brute.
 * - Une clé absente des traductions chargées n'est jamais renvoyée (null).
 *
 * Script classique, sans import ; chargé après `locales/*.js` et `i18n.js`,
 * avant `main.js`.
 */
(function () {
  "use strict";

  var hasOwn = function (o, k) {
    return Object.prototype.hasOwnProperty.call(o, k);
  };

  // ---------------------------------------------------------------------------
  // Tables. Pour chaque entrée :
  //   code : code du moteur = suffixe de la clé (« issue. » + code…) ;
  //   re   : la phrase telle que le moteur l'a écrite, toutes versions
  //          confondues — groupes capturés = paramètres, dans l'ordre de `p` ;
  //   n    : paramètre qui donne `count` (choix du pluriel) ;
  //   tr   : paramètre dont la valeur est elle-même un texte du moteur à
  //          traduire ("sd" : erreur JSON-LD ; "detail" : contexte Chrome).
  // ---------------------------------------------------------------------------

  /** Problèmes SEO (crawl-core/src/seo.rs `analyze`, codes::issue). */
  var ISSUES = [
    { code: "status_error", re: /^Code r[eé]ponse (\d+)$/, p: ["status"], n: "status" },
    { code: "redirect", re: /^Redirection (\d+)$/, p: ["status"], n: "status" },
    { code: "title_missing", re: /^Title manquant$/, p: [] },
    { code: "title_too_long", re: /^Title trop long \((\d+) car\.\)$/, p: ["len"], n: "len" },
    { code: "title_too_short", re: /^Title court \((\d+)\)$/, p: ["len"], n: "len" },
    { code: "meta_desc_missing", re: /^Meta description manquante$/, p: [] },
    { code: "meta_desc_too_long", re: /^Meta description trop longue \((\d+)\)$/, p: ["len"], n: "len" },
    { code: "meta_desc_too_short", re: /^Meta description courte \((\d+)\)$/, p: ["len"], n: "len" },
    { code: "h1_missing", re: /^H1 manquant$/, p: [] },
    { code: "h1_multiple", re: /^(\d+) H1 \(devrait [eê]tre unique\)$/, p: ["n"], n: "n" },
    { code: "images_missing_alt", re: /^(\d+) image\(s\) sans attribut alt$/, p: ["n"], n: "n" },
    { code: "noindex", re: /^Page en noindex$/, p: [] },
    { code: "canonical_missing", re: /^Canonical manquant$/, p: [] },
    { code: "canonicalised", re: /^Page canonicalis[eé]e \(non-indexable\)$/, p: [] },
    { code: "x_robots_noindex", re: /^X-Robots-Tag noindex$/, p: [] },
    { code: "paginated", re: /^Page pagin[eé]e$/, p: [] },
    { code: "sd_invalid", re: /^Donn[eé]es structur[eé]es invalides \(([\s\S]*)\)$/, p: ["detail"], tr: { detail: "sd" } },
    { code: "mixed_content", re: /^Contenu mixte \((\d+) ressources http\)$/, p: ["n"], n: "n" },
    { code: "security_header_missing", re: /^En-t[eê]te s[eé]curit[eé] manquant: ([\s\S]+)$/, p: ["header"] },
    { code: "url_non_ascii", re: /^URL avec caract[eè]res non-ASCII$/, p: [] },
    { code: "url_too_long", re: /^URL longue \((\d+)\)$/, p: ["len"], n: "len" },
    { code: "url_uppercase", re: /^URL avec majuscules$/, p: [] },
    { code: "url_underscores", re: /^URL avec underscores$/, p: [] }
  ];

  /** Erreurs de page (codes::PageError). `detail` = message du système, sauf
   *  quand c'est un contexte français de chrome_manager / chrome_probe. */
  var ERRORS = [
    { code: "robots_blocked", re: /^Bloqu[eé] par robots\.txt$/, p: [] },
    { code: "http_probe_failed", re: /^sonde HTTP [eé]chou[eé]e: ([\s\S]*)$/, p: ["detail"] },
    { code: "chrome_launch_failed", re: /^lancement Chrome impossible, repli HTTP statique: ([\s\S]*)$/, p: ["detail"], tr: { detail: "detail" } },
    { code: "chrome_measure_failed", re: /^mesure Chrome [eé]chou[eé]e, repli HTTP statique: ([\s\S]*)$/, p: ["detail"], tr: { detail: "detail" } },
    { code: "chrome_navigation_failed", re: /^navigation [eé]chou[eé]e: ([\s\S]*)$/, p: ["detail"] }
  ];

  /** Détails français possibles des erreurs Chrome (contextes `anyhow`). */
  var DETAILS = [
    { code: "chrome_not_found", re: /^[eé]chec du lancement du navigateur \(Chrome\/Chromium introuvable \?\)$/, p: [] },
    { code: "chrome_config_failed", re: /^impossible de construire BrowserConfig: ([\s\S]*)$/, p: ["detail"] },
    { code: "chrome_new_page", re: /^ouverture d'une nouvelle page Chrome$/, p: [] },
    { code: "chrome_subscribe", re: /^abonnement (Event[A-Za-z]+)$/, p: ["event"] }
  ];

  /** Textes de cache (crawl-core/src/cachestat.rs). */
  var CACHE = [
    { code: "origin", re: /^origine$/, p: [] },
    { code: "http_cache", re: /^cache HTTP$/, p: [] },
    { code: "edge_no_origin", re: /^servi par le r[eé]seau, sans origine \(aucun cf-cache-status\)$/, p: [] },
    { code: "html_signature", re: /^signature HTML$/, p: [] },
    { code: "wp_rocket_age", re: /^cached@ il y a (\d+) s$/, p: ["age"], n: "age" }
  ];

  /** Erreurs de données structurées (seo.rs `validate_ld_block`). */
  var SD = [
    { code: "context_missing", re: /^Bloc (\d+): @context manquant$/, p: ["block"], n: "block" },
    { code: "type_missing", re: /^Bloc (\d+): @type manquant$/, p: ["block"], n: "block" },
    { code: "invalid_json", re: /^Bloc (\d+): JSON invalide$/, p: ["block"], n: "block" },
    // Repli de seo.rs quand aucune erreur n'est listée : « … invalides (erreur) ».
    { code: "unknown", re: /^erreur$/, p: [] }
  ];

  /** Catégories, comparées sans accents ni casse (« Securite » = « Sécurité »). */
  var CATS = {
    "reponse": "cat.response",
    "titres": "cat.titles",
    "meta description": "cat.meta_description",
    "h1": "cat.h1",
    "images": "cat.images",
    "directives": "cat.directives",
    "canonical": "cat.canonical",
    "pagination": "cat.pagination",
    "donnees structurees": "cat.structured_data",
    "securite": "cat.security",
    "url": "cat.url",
    // Produite par main.js (problèmes hreflang dérivés côté interface).
    "hreflang": "cat.hreflang"
  };

  var FAMILLES = { issue: ISSUES, err: ERRORS, detail: DETAILS, cache: CACHE, sd: SD };
  // Préfixe de clé de chaque famille : les détails Chrome sont des clés `err.`.
  var PREFIXE = { issue: "issue.", err: "err.", detail: "err.", cache: "cache.", sd: "sd." };
  var PAR_CODE = {};
  (function () {
    for (var f in FAMILLES) {
      PAR_CODE[f] = {};
      for (var i = 0; i < FAMILLES[f].length; i++) PAR_CODE[f][FAMILLES[f][i].code] = FAMILLES[f][i];
    }
  })();

  // ---------------------------------------------------------------------------
  // Outils
  // ---------------------------------------------------------------------------

  /** Minuscules, sans accents, espaces resserrés. */
  function fold(s) {
    var t = String(s);
    try {
      t = t.normalize("NFD").replace(/[\u0300-\u036f]/g, "");
    } catch (e) {}
    return t.toLowerCase().replace(/\s+/g, " ").trim();
  }

  /** Une clé n'est renvoyée que si les traductions chargées la connaissent. */
  var connues = {};
  function keyKnown(key) {
    var L = window.SEO_SPIDER_LOCALES;
    if (!L) return true; // aucune table chargée (banc d'essai) : rien à vérifier
    if (!hasOwn(connues, key)) {
      connues[key] = !!((L.en && hasOwn(L.en, key)) || (L.fr && hasOwn(L.fr, key)));
    }
    return connues[key];
  }

  /** Reconnaissance d'une phrase, mémorisée : la grille rend chaque problème à
   *  chaque cycle (120 000 lignes sur un gros crawl). */
  var memo = { issue: new Map(), err: new Map(), detail: new Map(), cache: new Map(), sd: new Map() };
  var MEMO_MAX = 20000;
  var MEMO_LONGUEUR_MAX = 2000; // un texte anormalement long n'est pas gardé en mémoire
  function reconnait(famille, texte) {
    if (typeof texte !== "string" || !texte) return null;
    var m = memo[famille];
    if (m.has(texte)) return m.get(texte);
    var t = texte;
    try {
      t = t.normalize("NFC");
    } catch (e) {}
    var trouve = null;
    var liste = FAMILLES[famille];
    for (var i = 0; i < liste.length && !trouve; i++) {
      var r = liste[i].re.exec(t);
      if (!r) continue;
      var params = {};
      for (var j = 0; j < liste[i].p.length; j++) params[liste[i].p[j]] = r[j + 1];
      trouve = { def: liste[i], famille: famille, params: params };
    }
    if (texte.length <= MEMO_LONGUEUR_MAX) {
      if (m.size >= MEMO_MAX) m.clear();
      m.set(texte, trouve);
    }
    return trouve;
  }

  /** Depuis le code du moteur ; null si un paramètre manque (repli sur le texte). */
  function depuisCode(famille, code, brut) {
    if (typeof code !== "string" || !code || !hasOwn(PAR_CODE[famille], code)) return null;
    var def = PAR_CODE[famille][code];
    var params = {};
    for (var i = 0; i < def.p.length; i++) {
      var nom = def.p[i];
      var v = brut && typeof brut === "object" ? brut[nom] : undefined;
      if (typeof v === "number" && isFinite(v)) params[nom] = String(v);
      else if (typeof v === "string") params[nom] = v;
      else return null;
    }
    return { def: def, famille: famille, params: params };
  }

  /** Texte d'un détail qui est lui-même un texte du moteur, dans la langue
   *  courante ; sinon le détail tel quel (message du système). */
  function traduitDetail(famille, texte) {
    var I = window.I18N;
    if (!I || typeof I.t !== "function") return texte;
    var r = finit(reconnait(famille, texte));
    if (!r) return texte;
    var s = I.t(r.key, r.params);
    return typeof s === "string" && s && s !== r.key ? s : texte;
  }

  /** Résultat final : clé vérifiée, copie des paramètres, `count`, détails traduits. */
  function finit(r) {
    if (!r) return null;
    var key = PREFIXE[r.famille] + r.def.code;
    if (!keyKnown(key)) return null;
    var params = {};
    for (var k in r.params) if (hasOwn(r.params, k)) params[k] = r.params[k];
    if (r.def.n && params[r.def.n] != null) params.count = Number(params[r.def.n]);
    if (r.def.tr) {
      for (var nom in r.def.tr) {
        if (hasOwn(r.def.tr, nom) && typeof params[nom] === "string") {
          params[nom] = traduitDetail(r.def.tr[nom], params[nom]);
        }
      }
    }
    return { key: key, params: params };
  }

  function vide(o) {
    for (var k in o) if (hasOwn(o, k)) return false;
    return true;
  }

  // ---------------------------------------------------------------------------
  // API
  // ---------------------------------------------------------------------------

  function issue(it) {
    try {
      if (!it || typeof it !== "object") return null;
      return finit(depuisCode("issue", it.code, it.params) || reconnait("issue", it.message));
    } catch (e) {
      return null;
    }
  }

  function category(nom) {
    try {
      if (typeof nom !== "string" || !nom) return null;
      var f = fold(nom);
      if (!hasOwn(CATS, f)) return null;
      return keyKnown(CATS[f]) ? CATS[f] : null;
    } catch (e) {
      return null;
    }
  }

  function pageError(page) {
    try {
      var p = typeof page === "string" ? { error: page } : page;
      if (!p || typeof p !== "object") return null;
      return finit(depuisCode("err", p.error_code, p.error_params) || reconnait("err", p.error));
    } catch (e) {
      return null;
    }
  }

  function cacheTextParams(texte) {
    try {
      return finit(reconnait("cache", typeof texte === "string" ? texte.trim() : texte));
    } catch (e) {
      return null;
    }
  }

  function cacheText(texte) {
    var r = cacheTextParams(texte);
    return r && vide(r.params) ? r.key : null;
  }

  function sdError(texte) {
    try {
      return finit(reconnait("sd", typeof texte === "string" ? texte.trim() : texte));
    } catch (e) {
      return null;
    }
  }

  /** Toutes les clés que ce script peut renvoyer (contrôles de cohérence). */
  function keys() {
    var out = [];
    for (var f in FAMILLES) {
      for (var i = 0; i < FAMILLES[f].length; i++) {
        var k = PREFIXE[f] + FAMILLES[f][i].code;
        if (out.indexOf(k) < 0) out.push(k);
      }
    }
    for (var c in CATS) if (out.indexOf(CATS[c]) < 0) out.push(CATS[c]);
    return out;
  }

  /** Codes du moteur pris en charge : { issue: [...], err: [...] }. */
  function codes() {
    var lister = function (l) {
      return l.map(function (d) {
        return d.code;
      });
    };
    return { issue: lister(ISSUES), err: lister(ERRORS) };
  }

  window.EngineTexts = {
    issue: issue,
    category: category,
    pageError: pageError,
    cacheText: cacheText,
    sdError: sdError,
    cacheTextParams: cacheTextParams,
    keys: keys,
    codes: codes
  };
})();
