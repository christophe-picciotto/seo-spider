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
  // Etat global
  // =========================================================================
  var S = {
    pages: [],            // tous les PageReport bruts recus (verbatim serde)
    byUrl: {},            // index normUrl -> PageReport (pour le panneau detail)
    issues: [],           // toutes les issues agregees {url,severity,category,message}
    sevCounts: { critical: 0, warning: 0, opportunity: 0, info: 0 },

    activeTab: "internal",
    search: "",
    sevFilter: "all",
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

    // ----- Hreflang -----
    hreflang: {
      kind: "pages",
      columns: [
        { key: "url", label: "Adresse" },
        { key: "hl",  label: "Hreflang (lang -> href)" }
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
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: html, text: text, sort: hl.length }
        ];
      }
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
        var typesHtml = hasData
          ? types.map(function (t) { return '<span class="badge">' + esc(t) + '</span>'; }).join(" ")
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

    // ----- Bande passante : tableau de mesure (poids transfere reel) -----
    bandwidth: {
      kind: "pages",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "tech",   label: "Techno" },
        { key: "weight", label: "Poids total", num: true },
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
    S.pages.push(p);
    S.byUrl[normUrl(p.url)] = p;

    // Index inverse pour les Inlinks : pour chaque outlink interne, on note
    // "telle page pointe vers telle cible avec telle ancre".
    var outs = Array.isArray(p.outlinks) ? p.outlinks : [];
    for (var i = 0; i < outs.length; i++) {
      var l = outs[i];
      if (!l || !l.target) continue;
      var key = normUrl(l.target);
      if (!S.inlinksIndex[key]) S.inlinksIndex[key] = [];
      S.inlinksIndex[key].push({ from: p.url, anchor: l.anchor || "", rel: l.rel || "" });
    }

    // Issues agregees + compteurs de severite.
    var iss = Array.isArray(p.issues) ? p.issues : [];
    for (var j = 0; j < iss.length; j++) {
      var it = iss[j] || {};
      var sev = it.severity || "info";
      S.issues.push({ url: it.url || p.url || "", severity: sev,
                      category: it.category || "", message: it.message || "" });
      if (S.sevCounts[sev] === undefined) S.sevCounts[sev] = 0;
      S.sevCounts[sev] += 1;
    }

    // Mesure de bande passante (run courant + agregat multi-run).
    var w = p.total_bytes || 0;
    S.bw.totalBytes += w;
    if (w > S.bw.maxWeight) S.bw.maxWeight = w;

    scheduleRender();
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
      rebuildDupMaps();      // cartes de doublons fraiches pour grille + overview
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

  // Issues filtrees par recherche d'URL + filtre de severite.
  function issuesFiltered() {
    var q = S.search ? S.search.toLowerCase() : "";
    return S.issues.filter(function (it) {
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
    var hasRowClass = def.kind === "pages" && typeof def.rowClass === "function";
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
        empty.textContent = S.pages.length
          ? "Aucune ligne ne correspond au filtre."
          : "Lancez un crawl pour voir les resultats ici.";
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
      images: nPages,
      canonical: nPages,
      directives: nPages,
      hreflang: nPages,
      pagination: nPagination,
      schema: nPages,
      bandwidth: nPages,
      links: S.pages.reduce(function (s, p) { return s + (Array.isArray(p.outlinks) ? p.outlinks.length : 0); }, 0),
      issues: S.issues.length
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

  function countFor(pred) {
    var n = 0;
    for (var i = 0; i < S.pages.length; i++) if (pred(S.pages[i])) n++;
    return n;
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
    tree.innerHTML = html;

    // Compteurs des filtres de severite (onglet Problemes).
    setSevCount("all", S.issues.length);
    setSevCount("critical", S.sevCounts.critical || 0);
    setSevCount("warning", S.sevCounts.warning || 0);
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
    } else if (S.detailTab === "source") {
      body.innerHTML = renderSource(p);
    }
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
      ["Temps de reponse", (p.response_time_ms || 0) + " ms"],
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

    // Barre de progression (indeterminee si unlimited : on borne a 90%).
    var fill = $("status-fill");
    if (fill) {
      var total = done + queued;
      var pct = unlimited ? Math.min(90, done % 100) : (total > 0 ? Math.round((done / total) * 100) : 0);
      fill.style.width = pct + "%";
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
      if (p.current_url) setStatusState("Crawl : " + truncUrl(p.current_url, 70));
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:error", function (event) {
      setStatusState("Erreur : " + (event.payload || "inconnue"));
      console.error("crawl:error", event.payload);
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:done", function () {
      // Fin du run courant : on cumule dans l'agregat multi-run.
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

  function finishSequence() {
    S.multi.running = false;
    S.running = false;
    $("btn-start").disabled = false;
    $("btn-stop").disabled = true;
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
    S.issues = [];
    S.sevCounts = { critical: 0, warning: 0, opportunity: 0, info: 0 };
    S.inlinksIndex = {};
    S.dupMaps = { title: {}, meta: {}, h1: {} };
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
    return {
      seed_url: seedUrl,
      max_pages: maxPages,
      max_depth: maxDepth,
      same_domain_only: true,
      requests_per_second: rps,
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

    var repetitions = Math.max(1, Math.round(parseFloat($("repetitions").value) || 1));
    var config = readConfig();

    // (Re)initialise l'agregat multi-run.
    S.multi = {
      target: repetitions, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: true,
      config: config, lastTechs: [], lastRows: []
    };

    $("btn-start").disabled = true;
    $("btn-stop").disabled = false;

    // Nouvelle sequence : jeu de donnees vierge + reset des filtres.
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();

    unsubscribeEvents();
    subscribeEvents();
    startRun();
  }

  function onStop() {
    if (!TAURI) return;
    S.multi.stopRequested = true;
    TAURI.core.invoke("stop_crawl").catch(function (err) { console.error("stop_crawl error:", err); });
    $("btn-stop").disabled = true;
    setStatusState("Arret demande...");
  }

  // Bouton "Vider" : arrete un crawl en cours + remet l'affichage a zero.
  function onClear() {
    S.multi.stopRequested = true;
    if (TAURI && S.multi.running) TAURI.core.invoke("stop_crawl").catch(function () {});
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
    $("btn-start").disabled = false;
    $("btn-stop").disabled = true;
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
    // Filtres de severite : visibles seulement sur l'onglet Problemes.
    var sevBox = $("sev-filters");
    if (sevBox) sevBox.hidden = (key !== "issues");
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

  // =========================================================================
  // EXPORT CSV (onglet actif, avec filtres appliques)
  // =========================================================================
  function csvCell(value) {
    var v = value === null || value === undefined ? "" : String(value);
    if (/[",;\r\n]/.test(v)) return '"' + v.replace(/"/g, '""') + '"';
    return v;
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
    var lines = [];
    lines.push(def.columns.map(function (c) { return csvCell(c.label); }).join(","));
    mapped.forEach(function (cells) {
      lines.push(cells.map(function (c) { return csvCell(c.text); }).join(","));
    });
    var csv = "﻿" + lines.join("\r\n");   // BOM UTF-8 pour Excel
    var blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    var url = URL.createObjectURL(blob);
    var a = document.createElement("a");
    var stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
    a.href = url; a.download = "crawl-" + S.activeTab + "-" + stamp + ".csv";
    document.body.appendChild(a); a.click(); document.body.removeChild(a);
    setTimeout(function () { URL.revokeObjectURL(url); }, 1000);
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
  // BRANCHEMENT DES CONTROLES (delegation d'evenements)
  // =========================================================================
  function setupControls() {
    $("crawl-form").addEventListener("submit", onSubmit);
    $("btn-stop").addEventListener("click", onStop);
    $("btn-clear").addEventListener("click", onClear);
    $("btn-export-csv").addEventListener("click", exportCsv);

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
  loadNetInfo();
  loadHistory();
  renderTabCounts();
  renderOverview();
  renderGrid();
  setActiveTab("internal");

})();
