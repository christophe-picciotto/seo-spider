# SEO Spider

Application **desktop** d'audit de crawl. On lui donne une URL de départ, elle
explore le site (BFS) et mesure pour chaque page :

- la **bande passante réelle** transférée (octets sur le fil, compression incluse) ;
- le **poids par page et par image** (et par type de ressource : Document, Script, Stylesheet, Font, Image, Fetch...) ;
- la **détection de technologie** (Next.js, Nuxt, Gatsby, SvelteKit, Remix, Astro, React, Vue, WordPress, et hébergeurs Vercel / Netlify / Cloudflare) ;
- le nombre de **requêtes dynamiques** — routes `/api/`, navigations React Server Components (`_rsc=`), et cache CDN manqué (`x-vercel-cache: MISS/STALE/PRERENDER`) — utilise comme **proxy des invocations serverless** déclenchées par la page.

Les résultats arrivent en streaming dans l'interface (une ligne par page, totaux
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
│       ├── techno.rs       # détection de stack par signatures + heuristique requête dynamique
│       ├── http_probe.rs   # sonde HTTP légère (reqwest) + mesure des assets statiques
│       ├── chrome_probe.rs # sonde NAVIGATEUR (Chrome DevTools Protocol, domaine Network)
│       ├── chrome_manager.rs # localisation + lancement headless de Chrome/Edge/Chromium
│       └── crawler.rs      # orchestrateur BFS : decide quelle sonde utiliser, agrégé
│
├── src-tauri/          # CRATE 2 — coquille Tauri v2 (le binaire desktop)
│   ├── src/
│   │   ├── main.rs         # bootstrap Tauri, enregistre les commandes
│   │   └── commands.rs     # commandes start_crawl / stop_crawl + emission des événements
│   ├── build.rs
│   └── tauri.conf.json
│
└── ui/                 # FRONTEND statique (vanilla, withGlobalTauri)
    ├── index.html
    ├── styles.css
    └── main.js             # invoke() des commandes + listen() des événements
```

**Separation des responsabilites :**

- `crawl-core` ne connait pas Tauri. C'est une bibliotheque reutilisable et
  testable (elle contient ses propres tests unitaires dans `techno.rs`). Elle
  expose `crawler::run(config, on_progress, on_page)` qui prend deux callbacks.
- `src-tauri` est une fine coquille : elle branche les callbacks de `crawl-core`
  sur des événements Tauri emis vers le frontend, et expose les commandes.
- `ui/` est du HTML/CSS/JS sans build step ni framework. Le pont Tauri est
  expose globalement (`window.__TAURI__`, via `withGlobalTauri: true`).

### Contrat commandes / événements (Rust <-> JS)

| Sens        | Nom               | Charge utile                          |
|-------------|-------------------|---------------------------------------|
| JS -> Rust  | `start_crawl`     | `CrawlConfig`                         |
| JS -> Rust  | `stop_crawl`      | —                                     |
| Rust -> JS  | `crawl:progress`  | `CrawlProgress` (avancement)          |
| Rust -> JS  | `crawl:page`      | `PageReport` (une page mesurée)       |
| Rust -> JS  | `crawl:done`      | `CrawlSummary` (resume final)         |
| Rust -> JS  | `crawl:error`     | `string` (message d'erreur globale)   |

Les charges utiles sont les types de `report.rs` serialises par **serde**
(noms de champs Rust verbatim, sans `rename`). Le frontend lit donc exactement
`total_bytes`, `request_count`, `dynamic_requests`, `vercel_cache_misses`,
`measured_with`, le tableau `tech[]` et le tableau `images[]` (`{ url, bytes, kind }`).

### La sonde hybride : reqwest + Chrome CDP

Le coeur de l'outil est sa stratégie de mesure a deux niveaux, decidee par page
dans `crawler.rs` :

1. **Sonde HTTP légère (`http_probe`, reqwest)** — toujours en premier.
   Un simple GET recupere statut, en-têtes et HTML, puis on extrait les liens et
   les assets statiques (`img`, `script`, `link[rel=stylesheet]`, fonts preload)
   qu'on télécharge en parallele pour mesurer leur poids (`Content-Length` ou
   taille du corps). Rapide, peu couteux, suffisant pour un site statique.
   Le poids de chaque asset est **mis en cache pour tout le crawl** : le CSS et
   les bundles communs a tout le site ne sont telecharges qu'une fois (les
   échecs, eux, ne sont jamais caches — cf. `AssetCache`).

2. **Sonde navigateur (`chrome_probe`, Chrome DevTools Protocol)** — déclenchée
   quand un **meta-framework JS** est détecté (Next.js, Nuxt, Gatsby, SvelteKit,
   Remix) ou quand l'option **Forcer Chrome** est cochee.
   **React et Vue seuls ne la declenchent plus** : leur détection se satisfait
   d'un `data-reactroot` ou d'un `#root` + bundle, ce qui matche un WordPress
   avec Gutenberg ou un Astro portant une seule ile React. Comme la mesure
   Chrome est **serialisee** (un seul navigateur derriere un Mutex), un tel faux
   positif faisait tomber le crawl entier à une page à la fois. Pour une SPA
   React/Vue pure, cocher **Forcer Chrome**.
   On pilote un Chrome headless via `chromiumoxide` : on ecoute le domaine
   `Network` du CDP **avant** de naviguer, et on lit `encoded_data_length`
   (`LoadingFinished`) qui donne la bande passante **réellement** transférée, y
   compris les requêtes XHR/fetch/RSC déclenchées par le JavaScript après le
   chargement initial. C'est la **mesure de référence** : une simple lecture HTTP
   sous-estimerait fortement un site hydrate côté client.

