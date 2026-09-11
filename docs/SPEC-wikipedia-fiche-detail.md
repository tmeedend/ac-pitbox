# SPEC — Enrichissement Wikipédia de la fiche de détail

## 1. Objectif et cadrage

Afficher, sur la fiche de détail d'un mod (voiture ou circuit), un extrait de l'article
Wikipédia correspondant au véhicule ou au circuit réel, dans un onglet distinct de la
description fournie par l'auteur du mod.

**Cette fonctionnalité est décorative.** Elle sert le plaisir de lecture, pas la
constitution d'un référentiel. Ce cadrage a trois conséquences qui doivent guider chaque
arbitrage d'implémentation :

- **La précision prime sur le rappel.** En cas d'ambiguïté, on n'affiche rien. Un mauvais
  article coûte beaucoup plus cher que pas d'article.
- **L'absence n'est pas une erreur.** Aucun message, aucune icône d'avertissement, aucun
  encart grisé. L'onglet est simplement absent.
- **Rien ne dépend de cette donnée.** Elle n'alimente ni les filtres, ni le scoring, ni
  aucune autre fonctionnalité. Elle peut échouer silencieusement sans conséquence.

### Non-objectifs

- Pré-remplir la bibliothèque : la résolution se fait à l'ouverture d'une fiche, jamais en
  masse.
- Extraire des caractéristiques techniques pour les comparer à celles du mod.
- Afficher des images issues de Wikipédia ou de Commons (voir §9).
- Implémenter le partage communautaire des appariements (préparé en §10, non implémenté).

---

## 2. Contrainte juridique structurante

Le texte de Wikipédia est sous CC BY-SA 4.0 (les contributions anciennes restent en 3.0).
L'affichage doit rester une **collection** — des œuvres juxtaposées et identifiables —
et non une **œuvre dérivée**, faute de quoi la clause ShareAlike remonterait sur
l'application.

Règles impératives :

- **Ne jamais fusionner** le texte Wikipédia avec la description du mod, ni les afficher
  dans un même bloc continu.
- **Ne jamais reformuler, résumer, traduire ou faire passer le texte par un modèle de
  langage.** L'extrait est affiché tel que fourni par l'API.
- Le bloc Wikipédia est **visuellement distinct** et porte son attribution (§7.4).

---

## 3. Modèle de données

Trois tables (ou équivalent selon le stockage existant du projet).

### 3.1 `wiki_link` — l'appariement

| Champ         | Type     | Notes |
|---------------|----------|-------|
| `mod_key`     | TEXT PK  | Nom du dossier du mod (`ks_toyota_ae86`, `ks_nordschleife`). Choisi pour sa stabilité entre installations. |
| `entity_id`   | TEXT     | Identifiant Wikidata (`Q123456`). **Jamais une URL.** |
| `source`      | ENUM     | `auto` \| `manual` \| `import` |
| `resolved_at` | DATETIME | |

**Ne stocker en aucun cas une URL d'article.** Le Q-id est indépendant de la langue et
survit aux renommages d'articles. Les URL saisies par l'utilisateur sont résolues en Q-id
à la saisie, puis jetées.

**Précédence :** `manual` > `import` > `auto`. Une entrée `manual` n'est jamais écrasée
automatiquement. Cette précédence doit être respectée dès maintenant même si l'import
n'est pas implémenté.

### 3.2 `wiki_cache` — le contenu

| Champ             | Type     | Notes |
|-------------------|----------|-------|
| `entity_id`       | TEXT     | PK composite avec `lang` |
| `lang`            | TEXT     | Code de langue demandé |
| `article_title`   | TEXT     | |
| `article_url`     | TEXT     | Stockée avec le contenu, jamais reconstruite à l'affichage |
| `revision_id`     | INTEGER  | Traçabilité d'attribution + fraîcheur |
| `extract`         | TEXT     | Texte brut de l'introduction |
| `parent_entity`   | TEXT     | Non nul si repli sur l'entité parente (§5.3) |
| `available_langs` | TEXT     | Liste des langues où l'article existe, pour le sélecteur |
| `fetched_at`      | DATETIME | |

### 3.3 `wiki_no_match` — le cache négatif

