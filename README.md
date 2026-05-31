# Crawl Audit

Application **desktop** d'audit de crawl. On lui donne une URL de depart, elle
explore le site (BFS) et mesure pour chaque page :

- la **bande passante reelle** transferee (octets sur le fil, compression incluse) ;
- le **poids par page et par image** (et par type de ressource : Document, Script, Stylesheet, Font, Image, Fetch...) ;
- la **detection de technologie** (Next.js, Nuxt, Gatsby, SvelteKit, Remix, Astro, React, Vue, WordPress, et hebergeurs Vercel / Netlify / Cloudflare) ;
- le nombre de **requetes dynamiques** — routes `/api/`, navigations React Server Components (`_rsc=`), et cache CDN manque (`x-vercel-cache: MISS/STALE/PRERENDER`) — utilise comme **proxy des invocations serverless** declenchees par la page.

Les resultats arrivent en streaming dans l'interface (une ligne par page, totaux
en direct, barres de poids relatif).

---

## Architecture

Le projet est un **workspace Cargo a deux crates**, plus une UI statique servie par Tauri :

```
crawl-audit/
├── Cargo.toml          # workspace (members: crawl-core, src-tauri)
├── package.json        # @tauri-apps/cli (dev dep)
│
├── crawl-core/         # CRATE 1 — bibliotheque pure (zero dependance Tauri)
│   └── src/
│       ├── lib.rs          # declare et reexporte les modules
│       ├── report.rs       # types du contrat (PageReport, CrawlSummary, CrawlConfig, CrawlProgress...)
│       ├── techno.rs       # detection de stack par signatures + heuristique requete dynamique
│       ├── http_probe.rs   # sonde HTTP legere (reqwest) + mesure des assets statiques
│       ├── chrome_probe.rs # sonde NAVIGATEUR (Chrome DevTools Protocol, domaine Network)
│       ├── chrome_manager.rs # localisation + lancement headless de Chrome/Edge/Chromium
│       └── crawler.rs      # orchestrateur BFS : decide quelle sonde utiliser, agrege
│
├── src-tauri/          # CRATE 2 — coquille Tauri v2 (le binaire desktop)
│   ├── src/
│   │   ├── main.rs         # bootstrap Tauri, enregistre les commandes
│   │   └── commands.rs     # commandes start_crawl / stop_crawl + emission des evenements
│   ├── build.rs
│   └── tauri.conf.json
│
└── ui/                 # FRONTEND statique (vanilla, withGlobalTauri)
    ├── index.html
    ├── styles.css
    └── main.js             # invoke() des commandes + listen() des evenements
```

**Separation des responsabilites :**

- `crawl-core` ne connait pas Tauri. C'est une bibliotheque reutilisable et
  testable (elle contient ses propres tests unitaires dans `techno.rs`). Elle
  expose `crawler::run(config, on_progress, on_page)` qui prend deux callbacks.
- `src-tauri` est une fine coquille : elle branche les callbacks de `crawl-core`
  sur des evenements Tauri emis vers le frontend, et expose les commandes.
- `ui/` est du HTML/CSS/JS sans build step ni framework. Le pont Tauri est
  expose globalement (`window.__TAURI__`, via `withGlobalTauri: true`).

### Contrat commandes / evenements (Rust <-> JS)

| Sens        | Nom               | Charge utile                          |
|-------------|-------------------|---------------------------------------|
| JS -> Rust  | `start_crawl`     | `CrawlConfig`                         |
| JS -> Rust  | `stop_crawl`      | —                                     |
| Rust -> JS  | `crawl:progress`  | `CrawlProgress` (avancement)          |
| Rust -> JS  | `crawl:page`      | `PageReport` (une page mesuree)       |
| Rust -> JS  | `crawl:done`      | `CrawlSummary` (resume final)         |
| Rust -> JS  | `crawl:error`     | `string` (message d'erreur globale)   |

Les charges utiles sont les types de `report.rs` serialises par **serde**
(noms de champs Rust verbatim, sans `rename`). Le frontend lit donc exactement
`total_bytes`, `request_count`, `dynamic_requests`, `vercel_cache_misses`,
`measured_with`, le tableau `tech[]` et le tableau `images[]` (`{ url, bytes, kind }`).

### La sonde hybride : reqwest + Chrome CDP

Le coeur de l'outil est sa strategie de mesure a deux niveaux, decidee par page
dans `crawler.rs` :

1. **Sonde HTTP legere (`http_probe`, reqwest)** — toujours en premier.
   Un simple GET recupere statut, en-tetes et HTML, puis on extrait les liens et
   les assets statiques (`img`, `script`, `link[rel=stylesheet]`, fonts preload)
   qu'on telecharge en parallele pour mesurer leur poids (`Content-Length` ou
   taille du corps). Rapide, peu couteux, suffisant pour un site statique.