Le navigateur est lance **en lazy** (à la première page qui en a besoin), **une
seule fois** pour tout le crawl, et ferme proprement à la fin. Si Chrome ne peut
pas démarrer ou que la mesure echoue, le crawler **bascule automatiquement** sur
la sonde HTTP statique (avec la raison consignee dans le champ `error` du rapport).

---

## Prerequis

> L'application est concue pour tourner **sur Windows** (chemins Chrome/Edge,
> `windows_subsystem = "windows"` en release). Le developpement se fait donc
> sous Windows, pas dans WSL.

- **Rust** installe via [rustup](https://rustup.rs/) **sur Windows** (toolchain
  MSVC). C'est rustup qui fournit `cargo`, utilise en coulisses par Tauri.
- **Node.js** (pour `npm` et la CLI Tauri `@tauri-apps/cli`).
- **Google Chrome** — **déjà présent** sur la machine. à défaut, Microsoft Edge
  (moteur Chromium) est utilise en repli automatique ; `chromium`/`google-chrome`
  sont cherches dans le `PATH` sous Linux.
- **WebView2** (runtime système Windows, généralement déjà installe avec Edge) —
  c'est le moteur de rendu de la fenêtre Tauri.

> **Note Linux** : un build Linux necessiterait en plus les paquets système
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

Ouvre la fenêtre desktop et recompile a chaud. **Le premier build télécharge et
compile toutes les crates Rust** (reqwest, chromiumoxide, scraper, tauri...) :
comptez plusieurs minutes.

### 3. Construire le binaire de production

```bash
npx tauri build
```

Produit l'executable et les installeurs dans `src-tauri/target/release/`
(et `.../release/bundle/`).

> **Important — code genere sans compilateur.** Ce code a été ecrit sans qu'un
> compilateur Rust soit disponible dans l'environnement de génération. Le
> **premier `cargo build`** (déclenché par `tauri dev`/`tauri build`) est donc le
> premier vrai passage du compilateur : il peut reclamer de **petits ajustements
> d'API**, en particulier autour de **`chromiumoxide` 0.7** (formes exactes des
> événements CDP `EventResponseReceived` / `EventLoadingFinished`, accès aux
> en-têtes via `network::Headers`, signatures de `event_listener` / `exécute`).
> Voir la section *Limites & points de vigilance* ci-dessous.

---

## Limites honnetes

- **On ne lit PAS le compteur de facturation de Vercel** (ni d'aucun hébergeur).
  L'outil ne s'authentifie a aucune API de billing.
- Le compteur **`dynamic_requests` / cache MISS est une ESTIMATION** de la charge
  serverless, **pas** une mesure exacte des invocations facturees. Il s'appuie sur
  des heuristiques côté client :
  - URL contenant `/api/` ou `_rsc=` ;
  - en-tête `x-vercel-cache` valant `MISS`, `STALE` ou `PRERENDER`.
  Un `HIT` de cache est compte comme non dynamique. Selon la configuration réelle
  (ISR, revalidation, edge vs serverless, autre CDN), le nombre d'invocations
  facturees peut differer. A lire comme un **proxy / ordre de grandeur**, utile
  pour comparer des pages entre elles, pas comme une facture.
- La détection de techno est **par signatures** (style Wappalyzer) : elle peut
  manquer une stack obfusquee ou produire un faux positif sur un marqueur partage.
- La mesure de poids HTTP statique se fie au `Content-Length` quand il est
  annonce ; un serveur qui ne le renvoie pas fait retomber sur la taille du corps
  télécharge (toujours correct, mais une requête complète par asset).

### Points de vigilance au 1er build

- **`chromiumoxide` 0.7** : c'est la dependance la plus susceptible de demander
  un micro-ajustement (accès `.inner()` sur les `request_id`, type exact de
  `encoded_data_length`, enveloppe JSON de `network::Headers`, builder
  `BrowserConfig`). En cas d'erreur de compilation, c'est le premier endroit a
  regarder.
- Le pont front utilise `window.__TAURI__.core.invoke` et
  `window.__TAURI__.event.listen` ; cela suppose bien `withGlobalTauri: true`
  (déjà active dans `tauri.conf.json`).

---

## Licence

[MIT](LICENSE) — © 2026 Christophe Picciotto.
