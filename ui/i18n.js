/**
 * SEO Spider — moteur de traduction de l'interface (vanilla, aucun import).
 *
 * Chargé APRÈS `ui/locales/*.js` (qui remplissent `window.SEO_SPIDER_LOCALES`)
 * et AVANT `main.js`. Expose `window.I18N`.
 *
 * Choix de la langue :
 *   1. le choix de l'utilisateur, s'il en a fait un (localStorage `seoSpiderLang`) ;
 *   2. sinon la langue de Windows, si elle est traduite — WebView2 expose les
 *      langues préférées du système dans `navigator.languages` ;
 *   3. sinon l'anglais.
 *
 * Une clé absente d'une langue retombe sur l'anglais, puis sur le français
 * (langue d'origine de l'application), puis sur la clé elle-même.
 *
 * Marquage du HTML, retraduit par `I18N.apply` :
 *   data-i18n="clé"              -> textContent
 *   data-i18n-title="clé"        -> title
 *   data-i18n-placeholder="clé"  -> placeholder
 *   data-i18n-aria-label="clé"   -> aria-label
 *   data-i18n-tpl="clé"          -> phrase à CASES (voir `renderTemplate`)
 *
 * Construit aussi le sélecteur de langue (#lang-menu, à côté de « Réglages »).
 */
