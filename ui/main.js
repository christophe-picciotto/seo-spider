/**
 * SEO Spider — Frontend vanilla (Tauri v2, withGlobalTauri = true)
 * Acces Tauri : window.__TAURI__.core.invoke / window.__TAURI__.event.listen
 *
 * Interface dense facon Screaming Frog :
 *   - barre du haut : URL + cases "Tout le site" (unlimited) / "robots.txt" ;
 *   - barre d'onglets avec compteurs en direct ;
 *   - grille dense triable (en-tete fige, colonnes selon l'onglet) ;
 *   - sidebar "Overview" : categories cliquables avec compteurs ;
 *   - panneau de detail (Inlinks/Outlinks/SERP/Infos/Source) au clic ligne ;
 *   - barre de statut (progression / file / debit / poids total).
 *
 * Flux de crawl PRESERVE : invoke start_crawl/stop_crawl + listeners
 * crawl:progress / crawl:page / crawl:done / crawl:error. La logique de
 * mesure de bande passante (multi-run, agregats) est conservee a l'identique.
 */

(function () {
  "use strict";

  var TAURI = window.__TAURI__;

  // =========================================================================
  // Utilitaires de formatage
  // =========================================================================
  function fmtBytes(bytes) {
    if (!bytes || bytes < 0) return "0 o";
    if (bytes < 1024) return bytes + " o";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " Ko";
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + " Mo";
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " Go";
  }
  // Echappe le HTML (titres/meta peuvent contenir < > & " issus du site).
  function esc(v) {
    if (v === null || v === undefined) return "";
    return String(v)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }
  // Tronque une URL pour l'affichage (l'attribut title garde la version complete).
  function truncUrl(url, maxLen) {
    maxLen = maxLen || 90;
    if (!url) return "";
    if (url.length <= maxLen) return url;
    return url.slice(0, maxLen - 3) + "...";
  }
  function probeLabel(probe) {
    if (!probe) return "—";
    if (probe === "http") return "HTTP";
    if (probe === "chrome") return "Chrome";
    return probe;
  }
  function $(id) { return document.getElementById(id); }
  function setText(id, v) { var el = $(id); if (el) el.textContent = v; }

  // Classe de couleur d'un code HTTP (2xx vert, 3xx orange, 4xx/5xx rouge).
  function httpClass(status) {
    var s = parseInt(status, 10) || 0;
    if (s >= 200 && s < 300) return "http--ok";
    if (s >= 300 && s < 400) return "http--redir";
    if (s >= 400)            return "http--err";
    return "http--muted";
  }
  function seoOf(p) { return p && p.seo ? p.seo : null; }

  // ----- VAGUE 4 (A) — Temps de reponse (response_time_ms) -----------------
  // Lit le temps de reponse d'une page en millisecondes (0 si absent).
  function respMs(p) {
    return p && p.response_time_ms != null ? (p.response_time_ms || 0) : 0;
  }
  // Formate un temps : "<n> ms" si < 1000 ms, sinon "<x,x> s" (virgule FR).
  function fmtSpeed(ms) {
    ms = ms || 0;
    if (ms < 1000) return ms + " ms";
    return (ms / 1000).toFixed(1).replace(".", ",") + " s";
  }
  // Classe de couleur facon "rapidite" : vert <=500 ms, orange <=1500 ms,
  // rouge au-dela. Renvoie "" si pas de mesure (0) pour rester neutre.
  function speedClass(ms) {
    ms = ms || 0;
    if (ms <= 0)    return "speed--na";
    if (ms <= 500)  return "speed--fast";
    if (ms <= 1500) return "speed--mid";
    return "speed--slow";
  }
  // Cellule HTML "vitesse" coloree (pastille). text/sort fournis par l'appelant.
  function speedCellHtml(ms) {
    if (!ms) return '<span class="speed-pill speed--na">—</span>';
    return '<span class="speed-pill ' + speedClass(ms) + '">' + esc(fmtSpeed(ms)) + '</span>';
  }

  // Cellule URL : lien tronque, titre complet au survol. stopPropagation pour
  // ne pas selectionner la ligne quand on ouvre le lien dans le navigateur.
  function urlCellHtml(url) {
    var u = esc(url || "");
    return '<a href="' + u + '" target="_blank" rel="noopener noreferrer" title="' +
           u + '" class="cell-url" onclick="event.stopPropagation()">' +
           esc(truncUrl(url, 90)) + '</a>';
  }
  function pill(kind, label) {
    return '<span class="state-pill state-pill--' + kind + '">' + esc(label) + '</span>';
  }

  // Normalisation legere d'URL (comparer canonical vs URL de page, dedup inlinks).
  function normUrl(u) {
    if (!u) return "";
    return String(u).trim().replace(/#.*$/, "").replace(/\/+$/, "").toLowerCase();
  }

  // ----- Extraction des valeurs servant au calcul des DOUBLONS (Vague 2) ----
  // Comparaison EXACTE (apres trim). Une cle vide ne compte jamais comme doublon.
  function dupKeyTitle(p) { var s = seoOf(p); return s && s.title ? s.title.trim() : ""; }
  function dupKeyMeta(p)  { var s = seoOf(p); return s && s.meta_description ? s.meta_description.trim() : ""; }
  function dupKeyH1(p)    {
    var s = seoOf(p);
    var arr = s && Array.isArray(s.h1) ? s.h1 : [];
    return arr.length && arr[0] ? String(arr[0]).trim() : "";
  }

  // Acces aux nouveaux objets Vague 2 (peuvent etre null cote backend).
  function secOf(p)   { return p && p.security ? p.security : null; }
  function flagsOf(p) { return p && p.url_flags ? p.url_flags : null; }

  // Nombre d'en-tetes manquants (tableau security.missing_headers).
  function missingHeadersOf(p) {
    var sec = secOf(p);
    return sec && Array.isArray(sec.missing_headers) ? sec.missing_headers : [];
  }
  // Nombre de ressources en contenu mixte (tableau security.mixed_content).
  function mixedContentOf(p) {
    var sec = secOf(p);
    return sec && Array.isArray(sec.mixed_content) ? sec.mixed_content : [];
  }

  // =========================================================================
  // VAGUE 3 — Near-duplicates (empreinte simhash 64 bits)
  // -------------------------------------------------------------------------
  // Chaque page expose seo.simhash : une chaine HEXADECIMALE de 16 caracteres
  // representant une empreinte de 64 bits du contenu visible. Deux pages au
  // contenu tres proche ont des simhash dont peu de bits different.
  //
  // PRINCIPE de la DISTANCE DE HAMMING :
  //   distance = nombre de bits qui different entre les deux empreintes
  //            = popcount( simhashA XOR simhashB )   (sur 64 bits)
  // On la calcule via BigInt : BigInt("0x"+hex) donne l'entier 64 bits exact
  // (les nombres JS classiques perdent en precision au-dela de 53 bits).
  //   - XOR bit a bit : (a ^ b) met a 1 chaque bit qui differe ;
  //   - popcount : on compte ces bits a 1 en consommant l'entier 16 bits par
  //     16 bits (masque 0xFFFF) avec une table de comptage precalculee, ce qui
  //     evite une boucle sur 64 bits a chaque comparaison.
  // Seuil : distance <= NEAR_DUP_BITS bits => pages considerees quasi-dupliquees.
  // =========================================================================
  var NEAR_DUP_BITS = 6;          // <= 6 bits de difference = quasi-duplique
  var NEAR_DUP_MAX_PAGES = 2000;  // au-dela, on saute le calcul O(n^2) (cf. note UI)

  // Table de popcount sur 16 bits (65536 entrees) : POPCNT16[x] = nb de bits a 1.
  // Construite une seule fois ; rend popcountBig() quasi O(1) par comparaison.
  var POPCNT16 = (function () {
    var t = new Uint8Array(65536);
    for (var i = 1; i < 65536; i++) t[i] = t[i >> 1] + (i & 1);
    return t;
  })();

  // popcount d'un BigInt 64 bits : on decoupe en 4 tranches de 16 bits.
  function popcountBig(x) {
    var n = 0;
    // Number(x & 0xFFFFn) est sur dans [0..65535] -> indexation directe.
    n += POPCNT16[Number(x & 0xFFFFn)];
    n += POPCNT16[Number((x >> 16n) & 0xFFFFn)];
    n += POPCNT16[Number((x >> 32n) & 0xFFFFn)];
    n += POPCNT16[Number((x >> 48n) & 0xFFFFn)];
    return n;
  }

  // simhash hex (16 chars) d'une page, ou "" si absent/invalide.
  function simhashOf(p) {
    var s = seoOf(p);
    var h = s && s.simhash ? String(s.simhash).trim().toLowerCase() : "";
    // On exige une chaine hexa non nulle (un simhash "0000...0" signale un
    // contenu vide cote backend : on l'ignore pour ne pas tout regrouper).
    if (!h || !/^[0-9a-f]{1,16}$/.test(h)) return "";
    if (/^0+$/.test(h)) return "";
    return h;
  }

  // Convertit un simhash hex en BigInt 64 bits (prefixe 0x).
  function simhashBig(hex) { return BigInt("0x" + hex); }

  // Renvoie la liste (memorisee) des URLs quasi-dupliquees a la page p.
  function nearDupOf(p) {
    var arr = S.nearDup[normUrl(p.url)];
    return Array.isArray(arr) ? arr : [];
  }

  // =========================================================================
  // Etat global
  // =========================================================================
  var S = {
    pages: [],            // tous les PageReport bruts recus (verbatim serde)
    byUrl: {},            // index normUrl -> PageReport (pour le panneau detail)
    pageIndex: {},        // index normUrl -> position dans S.pages (ingestion idempotente)
    issues: [],           // toutes les issues agregees {url,severity,category,message}
    sevCounts: { critical: 0, warning: 0, opportunity: 0, info: 0 },

    activeTab: "internal",
    search: "",
    sevFilter: "all",
    sitemapView: "all",                 // Vague 3 : "all" | "notin" (onglet Sitemap)
    sort: { key: null, dir: null },     // tri de la grille (par onglet courant)

    // Filtre pousse par la sidebar Overview : { predicate(p)->bool, label } ou null
    catFilter: null,
    catId: null,                        // identifiant de la categorie active

    selectedUrl: null,                  // URL de la ligne selectionnee (detail)
    detailTab: "inlinks",

    // Inlinks : index inverse normUrl(cible) -> [{from, anchor, rel}]
    inlinksIndex: {},

    // Cartes de doublons (Vague 2) : valeur normalisee -> nombre de pages.
    // Reconstruites une fois par cycle de rendu (evite O(n^2) par ligne).
    dupMaps: { title: {}, meta: {}, h1: {} },

    // ----- Vague 3 -----
    // Near-duplicates : normUrl(page) -> [urls quasi-dupliquees] (distance de
    // Hamming <= NEAR_DUP_BITS sur le simhash 64 bits). Reconstruit une fois par
    // cycle de rendu, en O(n^2) BORNE (cf. NEAR_DUP_MAX_PAGES).
    nearDup: {},
    nearDupSkipped: false,         // vrai si trop de pages -> calcul desactive

    // Hreflang reciproque : normUrl(page) -> { declared:[langs], missingBack:[urls] }
    // missingBack = pages cibles crawlees qui ne declarent PAS de retour vers nous.
    hreflangRecip: {},
    // Issues hreflang derivees cote JS (categorie "Hreflang"), reconstruites a
    // chaque cycle et FUSIONNEES a la lecture (jamais poussees dans S.issues
    // pour ne pas doublonner ni polluer l'agregat backend).
    hreflangIssues: [],

    // Sitemap (recu via crawl:done) : liste brute + Set normalise pour les jointures.
    sitemapUrls: [],               // URLs telles que declarees dans le sitemap
    sitemapSet: {},                // normUrl(sitemapUrl) -> true (appartenance O(1))

    // Mesure de bande passante (run courant + agregats multi-run)
    bw: { totalBytes: 0, maxWeight: 0 },
    multi: {
      target: 0, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: false,
      config: {}, lastTechs: [], lastRows: []
    },

    // Cadence des re-rendus (batch via requestAnimationFrame pour gros crawls)
    dirty: false,
    rafPending: false,
    running: false,
    paused: false,                      // VAGUE 4 (C) : crawl mis en pause (Stop)
    rateStartTs: 0,                     // pour estimer le debit pages/seconde

    unlisteners: []
  };

  // Historique persistant (liste de sequences).
  var history = [];

  // =========================================================================
  // Definition des ONGLETS
  // Chaque onglet : columns (liste d'objets {key,label,num?,w?}) + une fonction
  // row(p) qui renvoie, pour chaque colonne, { html, text, sort }. "html" pour
  // l'affichage, "text" pour l'export CSV, "sort" pour le tri (valeur brute).
  // Les onglets sur les pages utilisent kind:"pages". L'onglet "links" liste
  // tous les outlinks (kind:"links"), "issues" liste les problemes (kind:"issues").
  // =========================================================================
  var TABS = {

    // ----- Internal : vue principale facon SF -----
    internal: {
      kind: "pages",
      columns: [
        { key: "url",       label: "Adresse" },
        { key: "status",    label: "Code", num: true },
        { key: "ctype",     label: "Type" },
        { key: "speed",     label: "Vitesse", num: true },
        { key: "words",     label: "Mots", num: true },
        { key: "title",     label: "Title" },
        { key: "indexable", label: "Indexable" },
        { key: "outlinks",  label: "Outlinks", num: true }
      ],
      row: function (p) {
        var s = seoOf(p);
        var title = s && s.title ? s.title : "";
        var words = s ? (s.word_count || 0) : 0;
        var indexable = s ? !!s.indexable : false;
        var nout = Array.isArray(p.outlinks) ? p.outlinks.length : 0;
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: '<span class="http-code ' + httpClass(p.status) + '">' + esc(p.status || 0) + '</span>',
            text: String(p.status || 0), sort: p.status || 0 },
          { html: esc(shortType(p.content_type)), text: p.content_type || "", sort: p.content_type || "" },
          // VAGUE 4 (A) : temps de reponse colore (vert/orange/rouge), triable.
          { html: '<span class="col-num">' + speedCellHtml(respMs(p)) + '</span>',
            text: respMs(p) ? String(respMs(p)) : "", sort: respMs(p) },
          { html: '<span class="col-num">' + words + '</span>', text: String(words), sort: words },
          { html: esc(title || "—"), text: title, sort: title },
          { html: indexable ? pill("ok", "Indexable") : pill("err", "Non indexable"),
            text: indexable ? "Indexable" : "Non indexable", sort: indexable ? 1 : 0 },
          { html: '<span class="col-num">' + nout + '</span>', text: String(nout), sort: nout }
        ];
      }
    },

    // ----- Response Codes -----
    response: {
      kind: "pages",
      columns: [
        { key: "url",      label: "Adresse" },
        { key: "status",   label: "Code", num: true },
        { key: "ctype",    label: "Type" },
        { key: "redirect", label: "Redirige vers" }
      ],
      row: function (p) {
        var redir = p.redirect_to || "";
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: '<span class="http-code ' + httpClass(p.status) + '">' + esc(p.status || 0) + '</span>',
            text: String(p.status || 0), sort: p.status || 0 },
          { html: esc(shortType(p.content_type)), text: p.content_type || "", sort: p.content_type || "" },
          { html: redir ? urlCellHtml(redir) : '<span class="muted">—</span>', text: redir, sort: redir }
        ];
      }
    },

    // ----- URL (Vague 2) : analyse de la forme des adresses -----
    // Colonnes : Adresse, Longueur, non-ASCII, Underscores, Majuscules, Parametres.
    // Une ligne est en alerte si l'URL depasse 115 caracteres OU contient du non-ASCII.
    url: {
      kind: "pages",
      columns: [
        { key: "url",        label: "Adresse" },
        { key: "length",     label: "Longueur", num: true },
        { key: "nonascii",   label: "non-ASCII" },
        { key: "underscores", label: "Underscores" },
        { key: "uppercase",  label: "Majuscules" },
        { key: "slashes",    label: "Slashes //" },
        { key: "params",     label: "Parametres" }
      ],
      row: function (p) {
        var f = flagsOf(p);
        // Repli sur l'URL elle-meme si le backend n'a pas (encore) rempli url_flags.
        var len = f && f.length != null ? f.length : (p.url ? p.url.length : 0);
        var nonAscii = f ? !!f.non_ascii : false;
        var underscores = f ? !!f.underscores : false;
        var uppercase = f ? !!f.uppercase : false;
        var multipleSlashes = f ? !!f.multiple_slashes : false;
        var params = f ? !!f.params : false;
        // Surlignage de la longueur si > 115 (le surlignage de ligne est gere par rowClass).
        var lenHtml = '<span class="col-num' + (len > 115 ? ' num-bad' : '') + '">' + len + '</span>';
        return [
          // La classe row--alert (posee via rowClass plus bas) surligne la ligne.
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: lenHtml, text: String(len), sort: len },
          { html: nonAscii ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: nonAscii ? "oui" : "non", sort: nonAscii ? 1 : 0 },
          { html: underscores ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: underscores ? "oui" : "non", sort: underscores ? 1 : 0 },
          { html: uppercase ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: uppercase ? "oui" : "non", sort: uppercase ? 1 : 0 },
          { html: multipleSlashes ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: multipleSlashes ? "oui" : "non", sort: multipleSlashes ? 1 : 0 },
          { html: params ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: params ? "oui" : "non", sort: params ? 1 : 0 }
        ];
      },
      // Surligne la ligne entiere si alerte (URL trop longue, non-ASCII ou double slash).
      rowClass: function (p) {
        var f = flagsOf(p);
        var len = f && f.length != null ? f.length : (p.url ? p.url.length : 0);
        var nonAscii = f ? !!f.non_ascii : false;
        var multipleSlashes = f ? !!f.multiple_slashes : false;
        return (len > 115 || nonAscii || multipleSlashes) ? "row--alert" : "";
      }
    },

    // ----- Securite (Vague 2) : HTTPS, contenu mixte, en-tetes manquants -----
    // Une ligne est en alerte si la page presente du contenu mixte.
    security: {
      kind: "pages",
      columns: [
        { key: "url",     label: "Adresse" },
        { key: "https",   label: "HTTPS" },
        { key: "mixed",   label: "Contenu mixte", num: true },
        { key: "headers", label: "En-tetes manquants" }
      ],
      row: function (p) {
        var sec = secOf(p);
        var https = sec ? !!sec.https : (p.url ? /^https:/i.test(p.url) : false);
        var mixed = mixedContentOf(p);
        var headers = missingHeadersOf(p);
        // Liste courte des en-tetes manquants (max 3 affiches, reste en "+N").
        var headHtml, headText;
        if (!headers.length) { headHtml = '<span class="bool-no">—</span>'; headText = ""; }
        else {
          var shown = headers.slice(0, 3).map(function (h) { return '<span class="badge">' + esc(h) + '</span>'; }).join("");
          if (headers.length > 3) shown += '<span class="muted">+' + (headers.length - 3) + '</span>';
          headHtml = shown;
          headText = headers.join(", ");
        }
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: https ? pill("ok", "oui") : pill("err", "non"),
            text: https ? "oui" : "non", sort: https ? 1 : 0 },
          { html: mixed.length ? '<span class="col-num num-bad">' + mixed.length + '</span>'
                               : '<span class="col-num">0</span>',
            text: String(mixed.length), sort: mixed.length },
          { html: headHtml, text: headText, sort: headers.length }
        ];
      },
      // Surligne la ligne entiere si contenu mixte present.
      rowClass: function (p) { return mixedContentOf(p).length ? "row--alert" : ""; }
    },

    // ----- Page Titles -----
    titles: {
      kind: "pages",
      columns: [
        { key: "url",   label: "Adresse" },
        { key: "title", label: "Title" },
        { key: "len",   label: "Longueur", num: true },
        { key: "dup",   label: "Duplique" },
        { key: "state", label: "Etat" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var title = s && s.title ? s.title : "";
        var len = s ? (s.title_len || 0) : 0;
        var st = titleState(title, len);
        var dup = isDup("title", dupKeyTitle, p);   // doublon exact calcule cote JS
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: esc(title || "—"), text: title, sort: title },
          { html: '<span class="col-num">' + len + '</span>', text: String(len), sort: len },
          { html: dup ? pill("warn", "Oui") : '<span class="muted">—</span>',
            text: dup ? "Oui" : "Non", sort: dup ? 1 : 0 },
          { html: pill(st.kind, st.label), text: st.label, sort: st.label }
        ];
      }
    },

    // ----- Meta Description -----
    meta: {
      kind: "pages",
      columns: [
        { key: "url",  label: "Adresse" },
        { key: "meta", label: "Meta Description" },
        { key: "len",  label: "Longueur", num: true },
        { key: "dup",  label: "Duplique" },
        { key: "state", label: "Etat" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var meta = s && s.meta_description ? s.meta_description : "";
        var len = s ? (s.meta_description_len || 0) : 0;
        var st = metaState(meta, len);
        var dup = isDup("meta", dupKeyMeta, p);     // doublon exact calcule cote JS
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: esc(meta || "—"), text: meta, sort: meta },
          { html: '<span class="col-num">' + len + '</span>', text: String(len), sort: len },
          { html: dup ? pill("warn", "Oui") : '<span class="muted">—</span>',
            text: dup ? "Oui" : "Non", sort: dup ? 1 : 0 },
          { html: pill(st.kind, st.label), text: st.label, sort: st.label }
        ];
      }
    },

    // ----- H1 -----
    h1: {
      kind: "pages",
      columns: [
        { key: "url",   label: "Adresse" },
        { key: "h1",    label: "Premier H1" },
        { key: "count", label: "Nombre", num: true },
        { key: "dup",   label: "Duplique" },
        { key: "state", label: "Etat" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var arr = s && Array.isArray(s.h1) ? s.h1 : [];
        var count = s ? (s.h1_count || arr.length) : 0;
        var first = arr.length ? arr[0] : "";
        var st = h1State(count);
        var dup = isDup("h1", dupKeyH1, p);         // doublon exact du 1er h1 (JS)
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: esc(first || "—"), text: first, sort: first },
          { html: '<span class="col-num">' + count + '</span>', text: String(count), sort: count },
          { html: dup ? pill("warn", "Oui") : '<span class="muted">—</span>',
            text: dup ? "Oui" : "Non", sort: dup ? 1 : 0 },
          { html: pill(st.kind, st.label), text: st.label, sort: st.label }
        ];
      }
    },

    // ----- Contenu (Vague 3) : near-duplicates -----
    // Colonnes : Adresse, Mots, Near-dup (Oui + nombre de pages similaires).
    // La detection repose sur seo.simhash + distance de Hamming (cf. rebuildNearDup).
    content: {
      kind: "pages",
      columns: [
        { key: "url",     label: "Adresse" },
        { key: "words",   label: "Mots", num: true },
        { key: "neardup", label: "Near-dup", num: true }
      ],
      row: function (p) {
        var s = seoOf(p);
        var words = s ? (s.word_count || 0) : 0;
        var sims = nearDupOf(p);                 // URLs quasi-dupliquees (memorise)
        var n = sims.length;
        var ndHtml = n
          ? '<span class="state-pill state-pill--warn">Oui (' + n + ')</span>'
          : '<span class="muted">—</span>';
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: '<span class="col-num">' + words + '</span>', text: String(words), sort: words },
          // tri par nombre de similaires (desc = les plus dupliquees en tete) ;
          // le texte CSV liste les URLs similaires pour exploitation hors-ligne.
          { html: ndHtml, text: n ? (n + " : " + sims.join(" | ")) : "0", sort: n }
        ];
      },
      // Surligne la ligne si la page a au moins une quasi-dupliquee.
      rowClass: function (p) { return nearDupOf(p).length ? "row--alert" : ""; }
    },

    // ----- Images (par page) -----
    images: {
      kind: "pages",
      columns: [
        { key: "url",     label: "Adresse" },
        { key: "total",   label: "Images", num: true },
        { key: "missing", label: "Sans alt", num: true },
        { key: "state",   label: "Etat" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var total = s ? (s.images_total || 0) : 0;
        var missing = s ? (s.images_missing_alt || 0) : 0;
        var kind = missing > 0 ? "warn" : "ok";
        var label = missing > 0 ? (missing + " sans alt") : "OK";
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: '<span class="col-num">' + total + '</span>', text: String(total), sort: total },
          { html: missing > 0 ? '<span class="col-num num-bad">' + missing + '</span>'
                              : '<span class="col-num">0</span>', text: String(missing), sort: missing },
          { html: pill(kind, label), text: label, sort: missing }
        ];
      }
    },

    // ----- Canonicals -----
    canonical: {
      kind: "pages",
      columns: [
        { key: "url",   label: "Adresse" },
        { key: "canon", label: "Canonical" },
        { key: "state", label: "Type" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var canon = s && s.canonical ? s.canonical : "";
        var st = canonState(p.url, canon);
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: canon ? urlCellHtml(canon) : '<span class="muted">—</span>', text: canon, sort: canon },
          { html: pill(st.kind, st.label), text: st.label, sort: st.label }
        ];
      }
    },

    // ----- Directives -----
    directives: {
      kind: "pages",
      columns: [
        { key: "url",       label: "Adresse" },
        { key: "robots",    label: "Meta Robots" },
        { key: "xrobots",   label: "X-Robots-Tag" },
        { key: "indexable", label: "Indexable" },
        { key: "noindex",   label: "Noindex" },
        { key: "nofollow",  label: "Nofollow" },
        { key: "canonsd",   label: "Canonicalisee" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var robots = s && s.meta_robots ? s.meta_robots : "";
        var xrobots = s && s.x_robots_tag ? s.x_robots_tag : "";    // en-tete HTTP (Vague 2)
        var indexable = s ? !!s.indexable : false;
        var noindex = s ? !!s.noindex : false;
        var nofollow = s ? !!s.nofollow : false;
        var canonicalised = s ? !!s.canonicalised : false;          // canonical != URL (Vague 2)
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: esc(robots || "—"), text: robots, sort: robots },
          { html: esc(xrobots || "—"), text: xrobots, sort: xrobots },
          { html: indexable ? pill("ok", "Oui") : pill("err", "Non"),
            text: indexable ? "Oui" : "Non", sort: indexable ? 1 : 0 },
          { html: noindex ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: noindex ? "oui" : "non", sort: noindex ? 1 : 0 },
          { html: nofollow ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: nofollow ? "oui" : "non", sort: nofollow ? 1 : 0 },
          { html: canonicalised ? '<span class="bool-yes">oui</span>' : '<span class="bool-no">—</span>',
            text: canonicalised ? "oui" : "non", sort: canonicalised ? 1 : 0 }
        ];
      }
    },

    // ----- Hreflang (Vague 3 : + colonne Reciproque) -----
    // Reciproque = OK si toutes les cibles hreflang crawlees pointent en retour
    // vers cette page ; Manquant si au moins une cible crawlee n'a pas de lien
    // retour ; "—" si la page ne declare aucun hreflang. Calcul cote JS dans
    // rebuildHreflangRecip() (S.hreflangRecip).
    hreflang: {
      kind: "pages",
      columns: [
        { key: "url",   label: "Adresse" },
        { key: "hl",    label: "Hreflang (lang -> href)" },
        { key: "recip", label: "Reciproque" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var hl = s && Array.isArray(s.hreflang) ? s.hreflang : [];
        var html, text;
        if (!hl.length) { html = '<span class="muted">—</span>'; text = ""; }
        else {
          html = hl.map(function (h) {
            return '<span class="badge">' + esc(h.lang) + '</span>' + esc(truncUrl(h.href, 50));
          }).join(" ");
          text = hl.map(function (h) { return h.lang + " -> " + h.href; }).join(" | ");
        }
        // Etat de reciprocite.
        var r = S.hreflangRecip[normUrl(p.url)];
        var recipHtml, recipText, recipSort;
        if (!hl.length) { recipHtml = '<span class="muted">—</span>'; recipText = ""; recipSort = -1; }
        else if (r && r.missingBack && r.missingBack.length) {
          recipHtml = '<span class="state-pill state-pill--warn" title="' +
                      esc(r.missingBack.join(" | ")) + '">Manquant (' + r.missingBack.length + ')</span>';
          recipText = "Manquant : " + r.missingBack.join(" | ");
          recipSort = 1;
        } else {
          recipHtml = pill("ok", "OK"); recipText = "OK"; recipSort = 0;
        }
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: html, text: text, sort: hl.length },
          { html: recipHtml, text: recipText, sort: recipSort }
        ];
      },
      // Surligne la ligne si un lien retour hreflang manque.
      rowClass: function (p) { return hasHreflangMissingBack(p) ? "row--alert" : ""; }
    },

    // ----- Pagination (Vague 2) : rel=prev / rel=next -----
    // N'affiche QUE les pages ayant au moins l'un des deux liens de pagination.
    pagination: {
      kind: "pages",
      // Filtre specifique a l'onglet : conserve seulement les pages paginees.
      filter: function (p) {
        var s = seoOf(p);
        return !!(s && (s.pagination_prev || s.pagination_next));
      },
      columns: [
        { key: "url",  label: "Adresse" },
        { key: "prev", label: "rel=prev" },
        { key: "next", label: "rel=next" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var prev = s && s.pagination_prev ? s.pagination_prev : "";
        var next = s && s.pagination_next ? s.pagination_next : "";
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: prev ? urlCellHtml(prev) : '<span class="muted">—</span>', text: prev, sort: prev },
          { html: next ? urlCellHtml(next) : '<span class="muted">—</span>', text: next, sort: next }
        ];
      }
    },

    // ----- Structured Data -----
    // Colonnes : Adresse, Types, Valide (structured_data_valid), Erreurs (structured_data_errors).
    // Une ligne est en alerte si la page a des donnees structurees invalides.
    schema: {
      kind: "pages",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "types",  label: "Types" },
        { key: "valid",  label: "Valide" },
        { key: "errors", label: "Erreurs", num: true }
      ],
      row: function (p) {
        var s = seoOf(p);
        var types = s && Array.isArray(s.structured_data_types) ? s.structured_data_types : [];
        var errors = s && Array.isArray(s.structured_data_errors) ? s.structured_data_errors : [];
        var hasData = types.length > 0;
        var valid = s ? !!s.structured_data_valid : true;
        // D) Badges de type CLIQUABLES : un clic ouvre le detail JSON-LD brut.
        var typesHtml = hasData
          ? types.map(function (t) {
              return '<span class="badge badge--schema" title="Voir le JSON-LD brut">' + esc(t) + '</span>';
            }).join(" ")
          : '<span class="muted">—</span>';
        // "Valide" n'a de sens que si la page porte des donnees structurees.
        var validHtml, validText, validSort;
        if (!hasData) { validHtml = '<span class="bool-no">—</span>'; validText = ""; validSort = -1; }
        else if (valid) { validHtml = pill("ok", "oui"); validText = "oui"; validSort = 1; }
        else { validHtml = pill("err", "non"); validText = "non"; validSort = 0; }
        // Erreurs : nombre + premiere erreur en infobulle.
        var errHtml, errText;
        if (!errors.length) { errHtml = '<span class="col-num">0</span>'; errText = "0"; }
        else {
          errHtml = '<span class="col-num num-bad" title="' + esc(errors.join(" | ")) + '">' + errors.length + '</span>';
          errText = errors.join(", ");
        }
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: typesHtml, text: types.join(", "), sort: types.length },
          { html: validHtml, text: validText, sort: validSort },
          { html: errHtml, text: errText, sort: errors.length }
        ];
      },
      // Surligne la ligne si la page a des donnees structurees invalides.
      rowClass: function (p) {
        var s = seoOf(p);
        var types = s && Array.isArray(s.structured_data_types) ? s.structured_data_types : [];
        return (types.length && s && !s.structured_data_valid) ? "row--alert" : "";
      }
    },

    // ----- Liens : tous les outlinks de toutes les pages -----
    links: {
      kind: "links",
      columns: [
        { key: "from",     label: "Source" },
        { key: "target",   label: "Destination" },
        { key: "anchor",   label: "Ancre" },
        { key: "type",     label: "Type" },
        { key: "rel",      label: "Rel" }
      ],
      // Les lignes sont produites par allLinks() (voir plus bas).
      row: function (l) {
        return [
          { html: urlCellHtml(l.from),   text: l.from || "",   sort: l.from || "" },
          { html: urlCellHtml(l.target), text: l.target || "", sort: l.target || "" },
          { html: esc(l.anchor || "—"),  text: l.anchor || "", sort: l.anchor || "" },
          { html: l.internal ? pill("ok", "Interne") : pill("muted", "Externe"),
            text: l.internal ? "Interne" : "Externe", sort: l.internal ? 0 : 1 },
          { html: esc(l.rel || "—"), text: l.rel || "", sort: l.rel || "" }
        ];
      }
    },

    // ----- Problemes : issues agregees -----
    issues: {
      kind: "issues",
      columns: [
        { key: "severity", label: "Severite" },
        { key: "category", label: "Categorie" },
        { key: "url",      label: "Adresse" },
        { key: "message",  label: "Message" }
      ],
      row: function (it) {
        return [
          { html: pill(sevKind(it.severity), sevLabel(it.severity)), text: it.severity, sort: sevRank(it.severity) },
          { html: esc(it.category || "—"), text: it.category || "", sort: it.category || "" },
          { html: urlCellHtml(it.url), text: it.url || "", sort: it.url || "" },
          { html: esc(it.message || ""), text: it.message || "", sort: it.message || "" }
        ];
      }
    },

    // ----- Sitemap (Vague 3) -----
    // Source dediee (kind "sitemap") : voir sitemapItems(). Deux vues commutables
    // via le sous-filtre S.sitemapView :
    //   "all"    : toutes les URLs du sitemap + statut Crawlee / Orpheline ;
    //   "notin"  : pages crawlees ABSENTES du sitemap (couverture inverse).
    sitemap: {
      kind: "sitemap",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "status", label: "Etat" },
        { key: "code",   label: "Code HTTP", num: true }
      ],
      row: function (it) {
        // it : { url, inSitemap, crawled, status }
        var stHtml, stText, stSort;
        if (it.kind === "orphan") {
          // Dans le sitemap mais jamais atteinte par le crawl.
          stHtml = '<span class="state-pill state-pill--warn" title="Declaree dans le sitemap mais non atteinte par le crawl">Orpheline</span>';
          stText = "Orpheline (sitemap, non crawlee)"; stSort = 1;
        } else if (it.kind === "notin") {
          // Crawlee mais absente du sitemap.
          stHtml = '<span class="state-pill state-pill--muted" title="Page atteinte par le crawl mais absente du sitemap">Hors sitemap</span>';
          stText = "Hors sitemap"; stSort = 2;
        } else {
          // Dans le sitemap ET crawlee.
          stHtml = pill("ok", "Crawlee"); stText = "Crawlee"; stSort = 0;
        }
        var codeHtml = it.crawled
          ? '<span class="http-code ' + httpClass(it.status) + '">' + esc(it.status || 0) + '</span>'
          : '<span class="muted">—</span>';
        return [
          { html: urlCellHtml(it.url), text: it.url || "", sort: it.url || "" },
          { html: stHtml, text: stText, sort: stSort },
          { html: codeHtml, text: it.crawled ? String(it.status || 0) : "", sort: it.status || 0 }
        ];
      },
      rowClass: function (it) {
        return (it.kind === "orphan") ? "row--alert" : "";
      }
    },

    // ----- Bande passante : tableau de mesure (poids transfere reel) -----
    bandwidth: {
      kind: "pages",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "tech",   label: "Techno" },
        { key: "weight", label: "Poids total", num: true },
        { key: "speed",  label: "Vitesse", num: true },
        { key: "imgs",   label: "Images", num: true },
        { key: "req",    label: "Requetes", num: true },
        { key: "dyn",    label: "dont dyn.", num: true },
        { key: "miss",   label: "Cache MISS", num: true },
        { key: "probe",  label: "Sonde" },
        { key: "bar",    label: "" }
      ],
      row: function (p) {
        var tech = Array.isArray(p.tech) ? p.tech : [];
        var imgs = Array.isArray(p.images) ? p.images : [];
        var imgBytes = imgs.reduce(function (s, i) { return s + (i && i.bytes ? i.bytes : 0); }, 0);
        var weight = p.total_bytes || 0;
        var dyn = p.dynamic_requests || 0;
        var miss = p.vercel_cache_misses || 0;
        var pct = S.bw.maxWeight > 0 ? Math.round((weight / S.bw.maxWeight) * 100) : 0;
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: tech.length ? tech.map(function (t) { return '<span class="badge">' + esc(t) + '</span>'; }).join("")
                              : '<span class="muted">—</span>', text: tech.join(", "), sort: tech.join(",") },
          { html: '<span class="col-num">' + fmtBytes(weight) + '</span>', text: String(weight), sort: weight },
          // VAGUE 4 (A) : temps de reponse colore, triable.
          { html: '<span class="col-num">' + speedCellHtml(respMs(p)) + '</span>',
            text: respMs(p) ? String(respMs(p)) : "", sort: respMs(p) },
          { html: '<span class="col-num">' + imgs.length + " / " + fmtBytes(imgBytes) + '</span>',
            text: String(imgBytes), sort: imgBytes },
          { html: '<span class="col-num">' + (p.request_count || 0) + '</span>', text: String(p.request_count || 0), sort: p.request_count || 0 },
          { html: '<span class="col-num">' + dyn + '</span>', text: String(dyn), sort: dyn },
          { html: '<span class="col-num">' + miss + '</span>', text: String(miss), sort: miss },
          { html: '<span class="badge">' + esc(probeLabel(p.measured_with)) + '</span>', text: probeLabel(p.measured_with), sort: p.measured_with || "" },
          { html: '<span class="wbar"><i style="width:' + pct + '%"></i></span>', text: "", sort: weight }
        ];
      }
    }
  };

  // Type de contenu abrege pour la colonne "Type" (text/html -> HTML, etc.).
  function shortType(ct) {
    if (!ct) return "—";
    ct = String(ct).toLowerCase();
    if (ct.indexOf("html") !== -1) return "HTML";
    if (ct.indexOf("json") !== -1) return "JSON";
    if (ct.indexOf("javascript") !== -1) return "JS";
    if (ct.indexOf("css") !== -1) return "CSS";
    if (ct.indexOf("xml") !== -1) return "XML";
    if (ct.indexOf("image/") !== -1) return ct.split("/")[1].toUpperCase();
    if (ct.indexOf("pdf") !== -1) return "PDF";
    return ct.split(";")[0];
  }

  // ----- Logique d'etat (manquant / court / long / ok) ---------------------
  function titleState(title, len) {
    if (!title) return { kind: "err",  label: "Manquant" };
    if (len < 30) return { kind: "warn", label: "Court" };
    if (len > 60) return { kind: "warn", label: "Long" };
    return { kind: "ok", label: "OK" };
  }
  function metaState(meta, len) {
    if (!meta) return { kind: "err",  label: "Manquant" };
    if (len < 70) return { kind: "warn", label: "Court" };
    if (len > 160) return { kind: "warn", label: "Long" };
    return { kind: "ok", label: "OK" };
  }
  function h1State(count) {
    if (count === 0) return { kind: "err",  label: "Manquant" };
    if (count > 1)   return { kind: "warn", label: "Multiple" };
    return { kind: "ok", label: "OK" };
  }
  function canonState(pageUrl, canon) {
    if (!canon) return { kind: "warn", label: "Manquant" };
    if (normUrl(canon) === normUrl(pageUrl)) return { kind: "ok", label: "Self" };
    return { kind: "muted", label: "Autre" };
  }
  function sevKind(sev) {
    if (sev === "critical") return "err";
    if (sev === "warning") return "warn";
    if (sev === "opportunity") return "muted";
    return "muted";
  }
  function sevLabel(sev) {
    if (sev === "critical")    return "Critique";
    if (sev === "warning")     return "Avertissement";
    if (sev === "opportunity") return "Opportunite";
    return "Info";
  }
  function sevRank(sev) {
    if (sev === "critical") return 0;
    if (sev === "warning") return 1;
    if (sev === "opportunity") return 2;
    return 3;
  }

  // =========================================================================
  // INGESTION D'UNE PAGE (event crawl:page)
  // =========================================================================
  function ingestPage(payload) {
    var p = payload || {};
    var key = normUrl(p.url);
    // IDEMPOTENT (Vague 4 / point C) : si l'URL est deja connue (cas d'une
    // REPRISE qui re-crawle), on REMPLACE l'entree en place au lieu d'empiler
    // un doublon. En crawl normal chaque URL est unique : comportement inchange.
    var existing = S.byUrl[key];
    if (existing && S.pageIndex[key] != null) {
      S.pages[S.pageIndex[key]] = p;
    } else {
      S.pageIndex[key] = S.pages.length;
      S.pages.push(p);
    }
    S.byUrl[key] = p;

    // Les agregats derives (inlinks, issues, severites, bande passante) sont
    // desormais RECALCULES par frame dans rebuildAggregates() — ainsi une page
    // re-ingeree ne double pas les compteurs.
    scheduleRender();
  }

  // (Re)construit en O(total) les agregats derives de S.pages :
  //   - inlinksIndex : index inverse cible -> [{from, anchor, rel}] ;
  //   - issues + sevCounts : problemes backend agreges par severite ;
  //   - bw : poids total cumule + poids max d'une page (barre bande passante).
  // Recalcule a chaque cycle de rendu => robuste a la re-ingestion (reprise).
  function rebuildAggregates() {
    var inlinks = {};
    var issues = [];
    var sev = { critical: 0, warning: 0, opportunity: 0, info: 0 };
    var totalBytes = 0, maxWeight = 0;

    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];

      var outs = Array.isArray(p.outlinks) ? p.outlinks : [];
      for (var k = 0; k < outs.length; k++) {
        var l = outs[k];
        if (!l || !l.target) continue;
        var t = normUrl(l.target);
        (inlinks[t] || (inlinks[t] = [])).push({ from: p.url, anchor: l.anchor || "", rel: l.rel || "" });
      }

      var iss = Array.isArray(p.issues) ? p.issues : [];
      for (var j = 0; j < iss.length; j++) {
        var it = iss[j] || {};
        var sv = it.severity || "info";
        issues.push({ url: it.url || p.url || "", severity: sv,
                      category: it.category || "", message: it.message || "" });
        if (sev[sv] === undefined) sev[sv] = 0;
        sev[sv] += 1;
      }

      var w = p.total_bytes || 0;
      totalBytes += w;
      if (w > maxWeight) maxWeight = w;
    }

    S.inlinksIndex = inlinks;
    S.issues = issues;
    S.sevCounts = sev;
    S.bw.totalBytes = totalBytes;
    S.bw.maxWeight = maxWeight;
  }

  // =========================================================================
  // RENDU BATCH (performant pour gros crawls)
  // On marque l'etat "dirty" et on declenche un seul rendu par frame.
  // =========================================================================
  function scheduleRender() {
    S.dirty = true;
    if (S.rafPending) return;
    S.rafPending = true;
    requestAnimationFrame(function () {
      S.rafPending = false;
      if (!S.dirty) return;
      S.dirty = false;
      rebuildAggregates();   // Vague 4 : inlinks/issues/bw recalcules (idempotent)
      rebuildDupMaps();      // cartes de doublons fraiches pour grille + overview
      rebuildNearDup();      // Vague 3 : near-duplicates (Hamming sur simhash)
      rebuildHreflangRecip();// Vague 3 : reciprocite hreflang (lien retour)
      renderTabCounts();
      renderOverview();
      renderGrid();
      // Si une ligne est selectionnee, on rafraichit le panneau de detail :
      // les Inlinks peuvent grossir au fil du crawl (nouvelles pages liantes).
      if (S.selectedUrl) renderDetail();
    });
  }

  // =========================================================================
  // SOURCES DE LIGNES selon l'onglet (avec filtres recherche/categorie/sev)
  // =========================================================================

  // Pages filtrees par recherche d'URL + filtre de categorie (sidebar) +
  // filtre propre a l'onglet courant (ex: Pagination ne montre que les pages
  // ayant rel=prev/next).
  function pagesFiltered() {
    var q = S.search ? S.search.toLowerCase() : "";
    var cat = S.catFilter;
    var def = TABS[S.activeTab];
    var tabFilter = def && typeof def.filter === "function" ? def.filter : null;
    return S.pages.filter(function (p) {
      if (tabFilter && !tabFilter(p)) return false;
      if (q && (p.url || "").toLowerCase().indexOf(q) === -1) return false;
      if (cat && !cat.predicate(p)) return false;
      return true;
    });
  }

  // Tous les liens sortants de toutes les pages (onglet Liens).
  function allLinks() {
    var q = S.search ? S.search.toLowerCase() : "";
    var out = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var outs = Array.isArray(p.outlinks) ? p.outlinks : [];
      for (var k = 0; k < outs.length; k++) {
        var l = outs[k];
        if (!l) continue;
        if (q && (l.target || "").toLowerCase().indexOf(q) === -1 &&
                 (p.url || "").toLowerCase().indexOf(q) === -1) continue;
        out.push({ from: p.url, target: l.target, anchor: l.anchor, rel: l.rel, internal: !!l.internal });
      }
    }
    return out;
  }

  // Lignes de l'onglet Sitemap (Vague 3) selon la vue S.sitemapView :
  //   "all"   : chaque URL declaree dans le sitemap, marquee Crawlee/Orpheline ;
  //   "notin" : pages crawlees absentes du sitemap.
  // Filtre de recherche d'URL applique dans les deux cas.
  function sitemapItems() {
    var q = S.search ? S.search.toLowerCase() : "";
    var out = [];
    if (S.sitemapView === "notin") {
      // Couverture inverse : pages crawlees qui ne figurent pas dans le sitemap.
      for (var i = 0; i < S.pages.length; i++) {
        var p = S.pages[i];
        var key = normUrl(p.url);
        if (S.sitemapSet[key]) continue;                 // presente -> on ignore
        if (q && (p.url || "").toLowerCase().indexOf(q) === -1) continue;
        out.push({ url: p.url, inSitemap: false, crawled: true,
                   status: p.status || 0, kind: "notin" });
      }
      return out;
    }
    // Vue "all" : on parcourt les URLs du sitemap.
    for (var s = 0; s < S.sitemapUrls.length; s++) {
      var u = S.sitemapUrls[s];
      if (!u) continue;
      if (q && u.toLowerCase().indexOf(q) === -1) continue;
      var pg = S.byUrl[normUrl(u)];                       // page crawlee correspondante
      out.push({
        url: u,
        inSitemap: true,
        crawled: !!pg,
        status: pg ? (pg.status || 0) : 0,
        kind: pg ? "crawled" : "orphan"
      });
    }
    return out;
  }

  // Issues filtrees par recherche d'URL + filtre de severite.
  // Fusionne les issues backend (S.issues) avec les issues hreflang derivees
  // cote JS (S.hreflangIssues, categorie "Hreflang") — cf. rebuildHreflangRecip.
  function issuesFiltered() {
    var q = S.search ? S.search.toLowerCase() : "";
    var all = S.issues.concat(S.hreflangIssues);
    return all.filter(function (it) {
      if (S.sevFilter !== "all" && it.severity !== S.sevFilter) return false;
      if (q && (it.url || "").toLowerCase().indexOf(q) === -1) return false;
      return true;
    });
  }

  // Renvoie le tableau d'objets (pages, liens ou issues) pour l'onglet courant.
  function currentItems() {
    var def = TABS[S.activeTab];
    if (!def) return [];
    if (def.kind === "links") return allLinks();
    if (def.kind === "issues") return issuesFiltered();
    if (def.kind === "sitemap") return sitemapItems();
    return pagesFiltered();
  }

  // =========================================================================
  // RENDU DE LA GRILLE
  // =========================================================================
  function renderGrid() {
    var def = TABS[S.activeTab];
    var head = $("grid-head");
    var body = $("grid-body");
    if (!def || !head || !body) return;

    // Bandeau d'information contextuel. Cas Vague 3 : sur l'onglet Contenu, si le
    // calcul des near-duplicates a ete saute (trop de pages), on le signale —
    // sinon l'absence de near-dup serait trompeuse.
    var banner = $("grid-banner");
    if (banner) {
      if (S.activeTab === "content" && S.nearDupSkipped) {
        banner.hidden = false;
        banner.textContent = "Detection des near-duplicates desactivee : plus de " +
          NEAR_DUP_MAX_PAGES + " pages (le calcul O(n^2) serait trop couteux).";
      } else {
        banner.hidden = true;
      }
    }

    // --- En-tete (colonnes triables au clic) ---
    var hh = "<tr>";
    for (var c = 0; c < def.columns.length; c++) {
      var col = def.columns[c];
      var cls = "";
      if (S.sort.key === col.key) cls = S.sort.dir === "asc" ? "sort-asc" : "sort-desc";
      hh += '<th data-key="' + esc(col.key) + '" class="' + cls + '">' + esc(col.label) + '</th>';
    }
    hh += "</tr>";
    head.innerHTML = hh;

    // --- Lignes (mappees + triees) ---
    var items = currentItems();
    var mapped = items.map(function (item) {
      return { item: item, cells: def.row(item) };
    });

    if (S.sort.key) {
      var idx = -1;
      for (var ci = 0; ci < def.columns.length; ci++) {
        if (def.columns[ci].key === S.sort.key) { idx = ci; break; }
      }
      if (idx >= 0) {
        var dir = S.sort.dir === "asc" ? 1 : -1;
        mapped.sort(function (a, b) {
          var va = a.cells[idx].sort, vb = b.cells[idx].sort;
          var cmp;
          if (typeof va === "number" && typeof vb === "number") cmp = va - vb;
          else cmp = String(va).localeCompare(String(vb), "fr", { numeric: true, sensitivity: "base" });
          return cmp * dir;
        });
      }
    }

    // --- Construction HTML (une seule affectation innerHTML = perf) ---
    var rows = "";
    var selKey = S.selectedUrl ? normUrl(S.selectedUrl) : null;
    // rowClass : applique pour les onglets "pages" ET "sitemap" (surlignage alerte).
    var hasRowClass = (def.kind === "pages" || def.kind === "sitemap") && typeof def.rowClass === "function";
    for (var r = 0; r < mapped.length; r++) {
      var cells = mapped[r].cells;
      var item = mapped[r].item;
      var rowUrl = item.url || item.from || "";   // pages/issues -> url ; liens -> from
      // Classes cumulees : selection (ligne ouverte dans le detail) + alerte
      // eventuelle propre a l'onglet (URL trop longue, contenu mixte...).
      var classes = [];
      if (selKey && def.kind === "pages" && normUrl(item.url) === selKey) classes.push("is-selected");
      if (hasRowClass) { var rc = def.rowClass(item); if (rc) classes.push(rc); }
      var clsAttr = classes.length ? ' class="' + classes.join(" ") + '"' : "";
      rows += '<tr data-url="' + esc(rowUrl) + '"' + clsAttr + '>';
      for (var k = 0; k < cells.length; k++) rows += "<td>" + cells[k].html + "</td>";
      rows += "</tr>";
    }
    body.innerHTML = rows;

    // Message "grille vide".
    var empty = $("grid-empty");
    if (empty) {
      if (mapped.length) { empty.hidden = true; }
      else {
        empty.hidden = false;
        if (def.kind === "sitemap") {
          // Distingue "pas de sitemap du tout" de "vue/filtre sans resultat".
          if (S.sitemapView === "all" && !S.sitemapUrls.length) {
            empty.textContent = S.pages.length
              ? "Aucun sitemap trouve pour ce site."
              : "Lancez un crawl pour analyser le sitemap.";
          } else {
            empty.textContent = "Aucune URL ne correspond a cette vue.";
          }
        } else {
          empty.textContent = S.pages.length
            ? "Aucune ligne ne correspond au filtre."
            : "Lancez un crawl pour voir les resultats ici.";
        }
      }
    }
  }

  // =========================================================================
  // COMPTEURS DES ONGLETS (mis a jour en direct)
  // Compteur = nombre de lignes affichables (avant filtre recherche, pour
  // refleter le volume total ; SF affiche le total de l'onglet).
  // =========================================================================
  function renderTabCounts() {
    var nPages = S.pages.length;
    // Pagination : ne compte que les pages ayant rel=prev OU rel=next.
    var nPagination = 0;
    for (var pi = 0; pi < S.pages.length; pi++) {
      var sp = seoOf(S.pages[pi]);
      if (sp && (sp.pagination_prev || sp.pagination_next)) nPagination++;
    }
    var counts = {
      internal: nPages,
      response: nPages,
      url: nPages,
      security: nPages,
      titles: nPages,
      meta: nPages,
      h1: nPages,
      content: nPages,
      images: nPages,
      canonical: nPages,
      directives: nPages,
      hreflang: nPages,
      pagination: nPagination,
      schema: nPages,
      bandwidth: nPages,
      // Sitemap : compteur = nombre d'URLs declarees dans le sitemap.
      sitemap: S.sitemapUrls.length,
      links: S.pages.reduce(function (s, p) { return s + (Array.isArray(p.outlinks) ? p.outlinks.length : 0); }, 0),
      // Problemes : issues backend + issues hreflang derivees cote JS.
      issues: S.issues.length + S.hreflangIssues.length
    };
    document.querySelectorAll(".tab-count").forEach(function (el) {
      var key = el.getAttribute("data-count");
      el.textContent = counts[key] != null ? counts[key] : 0;
    });
  }

  // =========================================================================
  // SIDEBAR OVERVIEW : categories cliquables avec compteurs en direct.
  // Chaque categorie a un predicate(p) ; cliquer applique catFilter +
  // bascule sur l'onglet pertinent.
  // =========================================================================
  // Definition des categories (groupes -> lignes).
  var OVERVIEW = [
    { group: "Codes de reponse", tab: "response", rows: [
      { id: "resp2xx", label: "Succes (2xx)", tone: "ok",   pred: function (p) { return p.status >= 200 && p.status < 300; } },
      { id: "resp3xx", label: "Redirections (3xx)", tone: "warn", pred: function (p) { return p.status >= 300 && p.status < 400; } },
      { id: "resp4xx", label: "Erreurs client (4xx)", tone: "err", pred: function (p) { return p.status >= 400 && p.status < 500; } },
      { id: "resp5xx", label: "Erreurs serveur (5xx)", tone: "err", pred: function (p) { return p.status >= 500; } }
    ]},
    { group: "Titres", tab: "titles", rows: [
      { id: "titleMissing", label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || !s.title; } },
      { id: "titleLong",    label: "> 60 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.title && s.title_len > 60; } },
      { id: "titleShort",   label: "< 30 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.title && s.title_len < 30; } },
      { id: "titleDup",     label: "Duplique", tone: "warn", pred: dupPred("title", dupKeyTitle) }
    ]},
    { group: "Meta Description", tab: "meta", rows: [
      { id: "metaMissing", label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || !s.meta_description; } },
      { id: "metaLong",    label: "> 160 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.meta_description && s.meta_description_len > 160; } },
      { id: "metaShort",   label: "< 70 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.meta_description && s.meta_description_len < 70; } },
      { id: "metaDup",     label: "Duplique", tone: "warn", pred: dupPred("meta", dupKeyMeta) }
    ]},
    { group: "H1", tab: "h1", rows: [
      { id: "h1Missing",  label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || (s.h1_count || 0) === 0; } },
      { id: "h1Multiple", label: "Multiple", tone: "warn", pred: function (p) { var s = seoOf(p); return s && (s.h1_count || 0) > 1; } },
      { id: "h1Dup",      label: "Duplique", tone: "warn", pred: dupPred("h1", dupKeyH1) }
    ]},
    { group: "Contenu", tab: "content", rows: [
      { id: "nearDup", label: "Near-duplicates", tone: "warn", pred: function (p) { return nearDupOf(p).length > 0; } }
    ]},
    { group: "Images", tab: "images", rows: [
      { id: "imgMissingAlt", label: "Sans alt", tone: "warn", pred: function (p) { var s = seoOf(p); return s && (s.images_missing_alt || 0) > 0; } }
    ]},
    { group: "Canonicals", tab: "canonical", rows: [
      { id: "canonMissing", label: "Manquant", tone: "warn", pred: function (p) { var s = seoOf(p); return !s || !s.canonical; } },
      { id: "canonicalised", label: "Canonicalisees", tone: "warn", pred: function (p) { var s = seoOf(p); return !!(s && s.canonicalised); } }
    ]},
    { group: "Indexabilite", tab: "directives", rows: [
      { id: "idxYes", label: "Indexable", tone: "ok",  pred: function (p) { var s = seoOf(p); return s && s.indexable; } },
      { id: "idxNo",  label: "Non indexable", tone: "err", pred: function (p) { var s = seoOf(p); return !s || !s.indexable; } }
    ]},
    // ----- Nouveaux groupes Vague 2 -----
    { group: "URL", tab: "url", rows: [
      { id: "urlNonAscii", label: "Non-ASCII", tone: "warn", pred: function (p) { var f = flagsOf(p); return !!(f && f.non_ascii); } },
      { id: "urlLong",     label: "Longues (>115)", tone: "warn", pred: function (p) {
          var f = flagsOf(p);
          var len = f && f.length != null ? f.length : (p.url ? p.url.length : 0);
          return len > 115;
      } }
    ]},
    { group: "Securite", tab: "security", rows: [
      { id: "secMixed",   label: "Contenu mixte", tone: "err",  pred: function (p) { return mixedContentOf(p).length > 0; } },
      { id: "secHeaders", label: "En-tetes manquants", tone: "warn", pred: function (p) { return missingHeadersOf(p).length > 0; } }
    ]},
    { group: "Hreflang", tab: "hreflang", rows: [
      { id: "hreflangNoBack", label: "Sans lien retour", tone: "warn", pred: function (p) { return hasHreflangMissingBack(p); } }
    ]}
  ];

  // (Re)construit les cartes de doublons (title / meta / 1er h1) en UN passage
  // O(n) sur S.pages. Cle = valeur exacte (trim) -> nombre de pages partageant
  // cette valeur. Appelee une fois par cycle de rendu (cf. scheduleRender).
  function rebuildDupMaps() {
    var maps = { title: {}, meta: {}, h1: {} };
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var kt = dupKeyTitle(p); if (kt) maps.title[kt] = (maps.title[kt] || 0) + 1;
      var km = dupKeyMeta(p);  if (km) maps.meta[km]  = (maps.meta[km]  || 0) + 1;
      var kh = dupKeyH1(p);    if (kh) maps.h1[kh]    = (maps.h1[kh]    || 0) + 1;
    }
    S.dupMaps = maps;
  }

  // Vrai si la page p partage sa valeur (title/meta/h1) avec au moins une autre.
  // Lit la carte pre-calculee S.dupMaps[mapKey] (pas de boucle interne).
  function isDup(mapKey, keyFn, p) {
    var val = keyFn(p);
    if (!val) return false;
    return (S.dupMaps[mapKey][val] || 0) >= 2;
  }

  // Fabrique un predicate "duplique" pour la sidebar Overview, base sur les
  // cartes pre-calculees (>= 2 pages partageant la valeur exacte).
  function dupPred(mapKey, keyFn) {
    return function (p) { return isDup(mapKey, keyFn, p); };
  }

  // -------------------------------------------------------------------------
  // VAGUE 3 — (Re)construction de la carte des NEAR-DUPLICATES
  // O(n^2) BORNE : pour chaque paire de pages portant un simhash valide, on
  // calcule la distance de Hamming (popcount du XOR). Si <= NEAR_DUP_BITS, on
  // enregistre la reciprocite (A->B et B->A). On precalcule une seule fois les
  // BigInt par page pour ne pas reparser le hex a chaque comparaison.
  // Garde-fou : au-dela de NEAR_DUP_MAX_PAGES pages avec simhash, on saute le
  // calcul (le cout n^2 deviendrait prohibitif) et on leve un drapeau affiche
  // dans l'UI.
  // -------------------------------------------------------------------------
  function rebuildNearDup() {
    var map = {};
    S.nearDupSkipped = false;

    // 1) Collecte des pages ayant un simhash exploitable (+ BigInt precalcule).
    var items = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var hex = simhashOf(p);
      if (!hex) continue;
      items.push({ key: normUrl(p.url), big: simhashBig(hex) });
    }

    // 2) Garde-fou volumetrie : O(n^2) borne.
    if (items.length > NEAR_DUP_MAX_PAGES) {
      S.nearDup = {};
      S.nearDupSkipped = true;
      return;
    }

    // 3) Comparaison de toutes les paires (i<j) -> distance de Hamming.
    for (var a = 0; a < items.length; a++) {
      for (var b = a + 1; b < items.length; b++) {
        var dist = popcountBig(items[a].big ^ items[b].big);   // XOR + popcount
        if (dist <= NEAR_DUP_BITS) {
          (map[items[a].key] || (map[items[a].key] = [])).push(items[b].key);
          (map[items[b].key] || (map[items[b].key] = [])).push(items[a].key);
        }
      }
    }
    S.nearDup = map;
  }

  // -------------------------------------------------------------------------
  // VAGUE 3 — (Re)construction de la carte HREFLANG RECIPROQUE
  // Pour chaque page A qui declare un hreflang vers une URL B :
  //   - si B est CRAWLEE (presente dans S.byUrl) ET que B ne declare PAS de
  //     hreflang retour vers A  => "lien retour manquant" (probleme SEO).
  //   - si B n'est pas crawlee, on ne peut pas conclure (ignore ici).
  // On indexe d'abord, par page, l'ensemble des cibles hreflang declarees
  // (normalisees), puis on teste la reciprocite en O(liens).
  // Resultat : normUrl(A) -> { count, missingBack:[urls B sans retour] }.
  // -------------------------------------------------------------------------
  function rebuildHreflangRecip() {
    var recip = {};

    // 1) Index : normUrl(page) -> Set des cibles hreflang declarees (normalisees).
    var declares = {};
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var s = seoOf(p);
      var hl = s && Array.isArray(s.hreflang) ? s.hreflang : [];
      if (!hl.length) continue;
      var set = (declares[normUrl(p.url)] = {});
      for (var k = 0; k < hl.length; k++) {
        if (hl[k] && hl[k].href) set[normUrl(hl[k].href)] = true;
      }
    }

    // 2) Test de reciprocite : pour A->B, B crawlee mais B ne pointe pas vers A.
    for (var j = 0; j < S.pages.length; j++) {
      var pa = S.pages[j];
      var sa = seoOf(pa);
      var hla = sa && Array.isArray(sa.hreflang) ? sa.hreflang : [];
      if (!hla.length) continue;
      var aKey = normUrl(pa.url);
      var missing = [];
      var seen = {};
      for (var m = 0; m < hla.length; m++) {
        var href = hla[m] && hla[m].href ? hla[m].href : "";
        if (!href) continue;
        var bKey = normUrl(href);
        if (bKey === aKey) continue;            // auto-reference : ignoree
        if (seen[bKey]) continue; seen[bKey] = true;
        if (!S.byUrl[bKey]) continue;           // cible non crawlee : indecidable
        var bDecl = declares[bKey];
        if (!bDecl || !bDecl[aKey]) missing.push(href);  // pas de retour vers A
      }
      recip[aKey] = { count: hla.length, missingBack: missing };
    }
    S.hreflangRecip = recip;

    // Derive les issues "Hreflang sans lien retour" (severite warning) pour
    // l'onglet Problemes. Une issue par page concernee, message listant les
    // cibles sans reciprocite. Recalculee a chaque cycle (pas d'accumulation).
    var issues = [];
    for (var u = 0; u < S.pages.length; u++) {
      var pp = S.pages[u];
      var rr = recip[normUrl(pp.url)];
      if (rr && rr.missingBack && rr.missingBack.length) {
        issues.push({
          url: pp.url || "",
          severity: "warning",
          category: "Hreflang",
          message: "Hreflang sans lien retour vers : " + rr.missingBack.join(", ")
        });
      }
    }
    S.hreflangIssues = issues;
  }

  // Vrai si la page p a au moins un hreflang sans lien retour reciproque.
  function hasHreflangMissingBack(p) {
    var r = S.hreflangRecip[normUrl(p.url)];
    return !!(r && r.missingBack && r.missingBack.length);
  }

  function countFor(pred) {
    var n = 0;
    for (var i = 0; i < S.pages.length; i++) if (pred(S.pages[i])) n++;
    return n;
  }

  // ----- Comptages SITEMAP (Vague 3) pour la sidebar Overview --------------
  // Orphelines : URLs du sitemap non atteintes par le crawl.
  function countSitemapOrphans() {
    var n = 0;
    for (var i = 0; i < S.sitemapUrls.length; i++) {
      var u = S.sitemapUrls[i];
      if (u && !S.byUrl[normUrl(u)]) n++;
    }
    return n;
  }
  // Hors sitemap : pages crawlees absentes du sitemap (uniquement si on a un sitemap).
  function countNotInSitemap() {
    if (!S.sitemapUrls.length) return 0;
    var n = 0;
    for (var i = 0; i < S.pages.length; i++) {
      if (!S.sitemapSet[normUrl(S.pages[i].url)]) n++;
    }
    return n;
  }

  // Fabrique une ligne d'Overview pour le groupe Sitemap (data-sm = vue cible).
  // "all"/"notin" basculent S.sitemapView ; "orphan" pointe la vue "all" (les
  // orphelines y sont surlignees). is-active suit S.activeTab + S.sitemapView.
  function smRow(view, label, n, tone) {
    var toneCls = n > 0 && tone ? (" ov-" + tone) : "";
    var targetView = (view === "notin") ? "notin" : "all";
    var active = (S.activeTab === "sitemap" && S.sitemapView === targetView &&
                  (view !== "orphan")) ? " is-active" : "";
    return '<div class="ov-row' + toneCls + active + '" data-sm="' + view +
           '" data-empty="' + (n ? 0 : 1) + '">' +
           '<span class="ov-label">' + esc(label) + '</span>' +
           '<span class="ov-count">' + n + '</span></div>';
  }

  function renderOverview() {
    var tree = $("overview-tree");
    if (!tree) return;
    var html = "";
    for (var g = 0; g < OVERVIEW.length; g++) {
      var grp = OVERVIEW[g];
      html += '<div class="ov-group"><div class="ov-group-title">' + esc(grp.group) + '</div>';
      for (var r = 0; r < grp.rows.length; r++) {
        var row = grp.rows[r];
        var n = countFor(row.pred);
        var toneCls = n > 0 ? (" ov-" + row.tone) : "";
        var activeCls = (S.catId === row.id) ? " is-active" : "";
        html += '<div class="ov-row' + toneCls + activeCls + '" data-cat="' + row.id +
                '" data-tab="' + grp.tab + '" data-empty="' + (n ? 0 : 1) + '">' +
                '<span class="ov-label">' + esc(row.label) + '</span>' +
                '<span class="ov-count">' + n + '</span></div>';
      }
      html += '</div>';
    }

    // ----- Groupe SITEMAP (Vague 3) : rendu special (lignes data-sm) ---------
    // Ses compteurs ne reposent pas sur des predicates de page (les orphelines
    // ne sont pas dans S.pages), d'ou un rendu et un handler dedies.
    var nOrphan = countSitemapOrphans();
    var nNotIn  = countNotInSitemap();
    var nSmTotal = S.sitemapUrls.length;
    html += '<div class="ov-group"><div class="ov-group-title">Sitemap</div>';
    html += smRow("all",   "URLs du sitemap", nSmTotal, nSmTotal ? "ok" : "");
    html += smRow("orphan","Orphelines",      nOrphan, "warn");
    html += smRow("notin", "Hors sitemap",    nNotIn,  "warn");
    html += '</div>';

    tree.innerHTML = html;

    // Compteurs des filtres de severite (onglet Problemes). Les issues hreflang
    // derivees cote JS sont toutes de severite "warning" : on les ajoute au
    // total et au compteur "warning".
    var nHreflangIss = S.hreflangIssues.length;
    setSevCount("all", S.issues.length + nHreflangIss);
    setSevCount("critical", S.sevCounts.critical || 0);
    setSevCount("warning", (S.sevCounts.warning || 0) + nHreflangIss);
    setSevCount("opportunity", S.sevCounts.opportunity || 0);
  }

  // Met a jour un <span data-sevcount="X"> (compteurs des filtres de severite).
  function setSevCount(val, n) {
    var el = document.querySelector('[data-sevcount="' + val + '"]');
    if (el) el.textContent = n;
  }

  // Retrouve la definition d'une categorie par son id.
  function findCat(id) {
    for (var g = 0; g < OVERVIEW.length; g++) {
      for (var r = 0; r < OVERVIEW[g].rows.length; r++) {
        if (OVERVIEW[g].rows[r].id === id) {
          return { row: OVERVIEW[g].rows[r], tab: OVERVIEW[g].tab };
        }
      }
    }
    return null;
  }

  // =========================================================================
  // PANNEAU DE DETAIL (au clic sur une ligne de page)
  // =========================================================================
  function openDetail(url) {
    var p = S.byUrl[normUrl(url)];
    if (!p) return;
    S.selectedUrl = url;
    var detail = $("detail");
    if (detail) detail.hidden = false;
    // Marque la ligne selectionnee dans la grille.
    document.querySelectorAll("#grid-body tr").forEach(function (tr) {
      tr.classList.toggle("is-selected", normUrl(tr.getAttribute("data-url")) === normUrl(url));
    });
    renderDetail();
  }

  function closeDetail() {
    S.selectedUrl = null;
    var detail = $("detail");
    if (detail) detail.hidden = true;
    document.querySelectorAll("#grid-body tr.is-selected").forEach(function (tr) {
      tr.classList.remove("is-selected");
    });
  }

  function renderDetail() {
    if (!S.selectedUrl) return;
    var p = S.byUrl[normUrl(S.selectedUrl)];
    if (!p) return;

    // Compteurs des sous-onglets.
    var inl = S.inlinksIndex[normUrl(S.selectedUrl)] || [];
    var outs = Array.isArray(p.outlinks) ? p.outlinks : [];
    setText("dt-inlinks-count", inl.length);
    setText("dt-outlinks-count", outs.length);
    // D) Compteur de blocs JSON-LD bruts presents sur la page.
    setText("dt-schema-count", structuredRawOf(p).length);

    // Onglets actifs.
    document.querySelectorAll(".dtab").forEach(function (b) {
      b.classList.toggle("is-active", b.getAttribute("data-dtab") === S.detailTab);
    });

    var body = $("detail-body");
    if (!body) return;

    if (S.detailTab === "inlinks") {
      body.innerHTML = renderLinkTable(
        inl.map(function (x) { return { url: x.from, anchor: x.anchor, rel: x.rel }; }),
        ["Source (qui pointe ici)", "Ancre", "Rel"],
        ["url", "anchor", "rel"],
        "Aucun lien entrant detecte parmi les pages crawlees."
      );
    } else if (S.detailTab === "outlinks") {
      body.innerHTML = renderLinkTable(
        outs.map(function (l) { return { url: l.target, anchor: l.anchor, rel: l.rel, internal: l.internal }; }),
        ["Destination", "Ancre", "Type", "Rel"],
        ["url", "anchor", "type", "rel"],
        "Aucun lien sortant sur cette page."
      );
    } else if (S.detailTab === "serp") {
      body.innerHTML = renderSerp(p);
    } else if (S.detailTab === "info") {
      body.innerHTML = renderInfo(p);
    } else if (S.detailTab === "schema") {
      body.innerHTML = renderStructuredData(p);   // D) JSON-LD bruts indentes
    } else if (S.detailTab === "source") {
      body.innerHTML = renderSource(p);
    }
  }

  // ----- VAGUE 4 (D) — Donnees structurees brutes (blocs JSON-LD) -----------
  // Lit seo.structured_data_raw (tableau de chaines JSON-LD brutes), ou [].
  function structuredRawOf(p) {
    var s = seoOf(p);
    return s && Array.isArray(s.structured_data_raw) ? s.structured_data_raw : [];
  }

  // Tente d'extraire un libelle de type lisible d'un objet JSON-LD parse
  // (champ @type, qui peut etre une chaine ou un tableau). Renvoie "" sinon.
  function jsonLdTypeLabel(obj) {
    if (!obj || typeof obj !== "object") return "";
    var t = obj["@type"];
    if (!t && obj["@graph"]) return "@graph";   // graphe : types multiples a l'interieur
    if (Array.isArray(t)) return t.join(", ");
    return t ? String(t) : "";
  }

  // Rendu du sous-onglet "Donnees structurees" : pour chaque bloc JSON-LD brut,
  // on tente JSON.parse + JSON.stringify(obj, null, 2) (indente, lisible) ; si le
  // parse echoue (JSON-LD malforme), on affiche le texte brut tel quel. Chaque
  // bloc est precede d'un petit en-tete (index + type detecte via @type).
  function renderStructuredData(p) {
    var raw = structuredRawOf(p);
    if (!raw.length) {
      // Repli informatif : on signale les types detectes meme sans bloc brut
      // (utile si le backend ne fournit pas structured_data_raw pour cette page).
      var s = seoOf(p);
      var types = s && Array.isArray(s.structured_data_types) ? s.structured_data_types : [];
      if (types.length) {
        return '<div class="detail-empty">Types detectes : ' +
               types.map(function (t) { return '<span class="badge">' + esc(t) + '</span>'; }).join(" ") +
               '<br>Aucun bloc JSON-LD brut fourni pour cette page.</div>';
      }
      return '<div class="detail-empty">Aucune donnee structuree (JSON-LD) sur cette page.</div>';
    }
    var html = '<div class="sd-list">';
    for (var i = 0; i < raw.length; i++) {
      var block = raw[i];
      var pretty, label = "";
      try {
        var obj = JSON.parse(block);
        pretty = JSON.stringify(obj, null, 2);     // indentation 2 espaces
        label = jsonLdTypeLabel(obj);
      } catch (e) {
        pretty = String(block);                    // JSON invalide : brut tel quel
        label = "(JSON invalide)";
      }
      html += '<div class="sd-block">' +
              '<div class="sd-block-head">Bloc ' + (i + 1) +
              (label ? ' — <span class="sd-type">' + esc(label) + '</span>' : '') +
              '</div>' +
              '<pre class="sd-json">' + esc(pretty) + '</pre>' +
              '</div>';
    }
    html += '</div>';
    return html;
  }

  // Tableau generique pour inlinks / outlinks.
  function renderLinkTable(items, headers, keys, emptyMsg) {
    if (!items.length) return '<div class="detail-empty">' + esc(emptyMsg) + '</div>';
    var h = '<table class="detail-table"><thead><tr>';
    headers.forEach(function (x) { h += "<th>" + esc(x) + "</th>"; });
    h += "</tr></thead><tbody>";
    for (var i = 0; i < items.length; i++) {
      var it = items[i];
      h += "<tr>";
      for (var k = 0; k < keys.length; k++) {
        var key = keys[k];
        if (key === "url") h += "<td>" + urlCellHtml(it.url) + "</td>";
        else if (key === "type") h += "<td>" + (it.internal ? pill("ok", "Interne") : pill("muted", "Externe")) + "</td>";
        else h += "<td>" + esc(it[key] || "—") + "</td>";
      }
      h += "</tr>";
    }
    h += "</tbody></table>";
    return h;
  }

  // Apercu SERP (title bleu, url verte, meta grise).
  function renderSerp(p) {
    var s = seoOf(p);
    var title = s && s.title ? s.title : "(titre manquant)";
    var meta = s && s.meta_description ? s.meta_description : "(meta description manquante)";
    var url = p.url || "";
    var pretty = url.replace(/^https?:\/\//, "").replace(/\/$/, "");
    pretty = pretty.replace(/\//g, " › ");
    return '<div class="serp">' +
           '<div class="serp-url">' + esc(pretty) + '</div>' +
           '<div class="serp-title">' + esc(title) + '</div>' +
           '<div class="serp-desc">' + esc(meta) + '</div>' +
           '<div class="serp-note">Apercu indicatif du rendu dans les resultats Google.</div>' +
           '</div>';
  }

  // --- Helpers texte pour les champs Securite du panneau Infos (Vague 2) ---
  // Repli sur le schema de l'URL si l'objet security n'est pas fourni.
  function secInfoHttps(p) {
    var sec = secOf(p);
    if (sec && typeof sec.https === "boolean") return sec.https ? "Oui" : "Non";
    return (p.url && /^https:/i.test(p.url)) ? "Oui" : "Non";
  }
  function secInfoMixed(p) {
    var m = mixedContentOf(p);
    if (!m.length) return "Aucun";
    // On liste les premieres ressources mixtes pour faciliter le diagnostic.
    var head = m.slice(0, 5).join("  |  ");
    return m.length + " ressource(s) : " + head + (m.length > 5 ? "  | ..." : "");
  }
  function secInfoHeaders(p) {
    var h = missingHeadersOf(p);
    return h.length ? h.join(", ") : "Aucun";
  }
  // --- Helpers texte pour les donnees structurees (Vague 2) ---
  // "Valide" n'a de sens que si la page porte des donnees structurees.
  function schemaInfoValid(p) {
    var s = seoOf(p);
    var types = s && Array.isArray(s.structured_data_types) ? s.structured_data_types : [];
    if (!types.length) return "—";
    return (s && s.structured_data_valid) ? "Oui" : "Non";
  }
  function schemaInfoErrors(p) {
    var s = seoOf(p);
    var errs = s && Array.isArray(s.structured_data_errors) ? s.structured_data_errors : [];
    return errs.length ? errs.join("  |  ") : "Aucune";
  }

  // --- Helpers panneau detail (Vague 3) ---
  // Liste HTML des URLs quasi-dupliquees a la page p (liens cliquables), ou "—".
  function nearDupHtml(p) {
    var sims = nearDupOf(p);
    if (!sims.length) return '<span class="muted">Aucune</span>';
    // Les cles memorisees sont des URLs normalisees ; on retrouve l'URL d'origine
    // via S.byUrl pour un affichage propre (sinon on retombe sur la cle).
    var parts = sims.map(function (k) {
      var pg = S.byUrl[k];
      var u = pg && pg.url ? pg.url : k;
      return urlCellHtml(u);
    });
    return sims.length + ' page(s) : ' + parts.join("  |  ");
  }
  // Texte de l'etat de reciprocite hreflang pour la page p.
  function hreflangRecipHtml(p) {
    var s = seoOf(p);
    var hl = s && Array.isArray(s.hreflang) ? s.hreflang : [];
    if (!hl.length) return '<span class="muted">—</span>';
    var r = S.hreflangRecip[normUrl(p.url)];
    if (r && r.missingBack && r.missingBack.length) {
      var parts = r.missingBack.map(function (u) { return urlCellHtml(u); });
      return '<span class="bool-yes">Lien retour manquant</span> : ' + parts.join("  |  ");
    }
    return '<span style="color:var(--ok);font-weight:600">OK (reciproque)</span>';
  }

  // Vue "Infos / En-tetes" : liste cle/valeur.
  function renderInfo(p) {
    var s = seoOf(p);
    var rows = [
      ["Adresse", p.url || ""],
      ["Code HTTP", String(p.status || 0)],
      ["Content-Type", p.content_type || "—"],
      ["Sonde", probeLabel(p.measured_with)],
      ["Poids total", fmtBytes(p.total_bytes || 0)],
      ["Requetes", String(p.request_count || 0) + " (dont " + (p.dynamic_requests || 0) + " dyn.)"],
      ["Redirige vers", p.redirect_to || "—"],
      ["Chaine de redirection", (Array.isArray(p.redirect_chain) && p.redirect_chain.length) ? p.redirect_chain.join("  ->  ") : "—"],
      ["Canonical", (s && s.canonical) ? s.canonical : "—"],
      ["Canonicalisee", (s && s.canonicalised) ? "Oui" : "Non"],
      ["Meta robots", (s && s.meta_robots) ? s.meta_robots : "—"],
      ["X-Robots-Tag", (s && s.x_robots_tag) ? s.x_robots_tag : "—"],
      ["Indexable", (s && s.indexable) ? "Oui" : "Non"],
      ["Langue", (s && s.lang) ? s.lang : "—"],
      ["Viewport", (s && s.viewport) ? s.viewport : "—"],
      ["Charset", (s && s.charset) ? s.charset : "—"],
      ["Mots", s ? String(s.word_count || 0) : "0"],
      ["Ratio texte/code", s ? (Math.round((s.text_ratio || 0) * 1000) / 10) + " %" : "—"],
      ["Liens internes / externes", s ? ((s.internal_links || 0) + " / " + (s.external_links || 0)) : "—"],
      ["Technos", (Array.isArray(p.tech) && p.tech.length) ? p.tech.join(", ") : "—"],
      ["Donnees structurees valides", schemaInfoValid(p)],
      ["Erreurs donnees structurees", schemaInfoErrors(p)],
      ["HTTPS", secInfoHttps(p)],
      ["Contenu mixte", secInfoMixed(p)],
      ["En-tetes manquants", secInfoHeaders(p)],
      ["rel=prev", (s && s.pagination_prev) ? s.pagination_prev : "—"],
      ["rel=next", (s && s.pagination_next) ? s.pagination_next : "—"],
      ["Erreur", p.error || "—"]
    ];
    var h = '<div class="kv">';
    for (var i = 0; i < rows.length; i++) {
      h += '<div class="kv-row"><div class="kv-key">' + esc(rows[i][0]) +
           '</div><div class="kv-val">' + esc(rows[i][1]) + '</div></div>';
    }
    // Lignes Vague 3 a contenu HTML (liens vers pages similaires / hreflang) :
    // injectees hors de la boucle car nearDupHtml/hreflangRecipHtml renvoient du
    // HTML deja echappe en interne (urlCellHtml echappe l'URL).
    // VAGUE 4 (A) : temps de reponse colore (pastille vert/orange/rouge).
    h += '<div class="kv-row"><div class="kv-key">Temps de reponse</div>' +
         '<div class="kv-val">' + speedCellHtml(respMs(p)) + '</div></div>';
    h += '<div class="kv-row"><div class="kv-key">Near-duplicates</div>' +
         '<div class="kv-val">' + nearDupHtml(p) + '</div></div>';
    h += '<div class="kv-row"><div class="kv-key">Hreflang reciproque</div>' +
         '<div class="kv-val">' + hreflangRecipHtml(p) + '</div></div>';
    h += "</div>";
    return h;
  }

  // Vue "Source" : on n'a pas le HTML brut cote frontend, on affiche donc une
  // synthese structuree (title, meta, H1, H2, og, twitter) en texte mono.
  function renderSource(p) {
    var s = seoOf(p);
    var lines = [];
    lines.push("URL        : " + (p.url || ""));
    lines.push("Status     : " + (p.status || 0) + "   Content-Type: " + (p.content_type || "—"));
    lines.push("");
    if (s) {
      lines.push("<title>     " + (s.title || ""));
      lines.push("<meta desc> " + (s.meta_description || ""));
      lines.push("");
      (s.h1 || []).forEach(function (h) { lines.push("<h1> " + h); });
      (s.h2 || []).forEach(function (h) { lines.push("<h2> " + h); });
      lines.push("");
      var og = s.og || {};
      Object.keys(og).forEach(function (k) { lines.push("og:" + k + " = " + og[k]); });
      var tw = s.twitter || {};
      Object.keys(tw).forEach(function (k) { lines.push("twitter:" + k + " = " + tw[k]); });
    } else {
      lines.push("(pas de donnees SEO : page en erreur ou non-HTML)");
    }
    return '<textarea class="source-box" readonly>' + esc(lines.join("\n")) + '</textarea>';
  }

  // =========================================================================
  // BARRE DE STATUT (event crawl:progress)
  // =========================================================================
  function updateStatus(prog) {
    var done = prog && prog.done != null ? prog.done : S.pages.length;
    var queued = prog && prog.queued != null ? prog.queued : 0;
    var bytes = prog && prog.total_bytes_so_far != null ? prog.total_bytes_so_far : S.bw.totalBytes;
    var unlimited = !!(S.multi.config && S.multi.config.unlimited);
    var totalLabel = unlimited ? "∞" : String(done + queued);

    setText("status-done", done);
    setText("status-total", totalLabel);
    setText("status-queued", queued);
    setText("status-bytes", fmtBytes(bytes));

    // Debit moyen en pages/seconde depuis le debut du run.
    if (S.rateStartTs) {
      var elapsed = (Date.now() - S.rateStartTs) / 1000;
      var rate = elapsed > 0 ? (done / elapsed) : 0;
      setText("status-rate", rate.toFixed(1));
    }

    // Temps de reponse moyen par page (ms) sur les pages mesurees (Vague 4+).
    var rts = S.pages.filter(function (p) { return p && p.response_time_ms; });
    var avgMs = rts.length ? Math.round(rts.reduce(function (a, p) { return a + p.response_time_ms; }, 0) / rts.length) : 0;
    setText("status-avgms", avgMs);

    // Barre de progression (indeterminee si unlimited : on borne a 90%).
    var fill = $("status-fill");
    if (fill) {
      var total = done + queued;
      var pct = unlimited ? Math.min(90, done % 100) : (total > 0 ? Math.round((done / total) * 100) : 0);
      fill.style.width = pct + "%";
    }
  }

  // VAGUE 4 (C) — Met a jour libelles + etats (disabled) des 3 boutons selon
  // l'etat courant. Trois etats :
  //   - running : crawl en cours        -> Demarrer OFF, Stop ON ("Stop"), Vider ON
  //   - paused  : crawl en pause (Stop) -> Demarrer ON ("Reprendre"), Stop OFF, Vider ON
  //   - idle    : rien en cours         -> Demarrer ON ("Demarrer"), Stop OFF, Vider ON
  // C'est CE point qui corrige le bug du bouton "Demarrer" reste grise apres Stop.
  function updateButtons() {
    var start = $("btn-start");
    var stop  = $("btn-stop");
    var clear = $("btn-clear");
    if (start) {
      start.disabled = S.running;                 // actif sauf pendant un crawl actif
      start.textContent = S.paused ? "Reprendre" : "Demarrer";
      start.title = S.paused
        ? "Reprendre le crawl en pause (sans effacer les donnees deja collectees)"
        : "Lancer un nouveau crawl";
    }
    if (stop) {
      stop.disabled = !S.running;                  // actif seulement pendant un crawl actif
      stop.title = "Mettre en pause (les donnees collectees sont conservees)";
    }
    if (clear) {
      clear.disabled = false;                      // toujours disponible
      clear.title = "Effacer toutes les donnees et remettre a zero";
    }
  }

  function setStatusState(txt) { setText("status-state", txt); }

  function updateMultiStatus() {
    var el = $("status-multi");
    if (!el) return;
    if (S.multi.target > 1) {
      el.hidden = false;
      el.textContent = "Run " + Math.min(S.multi.done + (S.running ? 1 : 0), S.multi.target) +
                       "/" + S.multi.target + " — cumul " + fmtBytes(S.multi.totalBytes) +
                       " (" + S.multi.totalPages + " pages)";
    } else {
      el.hidden = true;
    }
  }

  // =========================================================================
  // ABONNEMENTS AUX EVENEMENTS TAURI (flux de crawl PRESERVE)
  // =========================================================================
  function subscribeEvents() {
    if (!TAURI) return;

    TAURI.event.listen("crawl:page", function (event) {
      ingestPage(event.payload || {});
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:progress", function (event) {
      var p = event.payload || {};
      updateStatus(p);
      // VAGUE 4 (C) : si on vient de mettre en PAUSE, un dernier progress peut
      // arriver (page courante terminee) -> on garde le libelle "En pause".
      if (p.current_url && !S.paused) setStatusState("Crawl : " + truncUrl(p.current_url, 70));
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:error", function (event) {
      setStatusState("Erreur : " + (event.payload || "inconnue"));
      console.error("crawl:error", event.payload);
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:sitemap", function (event) {
      // Sitemap recu AU DEBUT du crawl (robots.txt + /sitemap.xml) : on remplit
      // l'onglet Sitemap tout de suite, sans attendre la fin du crawl.
      var smap = Array.isArray(event.payload) ? event.payload : [];
      if (!smap.length) return;
      S.sitemapUrls = smap;
      S.sitemapSet = {};
      for (var si = 0; si < smap.length; si++) { if (smap[si]) S.sitemapSet[normUrl(smap[si])] = true; }
      scheduleRender();
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:done", function (event) {
      // Vague 3 : le payload de crawl:done est le CrawlSummary complet. On y
      // recupere sitemap_urls pour alimenter l'onglet Sitemap. Les autres
      // champs (pages/total_bytes...) sont deja recus en flux via crawl:page,
      // on ne les reexploite donc pas ici (pas de double comptage).
      var summary = (event && event.payload) ? event.payload : {};
      var smap = Array.isArray(summary.sitemap_urls) ? summary.sitemap_urls : [];
      if (smap.length) {   // ne pas ecraser si vide : l'event crawl:sitemap (debut) fait foi
        S.sitemapUrls = smap;
        S.sitemapSet = {};
        for (var si = 0; si < smap.length; si++) {
          if (smap[si]) S.sitemapSet[normUrl(smap[si])] = true;
        }
      }

      // VAGUE 4 (C) : si l'utilisateur a mis EN PAUSE, ce crawl:done provient du
      // crawl backend qui s'est termine de lui-meme apres le stop_crawl. On
      // recupere le sitemap (ci-dessus) mais on NE cloture PAS la sequence et on
      // NE quitte PAS l'etat pause : l'affichage reste fige, "Reprendre" actif.
      if (S.paused) { scheduleRender(); return; }

      // Fin du run courant : on cumule dans l'agregat multi-run.
      // rebuildAggregates() tourne normalement dans le rAF (asynchrone) ; on le
      // rejoue ici de facon SYNCHRONE pour garantir que S.bw.totalBytes reflete
      // bien TOUTES les pages de ce run avant le cumul (mesure fiable).
      rebuildAggregates();
      S.multi.done       += 1;
      S.multi.totalBytes += S.bw.totalBytes;
      S.multi.totalPages += S.pages.length;
      S.multi.totalDynamic += S.pages.reduce(function (s, p) { return s + (p.dynamic_requests || 0); }, 0);
      S.multi.totalMiss  += S.pages.reduce(function (s, p) { return s + (p.vercel_cache_misses || 0); }, 0);
      var techSet = {};
      S.pages.forEach(function (p) { (p.tech || []).forEach(function (t) { if (t) techSet[t] = 1; }); });
      S.multi.lastTechs = Object.keys(techSet);
      S.multi.lastRows  = S.pages.slice();
      updateMultiStatus();
      scheduleRender();   // rafraichit compteur + grille de l'onglet Sitemap

      if (!S.multi.stopRequested && S.multi.done < S.multi.target) {
        startRun();                       // run suivant (multi-run bande passante)
      } else {
        finishSequence();
      }
    }).then(function (u) { S.unlisteners.push(u); });
  }

  function unsubscribeEvents() {
    S.unlisteners.forEach(function (fn) { try { fn(); } catch (e) { /* ignore */ } });
    S.unlisteners = [];
  }

  // =========================================================================
  // MULTI-RUN : enchainement + cloture (logique de mesure PRESERVEE)
  // =========================================================================
  function startRun() {
    // Nouveau run : on repart d'un jeu de donnees vide (mais on garde la config).
    resetData();
    S.running = true;
    S.paused = false;
    updateButtons();
    S.rateStartTs = Date.now();
    setStatusState("Demarrage...");
    updateMultiStatus();
    scheduleRender();

    TAURI.core.invoke("start_crawl", { config: S.multi.config })
      .catch(function (err) {
        setStatusState("Erreur : " + (err || "inconnue"));
        console.error("start_crawl error:", err);
        finishSequence();
      });
  }

  // VAGUE 4 (C) — REPRISE d'un crawl mis en pause. Le backend gere une VRAIE
  // reprise : start_crawl, appele avec la MEME seed alors qu'un etat reprenable
  // existe (file non vide), RELANCE les workers sur la file restante — il ne
  // re-crawle donc PAS les pages deja vues et n'emet que les NOUVELLES pages.
  // Cote UI on conserve les donnees deja collectees et on poursuit le cumul.
  // (L'ingestion idempotente reste un filet de securite si une page revenait.)
  function resumeRun() {
    S.running = true;
    S.paused = false;
    S.multi.stopRequested = false;
    S.multi.running = true;
    updateButtons();
    S.rateStartTs = Date.now();
    setStatusState("Reprise du crawl...");
    updateMultiStatus();

    // On se re-abonne aux evenements (ils avaient ete laisses actifs, mais on
    // garantit l'etat) puis on relance le crawl backend avec la meme config.
    TAURI.core.invoke("start_crawl", { config: S.multi.config })
      .catch(function (err) {
        setStatusState("Erreur : " + (err || "inconnue"));
        console.error("start_crawl (resume) error:", err);
        finishSequence();
      });
  }

  function finishSequence() {
    S.multi.running = false;
    S.running = false;
    S.paused = false;                 // VAGUE 4 (C) : fin reelle -> retour a l'etat idle
    updateButtons();
    var fill = $("status-fill");
    if (fill) fill.style.width = "100%";
    setStatusState("Termine — " + S.multi.done + " run(s), " + S.multi.totalPages + " page(s).");
    updateMultiStatus();

    // Une entree d'historique par sequence.
    addHistoryEntry({
      date: new Date().toISOString(),
      url: S.multi.config.seed_url || "",
      runs: S.multi.done,
      pages: S.multi.totalPages,
      bytes: S.multi.totalBytes,
      dynamic: S.multi.totalDynamic,
      miss: S.multi.totalMiss,
      techs: S.multi.lastTechs || [],
      config: {
        max_pages: S.multi.config.max_pages,
        max_depth: S.multi.config.max_depth,
        rps: S.multi.config.requests_per_second,
        unlimited: S.multi.config.unlimited,
        respect_robots: S.multi.config.respect_robots,
        force_chrome: S.multi.config.force_chrome,
        random_ua: S.multi.config.random_user_agent
      },
      pagesData: (S.multi.lastRows || []).slice(0, 5000)   // pour la consultation
    });
  }

  // Vide UNIQUEMENT les donnees du run courant (pages, index, issues, bw).
  function resetData() {
    S.pages = [];
    S.byUrl = {};
    S.pageIndex = {};
    S.issues = [];
    S.sevCounts = { critical: 0, warning: 0, opportunity: 0, info: 0 };
    S.inlinksIndex = {};
    S.dupMaps = { title: {}, meta: {}, h1: {} };
    S.nearDup = {};
    S.nearDupSkipped = false;
    S.hreflangRecip = {};
    S.hreflangIssues = [];
    S.sitemapUrls = [];
    S.sitemapSet = {};
    S.bw = { totalBytes: 0, maxWeight: 0 };
    closeDetail();
    renderTabCounts();
    renderOverview();
    renderGrid();
  }

  // =========================================================================
  // SOUMISSION DU FORMULAIRE (lance le crawl)
  // =========================================================================
  function readConfig() {
    var unlimited = $("opt-unlimited").checked;
    var seedUrl  = $("seed-url").value.trim();
    var maxPages = parseInt($("max-pages").value, 10) || 500;
    var maxDepth = parseInt($("max-depth").value, 10) || 10;
    var rps      = Math.max(1, Math.round(parseFloat($("rps").value) || 5));
    // VAGUE 4 (B) : nombre de pages traitees en parallele (threads du crawler).
    // Borne 1..50, defaut 5 (coherent avec l'attribut min/max du champ HTML).
    var threads  = Math.min(50, Math.max(1, parseInt($("threads").value, 10) || 5));
    return {
      seed_url: seedUrl,
      max_pages: maxPages,
      max_depth: maxDepth,
      same_domain_only: true,
      requests_per_second: rps,
      threads: threads,                           // <-- VAGUE 4 (B) : envoye au backend
      force_chrome: $("force-chrome").checked,
      random_user_agent: $("random-ua").checked,
      random_delay: $("random-rps").checked,
      unlimited: unlimited,                       // <-- envoye au backend
      respect_robots: $("opt-robots").checked     // <-- envoye au backend
    };
  }

  function onSubmit(evt) {
    evt.preventDefault();
    if (!TAURI) { alert("Interface Tauri non disponible. Lancez l'application via Tauri."); return; }

    // VAGUE 4 (C) — Aligne sur le backend (start_crawl = demarrer OU reprendre).
    // Si un crawl est EN PAUSE et que l'URL saisie correspond a la seed en pause
    // (ou est vide), le bouton "Reprendre" REPREND la file existante cote backend
    // SANS vider les donnees ni reinitialiser la sequence. Si l'URL a change, on
    // considere que l'utilisateur veut un NOUVEAU crawl.
    var typedUrl = $("seed-url").value.trim();
    if (S.paused) {
      var pausedSeed = (S.multi.config && S.multi.config.seed_url) || "";
      if (!typedUrl || typedUrl === pausedSeed) { resumeRun(); return; }
      // URL differente : on abandonne l'etat en pause et on repart proprement.
    }

    var repetitions = 1; // repetition retiree : crawl unique (bande passante mesuree a chaque passage)
    var config = readConfig();

    // (Re)initialise l'agregat multi-run.
    S.multi = {
      target: repetitions, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: true,
      config: config, lastTechs: [], lastRows: []
    };

    // Nouvelle sequence : jeu de donnees vierge + reset des filtres.
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();

    unsubscribeEvents();
    subscribeEvents();

    // Depart PROPRE : on vide d'abord l'etat backend (clear_crawl) pour ne pas
    // "reprendre" par erreur une file reprenable de la meme seed, PUIS on lance.
    // clear_crawl est async : on chaine startRun() pour eviter une course.
    Promise.resolve(TAURI.core.invoke("clear_crawl").catch(function () {}))
      .then(function () { startRun(); });  // running=true + paused=false + updateButtons()
  }

  // VAGUE 4 (C) — Bouton Stop = MISE EN PAUSE. Le crawl s'arrete cote affichage
  // (on coupe l'emission backend via stop_crawl) mais les DONNEES collectees
  // sont CONSERVEES. On stoppe aussi l'enchainement multi-run. Surtout : le
  // bouton "Demarrer" (devenu "Reprendre") redevient ACTIF immediatement
  // (correction du bug ou il restait grise apres un Stop).
  function onStop() {
    if (!TAURI) return;
    if (!S.running) return;                 // rien a mettre en pause
    S.multi.stopRequested = true;           // stoppe l'enchainement des runs
    TAURI.core.invoke("stop_crawl").catch(function (err) { console.error("stop_crawl error:", err); });
    S.running = false;
    S.paused = true;                        // -> "Reprendre" disponible
    updateButtons();
    setStatusState("En pause — " + S.pages.length + " page(s) collectee(s).");
  }

  // Bouton "Vider" : VIDE l'etat de crawl backend + remet l'affichage a zero.
  // VAGUE 4 (C) : on invoque clear_crawl (et non stop_crawl) pour que le backend
  // OUBLIE la file/visited de l'etat partage. Sans ca, un Stop met seulement en
  // PAUSE (etat conserve) et un Demarrer ulterieur sur la meme URL REPRENDRAIT
  // l'ancienne file au lieu de repartir de zero. clear_crawl est idempotent
  // (sans effet si aucun etat n'existe).
  function onClear() {
    S.multi.stopRequested = true;
    if (TAURI) TAURI.core.invoke("clear_crawl").catch(function () {});
    unsubscribeEvents();
    S.multi = {
      target: 0, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: false,
      config: {}, lastTechs: [], lastRows: []
    };
    S.running = false;
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();
    resetData();
    var fill = $("status-fill"); if (fill) fill.style.width = "0%";
    setStatusState("En attente");
    setText("status-done", 0); setText("status-total", 0);
    setText("status-queued", 0); setText("status-rate", 0);
    setText("status-bytes", "0 o");
    updateMultiStatus();
    S.paused = false;                  // VAGUE 4 (C) : retour a l'etat idle
    updateButtons();
  }

  // =========================================================================
  // FILTRES (recherche, categorie sidebar, severite) + bascule d'onglet
  // =========================================================================
  function setActiveTab(key) {
    S.activeTab = key;
    S.sort = { key: null, dir: null };     // nouveau tri par onglet
    document.querySelectorAll(".tab").forEach(function (t) {
      t.classList.toggle("is-active", t.getAttribute("data-tab") === key);
    });
    // VAGUE 4 (E) : on reporte l'onglet courant sur l'en-tete de grille, ce qui
    // declenche le lisere colore (CSS #grid-head[data-tab="..."]).
    var gh = $("grid-head");
    if (gh) gh.setAttribute("data-tab", key);
    // Filtres de severite : visibles seulement sur l'onglet Problemes.
    var sevBox = $("sev-filters");
    if (sevBox) sevBox.hidden = (key !== "issues");
    // Selecteur de vue Sitemap : visible seulement sur l'onglet Sitemap (Vague 3).
    var smBox = $("sitemap-views");
    if (smBox) smBox.hidden = (key !== "sitemap");
    renderGrid();
  }

  function applyCategoryFilter(catId) {
    var found = findCat(catId);
    if (!found) return;
    S.catId = catId;
    S.catFilter = { predicate: found.row.pred, label: found.row.label };
    // Indicateur de filtre actif.
    var af = $("active-filter");
    if (af) { af.hidden = false; setText("active-filter-label", found.row.label); }
    setActiveTab(found.tab);
    renderOverview();
  }

  function clearCategoryFilter() {
    S.catId = null;
    S.catFilter = null;
    var af = $("active-filter");
    if (af) af.hidden = true;
    renderOverview();
    renderGrid();
  }

  // Bascule sur l'onglet Sitemap avec la vue voulue (depuis la sidebar Overview).
  //   "all"/"orphan" -> vue "all" (orphelines surlignees) ; "notin" -> vue "notin".
  function selectSitemapView(view) {
    clearCategoryFilter();                 // les filtres categorie ne s'appliquent pas ici
    S.sitemapView = (view === "notin") ? "notin" : "all";
    var sel = $("sitemap-view"); if (sel) sel.value = S.sitemapView;  // sync du selecteur
    setActiveTab("sitemap");
    renderOverview();
  }

  // =========================================================================
  // EXPORT CSV (onglet actif, avec filtres appliques)
  // =========================================================================
  function csvCell(value) {
    var v = value === null || value === undefined ? "" : String(value);
    if (/[",;\r\n]/.test(v)) return '"' + v.replace(/"/g, '""') + '"';
    return v;
  }

  // Helper generique d'export CSV (BOM UTF-8 pour Excel + RFC4180 via csvCell).
  // header : tableau de libelles ; rows : tableau de tableaux de cellules brutes.
  // filenameBase : suffixe horodate ajoute automatiquement. Reutilise par
  // l'export par onglet ET par les exports en masse / rapports (Vague 3).
  function downloadCsv(filenameBase, header, rows) {
    var lines = [];
    lines.push(header.map(function (c) { return csvCell(c); }).join(","));
    for (var i = 0; i < rows.length; i++) {
      lines.push(rows[i].map(function (c) { return csvCell(c); }).join(","));
    }
    var csv = "\ufeff" + lines.join("\r\n");   // BOM UTF-8
    var blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    var url = URL.createObjectURL(blob);
    var a = document.createElement("a");
    var stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
    a.href = url; a.download = filenameBase + "-" + stamp + ".csv";
    document.body.appendChild(a); a.click(); document.body.removeChild(a);
    setTimeout(function () { URL.revokeObjectURL(url); }, 1000);
  }

  function exportCsv() {
    var def = TABS[S.activeTab];
    if (!def) return;
    var items = currentItems();
    // On applique le meme tri que l'affichage.
    var mapped = items.map(function (it) { return def.row(it); });
    if (S.sort.key) {
      var idx = -1;
      for (var ci = 0; ci < def.columns.length; ci++) if (def.columns[ci].key === S.sort.key) { idx = ci; break; }
      if (idx >= 0) {
        var dir = S.sort.dir === "asc" ? 1 : -1;
        mapped.sort(function (a, b) {
          var va = a[idx].sort, vb = b[idx].sort, cmp;
          if (typeof va === "number" && typeof vb === "number") cmp = va - vb;
          else cmp = String(va).localeCompare(String(vb), "fr", { numeric: true, sensitivity: "base" });
          return cmp * dir;
        });
      }
    }
    // Reutilise le helper commun downloadCsv (BOM + RFC4180).
    var header = def.columns.map(function (c) { return c.label; });
    var rows = mapped.map(function (cells) {
      return cells.map(function (c) { return c.text; });
    });
    downloadCsv("crawl-" + S.activeTab, header, rows);
  }

  // =========================================================================
  // VAGUE 3 — EXPORTS EN MASSE (toutes pages confondues)
  // Independants de l'onglet/filtre actif : balaient l'integralite de S.pages.
  // =========================================================================

  // Tous les liens sortants de toutes les pages : source,target,anchor,rel,interne.
  function exportAllLinks() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var outs = Array.isArray(p.outlinks) ? p.outlinks : [];
      for (var k = 0; k < outs.length; k++) {
        var l = outs[k]; if (!l) continue;
        rows.push([ p.url || "", l.target || "", l.anchor || "",
                    l.rel || "", l.internal ? "Interne" : "Externe" ]);
      }
    }
    downloadCsv("liens-tous", ["source", "target", "anchor", "rel", "interne"], rows);
  }

  // Toutes les images de toutes les pages : page,src,kind,octets.
  // NB : cote backend chaque image est un AssetReport { url, bytes, kind } ; on
  // expose 'url' sous l'en-tete 'src' (la source de l'image).
  function exportAllImages() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var imgs = Array.isArray(p.images) ? p.images : [];
      for (var k = 0; k < imgs.length; k++) {
        var im = imgs[k]; if (!im) continue;
        rows.push([ p.url || "", im.url || "", im.kind || "", String(im.bytes || 0) ]);
      }
    }
    downloadCsv("images-toutes", ["page", "src", "kind", "octets"], rows);
  }

  // Tous les problemes (issues backend + hreflang JS) : severite,categorie,adresse,message.
  function exportAllIssues() {
    var rows = S.issues.concat(S.hreflangIssues).map(function (it) {
      return [ sevLabel(it.severity), it.category || "", it.url || "", it.message || "" ];
    });
    downloadCsv("problemes-tous", ["severite", "categorie", "adresse", "message"], rows);
  }

  // =========================================================================
  // VAGUE 3 — RAPPORTS DE SYNTHESE (CSV cibles)
  // =========================================================================

  // Chaines de redirection : pages avec redirect_chain non vide.
  // Colonnes : url, chaine (jointe par " -> "), destination finale.
  function reportRedirectChains() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var chain = Array.isArray(p.redirect_chain) ? p.redirect_chain : [];
      if (!chain.length) continue;
      // Destination finale = dernier maillon de la chaine, ou redirect_to en repli.
      var dest = chain.length ? chain[chain.length - 1] : (p.redirect_to || "");
      rows.push([ p.url || "", chain.join(" -> "), dest ]);
    }
    downloadCsv("rapport-redirections", ["url", "chaine", "destination"], rows);
  }

  // Canonicals : url, canonical, type (Self / Autre / Manquant).
  function reportCanonicals() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var s = seoOf(p);
      var canon = s && s.canonical ? s.canonical : "";
      var st = canonState(p.url, canon);   // { kind, label } : Self / Autre / Manquant
      rows.push([ p.url || "", canon, st.label ]);
    }
    downloadCsv("rapport-canonicals", ["url", "canonical", "type"], rows);
  }

  // Pages non-indexables : url, raison (noindex / canonicalisee / bloquee).
  // On ne liste que les pages effectivement NON indexables. La raison est
  // deduite des signaux SEO : meta/x-robots noindex, canonical vers une autre
  // URL (canonicalised), ou statut HTTP non-2xx (bloquee/erreur).
  function reportNonIndexable() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var s = seoOf(p);
      var indexable = s ? !!s.indexable : false;
      if (indexable) continue;             // page indexable : exclue du rapport
      var reasons = [];
      if (s && s.noindex) reasons.push("noindex");
      if (s && s.canonicalised) reasons.push("canonicalisee");
      var code = p.status || 0;
      if (code < 200 || code >= 300) reasons.push("bloquee (HTTP " + code + ")");
      if (!s) reasons.push("non-HTML / erreur");
      if (!reasons.length) reasons.push("non indexable");
      rows.push([ p.url || "", reasons.join(" ; ") ]);
    }
    downloadCsv("rapport-non-indexables", ["url", "raison"], rows);
  }

  // Contenu mixte : url, ressources http (jointes par " | ").
  function reportMixedContent() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var mixed = mixedContentOf(p);
      if (!mixed.length) continue;
      rows.push([ p.url || "", mixed.join(" | ") ]);
    }
    downloadCsv("rapport-contenu-mixte", ["url", "ressources http"], rows);
  }

  // Aiguillage des actions des deux menus (data-export / data-report).
  function runBulkExport(action) {
    if (action === "links")  exportAllLinks();
    else if (action === "images") exportAllImages();
    else if (action === "issues") exportAllIssues();
  }
  function runReport(action) {
    if (action === "redirects") reportRedirectChains();
    else if (action === "canonicals") reportCanonicals();
    else if (action === "nonindex")   reportNonIndexable();
    else if (action === "mixed")      reportMixedContent();
  }

  // =========================================================================
  // INFO RESEAU (IP + reverse DNS) — PRESERVE
  // =========================================================================
  function loadNetInfo() {
    var el = $("net-info"), txt = $("net-info-text");
    if (!TAURI || !el || !txt) return;
    TAURI.core.invoke("network_info")
      .then(function (info) {
        var ip = info && info.ip ? info.ip : "?";
        var rev = info && info.reverse_dns ? info.reverse_dns : null;
        txt.textContent = rev ? (ip + " · " + rev) : ip;
        el.title = rev ? ("IP publique sortante " + ip + " — reverse DNS : " + rev)
                       : ("IP publique sortante " + ip + " — pas de reverse DNS");
        el.classList.remove("net-err"); el.classList.add("net-ok");
      })
      .catch(function (err) {
        txt.textContent = "IP indisponible";
        el.classList.remove("net-ok"); el.classList.add("net-err");
        console.error("network_info error:", err);
      });
  }

  // =========================================================================
  // HISTORIQUE PERSISTANT (load/save_history) — PRESERVE (sans UI dediee :
  // on conserve le stockage pour ne pas perdre la fonctionnalite backend).
  // =========================================================================
  function loadHistory() {
    if (!TAURI) return;
    TAURI.core.invoke("load_history")
      .then(function (json) {
        try { history = JSON.parse(json) || []; } catch (e) { history = []; }
        if (!Array.isArray(history)) history = [];
      })
      .catch(function (err) { console.error("load_history error:", err); });
  }
  function saveHistory() {
    if (!TAURI) return;
    TAURI.core.invoke("save_history", { json: JSON.stringify(history) })
      .catch(function (err) { console.error("save_history error:", err); });
  }
  function addHistoryEntry(entry) {
    history.unshift(entry);
    if (history.length > 100) history = history.slice(0, 100);
    saveHistory();
  }

  // =========================================================================
  // MENUS DEROULANTS (Export en masse / Rapports) — Vague 3
  // Petit composant maison : bouton .menu-btn qui bascule .is-open sur le
  // conteneur .menu. Un seul menu ouvert a la fois ; clic exterieur = fermeture.
  // =========================================================================
  function closeMenus() {
    document.querySelectorAll(".menu.is-open").forEach(function (m) { m.classList.remove("is-open"); });
  }
  function setupMenu(menuId) {
    var menu = $(menuId);
    if (!menu) return;
    var btn = menu.querySelector(".menu-btn");
    if (!btn) return;
    btn.addEventListener("click", function (e) {
      e.stopPropagation();
      var willOpen = !menu.classList.contains("is-open");
      closeMenus();                       // referme les autres menus
      if (willOpen) menu.classList.add("is-open");
    });
  }
  // Clic n'importe ou ailleurs : referme tous les menus ouverts.
  document.addEventListener("click", function (e) {
    if (!e.target.closest(".menu")) closeMenus();
  });

  // =========================================================================
  // BRANCHEMENT DES CONTROLES (delegation d'evenements)
  // =========================================================================
  function setupControls() {
    $("crawl-form").addEventListener("submit", onSubmit);
    $("btn-stop").addEventListener("click", onStop);
    $("btn-clear").addEventListener("click", onClear);
    $("btn-export-csv").addEventListener("click", exportCsv);

    // ----- Menus deroulants Vague 3 : Export en masse + Rapports -----
    // Ouverture/fermeture au clic sur le bouton ; fermeture au clic ailleurs.
    setupMenu("menu-bulk");
    setupMenu("menu-report");
    // Selection d'une entree dans le menu Export en masse.
    var bulkList = $("menu-bulk-list");
    if (bulkList) bulkList.addEventListener("click", function (e) {
      var it = e.target.closest("[data-export]");
      if (!it) return;
      runBulkExport(it.getAttribute("data-export"));
      closeMenus();
    });
    // Selection d'une entree dans le menu Rapports.
    var reportList = $("menu-report-list");
    if (reportList) reportList.addEventListener("click", function (e) {
      var it = e.target.closest("[data-report]");
      if (!it) return;
      runReport(it.getAttribute("data-report"));
      closeMenus();
    });

    // Selecteur de vue de l'onglet Sitemap (Toutes / Hors sitemap).
    var smSel = $("sitemap-view");
    if (smSel) smSel.addEventListener("change", function () {
      S.sitemapView = this.value === "notin" ? "notin" : "all";
      renderGrid();
      renderOverview();
    });

    // Case "Tout le site" : grise le champ "Pages max".
    var unlim = $("opt-unlimited");
    var maxPages = $("max-pages");
    function syncUnlimited() { maxPages.disabled = unlim.checked; }
    unlim.addEventListener("change", syncUnlimited);
    syncUnlimited();

    // Case "Debit aleatoire" : grise le champ "Req/sec".
    var randRps = $("random-rps");
    randRps.addEventListener("change", function () { $("rps").disabled = randRps.checked; });

    // Volet de reglages repliable.
    var toggle = $("btn-toggle-settings");
    var drawer = $("settings-drawer");
    toggle.addEventListener("click", function () {
      var open = drawer.hidden;
      drawer.hidden = !open;
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
      toggle.classList.toggle("is-active", open);
    });

    // Onglets (delegation sur la barre).
    $("tabsbar").addEventListener("click", function (e) {
      var t = e.target.closest(".tab");
      if (t) setActiveTab(t.getAttribute("data-tab"));
    });

    // Tri par clic sur les en-tetes de la grille (neutre->desc->asc->neutre).
    $("grid-head").addEventListener("click", function (e) {
      var th = e.target.closest("th[data-key]");
      if (!th) return;
      var key = th.getAttribute("data-key");
      if (S.sort.key !== key) { S.sort.key = key; S.sort.dir = "desc"; }
      else if (S.sort.dir === "desc") { S.sort.dir = "asc"; }
      else { S.sort.key = null; S.sort.dir = null; }
      renderGrid();
    });

    // Selection d'une ligne -> panneau de detail (delegation sur le tbody).
    $("grid-body").addEventListener("click", function (e) {
      var tr = e.target.closest("tr[data-url]");
      if (!tr) return;
      // L'onglet "Liens" ne represente pas une page unique : pas de detail.
      if (TABS[S.activeTab].kind !== "pages") return;
      var url = tr.getAttribute("data-url");
      // D) Sur l'onglet "Donnees structurees" (schema), ou si on clique sur un
      // badge de type, on ouvre directement le sous-onglet JSON-LD brut.
      if (S.activeTab === "schema" || e.target.closest(".badge--schema")) {
        S.detailTab = "schema";
      }
      if (url) openDetail(url);
    });

    // Sous-onglets du panneau de detail.
    $("detail-tabs").addEventListener("click", function (e) {
      var b = e.target.closest(".dtab");
      if (!b) return;
      S.detailTab = b.getAttribute("data-dtab");
      renderDetail();
    });
    $("detail-close").addEventListener("click", closeDetail);

    // Recherche : filtre l'onglet actif (rendu immediat).
    $("grid-search").addEventListener("input", function () {
      S.search = this.value.trim();
      renderGrid();
    });

    // Filtres de severite (onglet Problemes).
    $("sev-filters").addEventListener("click", function (e) {
      var b = e.target.closest(".sev-filter");
      if (!b) return;
      S.sevFilter = b.getAttribute("data-sev");
      document.querySelectorAll(".sev-filter").forEach(function (x) { x.classList.toggle("is-active", x === b); });
      renderGrid();
    });
    // Sidebar Overview : clic sur une categorie -> filtre la grille.
    $("overview-tree").addEventListener("click", function (e) {
      var row = e.target.closest(".ov-row");
      if (!row) return;
      // Lignes speciales du groupe Sitemap (data-sm) : basculent l'onglet/vue.
      var sm = row.getAttribute("data-sm");
      if (sm) { selectSitemapView(sm); return; }
      var catId = row.getAttribute("data-cat");
      if (S.catId === catId) clearCategoryFilter();    // re-clic = retire le filtre
      else applyCategoryFilter(catId);
    });

    // Retirer le filtre de categorie depuis l'indicateur.
    $("active-filter-clear").addEventListener("click", clearCategoryFilter);
  }

  // =========================================================================
  // INITIALISATION
  // =========================================================================
  setupControls();
  updateButtons();        // VAGUE 4 (C) : etat initial coherent des boutons
  loadNetInfo();
  loadHistory();
  renderTabCounts();
  renderOverview();
  renderGrid();
  setActiveTab("internal");

})();
