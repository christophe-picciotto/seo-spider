/**
 * SEO Spider — Frontend vanilla (Tauri v2, withGlobalTauri = true)
 * Accès Tauri : window.__TAURI__.core.invoke / window.__TAURI__.event.listen
 *
 * Interface dense facon Screaming Frog :
 *   - barre du haut : URL + cases "Tout le site" (unlimited) / "robots.txt" ;
 *   - barre d'onglets avec compteurs en direct ;
 *   - grille dense triable (en-tête fige, colonnes selon l'onglet) ;
 *   - sidebar "Overview" : catégories cliquables avec compteurs ;
 *   - panneau de detail (Inlinks/Outlinks/SERP/Infos/Source) au clic ligne ;
 *   - barre de statut (progression / file / débit / poids total).
 *
 * Flux de crawl PRESERVE : invoke start_crawl/stop_crawl + listeners
 * crawl:progress / crawl:page / crawl:done / crawl:error. La logique de
 * mesure de bande passante (multi-run, agrégats) est conservée à l'identique.
 */

(function () {
  "use strict";

  var TAURI = window.__TAURI__;

  // Plafond ABSOLU de pages applique par le backend, même en mode "Tout le
  // site". DOIT rester aligne sur `HARD_MAX_PAGES` (crawl-core/src/crawler.rs) :
  // cette valeur n'est qu'un LIBELLÉ, elle ne limite rien côté UI.
  var MAX_PAGES_HARD = 200000;
  var MAX_PAGES_HARD_LABEL = MAX_PAGES_HARD.toLocaleString("fr-FR");

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
  // Tronque une URL pour l'affichage (l'attribut title garde la version complète).
  function truncUrl(url, maxLen) {
    maxLen = maxLen || 90;
    if (!url) return "";
    if (url.length <= maxLen) return url;
    return url.slice(0, maxLen - 3) + "...";
  }
  // Hote d'une URL, pour les libellés courts (ex. "Site 12/40 · exemple.fr").
  function hostOf(url) {
    try { return new URL(url).host; } catch (e) { return url || ""; }
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

  // Explication (sens + impact SEO) de chaque code HTTP, pour l'infobulle au survol.
  var HTTP_EXPLAIN = {
    0:   "Pas de réponse : page inaccessible, bloquée par robots.txt, ou erreur réseau / délai dépassé.",
    200: "OK : la page répond normalement.",
    201: "Créé : la ressource a été créée avec succès.",
    204: "Aucun contenu : réponse valide mais sans corps.",
    206: "Contenu partiel : réponse fragmentée (requête Range).",
    301: "Redirection permanente : la page a définitivement déménagé. Le jus SEO est transmis à la cible — pensez à mettre à jour les liens.",
    302: "Redirection temporaire : déplacement provisoire. Le jus SEO n'est pas (ou peu) transmis. Si le changement est définitif, préférez un 301.",
    303: "Voir ailleurs : redirige vers une autre URL (typiquement après un POST).",
    304: "Non modifié : la version en cache est encore valide, rien n'est retéléchargé.",
    307: "Redirection temporaire stricte : comme 302, mais conserve la méthode HTTP.",
    308: "Redirection permanente stricte : comme 301 (transmet le jus SEO), mais conserve la méthode HTTP.",
    400: "Requête invalide : le serveur n'a pas compris la requête.",
    401: "Authentification requise : il faut être connecté pour accéder à la page.",
    403: "Accès interdit : le serveur refuse l'accès (droits, pare-feu, blocage de robot). Vérifiez que Googlebot n'est pas bloqué.",
    404: "Introuvable : la page n'existe pas (lien cassé). À corriger — rediriger en 301 ou retirer les liens entrants.",
    405: "Méthode non autorisée : la méthode HTTP (GET, POST…) n'est pas permise ici.",
    406: "Non acceptable : le serveur ne peut pas fournir le format demandé.",
    408: "Délai de requête dépassé : le serveur a trop attendu.",
    410: "Supprimé définitivement : page retirée volontairement (désindexation plus forte qu'un 404).",
    418: "Je suis une théière (clin d'oeil ; serveur facétieux).",
    429: "Trop de requêtes : le serveur limite le débit. Ralentissez le crawl (baissez Req/sec).",
    451: "Indisponible pour raisons légales.",
    500: "Erreur serveur interne : bug ou panne côté serveur. À corriger en priorité (bloque l'indexation).",
    501: "Non implémenté : le serveur ne gère pas cette fonctionnalité.",
    502: "Passerelle invalide : un serveur intermédiaire a reçu une réponse invalide.",
    503: "Service indisponible : serveur surchargé ou en maintenance. Temporaire, mais à surveiller.",
    504: "Délai de passerelle dépassé : un serveur intermédiaire n'a pas répondu à temps."
  };
  function httpExplain(status) {
    var s = parseInt(status, 10) || 0;
    if (HTTP_EXPLAIN[s]) return s + " · " + HTTP_EXPLAIN[s];
    if (s >= 200 && s < 300) return s + " · Succès (2xx) : la requête a abouti.";
    if (s >= 300 && s < 400) return s + " · Redirection (3xx) : la ressource est ailleurs.";
    if (s >= 400 && s < 500) return s + " · Erreur client (4xx) : page absente, accès refusé, etc.";
    if (s >= 500)            return s + " · Erreur serveur (5xx) : problème côté serveur.";
    return s + " · Code inhabituel.";
  }
  // Cellule "Code HTTP" : pastille colorée + infobulle explicative au survol.
  function httpCodeHtml(status) {
    return '<span class="http-code ' + httpClass(status) + '" title="' + esc(httpExplain(status)) + '">' + esc(status || 0) + '</span>';
  }
  function seoOf(p) { return p && p.seo ? p.seo : null; }

  // ----- VAGUE 4 (A) — Temps de réponse (response_time_ms) -----------------
  // Lit le temps de réponse d'une page en millisecondes (0 si absent).
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

  // ----- CACHE (cache.status) ----------------------------------------------
  // Statut de cache de la réponse, lu par le backend sur les en-têtes
  // (cf-cache-status, x-hivecache, x-cache, age...). Renseigne des la sonde
  // HTTP, donc présent sur toute page crawlée.

  // Objet cache d'une page, ou null.
  function cacheOf(p) { return p && p.cache ? p.cache : null; }

  // Verdict court en majuscules ("HIT", "MISS"...), "" si indetermine.
  function cacheStatus(p) {
    var c = cacheOf(p);
    return c && c.status ? String(c.status).toUpperCase() : "";
  }

  // Classe de couleur. Trois familles, pas une par valeur :
  //   servi par le cache (vert) / alle jusqu'à l'origine (rouge) /
  //   ni l'un ni l'autre — non eligible, contourne (gris).
  function cacheClass(st) {
    // EDGE : heberge sur le réseau (Cloudflare Pages/Workers). Aucune origine
    // n'existe, donc aucune n'a été sollicitée — même famille qu'un HIT, avec
    // sa teinte propre pour ne pas laisser croire à un cache classique.
    if (st === "EDGE") return "cache--edge";
    if (st === "HIT") return "cache--hit";
    if (st === "MISS" || st === "EXPIRED") return "cache--miss";
    if (st === "STALE" || st === "UPDATING" || st === "REVALIDATED" || st === "PRERENDER")
      return "cache--warm";
    if (!st) return "cache--na";
    return "cache--none";   // BYPASS, DYNAMIC, NONE, valeur inconnue
  }

  // Qui a RÉELLEMENT servi la page : "Cloudflare", "HiveCache", "WP Rocket"...
  // ou "origine" quand la requête est allée jusqu'au serveur.
  function cacheServedBy(p) {
    var c = cacheOf(p);
    return c && c.served_by ? String(c.served_by) : "";
  }

  // Ce qu'a fait UNE couche, en trois mots : « a fourni la page » ou
  // « est alle chercher plus loin ». C'est la seule chose qu'on veut savoir
  // en lisant la liste des couches.
  function layerRole(st) {
    if (st === "HIT" || st === "EDGE" || st === "STALE" || st === "UPDATING") return "a fourni la page";
    if (st === "MISS" || st === "EXPIRED") return "n'avait pas la page";
    if (st === "REVALIDATED") return "a vérifié auprès du serveur";
    if (st === "BYPASS") return "a été contourne";
    if (st === "DYNAMIC") return "ne met pas cette page en cache";
    if (!st) return "présent, sans indication";
    return "";
  }

  // Une couche formatee : « Cloudflare : MISS — n'avait pas la page ».
  function layerText(l) {
    if (!l) return "";
    var t = (l.engine || l.header || "?") + " : " + (l.raw || l.status || "?");
    var role = layerRole(l.status);
    if (role) t += "  — " + role;
    return t;
  }

  // Phrase expliquant un verdict, en francais et sans jargon. C'est elle qui
  // ouvre l'infobulle : le code technique ne dit rien a qui ne le connait pas.
  function cacheMeaning(st) {
    switch (st) {
      case "HIT":
        return "Page servie depuis le cache : le serveur n'a pas été sollicité.";
      case "MISS":
        return "Le cache n'avait pas la page : le serveur a dû la générer.";
      case "EDGE":
        return "Page hébergée sur le réseau Cloudflare (Pages / Workers).\n" +
               "Il n'y a pas de serveur d'origine a solliciter : ni HIT ni MISS possibles.";
      case "EXPIRED":
        return "Le cache avait la page mais périmée : il est allé la revalider auprès du serveur.";
      case "REVALIDATED":
        return "Le cache a vérifié sa copie auprès du serveur, qui l'a confirmée à jour.";
      case "STALE":
        return "Page servie périmée pendant que le cache la rafraîchit en arrière-plan.";
      case "UPDATING":
        return "Page servie périmée, sa mise à jour est en cours.";
      case "BYPASS":
        return "Le cache a été contourne : une règle, un cookie ou un en-tête l'a interdit.";
      case "DYNAMIC":
        return "Cloudflare ne met pas cette page en cache : aucune règle ne la couvre.\n" +
               "Chaque visite atteint le serveur.";
      case "PRERENDER":
        return "Page pré-générée servie telle quelle.";
      case "NONE":
        return "Aucune mise en cache pour cette réponse.";
      default:
        return "";
    }
  }

  // Traduit un `server-timing` Cloudflare en lignes lisibles.
  //
  //   cfEdge;dur=6,cfOrigin;dur=0,cfWorker;dur=18
  //
  // ne dit rien tel quel. Chaque clé designe une étape du trajet de la requête,
  // et `cfOrigin` est la plus parlante : a zero, le serveur n'a pas été touche.
  function serverTimingLines(raw) {
    var LABELS = {
      cfedge:   "au point de présence Cloudflare le plus proche",
      cforigin: "sur le serveur d'origine",
      cfworker: "dans un Worker (code exécute sur le réseau Cloudflare)",
      cfcachestatus: null
    };
    var out = [];
    var items = String(raw).split(",");
    for (var i = 0; i < items.length; i++) {
      var m = /^\s*([a-z]+)\s*;\s*dur\s*=\s*([0-9.]+)/i.exec(items[i]);
      if (!m) continue;
      var key = m[1].toLowerCase();
      var ms = parseFloat(m[2]);
      var label = LABELS[key];
      if (!label) continue;
      var line = "   " + ms + " ms  " + label;
      // La ligne qui repond à la question qu'on se pose vraiment.
      if (key === "cforigin") {
        line += ms > 0 ? "  <- il a travaille" : "  <- il n'a pas été sollicité";
      }
      out.push(line);
    }
    return out;
  }

  // Infobulle : le verdict explique en clair, puis qui a servi, puis le detail
  // technique. C'est le desaccord entre couches qui est parlant (CDN MISS +
  // cache d'origine HIT).
  function cacheTitle(p) {
    var c = cacheOf(p);
    if (!c) {
      return "Aucun en-tête de cache dans la réponse.\n" +
             "Impossible de conclure — et ce n'est PAS un MISS : " +
             "le serveur n'a simplement rien dit à ce sujet.";
    }
    var parts = [];

    // 1) Le verdict, en toutes lettres.
    var meaning = cacheMeaning(c.status);
    parts.push(c.status ? (c.status + (meaning ? " — " + meaning : "")) : "Statut indetermine.");

    // 2) Qui a fourni la page.
    if (c.served_by) {
      parts.push("");
      parts.push(c.served_by === "origine"
        ? "Servi par : le serveur d'origine (aucun cache n'a repondu)"
        : "Servi par : " + c.served_by);
    }
    if (c.age != null) parts.push("En cache depuis : " + fmtAge(c.age));

    // 3) Le detail technique, traduit quand c'est possible.
    var st = (c.source === "server-timing") ? c.raw : null;
    if (!st) {
      var ls = c.layers || [];
      for (var li = 0; li < ls.length; li++) {
        if (ls[li] && ls[li].header === "server-timing") { st = ls[li].raw; break; }
      }
    }
    if (st) {
      var lines = serverTimingLines(st);
      if (lines.length) {
        parts.push("");
        parts.push("Temps mesures par Cloudflare :");
        parts = parts.concat(lines);
      }
    }

    // 4) Les couches traversees, quand il y en a plusieurs.
    var layers = Array.isArray(c.layers) ? c.layers : [];
    if (layers.length > 1) {
      parts.push("");
      parts.push("Couches traversees, de l'extérieur vers le serveur :");
      for (var i = 0; i < layers.length; i++) {
        parts.push("   " + (i + 1) + ". " + layerText(layers[i]));
      }
    }

    // 5) La source, pour qui veut verifier.
    if (c.source) {
      parts.push("");
      parts.push("(lu dans l'en-tête " + c.source + ")");
    }
    return parts.join("\n");
  }

  // Age lisible : secondes, minutes, heures ou jours.
  function fmtAge(s) {
    s = s || 0;
    if (s < 60) return s + " s";
    if (s < 3600) return Math.round(s / 60) + " min";
    if (s < 86400) return (s / 3600).toFixed(1).replace(".", ",") + " h";
    return (s / 86400).toFixed(1).replace(".", ",") + " j";
  }

  // Ligne "Cache" du panneau de detail : verdict, en-tête qui a tranche, age,
  // puis TOUTES les couches quand il y en a plusieurs.
  function cacheInfoText(p) {
    var c = cacheOf(p);
    if (!c) return "— aucun en-tête de cache (ce n'est pas un MISS : le serveur n'a rien dit)";
    var out = (c.status || "?");
    var meaning = cacheMeaning(c.status);
    // L'explication d'abord : le code seul ne parle qu'aux inities.
    if (meaning) out += " — " + meaning.replace(/\n/g, " ");
    if (c.served_by) {
      out += "  |  Servi par " + (c.served_by === "origine" ? "le serveur d'origine" : c.served_by);
    }
    if (c.age != null) out += "  |  en cache depuis " + fmtAge(c.age);
    var layers = Array.isArray(c.layers) ? c.layers : [];
    if (layers.length > 1) {
      var txt = [];
      for (var i = 0; i < layers.length; i++) txt.push(layerText(layers[i]));
      out += "  —  couches : " + txt.join("  >  ");
    }
    return out;
  }

  // Cellule HTML "cache" (pastille coloree). "—" quand aucun en-tête ne permet
  // de conclure : l'absence d'information n'est pas un MISS.
  function cacheCellHtml(p) {
    var st = cacheStatus(p);
    var label = st || "—";
    return '<span class="cache-pill ' + cacheClass(st) + '" title="' +
           esc(cacheTitle(p)) + '">' + esc(label) + '</span>';
  }

  // Cellule "Servi par" : le moteur qui a fourni la page. "origine" est mis en
  // evidence — c'est le cas ou le serveur a réellement travaille, donc celui
  // qui coute et qu'on cherche a reduire.
  function cacheByCellHtml(p) {
    var by = cacheServedBy(p);
    if (!by) return '<span class="muted">—</span>';
    var cls = (by === "origine") ? "cache-by cache-by--origin" : "cache-by";
    return '<span class="' + cls + '" title="' + esc(cacheTitle(p)) + '">' + esc(by) + '</span>';
  }

  // Cellule URL : lien tronque, titre complet au survol.
  //
  // Le `href` est CONSERVE bien qu'il ne soit jamais suivi : il donne au lien
  // son apparence et son curseur, alimente le menu contextuel ("Copier
  // l'adresse du lien") et sert de source à l'ouverture. C'est le gestionnaire
  // delegue (cf. initExternalLinks) qui intercepte le clic et confie l'URL au
  // navigateur par defaut, la WebView n'ayant nulle part ou ouvrir un onglet.
  function urlCellHtml(url) {
    var u = esc(url || "");
    return '<a href="' + u + '" rel="noopener noreferrer" title="' +
           u + '&#10;Double-clic : ouvrir dans le navigateur" class="cell-url">' +
           esc(truncUrl(url, 90)) + '</a>';
  }
  function pill(kind, label) {
    return '<span class="state-pill state-pill--' + kind + '">' + esc(label) + '</span>';
  }

  // Normalisation légère d'URL (comparer canonical vs URL de page, dedup inlinks).
  function normUrl(u) {
    if (!u) return "";
    return String(u).trim().replace(/#.*$/, "").replace(/\/+$/, "").toLowerCase();
  }

  // ----- Extraction des valeurs servant au calcul des DOUBLONS (Vague 2) ----
  // Comparaison EXACTE (après trim). Une clé vide ne compte jamais comme doublon.
  function dupKeyTitle(p) { var s = seoOf(p); return s && s.title ? s.title.trim() : ""; }
  function dupKeyMeta(p)  { var s = seoOf(p); return s && s.meta_description ? s.meta_description.trim() : ""; }
  function dupKeyH1(p)    {
    var s = seoOf(p);
    var arr = s && Array.isArray(s.h1) ? s.h1 : [];
    return arr.length && arr[0] ? String(arr[0]).trim() : "";
  }

  // Accès aux nouveaux objets Vague 2 (peuvent être null côté backend).
  function secOf(p)   { return p && p.security ? p.security : null; }
  function flagsOf(p) { return p && p.url_flags ? p.url_flags : null; }

  // Nombre d'en-têtes manquants (tableau security.missing_headers).
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
  // Chaque page expose seo.simhash : une chaîne HEXADÉCIMALE de 16 caractères
  // representant une empreinte de 64 bits du contenu visible. Deux pages au
  // contenu très proche ont des simhash dont peu de bits different.
  //
  // PRINCIPE de la DISTANCE DE HAMMING :
  //   distance = nombre de bits qui different entre les deux empreintes
  //            = popcount( simhashA XOR simhashB )   (sur 64 bits)
  // On la calcule via BigInt : BigInt("0x"+hex) donne l'entier 64 bits exact
  // (les nombres JS classiques perdent en précision au-dela de 53 bits).
  //   - XOR bit a bit : (a ^ b) met a 1 chaque bit qui differe ;
  //   - popcount : on compte ces bits a 1 en consommant l'entier 16 bits par
  //     16 bits (masque 0xFFFF) avec une table de comptage precalculee, ce qui
  //     évite une boucle sur 64 bits à chaque comparaison.
  // Seuil : distance <= NEAR_DUP_BITS bits => pages considerees quasi-dupliquées.
  // =========================================================================
  var NEAR_DUP_BITS = 6;          // <= 6 bits de difference = quasi-dupliqué
  var NEAR_DUP_MAX_PAGES = 2000;  // au-dela, on saute le calcul O(n^2) (cf. note UI)

  // ----- Cadence des rendus PENDANT un crawl -------------------------------
  // Deux plafonds distincts, parce que les deux couts n'ont pas le même ordre
  // de grandeur. Ils ne s'appliquent QUE tant que le crawl tourne : hors
  // crawl, tri, filtre et recherche restent instantanés.
  var RENDER_MIN_INTERVAL_MS = 350;    // rendu complet : ~3/s au lieu de ~60/s
  var NEAR_DUP_MIN_INTERVAL_MS = 3000; // calcul O(n^2) : au plus une fois / 3 s

  // Table de popcount sur 16 bits (65536 entrées) : POPCNT16[x] = nb de bits a 1.
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
    // On exige une chaîne hexa non nulle (un simhash "0000...0" signale un
    // contenu vide côté backend : on l'ignore pour ne pas tout regrouper).
    if (!h || !/^[0-9a-f]{1,16}$/.test(h)) return "";
    if (/^0+$/.test(h)) return "";
    return h;
  }

  // Convertit un simhash hex en BigInt 64 bits (prefixe 0x).
  function simhashBig(hex) { return BigInt("0x" + hex); }

  // Renvoie la liste (memorisee) des URLs quasi-dupliquées à la page p.
  function nearDupOf(p) {
    var arr = S.nearDup[normUrl(p.url)];
    return Array.isArray(arr) ? arr : [];
  }

  // =========================================================================
  // État global
  // =========================================================================
  var S = {
    pages: [],            // tous les PageReport bruts reçus (verbatim serde)
    byUrl: {},            // index normUrl -> PageReport (pour le panneau detail)
    pageIndex: {},        // index normUrl -> position dans S.pages (ingestion idempotente)
    issues: [],           // toutes les issues agregees {url,severity,category,message}
    sevCounts: { critical: 0, warning: 0, opportunity: 0, info: 0 },

    activeTab: "internal",
    search: "",
    sevFilter: "all",
    sitemapView: "all",                 // Vague 3 : "all" | "notin" (onglet Sitemap)
    sort: { key: null, dir: null },     // tri de la grille (par onglet courant)

    // Sélection de CELLULES par colonne (clic, Shift+clic, Ctrl+clic) -> copie.
    // Une seule colonne à la fois : c'est ce qu'on copie. Clés = normUrl de la
    // ligne, pour survivre aux re-rendus de la grille pendant un crawl.
    sel: { col: -1, keys: {}, anchor: null },
    gridRows: [],                       // [{key, texts:[...par colonne]}] dans l'ORDRE AFFICHE

    // Filtre pousse par la sidebar Overview : { predicate(p)->bool, label } ou null
    catFilter: null,
    catId: null,                        // identifiant de la catégorie active

    selectedUrl: null,                  // URL de la ligne sélectionnée (detail)
    detailTab: "inlinks",

    // Inlinks : index inverse normUrl(cible) -> [{from, anchor, rel}]
    inlinksIndex: {},

    // Cartes de doublons (Vague 2) : valeur normalisée -> nombre de pages.
    // Reconstruites une fois par cycle de rendu (évite O(n^2) par ligne).
    dupMaps: { title: {}, meta: {}, h1: {} },

    // ----- Vague 3 -----
    // Near-duplicates : normUrl(page) -> [urls quasi-dupliquées] (distance de
    // Hamming <= NEAR_DUP_BITS sur le simhash 64 bits). Reconstruit une fois par
    // cycle de rendu, en O(n^2) BORNE (cf. NEAR_DUP_MAX_PAGES).
    nearDup: {},
    nearDupSkipped: false,         // vrai si trop de pages -> calcul désactivé

    // Hreflang reciproque : normUrl(page) -> { declared:[langs], missingBack:[urls] }
    // missingBack = pages cibles crawlées qui ne declarent PAS de retour vers nous.
    hreflangRecip: {},
    // Issues hreflang dérivées côté JS (catégorie "Hreflang"), reconstruites a
    // chaque cycle et FUSIONNEES à la lecture (jamais poussees dans S.issues
    // pour ne pas doublonner ni polluer l'agregat backend).
    hreflangIssues: [],

    // Sitemap (reçu via crawl:done) : liste brute + Set normalisé pour les jointures.
    sitemapUrls: [],               // URLs telles que déclarées dans le sitemap
    sitemapSet: {},                // normUrl(sitemapUrl) -> true (appartenance O(1))

    // Mesure de bande passante (run courant + agrégats multi-run)
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
    lastRenderAt: 0,                    // horodatage du dernier rendu complet
    lastNearDupAt: 0,                   // horodatage du dernier calcul O(n^2)
    running: false,
    paused: false,                      // VAGUE 4 (C) : crawl mis en pause (Stop)
    rateStartTs: 0,                     // pour estimer le débit pages/seconde

    unlisteners: []
  };

  // Historique persistant (liste de séquences).
  var history = [];

  // =========================================================================
  // Definition des ONGLETS
  // Chaque onglet : columns (liste d'objets {key,label,num?,w?}) + une fonction
  // row(p) qui renvoie, pour chaque colonne, { html, text, sort }. "html" pour
  // l'affichage, "text" pour l'export CSV, "sort" pour le tri (valeur brute).
  // Les onglets sur les pages utilisent kind:"pages". L'onglet "links" liste
  // tous les outlinks (kind:"links"), "issues" liste les problèmes (kind:"issues").
  // =========================================================================
  var TABS = {

    // ----- Internal : vue principale facon SF -----
    internal: {
      kind: "pages",
      columns: [
        { key: "url",       label: "Adresse" },
        { key: "status",    label: "Code", num: true },
        { key: "cache",     label: "Cache" },
        { key: "cacheby",   label: "Servi par" },
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
          { html: httpCodeHtml(p.status),
            text: String(p.status || 0), sort: p.status || 0 },
          // Statut de cache. Le tri regroupe par verdict ; les pages sans
          // information ("") tombent ensemble, en tête ou en queue.
          { html: cacheCellHtml(p), text: cacheStatus(p), sort: cacheStatus(p) },
          // Qui a fourni la page : trier ici isole d'un coup toutes les pages
          // servies par "origine", celles qui font travailler le serveur.
          { html: cacheByCellHtml(p), text: cacheServedBy(p), sort: cacheServedBy(p) },
          { html: esc(shortType(p.content_type)), text: p.content_type || "", sort: p.content_type || "" },
          // VAGUE 4 (A) : temps de réponse colore (vert/orange/rouge), triable.
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
          { html: httpCodeHtml(p.status),
            text: String(p.status || 0), sort: p.status || 0 },
          { html: esc(shortType(p.content_type)), text: p.content_type || "", sort: p.content_type || "" },
          { html: redir ? urlCellHtml(redir) : '<span class="muted">—</span>', text: redir, sort: redir }
        ];
      }
    },

    // ----- URL (Vague 2) : analyse de la forme des adresses -----
    // Colonnes : Adresse, Longueur, non-ASCII, Underscores, Majuscules, Paramètres.
    // Une ligne est en alerte si l'URL depasse 115 caractères OU contient du non-ASCII.
    url: {
      kind: "pages",
      columns: [
        { key: "url",        label: "Adresse" },
        { key: "length",     label: "Longueur", num: true },
        { key: "nonascii",   label: "non-ASCII" },
        { key: "underscores", label: "Underscores" },
        { key: "uppercase",  label: "Majuscules" },
        { key: "slashes",    label: "Slashes //" },
        { key: "params",     label: "Paramètres" }
      ],
      row: function (p) {
        var f = flagsOf(p);
        // Repli sur l'URL elle-même si le backend n'a pas (encore) rempli url_flags.
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
      // Surligne la ligne entière si alerte (URL trop longue, non-ASCII ou double slash).
      rowClass: function (p) {
        var f = flagsOf(p);
        var len = f && f.length != null ? f.length : (p.url ? p.url.length : 0);
        var nonAscii = f ? !!f.non_ascii : false;
        var multipleSlashes = f ? !!f.multiple_slashes : false;
        return (len > 115 || nonAscii || multipleSlashes) ? "row--alert" : "";
      }
    },

    // ----- Sécurité (Vague 2) : HTTPS, contenu mixte, en-têtes manquants -----
    // Une ligne est en alerte si la page présente du contenu mixte.
    security: {
      kind: "pages",
      columns: [
        { key: "url",     label: "Adresse" },
        { key: "https",   label: "HTTPS" },
        { key: "mixed",   label: "Contenu mixte", num: true },
        { key: "headers", label: "En-têtes manquants" }
      ],
      row: function (p) {
        var sec = secOf(p);
        var https = sec ? !!sec.https : (p.url ? /^https:/i.test(p.url) : false);
        var mixed = mixedContentOf(p);
        var headers = missingHeadersOf(p);
        // Liste courte des en-têtes manquants (max 3 affiches, reste en "+N").
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
      // Surligne la ligne entière si contenu mixte présent.
      rowClass: function (p) { return mixedContentOf(p).length ? "row--alert" : ""; }
    },

    // ----- Page Titles -----
    titles: {
      kind: "pages",
      columns: [
        { key: "url",   label: "Adresse" },
        { key: "title", label: "Title" },
        { key: "len",   label: "Longueur", num: true },
        { key: "dup",   label: "Dupliqué" },
        { key: "state", label: "État" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var title = s && s.title ? s.title : "";
        var len = s ? (s.title_len || 0) : 0;
        var st = titleState(title, len);
        var dup = isDup("title", dupKeyTitle, p);   // doublon exact calcule côté JS
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
        { key: "dup",  label: "Dupliqué" },
        { key: "state", label: "État" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var meta = s && s.meta_description ? s.meta_description : "";
        var len = s ? (s.meta_description_len || 0) : 0;
        var st = metaState(meta, len);
        var dup = isDup("meta", dupKeyMeta, p);     // doublon exact calcule côté JS
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
        { key: "dup",   label: "Dupliqué" },
        { key: "state", label: "État" }
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
        var sims = nearDupOf(p);                 // URLs quasi-dupliquées (memorise)
        var n = sims.length;
        var ndHtml = n
          ? '<span class="state-pill state-pill--warn">Oui (' + n + ')</span>'
          : '<span class="muted">—</span>';
        return [
          { html: urlCellHtml(p.url), text: p.url || "", sort: p.url || "" },
          { html: '<span class="col-num">' + words + '</span>', text: String(words), sort: words },
          // tri par nombre de similaires (desc = les plus dupliquées en tête) ;
          // le texte CSV liste les URLs similaires pour exploitation hors-ligne.
          { html: ndHtml, text: n ? (n + " : " + sims.join(" | ")) : "0", sort: n }
        ];
      },
      // Surligne la ligne si la page a au moins une quasi-dupliquée.
      rowClass: function (p) { return nearDupOf(p).length ? "row--alert" : ""; }
    },

    // ----- Images (par page) -----
    images: {
      kind: "pages",
      columns: [
        { key: "url",     label: "Adresse" },
        { key: "total",   label: "Images", num: true },
        { key: "missing", label: "Sans alt", num: true },
        { key: "state",   label: "État" }
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
        { key: "canonsd",   label: "Canonicalisée" }
      ],
      row: function (p) {
        var s = seoOf(p);
        var robots = s && s.meta_robots ? s.meta_robots : "";
        var xrobots = s && s.x_robots_tag ? s.x_robots_tag : "";    // en-tête HTTP (Vague 2)
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
    // Reciproque = OK si toutes les cibles hreflang crawlées pointent en retour
    // vers cette page ; Manquant si au moins une cible crawlée n'a pas de lien
    // retour ; "—" si la page ne declare aucun hreflang. Calcul côté JS dans
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
        // État de reciprocite.
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
      // Filtre spécifique à l'onglet : conserve seulement les pages paginees.
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
    // Une ligne est en alerte si la page a des données structurees invalides.
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
        // "Valide" n'a de sens que si la page porte des données structurees.
        var validHtml, validText, validSort;
        if (!hasData) { validHtml = '<span class="bool-no">—</span>'; validText = ""; validSort = -1; }
        else if (valid) { validHtml = pill("ok", "oui"); validText = "oui"; validSort = 1; }
        else { validHtml = pill("err", "non"); validText = "non"; validSort = 0; }
        // Erreurs : nombre + première erreur en infobulle.
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
      // Surligne la ligne si la page a des données structurees invalides.
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

    // ----- Problèmes : issues agregees -----
    issues: {
      kind: "issues",
      columns: [
        { key: "severity", label: "Sévérité" },
        { key: "category", label: "Catégorie" },
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
    // Source dédiée (kind "sitemap") : voir sitemapItems(). Trois vues commutables
    // via le sous-filtre S.sitemapView :
    //   "all"    : toutes les URLs du sitemap + statut Crawlée / Orpheline ;
    //   "orphan" : SEULES les URLs du sitemap jamais atteintes par le crawl ;
    //   "notin"  : pages crawlées ABSENTES du sitemap (couverture inverse).
    sitemap: {
      kind: "sitemap",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "status", label: "État" },
        { key: "code",   label: "Code HTTP", num: true }
      ],
      row: function (it) {
        // it : { url, inSitemap, crawled, status }
        var stHtml, stText, stSort;
        if (it.kind === "orphan") {
          // Dans le sitemap mais jamais atteinte par le crawl.
          stHtml = '<span class="state-pill state-pill--warn" title="Déclarée dans le sitemap mais non atteinte par le crawl">Orpheline</span>';
          stText = "Orpheline (sitemap, non crawlée)"; stSort = 1;
        } else if (it.kind === "notin") {
          // Crawlée mais absente du sitemap.
          stHtml = '<span class="state-pill state-pill--muted" title="Page atteinte par le crawl mais absente du sitemap">Hors sitemap</span>';
          stText = "Hors sitemap"; stSort = 2;
        } else {
          // Dans le sitemap ET crawlée.
          stHtml = pill("ok", "Crawlée"); stText = "Crawlée"; stSort = 0;
        }
        var codeHtml = it.crawled
          ? httpCodeHtml(it.status)
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

    // ----- Bande passante : tableau de mesure (poids transfere réel) -----
    bandwidth: {
      kind: "pages",
      columns: [
        { key: "url",    label: "Adresse" },
        { key: "tech",   label: "Techno" },
        { key: "weight", label: "Poids total", num: true },
        { key: "speed",  label: "Vitesse", num: true },
        { key: "imgs",   label: "Images", num: true },
        { key: "req",    label: "Requêtes", num: true },
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
          // VAGUE 4 (A) : temps de réponse colore, triable.
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

  // ----- Logique d'état (manquant / court / long / ok) ---------------------
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
    // IDEMPOTENT (Vague 4 / point C) : si l'URL est déjà connue (cas d'une
    // REPRISE qui re-crawle), on REMPLACE l'entrée en place au lieu d'empiler
    // un doublon. En crawl normal chaque URL est unique : comportement inchange.
    var existing = S.byUrl[key];
    if (existing && S.pageIndex[key] != null) {
      S.pages[S.pageIndex[key]] = p;
    } else {
      S.pageIndex[key] = S.pages.length;
      S.pages.push(p);
    }
    S.byUrl[key] = p;

    // Les agrégats derives (inlinks, issues, severites, bande passante) sont
    // desormais RECALCULES par frame dans rebuildAggregates() — ainsi une page
    // re-ingeree ne double pas les compteurs.
    scheduleRender();
  }

  // (Re)construit en O(total) les agrégats derives de S.pages :
  //   - inlinksIndex : index inverse cible -> [{from, anchor, rel}] ;
  //   - issues + sevCounts : problèmes backend agreges par sévérité ;
  //   - bw : poids total cumule + poids max d'une page (barre bande passante).
  // Recalcule à chaque cycle de rendu => robuste à la re-ingestion (reprise).
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
  // On marque l'état "dirty" et on declenche un seul rendu par frame.
  // =========================================================================
  function scheduleRender() {
    S.dirty = true;
    if (S.rafPending) return;

    // Pendant un crawl, les pages arrivent en continu : sans plafond, on
    // reconstruisait TOUT à chaque frame (agrégats sur toutes les pages et
    // tous leurs liens, near-duplicates en O(n^2), tri, grille entière). Le
    // thread principal n'avait plus une milliseconde pour traiter un clic —
    // d'ou l'impression que l'interface "coince" quand on trie en cours de
    // crawl. On plafonne donc la cadence tant que le crawl tourne ; hors
    // crawl (tri, filtre, recherche), le rendu reste immédiat.
    var wait = 0;
    if (S.running) {
      var since = Date.now() - S.lastRenderAt;
      if (since < RENDER_MIN_INTERVAL_MS) wait = RENDER_MIN_INTERVAL_MS - since;
    }

    S.rafPending = true;
    var run = function () {
      requestAnimationFrame(function () {
        S.rafPending = false;
        if (!S.dirty) return;
        S.dirty = false;
        S.lastRenderAt = Date.now();
        rebuildAggregates();   // Vague 4 : inlinks/issues/bw recalcules (idempotent)
        rebuildDupMaps();      // cartes de doublons fraiches pour grille + overview
        maybeRebuildNearDup(); // Vague 3 : near-duplicates (Hamming sur simhash)
        rebuildHreflangRecip();// Vague 3 : reciprocite hreflang (lien retour)
        renderTabCounts();
        renderOverview();
        renderGrid();
        // Si une ligne est sélectionnée, on rafraîchit le panneau de detail :
        // les Inlinks peuvent grossir au fil du crawl (nouvelles pages liantes).
        if (S.selectedUrl) renderDetail();
      });
    };
    if (wait > 0) setTimeout(run, wait);
    else run();
  }

  // Near-duplicates : calcul O(n^2) espace pendant le crawl.
  //
  // A 2000 pages, `rebuildNearDup` compare 2 millions de paires en BigInt
  // (XOR + popcount) : le refaire à chaque cycle de rendu bloquait le thread
  // principal a lui seul. Pendant le crawl on l'espace donc fortement — la
  // carte affichée a au pire quelques secondes de retard, ce qui est sans
  // consequence pour un indicateur qui evolue de page en page. Hors crawl
  // (donc à la fin, et à chaque interaction), le calcul reste systématique et
  // la carte est exacte.
  function maybeRebuildNearDup() {
    if (S.running) {
      var since = Date.now() - S.lastNearDupAt;
      if (since < NEAR_DUP_MIN_INTERVAL_MS) return;
    }
    S.lastNearDupAt = Date.now();
    rebuildNearDup();
  }

  // Rendu complet SANS plafond de cadence.
  //
  // A appeler quand l'affichage doit être exact et non plus seulement frais :
  // en fin de séquence, la carte des near-duplicates volontairement espacee
  // pendant le crawl doit être recalculee sur la totalite des pages.
  function renderNow() {
    S.lastRenderAt = 0;     // plus d'attente
    S.lastNearDupAt = 0;    // force le recalcul O(n^2)
    scheduleRender();
  }

  // =========================================================================
  // SOURCES DE LIGNES selon l'onglet (avec filtres recherche/catégorie/sev)
  // =========================================================================

  // Pages filtrees par recherche d'URL + filtre de catégorie (sidebar) +
  // filtre propre à l'onglet courant (ex: Pagination ne montre que les pages
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
  //   "all"    : chaque URL déclarée dans le sitemap, marquee Crawlée/Orpheline ;
  //   "orphan" : seules les URLs du sitemap qu'aucune page crawlée ne couvre ;
  //   "notin"  : pages crawlées absentes du sitemap.
  // Filtre de recherche d'URL applique dans les trois cas.
  function sitemapItems() {
    var q = S.search ? S.search.toLowerCase() : "";
    var out = [];
    if (S.sitemapView === "notin") {
      // Couverture inverse : pages crawlées qui ne figurent pas dans le sitemap.
      for (var i = 0; i < S.pages.length; i++) {
        var p = S.pages[i];
        var key = normUrl(p.url);
        if (S.sitemapSet[key]) continue;                 // présente -> on ignore
        if (q && (p.url || "").toLowerCase().indexOf(q) === -1) continue;
        out.push({ url: p.url, inSitemap: false, crawled: true,
                   status: p.status || 0, kind: "notin" });
      }
      return out;
    }
    // Vues "all" et "orphan" : on parcourt les URLs du sitemap.
    //
    // En vue "orphan", seules celles qu'aucune page crawlée ne couvre sont
    // gardees. Avant, cliquer « Orphelines » basculait sur "all" et se contentait
    // de les SURLIGNER au milieu des autres : impossible de les isoler, de les
    // compter dans la grille ou de les exporter seules.
    var onlyOrphans = (S.sitemapView === "orphan");
    for (var s = 0; s < S.sitemapUrls.length; s++) {
      var u = S.sitemapUrls[s];
      if (!u) continue;
      if (q && u.toLowerCase().indexOf(q) === -1) continue;
      var pg = S.byUrl[normUrl(u)];                       // page crawlée correspondante
      if (onlyOrphans && pg) continue;                    // crawlée -> pas orpheline
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

  // Issues filtrees par recherche d'URL + filtre de sévérité.
  // Fusionne les issues backend (S.issues) avec les issues hreflang dérivées
  // côté JS (S.hreflangIssues, catégorie "Hreflang") — cf. rebuildHreflangRecip.
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
    // calcul des near-duplicates a été saute (trop de pages), on le signale —
    // sinon l'absence de near-dup serait trompeuse.
    var banner = $("grid-banner");
    if (banner) {
      if (S.activeTab === "content" && S.nearDupSkipped) {
        banner.hidden = false;
        banner.textContent = "Detection des near-duplicates désactivée : plus de " +
          NEAR_DUP_MAX_PAGES + " pages (le calcul O(n^2) serait trop couteux).";
      } else {
        banner.hidden = true;
      }
    }

    // --- En-tête (colonnes triables au clic) ---
    //
    // L'en-tête n'est RÉÉCRIT que si son contenu logique change : onglet, tri,
    // ou largeurs enregistrées. Le reconstruire à chaque rendu — donc à chaque
    // page reçue pendant un crawl — detruisait les <th> en continu, avec deux
    // consequences visibles :
    //   - un clic de tri etait AVALE quand le remplacement tombait entre le
    //     mousedown et le mouseup (le navigateur n'emet `click` que si les deux
    //     visent le même element) ;
    //   - un redimensionnement en cours perdait sa référence `rz.th`, qui
    //     pointait alors sur un noeud detache.
    var widthsSig = "";
    var savedTab = PREFS.colWidths && PREFS.colWidths[S.activeTab];
    if (savedTab) {
      for (var wk in savedTab) {
        if (Object.prototype.hasOwnProperty.call(savedTab, wk)) widthsSig += wk + ":" + savedTab[wk] + ";";
      }
    }
    var headSig = S.activeTab + "|" + S.sort.key + "|" + S.sort.dir + "|" + widthsSig;
    if (head.__sig !== headSig) {
      var hh = "<tr>";
      for (var c = 0; c < def.columns.length; c++) {
        var col = def.columns[c];
        var cls = "";
        if (S.sort.key === col.key) cls = S.sort.dir === "asc" ? "sort-asc" : "sort-desc";
        var savedW = savedTab && savedTab[col.key];
        var wStyle = savedW ? ' style="width:' + savedW + 'px"' : '';
        hh += '<th data-key="' + esc(col.key) + '" class="' + cls + '"' + wStyle + '>' + esc(col.label) + '<span class="col-resizer"></span></th>';
      }
      hh += "</tr>";
      head.innerHTML = hh;
      head.__sig = headSig;
    }

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
    // Ordre d'affichage memorise : c'est lui qui donne son sens a « de cette
    // ligne a celle-la » (Shift+clic) et l'ordre des adresses copiees.
    var gridRows = [];
    // rowClass : applique pour les onglets "pages" ET "sitemap" (surlignage alerte).
    var hasRowClass = (def.kind === "pages" || def.kind === "sitemap") && typeof def.rowClass === "function";
    for (var r = 0; r < mapped.length; r++) {
      var cells = mapped[r].cells;
      var item = mapped[r].item;
      var rowUrl = item.url || item.from || "";   // pages/issues -> url ; liens -> from
      // Classes cumulees : sélection (ligne ouverte dans le detail) + alerte
      // eventuelle propre à l'onglet (URL trop longue, contenu mixte...).
      var classes = [];
      var rowKey = normUrl(rowUrl);
      gridRows.push({ key: rowKey, texts: cells.map(function (c) { return c.text; }) });
      var rowSel = (S.sel.col >= 0 && !!S.sel.keys[rowKey]);
      if (selKey && def.kind === "pages" && normUrl(item.url) === selKey) classes.push("is-selected");
      if (hasRowClass) { var rc = def.rowClass(item); if (rc) classes.push(rc); }
      var clsAttr = classes.length ? ' class="' + classes.join(" ") + '"' : "";
      rows += '<tr data-url="' + esc(rowUrl) + '"' + clsAttr + '>';
      for (var k = 0; k < cells.length; k++) {
        rows += (rowSel && k === S.sel.col) ? '<td class="is-multi">' : "<td>";
        rows += cells[k].html + "</td>";
      }
      rows += "</tr>";
    }
    body.innerHTML = rows;
    S.gridRows = gridRows;
    updateSelBar();

    // Message "grille vide".
    var empty = $("grid-empty");
    if (empty) {
      if (mapped.length) { empty.hidden = true; }
      else {
        empty.hidden = false;
        if (def.kind === "sitemap") {
          // Distingue "pas de sitemap du tout" de "vue/filtre sans résultat".
          if (S.sitemapView === "all" && !S.sitemapUrls.length) {
            empty.textContent = S.pages.length
              ? "Aucun sitemap trouvé pour ce site."
              : "Lancez un crawl pour analyser le sitemap.";
          } else if (S.sitemapView === "orphan" && S.sitemapUrls.length && !S.search) {
            empty.textContent = "Aucune orpheline : toutes les URLs du sitemap ont été atteintes par le crawl.";
          } else {
            empty.textContent = "Aucune URL ne correspond à cette vue.";
          }
        } else {
          empty.textContent = S.pages.length
            ? "Aucune ligne ne correspond au filtre."
            : "Lancez un crawl pour voir les résultats ici.";
        }
      }
    }
  }

  // =========================================================================
  // COMPTEURS DES ONGLETS (mis à jour en direct)
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
      // Sitemap : compteur = nombre d'URLs déclarées dans le sitemap.
      sitemap: S.sitemapUrls.length,
      links: S.pages.reduce(function (s, p) { return s + (Array.isArray(p.outlinks) ? p.outlinks.length : 0); }, 0),
      // Problèmes : issues backend + issues hreflang dérivées côté JS.
      issues: S.issues.length + S.hreflangIssues.length
    };
    document.querySelectorAll(".tab-count").forEach(function (el) {
      var key = el.getAttribute("data-count");
      el.textContent = counts[key] != null ? counts[key] : 0;
    });
  }

  // =========================================================================
  // SIDEBAR OVERVIEW : catégories cliquables avec compteurs en direct.
  // Chaque catégorie à un predicate(p) ; cliquer applique catFilter +
  // bascule sur l'onglet pertinent.
  // =========================================================================
  // Definition des catégories (groupes -> lignes).
  var OVERVIEW = [
    { group: "Codes de réponse", tab: "response", rows: [
      { id: "resp2xx", label: "Succès (2xx)", tone: "ok",   pred: function (p) { return p.status >= 200 && p.status < 300; } },
      { id: "resp3xx", label: "Redirections (3xx)", tone: "warn", pred: function (p) { return p.status >= 300 && p.status < 400; } },
      { id: "resp4xx", label: "Erreurs client (4xx)", tone: "err", pred: function (p) { return p.status >= 400 && p.status < 500; } },
      { id: "resp5xx", label: "Erreurs serveur (5xx)", tone: "err", pred: function (p) { return p.status >= 500; } }
    ]},
    { group: "Titres", tab: "titles", rows: [
      { id: "titleMissing", label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || !s.title; } },
      { id: "titleLong",    label: "> 60 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.title && s.title_len > 60; } },
      { id: "titleShort",   label: "< 30 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.title && s.title_len < 30; } },
      { id: "titleDup",     label: "Dupliqué", tone: "warn", pred: dupPred("title", dupKeyTitle) }
    ]},
    { group: "Meta Description", tab: "meta", rows: [
      { id: "metaMissing", label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || !s.meta_description; } },
      { id: "metaLong",    label: "> 160 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.meta_description && s.meta_description_len > 160; } },
      { id: "metaShort",   label: "< 70 car.", tone: "warn", pred: function (p) { var s = seoOf(p); return s && s.meta_description && s.meta_description_len < 70; } },
      { id: "metaDup",     label: "Dupliqué", tone: "warn", pred: dupPred("meta", dupKeyMeta) }
    ]},
    { group: "H1", tab: "h1", rows: [
      { id: "h1Missing",  label: "Manquant", tone: "err",  pred: function (p) { var s = seoOf(p); return !s || (s.h1_count || 0) === 0; } },
      { id: "h1Multiple", label: "Multiple", tone: "warn", pred: function (p) { var s = seoOf(p); return s && (s.h1_count || 0) > 1; } },
      { id: "h1Dup",      label: "Dupliqué", tone: "warn", pred: dupPred("h1", dupKeyH1) }
    ]},
    { group: "Contenu", tab: "content", rows: [
      { id: "nearDup", label: "Near-duplicates", tone: "warn", pred: function (p) { return nearDupOf(p).length > 0; } }
    ]},
    { group: "Images", tab: "images", rows: [
      { id: "imgMissingAlt", label: "Sans alt", tone: "warn", pred: function (p) { var s = seoOf(p); return s && (s.images_missing_alt || 0) > 0; } }
    ]},
    { group: "Canonicals", tab: "canonical", rows: [
      { id: "canonMissing", label: "Manquant", tone: "warn", pred: function (p) { var s = seoOf(p); return !s || !s.canonical; } },
      { id: "canonicalised", label: "Canonicalisées", tone: "warn", pred: function (p) { var s = seoOf(p); return !!(s && s.canonicalised); } }
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
    { group: "Sécurité", tab: "security", rows: [
      { id: "secMixed",   label: "Contenu mixte", tone: "err",  pred: function (p) { return mixedContentOf(p).length > 0; } },
      { id: "secHeaders", label: "En-têtes manquants", tone: "warn", pred: function (p) { return missingHeadersOf(p).length > 0; } }
    ]},
    { group: "Hreflang", tab: "hreflang", rows: [
      { id: "hreflangNoBack", label: "Sans lien retour", tone: "warn", pred: function (p) { return hasHreflangMissingBack(p); } }
    ]}
  ];

  // (Re)construit les cartes de doublons (title / meta / 1er h1) en UN passage
  // O(n) sur S.pages. Clé = valeur exacte (trim) -> nombre de pages partageant
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
  // Lit la carte pre-calculée S.dupMaps[mapKey] (pas de boucle interne).
  function isDup(mapKey, keyFn, p) {
    var val = keyFn(p);
    if (!val) return false;
    return (S.dupMaps[mapKey][val] || 0) >= 2;
  }

  // Fabrique un prédicat "dupliqué" pour la sidebar Overview, base sur les
  // cartes pre-calculées (>= 2 pages partageant la valeur exacte).
  function dupPred(mapKey, keyFn) {
    return function (p) { return isDup(mapKey, keyFn, p); };
  }

  // -------------------------------------------------------------------------
  // VAGUE 3 — (Re)construction de la carte des NEAR-DUPLICATES
  // O(n^2) BORNE : pour chaque paire de pages portant un simhash valide, on
  // calcule la distance de Hamming (popcount du XOR). Si <= NEAR_DUP_BITS, on
  // enregistre la reciprocite (A->B et B->A). On precalcule une seule fois les
  // BigInt par page pour ne pas reparser le hex à chaque comparaison.
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
  //   - si B est CRAWLÉE (présente dans S.byUrl) ET que B ne declare PAS de
  //     hreflang retour vers A  => "lien retour manquant" (problème SEO).
  //   - si B n'est pas crawlée, on ne peut pas conclure (ignore ici).
  // On indexe d'abord, par page, l'ensemble des cibles hreflang déclarées
  // (normalisées), puis on teste la reciprocite en O(liens).
  // Résultat : normUrl(A) -> { count, missingBack:[urls B sans retour] }.
  // -------------------------------------------------------------------------
  function rebuildHreflangRecip() {
    var recip = {};

    // 1) Index : normUrl(page) -> Set des cibles hreflang déclarées (normalisées).
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

    // 2) Test de reciprocite : pour A->B, B crawlée mais B ne pointe pas vers A.
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
        if (bKey === aKey) continue;            // auto-référence : ignorée
        if (seen[bKey]) continue; seen[bKey] = true;
        if (!S.byUrl[bKey]) continue;           // cible non crawlée : indecidable
        var bDecl = declares[bKey];
        if (!bDecl || !bDecl[aKey]) missing.push(href);  // pas de retour vers A
      }
      recip[aKey] = { count: hla.length, missingBack: missing };
    }
    S.hreflangRecip = recip;

    // Derive les issues "Hreflang sans lien retour" (sévérité warning) pour
    // l'onglet Problèmes. Une issue par page concernee, message listant les
    // cibles sans reciprocite. Recalculee à chaque cycle (pas d'accumulation).
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
  // Hors sitemap : pages crawlées absentes du sitemap (uniquement si on à un sitemap).
  function countNotInSitemap() {
    if (!S.sitemapUrls.length) return 0;
    var n = 0;
    for (var i = 0; i < S.pages.length; i++) {
      if (!S.sitemapSet[normUrl(S.pages[i].url)]) n++;
    }
    return n;
  }

  // Fabrique une ligne d'Overview pour le groupe Sitemap (data-sm = vue cible).
  // Chaque ligne à sa vue — "orphan" comprise, qui n'est plus un simple
  // raccourci vers "all". is-active suit S.activeTab + S.sitemapView.
  function smRow(view, label, n, tone) {
    var toneCls = n > 0 && tone ? (" ov-" + tone) : "";
    var active = (S.activeTab === "sitemap" && S.sitemapView === view) ? " is-active" : "";
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

    // Compteurs des filtres de sévérité (onglet Problèmes). Les issues hreflang
    // dérivées côté JS sont toutes de sévérité "warning" : on les ajoute au
    // total et au compteur "warning".
    var nHreflangIss = S.hreflangIssues.length;
    setSevCount("all", S.issues.length + nHreflangIss);
    setSevCount("critical", S.sevCounts.critical || 0);
    setSevCount("warning", (S.sevCounts.warning || 0) + nHreflangIss);
    setSevCount("opportunity", S.sevCounts.opportunity || 0);
  }

  // Met à jour un <span data-sevcount="X"> (compteurs des filtres de sévérité).
  function setSevCount(val, n) {
    var el = document.querySelector('[data-sevcount="' + val + '"]');
    if (el) el.textContent = n;
  }

  // Retrouve la definition d'une catégorie par son id.
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
  // PANNEAU DE DÉTAIL (au clic sur une ligne de page)
  // =========================================================================
  // =========================================================================
  // SÉLECTION DE CELLULES PAR COLONNE + COPIE
  //
  // Modèle « tableur » : un clic prend une cellule (et pose l'ancre),
  // Shift+clic etend la plage de l'ancre à la ligne cliquee DANS LA COLONNE
  // cliquee, Ctrl+clic ajoute ou retire une ligne, Ctrl+A prend toute la
  // colonne affichée, Échap deselectionne. Ctrl+C, le bouton Copier ou le
  // clic droit > Copier recopient les valeurs de la colonne, une par ligne.
  //
  // UNE SEULE COLONNE à la fois : c'est ce qu'on veut copier — toutes les
  // adresses, ou toutes les meta descriptions, jamais un mélange. Le lien
  // d'une adresse ne s'ouvre plus qu'au DOUBLE-clic : au simple clic, il
  // rendait cette colonne impossible a sélectionner.
  //
  // Les clés sont les URLs normalisées, pas des noeuds DOM : la grille est
  // reconstruite à chaque rendu pendant un crawl, la sélection est re-appliquée.
  // =========================================================================
  function selCount() { return Object.keys(S.sel.keys).length; }

  // Re-applique .is-multi sur les CELLULES sans reconstruire la grille (le
  // clic doit répondre à l'instant, pas au prochain rendu).
  function applySelClasses() {
    var col = S.sel.col;
    document.querySelectorAll("#grid-body tr[data-url]").forEach(function (tr) {
      var on = col >= 0 && !!S.sel.keys[normUrl(tr.getAttribute("data-url") || "")];
      for (var i = 0; i < tr.children.length; i++) {
        tr.children[i].classList.toggle("is-multi", on && i === col);
      }
    });
  }

  function selClear() {
    S.sel.keys = {}; S.sel.col = -1; S.sel.anchor = null;
    applySelClasses(); updateSelBar(); hideCtxMenu();
  }

  // Une seule cellule : point de depart de toute sélection.
  function selOne(col, key) {
    S.sel.col = col; S.sel.keys = {}; S.sel.keys[key] = true; S.sel.anchor = key;
    applySelClasses(); updateSelBar();
  }

  // Plage [ancre .. clé] dans l'ordre affiche, dans la colonne donnée. Remplace
  // la sélection courante, comme dans un tableur.
  function selRange(col, fromKey, toKey) {
    var rows = S.gridRows, a = -1, b = -1;
    for (var i = 0; i < rows.length; i++) {
      if (a < 0 && rows[i].key === fromKey) a = i;
      if (b < 0 && rows[i].key === toKey) b = i;
    }
    if (a < 0) a = b;
    if (b < 0) b = a;
    if (a < 0) return;
    var lo = Math.min(a, b), hi = Math.max(a, b);
    S.sel.col = col; S.sel.keys = {};
    for (var j = lo; j <= hi; j++) S.sel.keys[rows[j].key] = true;
  }

  // Toute la colonne affichée (tri + filtre compris).
  function selAllColumn(col) {
    if (col < 0 || !S.gridRows.length) return;
    S.sel.col = col; S.sel.keys = {};
    for (var i = 0; i < S.gridRows.length; i++) S.sel.keys[S.gridRows[i].key] = true;
    applySelClasses(); updateSelBar();
  }

  // Valeurs de la colonne sélectionnée, dans l'ordre affiche, dedoublonnees
  // (l'onglet Liens porte plusieurs lignes pour une même source).
  function selTexts() {
    var out = [], seen = {}, col = S.sel.col;
    if (col < 0) return out;
    for (var i = 0; i < S.gridRows.length; i++) {
      var r = S.gridRows[i];
      if (!S.sel.keys[r.key]) continue;
      var t = (r.texts[col] != null) ? String(r.texts[col]) : "";
      if (!seen[t]) { seen[t] = true; out.push(t); }
    }
    return out;
  }

  function selColumnLabel() {
    var def = TABS[S.activeTab];
    var c = def && def.columns[S.sel.col];
    return c ? c.label : "";
  }

  // Ecriture dans le presse-papiers : API moderne d'abord, repli sur l'ancien
  // execCommand si la WebView la refuse. `done(ok)` recoit le verdict.
  function copyText(text, done) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(
        function () { done(true); },
        function () { done(copyTextLegacy(text)); }
      );
    } else {
      done(copyTextLegacy(text));
    }
  }
  function copyTextLegacy(text) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed"; ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (err) { ok = false; }
    document.body.removeChild(ta);
    return ok;
  }

  function copySelection() {
    hideCtxMenu();
    var lines = selTexts();
    if (!lines.length) return;
    var label = selColumnLabel();
    copyText(lines.join("\n"), function (ok) {
      // Retour VISIBLE : le presse-papiers est invisible, sans message rien ne
      // dit que la copie a eu lieu ni ce qui a été copie.
      toast(ok ? (lines.length + " valeur(s) copiee(s) \u2014 colonne " + label) : "Échec de la copie");
      var b = $("multi-sel-copy");
      if (b) {
        b.textContent = ok ? "Copie !" : "Échec";
        b.classList.toggle("is-done", ok);
        setTimeout(function () { b.textContent = "Copier"; b.classList.remove("is-done"); }, 1600);
      }
    });
  }

  function updateSelBar() {
    var bar = $("multi-sel");
    if (!bar) return;
    var n = selCount();
    bar.hidden = !n;
    setText("multi-sel-count", n);
    setText("multi-sel-col", selColumnLabel());
  }

  // Message flottant bref, en bas de la fenêtre.
  function toast(msg) {
    var t = $("toast");
    if (!t) return;
    t.textContent = msg;
    t.hidden = false;
    clearTimeout(toast._h);
    toast._h = setTimeout(function () { t.hidden = true; }, 2000);
  }

  // --- Menu contextuel (clic droit sur une cellule) -----------------------
  function showCtxMenu(x, y) {
    var m = $("ctx-menu");
    if (!m) return;
    setText("ctx-copy-count", selCount());
    setText("ctx-col-label", selColumnLabel());
    // « Ouvrir » n'a de sens que sur une cellule d'adresse.
    var open = $("ctx-open");
    if (open) open.hidden = !(S.sel.col === 0 && selCount() === 1);
    m.hidden = false;
    // Garder le menu dans la fenêtre.
    var w = m.offsetWidth, h = m.offsetHeight;
    m.style.left = Math.max(0, Math.min(x, window.innerWidth - w - 4)) + "px";
    m.style.top  = Math.max(0, Math.min(y, window.innerHeight - h - 4)) + "px";
  }
  function hideCtxMenu() { var m = $("ctx-menu"); if (m) m.hidden = true; }

  // Ouvre dans le navigateur par defaut (commande Rust `open_external`).
  function openExternal(href) {
    if (!href || !TAURI) return;
    TAURI.core.invoke("open_external", { url: href })
      .catch(function (err) { console.error("open_external", href, err); });
  }

  function initMultiSelect() {
    var body = $("grid-body");

    // Shift+clic : le navigateur voudrait étendre une sélection de TEXTE entre
    // les deux clics. On la coupe à la racine, sur le mousedown.
    body.addEventListener("mousedown", function (e) {
      if (e.shiftKey && e.target.closest("tr[data-url]")) e.preventDefault();
    });

    // Clic droit : la cellule visee rejoint la sélection si elle n'y est pas
    // déjà (sinon on perdrait une plage qu'on vient de construire), puis menu.
    body.addEventListener("contextmenu", function (e) {
      var td = e.target.closest("td"), tr = e.target.closest("tr[data-url]");
      if (!td || !tr) return;
      e.preventDefault();
      var key = normUrl(tr.getAttribute("data-url") || ""), col = td.cellIndex;
      if (!(S.sel.col === col && S.sel.keys[key])) selOne(col, key);
      showCtxMenu(e.clientX, e.clientY);
    });

    $("ctx-copy").addEventListener("click", copySelection);
    $("ctx-copy-col").addEventListener("click", function () { selAllColumn(S.sel.col); copySelection(); });
    $("ctx-open").addEventListener("click", function () {
      hideCtxMenu();
      var t = selTexts();
      if (t.length === 1) openExternal(t[0]);
    });
    // Le menu se ferme des qu'on clique ailleurs, qu'on defile ou qu'on echappe.
    document.addEventListener("click", function (e) { if (!e.target.closest("#ctx-menu")) hideCtxMenu(); });
    document.addEventListener("scroll", hideCtxMenu, true);

    $("multi-sel-copy").addEventListener("click", copySelection);
    $("multi-sel-clear").addEventListener("click", selClear);

    document.addEventListener("keydown", function (e) {
      // Jamais dans un champ de saisie : Ctrl+C / Ctrl+A y gardent leur sens.
      var t = e.target, tag = t && t.tagName ? t.tagName.toLowerCase() : "";
      if (tag === "input" || tag === "textarea" || tag === "select" || (t && t.isContentEditable)) return;
      var mod = e.ctrlKey || e.metaKey;
      if (mod && (e.key === "c" || e.key === "C")) {
        // Sans cellules sélectionnées, on laisse Ctrl+C copier le texte surligne.
        if (selCount()) { e.preventDefault(); copySelection(); }
      } else if (mod && (e.key === "a" || e.key === "A")) {
        if (S.gridRows.length) { e.preventDefault(); selAllColumn(S.sel.col >= 0 ? S.sel.col : 0); }
      } else if (e.key === "Escape") {
        if (!$("ctx-menu").hidden) hideCtxMenu();
        else if (selCount()) selClear();
      }
    });
  }

  function openDetail(url) {
    var p = S.byUrl[normUrl(url)];
    if (!p) return;
    S.selectedUrl = url;
    var detail = $("detail");
    if (detail) detail.hidden = false;
    // Marque la ligne sélectionnée dans la grille.
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
    setText("dt-images-count", imagesMissingAltOf(p).length);
    // D) Compteur de blocs JSON-LD bruts présents sur la page.
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
        "Aucun lien entrant detecte parmi les pages crawlées."
      );
    } else if (S.detailTab === "outlinks") {
      body.innerHTML = renderLinkTable(
        outs.map(function (l) { return { url: l.target, anchor: l.anchor, rel: l.rel, internal: l.internal }; }),
        ["Destination", "Ancre", "Type", "Rel"],
        ["url", "anchor", "type", "rel"],
        "Aucun lien sortant sur cette page."
      );
    } else if (S.detailTab === "images") {
      body.innerHTML = renderImagesMissingAlt(p);
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

  // ----- VAGUE 4 (D) — Données structurees brutes (blocs JSON-LD) -----------
  // Lit seo.structured_data_raw (tableau de chaînes JSON-LD brutes), ou [].
  function structuredRawOf(p) {
    var s = seoOf(p);
    return s && Array.isArray(s.structured_data_raw) ? s.structured_data_raw : [];
  }

  // Tente d'extraire un libellé de type lisible d'un objet JSON-LD parse
  // (champ @type, qui peut être une chaîne ou un tableau). Renvoie "" sinon.
  function jsonLdTypeLabel(obj) {
    if (!obj || typeof obj !== "object") return "";
    var t = obj["@type"];
    if (!t && obj["@graph"]) return "@graph";   // graphe : types multiples à l'intérieur
    if (Array.isArray(t)) return t.join(", ");
    return t ? String(t) : "";
  }

  // Rendu du sous-onglet "Données structurees" : pour chaque bloc JSON-LD brut,
  // on tente JSON.parse + JSON.stringify(obj, null, 2) (indente, lisible) ; si le
  // parse echoue (JSON-LD malforme), on affiche le texte brut tel quel. Chaque
  // bloc est precede d'un petit en-tête (index + type detecte via @type).
  function renderStructuredData(p) {
    var raw = structuredRawOf(p);
    if (!raw.length) {
      // Repli informatif : on signale les types detectes même sans bloc brut
      // (utile si le backend ne fournit pas structured_data_raw pour cette page).
      var s = seoOf(p);
      var types = s && Array.isArray(s.structured_data_types) ? s.structured_data_types : [];
      if (types.length) {
        return '<div class="detail-empty">Types detectes : ' +
               types.map(function (t) { return '<span class="badge">' + esc(t) + '</span>'; }).join(" ") +
               '<br>Aucun bloc JSON-LD brut fourni pour cette page.</div>';
      }
      return '<div class="detail-empty">Aucune donnée structuree (JSON-LD) sur cette page.</div>';
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

  // Images sans attribut alt d'une page : URLs absolues remontees par le backend.
  function imagesMissingAltOf(p) {
    var s = seoOf(p);
    return (s && Array.isArray(s.images_missing_alt_urls)) ? s.images_missing_alt_urls : [];
  }

  // Sous-onglet "Images sans alt" : la LISTE des images en defaut, pas seulement
  // leur nombre — c'est elle qu'il faut pour corriger. Chaque URL s'ouvre dans le
  // navigateur ; le nom de fichier seul est repete a côté, pour reperer l'image
  // d'un coup d'oeil quand l'URL est longue.
  function renderImagesMissingAlt(p) {
    var urls = imagesMissingAltOf(p);
    var s = seoOf(p);
    var missing = s ? (s.images_missing_alt || 0) : 0;
    var items = urls.map(function (u) {
      var file = String(u).split("?")[0].split("/").pop() || u;
      return { url: u, file: file };
    });
    var emptyMsg = missing > 0
      // Le compteur est la mais pas la liste : rapport enregistré AVANT que le
      // backend ne remonte les URLs. Le dire, sinon on cherche un bug ailleurs.
      ? missing + " image(s) sans alt comptee(s), mais aucune URL enregistrée : ce crawl est anterieur à cette version, relancez-le."
      : "Toutes les images de cette page ont un attribut alt.";
    var h = renderLinkTable(items, ["Image (src)", "Fichier"], ["url", "file"], emptyMsg);
    // Le compteur peut depasser la liste : doublons fusionnes, plafond backend,
    // ou `src` vide. On l'explique plutot que de laisser croire à une liste
    // tronquee par erreur.
    if (items.length && missing > items.length) {
      h += '<div class="detail-empty">' + esc(missing + " sans alt au total, " + items.length +
           " URL(s) distincte(s) : la même image répétée ne compte qu'une ligne.") + '</div>';
    }
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
           '<div class="serp-note">Apercu indicatif du rendu dans les résultats Google.</div>' +
           '</div>';
  }

  // --- Helpers texte pour les champs Sécurité du panneau Infos (Vague 2) ---
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
  // --- Helpers texte pour les données structurees (Vague 2) ---
  // "Valide" n'a de sens que si la page porte des données structurees.
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
  // Liste HTML des URLs quasi-dupliquées à la page p (liens cliquables), ou "—".
  function nearDupHtml(p) {
    var sims = nearDupOf(p);
    if (!sims.length) return '<span class="muted">Aucune</span>';
    // Les clés memorisees sont des URLs normalisées ; on retrouve l'URL d'origine
    // via S.byUrl pour un affichage propre (sinon on retombe sur la clé).
    var parts = sims.map(function (k) {
      var pg = S.byUrl[k];
      var u = pg && pg.url ? pg.url : k;
      return urlCellHtml(u);
    });
    return sims.length + ' page(s) : ' + parts.join("  |  ");
  }
  // Texte de l'état de reciprocite hreflang pour la page p.
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

  // Vue "Infos / En-têtes" : liste clé/valeur.
  function renderInfo(p) {
    var s = seoOf(p);
    var rows = [
      ["Adresse", p.url || ""],
      ["Code HTTP", String(p.status || 0)],
      ["Cache", cacheInfoText(p)],
      ["Content-Type", p.content_type || "—"],
      ["Sonde", probeLabel(p.measured_with)],
      ["Poids total", fmtBytes(p.total_bytes || 0)],
      ["Requêtes", String(p.request_count || 0) + " (dont " + (p.dynamic_requests || 0) + " dyn.)"],
      ["Redirige vers", p.redirect_to || "—"],
      ["Chaîne de redirection", (Array.isArray(p.redirect_chain) && p.redirect_chain.length) ? p.redirect_chain.join("  ->  ") : "—"],
      ["Canonical", (s && s.canonical) ? s.canonical : "—"],
      ["Canonicalisée", (s && s.canonicalised) ? "Oui" : "Non"],
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
      ["Données structurees valides", schemaInfoValid(p)],
      ["Erreurs données structurees", schemaInfoErrors(p)],
      ["HTTPS", secInfoHttps(p)],
      ["Contenu mixte", secInfoMixed(p)],
      ["En-têtes manquants", secInfoHeaders(p)],
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
    // HTML déjà echappe en interne (urlCellHtml echappe l'URL).
    // VAGUE 4 (A) : temps de réponse colore (pastille vert/orange/rouge).
    h += '<div class="kv-row"><div class="kv-key">Temps de réponse</div>' +
         '<div class="kv-val">' + speedCellHtml(respMs(p)) + '</div></div>';
    h += '<div class="kv-row"><div class="kv-key">Near-duplicates</div>' +
         '<div class="kv-val">' + nearDupHtml(p) + '</div></div>';
    h += '<div class="kv-row"><div class="kv-key">Hreflang reciproque</div>' +
         '<div class="kv-val">' + hreflangRecipHtml(p) + '</div></div>';
    h += "</div>";
    return h;
  }

  // Vue "Source" : on n'a pas le HTML brut côté frontend, on affiche donc une
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
      lines.push("(pas de données SEO : page en erreur ou non-HTML)");
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

    var curUrl = (prog && prog.current_url) ? prog.current_url : "";
    var urlEl = $("status-url");
    if (urlEl) { urlEl.textContent = curUrl; urlEl.title = curUrl; }
    setText("status-done", done);
    setText("status-total", totalLabel);
    setText("status-queued", queued);
    setText("status-bytes", fmtBytes(bytes));

    // Débit moyen en pages/seconde depuis le début du run.
    if (S.rateStartTs) {
      var elapsed = (Date.now() - S.rateStartTs) / 1000;
      var rate = elapsed > 0 ? (done / elapsed) : 0;
      setText("status-rate", rate.toFixed(1));
    }

    // Workers RÉELLEMENT occupés / threads configurés.
    // Diagnostic : si ce compteur reste bien en dessous du nombre de threads,
    // ajouter des threads ne servira a rien — le goulot est ailleurs (plafond
    // de débit encore actif, file vide, ou mesures Chrome serialisees).
    if (prog && typeof prog.active_workers === "number") {
      setText("status-workers", prog.active_workers);
    }
    setText("status-threads", (S.multi.config && S.multi.config.threads) || 0);

    // Temps de réponse moyen par page (ms) sur les pages mesurées (Vague 4+).
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

  // VAGUE 4 (C) — Met à jour libellés + états (disabled) des 3 boutons selon
  // l'état courant. Trois états :
  //   - running : crawl en cours        -> Démarrer OFF, Stop ON ("Stop"), Vider ON
  //   - paused  : crawl en pause (Stop) -> Démarrer ON ("Reprendre"), Stop OFF, Vider ON
  //   - idle    : rien en cours         -> Démarrer ON ("Démarrer"), Stop OFF, Vider ON
  // C'est CE point qui corrige le bug du bouton "Démarrer" reste grise après Stop.
  function updateButtons() {
    var start = $("btn-start");
    var stop  = $("btn-stop");
    var clear = $("btn-clear");
    if (start) {
      start.disabled = S.running;                 // actif sauf pendant un crawl actif
      start.textContent = S.paused ? "Reprendre" : "Démarrer";
      start.title = S.paused
        ? "Reprendre le crawl en pause (sans effacer les données déjà collectees)"
        : "Lancer un nouveau crawl";
    }
    if (stop) {
      stop.disabled = !S.running;                  // actif seulement pendant un crawl actif
      stop.title = "Mettre en pause (les données collectees sont conservees)";
    }
    if (clear) {
      clear.disabled = false;                      // toujours disponible
      clear.title = "Effacer toutes les données et remettre a zero";
    }
  }

  function setStatusState(txt) { setText("status-state", txt); }

  function updateMultiStatus() {
    var el = $("status-multi");
    if (!el) return;
    if (S.multi.target > 1) {
      el.hidden = false;
      var isList = !!(S.multi.seeds && S.multi.seeds.length);
      var idx = Math.min(S.multi.done + (S.running ? 1 : 0), S.multi.target);
      // En multi-sites, afficher AUSSI le site en cours : sur une liste de 40
      // domaines, "Site 12/40" seul ne dit pas lequel est en train de tourner.
      var label = isList
        ? "Site " + idx + "/" + S.multi.target + " · " + hostOf(S.multi.config.seed_url)
        : "Run " + idx + "/" + S.multi.target;
      el.textContent = label + " — cumul " + fmtBytes(S.multi.totalBytes) +
                       " (" + S.multi.totalPages + " pages)";
    } else {
      el.hidden = true;
    }
  }

  // =========================================================================
  // ABONNEMENTS AUX ÉVÉNEMENTS TAURI (flux de crawl PRESERVE)
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
      // arriver (page courante terminée) -> on garde le libellé "En pause".
      if (p.current_url && !S.paused) setStatusState("Crawl : " + truncUrl(p.current_url, 70));
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:error", function (event) {
      setStatusState("Erreur : " + (event.payload || "inconnue"));
      console.error("crawl:error", event.payload);
    }).then(function (u) { S.unlisteners.push(u); });

    TAURI.event.listen("crawl:sitemap", function (event) {
      // Sitemap reçu AU DÉBUT du crawl (robots.txt + /sitemap.xml) : on remplit
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
      // champs (pages/total_bytes...) sont déjà reçus en flux via crawl:page,
      // on ne les reexploite donc pas ici (pas de double comptage).
      var summary = (event && event.payload) ? event.payload : {};
      var smap = Array.isArray(summary.sitemap_urls) ? summary.sitemap_urls : [];
      if (smap.length) {   // ne pas écraser si vide : l'event crawl:sitemap (début) fait foi
        S.sitemapUrls = smap;
        S.sitemapSet = {};
        for (var si = 0; si < smap.length; si++) {
          if (smap[si]) S.sitemapSet[normUrl(smap[si])] = true;
        }
      }

      // VAGUE 4 (C) : si l'utilisateur a mis EN PAUSE, ce crawl:done provient du
      // crawl backend qui s'est termine de lui-même après le stop_crawl. On
      // recupere le sitemap (ci-dessus) mais on NE cloture PAS la séquence et on
      // NE quitte PAS l'état pause : l'affichage reste fige, "Reprendre" actif.
      if (S.paused) {
        // Seul point ou le backend GARANTIT que tous les workers sont sortis.
        // Le reset fait dans `onStop()` est réécrit par les `crawl:progress`
        // tardifs : au clic Stop, jusqu'à `threads` pages sont encore en vol et
        // chacune emet sa progression (active_workers = N…1) APRÈS lui.
        setText("status-workers", 0);
        scheduleRender();
        return;
      }

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
      scheduleRender();   // rafraîchit compteur + grille de l'onglet Sitemap

      // MULTI-SITES : chaque site est enregistré DÈS QU'IL EST FINI, avant que
      // `startRun()` ne bascule la seed vers le suivant. Un plantage en cours de
      // liste ne fait donc plus perdre les sites déjà crawles.
      if (S.multi.seeds && S.multi.seeds.length) saveCurrentCrawl();

      if (!S.multi.stopRequested && S.multi.done < S.multi.target) {
        startRun();                       // site suivant de la liste importée
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
    // MULTI-SITES : la seed du run courant vient de la liste importée, et on
    // n'efface PAS les données — les pages de tous les sites s'accumulent dans
    // la même grille (c'est l'intérêt d'un audit de réseau). Chaque site est
    // néanmoins enregistré SÉPARÉMENT dans son propre fichier projet, d'ou le
    // curseur `pagesOffset` qui delimite ses pages dans `S.pages`.
    if (S.multi.seeds && S.multi.seeds.length) {
      S.multi.config.seed_url = S.multi.seeds[S.multi.done] || S.multi.config.seed_url;
      S.multi.pagesOffset = S.pages.length;
    } else {
      // Run unique : on repart d'un jeu de données vide (mais on garde la config).
      resetData();
    }
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
  // reprise : start_crawl, appele avec la MÊME seed alors qu'un état reprenable
  // existe (file non vide), RELANCE les workers sur la file restante — il ne
  // re-crawle donc PAS les pages déjà vues et n'emet que les NOUVELLES pages.
  // Côté UI on conserve les données déjà collectees et on poursuit le cumul.
  // (L'ingestion idempotente reste un filet de sécurité si une page revenait.)
  function resumeRun() {
    S.running = true;
    S.paused = false;
    S.multi.stopRequested = false;
    S.multi.running = true;
    updateButtons();
    S.rateStartTs = Date.now();
    setStatusState("Reprise du crawl...");
    updateMultiStatus();

    // On se re-abonne aux événements (ils avaient été laisses actifs, mais on
    // garantit l'état) puis on relance le crawl backend avec la même config.
    TAURI.core.invoke("start_crawl", { config: S.multi.config })
      .catch(function (err) {
        setStatusState("Erreur : " + (err || "inconnue"));
        console.error("start_crawl (resume) error:", err);
        finishSequence();
      });
  }

  // B : enregistre le crawl courant dans le local data (crawls/<site>.json).
  // Appele à la FIN et au STOP. save_crawl ÉCRASE l'enregistrement précédent du
  // même site -> relancer un crawl sur la même URL remplace l'ancien.
  function saveCurrentCrawl(showPath) {
    var seed = S.multi.config && S.multi.config.seed_url;
    if (!TAURI || !seed || !S.pages.length) return;
    // MULTI-SITES : n'enregistrer que les pages DU SITE COURANT, pas la grille
    // cumulee. Sans ce decoupage, le fichier projet de chaque site contiendrait
    // aussi les pages de tous les sites précédents.
    var pages = (S.multi.seeds && S.multi.seeds.length)
      ? S.pages.slice(S.multi.pagesOffset || 0)
      : S.pages.slice();
    if (!pages.length) return;
    var crawlData = JSON.stringify({
      seed_url: seed,
      date: new Date().toISOString(),
      pages: pages,
      sitemap_urls: S.sitemapUrls || []
    });
    TAURI.core.invoke("save_crawl", { id: seed, json: crawlData, dir: saveDirArg() })
      .then(function (path) { if (path) { S.lastSavePath = path; if (showPath) setStatusState("Enregistre : " + path); } })
      .catch(function (err) { console.error("save_crawl error:", err); });
  }

  function finishSequence() {
    S.multi.running = false;
    S.running = false;
    S.paused = false;                 // VAGUE 4 (C) : fin réelle -> retour à l'état idle
    updateButtons();
    var fill = $("status-fill");
    if (fill) fill.style.width = "100%";
    var isList = !!(S.multi.seeds && S.multi.seeds.length);
    setStatusState(isList
      ? "Termine — " + S.multi.done + " site(s), " + S.multi.totalPages + " page(s)."
      : "Termine — " + S.multi.done + " run(s), " + S.multi.totalPages + " page(s).");
    // Plus aucun worker ne tourne : le dernier `crawl:progress` a été emis par
    // le dernier worker ALORS QU'IL etait encore compte actif (voulu en cours de
    // crawl, mensonger une fois termine).
    setText("status-workers", 0);
    updateMultiStatus();

    // Une entrée d'historique par séquence.
    addHistoryEntry({
      date: new Date().toISOString(),
      // En multi-sites, la seed courante est le DERNIER site de la liste :
      // l'historique afficherait "https://dernier-site.fr" pour une série de 40.
      url: isList
        ? S.multi.seeds.length + " sites (" + hostOf(S.multi.seeds[0]) + "…)"
        : (S.multi.config.seed_url || ""),
      seeds: isList ? S.multi.seeds.slice() : undefined,
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
        no_rate_limit: S.multi.config.no_rate_limit,
        threads: S.multi.config.threads,
        unlimited: S.multi.config.unlimited,
        respect_robots: S.multi.config.respect_robots,
        force_chrome: S.multi.config.force_chrome,
        random_ua: S.multi.config.random_user_agent
      },
      pagesData: (S.multi.lastRows || []).slice(0, 5000)   // pour la consultation
    });

    // B : enregistre le crawl complet (écrase l'ancien du même site).
    // En multi-sites, chaque site a DÉJÀ été enregistré à la fin de son propre
    // crawl (dans le handler `crawl:done`) : re-sauver ici ne ferait que
    // reecrire à l'identique le dernier de la liste.
    if (!isList) saveCurrentCrawl(true);

    // Le crawl est fini : on repasse d'un affichage FRAIS (cadence plafonnee)
    // à un affichage EXACT. Sans ce rendu force, la carte des near-duplicates
    // garderait l'état de son dernier calcul, jusqu'à 3 s avant la fin.
    renderNow();
  }

  // Vide UNIQUEMENT les données du run courant (pages, index, issues, bw).
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
    S.sel = { col: -1, keys: {}, anchor: null };
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

  // C : recharge un crawl ENREGISTRE (consultation, sans relancer de crawl).
  // Reutilise ingestPage() -> tous les index/agrégats se reconstruisent au rendu.
  function loadSavedCrawl(data) {
    if (TAURI) TAURI.core.invoke("clear_crawl").catch(function () {});
    unsubscribeEvents();
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();
    resetData();
    S.running = false; S.paused = false;
    S.multi = {
      target: 0, done: 0, totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: false,
      config: { seed_url: data.seed_url || "" }, lastTechs: [], lastRows: []
    };
    var pages = Array.isArray(data.pages) ? data.pages : [];
    for (var i = 0; i < pages.length; i++) ingestPage(pages[i]);
    S.sitemapUrls = Array.isArray(data.sitemap_urls) ? data.sitemap_urls : [];
    S.sitemapSet = {};
    for (var s = 0; s < S.sitemapUrls.length; s++) {
      if (S.sitemapUrls[s]) S.sitemapSet[normUrl(S.sitemapUrls[s])] = true;
    }
    var when = data.date ? (" (enregistré le " + String(data.date).slice(0, 10) + ")") : "";
    setStatusState("Consultation : " + (data.seed_url || "") + " — " + pages.length + " page(s)" + when);
    updateButtons();
    scheduleRender();
  }

  // C : dialogue Nouveau crawl / Revoir l'enregistrement (créé dynamiquement).
  function showCrawlChoice(url, data) {
    var old = $("crawl-choice-modal"); if (old) old.parentNode.removeChild(old);
    var nb = Array.isArray(data.pages) ? data.pages.length : 0;
    var when = data.date ? (" le " + String(data.date).slice(0, 10)) : "";
    var ov = document.createElement("div");
    ov.id = "crawl-choice-modal";
    ov.style.cssText = "position:fixed;inset:0;background:rgba(0,0,0,.45);display:flex;align-items:center;justify-content:center;z-index:9999;";
    var box = document.createElement("div");
    box.style.cssText = "background:var(--header-bg2,#f5f6f8);color:var(--text,#1c2330);border:1px solid var(--border,#ccd);border-radius:8px;padding:18px 20px;max-width:500px;font-size:13px;box-shadow:0 10px 40px rgba(0,0,0,.35);";
    box.innerHTML =
      '<div style="font-weight:600;margin-bottom:8px;font-size:14px;">Crawl déjà enregistré</div>' +
      '<div style="margin-bottom:16px;line-height:1.45;">Le site <strong>' + esc(url) + '</strong> a déjà été crawle' + when +
      ' (' + nb + ' page(s) enregistrées). Que veux-tu faire ?</div>' +
      '<div style="display:flex;gap:8px;justify-content:flex-end;flex-wrap:wrap;">' +
      '<button type="button" id="cc-cancel" class="btn">Annuler</button>' +
      '<button type="button" id="cc-review" class="btn">Revoir cet enregistrement</button>' +
      '<button type="button" id="cc-new" class="btn btn--primary">Nouveau crawl</button>' +
      '</div>';
    ov.appendChild(box);
    document.body.appendChild(ov);
    function close() { if (ov.parentNode) ov.parentNode.removeChild(ov); }
    $("cc-cancel").onclick = close;
    $("cc-review").onclick = function () { close(); loadSavedCrawl(data); };
    $("cc-new").onclick = function () { close(); S._forceNew = true; onSubmit({ preventDefault: function () {} }); };
    ov.onclick = function (e) { if (e.target === ov) close(); };
  }

  // =========================================================================
  // SOUMISSION DU FORMULAIRE (lance le crawl)
  // =========================================================================
  function readConfig() {
    var unlimited = $("opt-unlimited").checked;
    var seedUrl  = $("seed-url").value.trim();
    // En mode "Tout le site" le champ est verrouille et affiche "∞ (max ...)" :
    // sa `value` n'est plus un nombre. On lit alors la dernière valeur saisie
    // (`data-last`). De toute facon `unlimited` fait ignorer `max_pages` côté
    // backend — mais on évite d'envoyer un NaN silencieusement retombe a 500.
    var mpEl = $("max-pages");
    var mpRaw = (mpEl.type === "number") ? mpEl.value : mpEl.dataset.last;
    var maxPages = parseInt(mpRaw, 10) || 500;
    var maxDepth = parseInt($("max-depth").value, 10) || 10;
    var rps      = Math.max(1, Math.round(parseFloat($("rps").value) || 5));
    // VAGUE 4 (B) : nombre de pages traitées en parallele (threads du crawler).
    // Borne 1..50, defaut 5 (coherent avec l'attribut min/max du champ HTML).
    var threads  = Math.min(50, Math.max(1, parseInt($("threads").value, 10) || 5));
    return {
      seed_url: seedUrl,
      max_pages: maxPages,
      max_depth: maxDepth,
      same_domain_only: true,
      requests_per_second: rps,
      threads: threads,                           // <-- VAGUE 4 (B) : envoye au backend
      // Leve le plafond de débit : sans ca le bucket est GLOBAL et le crawl
      // reste colle a `rps` quel que soit le nombre de threads.
      no_rate_limit: $("no-rate-limit").checked,
      force_chrome: $("force-chrome").checked,
      random_user_agent: $("random-ua").checked,
      // Le délai aléatoire (0,5-4,5 s par page et par worker) annulerait le
      // benefice du débit illimité : les deux s'excluent.
      random_delay: $("no-rate-limit").checked ? false : $("random-rps").checked,
      unlimited: unlimited,                       // <-- envoye au backend
      respect_robots: $("opt-robots").checked     // <-- envoye au backend
    };
  }

  // =========================================================================
  // IMPORT D'UNE LISTE D'URLs (mode multi-sites)
  //
  // Chaque ligne du fichier devient la SEED d'un crawl complet. Les sites sont
  // enchaines l'un après l'autre (`S.multi.target` = nombre d'URLs) et leurs
  // pages s'accumulent dans la même grille, tout en etant enregistrées dans un
  // fichier projet PAR SITE (cf. `saveCurrentCrawl` et `S.multi.pagesOffset`).
  // =========================================================================
  var IMPORT = { urls: [] };

  function renderImportBadge() {
    var b = $("import-badge");
    if (!b) return;
    if (!IMPORT.urls.length) { b.hidden = true; b.innerHTML = ""; return; }
    b.hidden = false;
    b.innerHTML = IMPORT.urls.length + " site" + (IMPORT.urls.length > 1 ? "s" : "") +
                  ' <span class="x" title="Retirer la liste">×</span>';
    b.querySelector(".x").addEventListener("click", function () {
      IMPORT.urls = [];
      renderImportBadge();
      setStatusState("Liste retiree.");
    });
  }

  function onImportList() {
    if (!TAURI) return;
    TAURI.core.invoke("pick_url_list").then(function (urls) {
      if (!urls) return;                       // dialogue annule
      if (!urls.length) {
        setStatusState("Aucune URL exploitable dans ce fichier.");
        return;
      }
      IMPORT.urls = urls;
      renderImportBadge();
      // Montre la 1re URL dans la barre : l'utilisateur voit ce qui va partir.
      var se = $("seed-url"); if (se) se.value = urls[0];
      setStatusState(urls.length + " URL(s) importée(s) — cliquez sur Crawler pour lancer la série.");
    }).catch(function (err) {
      setStatusState("Import impossible : " + (err || "erreur inconnue"));
      console.error("pick_url_list error:", err);
    });
  }

  // Demarre la série de crawls sur la liste importée.
  function startSiteList() {
    var config = readConfig();
    config.seed_url = IMPORT.urls[0];

    S.multi = {
      target: IMPORT.urls.length, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: true,
      config: config, lastTechs: [], lastRows: [],
      seeds: IMPORT.urls.slice(),   // <- active le mode multi-sites
      pagesOffset: 0
    };

    // Nouvelle série : grille vierge et filtres remis a zero.
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();
    resetData();
    S.multi.pagesOffset = 0;

    // Même séquence de demarrage que le crawl simple : desabonnement d'abord
    // (sinon les listeners s'empilent et chaque page serait ingeree N fois),
    // puis vidage de l'état backend pour ne pas "reprendre" par erreur une file
    // reprenable. `clear_crawl` est async : on chaîne pour éviter la course.
    unsubscribeEvents();
    subscribeEvents();
    Promise.resolve(TAURI.core.invoke("clear_crawl").catch(function () {}))
      .then(function () { startRun(); });
  }

  // =========================================================================
  // PANNEAU DE DÉTAIL REDIMENSIONNABLE
  //
  // Le panneau etait fige a 240 px : sur une page a 30 inlinks, on n'en voyait
  // que 8 et il fallait faire defiler dans une lucarne. On le redimensionne en
  // glissant sa bordure supérieure ; la hauteur est persistee.
  // =========================================================================
  var DETAIL_DEFAULT_H = 240;

  function applyDetailHeight(px) {
    var d = $("detail");
    if (!d) return;
    // Bornes : jamais plus haut que 85% de la fenêtre (la grille doit rester
    // visible), jamais plus bas que 120 px (les onglets doivent tenir).
    var maxH = Math.round(window.innerHeight * 0.85);
    var h = Math.max(120, Math.min(maxH, Math.round(px)));
    d.style.flexBasis = h + "px";
    return h;
  }

  function initDetailResize() {
    var grip = $("detail-resize");
    var panel = $("detail");
    if (!grip || !panel) return;

    // Hauteur memorisee de la session précédente.
    if (PREFS.detailHeight) applyDetailHeight(PREFS.detailHeight);

    var startY = 0, startH = 0, dragging = false;

    function onMove(e) {
      if (!dragging) return;
      // La poignee est EN HAUT du panneau : tirer vers le haut (deltaY négatif)
      // doit AGRANDIR, d'ou la soustraction.
      var h = applyDetailHeight(startH - (e.clientY - startY));
      PREFS.detailHeight = h;
    }
    function onUp() {
      if (!dragging) return;
      dragging = false;
      grip.classList.remove("is-dragging");
      document.body.classList.remove("is-resizing-detail");
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      persistPrefs();          // une seule ecriture, à la fin du glisser
    }

    grip.addEventListener("mousedown", function (e) {
      e.preventDefault();
      dragging = true;
      startY = e.clientY;
      startH = panel.getBoundingClientRect().height;
      grip.classList.add("is-dragging");
      document.body.classList.add("is-resizing-detail");
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    });

    // Double-clic : retour à la hauteur par defaut.
    grip.addEventListener("dblclick", function () {
      PREFS.detailHeight = applyDetailHeight(DETAIL_DEFAULT_H);
      persistPrefs();
    });

    // Fenêtre retrecie : re-borner pour ne pas garder un panneau plus grand
    // que l'écran (la grille deviendrait inatteignable).
    window.addEventListener("resize", function () {
      if (PREFS.detailHeight) applyDetailHeight(PREFS.detailHeight);
    });
  }

  function onSubmit(evt) {
    evt.preventDefault();
    if (!TAURI) { alert("Interface Tauri non disponible. Lancez l'application via Tauri."); return; }

    // VAGUE 4 (C) — Aligne sur le backend (start_crawl = démarrer OU reprendre).
    // Si un crawl est EN PAUSE et que l'URL saisie correspond à la seed en pause
    // (ou est vide), le bouton "Reprendre" REPREND la file existante côté backend
    // SANS vider les données ni réinitialiser la séquence. Si l'URL a change, on
    // considere que l'utilisateur veut un NOUVEAU crawl.
    // LISTE IMPORTÉE : elle prime sur la barre d'URL. On court-circuite le
    // dialogue "Nouveau / Revoir" (il n'a pas de sens pour 40 sites) et on
    // enchaine directement les crawls.
    if (IMPORT.urls.length && !S.paused) {
      startSiteList();
      return;
    }

    var typedUrl = $("seed-url").value.trim();
    if (S.paused) {
      var pausedSeed = (S.multi.config && S.multi.config.seed_url) || "";
      if (!typedUrl || typedUrl === pausedSeed) { resumeRun(); return; }
      // URL différente : on abandonne l'état en pause et on repart proprement.
    }

    // C : si un crawl est déjà enregistré pour cette URL, proposer Nouveau / Revoir.
    if (!S._forceNew && typedUrl) {
      TAURI.core.invoke("load_crawl", { id: typedUrl, dir: saveDirArg() }).then(function (json) {
        var saved = null;
        try { saved = json ? JSON.parse(json) : null; } catch (e) { saved = null; }
        if (saved && Array.isArray(saved.pages) && saved.pages.length) {
          showCrawlChoice(typedUrl, saved);
        } else {
          S._forceNew = true; onSubmit(evt);
        }
      }).catch(function () { S._forceNew = true; onSubmit(evt); });
      return;
    }
    S._forceNew = false;

    var repetitions = 1; // repetition retiree : crawl unique (bande passante mesurée à chaque passage)
    var config = readConfig();

    // (Re)initialise l'agregat multi-run.
    S.multi = {
      target: repetitions, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: true,
      config: config, lastTechs: [], lastRows: []
    };

    // Nouvelle séquence : jeu de données vierge + reset des filtres.
    S.search = ""; var se = $("grid-search"); if (se) se.value = "";
    clearCategoryFilter();

    unsubscribeEvents();
    subscribeEvents();

    // Depart PROPRE : on vide d'abord l'état backend (clear_crawl) pour ne pas
    // "reprendre" par erreur une file reprenable de la même seed, PUIS on lance.
    // clear_crawl est async : on chaîne startRun() pour éviter une course.
    Promise.resolve(TAURI.core.invoke("clear_crawl").catch(function () {}))
      .then(function () { startRun(); });  // running=true + paused=false + updateButtons()
  }

  // VAGUE 4 (C) — Bouton Stop = MISE EN PAUSE. Le crawl s'arrete côté affichage
  // (on coupe l'emission backend via stop_crawl) mais les DONNÉES collectees
  // sont CONSERVEES. On stoppe aussi l'enchainement multi-run. Surtout : le
  // bouton "Démarrer" (devenu "Reprendre") redevient ACTIF immédiatement
  // (correction du bug ou il restait grise après un Stop).
  function onStop() {
    if (!TAURI) return;
    if (!S.running) return;                 // rien a mettre en pause
    S.multi.stopRequested = true;           // stoppe l'enchainement des runs
    TAURI.core.invoke("stop_crawl").catch(function (err) { console.error("stop_crawl error:", err); });
    S.running = false;
    S.paused = true;                        // -> "Reprendre" disponible
    updateButtons();
    setStatusState("En pause — " + S.pages.length + " page(s) collectee(s).");
    // Les workers finissent leur page courante puis sortent : le compteur ne
    // doit pas rester fige sur la dernière valeur observee en plein crawl.
    setText("status-workers", 0);
    saveCurrentCrawl();   // B : enregistre aussi à la pause (reprise / consultation ensuite)
  }

  // Bouton "Vider" : VIDE l'état de crawl backend + remet l'affichage a zero.
  // VAGUE 4 (C) : on invoque clear_crawl (et non stop_crawl) pour que le backend
  // OUBLIÉ la file/visited de l'état partage. Sans ca, un Stop met seulement en
  // PAUSE (état conserve) et un Démarrer ulterieur sur la même URL REPRENDRAIT
  // l'ancienne file au lieu de repartir de zero. clear_crawl est idempotent
  // (sans effet si aucun état n'existe).
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
    setText("status-workers", 0); setText("status-threads", 0);
    updateMultiStatus();
    S.paused = false;                  // VAGUE 4 (C) : retour à l'état idle
    updateButtons();
  }

  // =========================================================================
  // FILTRES (recherche, catégorie sidebar, sévérité) + bascule d'onglet
  // =========================================================================
  function setActiveTab(key) {
    S.activeTab = key;
    S.sort = { key: null, dir: null };     // nouveau tri par onglet
    document.querySelectorAll(".tab").forEach(function (t) {
      t.classList.toggle("is-active", t.getAttribute("data-tab") === key);
    });
    // VAGUE 4 (E) : on reporte l'onglet courant sur l'en-tête de grille, ce qui
    // declenche le lisere colore (CSS #grid-head[data-tab="..."]).
    var gh = $("grid-head");
    if (gh) gh.setAttribute("data-tab", key);
    // Filtres de sévérité : visibles seulement sur l'onglet Problèmes.
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
  //   "all", "orphan" et "notin" sont trois vues distinctes ; toute autre valeur
  //   retombe sur "all".
  function selectSitemapView(view) {
    clearCategoryFilter();                 // les filtres catégorie ne s'appliquent pas ici
    S.sitemapView = (view === "notin" || view === "orphan") ? view : "all";
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
  // header : tableau de libellés ; rows : tableau de tableaux de cellules brutes.
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
    // On applique le même tri que l'affichage.
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
  // NB : côté backend chaque image est un AssetReport { url, bytes, kind } ; on
  // expose 'url' sous l'en-tête 'src' (la source de l'image).
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

  // Tous les problèmes (issues backend + hreflang JS) : sévérité,catégorie,adresse,message.
  function exportAllIssues() {
    var rows = S.issues.concat(S.hreflangIssues).map(function (it) {
      return [ sevLabel(it.severity), it.category || "", it.url || "", it.message || "" ];
    });
    downloadCsv("problèmes-tous", ["sévérité", "catégorie", "adresse", "message"], rows);
  }

  // =========================================================================
  // VAGUE 3 — RAPPORTS DE SYNTHESE (CSV cibles)
  // =========================================================================

  // Chaînes de redirection : pages avec redirect_chain non vide.
  // Colonnes : url, chaîne (jointe par " -> "), destination finale.
  function reportRedirectChains() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var chain = Array.isArray(p.redirect_chain) ? p.redirect_chain : [];
      if (!chain.length) continue;
      // Destination finale = dernier maillon de la chaîne, ou redirect_to en repli.
      var dest = chain.length ? chain[chain.length - 1] : (p.redirect_to || "");
      rows.push([ p.url || "", chain.join(" -> "), dest ]);
    }
    downloadCsv("rapport-redirections", ["url", "chaîne", "destination"], rows);
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

  // Pages non-indexables : url, raison (noindex / canonicalisée / bloquée).
  // On ne liste que les pages effectivement NON indexables. La raison est
  // déduite des signaux SEO : meta/x-robots noindex, canonical vers une autre
  // URL (canonicalised), ou statut HTTP non-2xx (bloquée/erreur).
  function reportNonIndexable() {
    var rows = [];
    for (var i = 0; i < S.pages.length; i++) {
      var p = S.pages[i];
      var s = seoOf(p);
      var indexable = s ? !!s.indexable : false;
      if (indexable) continue;             // page indexable : exclue du rapport
      var reasons = [];
      if (s && s.noindex) reasons.push("noindex");
      if (s && s.canonicalised) reasons.push("canonicalisée");
      var code = p.status || 0;
      if (code < 200 || code >= 300) reasons.push("bloquée (HTTP " + code + ")");
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
  // INFO RÉSEAU (IP + reverse DNS) — PRESERVE
  // =========================================================================
  // Préférences persistantes (localStorage) : dossier d'enregistrement, largeurs de colonnes...
  var PREFS = (function () {
    try { return JSON.parse(localStorage.getItem("seoSpiderPrefs")) || {}; } catch (e) { return {}; }
  })();
  function persistPrefs() {
    try { localStorage.setItem("seoSpiderPrefs", JSON.stringify(PREFS)); } catch (e) {}
  }
  // Dossier d'enregistrement a passer aux commandes save_crawl/load_crawl (null = defaut app).
  function saveDirArg() { return (PREFS.saveDir && PREFS.saveDir.trim()) ? PREFS.saveDir.trim() : null; }

  // =========================================================================
  // PERSISTANCE DES RÉGLAGES (PREFS.settings)
  //
  // Tous les réglages du volet + les deux cases de la barre du haut sont
  // memorises et restitues au lancement suivant : on ne recommence pas sa
  // configuration à chaque ouverture.
  //
  // Les CASES a cocher et les CHAMPS sont traites séparément (`.checked` vs
  // `.value`). "max-pages" est un cas particulier : il bascule entre `number`
  // et `text` (cf. lockMaxPages), donc on persiste `data-last` — la dernière
  // valeur NUMÉRIQUE saisie — et jamais le libellé "∞ (max ...)".
  // =========================================================================
  var SETTING_CHECKS = [
    "opt-unlimited", "opt-robots",
    "no-rate-limit", "random-rps", "force-chrome", "random-ua"
  ];
  var SETTING_FIELDS = ["max-depth", "rps", "threads"];

  function persistSettings() {
    var s = {};
    SETTING_CHECKS.forEach(function (id) {
      var el = $(id); if (el) s[id] = !!el.checked;
    });
    SETTING_FIELDS.forEach(function (id) {
      var el = $(id); if (el) s[id] = el.value;
    });
    // Valeur numérique de "Pages max", même quand le champ est verrouille.
    var mp = $("max-pages");
    if (mp) s["max-pages"] = (mp.type === "number" && mp.value) ? mp.value : (mp.dataset.last || "500");
    PREFS.settings = s;
    persistPrefs();
  }

  // Restaure les réglages memorises. A appeler AVANT syncUnlimited()/syncRate(),
  // qui derivent l'état grise (et donc l'affichage) de ces valeurs.
  function restoreSettings() {
    var s = PREFS.settings;
    if (!s) return;
    SETTING_CHECKS.forEach(function (id) {
      var el = $(id); if (el && typeof s[id] === "boolean") el.checked = s[id];
    });
    SETTING_FIELDS.forEach(function (id) {
      var el = $(id); if (el && s[id] != null && s[id] !== "") el.value = s[id];
    });
    var mp = $("max-pages");
    if (mp && s["max-pages"]) {
      mp.dataset.last = s["max-pages"];
      // Ecrire AUSSI dans `value` tant que le champ est en mode `number` (son
      // état au chargement) : `lockMaxPages()`, appele juste après par
      // `syncUnlimited()`, recopie `value` dans `data-last` et ECRASERAIT la
      // valeur restauree par le "500" de l'attribut HTML.
      if (mp.type === "number") mp.value = s["max-pages"];
    }
  }

  // Branche la sauvegarde sur tout changement de réglage.
  function watchSettings() {
    SETTING_CHECKS.concat(SETTING_FIELDS).concat(["max-pages"]).forEach(function (id) {
      var el = $(id);
      if (el) el.addEventListener("change", persistSettings);
    });
  }

  // #E : init du champ "Dossier des projets" + bouton Ouvrir.
  function initSaveDir() {
    var input = $("save-dir");
    if (input) {
      input.value = PREFS.saveDir || "";
      input.addEventListener("change", function () { PREFS.saveDir = input.value.trim(); persistPrefs(); });
    }
    // #E : choisir le DOSSIER de sauvegarde via un dialogue natif.
    var pick = $("btn-pick-folder");
    if (pick) pick.addEventListener("click", function () {
      if (!TAURI) return;
      TAURI.core.invoke("pick_folder").then(function (path) {
        if (path) {
          PREFS.saveDir = path; persistPrefs();
          if (input) input.value = path;
          setStatusState("Dossier de sauvegarde : " + path);
        }
      }).catch(function (e) { console.error("pick_folder:", e); });
    });
    // #E : ouvrir un PROJET enregistré (dialogue fichier .json) -> consultation.
    var openBtn = $("btn-open-project");
    if (openBtn) openBtn.addEventListener("click", function () {
      if (!TAURI) return;
      TAURI.core.invoke("pick_open_project").then(function (json) {
        if (!json) return;
        var data = null;
        try { data = JSON.parse(json); } catch (e) { data = null; }
        if (data && Array.isArray(data.pages)) { loadSavedCrawl(data); }
        else { setStatusState("Fichier de projet invalide ou vide."); }
      }).catch(function (e) { console.error("pick_open_project:", e); });
    });
  }

  // Clic sur une URL -> ouverture dans le NAVIGATEUR PAR DEFAUT du système.
  //
  // Dans une WebView il n'y a ni onglet ni fenêtre ou aller : un
  // `target="_blank"` ne produit rien, et laisser le lien se suivre remplacerait
  // l'application elle-même par la page distante. Le clic est donc intercepte et
  // l'URL confiee au système via la commande `open_external`.
  //
  // Un SEUL gestionnaire, delegue sur le document : il couvre tous les liens
  // d'URL de l'application — grille, panneau de detail, onglets Liens et
  // Sitemap — y compris ceux crees plus tard, puisqu'il ne vise pas les noeuds
  // mais la classe.
  //
  // En phase de CAPTURE (3e argument a `true`), et c'est indispensable : la
  // sélection de ligne est branchee sur le tbody, qui en phase de bouillonnage
  // recevrait le clic AVANT le document et ouvrirait le panneau de detail au
  // passage. En capture, le document voit l'événement en premier et peut
  // l'arrêter. C'est ce que faisait l'ancien `onclick` pose sur le lien lui-même.
  function initExternalLinks() {
    // Simple clic : on BLOQUE la navigation (la WebView n'a nulle part ou aller)
    // et on laisse l'événement descendre jusqu'à la grille, qui selectionne la
    // cellule. Ouvrir au simple clic rendait la colonne Adresse impossible a
    // sélectionner : c'est le double-clic qui ouvre.
    document.addEventListener("click", function (e) {
      var a = e.target.closest && e.target.closest("a.cell-url");
      if (a) e.preventDefault();
    }, true);

    // Double-clic sur une adresse -> navigateur par defaut. En phase de CAPTURE
    // pour passer avant tout autre gestionnaire ; la sélection de texte que le
    // double-clic aurait posee sur le mot est retiree, elle n'a plus de sens.
    document.addEventListener("dblclick", function (e) {
      var a = e.target.closest && e.target.closest("a.cell-url");
      if (!a) return;
      e.preventDefault();
      e.stopPropagation();
      var s = window.getSelection ? window.getSelection() : null;
      if (s && s.removeAllRanges) s.removeAllRanges();
      openExternal(a.getAttribute("href"));
    }, true);
  }

  // #F : redimensionnement des colonnes à la souris ; largeurs persistees par onglet.
  function initColResize() {
    var rz = null;
    document.addEventListener("mousedown", function (e) {
      var h = e.target.closest && e.target.closest(".col-resizer");
      if (!h) return;
      var th = h.closest("th"); if (!th) return;
      e.preventDefault();
      rz = { th: th, key: th.getAttribute("data-key"), startX: e.clientX, startW: th.offsetWidth, moved: false };
      document.body.style.cursor = "col-resize";
    });
    document.addEventListener("mousemove", function (e) {
      if (!rz) return;
      var w = Math.max(40, rz.startW + (e.clientX - rz.startX));
      rz.th.style.width = w + "px";
      rz.moved = true;
    });
    document.addEventListener("mouseup", function () {
      if (!rz) return;
      if (rz.key && rz.moved) {
        PREFS.colWidths = PREFS.colWidths || {};
        PREFS.colWidths[S.activeTab] = PREFS.colWidths[S.activeTab] || {};
        PREFS.colWidths[S.activeTab][rz.key] = rz.th.offsetWidth;
        persistPrefs();
        S._suppressSort = true; setTimeout(function () { S._suppressSort = false; }, 0);
      }
      document.body.style.cursor = "";
      rz = null;
    });
  }

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
  // HISTORIQUE PERSISTANT (load/save_history) — PRESERVE (sans UI dédiée :
  // on conserve le stockage pour ne pas perdre la fonctionnalite backend).
  // =========================================================================
  function loadHistory() {
    if (!TAURI) return;
    TAURI.core.invoke("load_history")
      .then(function (json) {
        try { history = JSON.parse(json) || []; } catch (e) { history = []; }
        if (!Array.isArray(history)) history = [];
        refreshUrlDatalist();
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
    refreshUrlDatalist();
  }
  // Autocomplete de la barre d'URL : propose les sites déjà crawles (historique).
  function refreshUrlDatalist() {
    var dl = $("url-history");
    if (!dl) return;
    var seen = {}, opts = [];
    for (var i = 0; i < history.length; i++) {
      var u = history[i] && history[i].url;
      if (!u) continue;
      var k = normUrl(u);                 // dedup par URL normalisée : un site ne s'affiche qu'une fois
      if (seen[k]) continue;
      seen[k] = 1;
      opts.push('<option value="' + esc(u) + '"></option>');
    }
    dl.innerHTML = opts.join("");
  }

  // =========================================================================
  // MENUS DEROULANTS (Export en masse / Rapports) — Vague 3
  // Petit composant maison : bouton .menu-btn qui bascule .is-open sur le
  // conteneur .menu. Un seul menu ouvert à la fois ; clic extérieur = fermeture.
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
  // BRANCHEMENT DES CONTRÔLES (delegation d'événements)
  // =========================================================================
  function setupControls() {
    $("crawl-form").addEventListener("submit", onSubmit);
    $("btn-stop").addEventListener("click", onStop);
    $("btn-clear").addEventListener("click", onClear);
    $("btn-import").addEventListener("click", onImportList);
    $("btn-export-csv").addEventListener("click", exportCsv);

    // ----- Menus deroulants Vague 3 : Export en masse + Rapports -----
    // Ouverture/fermeture au clic sur le bouton ; fermeture au clic ailleurs.
    setupMenu("menu-bulk");
    setupMenu("menu-report");
    // Sélection d'une entrée dans le menu Export en masse.
    var bulkList = $("menu-bulk-list");
    if (bulkList) bulkList.addEventListener("click", function (e) {
      var it = e.target.closest("[data-export]");
      if (!it) return;
      runBulkExport(it.getAttribute("data-export"));
      closeMenus();
    });
    // Sélection d'une entrée dans le menu Rapports.
    var reportList = $("menu-report-list");
    if (reportList) reportList.addEventListener("click", function (e) {
      var it = e.target.closest("[data-report]");
      if (!it) return;
      runReport(it.getAttribute("data-report"));
      closeMenus();
    });

    // Selecteur de vue de l'onglet Sitemap (Toutes / Orphelines / Hors sitemap).
    var smSel = $("sitemap-view");
    if (smSel) smSel.addEventListener("change", function () {
      S.sitemapView = (this.value === "notin" || this.value === "orphan") ? this.value : "all";
      renderGrid();
      renderOverview();
    });

    // Case "Tout le site" : le champ "Pages max" affiche alors le plafond
    // RÉELLEMENT actif au lieu d'une valeur grisee sans effet.
    //
    // Un champ `disabled` affichant "500" alors que la limite est de 200 000
    // ment deux fois : il montre une valeur qui ne s'applique pas, et il cache
    // celle qui s'applique. On le passe donc en VERROUILLE (readonly + style
    // grise) : il affiche le vrai plafond, et un clic rend la main.
    //
    // `readonly` et non `disabled` : un input désactivé ne recoit AUCUN
    // événement souris, le clic serait impossible.
    // Réglages memorises de la session précédente : AVANT syncUnlimited() et
    // syncRate(), qui derivent l'affichage (champs grises, libellés) de ces
    // valeurs. Les restaurer après laisserait l'UI dans un état incoherent.
    restoreSettings();

    var unlim = $("opt-unlimited");
    var maxPages = $("max-pages");

    function lockMaxPages() {
      // Memorise la dernière valeur saisie pour la restituer au declic.
      if (maxPages.type === "number" && maxPages.value) {
        maxPages.dataset.last = maxPages.value;
      }
      maxPages.type = "text";                 // "number" n'accepte pas de texte
      maxPages.value = "∞ (max " + MAX_PAGES_HARD_LABEL + ")";
      maxPages.readOnly = true;
      maxPages.classList.add("is-locked");
      maxPages.title =
        "Tout le site : aucune limite de pages. Un garde-fou interne arrete le " +
        "crawl a " + MAX_PAGES_HARD_LABEL + " pages (protection contre les sites " +
        "a facettes ou a calendrier, ou le nombre d'URLs est quasi infini).\n" +
        "Cliquez pour fixer vous-même une limite.";
    }

    function unlockMaxPages() {
      maxPages.type = "number";
      maxPages.value = maxPages.dataset.last || "500";
      maxPages.readOnly = false;
      maxPages.classList.remove("is-locked");
      maxPages.title = "Nombre maximal de pages à crawler.";
    }

    function syncUnlimited() {
      if (unlim.checked) lockMaxPages(); else unlockMaxPages();
    }

    // Clic sur le champ verrouille : on decoche "Tout le site" et on rend la
    // main, curseur prêt a saisir.
    maxPages.addEventListener("mousedown", function (e) {
      if (!unlim.checked) return;
      e.preventDefault();
      unlim.checked = false;
      syncUnlimited();
      // Decochage PAR CODE : aucun événement `change` n'est emis, donc le
      // listener de persistance ne se declenche pas tout seul.
      persistSettings();
      maxPages.focus();
      maxPages.select();
    });

    unlim.addEventListener("change", syncUnlimited);
    syncUnlimited();

    // Cases "Débit illimité" / "Débit aléatoire" : les deux pilotent le même
    // réglage (la cadence) et s'excluent mutuellement.
    //   - illimité  : plus de plafond du tout -> "Req/sec" ET "Débit aléatoire"
    //                 n'ont plus de sens, on les grise ;
    //   - aléatoire : délai tire au sort par page -> "Req/sec" est ignore.
    var randRps = $("random-rps");
    var noLimit = $("no-rate-limit");
    var forceChrome = $("force-chrome");
    function syncRate() {
      var free = noLimit.checked;
      randRps.disabled = free;
      $("rps").disabled = free || randRps.checked;
      // "Débit illimité" ne tient pas sa promesse avec "Forcer Chrome" : les
      // mesures Chrome sont SERIALISEES (un seul navigateur derriere un Mutex,
      // tenu pendant une tempo fixe de 1,5 s par page). Le débit plafonnerait a
      // ~0,66 page/s quel que soit le nombre de threads.
      noLimit.disabled = randRps.checked || forceChrome.checked;
      // On ne DECOCHE que pour Chrome (ou l'illimité est intenable, cf. verrou
      // serialise). Decocher aussi sur "Débit aléatoire" ferait perdre le
      // réglage silencieusement : cocher puis decocher "aléatoire" laisserait
      // l'utilisateur persuade d'être en illimité alors qu'il serait retombe a
      // `rps`.
      if (forceChrome.checked) noLimit.checked = false;
    }
    randRps.addEventListener("change", syncRate);
    noLimit.addEventListener("change", syncRate);
    forceChrome.addEventListener("change", syncRate);
    syncRate();

    // Sauvegarde à chaque changement. Branche APRÈS syncUnlimited/syncRate :
    // sur un `change`, les listeners s'executent dans l'ordre d'ajout, donc la
    // persistance voit l'état DÉJÀ reconcilie (ex. "Débit illimité" decoche par
    // syncRate quand on coche "Forcer Chrome").
    watchSettings();

    initSaveDir();   // #E : champ dossier des projets
    initColResize();   // #F : colonnes redimensionnables
    initDetailResize();   // panneau de detail redimensionnable (hauteur persistee)
    initExternalLinks();  // liens d'URL -> navigateur par defaut du système
    initMultiSelect();    // clic / Shift+clic / Ctrl+clic -> copie des adresses

    // Volet de réglages repliable.
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

    // Tri par clic sur les en-têtes de la grille (neutre->desc->asc->neutre).
    $("grid-head").addEventListener("click", function (e) {
      if (S._suppressSort || (e.target.closest && e.target.closest(".col-resizer"))) return;  // #F
      var th = e.target.closest("th[data-key]");
      if (!th) return;
      var key = th.getAttribute("data-key");
      if (S.sort.key !== key) { S.sort.key = key; S.sort.dir = "desc"; }
      else if (S.sort.dir === "desc") { S.sort.dir = "asc"; }
      else { S.sort.key = null; S.sort.dir = null; }
      renderGrid();
    });

    // Sélection d'une ligne -> panneau de detail (delegation sur le tbody).
    $("grid-body").addEventListener("click", function (e) {
      var tr = e.target.closest("tr[data-url]");
      if (!tr) return;
      var url = tr.getAttribute("data-url");
      var key = normUrl(url || "");
      var td = e.target.closest("td");
      var col = td ? td.cellIndex : 0;
      hideCtxMenu();

      // --- Sélection de cellules, sur TOUS les onglets et dans la COLONNE
      //     cliquee. Shift et Ctrl n'ouvrent PAS le detail : ce sont des
      //     gestes de sélection, pas de consultation.
      if (e.shiftKey) {
        selRange(col, S.sel.anchor || key, key);
        applySelClasses();
        updateSelBar();
        return;
      }
      if (e.ctrlKey || e.metaKey) {
        // Changer de colonne repart de zero : une sélection ne mélange jamais
        // deux colonnes, sinon la copie n'aurait plus de sens.
        if (S.sel.col !== col) { S.sel.col = col; S.sel.keys = {}; }
        if (S.sel.keys[key]) delete S.sel.keys[key]; else S.sel.keys[key] = true;
        S.sel.anchor = key;
        applySelClasses();
        updateSelBar();
        return;
      }
      // Clic simple : cette cellule devient la sélection ET l'ancre. Ctrl+C
      // copie donc sa valeur même sans autre geste.
      selOne(col, key);

      // L'onglet "Liens" ne represente pas une page unique : pas de detail.
      if (TABS[S.activeTab].kind !== "pages") return;
      // D) Sur l'onglet "Données structurees" (schema), ou si on clique sur un
      // badge de type, on ouvre directement le sous-onglet JSON-LD brut.
      if (S.activeTab === "schema" || e.target.closest(".badge--schema")) {
        S.detailTab = "schema";
      }
      // Sur l'onglet "Images", on ouvre directement la liste des images sans
      // alt : c'est ce qu'on vient chercher en cliquant une ligne de cet onglet.
      if (S.activeTab === "images") {
        S.detailTab = "images";
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

    // Recherche : filtre l'onglet actif (rendu immédiat).
    $("grid-search").addEventListener("input", function () {
      S.search = this.value.trim();
      renderGrid();
    });

    // Filtres de sévérité (onglet Problèmes).
    $("sev-filters").addEventListener("click", function (e) {
      var b = e.target.closest(".sev-filter");
      if (!b) return;
      S.sevFilter = b.getAttribute("data-sev");
      document.querySelectorAll(".sev-filter").forEach(function (x) { x.classList.toggle("is-active", x === b); });
      renderGrid();
    });
    // Sidebar Overview : clic sur une catégorie -> filtre la grille.
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

    // Retirer le filtre de catégorie depuis l'indicateur.
    $("active-filter-clear").addEventListener("click", clearCategoryFilter);
  }

  // =========================================================================
  // INITIALISATION
  // =========================================================================
  setupControls();
  updateButtons();        // VAGUE 4 (C) : état initial coherent des boutons
  loadNetInfo();
  loadHistory();
  renderTabCounts();
  renderOverview();
  renderGrid();
  setActiveTab("internal");

})();