2. **Sonde navigateur (`chrome_probe`, Chrome DevTools Protocol)** — declenchee
   quand un **framework JS** est detecte (Next.js, Nuxt, Gatsby, SvelteKit,
   Remix, React, Vue) ou quand l'option **Forcer Chrome** est cochee.
   On pilote un Chrome headless via `chromiumoxide` : on ecoute le domaine
   `Network` du CDP **avant** de naviguer, et on lit `encoded_data_length`
   (`LoadingFinished`) qui donne la bande passante **reellement** transferee, y
   compris les requetes XHR/fetch/RSC declenchees par le JavaScript apres le
   chargement initial. C'est la **mesure de reference** : une simple lecture HTTP
   sous-estimerait fortement un site hydrate cote client.

Le navigateur est lance **en lazy** (a la premiere page qui en a besoin), **une
seule fois** pour tout le crawl, et ferme proprement a la fin. Si Chrome ne peut
pas demarrer ou que la mesure echoue, le crawler **bascule automatiquement** sur
la sonde HTTP statique (avec la raison consignee dans le champ `error` du rapport).

---

## Prerequis

> L'application est concue pour tourner **sur Windows** (chemins Chrome/Edge,
> `windows_subsystem = "windows"` en release). Le developpement se fait donc
> sous Windows, pas dans WSL.

- **Rust** installe via [rustup](https://rustup.rs/) **sur Windows** (toolchain
  MSVC). C'est rustup qui fournit `cargo`, utilise en coulisses par Tauri.
- **Node.js** (pour `npm` et la CLI Tauri `@tauri-apps/cli`).
- **Google Chrome** — **deja present** sur la machine. A defaut, Microsoft Edge
  (moteur Chromium) est utilise en repli automatique ; `chromium`/`google-chrome`
  sont cherches dans le `PATH` sous Linux.
- **WebView2** (runtime systeme Windows, generalement deja installe avec Edge) —
  c'est le moteur de rendu de la fenetre Tauri.

> **Note Linux** : un build Linux necessiterait en plus les paquets systeme
> `webkit2gtk` (ex. `webkit2gtk-4.1`) et leurs dependances. Ce n'est **pas**
> requis pour la cible Windows visee ici.

---

## Build / Run

Toutes les commandes se lancent depuis la racine `crawl-audit/`.

### 1. Installer les dependances Node (CLI Tauri)

```bash
npm install
```

### 2. Lancer en developpement

```bash
npx tauri dev
```

Ouvre la fenetre desktop et recompile a chaud. **Le premier build telecharge et
compile toutes les crates Rust** (reqwest, chromiumoxide, scraper, tauri...) :
comptez plusieurs minutes.

### 3. Construire le binaire de production

```bash
npx tauri build
```

Produit l'executable et les installeurs dans `src-tauri/target/release/`
(et `.../release/bundle/`).

> **Important — code genere sans compilateur.** Ce code a ete ecrit sans qu'un
> compilateur Rust soit disponible dans l'environnement de generation. Le
> **premier `cargo build`** (declenche par `tauri dev`/`tauri build`) est donc le
> premier vrai passage du compilateur : il peut reclamer de **petits ajustements
> d'API**, en particulier autour de **`chromiumoxide` 0.7** (formes exactes des
> evenements CDP `EventResponseReceived` / `EventLoadingFinished`, acces aux
> en-tetes via `network::Headers`, signatures de `event_listener` / `execute`).
> Voir la section *Limites & points de vigilance* ci-dessous.

---

## Limites honnetes

- **On ne lit PAS le compteur de facturation de Vercel** (ni d'aucun hebergeur).
  L'outil ne s'authentifie a aucune API de billing.
- Le compteur **`dynamic_requests` / cache MISS est une ESTIMATION** de la charge
  serverless, **pas** une mesure exacte des invocations facturees. Il s'appuie sur
  des heuristiques cote client :
  - URL contenant `/api/` ou `_rsc=` ;
  - en-tete `x-vercel-cache` valant `MISS`, `STALE` ou `PRERENDER`.
  Un `HIT` de cache est compte comme non dynamique. Selon la configuration reelle
  (ISR, revalidation, edge vs serverless, autre CDN), le nombre d'invocations
  facturees peut differer. A lire comme un **proxy / ordre de grandeur**, utile
  pour comparer des pages entre elles, pas comme une facture.
- La detection de techno est **par signatures** (style Wappalyzer) : elle peut
  manquer une stack obfusquee ou produire un faux positif sur un marqueur partage.
- La mesure de poids HTTP statique se fie au `Content-Length` quand il est
  annonce ; un serveur qui ne le renvoie pas fait retomber sur la taille du corps
  telecharge (toujours correct, mais une requete complete par asset).

### Points de vigilance au 1er build

- **`chromiumoxide` 0.7** : c'est la dependance la plus susceptible de demander
  un micro-ajustement (acces `.inner()` sur les `request_id`, type exact de
  `encoded_data_length`, enveloppe JSON de `network::Headers`, builder
  `BrowserConfig`). En cas d'erreur de compilation, c'est le premier endroit a
  regarder.
- Le pont front utilise `window.__TAURI__.core.invoke` et
  `window.__TAURI__.event.listen` ; cela suppose bien `withGlobalTauri: true`
  (deja active dans `tauri.conf.json`).
