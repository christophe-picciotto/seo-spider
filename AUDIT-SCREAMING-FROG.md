# Audit Screaming Frog SEO Spider vs seo-spider (notre app)

> Date : 01/06/2026. Légende état de NOTRE app : ✅ présent · 🟡 partiel · ❌ absent.
> Source : doc officielle screamingfrog.co.uk + analyse de l'app installée (configs IA anthropic/gemini/openai/ollama + libs Lucene/Derby/CDP repérées).

---

## 1. Modes de crawl
| Fonction SF | État |
|---|---|
| **Spider Mode** (suit les liens depuis une URL seed, crawl tout le site) | 🟡 (on a le BFS, mais limité par défaut à 100 — à passer en illimité) |
| **List Mode** (crawler une liste d'URLs fournie / fichier / sitemap collé) | ❌ |
| **SERP Mode** (importer titres/metas pour calculer pixels, sans crawl) | ❌ |
| **Compare Mode** (comparer 2 crawls : nouveau / disparu / modifié) | ❌ |

## 2. Configuration du crawl
| Fonction SF | État |
|---|---|
| Limites : total URLs, profondeur, query strings, **crawl illimité** | 🟡 (profondeur + max ; pas d'illimité) |
| Include / Exclude par regex | ❌ |
| Respect du **robots.txt** (+ robots custom, ignore) | ❌ (le crawler ne lit pas robots.txt) |
| Rendu : Texte seul / **JavaScript** (Chromium) / vieux AJAX | ✅ (chromiumoxide CDP) |
| Vitesse : threads + URLs/s (rate limit) | ✅ (governor) |
| User-Agent (préréglages + custom) + rotation | ✅ (pool UA + rotation) |
| En-têtes HTTP custom, cookies | 🟡 (UA oui ; pas d'éditeur de headers/cookies) |
| Authentification (Basic/Digest, formulaire) | ❌ |
| **Stockage RAM vs Base de données** (gros crawls millions URLs) | ❌ (tout en RAM) |
| Crawl sous-domaines / hors domaine / liens externes | 🟡 (same-domain on/off) |

## 3. Onglets de données (le cœur de SF — ~30 onglets)
| Onglet SF | Ce qu'il montre | État |
|---|---|---|
| **Internal** | toutes les URLs internes, ~60 colonnes (statut, type, taille, mots, profondeur, inlinks…) | 🟡 |
| **External** | URLs externes + leur statut | ❌ |
| **Security** | HTTP/HTTPS, mixed content, HSTS, en-têtes sécurité | ❌ |
| **Response Codes** | filtres : Blocked robots, No Response, 2xx, 3xx (redirection), 4xx, 5xx | 🟡 (statut oui, pas les filtres ni chaînes de redirection) |
| **URL** | longueur, caractères non ASCII, majuscules, doublons, params | ❌ |
| **Page Titles** | manquant, dupliqué, long/court (pixels), multiple, identique au H1 | 🟡 (manquant/long/court ; pas pixels/dupliqué) |
| **Meta Description** | manquant, dupliqué, long/court (pixels), multiple | 🟡 |
| **Meta Keywords** | présence | ❌ |
| **H1 / H2** | manquant, dupliqué, multiple, trop long | 🟡 (H1 manquant/multiple) |
| **Content** | nb mots, ratio texte/code, **near-duplicates**, **spelling & grammar**, readability | 🟡 (word_count + text_ratio ; pas duplicates/orthographe) |
| **Images** | alt manquant/long, images > X Ko, manquantes | 🟡 (alt manquant ; pas poids/longueur alt) |
| **Canonicals** | manquant, canonicalised, dupliqués, chaînes, hors domaine | 🟡 (présence) |
| **Pagination** (rel next/prev) | séquences, boucles, non-indexable | ❌ |
| **Directives** | index/noindex, follow/nofollow, **X-Robots-Tag (header)**, refresh | 🟡 (meta robots ; pas X-Robots-Tag header) |
| **Hreflang** | confirmation réciproque, manquant, incohérences, codes invalides | 🟡 (liste hreflang ; pas la validation réciproque) |
| **JavaScript** | contenu dépendant du JS, liens/contenu seulement en rendu | ❌ |
| **AMP** | URLs AMP, erreurs de validation | ❌ |
| **Structured Data** | types JSON-LD/Microdata/RDFa + **validation** (Schema.org, Google) | 🟡 (types JSON-LD ; pas validation) |
| **Sitemaps** | URLs orphelines, non dans sitemap, non indexables dans sitemap | ❌ |
| **PageSpeed** | métriques CrUX + Lighthouse via API PSI, opportunités | ❌ |
| **Mobile** | balises mobiles, viewport, usability | ❌ |
| **Links** | tous les liens (inlinks/outlinks), anchor, distance, liens cassés | 🟡 (liens int/ext comptés ; pas la matrice inlinks/outlinks) |
| **Analytics / Search Console** | données GA4 / GSC jointes par URL ; pages orphelines | ❌ |

## 4. Onglet Issues + panneaux latéraux
| Fonction SF | État |
|---|---|
| **Issues tab** centralisé (tous les problèmes, priorité, % URLs affectées, lien d'aide) | 🟡 (issues basiques, pas de priorité/%) |
| **Right-hand Overview** (arbo des catégories avec compteurs + % en direct) | ❌ |
| **Lower window — URL Details** : Inlinks, Outlinks, Image Details, Resources, **SERP Snippet**, **Rendered Page**, View Source (brut/rendu), HTTP Headers, Cookies, Duplicate Details, Structured Data Details, PageSpeed, Spelling | ❌ |

## 5. Extraction & recherche personnalisées
| Fonction SF | État |
|---|---|
| **Custom Extraction** (CSS Path / XPath / Regex) → colonnes sur mesure | ❌ |
| **Custom Search** (présence/absence de texte/code dans les pages) | ❌ |
| **Custom JavaScript** (snippets exécutés pendant le crawl, dont appels **LLM** OpenAI/Gemini/Ollama/Anthropic) | ❌ |

## 6. Intégrations API
| Fonction SF | État |
|---|---|
| Google Analytics 4 · Search Console · PageSpeed Insights | ❌ |
| Majestic · Ahrefs · Moz (backlinks/metrics) | ❌ |

## 7. Exports & rapports
| Fonction SF | État |
|---|---|
| Export par onglet/filtre (CSV/Excel/Sheets) | ✅ (CSV par onglet) |
| **Bulk Export** (tous les inlinks, images+alt, réponses par source, etc.) | ❌ |
| **Reports** (Crawl Overview, Redirect/Canonical chains, Hreflang, Insecure content, Orphan pages, Structured Data, Issues) | ❌ |
| **XML Sitemap / Image Sitemap** (génération) | ❌ |

## 8. Visualisations
| Fonction SF | État |
|---|---|
| Crawl Tree Graph · Directory Tree · **Force-Directed Diagram** · Inlink/Outlink visualisations | ❌ |

## 9. Avancé
| Fonction SF | État |
|---|---|
| **Scheduling** (crawls planifiés + export auto) | ❌ |
| **CLI / ligne de commande** (headless, automatisation) | ❌ |
| **Crawl Comparison / Change Detection** | ❌ |
| **Segments** (regrouper les URLs par règles) | ❌ |
| **Near Duplicates** (seuil de similarité, minhash) | ❌ |
| **Spelling & Grammar** (multi-langues) | ❌ |
| robots.txt tester | ❌ |

---

## BONUS — ce que NOTRE app a et que SF n'a PAS
| Fonction | Intérêt |
|---|---|
| **Mesure de bande passante réelle** (octets transférés, compression incluse, via CDP `encoded_data_length`) | unique, pour audit perf/coût |
| **Estimation coût serverless** (cache MISS Vercel, /api, RSC) | unique |
| **Détection de techno/stack** (Next, Nuxt, Astro, WordPress, Vercel…) | proche Wappalyzer |
| **Poids par type de ressource** (Document/Script/CSS/Font/Image) | unique |

---

## Synthèse du gap (priorisé)

### Vague 1 — « Ça ressemble enfin à Screaming Frog » (le plus impactant)
- **Refonte UI façon SF** : grille dense plein écran + onglets à compteurs + **sidebar Overview** (catégories/compteurs/%) + **panneau de détail en bas** (Inlinks/Outlinks/SERP snippet/headers) + barre de statut.
- **Crawl « tout le site »** (illimité par défaut + limite optionnelle).
- **Response Codes** complets (filtres 2xx/3xx/4xx/5xx + chaînes de redirection).
- **Inlinks/Outlinks par URL** (matrice de liens) + liens cassés avec source.
- **Respect du robots.txt**.

### Vague 2 — Profondeur d'analyse SEO
- Doublons (titres/metas/H1) + **near-duplicates** (minhash).
- Canonicals (chaînes, hors domaine), Pagination, Hreflang (validation réciproque), X-Robots-Tag header.
- Structured Data **validation**, Security tab, URL tab, Sitemap (lecture + pages orphelines).
- Custom Extraction (CSS/XPath/Regex) + Custom Search.

### Vague 3 — Intégrations & industrialisation
- API GA4 / GSC / PageSpeed.
- Bulk Export + Reports + génération XML Sitemap.
- Visualisations (force-directed), Scheduling, CLI, Compare, stockage base de données (gros crawls).
- Custom JavaScript / LLM (réutilise les configs IA que SF avait — on peut faire mieux côté Rust).