| Champ          | Type     |
|----------------|----------|
| `mod_key`      | TEXT PK  |
| `attempted_at` | DATETIME |

**Indispensable.** Sans cette table, chaque ouverture d'une fiche sans correspondance
relance une résolution complète. TTL : 90 jours (un article peut être créé entre-temps).

---

## 4. Appariement automatique

Deux stratégies distinctes selon le type de contenu. Dans les deux cas, le résultat est
soit un Q-id unique avec une confiance suffisante, soit **rien**.

### 4.1 Voitures — par nom, depuis les champs structurés

Source : `ui_car.json` (`brand`, `name`, `year`).

1. Recherche d'entités Wikidata sur `"{brand} {name}"` nettoyé (voir §4.3).
2. Récupération de `P31` (nature de l'élément) pour les candidats.
3. Filtrage sur les types acceptés : modèle d'automobile et ses sous-classes.
4. Scoring : correspondance de la marque, similarité du nom, cohérence de l'année avec la
   période de production si disponible.
5. **Rejet si l'écart de score entre le premier et le deuxième candidat est faible.**
   L'ambiguïté produit un non-résultat, pas un tirage au sort.

Le repli vers le modèle générique étant acceptable (§5.3), le scoring n'a pas besoin
d'exiger une correspondance de génération. « Toyota Corolla » est un résultat valide pour
un mod d'AE86.

### 4.2 Circuits — par coordonnées, d'abord

Source : `ui_track.json` (`geotags` ou `latitude`/`longitude`, `country`, `city`).

Les coordonnées sont **nettement plus fiables que le nom** : Nordschleife → Nürburgring,
Shutoko → Metropolitan Expressway, cols d'Initial D → nom réel de la montagne. Sur le nom
seul, ces trois cas échouent.

1. Si coordonnées disponibles : recherche géographique d'articles dans un rayon de ~5 km
   (l'API MediaWiki expose une recherche par proximité, `list=geosearch`, plus simple à
   mettre en œuvre qu'une requête SPARQL).
2. Récupération du Q-id de chaque page trouvée, puis filtrage sur les types acceptés —
   **allowlist plus large que pour les voitures** : circuit automobile, route, autoroute,
   col de montagne.
3. Sélection du candidat le plus proche parmi ceux dont le type est accepté.
4. Repli sur la recherche par nom si aucune coordonnée n'est présente.

**Apparier au niveau du circuit, pas de la configuration.** Un dossier contenant `gp`,
`national` et `short` produit un seul appariement, porté par le dossier parent.

### 4.3 Nettoyage des noms

Avant toute recherche, retirer des chaînes : versions (`v1.2`, `1.05`), mentions de
qualité (`[4K]`, `HD`, `Remaster`), tags de packs et crochets, préparateurs (`Rocket
Bunny`, `Liberty Walk`), suffixes de conversion (`AC`, `Conversion`, `Fixed`). Cette liste
doit vivre dans un fichier de configuration, pas dans le code.

### 4.4 Identifiants Wikidata

Les identifiants de propriétés et de classes (nature de l'élément, sous-classe de, partie
de, coordonnées, type « modèle d'automobile », type « circuit automobile ») doivent être
**vérifiés sur Wikidata avant d'être câblés**, et regroupés dans un module de constantes
unique et commenté. Ne pas les disperser dans le code.

---

## 5. Résolution à l'affichage

### 5.1 Langue

Défaut : locale de l'application. Préférence utilisateur mémorisée **globalement**, pas
par mod.

### 5.2 Chaîne de repli

1. Article de l'entité dans la langue demandée
2. Article de l'**entité parente** dans la langue demandée
3. Article de l'entité en anglais
4. Article de l'entité parente en anglais
5. Rien

Le niveau 2 avant le niveau 3 est délibéré : sur les modèles japonais et américains, la
Wikipédia française couvre souvent le modèle générique sans éclatement par génération. Ce
repli augmente sensiblement la couverture en français.

### 5.3 Remontée vers l'entité parente

Relations acceptées : génération → modèle, configuration ou tronçon → circuit ou voie
d'ensemble (relation « partie de » sur Wikidata).

**Un seul niveau de remontée.** Ne jamais remonter jusqu'au constructeur : si aucun
article n'existe pour le modèle, on n'affiche rien plutôt qu'un article sur Toyota.

Quand un repli parent est actif, l'interface l'indique (§7.3).

### 5.4 Sélecteur de langue

Alimenté par les langues où l'article existe réellement. **Ne pas limiter à deux langues
en dur** : sur les JDM, l'article japonais est souvent le plus complet.

Un indice discret de complétude (« EN · plus détaillé ») est acceptable si la taille des
articles est disponible, mais **jamais de bascule automatique** sur cette base.

---

## 6. Accès réseau

### 6.1 Choix d'API

Utiliser l'**Action API** (`/w/api.php`) plutôt que les API REST. Motif : l'API Core sur
`api.wikimedia.org` est en dépréciation progressive depuis juillet 2026 avec des routes de
remplacement non encore annoncées, et RESTBase (`/api/rest_v1/`) est également en cours de
retrait. L'Action API est la plus stable et permet de tout récupérer en une requête.

Une seule requête par article suffit pour obtenir l'extrait d'introduction en texte brut,
les liens interlangues, l'identifiant Wikidata et le numéro de révision.

### 6.2 Politesse

- **En-tête User-Agent obligatoire et identifiable** : nom de l'application, version, URL
  de contact. Son absence entraîne un blocage.
- Requêtes **séquentielles**, jamais en rafale parallèle.
- Backoff exponentiel sur code 429 (quota dépassé), avec abandon silencieux après quelques
  tentatives.
- Timeout court (5 s). Un échec réseau est un non-résultat, pas une erreur affichée.

### 6.3 Volume attendu

Résolution à l'ouverture de fiche uniquement, avec cache et cache négatif : quelques
dizaines de requêtes par mois en usage réel. Aucune stratégie de quota n'est nécessaire
au-delà du backoff.

---

## 7. Interface

### 7.1 Structure

Sur la fiche de détail, un jeu d'onglets au niveau du bloc textuel :

- `Description` — contenu actuel, inchangé
- `Le modèle réel` (voitures) / `Le circuit` (circuits) — contenu Wikipédia

L'onglet Wikipédia est **entièrement masqué** si aucun contenu n'est disponible. Un onglet
présent mais vide est pire que pas d'onglet.

### 7.2 Onglet par défaut

`Description`, **sauf si** la description du mod est vide ou très courte (seuil à calibrer,
de l'ordre de 150 caractères), auquel cas l'onglet Wikipédia est ouvert par défaut. C'est
le cas où il apporte le plus, et il est fréquent.

### 7.3 Contenu de l'onglet

- Si repli parent actif : une ligne discrète en tête, `Article général : {titre}`.
  Affichée uniquement dans ce cas.
- Extrait d'introduction, tel que renvoyé par l'API.
- Sélecteur de langue.
- Lien « Lire l'article complet » ouvrant le **navigateur système**. Pas de webview
  intégrée pointant vers wikipedia.org : cela casse le mode hors ligne, impose leur CSP et
  fait perdre l'identité visuelle de l'application.
- Bloc d'attribution (§7.4).

### 7.4 Attribution

En pied d'onglet, obligatoire :

> Extrait de l'article **{titre}** de Wikipédia — {lien} · Texte disponible sous licence
> [CC BY-SA 4.0]({lien vers la licence})

Le mot « extrait » est nécessaire : n'afficher qu'un fragment constitue une modification,
qui doit être signalée.

### 7.5 Chargement

La fiche s'affiche **complète et immédiatement** avec sa photo et ses caractéristiques.
L'onglet Wikipédia apparaît lorsque le contenu arrive. Réserver l'espace ou utiliser une
transition pour éviter tout décalage de mise en page à l'arrivée du contenu.

### 7.6 Correction manuelle

Dans un menu contextuel de l'onglet (pas un assistant, pas de validation au premier
affichage) : « Ce n'est pas le bon article ? »

Ouvre une recherche libre affichant les candidats avec leur description courte Wikidata
(« modèle d'automobile Toyota, 1983–1987 »), qui lève l'ambiguïté d'un coup d'œil. Accepte
également le collage d'une URL Wikipédia, résolue immédiatement en Q-id.

**En mode manuel, relâcher le filtrage par type.** Si l'utilisateur veut lier son mod à
une entité hors taxonomie, c'est son droit — la taxonomie a plus de chances d'être en tort
que lui.

Le choix est enregistré avec `source = manual`.

---

## 8. Réglages

- **Enrichissement en ligne** — activé par défaut, désactivable. Motif : l'application
  interroge Wikipédia à chaque ouverture de fiche, ce qui révèle indirectement le contenu
  de la bibliothèque. Une partie du public joue délibérément hors ligne. Désactivé, aucune
  requête sortante n'est émise et le cache existant reste consultable.
- **Langue de préférence** pour les articles.
- **Vider le cache Wikipédia** — purge `wiki_cache` et `wiki_no_match`, conserve
  `wiki_link`.

---

## 9. Images

**Ne pas afficher d'images issues de Wikipédia ou de Commons dans cette version.**

Chaque image possède sa propre licence, distincte de celle du texte. Beaucoup sont non
libres (usage loyal), en particulier les logos de constructeurs. Même pour les fichiers
libres de Commons, la licence impose de créditer l'auteur individuellement.

Les mods disposent déjà de leurs previews. Le rapport bénéfice/complexité juridique est
défavorable.

---

## 10. Préparation du partage communautaire

Non implémenté dans cette version, mais le modèle de données doit le permettre sans
migration.

- Le champ `source` avec ses trois valeurs est le mécanisme central : il permet qu'un
  import futur n'écrase jamais les corrections locales, et rend possible un retour à
  l'automatique.
- La clé d'appariement (`mod_key` = nom du dossier) est ce qui circulera entre
  installations. Ne pas indexer sur le nom affiché, trop variable.
- Le format d'export doit prévoir dès maintenant un champ `entity_id` par mod, même
  toujours vide côté import.

**À l'implémentation future, la validation à l'import devra être stricte** — une donnée
importée est du contenu tiers arbitraire :

- Q-id : validation de format uniquement.
- URL : **allowlist sur les sous-domaines de `wikipedia.org`**. Un homographe du type
  `wikipedıa.org` passerait sinon inaperçu et ouvrirait un vecteur d'hameçonnage dans une
  fonctionnalité purement décorative.
- Import toujours explicite, jamais automatique.

---

## 11. Tests

Le cœur logique doit vivre dans des fonctions pures, testables sans réseau ni interface.

- **Choix de langue et chaîne de repli** : les cinq niveaux de §5.2, incluant le cas où
  seule l'entité parente a un article dans la langue demandée.
- **Nettoyage des noms** : jeu de noms de mods réels tirés de la bibliothèque.
- **Scoring d'appariement** : rejet effectif en cas de candidats proches ; acceptation du
  modèle générique quand la génération est absente.
- **Précédence des sources** : `manual` non écrasée par `auto` ni par `import`.
- **Validation d'URL** : rejet des domaines non-Wikipédia, y compris homographes.
- **Cache négatif** : pas de nouvelle requête avant expiration du TTL.

Les appels réseau sont mockés. Aucun test ne doit dépendre de la disponibilité de
Wikipédia.

---

## 12. Découpage proposé

1. **Socle** — modèle de données, client Action API avec User-Agent et backoff, cache et
   cache négatif, résolution de langue avec repli parent. Sans interface.
2. **Appariement voitures** — nettoyage des noms, recherche, filtrage par type, scoring.
3. **Appariement circuits** — recherche géographique, allowlist élargie, appariement au
   niveau du dossier parent.
4. **Interface** — onglets, extrait, sélecteur de langue, attribution, états de chargement.
5. **Correction manuelle** — recherche libre, collage d'URL, précédence.
6. **Réglages** — interrupteur, langue, purge du cache.

Les lots 2 et 3 sont indépendants et parallélisables une fois le lot 1 en place.

---

## 13. Points à trancher à l'implémentation

- Seuil de longueur de description en dessous duquel l'onglet Wikipédia devient l'onglet
  par défaut.
- Seuil d'écart de score en dessous duquel un appariement est rejeté pour ambiguïté.
- Rayon exact de la recherche géographique pour les circuits (5 km est un point de départ,
  à ajuster sur les cas réels — les tracés urbains et les cols demandent peut-être plus).
- TTL du cache positif (30 jours proposé) et du cache négatif (90 jours proposé).