(function () {
  "use strict";

  var LOCALES = window.SEO_SPIDER_LOCALES || {};
  var FALLBACK = "en";
  var SOURCE = "fr";
  var STORAGE_KEY = "seoSpiderLang"; // "auto" ou un code de langue
  var FLAGS_DIR = "flags/"; // ui/flags/<code>.svg : Windows n'affiche pas les émojis de drapeaux

  var lang = FALLBACK;
  var numberFormat = null; // format sans option, mémorisé : `fmtNumber` est appelée par cellule de grille
  var pluralRules = {}; // code de langue -> Intl.PluralRules, mémorisées (voir `rulesFor`)
  var sessionChoice = null; // choix fait pendant cette session (voir `choice`)
  var listeners = [];

  function own(obj, key) {
    return Object.prototype.hasOwnProperty.call(obj, key);
  }

  function has(code) {
    return own(LOCALES, code);
  }

  /**
   * Règles de pluriel d'une langue. Celles de la langue qui FOURNIT le texte,
   * pas forcément de la langue affichée : une clé absente du japonais retombe
   * sur l'anglais, qui doit dire « 1 cell » et non « 1 cells » (le japonais n'a
   * qu'une forme).
   */
  function rulesFor(code) {
    if (!own(pluralRules, code)) {
      try {
        pluralRules[code] = new Intl.PluralRules(code);
      } catch (e) {
        pluralRules[code] = new Intl.PluralRules(FALLBACK);
      }
    }
    return pluralRules[code];
  }

  /** Langue du système parmi celles qui sont traduites ; anglais sinon. */
  function detect() {
    var wanted = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language];
    for (var i = 0; i < wanted.length; i++) {
      var loc;
      try {
        // maximize() complète la région : "pt" -> pt-Latn-BR, "es-419" -> es-Latn-419.
        loc = new Intl.Locale(wanted[i]).maximize();
      } catch (e) {
        continue;
      }
      if (loc.language === "pt") {
        // Deux portugais traduits : celui de la région demandée, sinon l'autre
        // (plus proche que l'anglais).
        var pt = loc.region === "BR" ? "pt-BR" : "pt-PT";
        if (has(pt)) return pt;
        var autre = pt === "pt-BR" ? "pt-PT" : "pt-BR";
        if (has(autre)) return autre;
        continue;
      }
      if (has(loc.language)) return loc.language;
    }
    return FALLBACK;
  }

  /**
   * Choix : "auto" ou un code de langue traduite. Celui fait pendant la session
   * prime ; sinon celui que le stockage a gardé. Si le stockage refuse
   * l'écriture (désactivé, plein), le choix vaut donc au moins jusqu'à la
   * fermeture.
   */
  function choice() {
    var c = sessionChoice;
    if (c === null) {
      try {
        c = localStorage.getItem(STORAGE_KEY);
      } catch (e) {}
    }
    return c && c !== "auto" && has(c) ? c : "auto";
  }

  /** Valeur d'une clé dans une langue ; seules les vraies entrées comptent (pas « toString »). */
  function lookup(code, key) {
    return has(code) && own(LOCALES[code], key) ? LOCALES[code][key] : undefined;
  }

  /** Nom d'une langue écrit dans cette langue (« Deutsch », « Polski »). */
  function nameOf(code) {
    return (has(code) && own(LOCALES[code], "_name") && LOCALES[code]._name) || code;
  }

  /**
   * Valeur traduite AVANT remplacement des `{variables}` : repli langue -> en
   * -> fr -> clé ; une valeur à formes plurielles est choisie d'après `count`,
   * avec les règles de la langue qui a fourni la valeur.
   */
  function raw(key, count) {
    var from = lang;
    var s = lookup(lang, key);
    if (s === undefined) {
      from = FALLBACK;
      s = lookup(FALLBACK, key);
    }
    if (s === undefined) {
      from = SOURCE;
      s = lookup(SOURCE, key);
    }
    if (s === undefined) return key;
    if (s && typeof s === "object") {
      var forme = rulesFor(from).select(Number(count != null ? count : 0));
      s = own(s, forme) ? s[forme] : own(s, "other") ? s.other : "";
    }
    return String(s);
  }

  /**
   * Texte traduit.
   * - `{nom}` est remplacé par `params.nom` ; un paramètre absent, `null` ou
   *   `undefined` laisse `{nom}` tel quel (jamais « null » ni « undefined »).
   * - Une valeur à formes plurielles (`{ one, few, many, other }`) est choisie
   *   par `Intl.PluralRules` d'après `params.count` (le nombre BRUT), repli sur
   *   `other`.
   */
  function t(key, params) {
    var s = raw(key, params ? params.count : null);
    if (params) {
      s = s.replace(/\{(\w+)\}/g, function (m, k) {
        return own(params, k) && params[k] != null ? String(params[k]) : m;
      });
    }
    return s;
  }

  /** `fn` sur la racine (si elle correspond) puis sur ses descendants qui correspondent. */
  function each(root, selector, fn) {
    if (root.nodeType === 1 && root.matches && root.matches(selector)) fn(root);
    var list = root.querySelectorAll(selector);
    for (var i = 0; i < list.length; i++) fn(list[i]);
  }

  // ---------------------------------------------------------------------------
  // Phrases à CASES
  // ---------------------------------------------------------------------------
  //
  // Une phrase qui entoure des valeurs écrites par main.js ne peut pas être
  // traduite par `textContent` (ce qui détruirait les valeurs), ni découpée en
  // morceaux (l'ordre des mots change d'une langue à l'autre). On la marque :
  //
  //   <span data-i18n-tpl="statusbar.pages">Pages : <strong id="status-done"
  //     data-i18n-slot="done">0</strong> / <span id="status-total"
  //     data-i18n-slot="total">0</span></span>
  //
  // avec la valeur "Pages : {done} / {total}". Les cases sont les enfants
  // DIRECTS marqués `data-i18n-slot` (le nom « count » est réservé). Elles sont
  // DÉPLACÉES à leur place dans la phrase traduite, jamais recréées ni
  // réécrites : leur id, leur contenu et leurs écouteurs survivent, et main.js
  // continue d'y écrire avec `setText("status-done", …)`. Un enfant qui n'est
  // pas une case est remplacé par le texte de la phrase.
  //
  // `data-i18n-count="n"` : la case {n} contient un nombre (écrit par main.js)
  // qui choisit la forme plurielle ; la phrase se recalcule d'elle-même quand
  // ce nombre change.

  /** Nombre affiché dans une case (« 1 234 », « 1,234 », « 12 ») ; 0 si vide. */
  function countOf(slot) {
    var digits = String(slot.textContent || "").replace(/\D+/g, "");
    return digits ? Number(digits) : 0;
  }

  /** Remplace les enfants de `el` par `text`, chaque {case} recevant son élément. */
  function fill(el, text, slots) {
    var frag = document.createDocumentFragment();
    var used = {};
    var re = /\{(\w+)\}/g;
    var last = 0;
    var m;
    while ((m = re.exec(text)) !== null) {
      if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
      var name = m[1];
      if (Object.prototype.hasOwnProperty.call(slots, name) && !used[name]) {
        used[name] = true;
        frag.appendChild(slots[name]);
      } else {
        frag.appendChild(document.createTextNode(m[0])); // même règle que t() : variable inconnue laissée visible
      }
      last = re.lastIndex;
    }
    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
    // Une case oubliée par une traduction reste dans le document : main.js y écrit.
    for (var k in slots) {
      if (Object.prototype.hasOwnProperty.call(slots, k) && !used[k]) frag.appendChild(slots[k]);
    }
    while (el.firstChild) el.removeChild(el.firstChild);
    el.appendChild(frag);
  }

  var countObserver = null;

  /** Recalcule la phrase quand main.js change le nombre de sa case de comptage. */
  function watchCount(slot) {
    if (slot.__i18nWatched || typeof MutationObserver !== "function") return;
    slot.__i18nWatched = true;
    if (!countObserver) {
      countObserver = new MutationObserver(function (records) {
        var seen = [];
        for (var i = 0; i < records.length; i++) {
          var node = records[i].target;
          while (node && !(node.nodeType === 1 && node.hasAttribute("data-i18n-slot"))) node = node.parentNode;
          var tpl = node && node.parentNode;
          if (tpl && tpl.nodeType === 1 && tpl.hasAttribute("data-i18n-tpl") && seen.indexOf(tpl) < 0) {
            seen.push(tpl);
            renderTemplate(tpl);
          }
        }
      });
    }
    countObserver.observe(slot, { childList: true, characterData: true, subtree: true });
  }

  function renderTemplate(el) {
    var slots = {};
    for (var c = el.firstElementChild; c; c = c.nextElementSibling) {
      var name = c.getAttribute("data-i18n-slot");
      if (name) slots[name] = c;
    }
    var countName = el.getAttribute("data-i18n-count");
    var countSlot = countName && Object.prototype.hasOwnProperty.call(slots, countName) ? slots[countName] : null;
    if (countSlot) watchCount(countSlot);
    var text = raw(el.getAttribute("data-i18n-tpl"), countSlot ? countOf(countSlot) : 0);
    if (el.__i18nText === text) return; // phrase déjà en place : rien à déplacer
    el.__i18nText = text;
    fill(el, text, slots);
  }

  /**
   * (Re)traduit les éléments marqués d'un sous-arbre, racine comprise :
   *   data-i18n="clé"              -> textContent
   *   data-i18n-title="clé"        -> title
   *   data-i18n-placeholder="clé"  -> placeholder
   *   data-i18n-aria-label="clé"   -> aria-label
   *   data-i18n-tpl="clé"          -> phrase à cases (voir plus haut)
   */
  function apply(root) {
    root = root || document;
    each(root, "[data-i18n]", function (el) {
      el.textContent = t(el.getAttribute("data-i18n"));
    });
    each(root, "[data-i18n-title]", function (el) {
      el.title = t(el.getAttribute("data-i18n-title"));
    });
    each(root, "[data-i18n-placeholder]", function (el) {
      el.placeholder = t(el.getAttribute("data-i18n-placeholder"));
    });
    each(root, "[data-i18n-aria-label]", function (el) {
      el.setAttribute("aria-label", t(el.getAttribute("data-i18n-aria-label")));
    });
    each(root, "[data-i18n-tpl]", renderTemplate);
  }

  // ---------------------------------------------------------------------------
  // Sélecteur de langue (#lang-menu, à côté de « Réglages »)
  // ---------------------------------------------------------------------------
  //
  // Même allure que les menus « Export en masse » / « Rapports » (classes
  // .menu-list / .menu-item), mais PAS la classe `.menu` : main.js referme les
  // `.menu.is-open` au clic extérieur sans prévenir personne, ce qui laisserait
  // aria-expanded faux. Le sélecteur se referme donc lui-même, et referme les
  // menus de main.js quand il s'ouvre (un seul menu ouvert à la fois).

  var picker = null; // { root, btn, flag, name, list, open }

  function setFlag(img, code) {
    // Drapeau absent (langue ajoutée sans son SVG) : place vide plutôt qu'une icône cassée.
    img.onerror = function () {
      img.style.visibility = "hidden";
    };
    img.style.visibility = "";
    img.src = FLAGS_DIR + code + ".svg";
  }

  function flagImg(code) {
    var img = document.createElement("img");
    img.className = "lang-flag";
    img.alt = "";
    img.width = 20;
    img.height = 15;
    img.draggable = false;
    setFlag(img, code);
    return img;
  }

  /** Icône d'« Automatique » : un écran, pour « comme le système ». */
  function autoIcon() {
    var NS = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(NS, "svg");
    svg.setAttribute("class", "lang-auto-icon");
    svg.setAttribute("viewBox", "0 0 20 15");
    svg.setAttribute("width", "20");
    svg.setAttribute("height", "15");
    svg.setAttribute("aria-hidden", "true");
    var path = document.createElementNS(NS, "path");
    path.setAttribute("d", "M3.5 1.5h13v8.5h-13z M10 10v2.6 M6.5 13.2h7");
    path.setAttribute("fill", "none");
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "1.3");
    path.setAttribute("stroke-linecap", "round");
    path.setAttribute("stroke-linejoin", "round");
    svg.appendChild(path);
    return svg;
  }

  /** Langues traduites, par ordre alphabétique de leur nom (latin, puis cyrillique, puis japonais). */
  function sortedLangs() {
    var codes = Object.keys(LOCALES);
    var coll = null;
    try {
      coll = new Intl.Collator("en", { sensitivity: "base" });
    } catch (e) {}
    return codes.sort(function (a, b) {
      var na = nameOf(a);
      var nb = nameOf(b);
      return coll ? coll.compare(na, nb) : na < nb ? -1 : na > nb ? 1 : 0;
    });
  }

  function makeItem(code, checked) {
    var b = document.createElement("button");
    b.type = "button";
    b.className = "menu-item lang-item";
    b.setAttribute("role", "menuitemradio");
    b.setAttribute("aria-checked", checked ? "true" : "false");
    b.setAttribute("data-lang", code);
    b.tabIndex = -1;
    var label = document.createElement("span");
    label.className = "lang-label";
    if (code === "auto") {
      var detected = detect();
      var hint = document.createElement("span");
      hint.className = "lang-hint";
      hint.lang = detected;
      hint.textContent = nameOf(detected);
      b.appendChild(autoIcon());
      fill(label, raw("lang.auto"), { name: hint });
      b.title = t("lang.auto_title");
    } else {
      b.appendChild(flagImg(code));
      label.lang = code;
      label.textContent = nameOf(code);
    }
    b.appendChild(label);
    var check = document.createElement("span");
    check.className = "lang-check";
    check.setAttribute("aria-hidden", "true");
    check.textContent = "✓";
    b.appendChild(check);
    return b;
  }

  function buildItems() {
    var list = picker.list;
    while (list.firstChild) list.removeChild(list.firstChild);
    var current = choice();
    list.appendChild(makeItem("auto", current === "auto"));
    var sep = document.createElement("div");
    sep.className = "lang-sep";
    sep.setAttribute("role", "separator");
    list.appendChild(sep);
    var codes = sortedLangs();
    for (var i = 0; i < codes.length; i++) list.appendChild(makeItem(codes[i], current === codes[i]));
  }

  function items() {
    return Array.prototype.slice.call(picker.list.querySelectorAll(".lang-item"));
  }

  function checkedIndex() {
    var list = items();
    for (var i = 0; i < list.length; i++) {
      if (list[i].getAttribute("aria-checked") === "true") return i;
    }
    return 0;
  }

  function focusItem(i) {
    var list = items();
    if (list.length) list[Math.max(0, Math.min(i, list.length - 1))].focus();
  }

  function openPicker(focusCurrent) {
    if (picker.open) return;
    buildItems();
    var others = document.querySelectorAll(".menu.is-open");
    for (var i = 0; i < others.length; i++) others[i].classList.remove("is-open");
    picker.open = true;
    picker.root.classList.add("is-open");
    picker.btn.setAttribute("aria-expanded", "true");
    // Calée à droite du bouton comme les autres menus ; mais si la barre du haut
    // est passée sur deux lignes, le bouton peut se retrouver tout à gauche :
    // la liste s'aligne alors sur son bord gauche pour ne pas sortir de la fenêtre.
    picker.list.style.left = "";
    picker.list.style.right = "";
    if (picker.list.getBoundingClientRect().left < 4) {
      picker.list.style.left = "0";
      picker.list.style.right = "auto";
    }
    // Jamais plus bas que la fenêtre : la liste défile si la fenêtre est basse.
    var top = picker.list.getBoundingClientRect().top;
    picker.list.style.maxHeight = Math.max(120, Math.floor(window.innerHeight - top - 8)) + "px";
    if (focusCurrent) focusItem(checkedIndex());
  }

  function closePicker(returnFocus) {
    if (!picker || !picker.open) return;
    picker.open = false;
    picker.root.classList.remove("is-open");
    picker.btn.setAttribute("aria-expanded", "false");
    if (returnFocus) picker.btn.focus();
  }

  function choose(code) {
    closePicker(true);
    if (code !== choice()) setLang(code);
  }

  /** Initiale sans accent ni casse, pour atteindre une langue en tapant sa première lettre. */
  function initial(s) {
    s = String(s || "").trim();
    if (s.normalize) s = s.normalize("NFD").replace(/\p{Mn}/gu, "");
    return s.charAt(0).toLowerCase();
  }

  function initPicker() {
    var root = document.getElementById("lang-menu");
    if (!root) return;
    picker = {
      root: root,
      btn: document.getElementById("lang-btn"),
      flag: document.getElementById("lang-flag"),
      name: document.getElementById("lang-name"),
      list: document.getElementById("lang-list"),
      open: false
    };
    if (!picker.btn || !picker.flag || !picker.name || !picker.list) {
      picker = null;
      return;
    }

    picker.btn.addEventListener("click", function (e) {
      if (picker.open) closePicker(false);
      else openPicker(e.detail === 0); // detail 0 = Entrée / Espace : le focus entre dans la liste
    });

    picker.btn.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        if (!picker.open) openPicker(false);
        focusItem(e.key === "ArrowUp" ? items().length - 1 : checkedIndex());
      } else if (e.key === "Escape" && picker.open) {
        // stopPropagation : main.js viderait sinon la sélection de cellules (Échap sur le document).
        e.preventDefault();
        e.stopPropagation();
        closePicker(true);
      }
    });

    picker.list.addEventListener("click", function (e) {
      var it = e.target.closest ? e.target.closest(".lang-item") : null;
      if (it) choose(it.getAttribute("data-lang"));
    });

    picker.list.addEventListener("keydown", function (e) {
      var list = items();
      var i = list.indexOf(document.activeElement);
      if (e.key === "ArrowDown") {
        e.preventDefault();
        focusItem(i < 0 ? 0 : (i + 1) % list.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        focusItem(i < 0 ? list.length - 1 : (i - 1 + list.length) % list.length);
      } else if (e.key === "Home" || e.key === "PageUp") {
        e.preventDefault();
        focusItem(0);
      } else if (e.key === "End" || e.key === "PageDown") {
        e.preventDefault();
        focusItem(list.length - 1);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        closePicker(true);
      } else if (e.key === "Tab") {
        closePicker(false); // le focus continue son chemin normal
      } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        var want = initial(e.key);
        for (var step = 1; step <= list.length; step++) {
          var j = (Math.max(i, 0) + step) % list.length;
          var label = list[j].querySelector(".lang-label");
          if (label && initial(label.textContent) === want) {
            e.preventDefault();
            focusItem(j);
            break;
          }
        }
      }
    });

    // Clic ailleurs : en phase de CAPTURE, car les boutons des menus de main.js
    // arrêtent la propagation de leur clic.
    document.addEventListener(
      "click",
      function (e) {
        if (picker.open && !picker.root.contains(e.target)) closePicker(false);
      },
      true
    );
    picker.root.addEventListener("focusout", function (e) {
      if (picker.open && e.relatedTarget && !picker.root.contains(e.relatedTarget)) closePicker(false);
    });
    window.addEventListener("resize", function () {
      closePicker(false);
    });
    window.addEventListener("blur", function () {
      closePicker(false);
    });

    // La liste est reconstruite à chaque ouverture : drapeaux chargés d'avance
    // (quelques Ko), pour qu'ils ne s'affichent pas en retard la première fois.
    setTimeout(function () {
      var codes = Object.keys(LOCALES);
      for (var i = 0; i < codes.length; i++) new Image().src = FLAGS_DIR + codes[i] + ".svg";
    }, 0);
  }

  /** Bouton : drapeau et nom de la langue affichée, écrit dans cette langue. */
  function updatePicker() {
    if (!picker) return;
    setFlag(picker.flag, lang);
    picker.name.lang = lang;
    picker.name.textContent = nameOf(lang);
    if (picker.open) buildItems();
  }

  // ---------------------------------------------------------------------------

  /** Active la langue voulue, retraduit le document, prévient les abonnés. */
  function activate() {
    var c = choice();
    lang = c !== "auto" ? c : detect();
    try {
      numberFormat = new Intl.NumberFormat(lang);
    } catch (e) {
      numberFormat = new Intl.NumberFormat(FALLBACK);
    }
    document.documentElement.lang = lang;
    apply(document);
    updatePicker();
    for (var i = 0; i < listeners.length; i++) {
      try {
        listeners[i](lang);
      } catch (e) {
        console.error("I18N listener:", e);
      }
    }
  }

  /** Change de langue : un code traduit, ou "auto" pour suivre Windows. */
  function setLang(code) {
    sessionChoice = code && code !== "auto" && has(code) ? code : "auto";
    try {
      localStorage.setItem(STORAGE_KEY, sessionChoice);
    } catch (e) {}
    activate();
  }

  /** Nombre au format de la langue de l'interface (options de Intl.NumberFormat). */
  function fmtNumber(n, options) {
    try {
      return options ? new Intl.NumberFormat(lang, options).format(n) : numberFormat.format(n);
    } catch (e) {
      return String(n);
    }
  }

  window.I18N = {
    t: t,
    apply: apply,
    setLang: setLang,
    detect: detect,
    fmtNumber: fmtNumber,
    /** Abonne `fn(lang)` aux changements de langue (appelée APRÈS la retraduction du document). */
    onChange: function (fn) {
      listeners.push(fn);
    },
    /** Codes des langues traduites. */
    langs: function () {
      return Object.keys(LOCALES);
    },
    /** Nom d'une langue écrit dans cette langue (« Deutsch », « Polski »). */
    nameOf: nameOf,
    /** Langue affichée en ce moment. */
    get lang() {
      return lang;
    },
    /** Choix enregistré : "auto" ou un code. */
    get choice() {
      return choice();
    }
  };

  initPicker();
  activate();
})();
