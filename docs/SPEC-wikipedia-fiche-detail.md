# SPEC — Enrichissement Wikipédia de la fiche de détail

## 1. Objectif et cadrage

Afficher, sur la fiche de détail d'un mod (voiture ou circuit), l'article Wikipédia
correspondant au véhicule ou au circuit réel, dans un onglet distinct de la
description fournie par l'auteur du mod. L'article est affiché **en entier** (§7.3).

**Cette fonctionnalité est décorative.** Elle sert le plaisir de lecture, pas la
constitution d'un référentiel. Ce cadrage a trois conséquences qui doivent guider chaque
arbitrage d'implémentation :

- **La précision prime sur le rappel.** En cas d'ambiguïté, on n'affiche rien. Un mauvais
  article coûte beaucoup plus cher que pas d'article.
- **L'absence n'est pas une erreur.** Aucune icône d'avertissement, aucun encart grisé,
  aucun ton d'échec : l'absence se dit en une phrase et s'accompagne d'une proposition
  d'agir (§7.1).
- **Rien ne dépend de cette donnée.** Elle n'alimente ni les filtres, ni le scoring, ni
  aucune autre fonctionnalité. Elle peut échouer silencieusement sans conséquence.

### Non-objectifs

- Pré-remplir la bibliothèque : la résolution se fait à l'ouverture d'une fiche, jamais en
  masse.
- Extraire des caractéristiques techniques pour les comparer à celles du mod.
- Importer les appariements d'un tiers : seuls ceux que livre l'application sont repris (§10).

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
- Les images ne sont affichées que créditées et sous licence connue (§9).

---

## 3. Modèle de données

### 3.1 `wiki_link` — l'appariement

Un appariement relie un mod, **par le nom de son dossier** (stable d'une installation à l'autre), à un **identifiant Wikidata**. **Jamais une URL d'article** : le Q-id est indépendant de la langue et survit aux renommages ; une URL saisie est résolue en Q-id, puis jetée. Chaque appariement porte sa source, avec une **précédence stricte : `manual` > `import` > `auto`** — une entrée `manual` n'est jamais écrasée automatiquement.

### 3.2 `wiki_cache` — le contenu

L'article d'une entité dans une langue (titre, URL **stockée avec le contenu**, numéro de révision pour l'attribution, rendu et texte brut, entité parente quand le repli de §5.3 a joué, langues où l'article existe), gardé **30 jours**.

### 3.3 `wiki_no_match` — le cache négatif

**Indispensable** : sans lui, chaque ouverture d'une fiche sans correspondance relancerait une résolution complète. Gardé **90 jours** (un article peut être créé entre-temps).

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

