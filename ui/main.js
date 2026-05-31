/**
 * Crawl Audit — Frontend vanilla (Tauri v2, withGlobalTauri = true)
 * Acces Tauri : window.__TAURI__.core.invoke / window.__TAURI__.event.listen
 *
 * Fonctions : crawl, tri des colonnes, IP+reverse DNS, multi-run (repetitions),
 * tableau de bord agrege, user-agent aleatoire, historique persistant.
 */

(function () {
  "use strict";

  // -------------------------------------------------------------------------
  // Utilitaires
  // -------------------------------------------------------------------------
  function fmtBytes(bytes) {
    if (!bytes || bytes < 0) return "0 o";
    if (bytes < 1024) return bytes + " o";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " Ko";
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + " Mo";
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " Go";
  }
  function truncUrl(url, maxLen) {
    maxLen = maxLen || 60;
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
  function fmtDate(iso) {
    if (!iso) return "—";
    try {
      return new Date(iso).toLocaleString("fr-FR", { dateStyle: "short", timeStyle: "short" });
    } catch (e) {
      return iso;
    }
  }

  // -------------------------------------------------------------------------
  // References DOM
  // -------------------------------------------------------------------------
  var form         = document.getElementById("crawl-form");
  var btnStart     = document.getElementById("btn-start");
  var btnStop      = document.getElementById("btn-stop");
  var progressSec  = document.getElementById("progress-section");
  var progressBar  = document.getElementById("progress-bar");
  var progressText = document.getElementById("progress-text");
  var resultsBody  = document.getElementById("results-body");

  var totPages     = document.getElementById("tot-pages");
  var totBandwidth = document.getElementById("tot-bandwidth");
  var totDynamic   = document.getElementById("tot-dynamic");
  var totMiss      = document.getElementById("tot-miss");
  var totTechs     = document.getElementById("tot-techs");

  var multiSection = document.getElementById("multi-section");
  var mRuns        = document.getElementById("m-runs");
  var mBandwidth   = document.getElementById("m-bandwidth");
  var mPages       = document.getElementById("m-pages");
  var mAvg         = document.getElementById("m-avg");

  var historyBody  = document.getElementById("history-body");

  // -------------------------------------------------------------------------
  // Etat
  // -------------------------------------------------------------------------
  // Run courant (alimente tableau + tuiles "totaux").
  var state = {
    pages: 0, bandwidth: 0, dynamic: 0, miss: 0,
    techs: {}, maxWeight: 0,
    rows: [],
    sort: { key: null, dir: null },
    unlisteners: []
  };

  // Agregat sur tous les runs d'une sequence (multi-run).
  var multi = {
    target: 0, done: 0,
    totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
    stopRequested: false, running: false,
    config: {}, lastTechs: [], lastRows: []
  };

  // Historique persistant (liste de sequences).
  var history = [];

  function resetRunState() {
    state.pages = 0; state.bandwidth = 0; state.dynamic = 0; state.miss = 0;
    state.techs = {}; state.maxWeight = 0; state.rows = [];
    // state.sort conserve.
  }

  function updateTotals() {
    totPages.textContent     = state.pages;
    totBandwidth.textContent = fmtBytes(state.bandwidth);
    totDynamic.textContent   = state.dynamic;
    totMiss.textContent      = state.miss;
    var techNames = Object.keys(state.techs);
    if (techNames.length === 0) {
      totTechs.textContent = "—";
    } else {
      techNames.sort(function (a, b) { return state.techs[b] - state.techs[a]; });
      totTechs.textContent = techNames.join(", ");
    }
  }

  function updateMultiDashboard() {
    mRuns.textContent      = multi.done + " / " + multi.target;
    mBandwidth.textContent = fmtBytes(multi.totalBytes);
    mPages.textContent     = multi.totalPages;
    var avg = multi.done > 0 ? Math.round(multi.totalBytes / multi.done) : 0;
    mAvg.textContent       = fmtBytes(avg);
  }

  // -------------------------------------------------------------------------
  // Tableau des resultats (run courant)
  // -------------------------------------------------------------------------
  function clearEmptyRow() {
    var emptyRow = resultsBody.querySelector(".empty-row");
    if (emptyRow) emptyRow.remove();
  }

  function refreshWeightBars() {
    resultsBody.querySelectorAll("tr[data-weight]").forEach(function (row) {
      var w = parseInt(row.getAttribute("data-weight"), 10) || 0;
      var pct = state.maxWeight > 0 ? Math.round((w / state.maxWeight) * 100) : 0;
      var bar = row.querySelector(".weight-bar-fill");
      if (bar) bar.style.width = pct + "%";
    });
  }

  function normalizePayload(p) {
    var techList = Array.isArray(p.tech) ? p.tech : [];
    var images = Array.isArray(p.images) ? p.images : [];
    var imgBytes = images.reduce(function (s, img) {
      return s + (img && img.bytes ? img.bytes : 0);
    }, 0);
    return {
      url:       p.url || "",
      techList:  techList,
      techno:    techList.length ? techList.join(", ") : "—",
      weight:    p.total_bytes || 0,
      imgCount:  images.length,
      imgBytes:  imgBytes,
      reqCount:  p.request_count || 0,
      dynReq:    p.dynamic_requests || 0,
      cacheMiss: p.vercel_cache_misses || 0,
      probe:     p.measured_with || ""
    };
  }

  function buildRowElement(d) {
    var tr = document.createElement("tr");
    tr.setAttribute("data-weight", d.weight);

    var tdUrl = document.createElement("td");
    tdUrl.className = "col-url";
    var link = document.createElement("a");
    link.href = d.url; link.target = "_blank"; link.rel = "noopener noreferrer";
    link.title = d.url; link.textContent = truncUrl(d.url);
    tdUrl.appendChild(link);
    tr.appendChild(tdUrl);

    var tdTechno = document.createElement("td");
    tdTechno.className = "col-techno";
    if (d.techno !== "—") {
      var badge = document.createElement("span");
      badge.className = "techno-badge";
      badge.textContent = d.techno;
      tdTechno.appendChild(badge);
    } else { tdTechno.textContent = "—"; }
    tr.appendChild(tdTechno);

    var tdWeight = document.createElement("td");
    tdWeight.className = "col-weight";
    tdWeight.textContent = fmtBytes(d.weight);
    tr.appendChild(tdWeight);

    var tdImages = document.createElement("td");
    tdImages.className = "col-images";
    tdImages.textContent = d.imgCount + " / " + fmtBytes(d.imgBytes);
    tr.appendChild(tdImages);

    var tdReq = document.createElement("td");
    tdReq.className = "col-req";
    tdReq.textContent = d.reqCount;
    tr.appendChild(tdReq);

    var tdDyn = document.createElement("td");
    tdDyn.className = "col-dyn";
    if (d.dynReq > 0) {
      var dynSpan = document.createElement("span");
      dynSpan.className = "dyn-badge";
      dynSpan.textContent = d.dynReq;
      tdDyn.appendChild(dynSpan);
    } else { tdDyn.textContent = "0"; }
    tr.appendChild(tdDyn);

    var tdMiss = document.createElement("td");
    tdMiss.className = "col-miss";
    if (d.cacheMiss > 0) {
      var missSpan = document.createElement("span");
      missSpan.className = "miss-badge";
      missSpan.textContent = d.cacheMiss;
      tdMiss.appendChild(missSpan);
    } else { tdMiss.textContent = "0"; }
    tr.appendChild(tdMiss);

    var tdProbe = document.createElement("td");
    tdProbe.className = "col-probe";
    var probeSpan = document.createElement("span");
    probeSpan.className = "probe-tag probe-tag--" + (d.probe || "http");
    probeSpan.textContent = probeLabel(d.probe);
    tdProbe.appendChild(probeSpan);
    tr.appendChild(tdProbe);

    var tdBar = document.createElement("td");
    tdBar.className = "col-bar";
    var barTrack = document.createElement("div");
    barTrack.className = "weight-bar-track";
    var barFill = document.createElement("div");
    barFill.className = "weight-bar-fill";
    barFill.style.width = "0%";
    barTrack.appendChild(barFill);
    tdBar.appendChild(barTrack);
    tr.appendChild(tdBar);

    return tr;
  }

  function compareRows(a, b, key, dir) {
    var va = a[key], vb = b[key], cmp;
    if (typeof va === "number" && typeof vb === "number") {
      cmp = va - vb;
    } else {
      cmp = String(va).localeCompare(String(vb), "fr", { numeric: true, sensitivity: "base" });
    }
    return dir === "asc" ? cmp : -cmp;
  }

  function renderRows() {
    var rows = state.rows.slice();
    if (state.sort.key) {
      rows.sort(function (a, b) { return compareRows(a, b, state.sort.key, state.sort.dir); });
    }
    resultsBody.innerHTML = "";
    if (rows.length === 0) {
      resultsBody.innerHTML =
        '<tr class="empty-row"><td colspan="9">Lancez un crawl pour voir les resultats ici.</td></tr>';
      return;
    }
    rows.forEach(function (d) { resultsBody.appendChild(buildRowElement(d)); });
    refreshWeightBars();
  }

  function addPageRow(payload) {
    var d = normalizePayload(payload);
    state.rows.push(d);

    state.pages     += 1;
    state.bandwidth += d.weight;
    state.dynamic   += d.dynReq;
    state.miss      += d.cacheMiss;
    d.techList.forEach(function (t) { if (t) state.techs[t] = (state.techs[t] || 0) + 1; });
    if (d.weight > state.maxWeight) state.maxWeight = d.weight;
    updateTotals();

    if (!state.sort.key) {
      clearEmptyRow();
      var tr = buildRowElement(d);
      resultsBody.appendChild(tr);
      refreshWeightBars();
      tr.scrollIntoView({ behavior: "smooth", block: "nearest" });
    } else {
      renderRows();
    }
  }

  // -------------------------------------------------------------------------
  // Tri par clic sur les en-tetes (neutre -> decroissant -> croissant)
  // -------------------------------------------------------------------------
  function setupSorting() {
    document.querySelectorAll("th[data-sort-key]").forEach(function (th) {
      th.classList.add("sortable");
      th.addEventListener("click", function () {
        var key = th.getAttribute("data-sort-key");
        if (state.sort.key !== key) {
          state.sort.key = key; state.sort.dir = "desc";
        } else if (state.sort.dir === "desc") {
          state.sort.dir = "asc";
        } else {
          state.sort.key = null; state.sort.dir = null;
        }
        updateSortIndicators();
        renderRows();
      });
    });
  }

  function updateSortIndicators() {
    document.querySelectorAll("th[data-sort-key]").forEach(function (th) {
      th.classList.remove("sort-asc", "sort-desc", "sort-active");
      if (th.getAttribute("data-sort-key") === state.sort.key) {
        th.classList.add("sort-active");
        th.classList.add(state.sort.dir === "asc" ? "sort-asc" : "sort-desc");
      }
    });
  }

  // -------------------------------------------------------------------------
  // Info reseau (IP + reverse DNS)
  // -------------------------------------------------------------------------
  function loadNetInfo() {
    var el  = document.getElementById("net-info");
    var txt = document.getElementById("net-info-text");
    if (!window.__TAURI__ || !el || !txt) return;
    window.__TAURI__.core.invoke("network_info")
      .then(function (info) {
        var ip  = info && info.ip ? info.ip : "?";
        var rev = info && info.reverse_dns ? info.reverse_dns : null;
        txt.textContent = rev ? (ip + " · " + rev) : ip;
        el.title = rev
          ? ("IP publique sortante " + ip + " — reverse DNS : " + rev)
          : ("IP publique sortante " + ip + " — pas de reverse DNS");
        el.classList.remove("net-err");
        el.classList.add("net-ok");
      })
      .catch(function (err) {
        txt.textContent = "IP indisponible";
        el.classList.remove("net-ok");
        el.classList.add("net-err");
        console.error("network_info error:", err);
      });
  }

  // -------------------------------------------------------------------------
  // Historique persistant (commandes Rust load_history / save_history)
  // -------------------------------------------------------------------------
  function loadHistory() {
    if (!window.__TAURI__) return;
    window.__TAURI__.core.invoke("load_history")
      .then(function (json) {
        try { history = JSON.parse(json) || []; } catch (e) { history = []; }
        if (!Array.isArray(history)) history = [];
        renderHistory();
      })
      .catch(function (err) { console.error("load_history error:", err); });
  }

  function saveHistory() {
    if (!window.__TAURI__) return;
    window.__TAURI__.core.invoke("save_history", { json: JSON.stringify(history) })
      .catch(function (err) { console.error("save_history error:", err); });
  }

  function addHistoryEntry(entry) {
    history.unshift(entry);                 // plus recent en haut
    if (history.length > 100) history = history.slice(0, 100);
    saveHistory();
    renderHistory();
  }

  function renderHistory() {
    if (!history.length) {
      historyBody.innerHTML = '<tr class="empty-row"><td colspan="7">Aucun crawl enregistré.</td></tr>';
      return;
    }
    historyBody.innerHTML = "";
    history.forEach(function (e, idx) {
      var tr = document.createElement("tr");
      tr.title = "Cliquer pour consulter ce crawl";
      tr.addEventListener("click", function () { viewHistoryEntry(idx); });

      var opts = [];
      if (e.config && e.config.random_ua) opts.push("UA aléatoire");
      if (e.config && e.config.force_chrome) opts.push("Chrome forcé");

      var cells = [
        fmtDate(e.date),
        e.url || "",
        String(e.runs || 1),
        String(e.pages || 0),
        fmtBytes(e.bytes || 0),
        (e.techs && e.techs.length ? e.techs.join(", ") : "—"),
        (opts.length ? opts.join(" · ") : "—")
      ];
      cells.forEach(function (c, i) {
        var td = document.createElement("td");
        if (i === 1) td.className = "h-url";
        td.textContent = c;
        td.title = c;
        tr.appendChild(td);
      });
      historyBody.appendChild(tr);
    });
  }

  function viewHistoryEntry(idx) {
    var e = history[idx];
    if (!e || !Array.isArray(e.rows)) return;
    state.rows = e.rows.slice();
    state.pages     = state.rows.length;
    state.bandwidth = state.rows.reduce(function (s, r) { return s + (r.weight || 0); }, 0);
    state.dynamic   = state.rows.reduce(function (s, r) { return s + (r.dynReq || 0); }, 0);
    state.miss      = state.rows.reduce(function (s, r) { return s + (r.cacheMiss || 0); }, 0);
    state.maxWeight = state.rows.reduce(function (m, r) { return Math.max(m, r.weight || 0); }, 0);
    state.techs = {};
    state.rows.forEach(function (r) {
      (r.techList || []).forEach(function (t) { if (t) state.techs[t] = (state.techs[t] || 0) + 1; });
    });
    updateTotals();
    renderRows();
    progressSec.hidden = false;
    progressBar.style.width = "100%";
    progressText.textContent = "Consultation : " + (e.url || "") + " — " + fmtDate(e.date);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  function clearHistory() {
    history = [];
    saveHistory();
    renderHistory();
  }

  // -------------------------------------------------------------------------
  // Abonnements aux evenements Tauri (une fois par sequence)
  // -------------------------------------------------------------------------
  function subscribeEvents() {
    var tauri = window.__TAURI__;

    tauri.event.listen("crawl:page", function (event) {
      addPageRow(event.payload || {});
    }).then(function (u) { state.unlisteners.push(u); });

    tauri.event.listen("crawl:progress", function (event) {
      var p = event.payload || {};
      var done = p.done || 0, queued = p.queued || 0, total = done + queued;
      var pct = total > 0 ? Math.round((done / total) * 100) : 0;
      progressBar.style.width = pct + "%";
      var runPrefix = multi.target > 1 ? ("Run " + (multi.done + 1) + "/" + multi.target + " — ") : "";
      var currentUrl = p.current_url ? truncUrl(p.current_url, 70) : "";
      progressText.textContent =
        runPrefix + done + " / " + (total || "?") + (currentUrl ? " — " + currentUrl : "");
    }).then(function (u) { state.unlisteners.push(u); });

    tauri.event.listen("crawl:done", function () {
      // Fin du run courant : on cumule dans l'agregat multi-run.
      multi.done        += 1;
      multi.totalBytes  += state.bandwidth;
      multi.totalPages  += state.pages;
      multi.totalDynamic += state.dynamic;
      multi.totalMiss   += state.miss;
      multi.lastTechs    = Object.keys(state.techs);
      multi.lastRows     = state.rows.slice();
      updateMultiDashboard();

      if (!multi.stopRequested && multi.done < multi.target) {
        startRun();                          // run suivant
      } else {
        finishSequence();                    // sequence terminee
      }
    }).then(function (u) { state.unlisteners.push(u); });
  }

  function unsubscribeEvents() {
    state.unlisteners.forEach(function (fn) { try { fn(); } catch (e) { /* ignore */ } });
    state.unlisteners = [];
  }

  // -------------------------------------------------------------------------
  // Multi-run : enchainement des runs + cloture de la sequence
  // -------------------------------------------------------------------------
  function startRun() {
    resetRunState();
    resultsBody.innerHTML =
      '<tr class="empty-row"><td colspan="9">Crawl en cours...</td></tr>';
    updateTotals();
    state.running = true;
    progressBar.style.width = "0%";
    progressText.textContent =
      (multi.target > 1 ? "Run " + (multi.done + 1) + "/" + multi.target + " — " : "") + "Démarrage...";

    window.__TAURI__.core.invoke("start_crawl", { config: multi.config })
      .catch(function (err) {
        progressText.textContent = "Erreur : " + (err || "inconnue");
        console.error("start_crawl error:", err);
        finishSequence();
      });
  }

  function finishSequence() {
    multi.running = false;
    state.running = false;
    btnStart.disabled = false;
    btnStop.disabled = true;
    progressBar.style.width = "100%";
    progressText.textContent =
      "Terminé — " + multi.done + " run(s), " + multi.totalPages + " page(s) au total.";

    // Une entree d'historique par sequence (lancement).
    addHistoryEntry({
      date: new Date().toISOString(),
      url: multi.config.seed_url || "",
      runs: multi.done,
      pages: multi.totalPages,
      bytes: multi.totalBytes,
      dynamic: multi.totalDynamic,
      miss: multi.totalMiss,
      techs: multi.lastTechs || [],
      config: {
        max_pages: multi.config.max_pages,
        max_depth: multi.config.max_depth,
        rps: multi.config.requests_per_second,
        force_chrome: multi.config.force_chrome,
        random_ua: multi.config.random_user_agent
      },
      rows: multi.lastRows || []
    });
  }

  // -------------------------------------------------------------------------
  // Lancement (formulaire)
  // -------------------------------------------------------------------------
  form.addEventListener("submit", function (evt) {
    evt.preventDefault();
    if (!window.__TAURI__) {
      alert("Interface Tauri non disponible. Lancez l'application via Tauri.");
      return;
    }

    var seedUrl     = document.getElementById("seed-url").value.trim();
    var maxPages    = parseInt(document.getElementById("max-pages").value, 10) || 100;
    var maxDepth    = parseInt(document.getElementById("max-depth").value, 10) || 3;
    var rps         = Math.max(1, Math.round(parseFloat(document.getElementById("rps").value) || 2));
    var forceChrome = document.getElementById("force-chrome").checked;
    var randomUa    = document.getElementById("random-ua").checked;
    var randomDelay = document.getElementById("random-rps").checked;
    var repetitions = Math.max(1, Math.round(parseFloat(document.getElementById("repetitions").value) || 1));

    // (Re)initialise l'agregat multi-run.
    multi = {
      target: repetitions, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: true,
      config: {
        seed_url: seedUrl,
        max_pages: maxPages,
        max_depth: maxDepth,
        same_domain_only: true,
        requests_per_second: rps,
        force_chrome: forceChrome,
        random_user_agent: randomUa,
        random_delay: randomDelay
      },
      lastTechs: [], lastRows: []
    };

    multiSection.hidden = repetitions <= 1;   // dashboard agrege seulement si multi-run
    updateMultiDashboard();

    progressSec.hidden = false;
    btnStart.disabled = true;
    btnStop.disabled = false;

    unsubscribeEvents();
    subscribeEvents();
    startRun();
  });

  // -------------------------------------------------------------------------
  // Arret
  // -------------------------------------------------------------------------
  btnStop.addEventListener("click", function () {
    if (!window.__TAURI__) return;
    multi.stopRequested = true;
    window.__TAURI__.core.invoke("stop_crawl").catch(function (err) {
      console.error("stop_crawl error:", err);
    });
    btnStop.disabled = true;
    progressText.textContent = "Arrêt demandé...";
  });

  // Reinitialise l'AFFICHAGE (tableau, totaux, dashboard, progression) et arrete
  // un crawl en cours. Ne touche PAS a l'historique persistant.
  function resetView() {
    multi.stopRequested = true;
    if (window.__TAURI__ && multi.running) {
      window.__TAURI__.core.invoke("stop_crawl").catch(function () {});
    }
    unsubscribeEvents();
    resetRunState();
    multi = {
      target: 0, done: 0,
      totalBytes: 0, totalPages: 0, totalDynamic: 0, totalMiss: 0,
      stopRequested: false, running: false,
      config: {}, lastTechs: [], lastRows: []
    };
    renderRows();
    updateTotals();
    updateMultiDashboard();
    multiSection.hidden = true;
    progressBar.style.width = "0%";
    progressText.textContent = "En attente...";
    progressSec.hidden = true;
    btnStart.disabled = false;
    btnStop.disabled = true;
  }
  document.getElementById("btn-reset").addEventListener("click", resetView);

  // -------------------------------------------------------------------------
  // Bouton de (Req/sec aleatoire) + presets de repetitions + vider historique
  // -------------------------------------------------------------------------
  // Case "aleatoire" (Req/sec) : grise le champ de debit fixe quand active.
  var randomRpsBox = document.getElementById("random-rps");
  randomRpsBox.addEventListener("change", function () {
    document.getElementById("rps").disabled = randomRpsBox.checked;
  });

  document.getElementById("rep-presets").addEventListener("click", function (evt) {
    var b = evt.target.closest("button[data-rep]");
    if (b) document.getElementById("repetitions").value = b.getAttribute("data-rep");
  });

  document.getElementById("btn-clear-history").addEventListener("click", function () {
    clearHistory();
  });

  document.getElementById("btn-open-folder").addEventListener("click", function () {
    if (!window.__TAURI__) return;
    window.__TAURI__.core.invoke("open_history_folder").catch(function (err) {
      console.error("open_history_folder error:", err);
    });
  });

  // Affiche le chemin du fichier d'historique sous le titre du panneau.
  function loadHistoryLocation() {
    if (!window.__TAURI__) return;
    var el = document.getElementById("history-path");
    if (!el) return;
    window.__TAURI__.core.invoke("history_location")
      .then(function (p) { el.textContent = p || ""; el.title = p || ""; })
      .catch(function (err) { console.error("history_location error:", err); });
  }

  // -------------------------------------------------------------------------
  // Initialisation
  // -------------------------------------------------------------------------
  setupSorting();
  loadNetInfo();
  loadHistory();
  loadHistoryLocation();

})();
