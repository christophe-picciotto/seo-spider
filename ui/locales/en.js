// Traductions de l'interface de SEO Spider — English.
// Une clé absente d'une langue retombe sur l'anglais, puis sur le français (voir ui/i18n.js).
window.SEO_SPIDER_LOCALES = window.SEO_SPIDER_LOCALES || {};
window.SEO_SPIDER_LOCALES["en"] = {
  "_name": "English",

  // ----- cles-interface.json -----
  // Titre de la fenêtre (balise <title>). C'est le nom de l'application : ne pas traduire.
  "page.title": "SEO Spider",
  // Titre du panneau latéral droit qui résume le crawl par catégories. Affiché en MAJUSCULES par la mise en page (écrire en casse normale). Vocabulaire de Screaming Frog, gardé en anglais dans l'interface française : faire de même si c'est l'usage des outils SEO dans ta langue. Place : ~20 caractères.
  "page.overview_head": "Overview",
  // Texte grisé du champ d'adresse quand il est vide : un exemple d'adresse de site à crawler. Garder une URL valide, en lettres latines ASCII seulement (https://, .com ; https://example.com convient partout) ; seul le mot « exemple » peut devenir celui de ta langue s'il s'écrit en ASCII (example, beispiel…), jamais en cyrillique ni en kanji.
  "topbar.url_placeholder": "https://example.com",
  // Bouton de la barre du haut : importe un fichier texte d'URL à crawler l'une après l'autre. Barre chargée : ~20 caractères conseillés (elle passe à la ligne s'il le faut).
  "topbar.import": "Import URL list",
  // Infobulle du bouton « Importer une liste ». Les MAJUSCULES marquent l'insistance : les garder si ta langue a des majuscules ; sinon (japonais…), souligner l'insistance autrement.
  "topbar.import_title": "Import a .txt or .csv file with one URL per line. Each site will be crawled IN FULL, one after another.",
  // Case à cocher de la barre du haut : crawler tout le site, sans limite de nombre de pages. ~15 caractères conseillés.
  "topbar.unlimited": "Entire site",
  // Infobulle de la case « Tout le site ».
  "topbar.unlimited_title": "Crawl the entire site, with no page limit",
  // Case à cocher de la barre du haut : suivre les interdictions du fichier robots.txt du site. « robots.txt » est un nom de fichier : ne pas traduire. ~22 caractères conseillés.
  "topbar.robots": "Respect robots.txt",
  // Infobulle de la case « Respecter robots.txt ».
  "topbar.robots_title": "Follow the rules in the site's robots.txt file",
  // Petit bouton qui ouvre ou ferme le volet des réglages du crawl. ~12 caractères conseillés.
  "topbar.settings": "Settings",
  // Volet des réglages : étiquette au-dessus d'un champ numérique (nombre maximal de pages à crawler). Affichée en MAJUSCULES, petite police : ~15 caractères.
  "drawer.max_pages": "Max pages",
  // Volet des réglages : étiquette au-dessus d'un champ numérique (nombre maximal de clics depuis la page de départ). Affichée en MAJUSCULES : ~15 caractères.
  "drawer.max_depth": "Max depth",
  // Volet des réglages : étiquette au-dessus d'un champ numérique = nombre maximal de requêtes HTTP par seconde. Abréviation voulue, affichée en MAJUSCULES : ~12 caractères.
  "drawer.rps": "Req / sec",
  // Infobulle du réglage « Req / sec ». « threads » = fils d'exécution parallèles (réglage voisin « Threads »). Les MAJUSCULES marquent l'insistance : les garder si ta langue a des majuscules ; sinon (japonais…), souligner l'insistance autrement.
  "drawer.rps_title": "Maximum rate: how fast requests are sent (a cap that keeps the server from being overloaded). This cap is GLOBAL: while it is active, adding threads changes nothing.",
  // Volet des réglages : étiquette au-dessus d'un champ numérique = nombre de pages téléchargées en parallèle. Terme technique souvent gardé en anglais. MAJUSCULES, ~12 caractères.
  "drawer.threads": "Threads",
  // Infobulle du réglage « Threads ». « workers » = tâches de téléchargement simultanées (même mot que dans la barre d'état). Les MAJUSCULES marquent l'insistance : les garder si ta langue a des majuscules ; sinon (japonais…), souligner l'insistance autrement.
  "drawer.threads_title": "Number of pages downloaded IN PARALLEL (concurrent workers)",
  // Case à cocher du volet des réglages : supprime la limite de requêtes par seconde. ~18 caractères.
  "drawer.no_rate_limit": "Unlimited rate",
  // Infobulle de la case « Débit illimité ». Screaming Frog = logiciel SEO concurrent (nom propre). Serveur mutualisé = hébergement partagé.
  "drawer.no_rate_limit_title": "Removes the rate cap: speed is then limited only by the threads and the server's response time (Screaming Frog behaviour). Be careful not to overload a shared server.",
  // Case à cocher du volet des réglages : attendre un délai aléatoire entre deux requêtes. ~18 caractères.
  "drawer.random_rps": "Random delays",
  // Infobulle de la case « Débit aléatoire ».
  "drawer.random_rps_title": "Random delays between requests (human-like behaviour). Cannot be combined with unlimited rate.",
  // Case à cocher du volet des réglages : mesurer chaque page avec le navigateur Chrome. « Chrome » est un nom propre. ~18 caractères.
  "drawer.force_chrome": "Force Chrome",
  // Infobulle de la case « Forcer Chrome ». Termes techniques à garder : Chrome headless, XHR, serverless, SPA, React, Vue. Les MAJUSCULES marquent l'insistance : les garder si ta langue a des majuscules ; sinon (japonais…), souligner l'insistance autrement.
  "drawer.force_chrome_title": "Measures every page with headless Chrome (REAL bandwidth: XHR, lazy-loaded images, serverless invocations). Required for React/Vue SPAs, which no longer trigger Chrome automatically — Chrome measurements run one at a time (a single browser), so they are slow on a large crawl.",
  // Case à cocher du volet des réglages : changer d'en-tête User-Agent à chaque requête. UA = User-Agent (garder l'abréviation). ~15 caractères.
  "drawer.random_ua": "Random UA",
  // Volet des réglages : étiquette du champ qui indique où les crawls sont enregistrés. Affichée en MAJUSCULES : ~22 caractères.
  "drawer.save_dir": "Projects folder",
  // Texte grisé du champ « Dossier des projets » quand il est vide : sans choix, les projets vont dans le dossier de données de l'application. Champ d'environ 260 px.
  "drawer.save_dir_placeholder": "(default: app data folder)",
  // Petit bouton qui ouvre la boîte de dialogue de choix de dossier (le mot habituel de Windows pour ce bouton dans ta langue). Garder les points de suspension « … ».
  "drawer.pick_folder": "Browse…",
  // Infobulle du bouton « Choisir… ».
  "drawer.pick_folder_title": "Choose the folder where projects are saved",
  // Petit bouton du volet des réglages : rouvrir un crawl enregistré pour le consulter. ~18 caractères.
  "drawer.open_project": "Open project",
  // Infobulle du bouton « Ouvrir un projet ». « .json » = extension de fichier, ne pas traduire.
  "drawer.open_project_title": "Open a saved project (.json)",
  // Onglet principal (pages internes du site), suivi d'un compteur. Gardé en anglais dans l'interface française (vocabulaire de Screaming Frog, connu des SEO) : faire de même si c'est l'usage dans ta langue, sinon traduire. 18 onglets côte à côte : ~15 caractères conseillés ; la barre défile s'il le faut, un mot juste vaut mieux qu'une abréviation obscure.
  "tabs.internal": "Internal",
  // Onglet principal : codes de réponse HTTP (200, 301, 404…). Gardé en anglais en français (vocabulaire de Screaming Frog) : faire de même si c'est l'usage, sinon traduire. ~15 caractères conseillés (la barre défile).
  "tabs.response": "Response Codes",
  // Onglet principal : analyse des adresses (longueur, caractères). Sigle technique, en principe identique dans toutes les langues.
  "tabs.url": "URL",
  // Onglet principal : HTTPS, en-têtes de sécurité, contenu mixte. ~15 caractères conseillés (la barre défile).
  "tabs.security": "Security",
  // Onglet principal : balises <title> des pages. Gardé en anglais en français (vocabulaire de Screaming Frog) : faire de même si c'est l'usage, sinon traduire. ~15 caractères conseillés (la barre défile).
  "tabs.titles": "Page Titles",
  // Onglet principal : balises meta description. Terme SEO souvent gardé tel quel. ~16 caractères conseillés (la barre défile).
  "tabs.meta": "Meta Description",
  // Onglet principal : titres <h1> des pages. Nom de balise HTML : ne pas traduire.
  "tabs.h1": "H1",
  // Onglet principal : contenu des pages (nombre de mots, doublons). ~15 caractères conseillés (la barre défile).
  "tabs.content": "Content",
  // Onglet principal : images des pages (attribut alt, poids). ~15 caractères conseillés (la barre défile).
  "tabs.images": "Images",
  // Onglet principal : balises link rel=canonical. Gardé en anglais en français (vocabulaire de Screaming Frog) : faire de même si c'est l'usage, sinon traduire. ~15 caractères conseillés (la barre défile).
  "tabs.canonical": "Canonicals",
  // Onglet principal : directives aux robots (noindex, nofollow, meta robots). ~15 caractères conseillés (la barre défile).
  "tabs.directives": "Directives",
  // Onglet principal : balises hreflang (versions linguistiques d'une page). Nom d'attribut HTML : ne pas traduire.
  "tabs.hreflang": "Hreflang",
  // Onglet principal : pages liées par rel=prev / rel=next. ~15 caractères conseillés (la barre défile).
  "tabs.pagination": "Pagination",
  // Onglet principal : données structurées (JSON-LD, schema.org). Gardé en anglais en français (vocabulaire de Screaming Frog) : faire de même si c'est l'usage, sinon traduire. ~16 caractères conseillés (la barre défile).
  "tabs.schema": "Structured Data",
  // Onglet principal : tous les liens trouvés (source, cible, ancre). ~15 caractères conseillés (la barre défile).
  "tabs.links": "Links",
  // Onglet principal : liste des problèmes SEO détectés (critiques, avertissements, opportunités). ~15 caractères conseillés (la barre défile).
  "tabs.issues": "Issues",
  // Onglet principal : URL déclarées dans le sitemap XML du site. Terme technique souvent gardé tel quel. ~15 caractères conseillés (la barre défile).
  "tabs.sitemap": "Sitemap",
  // Onglet principal : poids des pages et volume de données transférées. ~15 caractères conseillés (la barre défile).
  "tabs.bandwidth": "Bandwidth",
  // Texte grisé du champ de recherche au-dessus de la grille (filtre les lignes par adresse). Champ de 280 px.
  "tools.search_placeholder": "Filter by URL...",
  // Bouton-filtre de l'onglet Problèmes, suivi d'un compteur : tous les problèmes, quelle que soit leur gravité. Très court.
  "tools.sev_all": "All",
  // Bouton-filtre de l'onglet Problèmes, suivi d'un compteur : problèmes de gravité critique. ~12 caractères conseillés (abréger au besoin).
  "tools.sev_critical": "Critical",
  // Bouton-filtre de l'onglet Problèmes, suivi d'un compteur : avertissements. Le français abrège « Avertissements » faute de place : abréger si le mot dépasse ~12 caractères.
  "tools.sev_warning": "Warnings",
  // Bouton-filtre de l'onglet Problèmes, suivi d'un compteur : opportunités d'amélioration (gravité la plus faible). Le français abrège « Opportunités » : abréger si le mot dépasse ~13 caractères.
  "tools.sev_opportunity": "Opportunities",
  // Pastille au-dessus de la grille quand une catégorie du panneau Overview filtre la liste. {label} = nom de la catégorie (déjà traduit, affiché en gras). Suivie d'une petite croix pour retirer le filtre.
  "tools.active_filter": "Filter: {label}",
  // Infobulle de la petite croix de la pastille « Filtre : … ».
  "tools.active_filter_clear_title": "Remove filter",
  // Pastille au-dessus de la grille pendant une sélection de cellules (clic, Maj+clic, Ctrl+clic). {n} = nombre de cellules sélectionnées (en gras ; choisit la forme plurielle) ; {col} = nom de la colonne (déjà traduit). Le français garde « (s) » ; ailleurs, une forme par cas pluriel. Garder le point médian « · ».
  "tools.selection": {"one":"{n} cell · {col}","other":"{n} cells · {col}"},
  // Infobulle de la petite croix de la pastille de sélection. Entre parenthèses : le nom de la touche Échap tel qu'il est écrit sur les claviers de ta langue.
  "tools.selection_clear_title": "Deselect (Esc)",
  // Étiquette devant la liste déroulante qui choisit la vue de l'onglet Sitemap. Très court.
  "tools.sitemap_view": "View",
  // Choix de la liste déroulante de l'onglet Sitemap : toutes les adresses déclarées dans le sitemap.
  "tools.sitemap_all": "All sitemap URLs",
  // Choix de la liste déroulante de l'onglet Sitemap : pages présentes dans le sitemap mais qu'aucun lien n'a permis d'atteindre (« orphelines »).
  "tools.sitemap_orphan": "Orphan URLs (in sitemap, not crawled)",
  // Choix de la liste déroulante de l'onglet Sitemap : pages trouvées par le crawl mais absentes du sitemap.
  "tools.sitemap_notin": "Pages not in sitemap",
  // Petit bouton de menu au-dessus de la grille (suivi d'une flèche ▾ ajoutée à part) : exports CSV de tout le crawl. En anglais « Bulk export », en casse de phrase comme les autres boutons (« Bulk Export » chez Screaming Frog). ~18 caractères conseillés.
  "tools.bulk_export": "Bulk export",
  // Entrée du menu « Export en masse » : exporter tous les liens en CSV.
  "tools.bulk_links": "All links",
  // Entrée du menu « Export en masse » : exporter toutes les images en CSV.
  "tools.bulk_images": "All images",
  // Entrée du menu « Export en masse » : exporter tous les problèmes SEO en CSV.
  "tools.bulk_issues": "All issues",
  // Petit bouton de menu au-dessus de la grille (suivi d'une flèche ▾ ajoutée à part) : rapports de synthèse exportés en CSV. ~12 caractères.
  "tools.reports": "Reports",
  // Entrée du menu « Rapports » : redirections qui s'enchaînent (A → B → C).
  "tools.report_redirects": "Redirect chains",
  // Entrée du menu « Rapports » : balises canonical. Gardé en anglais en français (vocabulaire de Screaming Frog) ; même mot que l'onglet « Canonicals ».
  "tools.report_canonicals": "Canonicals",
  // Entrée du menu « Rapports » : pages que les moteurs de recherche ne peuvent pas indexer (noindex, canonical vers une autre page, erreur…).
  "tools.report_nonindex": "Non-indexable pages",
  // Entrée du menu « Rapports » : pages HTTPS qui chargent des ressources en HTTP (terme consacré « mixed content »).
  "tools.report_mixed": "Mixed content",
  // Petit bouton au-dessus de la grille : exporte la vue affichée en fichier CSV. ~14 caractères.
  "tools.export_csv": "Export CSV",
  // Infobulle de la poignée en haut du panneau de détail (bas de l'écran) : la faire glisser change sa hauteur, un double-clic rend la hauteur d'origine.
  "dtabs.resize_title": "Drag to resize — double-click to reset",
  // Sous-onglet du panneau de détail d'une page, suivi d'un compteur : liens qui pointent VERS cette page. Vocabulaire de Screaming Frog gardé en anglais en français : faire de même si c'est l'usage, sinon traduire. ~14 caractères.
  "dtabs.inlinks": "Inlinks",
  // Sous-onglet du panneau de détail, suivi d'un compteur : liens qui partent DE cette page. Vocabulaire de Screaming Frog gardé en anglais en français : faire de même si c'est l'usage, sinon traduire. ~14 caractères.
  "dtabs.outlinks": "Outlinks",
  // Sous-onglet du panneau de détail, suivi d'un compteur : images de la page dont l'attribut alt est absent. « alt » = nom d'attribut HTML, ne pas traduire. ~20 caractères.
  "dtabs.images": "Images missing alt text",
  // Sous-onglet du panneau de détail : aperçu de la page dans les résultats de Google. Terme SEO (SERP = page de résultats) gardé en anglais en français : faire de même si c'est l'usage.
  "dtabs.serp": "SERP Snippet",
  // Sous-onglet du panneau de détail : informations de la page et en-têtes HTTP de sa réponse. ~18 caractères.
  "dtabs.info": "Info / Headers",
  // Sous-onglet du panneau de détail, suivi d'un compteur : blocs JSON-LD bruts de la page. ~20 caractères.
  "dtabs.schema": "Structured Data",
  // Sous-onglet du panneau de détail : code HTML source de la page.
  "dtabs.source": "Source",
  // Infobulle de la croix qui ferme le panneau de détail.
  "dtabs.close_title": "Close",
  // Barre d'état en bas de la fenêtre (petite police ; elle passe à la ligne si la place manque : rester court, sans sacrifier le sens). {done} = pages crawlées (en gras), {total} = total connu ou « ∞ ». Ponctuation et espaces avant « : » selon l'usage de ta langue.
  "statusbar.pages": "Pages: {done} / {total}",
  // Barre d'état : {queued} = nombre d'URL en attente d'être crawlées (file d'attente). Court de préférence (la barre passe à la ligne s'il le faut).
  "statusbar.queue": "Queue: {queued}",
  // Barre d'état : vitesse du crawl. {rate} = nombre de pages par seconde, une décimale, déjà mis au format de la langue par le code (ex. « 4.2 » en anglais) : ne rien y ajouter. Unité à abréger selon l'usage de ta langue (fr « p/s » = pages par seconde). Court de préférence (la barre passe à la ligne s'il le faut).
  "statusbar.rate": "Rate: {rate} pages/s",
  // Barre d'état : {active} = tâches de téléchargement occupées en ce moment, {threads} = nombre configuré (réglage « Threads »). « Workers » est souvent gardé en anglais. Court de préférence (la barre passe à la ligne s'il le faut).
  "statusbar.workers": "Workers: {active}/{threads}",
  // Infobulle de « Workers : x/y » dans la barre d'état.
  "statusbar.workers_title": "Workers actually busy / threads configured. If it stays low, the bottleneck is not the number of threads (rate cap, empty queue, or Chrome measurements running one at a time).",
  // Barre d'état : temps de réponse moyen du serveur. {avgms} = millisecondes (nombre entier). « ms » = milliseconde. Le français abrège (« Resp. moy. » = réponse moyenne) : rester court.
  "statusbar.avg_response": "Avg. response: {avgms} ms/page",
  // Barre d'état : volume total téléchargé. {bytes} = taille déjà formatée par le code : nombre au format de la langue ET unité comprise (ex. « 12.40 Mo » en français, « 12.40 MB » en anglais) : ne rien y ajouter, ni nombre ni unité. Court de préférence (la barre passe à la ligne s'il le faut).
  "statusbar.total_size": "Total size: {bytes}",
  // Menu du clic droit sur la grille : copier les cellules sélectionnées. {n} = nombre de valeurs (choisit la forme plurielle) ; {col} = nom de la colonne (déjà traduit). Le français garde « (s) » ; ailleurs, une forme par cas pluriel. Garder le tiret « — ».
  "ctx.copy": {"one":"Copy {n} value — {col}","other":"Copy {n} values — {col}"},
  // Menu du clic droit sur la grille : copier toutes les valeurs de la colonne visible (après filtre).
  "ctx.copy_column": "Copy entire column as displayed",
  // Menu du clic droit sur une cellule d'adresse : ouvrir l'URL dans le navigateur web par défaut.
  "ctx.open": "Open address in browser",
  // Infobulle du bouton de choix de la langue (drapeau + nom de la langue, à côté de « Réglages ») et nom de sa liste pour les lecteurs d'écran.
  "lang.title": "Interface language",
  // Première entrée de la liste des langues : suivre la langue de Windows. {name} = nom de la langue détectée, écrit dans sa propre langue (« English », « Deutsch »), affiché en gris. PAS de parenthèses autour de {name} : certains noms de langue en contiennent déjà (« Português (Brasil) »). Ponctuation de votre langue (deux-points ou tiret).
  "lang.auto": "Automatic: {name}",
  // Infobulle de l'entrée « Automatique » de la liste des langues.
  "lang.auto_title": "Follow the Windows display language",

  // ----- cles-mainjs.json -----
  // Poids en octets (moins de 1 Ko) : grille, barre d'état, panneau Infos. {n} = nombre entier. Unité courte.
  "unit.bytes": "{n} B",
  // Poids en kilo-octets. {n} = nombre déjà écrit au format de la langue (1 décimale : « 12.3 » en anglais, « 12,3 » en allemand ; le français garde provisoirement son point). Ne traduire que l'unité.
  "unit.kb": "{n} KB",
  // Poids en mégaoctets. {n} = nombre déjà écrit au format de la langue (2 décimales : « 1.25 » en anglais). Ne traduire que l'unité.
  "unit.mb": "{n} MB",
  // Poids en gigaoctets. {n} = nombre déjà écrit au format de la langue (2 décimales). Ne traduire que l'unité.
  "unit.gb": "{n} GB",
  // Durée en millisecondes (temps de réponse, temps Cloudflare). {n} = nombre.
  "unit.ms": "{n} ms",
  // Durée en secondes (temps de réponse ≥ 1 s, âge en cache). {n} = nombre déjà écrit avec le séparateur décimal de la langue (« 1,5 » en français).
  "unit.s": "{n} s",
  // Âge en cache, en minutes. {n} = nombre entier.
  "unit.min": "{n} min",
  // Âge en cache, en heures. {n} = nombre avec le séparateur décimal de la langue.
  "unit.h": "{n} h",
  // Âge en cache, en jours (« j » = jours). {n} = nombre avec le séparateur décimal de la langue.
  "unit.d": "{n} d",
  // Pourcentage (ligne « Ratio texte/code » du panneau Infos). {n} = nombre déjà écrit au format de la langue (0 ou 1 décimale). Typographie de la langue (espace avant % en français).
  "unit.percent": "{n}%",
  // Infobulle de la pastille du code HTTP (grille, onglet Sitemap), affichée après « 0 · ». Code 0 = aucune réponse. Forme « Nom : explication ».
  "http.explain_0": "No response: page unreachable, blocked by robots.txt, or network error / timeout.",
  // Infobulle du code HTTP 200, affichée après « 200 · ». Forme « Nom du statut : explication ».
  "http.explain_200": "OK: the page responds normally.",
  // Infobulle du code HTTP 201 (après « 201 · »).
  "http.explain_201": "Created: the resource was created successfully.",
  // Infobulle du code HTTP 204 (après « 204 · »).
  "http.explain_204": "No Content: valid response, but with no body.",
  // Infobulle du code HTTP 206 (après « 206 · »). « Range » = en-tête HTTP, ne pas traduire.
  "http.explain_206": "Partial Content: fragmented response (Range request).",
  // Infobulle du code HTTP 301 (après « 301 · »). « jus SEO » = link equity / PageRank transmis.
  "http.explain_301": "Moved Permanently: the page has moved for good. Link equity is passed to the target — remember to update your links.",
  // Infobulle du code HTTP 302 (après « 302 · »).
  "http.explain_302": "Found (temporary redirect): a provisional move. Little or no link equity is passed. If the change is permanent, use a 301 instead.",
  // Infobulle du code HTTP 303 (après « 303 · »). POST = méthode HTTP, ne pas traduire.
  "http.explain_303": "See Other: redirects to another URL (typically after a POST).",
  // Infobulle du code HTTP 304 (après « 304 · »).
  "http.explain_304": "Not Modified: the cached version is still valid, nothing is downloaded again.",
  // Infobulle du code HTTP 307 (après « 307 · »).
  "http.explain_307": "Temporary Redirect (strict): like a 302, but keeps the HTTP method.",
  // Infobulle du code HTTP 308 (après « 308 · »).
  "http.explain_308": "Permanent Redirect (strict): like a 301 (passes link equity), but keeps the HTTP method.",
  // Infobulle du code HTTP 400 (après « 400 · »).
  "http.explain_400": "Bad Request: the server could not understand the request.",
  // Infobulle du code HTTP 401 (après « 401 · »).
  "http.explain_401": "Unauthorized: you must be logged in to access the page.",
  // Infobulle du code HTTP 403 (après « 403 · »). Googlebot = nom propre.
  "http.explain_403": "Forbidden: the server refuses access (permissions, firewall, bot blocking). Make sure Googlebot is not blocked.",
  // Infobulle du code HTTP 404 (après « 404 · »).
  "http.explain_404": "Not Found: the page does not exist (broken link). Fix it — 301-redirect it or remove the inbound links.",
  // Infobulle du code HTTP 405 (après « 405 · »). GET, POST = méthodes HTTP, ne pas traduire.
  "http.explain_405": "Method Not Allowed: the HTTP method (GET, POST…) is not allowed here.",
  // Infobulle du code HTTP 406 (après « 406 · »).
  "http.explain_406": "Not Acceptable: the server cannot provide the requested format.",
  // Infobulle du code HTTP 408 (après « 408 · »).
  "http.explain_408": "Request Timeout: the server waited too long.",
  // Infobulle du code HTTP 410 (après « 410 · »).
  "http.explain_410": "Gone: the page was removed on purpose (a stronger deindexing signal than a 404).",
  // Infobulle du code HTTP 418 (après « 418 · ») : code humoristique du protocole « HTCPCP ».
  "http.explain_418": "I'm a teapot (an inside joke; a playful server).",
  // Infobulle du code HTTP 429 (après « 429 · »). « Req / sec » (« Req/sec » dans le texte français d'origine) = nom exact du réglage du volet Réglages (drawer.rps) : reprendre sa traduction.
  "http.explain_429": "Too Many Requests: the server is rate-limiting. Slow the crawl down (lower Req / sec).",
  // Infobulle du code HTTP 451 (après « 451 · »).
  "http.explain_451": "Unavailable For Legal Reasons.",
  // Infobulle du code HTTP 500 (après « 500 · »).
  "http.explain_500": "Internal Server Error: a bug or outage on the server side. Fix it first (it blocks indexing).",
  // Infobulle du code HTTP 501 (après « 501 · »).
  "http.explain_501": "Not Implemented: the server does not support this feature.",
  // Infobulle du code HTTP 502 (après « 502 · »).
  "http.explain_502": "Bad Gateway: an intermediate server received an invalid response.",
  // Infobulle du code HTTP 503 (après « 503 · »).
  "http.explain_503": "Service Unavailable: the server is overloaded or under maintenance. Temporary, but keep an eye on it.",
  // Infobulle du code HTTP 504 (après « 504 · »).
  "http.explain_504": "Gateway Timeout: an intermediate server did not respond in time.",
  // Infobulle d'un code 2xx sans explication propre (après « 2xx · »).
  "http.class_2xx": "Success (2xx): the request succeeded.",
  // Infobulle d'un code 3xx sans explication propre.
  "http.class_3xx": "Redirection (3xx): the resource is elsewhere.",
  // Infobulle d'un code 4xx sans explication propre.
  "http.class_4xx": "Client error (4xx): missing page, access denied, etc.",
  // Infobulle d'un code 5xx sans explication propre.
  "http.class_5xx": "Server error (5xx): a problem on the server side.",
  // Infobulle d'un code HTTP hors des familles 2xx à 5xx.
  "http.unusual": "Unusual code.",
  // Ce qu'a fait UNE couche de cache, en fin de ligne : « Cloudflare : HIT — a fourni la page ». Sujet implicite = la couche (un CDN ou un cache). Minuscule initiale.
  "cachetip.role_served": "served the page",
  // Rôle d'une couche de cache en MISS (même place que cachetip.role_served).
  "cachetip.role_missing": "did not have the page",
  // Rôle d'une couche de cache en EXPIRED (même place que cachetip.role_served, en fin de ligne « Cloudflare : EXPIRED — avait une copie périmée ») : la couche avait bien la page, mais une copie périmée, qu'elle est allée revalider auprès du serveur. À ne pas confondre avec cachetip.role_missing (MISS : pas de copie du tout). Minuscule initiale.
  "cachetip.role_expired": "had a stale copy",
  // Rôle d'une couche de cache en REVALIDATED (même place que cachetip.role_served).
  "cachetip.role_revalidated": "checked with the server",
  // Rôle d'une couche de cache en BYPASS (même place que cachetip.role_served).
  "cachetip.role_bypassed": "was bypassed",
  // Rôle d'une couche de cache en DYNAMIC (même place que cachetip.role_served).
  "cachetip.role_dynamic": "does not cache this page",
  // Rôle d'une couche de cache dont l'en-tête est présent mais sans verdict lisible.
  "cachetip.role_unknown": "present, no indication",
  // Une couche de cache dans une liste. {engine} = nom du cache (Cloudflare, HiveCache…) ou de l'en-tête ; {raw} = valeur brute de l'en-tête (HIT, MISS…), non traduite. Seule la ponctuation suit la langue.
  "cachetip.layer": "{engine}: {raw}",
  // Même chose suivie du rôle de la couche : {engine} = nom du cache ou de l'en-tête (Cloudflare, HiveCache…), {raw} = valeur brute de l'en-tête (HIT, MISS…, non traduite), {role} = ce qu'a fait la couche (une des clés cachetip.role_*). Garder les deux espaces avant le tiret.
  "cachetip.layer_role": "{engine}: {raw}  — {role}",
  // Explication du verdict de cache HIT (infobulle de la colonne Cache, ligne Cache du panneau Infos), après « HIT — ».
  "cachetip.meaning_hit": "Page served from the cache: the server was not called.",
  // Explication du verdict MISS, après « MISS — ».
  "cachetip.meaning_miss": "The cache did not have the page: the server had to generate it.",
  // Explication du verdict EDGE (site hébergé sur Cloudflare Pages/Workers). \n = retour à la ligne dans l'infobulle : le garder. Pages et Workers = noms de produits.
  "cachetip.meaning_edge": "Page hosted on the Cloudflare network (Pages / Workers).\nThere is no origin server to call: neither HIT nor MISS is possible.",
  // Explication du verdict EXPIRED.
  "cachetip.meaning_expired": "The cache had the page, but stale: it went back to the server to revalidate it.",
  // Explication du verdict REVALIDATED.
  "cachetip.meaning_revalidated": "The cache checked its copy with the server, which confirmed it is up to date.",
  // Explication du verdict STALE.
  "cachetip.meaning_stale": "Stale page served while the cache refreshes it in the background.",
  // Explication du verdict UPDATING.
  "cachetip.meaning_updating": "Stale page served; its update is in progress.",
  // Explication du verdict BYPASS.
  "cachetip.meaning_bypass": "The cache was bypassed: a rule, a cookie or a header prevented it.",
  // Explication du verdict DYNAMIC (Cloudflare). \n = retour à la ligne : le garder.
  "cachetip.meaning_dynamic": "Cloudflare does not cache this page: no rule covers it.\nEvery visit reaches the server.",
  // Explication du verdict PRERENDER.
  "cachetip.meaning_prerender": "Pre-generated page served as is.",
  // Explication du verdict NONE.
  "cachetip.meaning_none": "No caching for this response.",
  // Infobulle de cache, tableau des temps Cloudflare : étape du trajet de la requête, après une durée (« 6 ms au point de présence… »). Minuscule initiale.
  "cachetip.timing_edge": "at the nearest Cloudflare point of presence",
  // Même tableau : temps passé sur le serveur d'origine (« 0 ms sur le serveur d'origine »).
  "cachetip.timing_origin": "on the origin server",
  // Même tableau : temps passé dans un Cloudflare Worker (nom de produit).
  "cachetip.timing_worker": "in a Worker (code running on the Cloudflare network)",
  // Remarque en bout de la ligne « sur le serveur d'origine » quand sa durée est > 0 : le serveur d'origine a travaillé. Garder la flèche « <- ».
  "cachetip.origin_worked": "<- it did some work",
  // Remarque en bout de la même ligne quand la durée vaut 0 : le serveur d'origine n'a pas été sollicité. Garder la flèche « <- ».
  "cachetip.origin_idle": "<- it was not called",
  // Infobulle de la colonne Cache quand la réponse n'a aucun en-tête de cache. \n = retour à la ligne. « PAS » en capitales : insistance voulue.
  "cachetip.no_headers": "No cache header in the response.\nNo conclusion possible — and this is NOT a MISS: the server simply said nothing about it.",
  // Première ligne de l'infobulle de cache. {status} = verdict technique (HIT, MISS…, non traduit) ; {meaning} = son explication (clés cachetip.meaning_*).
  "cachetip.verdict": "{status} — {meaning}",
  // Infobulle de cache quand des en-têtes existent mais sans verdict.
  "cachetip.status_unknown": "Status undetermined.",
  // Infobulle de cache : la page est venue du serveur lui-même.
  "cachetip.served_origin": "Served by: the origin server (no cache answered)",
  // Infobulle de cache. {who} = nom du cache ou du CDN qui a fourni la page (Cloudflare, WP Rocket…).
  "cachetip.served_by": "Served by: {who}",
  // Infobulle de cache. {age} = durée déjà formatée avec son unité (« 45 s », « 3 min », « 2,5 h »).
  "cachetip.age": "Cached for: {age}",
  // Infobulle de cache : titre du petit tableau des temps (en-tête server-timing de Cloudflare).
  "cachetip.timings_title": "Timings measured by Cloudflare:",
  // Infobulle de cache : titre de la liste numérotée des couches de cache traversées.
  "cachetip.layers_title": "Layers crossed, from the outside in to the server:",
  // Dernière ligne de l'infobulle de cache. {header} = nom technique d'un en-tête HTTP (cf-cache-status…), non traduit.
  "cachetip.read_from": "(read from the {header} header)",
  // Valeur de la ligne « Cache » du panneau Infos quand la réponse n'a aucun en-tête de cache. Commence par un tiret.
  "cachetip.info_no_headers": "— no cache header (this is not a MISS: the server said nothing)",
  // Segment de la ligne « Cache » du panneau Infos (segments séparés par « | »).
  "cachetip.info_served_origin": "Served by the origin server",
  // Segment de la ligne « Cache » du panneau Infos. {who} = nom du cache ou du CDN.
  "cachetip.info_served_by": "Served by {who}",
  // Segment de la ligne « Cache » du panneau Infos, minuscule initiale. {age} = durée formatée avec son unité.
  "cachetip.info_age": "cached for {age}",
  // Dernier segment de la ligne « Cache » du panneau Infos. {layers} = liste des couches (« Cloudflare : MISS — … ») séparées par « > ».
  "cachetip.info_layers": "layers: {layers}",
  // Infobulle d'une adresse (URL) dans la grille et le panneau de détail, sous l'URL complète.
  "cell.url_open_hint": "Double-click: open in browser",
  // Pastille de la colonne Indexable (onglet Internal). COLONNE ÉTROITE : environ 48 px de texte à 1100 px, 70 px à 1400 px. Valeur COURTE : le « Oui » de votre langue, comme cell.yes (le français écrit « Oui », l'anglais « Yes »). Le libellé complet va dans cell.indexable_full : infobulle de la pastille ET valeur de l'export CSV.
  "cell.indexable": "Yes",
  // Infobulle de la pastille cell.indexable et valeur de l'export CSV de la colonne Indexable : libellé COMPLET, sans limite de place.
  "cell.indexable_full": "Indexable",
  // Pastille de la colonne Indexable : la page n'est pas indexable. COLONNE ÉTROITE : environ 48 px de texte à 1100 px, 70 px à 1400 px. Valeur COURTE : le « Non » de votre langue, comme cell.no (le français écrit « Non », l'anglais « No »). Le libellé complet va dans cell.non_indexable_full : infobulle de la pastille ET valeur de l'export CSV.
  "cell.non_indexable": "No",
  // Infobulle de la pastille cell.non_indexable et valeur de l'export CSV de la colonne Indexable : libellé COMPLET, sans limite de place.
  "cell.non_indexable_full": "Non-Indexable",
  // Valeur « oui » avec majuscule : pastilles (colonne Dupliqué, Indexable de l'onglet Directives), panneau Infos, exports CSV. Court.
  "cell.yes": "Yes",
  // Valeur « non » avec majuscule (mêmes places que cell.yes). Court.
  "cell.no": "No",
  // Valeur « oui » en minuscules : colonnes oui/non des onglets URL, Sécurité, Directives, Structured Data, et leurs exports. Colonne étroite.
  "cell.yes_lc": "yes",
  // Valeur « non » en minuscules (mêmes places que cell.yes_lc).
  "cell.no_lc": "no",
  // Pastille de la colonne Near-dup (onglet Contenu) : la page a {n} pages quasi dupliquées. {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "cell.yes_count": "Yes ({n})",
  // Export CSV de la colonne Near-dup : {n} = nombre de pages similaires, {urls} = leurs adresses séparées par « | ».
  "cell.near_dup_csv": "{n}: {urls}",
  // Pastille de l'onglet Images : {n} images sans attribut alt (n ≥ 1). « alt » = attribut HTML, ne pas traduire. {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "cell.n_missing_alt": "{n} missing alt",
  // Pastille de la colonne Réciproque (onglet Hreflang) : {n} pages visées par un hreflang ne renvoient pas de lien retour. {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "cell.recip_missing_count": "Missing ({n})",
  // Export CSV de la colonne Réciproque. {urls} = adresses séparées par « | ».
  "cell.recip_missing_list": "Missing: {urls}",
  // Infobulle d'un badge de type de données structurées (onglet Structured Data) : un clic ouvre le JSON-LD.
  "cell.view_jsonld": "View raw JSON-LD",
  // Type d'un lien : pastille de l'onglet Liens et du sous-onglet Outlinks, valeur des exports.
  "cell.internal": "Internal",
  // Type d'un lien : lien vers un autre site (mêmes places que cell.internal).
  "cell.external": "External",
  // Pastille de l'onglet Sitemap : URL déclarée dans le sitemap mais jamais atteinte par le crawl (féminin en français : « URL orpheline »).
  "cell.orphan": "Orphan",
  // Infobulle de la pastille « Orpheline » (cell.orphan).
  "cell.orphan_title": "Listed in the sitemap but not reached by the crawl",
  // Valeur de l'export CSV de l'onglet Sitemap pour une URL orpheline.
  "cell.orphan_csv": "Orphan (in sitemap, not crawled)",
  // Pastille de l'onglet Sitemap : page atteinte par le crawl mais absente du sitemap.
  "cell.not_in_sitemap": "Not in sitemap",
  // Infobulle de la pastille « Hors sitemap ».
  "cell.not_in_sitemap_title": "Page reached by the crawl but missing from the sitemap",
  // Pastille de l'onglet Sitemap : URL du sitemap ET atteinte par le crawl.
  "cell.crawled": "Crawled",
  // En-tête de la colonne des URL (1re colonne de presque tous les onglets, 3e de l'onglet Problèmes), et de l'export CSV. Place : 164 px de texte au plus étroit (environ 25 lettres). Vocabulaire Screaming Frog.
  "col.address": "Address",
  // Infobulle de l'en-tête col.address, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.address n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.address_full": "Address",
  // En-tête de colonne (onglets Internal et Response Codes) : code de réponse HTTP. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.status_code_full.
  "col.status_code": "Status",
  // Infobulle de l'en-tête col.status_code, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (un seul nom pour le code HTTP dans toute l'interface : col.http_code, detail.k_http_code) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.status_code_full": "Status Code",
  // En-tête de colonne (onglet Internal) : verdict de cache HIT/MISS. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.cache": "Cache",
  // Infobulle de l'en-tête col.cache, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (HIT, MISS…) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.cache_full": "Cache Status",
  // En-tête de colonne (onglet Internal) : qui a réellement servi la page (Cloudflare, origine…). COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.served_by_full.
  "col.served_by": "Served",
  // Infobulle de l'en-tête col.served_by, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place. En français : le libellé affiché.
  "col.served_by_full": "Served By",
  // En-tête de colonne (onglets Internal et Response Codes) : type de contenu (HTML, CSS, PDF…). COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.content_type": "Type",
  // Infobulle de l'en-tête col.content_type, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (HTML, CSS, PDF… ; « Type » seul ne dit pas de quoi) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.content_type_full": "Content Type",
  // En-tête de colonne (onglets Internal et Bande passante) : temps de réponse de la page, pastille colorée en ms ou s. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.speed_full.
  "col.speed": "Speed",
  // Infobulle de l'en-tête col.speed, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que detail.k_response_time) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.speed_full": "Response Time",
  // En-tête de colonne (onglets Internal et Contenu) : nombre de mots de la page. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.words_full.
  "col.words": "Words",
  // Infobulle de l'en-tête col.words, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.words_full": "Word Count",
  // En-tête de colonne (onglets Internal et Page Titles) : balise <title> de la page. En français aussi le mot anglais « Title » (vocabulaire Screaming Frog) ; à adapter si la langue a un terme SEO courant. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.title": "Title",
  // Infobulle de l'en-tête col.title, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.title n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.title_full": "Title",
  // En-tête de colonne (onglets Internal et Directives) : la page est-elle indexable. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.indexable_full.
  "col.indexable": "Index.",
  // Infobulle de l'en-tête col.indexable, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place. En français : le libellé affiché.
  "col.indexable_full": "Indexable",
  // En-tête de colonne (onglet Internal) : nombre de liens sortants. Terme Screaming Frog conservé en français. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. « Outlinks » occupe toute la place disponible : une traduction plus longue sera abrégée. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.outlinks": "Outlinks",
  // Infobulle de l'en-tête col.outlinks, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.outlinks n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.outlinks_full": "Outlinks",
  // En-tête de colonne (onglet Response Codes) : cible de la redirection. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.redirect_to": "Redirect URL",
  // Infobulle de l'en-tête col.redirect_to, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.redirect_to n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.redirect_to_full": "Redirect URL",
  // En-tête de colonne (onglets URL, Page Titles, Meta Description) : longueur en caractères. Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.length": "Length",
  // Infobulle de l'en-tête col.length (onglet URL), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.url_length_full": "URL Length",
  // Infobulle de l'en-tête col.length (onglet Page Titles), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.title_length_full": "Title Length",
  // Infobulle de l'en-tête col.length (onglet Meta Description), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.meta_length_full": "Meta Description Length",
  // En-tête de colonne (onglet URL) : l'URL contient des caractères non ASCII (oui/—). Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.non_ascii": "Non-ASCII",
  // Infobulle de l'en-tête col.non_ascii, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.non_ascii_full": "Non-ASCII Characters",
  // En-tête de colonne (onglet URL) : l'URL contient des « _ » (oui/—). Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.underscores": "Underscores",
  // Infobulle de l'en-tête col.underscores, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.underscores n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.underscores_full": "Underscores",
  // En-tête de colonne (onglet URL) : l'URL contient des majuscules (oui/—). Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.uppercase": "Uppercase",
  // Infobulle de l'en-tête col.uppercase, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.uppercase n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.uppercase_full": "Uppercase",
  // En-tête de colonne (onglet URL) : l'URL contient des barres obliques doublées. Garder « // ». Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.slashes": "Slashes //",
  // Infobulle de l'en-tête col.slashes, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.slashes n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.slashes_full": "Slashes //",
  // En-tête de colonne (onglet URL) : l'URL contient des paramètres (?a=b). Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.parameters": "Parameters",
  // Infobulle de l'en-tête col.parameters, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.parameters n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.parameters_full": "Parameters",
  // En-tête de colonne (onglet Sécurité) : nombre de ressources http:// chargées par une page https://. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.mixed_content": "Mixed Content",
  // Infobulle de l'en-tête col.mixed_content, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.mixed_content n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.mixed_content_full": "Mixed Content",
  // En-tête de colonne (onglet Sécurité) : en-têtes HTTP de sécurité absents. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.missing_headers": "Missing Headers",
  // Infobulle de l'en-tête col.missing_headers, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.missing_headers_full": "Missing Security Headers",
  // En-tête de colonne (onglets Page Titles, Meta Description, H1) : la valeur se retrouve à l'identique sur une autre page. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.duplicate": "Duplicate",
  // Infobulle de l'en-tête col.duplicate (onglet Page Titles), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.title_dup_full": "Duplicate Title",
  // Infobulle de l'en-tête col.duplicate (onglet Meta Description), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.meta_dup_full": "Duplicate Meta Description",
  // Infobulle de l'en-tête col.duplicate (onglet H1), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.h1_dup_full": "Duplicate H1",
  // En-tête de colonne (onglets Page Titles, Meta Description, H1, Images) : Manquant / Court / Long / OK. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.state": "Status",
  // Infobulle de l'en-tête col.state (onglet Page Titles : Manquant / Court / Long / OK), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.title_state_full": "Title Status",
  // Infobulle de l'en-tête col.state (onglet Meta Description : Manquant / Court / Long / OK), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.meta_state_full": "Meta Description Status",
  // Infobulle de l'en-tête col.state (onglet H1 : Manquant / Multiple / OK), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.h1_state_full": "H1 Status",
  // Infobulle de l'en-tête col.state (onglet Images : OK / N sans alt), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.images_state_full": "Alt Text Status",
  // En-tête de colonne (onglet Meta Description) : contenu de la balise meta description. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.meta_description": "Meta Description",
  // Infobulle de l'en-tête col.meta_description, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.meta_description n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.meta_description_full": "Meta Description",
  // En-tête de colonne (onglet H1) : texte du premier titre H1. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.first_h1": "First H1",
  // Infobulle de l'en-tête col.first_h1, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.first_h1 n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.first_h1_full": "First H1",
  // En-tête de colonne (onglet H1) : nombre de balises H1 de la page. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.count": "Count",
  // Infobulle de l'en-tête col.count, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (onglet H1) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.count_full": "H1 Count",
  // En-tête de colonne (onglet Contenu) : pages quasi dupliquées. Abréviation anglaise conservée en français. Place : environ 255 px de texte à 1100 px.
  "col.near_dup": "Near-dup",
  // Infobulle de l'en-tête col.near_dup, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que overview.near_dup et detail.k_near_dup) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.near_dup_full": "Near Duplicates",
  // En-tête de colonne (onglets Images et Bande passante) : nombre d'images. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.images": "Images",
  // Infobulle de l'en-tête col.images (onglet Images), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.image_count_full": "Image Count",
  // Infobulle de l'en-tête col.images (onglet Bande passante : la cellule montre « nombre / poids », le CSV ne contient que le poids, en octets), et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.images_weight_full": "Image Size",
  // En-tête de colonne (onglet Images) : nombre d'images sans attribut alt (« alt » = attribut HTML). Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.missing_alt": "Missing Alt Text",
  // Infobulle de l'en-tête col.missing_alt, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que dtabs.images) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.missing_alt_full": "Images Missing Alt Text",
  // En-tête de colonne (onglet Canonicals) : URL de la balise canonical. Place : environ 255 px de texte à 1100 px.
  "col.canonical": "Canonical",
  // Infobulle de l'en-tête col.canonical, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.canonical n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.canonical_full": "Canonical",
  // En-tête de colonne (onglet Canonicals) : Self / Autre / Manquant. Place : environ 255 px de texte à 1100 px.
  "col.canonical_type": "Type",
  // Infobulle de l'en-tête col.canonical_type, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (Self / Autre / Manquant) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.canonical_type_full": "Canonical Type",
  // En-tête de colonne (onglet Directives) : contenu de la balise meta robots. Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court.
  "col.meta_robots": "Meta Robots",
  // Infobulle de l'en-tête col.meta_robots, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.meta_robots n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.meta_robots_full": "Meta Robots",
  // En-tête de colonne (onglet Directives) : la page déclare une canonical vers une autre URL (oui/—). Place : environ 74 px de texte à 1100 px (10-11 lettres), 106 px à 1400 px. Rester court. (« Canonicalised » occupe 73 px : la limite.)
  "col.canonicalised": "Canonicalised",
  // Infobulle de l'en-tête col.canonicalised, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.canonicalised n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.canonicalised_full": "Canonicalised",
  // En-tête de colonne (onglet Hreflang) : liste « langue -> URL ». Garder « lang -> href ». Place : environ 255 px de texte à 1100 px.
  "col.hreflang": "Hreflang (lang -> href)",
  // Infobulle de l'en-tête col.hreflang, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.hreflang n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.hreflang_full": "Hreflang (lang -> href)",
  // En-tête de colonne (onglet Hreflang) : les pages visées renvoient-elles un hreflang vers celle-ci (OK / Manquant). Place : environ 255 px de texte à 1100 px.
  "col.reciprocal": "Return Links",
  // Infobulle de l'en-tête col.reciprocal, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que detail.k_hreflang_recip) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.reciprocal_full": "Hreflang Return Links",
  // En-tête de colonne (onglet Structured Data) : types schema.org détectés. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.types": "Types",
  // Infobulle de l'en-tête col.types, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.types_full": "Structured Data Types",
  // En-tête de colonne (onglet Structured Data) : données structurées valides (oui/non). Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.valid": "Valid",
  // Infobulle de l'en-tête col.valid, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que detail.k_sd_valid) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.valid_full": "Valid Structured Data",
  // En-tête de colonne (onglet Structured Data) : nombre d'erreurs de données structurées. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.errors": "Errors",
  // Infobulle de l'en-tête col.errors, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que detail.k_sd_errors) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.errors_full": "Structured Data Errors",
  // En-tête de la 1re colonne de l'onglet Liens : page qui contient le lien. Place : environ 289 px de texte à 1100 px (1re colonne).
  "col.source": "Source",
  // Infobulle de l'en-tête col.source, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.source n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.source_full": "Source",
  // En-tête de colonne (onglet Liens) : URL visée par le lien. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.destination": "Destination",
  // Infobulle de l'en-tête col.destination, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.destination n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.destination_full": "Destination",
  // En-tête de colonne (onglet Liens) : texte d'ancre du lien. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.anchor": "Anchor Text",
  // Infobulle de l'en-tête col.anchor, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.anchor_full": "Anchor Text",
  // En-tête de colonne (onglet Liens) : Interne / Externe. Place : environ 119 px de texte à 1100 px (17-18 lettres).
  "col.link_type": "Type",
  // Infobulle de l'en-tête col.link_type, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (Interne / Externe) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.link_type_full": "Link Type",
  // En-tête de la 1re colonne de l'onglet Problèmes : Critique / Avertissement / Opportunité / Info. Place : environ 289 px de texte à 1100 px (1re colonne).
  "col.severity": "Severity",
  // Infobulle de l'en-tête col.severity, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.severity n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.severity_full": "Severity",
  // En-tête de colonne (onglet Problèmes) : catégorie du problème (Titres, Images…). Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.category": "Category",
  // Infobulle de l'en-tête col.category, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.category n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.category_full": "Category",
  // En-tête de colonne (onglet Problèmes) : description du problème SEO. Place : environ 164 px de texte à 1100 px (environ 25 lettres).
  "col.message": "Message",
  // Infobulle de l'en-tête col.message, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.message n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.message_full": "Message",
  // En-tête de colonne (onglet Sitemap) : Crawlée / Orpheline / Hors sitemap. Place : environ 255 px de texte à 1100 px.
  "col.sitemap_state": "Status",
  // Infobulle de l'en-tête col.sitemap_state, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (Crawlée / Orpheline / Hors sitemap) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.sitemap_state_full": "Sitemap Status",
  // En-tête de colonne (onglet Sitemap) : code de réponse HTTP. Place : environ 255 px de texte à 1100 px. Même nom que col.status_code_full (un seul nom pour le code HTTP).
  "col.http_code": "Status Code",
  // Infobulle de l'en-tête col.http_code, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que col.status_code_full) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.http_code_full": "Status Code",
  // En-tête de colonne (onglet Bande passante) : technologies détectées (WordPress, Next.js…). Abréviation. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.tech": "Tech",
  // Infobulle de l'en-tête col.tech, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (même nom que detail.k_techs) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.tech_full": "Technologies",
  // En-tête de colonne (onglet Bande passante) : poids total transféré par la page. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.total_size_full.
  "col.total_size": "Size",
  // Infobulle de l'en-tête col.total_size, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place. En français : le libellé affiché.
  "col.total_size_full": "Total Size",
  // En-tête de colonne (onglet Bande passante) : nombre de requêtes HTTP de la page. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.requests_full.
  "col.requests": "Req.",
  // Infobulle de l'en-tête col.requests, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place. En français : le libellé affiché.
  "col.requests_full": "Requests",
  // En-tête de colonne (onglet Bande passante) : parmi ces requêtes, combien sont dynamiques (non mises en cache). COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.dynamic_full.
  "col.dynamic": "Dyn.",
  // Infobulle de l'en-tête col.dynamic, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.dynamic_full": "Dynamic Requests",
  // En-tête de colonne (onglet Bande passante) : nombre de réponses en MISS (MISS = terme technique, non traduit). COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. Le libellé complet est dans col.cache_miss_full.
  "col.cache_miss": "MISS",
  // Infobulle de l'en-tête col.cache_miss, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET qui dit ce que contient la colonne quand on lit le fichier CSV seul (MISS = terme technique, non traduit) ; sans limite de place et SANS abréviation. Un seul nom par notion : reprenez le nom déjà employé ailleurs dans l'interface pour la même notion.
  "col.cache_miss_full": "Cache MISS Requests",
  // En-tête de colonne (onglet Bande passante) : méthode de mesure, HTTP ou Chrome. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Libellé COURT obligatoire, abréviation d'usage admise. L'infobulle et l'export CSV prennent le libellé complet (la clé du même nom terminée par _full).
  "col.probe": "Probe",
  // Infobulle de l'en-tête col.probe, et en-tête de l'export CSV / de la barre de sélection : libellé COMPLET, sans limite de place et SANS abréviation. Si l'en-tête col.probe n'est pas abrégé dans votre langue, recopiez-le tel quel. En français : le libellé affiché.
  "col.probe_full": "Probe",
  // En-tête de la dernière colonne de l'onglet Bande passante, au-dessus d'une barre : poids de la page rapporté à celui de la page la plus lourde du crawl. COLONNE ÉTROITE : environ 48 px de texte à 1100 px (6-7 lettres latines ou cyrilliques, 3 caractères en japonais ou en chinois ; au-delà, « … » : abrégez, le libellé complet est en infobulle), 70 px à 1400 px. Le signe pour-cent suffit ; le libellé complet est dans col.weight_share_full.
  "col.weight_share": "%",
  // Infobulle de l'en-tête col.weight_share, et en-tête de l'export CSV de l'onglet Bande passante : libellé COMPLET. La colonne donne le poids de la page en pourcentage de celui de la page la plus lourde du crawl (100 = la plus lourde) ; même notion que col.total_size_full (« Poids total »).
  "col.weight_share_full": "Relative Size (%)",
  // État d'un title (pastille, onglet Page Titles) : la page n'a pas de <title>.
  "state.title_missing": "Missing",
  // État d'un title : moins de 30 caractères.
  "state.title_short": "Short",
  // État d'un title : plus de 60 caractères.
  "state.title_long": "Long",
  // État d'une meta description (onglet Meta Description) : absente. Le français garde le masculin ; accorder si la langue le demande.
  "state.meta_missing": "Missing",
  // État d'une meta description : moins de 70 caractères.
  "state.meta_short": "Short",
  // État d'une meta description : plus de 160 caractères.
  "state.meta_long": "Long",
  // État du H1 (onglet H1) : la page n'en a aucun.
  "state.h1_missing": "Missing",
  // État du H1 : la page en a plusieurs.
  "state.h1_multiple": "Multiple",
  // Type de canonical (onglet Canonicals, rapport Canonicals) : pas de balise canonical.
  "state.canonical_missing": "Missing",
  // Type de canonical : elle pointe vers la page elle-même (auto-référente). Terme anglais conservé en français.
  "state.canonical_self": "Self",
  // Type de canonical : elle pointe vers une autre URL.
  "state.canonical_other": "Other",
  // État conforme (pastille verte), plusieurs onglets.
  "state.ok": "OK",
  // Sévérité d'un problème SEO : pastille de l'onglet Problèmes, export « Tous les problèmes ».
  "severity.critical": "Critical",
  // Sévérité d'un problème SEO (mêmes places que severity.critical).
  "severity.warning": "Warning",
  // Sévérité d'un problème SEO : piste d'amélioration (mêmes places que severity.critical).
  "severity.opportunity": "Opportunity",
  // Sévérité d'un problème SEO : simple information.
  "severity.info": "Info",
  // Bandeau au-dessus de la grille, onglet Contenu. {max} = nombre de pages (2 000, écrit par l'application avec le séparateur de milliers de la langue). « near-duplicates » = terme SEO (pages quasi dupliquées), à garder ou traduire selon l'usage.
  "grid.near_dup_disabled": "Near-duplicate detection disabled: more than {max} pages (the O(n^2) computation would be too costly).",
  // Message de grille vide, onglet Sitemap, après un crawl.
  "grid.no_sitemap": "No sitemap found for this site.",
  // Message de grille vide, onglet Sitemap, avant tout crawl.
  "grid.sitemap_start": "Start a crawl to analyse the sitemap.",
  // Message de grille vide, onglet Sitemap, vue Orphelines.
  "grid.no_orphans": "No orphan URLs: every URL in the sitemap was reached by the crawl.",
  // Message de grille vide, onglet Sitemap, vue ou recherche sans résultat.
  "grid.sitemap_no_match": "No URL matches this view.",
  // Message de grille vide quand la recherche ou un filtre ne laisse aucune ligne.
  "grid.no_match": "No rows match the filter.",
  // Message de grille vide au démarrage de l'application.
  "grid.start": "Start a crawl to see the results here.",
  // Titre de groupe du panneau Overview (à droite de la grille).
  "overview.group_response": "Response Codes",
  // Ligne cliquable du panneau Overview (filtre la grille), suivie d'un compteur. Colonne étroite.
  "overview.resp_2xx": "Success (2xx)",
  // Ligne du panneau Overview, groupe Codes de réponse.
  "overview.resp_3xx": "Redirection (3xx)",
  // Ligne du panneau Overview, groupe Codes de réponse.
  "overview.resp_4xx": "Client Error (4xx)",
  // Ligne du panneau Overview, groupe Codes de réponse.
  "overview.resp_5xx": "Server Error (5xx)",
  // Titre de groupe du panneau Overview : balises <title>.
  "overview.group_titles": "Page Titles",
  // Ligne du groupe Titres : pages sans title.
  "overview.title_missing": "Missing",
  // Ligne du groupe Titres : title de plus de 60 caractères (« car. » = caractères).
  "overview.title_long": "> 60 chars",
  // Ligne du groupe Titres : title de moins de 30 caractères.
  "overview.title_short": "< 30 chars",
  // Ligne du groupe Titres : title identique à celui d'une autre page.
  "overview.title_dup": "Duplicate",
  // Titre de groupe du panneau Overview.
  "overview.group_meta": "Meta Description",
  // Ligne du groupe Meta Description : absente (masculin conservé en français ; accorder si besoin).
  "overview.meta_missing": "Missing",
  // Ligne du groupe Meta Description : plus de 160 caractères.
  "overview.meta_long": "> 160 chars",
  // Ligne du groupe Meta Description : moins de 70 caractères.
  "overview.meta_short": "< 70 chars",
  // Ligne du groupe Meta Description : identique à celle d'une autre page.
  "overview.meta_dup": "Duplicate",
  // Titre de groupe du panneau Overview : balises H1.
  "overview.group_h1": "H1",
  // Ligne du groupe H1 : pages sans H1.
  "overview.h1_missing": "Missing",
  // Ligne du groupe H1 : pages avec plusieurs H1.
  "overview.h1_multiple": "Multiple",
  // Ligne du groupe H1 : premier H1 identique à celui d'une autre page.
  "overview.h1_dup": "Duplicate",
  // Titre de groupe du panneau Overview.
  "overview.group_content": "Content",
  // Ligne du groupe Contenu : pages quasi dupliquées.
  "overview.near_dup": "Near Duplicates",
  // Titre de groupe du panneau Overview.
  "overview.group_images": "Images",
  // Ligne du groupe Images : pages ayant des images sans attribut alt.
  "overview.img_missing_alt": "Missing Alt Text",
  // Titre de groupe du panneau Overview (terme Screaming Frog conservé en français).
  "overview.group_canonicals": "Canonicals",
  // Ligne du groupe Canonicals : pas de canonical.
  "overview.canonical_missing": "Missing",
  // Ligne du groupe Canonicals : pages dont la canonical pointe ailleurs (féminin pluriel : « pages canonicalisées »).
  "overview.canonicalised": "Canonicalised",
  // Titre de groupe du panneau Overview.
  "overview.group_indexability": "Indexability",
  // Ligne du groupe Indexabilité.
  "overview.indexable": "Indexable",
  // Ligne du groupe Indexabilité.
  "overview.non_indexable": "Non-Indexable",
  // Titre de groupe du panneau Overview : forme des adresses.
  "overview.group_url": "URL",
  // Ligne du groupe URL : adresses avec des caractères non ASCII.
  "overview.url_non_ascii": "Non-ASCII",
  // Ligne du groupe URL : adresses de plus de 115 caractères (féminin pluriel : « URL longues »).
  "overview.url_long": "Long (>115)",
  // Titre de groupe du panneau Overview.
  "overview.group_security": "Security",
  // Ligne du groupe Sécurité : pages https qui chargent des ressources http.
  "overview.mixed_content": "Mixed Content",
  // Ligne du groupe Sécurité : en-têtes HTTP de sécurité absents.
  "overview.missing_headers": "Missing Headers",
  // Titre de groupe du panneau Overview.
  "overview.group_hreflang": "Hreflang",
  // Ligne du groupe Hreflang : une page visée ne renvoie pas de hreflang en retour.
  "overview.hreflang_no_return": "Missing Return Links",
  // Titre de groupe du panneau Overview.
  "overview.group_sitemap": "Sitemap",
  // Ligne du groupe Sitemap : toutes les URLs déclarées dans le sitemap.
  "overview.sitemap_all": "URLs in Sitemap",
  // Ligne du groupe Sitemap : URLs du sitemap jamais atteintes par le crawl (féminin pluriel).
  "overview.sitemap_orphans": "Orphan URLs",
  // Ligne du groupe Sitemap : pages crawlées absentes du sitemap.
  "overview.sitemap_not_in": "Not in Sitemap",
  // Texte de la pastille « Filtre : … » de la barre d'outils quand on a cliqué une ligne du panneau de synthèse : {group} = titre du groupe (Titres, Meta Description, H1, Canonicals…), {row} = la ligne cliquée (Manquant, Dupliqué, > 60 car.…). Sans le groupe, « Filtre : Manquant » ne disait pas de quoi. Le chevron « › » sépare les deux ; ordre et signe de votre langue. Court : la pastille partage la barre d'outils.
  "overview.filter_label": "{group} › {row}",
  // En-tête de colonne du sous-onglet Inlinks : page qui contient un lien vers la page sélectionnée.
  "detail.h_inlink_source": "Source (linking here)",
  // En-tête de colonne des sous-onglets Inlinks et Outlinks : texte d'ancre.
  "detail.h_anchor": "Anchor Text",
  // En-tête de colonne du sous-onglet Outlinks : URL visée par le lien.
  "detail.h_destination": "Destination",
  // En-tête de colonne du sous-onglet Outlinks : Interne / Externe.
  "detail.h_link_type": "Type",
  // Sous-onglet Inlinks vide.
  "detail.no_inlinks": "No inbound links found among the crawled pages.",
  // Sous-onglet Outlinks vide.
  "detail.no_outlinks": "No outbound links on this page.",
  // En-tête de colonne du sous-onglet Images sans alt : adresse de l'image. « src » = attribut HTML, ne pas traduire.
  "detail.h_image_src": "Image (src)",
  // En-tête de colonne du sous-onglet Images sans alt : nom du fichier de l'image.
  "detail.h_file": "File",
  // Sous-onglet Images sans alt : le crawl enregistré compte {n} images sans alt mais ne contient pas leur liste (ancien format). {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.alt_counted_no_urls": {"one":"{n} image without alt counted, but no URL was recorded: this crawl predates this version, run it again.","other":"{n} images without alt counted, but no URLs were recorded: this crawl predates this version, run it again."},
  // Sous-onglet Images sans alt vide.
  "detail.alt_all_ok": "Every image on this page has an alt attribute.",
  // Sous la liste des images sans alt. {missing} = nombre d'images sans alt (toujours ≥ 2) ; {n} = nombre d'URL distinctes (l'accord suit {n}). {missing} et {n} sont écrits par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.alt_more_than_urls": {"one":"{missing} without alt in total, {n} distinct URL: the same image repeated only counts as one row.","other":"{missing} without alt in total, {n} distinct URLs: the same image repeated only counts as one row."},
  // Aperçu SERP (sous-onglet SERP Snippet) quand la page n'a pas de title.
  "detail.serp_no_title": "(missing title)",
  // Aperçu SERP quand la page n'a pas de meta description.
  "detail.serp_no_meta": "(missing meta description)",
  // Note sous l'aperçu SERP.
  "detail.serp_note": "Indicative preview of how the page may look in Google results.",
  // Panneau Infos, ligne « Contenu mixte » : aucune ressource mixte (« aucun contenu mixte », masculin en français).
  "detail.no_mixed_content": "None",
  // Panneau Infos, ligne « Contenu mixte ». {n} = nombre de ressources http:// chargées par la page ; {list} = jusqu'à 5 de leurs adresses, séparées par « | ». {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.mixed_resources": {"one":"{n} resource: {list}","other":"{n} resources: {list}"},
  // Panneau Infos, ligne « En-têtes manquants » : aucun en-tête manquant (masculin en français).
  "detail.no_missing_headers": "None",
  // Panneau Infos, ligne « Erreurs de données structurées » (detail.k_sd_errors) : aucune erreur (féminin en français).
  "detail.no_sd_errors": "None",
  // Panneau Infos, ligne « Near-duplicates » : aucune page quasi dupliquée (féminin en français).
  "detail.no_near_dup": "None",
  // Panneau Infos, ligne « Near-duplicates ». {n} = nombre de pages quasi dupliquées ; {urls} = liens vers ces pages, séparés par « | ». {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.near_dup_pages": {"one":"{n} page: {urls}","other":"{n} pages: {urls}"},
  // Panneau Infos, ligne « Hreflang réciproque » : une page visée par un hreflang ne renvoie pas de hreflang vers celle-ci.
  "detail.hreflang_missing_label": "Missing return link",
  // {label} = « Lien retour manquant » (mis en évidence) ; {urls} = liens vers les pages concernées, séparés par « | ».
  "detail.hreflang_missing_list": "{label}: {urls}",
  // Panneau Infos, ligne « Hreflang réciproque » : tous les liens retour sont présents.
  "detail.hreflang_ok": "OK (reciprocal)",
  // Libellé de ligne du sous-onglet Infos / En-têtes du panneau de détail (colonne de gauche).
  "detail.k_address": "Address",
  // Libellé de ligne du panneau Infos. Même nom que col.status_code_full (un seul nom pour le code HTTP).
  "detail.k_http_code": "Status Code",
  // Libellé de ligne du panneau Infos : verdict de cache détaillé.
  "detail.k_cache": "Cache",
  // Libellé de ligne du panneau Infos : méthode de mesure (HTTP ou Chrome).
  "detail.k_probe": "Probe",
  // Libellé de ligne du panneau Infos : poids total transféré.
  "detail.k_total_size": "Total Size",
  // Libellé de ligne du panneau Infos : nombre de requêtes HTTP.
  "detail.k_requests": "Requests",
  // Valeur de la ligne « Requêtes » : {total} = requêtes, {dyn} = dont requêtes dynamiques. ATTENTION : {dyn} peut valoir 0, 1 ou plus et la phrase n'a qu'UNE forme : employez une tournure INVARIABLE (abréviation comme le français « dyn. », ou « libellé : nombre »), jamais un adjectif qui s'accorde en nombre (« 1 dinâmicos » est faux).
  "detail.requests_value": "{total} ({dyn} dynamic)",
  // Libellé de ligne du panneau Infos : cible de la redirection. Même nom que col.redirect_to (un seul nom pour la cible d'une redirection).
  "detail.k_redirect_to": "Redirect URL",
  // Libellé de ligne du panneau Infos.
  "detail.k_redirect_chain": "Redirect Chain",
  // Libellé de ligne du panneau Infos.
  "detail.k_canonical": "Canonical",
  // Libellé de ligne du panneau Infos : la canonical pointe vers une autre URL (Oui/Non).
  "detail.k_canonicalised": "Canonicalised",
  // Libellé de ligne du panneau Infos : contenu de la balise meta robots.
  "detail.k_meta_robots": "Meta Robots",
  // Libellé de ligne du panneau Infos (Oui/Non).
  "detail.k_indexable": "Indexable",
  // Libellé de ligne du panneau Infos : attribut lang de la page.
  "detail.k_language": "Language",
  // Libellé de ligne du panneau Infos : balise meta viewport.
  "detail.k_viewport": "Viewport",
  // Libellé de ligne du panneau Infos : encodage déclaré.
  "detail.k_charset": "Charset",
  // Libellé de ligne du panneau Infos : nombre de mots.
  "detail.k_words": "Word Count",
  // Libellé de ligne du panneau Infos : part du texte visible dans le HTML, en %.
  "detail.k_text_ratio": "Text to Code Ratio",
  // Libellé de ligne du panneau Infos ; la valeur est « N / M ».
  "detail.k_links_int_ext": "Internal / External Links",
  // Libellé de ligne du panneau Infos : technologies détectées. Même nom que col.tech_full (un seul nom par notion).
  "detail.k_techs": "Technologies",
  // Libellé de ligne du panneau Infos (Oui/Non/—).
  "detail.k_sd_valid": "Valid Structured Data",
  // Libellé de ligne du panneau Infos.
  "detail.k_sd_errors": "Structured Data Errors",
  // Libellé de ligne du panneau Infos.
  "detail.k_mixed_content": "Mixed Content",
  // Libellé de ligne du panneau Infos : en-têtes HTTP de sécurité absents.
  "detail.k_missing_headers": "Missing Headers",
  // Libellé de ligne du panneau Infos : erreur rencontrée sur la page.
  "detail.k_error": "Error",
  // Libellé de ligne du panneau Infos.
  "detail.k_response_time": "Response Time",
  // Libellé de ligne du panneau Infos : pages quasi dupliquées.
  "detail.k_near_dup": "Near Duplicates",
  // Libellé de ligne du panneau Infos : les pages visées par un hreflang renvoient-elles un hreflang.
  "detail.k_hreflang_recip": "Hreflang Return Links",
  // Sous-onglet Source quand la page n'a pas pu être analysée.
  "detail.source_no_seo": "(no SEO data: page in error or not HTML)",
  // Sous-onglet Données structurées. {types} = badges des types schema.org.
  "detail.sd_types_detected": "Detected types: {types}",
  // Sous-onglet Données structurées, sous les types détectés.
  "detail.sd_no_raw_block": "No raw JSON-LD block provided for this page.",
  // Sous-onglet Données structurées vide.
  "detail.sd_none": "No structured data (JSON-LD) on this page.",
  // Type affiché pour un bloc JSON-LD illisible.
  "detail.sd_invalid_json": "(invalid JSON)",
  // En-tête d'un bloc JSON-LD (numéro {n}) dans le sous-onglet Données structurées. {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.sd_block": "Block {n}",
  // Même en-tête suivi du type du bloc : {n} = numéro du bloc JSON-LD (1, 2…) ; {type} = type schema.org du bloc, ou « (JSON invalide) ». {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "detail.sd_block_typed": "Block {n} — {type}",
  // Barre d'état (en bas à gauche) : aucun crawl en cours.
  "status.idle": "Idle",
  // Barre d'état pendant un crawl. {url} = adresse en cours (tronquée).
  "status.crawling": "Crawling: {url}",
  // Barre d'état. {error} = message d'erreur du moteur (souvent en anglais technique) ou « inconnue » (status.unknown).
  "status.error": "Error: {error}",
  // Remplace {error} dans « Erreur : {error} » quand l'erreur est vide (« erreur inconnue », féminin en français).
  "status.unknown": "unknown",
  // Remplace {error} dans « Import impossible : {error} » quand l'erreur est vide.
  "status.unknown_error": "unknown error",
  // Barre d'état au lancement d'un crawl.
  "status.starting": "Starting...",
  // Barre d'état à la reprise d'un crawl en pause.
  "status.resuming": "Resuming crawl...",
  // Barre d'état en fin de crawl. {path} = chemin du fichier projet enregistré.
  "status.saved": "Saved: {path}",
  // Barre d'état en fin de série multi-sites. {sites} = « N site(s) » (status.n_sites) et {pages} = « N page(s) » (status.n_pages), déjà accordés.
  "status.done_sites": "Done — {sites}, {pages}.",
  // Barre d'état à la fin d'un crawl unique (un seul site). {pages} = « N page(s) » (status.n_pages), déjà accordé. Remplace l'ancien « Terminé — 1 run(s), N page(s). » : la répétition des crawls n'existe plus.
  "status.done": "Done — {pages}.",
  // Groupe nominal inséré dans status.done_sites : {n} = nombre de sites crawlés (avec le séparateur de milliers de la langue).
  "status.n_sites": {"one":"{n} site","other":"{n} sites"},
  // Groupe nominal inséré dans status.done_sites et status.done : {n} = nombre de pages (avec le séparateur de milliers de la langue).
  "status.n_pages": {"one":"{n} page","other":"{n} pages"},
  // Barre d'état : un crawl enregistré est rouvert pour consultation (sans recrawler). {url} = adresse du site ; {n} = nombre de pages enregistrées (séparateur de milliers de la langue).
  "status.viewing": {"one":"Viewing: {url} — {n} page","other":"Viewing: {url} — {n} pages"},
  // Même chose avec la date d'enregistrement : {url} = adresse du site ; {n} = nombre de pages enregistrées (séparateur de milliers de la langue) ; {date} = date écrite par l'application au format de la langue (13/11/2026 en français et en anglais britannique, 13.11.2026 en allemand, 2026/11/13 en japonais) : ne pas la mettre en forme vous-même.
  "status.viewing_dated": {"one":"Viewing: {url} — {n} page (saved on {date})","other":"Viewing: {url} — {n} pages (saved on {date})"},
  // Barre d'état : la liste d'URL importée a été retirée.
  "status.list_removed": "List removed.",
  // Barre d'état après l'import d'un fichier sans URL valide.
  "status.import_empty": "No usable URL in this file.",
  // Barre d'état après l'import d'une liste d'URL : {n} = nombre d'URL importées (séparateur de milliers de la langue). Le bouton de lancement est btn.start (« Démarrer » / « Start ») : écrivez le libellé de ce bouton dans votre langue.
  "status.imported": {"one":"{n} URL imported — click Start to run the series.","other":"{n} URLs imported — click Start to run the series."},
  // Barre d'état. {error} = message du système ou « erreur inconnue » (status.unknown_error).
  "status.import_failed": "Import failed: {error}",
  // Barre d'état après le bouton Stop (le crawl est en pause, les données sont gardées) : {n} = nombre de pages déjà collectées.
  "status.paused": {"one":"Paused — {n} page collected.","other":"Paused — {n} pages collected."},
  // Barre d'état après le choix du dossier des projets. {path} = chemin.
  "status.save_dir": "Save folder: {path}",
  // Barre d'état après l'ouverture d'un fichier projet illisible.
  "status.bad_project": "Invalid or empty project file.",
  // Barre d'état pendant une série multi-sites. {i}/{total} = rang du site ; {host} = domaine ; {bytes} = poids cumulé formaté ; {pages} = pages cumulées (l'accord suit {pages} ; le français écrit toujours « pages »).
  "status.multi_site": {"one":"Site {i}/{total} · {host} — total {bytes} ({pages} page)","other":"Site {i}/{total} · {host} — total {bytes} ({pages} pages)"},
  // Bouton principal de la barre du haut : lance un crawl. Court.
  "btn.start": "Start",
  // Même bouton quand un crawl est en pause. Court.
  "btn.resume": "Resume",
  // Infobulle du bouton Démarrer.
  "btn.start_title": "Start a new crawl",
  // Infobulle du bouton Reprendre.
  "btn.resume_title": "Resume the paused crawl (without clearing the data already collected)",
  // Bouton qui met le crawl en pause (le français affiche « Stop »). Court.
  "btn.stop": "Stop",
  // Infobulle du bouton Stop.
  "btn.stop_title": "Pause (collected data is kept)",
  // Bouton qui efface toutes les données du crawl. Court.
  "btn.clear": "Clear",
  // Infobulle du bouton Vider.
  "btn.clear_title": "Clear all data and reset",
  // Bouton de la barre de sélection : copie les valeurs sélectionnées. Court.
  "sel.copy": "Copy",
  // Même bouton pendant 1,6 s après une copie réussie. Court.
  "sel.copied": "Copied!",
  // Même bouton après un échec de copie. Très court.
  "sel.copy_failed": "Failed",
  // Infobulle du bouton Copier.
  "sel.copy_title": "Copy the selected values, one per line (Ctrl+C or right-click)",
  // Message bref (2 s) en bas de la fenêtre après une copie : {n} = nombre de valeurs copiées (séparateur de milliers de la langue) ; {col} = nom de la colonne copiée.
  "toast.copied": {"one":"{n} value copied — {col} column","other":"{n} values copied — {col} column"},
  // Message bref (2 s) quand le presse-papiers a refusé la copie.
  "toast.copy_failed": "Copy failed",
  // Titre du dialogue affiché quand on lance un crawl sur un site déjà enregistré.
  "dialog.saved_title": "Crawl already saved",
  // Texte du même dialogue : {url} = adresse du site (en gras) ; {n} = nombre de pages enregistrées (séparateur de milliers de la langue). Le français tutoie (« veux-tu ») : registre familier voulu par l'auteur, à garder si la langue le permet.
  "dialog.saved_text": {"one":"The site {url} has already been crawled ({n} page saved). What would you like to do?","other":"The site {url} has already been crawled ({n} pages saved). What would you like to do?"},
  // Même texte avec la date du crawl : {url} = adresse du site (en gras) ; {n} = nombre de pages enregistrées (séparateur de milliers de la langue) ; {date} = date écrite par l'application au format de la langue (13/11/2026 en français et en anglais britannique, 13.11.2026 en allemand, 2026/11/13 en japonais) : ne pas la mettre en forme vous-même.
  "dialog.saved_text_dated": {"one":"The site {url} was already crawled on {date} ({n} page saved). What would you like to do?","other":"The site {url} was already crawled on {date} ({n} pages saved). What would you like to do?"},
  // Bouton du dialogue « Crawl déjà enregistré ».
  "dialog.cancel": "Cancel",
  // Bouton du dialogue : rouvrir le crawl enregistré sans recrawler.
  "dialog.review": "Review this saved crawl",
  // Bouton principal du dialogue : recrawler le site.
  "dialog.new_crawl": "New crawl",
  // Alerte quand la page est ouverte hors de l'application (Tauri = nom propre).
  "dialog.no_tauri": "Tauri interface unavailable. Launch the app through Tauri.",
  // Entrée de la liste d'autocomplétion du champ d'adresse (barre du haut) pour une SÉRIE de sites crawlée depuis une liste importée : {n} = nombre de sites de la série ; {host} = domaine du premier site. Choisir l'entrée recopie ce texte dans le champ (ce n'est pas une adresse). Le français écrit toujours « sites », même pour 1 (texte d'origine). {n} est écrit par l'application avec le séparateur de milliers de la langue (1 234).
  "history.site_list": {"one":"{n} site ({host}…)","other":"{n} sites ({host}…)"},
  // Pastille à côté du bouton « Importer une liste » : {n} = nombre de sites de la liste importée (séparateur de milliers de la langue).
  "import.badge": {"one":"{n} site","other":"{n} sites"},
  // Infobulle de la croix de la pastille de liste importée.
  "import.remove_title": "Remove the list",
  // Texte de la pastille IP (barre du haut, à droite) en attendant la réponse. Très court.
  "netinfo.pending": "IP...",
  // Infobulle de la pastille IP (barre du haut, à droite) avant la réponse ou en cas d'échec. « reverse DNS » = terme technique.
  "netinfo.title_pending": "Outgoing public IP + reverse DNS",
  // Infobulle de la pastille IP. {ip} = adresse IP ; {rev} = nom d'hôte inverse.
  "netinfo.title_rev": "Outgoing public IP {ip} — reverse DNS: {rev}",
  // Infobulle de la pastille IP quand l'IP n'a pas de nom d'hôte inverse : {ip} = adresse IP publique sortante.
  "netinfo.title_no_rev": "Outgoing public IP {ip} — no reverse DNS",
  // Texte de la pastille IP quand l'adresse n'a pas pu être obtenue. Court.
  "netinfo.unavailable": "IP unavailable",
  // Texte affiché DANS le champ « Pages max » quand « Tout le site » est coché. {max} = plafond formaté (« 200 000 »). Le champ fait environ 15 caractères de large (police à chasse fixe ; l'agent interface l'élargit un peu hors français) : garder « ∞ (… {max}) » le plus court possible, abréviation de « max » admise.
  "maxpages.locked_value": "∞ (max {max})",
  // Infobulle du champ « Pages max » verrouillé. {max} = plafond formaté. « Tout le site » = nom de la case à cocher de la barre du haut (topbar.unlimited, « Entire site » en anglais) : reprendre sa traduction. \n = retour à la ligne : le garder.
  "maxpages.locked_title": "Entire site: no page limit. A built-in safeguard stops the crawl at {max} pages (protection against faceted or calendar sites, where the number of URLs is nearly infinite).\nClick to set a limit yourself.",
  // Infobulle du champ « Pages max ».
  "maxpages.title": "Maximum number of pages to crawl.",
  // Nom du fichier CSV « Export CSV » (suivi de la date et de .csv). {tab} = identifiant technique de l'onglet, en anglais, à garder tel quel. Pas de / \ : * ? " < > | dans un nom de fichier.
  "export.file_tab": "crawl-{tab}",
  // Nom du fichier CSV « Tous les liens » (suivi de la date et de .csv). Minuscules et tirets, sans / \ : * ? " < > |.
  "export.file_links": "all-links",
  // Nom du fichier CSV « Toutes les images » (mêmes règles que export.file_links).
  "export.file_images": "all-images",
  // Nom du fichier CSV « Tous les problèmes » (mêmes règles que export.file_links).
  "export.file_issues": "all-issues",
  // En-tête de colonne du CSV « Tous les liens » (minuscules) : page qui contient le lien.
  "export.h_source": "source",
  // En-tête de colonne du CSV « Tous les liens » : URL visée par le lien (mot anglais dans le fichier français).
  "export.h_target": "target",
  // En-tête de colonne du CSV « Tous les liens » : texte d'ancre (mot anglais dans le fichier français).
  "export.h_anchor": "anchor",
  // En-tête de la dernière colonne du CSV « Tous les liens » (minuscules) : elle contient Interne / Externe (cell.internal / cell.external). Même nom que col.link_type_full.
  "export.h_link_type": "link type",
  // En-tête de l'export CSV d'un onglet, pour une colonne qui contient des nombres BRUTS en millisecondes : {label} = libellé complet de la colonne (col.…_full), suivi de l'unité entre parenthèses, ex. « Temps de réponse (ms) » (onglets Internal et Bande passante). Dans le CSV seulement (l'infobulle de l'en-tête garde le libellé seul). Ordre et ponctuation de votre langue ; unité en toutes lettres ou en symbole d'usage.
  "export.unit_ms": "{label} (ms)",
  // En-tête de l'export CSV d'un onglet, pour une colonne qui contient des nombres BRUTS en octets : {label} = libellé complet de la colonne (col.…_full), suivi de l'unité entre parenthèses, ex. « Poids total (octets) », « Poids des images (octets) » (onglet Bande passante) ; même mot que export.h_bytes. Dans le CSV seulement (l'infobulle de l'en-tête garde le libellé seul). Ordre et ponctuation de votre langue ; unité en toutes lettres ou en symbole d'usage.
  "export.unit_bytes": "{label} (bytes)",
  // En-tête de l'export CSV d'un onglet, pour une colonne qui contient des nombres BRUTS en caractères : {label} = libellé complet de la colonne (col.…_full), suivi de l'unité entre parenthèses, ex. « Longueur du title (caractères) » (onglets URL, Page Titles, Meta Description). Dans le CSV seulement (l'infobulle de l'en-tête garde le libellé seul). Ordre et ponctuation de votre langue ; unité en toutes lettres ou en symbole d'usage.
  "export.unit_chars": "{label} (characters)",
  // En-tête de colonne du CSV « Toutes les images » : page qui contient l'image.
  "export.h_page": "page",
  // En-tête de colonne du CSV « Toutes les images » : type de ressource donné par le moteur (mot anglais dans le fichier français).
  "export.h_kind": "kind",
  // En-tête de colonne du CSV « Toutes les images » : poids en octets.
  "export.h_bytes": "bytes",
  // En-tête de colonne du CSV « Tous les problèmes » (minuscules).
  "export.h_severity": "severity",
  // En-tête de colonne du CSV « Tous les problèmes ».
  "export.h_category": "category",
  // En-tête de colonne du CSV « Tous les problèmes » : URL de la page.
  "export.h_address": "address",
  // En-tête de colonne du CSV « Tous les problèmes » : description du problème.
  "export.h_message": "message",
  // Nom du fichier CSV du rapport « Chaînes de redirection » (suivi de la date et de .csv). Minuscules et tirets, sans / \ : * ? " < > |.
  "report.file_redirects": "redirect-chains-report",
  // Nom du fichier CSV du rapport « Canonicals » (mêmes règles).
  "report.file_canonicals": "canonicals-report",
  // Nom du fichier CSV du rapport « Pages non-indexables » (mêmes règles).
  "report.file_non_indexable": "non-indexable-report",
  // Nom du fichier CSV du rapport « Contenu mixte » (mêmes règles).
  "report.file_mixed_content": "mixed-content-report",
  // En-tête de colonne des CSV du menu Rapports (minuscules) : URL de la page.
  "report.h_url": "url",
  // En-tête de colonne du rapport « Chaînes de redirection » : la chaîne d'URL.
  "report.h_chain": "chain",
  // En-tête de colonne du rapport « Chaînes de redirection » : URL finale.
  "report.h_destination": "destination",
  // En-tête de colonne du rapport « Canonicals ».
  "report.h_canonical": "canonical",
  // En-tête de colonne du rapport « Canonicals » (minuscules) : Self / Autre / Manquant. Même nom que col.canonical_type_full.
  "report.h_type": "canonical type",
  // En-tête de colonne du rapport « Pages non-indexables ».
  "report.h_reason": "reason",
  // En-tête de colonne du rapport « Contenu mixte » : ressources chargées en http://.
  "report.h_http_resources": "http resources",
  // Raison de non-indexabilité (rapport Pages non-indexables), minuscules ; plusieurs raisons sont séparées par « ; ».
  "report.reason_canonicalised": "canonicalised",
  // Raison de non-indexabilité (rapport Pages non-indexables), en minuscules : la page a répondu avec un code HTTP hors 2xx ({code} = 301, 404, 500…). Neutre : une redirection ou une page absente n'est pas « bloquée ». Même nom que col.status_code_full (« Code HTTP » / « Status Code »). Plusieurs raisons sont séparées par « ; ».
  "report.reason_status_code": "status code {code}",
  // Raison de non-indexabilité (rapport Pages non-indexables), en minuscules : la page n'a pas répondu du tout (code 0 : bloquée par robots.txt, erreur réseau, délai dépassé).
  "report.reason_no_response": "no HTTP response",
  // Raison de non-indexabilité : page non HTML ou en erreur.
  "report.reason_not_html": "non-HTML / error",
  // Raison de non-indexabilité quand aucune autre n'est connue.
  "report.reason_non_indexable": "non-indexable",
  // Message de problème (onglet Problèmes, catégorie Hreflang) : la page déclare un hreflang vers chacune des {urls}, mais ces pages ne déclarent pas de hreflang en retour vers elle. {urls} = adresses séparées par « , ».
  "jsissue.hreflang_no_return": "Missing hreflang return link from: {urls}",

  // ----- cles-moteur.json -----
  // Problème SEO critique, onglet Problèmes (colonne Message) et export CSV des problèmes. La page renvoie un code HTTP d'erreur. {status} = ce code (404, 500…), jamais formaté.
  "issue.status_error": "Response code {status}",
  // Problème SEO (avertissement), onglet Problèmes, colonne Message. La page redirige ailleurs. {status} = code HTTP de redirection (301, 302, 307, 308), jamais formaté.
  "issue.redirect": "Redirect {status}",
  // Problème SEO, onglet Problèmes, colonne Message. La page n'a pas de balise HTML <title>. Le français garde le mot anglais « Title » (jargon SEO, comme Screaming Frog) : gardez-le ou prenez le terme SEO courant de votre langue.
  "issue.title_missing": "Missing title",
  // Problème SEO, colonne Message. Le <title> dépasse 60 caractères. {len} = sa longueur en caractères. « car. » abrège « caractères » : gardez une abréviation courte.
  "issue.title_too_long": "Title too long ({len} chars)",
  // Problème SEO (opportunité), colonne Message. Le <title> fait moins de 30 caractères. {len} = sa longueur en caractères.
  "issue.title_too_short": "Title too short ({len})",
  // Problème SEO, colonne Message. La balise <meta name=description> est absente. « Meta description » reste en anglais en français (jargon SEO).
  "issue.meta_desc_missing": "Missing meta description",
  // Problème SEO, colonne Message. La meta description dépasse 160 caractères. {len} = sa longueur en caractères.
  "issue.meta_desc_too_long": "Meta description too long ({len})",
  // Problème SEO (opportunité), colonne Message. La meta description fait moins de 70 caractères. {len} = sa longueur en caractères.
  "issue.meta_desc_too_short": "Meta description too short ({len})",
  // Problème SEO, colonne Message. La page n'a pas de titre principal <h1>. « H1 » ne se traduit pas.
  "issue.h1_missing": "Missing H1",
  // Problème SEO, colonne Message. La page a plusieurs balises <h1> alors qu'il en faut une seule. {n} = leur nombre (2 ou plus) ; formes plurielles selon {n}. « H1 » ne se traduit pas.
  "issue.h1_multiple": {"one":"{n} H1 tag (should be unique)","other":"{n} H1 tags (should be unique)"},
  // Problème SEO (opportunité), colonne Message. {n} = nombre d'images de la page dont l'attribut alt est absent ou vide ; formes plurielles selon {n} (le français garde « image(s) », texte historique). « alt » = nom d'attribut HTML, ne pas traduire.
  "issue.images_missing_alt": {"one":"{n} image without alt text","other":"{n} images without alt text"},
  // Problème SEO, colonne Message. La page porte une directive meta robots « noindex » : les moteurs ne l'indexent pas. « noindex » ne se traduit pas.
  "issue.noindex": "Noindex page",
  // Problème SEO (opportunité), colonne Message. Pas de balise <link rel=canonical>. « Canonical » reste en anglais en français (jargon SEO).
  "issue.canonical_missing": "Missing canonical",
  // Problème SEO, colonne Message. La balise canonical pointe vers une AUTRE URL : la page se déclare non indexable. Terme de Screaming Frog (« Canonicalised », orthographe britannique).
  "issue.canonicalised": "Canonicalised page (non-indexable)",
  // Problème SEO, colonne Message. L'en-tête HTTP X-Robots-Tag contient « noindex ». Nom d'en-tête et directive : ne se traduisent pas (texte en principe identique dans toutes les langues).
  "issue.x_robots_noindex": "X-Robots-Tag noindex",
  // Information SEO, colonne Message. La page porte <link rel=next> ou <link rel=prev> : elle fait partie d'une série paginée. Ce n'est pas une erreur.
  "issue.paginated": "Paginated page",
  // Problème SEO, colonne Message. Les données structurées JSON-LD de la page sont invalides. {detail} = la première erreur trouvée, DÉJÀ traduite (clés sd.*, ex. « Bloc 1: @type manquant ») : gardez les parenthèses autour.
  "issue.sd_invalid": "Invalid structured data ({detail})",
  // Problème SEO, colonne Message. Page en HTTPS qui charge {n} ressource(s) en http:// non sécurisé (« mixed content »). Formes plurielles selon {n} ; le français écrit « ressources » même pour 1 (texte historique).
  "issue.mixed_content": {"one":"Mixed content ({n} HTTP resource)","other":"Mixed content ({n} HTTP resources)"},
  // Information SEO, colonne Message. Un en-tête HTTP de sécurité recommandé est absent. {header} = son nom (Content-Security-Policy, X-Frame-Options, Referrer-Policy…), jamais traduit. Le français colle les deux-points (texte historique) : suivez la typographie de votre langue.
  "issue.security_header_missing": "Missing security header: {header}",
  // Information SEO, colonne Message. L'adresse de la page contient des caractères hors ASCII (lettres accentuées, etc.).
  "issue.url_non_ascii": "URL with non-ASCII characters",
  // Problème SEO (opportunité), colonne Message. L'adresse dépasse 115 caractères. {len} = sa longueur en caractères.
  "issue.url_too_long": "URL too long ({len})",
  // Information SEO, colonne Message. L'adresse de la page contient des lettres majuscules.
  "issue.url_uppercase": "URL with uppercase letters",
  // Information SEO, colonne Message. L'adresse contient des tirets bas « _ » ; le français garde le mot anglais « underscores ».
  "issue.url_underscores": "URL with underscores",
  // Catégorie de problème : colonne « Catégorie » de l'onglet Problèmes (colonne étroite, un ou deux mots) et export CSV. Regroupe les codes de réponse HTTP (erreurs 4xx/5xx, redirections 3xx).
  "cat.response": "Response",
  // Catégorie de problème (colonne étroite) : problèmes de balise <title> de la page.
  "cat.titles": "Titles",
  // Catégorie de problème (colonne étroite) : problèmes de meta description. Jargon SEO gardé en anglais en français.
  "cat.meta_description": "Meta description",
  // Catégorie de problème (colonne étroite) : titre principal <h1>. Ne se traduit pas.
  "cat.h1": "H1",
  // Catégorie de problème (colonne étroite) : images de la page (attribut alt).
  "cat.images": "Images",
  // Catégorie de problème (colonne étroite) : directives d'indexation pour les robots (noindex en meta robots ou en-tête X-Robots-Tag).
  "cat.directives": "Directives",
  // Catégorie de problème (colonne étroite) : balise <link rel=canonical>. Jargon SEO gardé en anglais en français.
  "cat.canonical": "Canonical",
  // Catégorie de problème (colonne étroite) : pages d'une série paginée (rel=next / rel=prev).
  "cat.pagination": "Pagination",
  // Catégorie de problème (colonne étroite) : données structurées JSON-LD (schema.org).
  "cat.structured_data": "Structured data",
  // Catégorie de problème (colonne étroite) : contenu mixte et en-têtes HTTP de sécurité.
  "cat.security": "Security",
  // Catégorie de problème (colonne étroite) : forme de l'adresse de la page (longueur, majuscules, caractères).
  "cat.url": "URL",
  // Catégorie de problème (colonne étroite) : liens hreflang entre versions linguistiques d'une page. Nom d'attribut HTML, en principe non traduit.
  "cat.hreflang": "Hreflang",
  // Erreur d'une page, panneau de détail (ligne « Erreur »). L'adresse est interdite par le fichier robots.txt du site : elle n'a pas été explorée. « robots.txt » ne se traduit pas.
  "err.robots_blocked": "Blocked by robots.txt",
  // Erreur d'une page, panneau de détail (ligne « Erreur »). La requête HTTP vers la page n'a pas abouti (site injoignable, réponse illisible). {detail} = message technique du système, en anglais (ex. « error sending request for url (…) »), jamais traduit. Le français commence par une minuscule et colle les deux-points (texte historique).
  "err.http_probe_failed": "HTTP probe failed: {detail}",
  // Erreur d'une page, panneau de détail. Chrome n'a pas pu démarrer : la page a été mesurée par une simple requête HTTP à la place. {detail} = la raison, déjà traduite quand c'est un texte de l'application (err.chrome_not_found, err.chrome_config_failed), sinon message technique tel quel.
  "err.chrome_launch_failed": "Could not launch Chrome, fell back to static HTTP: {detail}",
  // Erreur d'une page, panneau de détail. La mesure de la page dans Chrome a échoué : elle a été mesurée par une simple requête HTTP à la place. {detail} = l'étape qui a échoué, déjà traduite (err.chrome_new_page, err.chrome_subscribe) ou nom technique (« Network.enable »).
  "err.chrome_measure_failed": "Chrome measurement failed, fell back to static HTTP: {detail}",
  // Erreur d'une page, panneau de détail. Chrome n'a pas pu charger la page. {detail} = message technique de Chrome, en anglais, jamais traduit.
  "err.chrome_navigation_failed": "Navigation failed: {detail}",
  // Détail inséré à la fin de err.chrome_launch_failed (après les deux-points) : d'où la minuscule initiale. Chrome et Chromium sont des noms de logiciels.
  "err.chrome_not_found": "browser launch failed (Chrome/Chromium not found?)",
  // Détail inséré à la fin de err.chrome_launch_failed : minuscule initiale. BrowserConfig = nom technique, ne pas traduire. {detail} = message technique tel quel.
  "err.chrome_config_failed": "could not build BrowserConfig: {detail}",
  // Détail inséré à la fin de err.chrome_measure_failed : l'étape qui a échoué. Minuscule initiale.
  "err.chrome_new_page": "opening a new Chrome page",
  // Détail inséré à la fin de err.chrome_measure_failed : abonnement à un événement du protocole Chrome DevTools. {event} = nom technique (EventResponseReceived, EventLoadingFinished), jamais traduit. Minuscule initiale.
  "err.chrome_subscribe": "subscribing to {event}",
  // Colonne « Servi par » (étroite), infobulle et détail du cache : aucun cache n'a servi la page, c'est le serveur d'origine qui a répondu. Un seul mot, en minuscule comme en français.
  "cache.origin": "origin",
  // Colonne « Servi par » (étroite) : un cache HTTP intermédiaire non identifié a servi la page (déduit de l'en-tête Age). Très court.
  "cache.http_cache": "HTTP cache",
  // Infobulle et détail du cache, après « Cloudflare edge : ». La page est hébergée sur le réseau Cloudflare (Pages / Workers) : il n'existe pas de serveur d'origine. « cf-cache-status » = nom d'en-tête HTTP, ne pas traduire. Minuscule initiale.
  "cache.edge_no_origin": "served by the network, no origin (no cf-cache-status)",
  // Infobulle et détail du cache, après « WP Rocket : ». WP Rocket a été reconnu à sa signature dans le code HTML de la page, faute d'en-tête HTTP.
  "cache.html_signature": "HTML signature",
  // Infobulle et détail du cache, après « WP Rocket : ». « cached@ » = marqueur de débogage de WP Rocket, ne pas traduire. {age} = depuis combien de SECONDES la page est en cache ; « s » = secondes.
  "cache.wp_rocket_age": "cached@ {age} s ago",
  // Erreur de données structurées : onglet Structured Data (infobulle de la colonne Erreurs, export CSV), panneau de détail, et entre parenthèses dans issue.sd_invalid. {block} = numéro du bloc JSON-LD dans la page (1, 2…). « @context » = mot-clé JSON-LD, ne pas traduire. Le français colle les deux-points (texte historique).
  "sd.context_missing": "Block {block}: missing @context",
  // Erreur de données structurées (mêmes emplacements que sd.context_missing). {block} = numéro du bloc JSON-LD. « @type » = mot-clé JSON-LD, ne pas traduire.
  "sd.type_missing": "Block {block}: missing @type",
  // Erreur de données structurées (mêmes emplacements que sd.context_missing) : le bloc n'est pas du JSON lisible. {block} = numéro du bloc JSON-LD.
  "sd.invalid_json": "Block {block}: invalid JSON",
  // Repli quand des données structurées sont invalides sans erreur précise : apparaît seul entre parenthèses dans issue.sd_invalid. Minuscule.
  "sd.unknown": "error",
  // Dialogue Windows « Ouvrir » d'un projet enregistré : nom du filtre de fichiers (*.json), dans la liste des types. Court.
  "engine.dialog_project": "Crawl project",
  // Dialogue Windows d'import d'une liste d'adresses à crawler : nom du filtre (*.txt, *.csv, *.list). Court.
  "engine.dialog_url_list": "URL list",
  // Même dialogue d'import : filtre qui affiche tous les fichiers (*). Libellé habituel du système dans votre langue.
  "engine.dialog_all_files": "All files",
  // Barre d'état (en bas, à gauche), pendant l'ouverture d'un crawl enregistré : quelques secondes pour un très gros crawl. Dit combien de pages du fichier ont déjà été lues ; le nombre grandit en continu, puis le texte est remplacé par « Consultation : … » quand tout est lu. {n} = ce nombre, déjà mis en forme (séparateur de milliers) ; formes plurielles selon ce nombre. « page lue » : page du fichier chargée dans la fenêtre, pas une page du site visitée à nouveau.
  "engine.opening_saved": {"one":"Opening the saved crawl… {n} page read","other":"Opening the saved crawl… {n} pages read"},
  // Barre d'état (en bas, à gauche), à la fin d'un crawl volumineux, pendant que le fichier du crawl s'écrit sur le disque (quelques secondes ; quelques dizaines de secondes pour un très gros crawl). {pct} = pourcentage déjà écrit, de 0 à 99 (nombre entier) ; remplacé à la fin par « Enregistré : <chemin du fichier> ». Pour « % », suivez la typographie de votre langue (le français écrit « 42 % », comme la clé unit.percent).
  "engine.saving": "Saving the crawl… {pct}%",
  // Encadré rouge en bas à droite, qui reste affiché jusqu'au clic (avec les autres échecs d'enregistrement de la séance, un par ligne) ; aussi dans la barre d'état (en bas, à gauche) si le crawl affiché est celui-là. Le fichier du crawl n'a pas pu être écrit sur le disque (disque plein, dossier interdit en écriture…). {error} = la raison, telle que la donne le système (souvent déjà dans la langue de Windows), jamais traduite. L'ancien fichier du même site, s'il existait, reste intact.
  "engine.save_failed": "Could not save the crawl: {error}",
  // Encadré rouge en bas à droite, qui reste affiché jusqu'au clic (avec les autres échecs d'enregistrement de la séance, un par ligne) ; aussi dans la barre d'état (en bas, à gauche) si le crawl affiché est celui-là. Le crawl est entièrement écrit, mais n'a pas pu REMPLACER le fichier précédent du même site (souvent : un antivirus, un outil de synchronisation ou un autre programme tient ce fichier ouvert). Il est gardé dans un autre fichier, à côté, que « Ouvrir un projet » sait ouvrir. {error} = la raison donnée par le système, jamais traduite ; {path} = le chemin complet de ce fichier (par exemple C:\…\crawls\https___site.fr_.20261114T021507Z.json), jamais traduit.
  "engine.save_kept": "Could not replace the previous file: {error} — the crawl was kept in {path}",
  // Première ligne de l'encadré de fermeture (en bas à droite, gris, il disparaît seul à la fin) : l'utilisateur a demandé à fermer la fenêtre pendant un crawl ou pendant que le crawl s'enregistrait. Un crawl en cours est mis en pause, puis enregistré ; la fenêtre attend la fin de l'enregistrement (de quelques secondes à quelques minutes pour un très gros crawl), puis se ferme toute seule. {pct} = pourcentage déjà enregistré, entier de 0 à 99. La seconde ligne de l'encadré est engine.closing_warning. Pour « % », suivez la typographie de votre langue (comme engine.saving).
  "engine.closing_after_save": "Saving the crawl before closing… {pct}%",
  // Seconde ligne de l'encadré de fermeture, en gras, sous engine.closing_reading, engine.closing_after_save ou engine.closing_waiting : une seconde demande de fermeture ferme TOUT DE SUITE, et le crawl n'est alors pas enregistré. C'est ce qui évite de perdre le crawl en recliquant sur une fenêtre qui paraît ne rien faire : à rendre clairement.
  "engine.closing_warning": "Closing the window again quits without saving the crawl.",
  // Infobulle du bouton « Ouvrir un projet » (volet des réglages, à côté de « Dossier des projets »), quand il est grisé parce qu'un crawl tourne : ouvrir un projet remplacerait le crawl en cours, qui serait perdu. En pause, le bouton redevient actif (le crawl a été enregistré à la pause).
  "engine.open_project_busy": "Not available while a crawl is running: pause it (it is then saved) or wait until it finishes.",
  // Ajouté au TITRE de la fenêtre (la barre de titre de Windows, après « SEO Spider — »), dès que l'utilisateur demande à fermer la fenêtre. C'est le premier signe, immédiat, avant même que la fenêtre ait lu les pages déjà reçues (après un crawl très rapide, cela peut durer longtemps) ; l'encadré de fermeture (engine.closing_reading, engine.closing_waiting, engine.closing_after_save) prend ensuite le relais. Le titre redevient normal si la fermeture est annulée. Commence par une minuscule (il suit un tiret) ; la fin prévient qu'une seconde demande de fermeture quitte SANS enregistrer : à garder, et à dire sans équivoque — pas de verbe « annuler » qui pourrait se lire « annule la fermeture » (c'est le mot du bouton Annuler de Windows) : dire que fermer de nouveau n'enregistre pas. Viser 50 signes au plus : avec « SEO Spider — » le titre fait déjà 61 à 65 signes en anglais et en français, et la barre de titre d'une fenêtre étroite en tient environ 75 ; Windows coupe un titre trop long par la fin.
  "engine.closing_title": "saving before closing… closing again won't save",
  // Première ligne de l'encadré de fermeture (en bas à droite, gris), dès que l'utilisateur a demandé à fermer la fenêtre : celle-ci lit d'abord les pages du crawl qu'elle a déjà reçues mais pas encore traitées (après un crawl très rapide, il peut y en avoir des milliers), puis les enregistre (« Enregistrement du crawl avant fermeture… N % »). {n} = nombre de pages qu'il lui reste à lire, déjà mis en forme ; formes plurielles selon ce nombre. La seconde ligne de l'encadré est engine.closing_warning.
  "engine.closing_reading": {"one":"Closing: reading the pages already received… {n} page left","other":"Closing: reading the pages already received… {n} pages left"},
  // Première ligne de l'encadré de fermeture (en bas à droite, gris), pendant la fermeture de la fenêtre : la fermeture a mis en pause un crawl qui tournait, et celui-ci termine les quelques pages qu'il était en train de lire (quelques secondes) avant qu'elles soient enregistrées avec le reste ; ensuite vient « Enregistrement du crawl avant fermeture… N % ». La seconde ligne de l'encadré est engine.closing_warning.
  "engine.closing_waiting": "Closing: the crawl is finishing its current pages…",
  // Ligne de l'encadré rouge des échecs d'enregistrement (en bas à droite, gardé jusqu'au clic), juste sous le message de l'échec : l'utilisateur avait demandé à fermer la fenêtre, mais l'enregistrement a échoué sans rien garder sur le disque (disque plein, dossier interdit en écriture…) ; la fenêtre reste ouverte pour qu'il lise ce qui s'est passé (il peut, par exemple, choisir un autre « Dossier des projets » puis mettre le crawl en pause, ce qui l'enregistre). La fin dit comment quitter quand même : la fermeture suivante ferme la fenêtre sans réessayer, et le crawl est alors perdu. « SANS » est en capitales : c'est l'information importante, à rendre aussi visible dans votre langue.
  "engine.closing_cancelled": "Closing cancelled: the crawl could not be saved. Closing the window again will close it WITHOUT saving the crawl.",
  // Ligne de l'encadré rouge des échecs d'enregistrement, juste sous le message engine.save_kept (qui donne le chemin du fichier) : l'utilisateur avait demandé à fermer la fenêtre ; le crawl n'a pas pu remplacer le fichier précédent du site, mais il est entièrement enregistré dans un autre fichier, à côté (« Ouvrir un projet » sait l'ouvrir). La fenêtre reste ouverte pour qu'il le lise ; la fermeture suivante la ferme, sans rien réenregistrer.
  "engine.closing_cancelled_kept": "Closing cancelled: the crawl was kept in the file shown above. You can close the window again.",
  // Ligne de l'encadré rouge, sous engine.closing_cancelled ou engine.closing_cancelled_kept, quand c'est la fermeture qui avait mis en pause un crawl en cours : il reste en pause (rien n'est perdu ni relancé sans qu'on le demande). {resume} = le libellé du bouton qui relance le crawl (clé btn.resume, « Reprendre »), inséré tel quel dans la langue de l'interface ; mettez-le entre les guillemets de votre langue.
  "engine.closing_paused": "The crawl is paused: “{resume}” to continue it.",
  // Infobulle de l'encadré rouge (en bas à droite) qui liste les enregistrements de crawl impossibles (textes engine.save_failed et engine.save_kept, un par ligne) : il reste affiché jusqu'à ce qu'on clique dessus, pour qu'on ait le temps de lire un chemin de fichier.
  "engine.save_failures_close": "Click to dismiss this message",
};