1. Si coordonnées disponibles : recherche géographique d'articles (`list=geosearch`) dans un
   rayon de **10 km**, le maximum de l'API — les coordonnées que CSP donne d'un circuit sont
   parfois celles de la ville voisine ; une route garde un rayon bien plus court.
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
Bunny`, `Liberty Walk`), suffixes de conversion (`AC`, `Conversion`, `Fixed`). La liste et
les poids du scoring vivent dans un fichier de données (`rules/wiki-matching.json`), pas
dans le code.

### 4.4 Identifiants Wikidata

Les identifiants de propriétés et de classes Wikidata sont **vérifiés avant d'être câblés**,
et regroupés dans un seul module de constantes commenté (`wiki/ids.rs`).

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

L'**Action API** (`/w/api.php`), la plus stable (les API REST de Wikimédia sont en cours de
retrait). **Un article demande trois requêtes**, séquentielles et mises en cache d'un bloc :
`action=query` (métadonnées, langues, Wikidata, révision), `action=parse` (HTML rendu et
sommaire), et un `imageinfo` groupé pour l'auteur et la licence de chaque image — qui
n'est pas facultatif : c'est lui qui autorise l'affichage (§9).

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

Le bloc textuel de la fiche a trois sous-onglets : `Description`, `Notes`, et l'article —
`Le modèle réel` (voitures) ou `Le circuit` (circuits).

**L'onglet est permanent** : un onglet absent ne se distingue ni d'une recherche en cours,
ni d'une fonctionnalité qui n'existe pas, et il priverait l'utilisateur de tout recours
au moment où l'appariement est raté — ce que §7.6 lui permet de corriger. L'état vide n'est
donc **jamais un cadre vide, jamais une alerte** : une phrase qui dit ce qui s'est passé,
et une proposition d'agir. Six états, tous non-erreurs :

| État | Ce que l'onglet dit |
|---|---|
| recherche en cours | qu'il cherche — le seul état qui n'est pas un verdict |
| article trouvé | l'article (§7.3) |
| rien trouvé | ce qui a été cherché, et « Associer un article… » |
| ambiguïté | que plusieurs articles se valaient, et la même proposition |
| entité sans article lisible | qu'aucune langue lue n'a d'article |
| enrichissement désactivé | le réglage, sans reproche (§8) |

### 7.2 Onglet par défaut

`Description`, toujours, à chaque ouverture de fiche.

### 7.3 Contenu de l'onglet

- Si repli parent actif : une ligne discrète en tête, `Article général : {titre}`.
  Affichée uniquement dans ce cas.
- **L'article entier, rendu** : on lit dans Pit Box, avec ses sections, ses tableaux, son
  infobox et ses images (§9).
- **Une table des matières**, construite sur l'arbre de sections que l'API rend.
- **Le HTML n'est jamais injecté tel quel.** La webview a accès à `invoke`, et Wikipédia
  est éditable par n'importe qui : un arbre neuf est **reconstruit** à partir d'une liste
  blanche de balises et d'attributs (`src/lib/wiki/wikiHtml.ts`), plutôt que filtré. Ce qui
  n'est pas explicitement prévu n'existe pas. Aucune dépendance : ni assainisseur Rust,
  ni DOMPurify — la sécurité vient de la liste blanche.
- **Le texte brut reste récupéré en plus du rendu**, et sert de repli quand celui-ci
  échoue. Un article dégradé vaut mieux qu'un onglet vide (§1).
- Sélecteur de langue.
- Lien **« Voir sur Wikipédia »** ouvrant le **navigateur système** — ce qu'il apporte encore,
  ce sont les références, la navigation et la version vivante de la page. Jamais une
  webview pointant vers wikipedia.org : elle casserait le mode hors ligne, imposerait leur
  CSP et ferait perdre l'identité visuelle de l'application.
- Bloc d'attribution (§7.4).

### 7.4 Attribution

En pied d'onglet, obligatoire :

> Article **{titre}** de Wikipédia — {lien} · Texte disponible sous licence
> [CC BY-SA 4.0]({lien vers la licence})

Pas de mot « extrait » : l'article est reproduit **intégralement et tel quel**, ce que
CC BY-SA autorise sans réserve. Si l'affichage redevenait partiel un jour, il faudrait le
dire (« extrait ») : n'afficher qu'un fragment est une modification.

### 7.5 Chargement

La fiche s'affiche **complète et immédiatement** avec sa photo et ses caractéristiques.
L'onglet Wikipédia apparaît lorsque le contenu arrive. Réserver l'espace ou utiliser une
transition pour éviter tout décalage de mise en page à l'arrivée du contenu.

### 7.6 Correction manuelle

**Dans l'onglet lui-même** : quand rien n'a été trouvé, l'onglet ne contient que ça, et
quand un article est affiché, « Ce n'est pas le bon article ? » est en pied. **Pas un
assistant, pas de validation au premier affichage** : l'utilisateur vient corriger s'il le
veut, on ne lui demande rien.

Ouvre une recherche libre affichant les candidats avec leur description courte Wikidata
(« modèle d'automobile Toyota, 1983–1987 »), qui lève l'ambiguïté d'un coup d'œil. Le
champ est **pré-rempli avec ce qui a été cherché** : les règles de nettoyage (§4.3)
vivent côté Rust, et l'utilisateur doit pouvoir corriger des mots-clés plutôt que tout
retaper. Accepte également le collage d'une URL Wikipédia, résolue immédiatement en
Q-id — et l'URL n'est jamais stockée (§3.1).

Un bouton **détache** l'article : un mod sans appariement est une réponse valable (§1),
et le cache négatif est vidé au passage pour que l'appariement automatique ait une
nouvelle chance au lieu de rester condamné 90 jours.

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
- **Partager vos corrections** — écrit le contenu de `rules/wiki-links.json` avec les
  corrections locales fondues dedans, prêt à recoller dans le dépôt (§10). **Seules les
  entrées `manual` en sortent** : exporter les verdicts automatiques les figerait dans le
  binaire, où la précédence (`import` > `auto`) les ferait ensuite écraser un moteur
  futur, meilleur, par ses propres vieilles réponses.

Le tout vit dans `Réglages › Wikipédia`.

---

## 9. Images

**Les images sont affichées**, et chacune des trois objections qu'elles soulèvent a sa
réponse :

| Objection | Réponse |
|---|---|
| Chaque image a sa propre licence, distincte du texte | Elle est lue par fichier (`imageinfo`/`extmetadata`), jamais supposée |
| Beaucoup sont non libres (usage loyal), surtout les logos | **Seuls les fichiers hébergés sur Commons** sont affichés (`imagerepository == "shared"`). Commons n'accepte que du libre ; l'usage loyal est hébergé localement par le wiki, donc écarté **structurellement** et non au cas par cas — mesuré sur une pochette d'album, qui revient sans dépôt, sans vignette et sans licence |
| Même libre, il faut créditer l'auteur individuellement | Une ligne de crédit — auteur et licence — sous **chaque** image, non masquable. Un fichier sans auteur ou sans licence connus n'est pas affiché |

Une vignette est demandée au serveur (640 px), jamais l'original : une photo de Commons
fait couramment vingt mégapixels, et rien ici n'en a l'usage.

**La taille d'affichage voyage avec le crédit**, et l'image la réserve avant d'arriver :
sans elle l'article se met en page trop court, et chaque image qui se charge repousse le
lecteur. C'est la largeur **demandée** qu'on réserve, pas celle du fichier plus dense que
Commons peut servir aux écrans à haute résolution.

Ce que ça ne change pas : les previews du mod restent les images de la fiche. Celles de
l'article illustrent un texte, elles ne prétendent pas montrer le mod.

---

## 10. Préparation du partage communautaire

**Ce qui existe** : des appariements **livrés avec l'application** (`rules/wiki-links.json`),
repris en source `import` — ils passent devant l'automatique et jamais devant une
correction locale, et un fichier compilé dans l'application porte la confiance du binaire.
L'export des corrections (§8) produit ce fichier. La clé qui circule est le nom du
dossier du mod, jamais le nom affiché.

**Ce qui n'existe pas, et la règle qui l'attend** : importer le fichier d'un tiers. Ce
jour-là, la validation sera stricte — Q-id par format, URL **sur une liste blanche des
sous-domaines de `wikipedia.org`** (un homographe du type `wikipedıa.org` ouvrirait un
hameçonnage), import toujours explicite.

---

## 11. Tests

Le cœur logique vit dans des fonctions pures, testées sans réseau : la chaîne de repli de
langue (§5.2), le nettoyage des noms sur des noms réels, le rejet des candidats trop
proches, la précédence des sources, la validation d'URL et le cache négatif. Aucun test ne
dépend de la disponibilité de Wikipédia.

---

## 12. Découpage proposé

Livré ; le reste à faire est dans `CHANTIERS.md`.

---

## 13. Points à trancher à l'implémentation

Tranchés, et réglables par la mesure plutôt qu'à l'écran (paramètres de `Prefs`,
`wiki_match_*`, `wiki_track_*`, jamais à l'écran, §8) : score minimal 0,70,
écart minimal 0,12 entre les deux premiers candidats, rayon de 10 km pour les circuits
(§4.2), caches de 30 et 90 jours (§3). L'onglet par défaut est toujours la description
(§7.2) : le basculement sur l'article quand la description est courte n'a pas été retenu.
