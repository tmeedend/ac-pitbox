# Pit Box — taxonomies

## Spécification d'implémentation

*Marques, pays et catégories deviennent des entités du produit, de forme identique. **Remplace intégralement la spec « marques et logos »**, dont le contenu est repris et généralisé.*

---

# 1. Objet

Trois informations sont aujourd'hui des **chaînes de caractères** extraites des mods : la marque d'une voiture, le pays d'un circuit, les tags de catégorie. Trois conséquences identiques :

- `Alfa` et `Alfa Romeo`, `Japan` et `japan`, sont des valeurs distinctes dans les filtres ;
- rien ne peut leur être attaché — notamment pas un emblème ;
- rien ne les regroupe : `#LMP1`, `#group-c` et `#LMH` ne se savent pas parents.

Elles deviennent des **termes** : un nom canonique, une liste d'alias, un emblème.

---

# 2. Une seule forme, trois usages

```ts
type TaxonomyKind = 'brand' | 'country' | 'category';

interface Term {
  kind:    TaxonomyKind;
  id:      string;        // slug canonique, stable — 'alfa-romeo', 'jp', 'prototype'
  name:    string;        // nom affiché — 'Alfa Romeo', 'Japon', 'Prototype'
  aliases: string[];      // chaînes brutes rencontrées dans les mods
  emblem:  Emblem;
  hidden?: boolean;       // retiré des index, conservé en base
}
```

**Résolution, commune aux trois :** une chaîne brute est cherchée dans les `aliases` de tous les termes du même `kind`. Trouvée → le terme canonique. Non trouvée → un nouveau terme est créé, avec cette chaîne comme nom et comme premier alias.

C'est un **index, pas un moteur de règles** (§8). Un mod importé demain trouve ses termes par simple recherche, sans réévaluation, et hérite de leurs emblèmes au passage.

## 2.1 Ce qui diffère d'un usage à l'autre

| | **Marque** | **Pays** | **Catégorie** |
|---|---|---|---|
| Source des chaînes | champ `brand` | champ `country` | tags commençant par `#` |
| Cardinalité par mod | 1 | 1 | **N** |
| **Partitionne la bibliothèque** | oui | oui | **non** |
| Emblème | logo issu des mods, **curé** (§4) | drapeau **embarqué** | icône **embarquée** |
| Variants à départager | oui | non | non |
| Deux niveaux d'emblème | oui (§3) | non | non |
| S'applique à | voitures | voitures **et** circuits | voitures |

**La ligne qui compte le plus est la cardinalité.** Une voiture porte une marque et un pays, mais plusieurs catégories : sur l'installation de référence, 535 appartenances de catégorie pour 287 voitures. Une 250 GTO est Classic, Sportscars *et* Race.

Conséquence directe, spécifiée dans la spec index : poser une seconde catégorie **croise** (`ET`), poser une seconde marque ou un second pays **remplace**.

## 2.2 Pays : une table pour deux bibliothèques

Le champ `country` existe dans les métadonnées des circuits comme dans celles des voitures. **Une seule table de pays**, partagée. Curer `Japan` / `japan` / `JP` une fois corrige les deux bibliothèques.

---

# 3. Emblème — deux niveaux, pour les marques seulement

**Un logo de marque évolue avec le temps.** Le blason Porsche a changé, le rond BMW s'est aplati en 2020, Nissan a refait le sien la même année. Une Fairlady de 1969 portant le logo d'époque et une GT-R de 2013 portant le moderne, **ce n'est pas une incohérence — c'est juste**.

Mais toutes les différences entre variants ne viennent pas de là. Trois causes coexistent, indiscernables depuis le fichier seul : la justesse historique, la qualité variable des sources, et les défauts francs comme un fond blanc cuit.

La bonne question n'est donc pas *quel logo*, c'est **à quel niveau**.

| Niveau | Où | Emblème utilisé | Curation |
|---|---|---|---|
| **Voiture** | carte de grille, colonne de session, fiche de détail | le `badge.png` **de cette voiture**, tel quel | **aucune** |
| **Marque** | index par marque, éditeur du filtre `Marque`, onglet Marques | un logo **canonique** | §4 et §6 |

Au niveau voiture, la variation d'époque est signifiante et l'application n'a rien à corriger — c'est déjà le comportement actuel, il est conservé.

Au niveau marque, il n'y a pas d'année à représenter : la marque est une catégorie, pas un millésime.

**Pays et catégories n'ont qu'un niveau.** Leur emblème est embarqué dans l'application, identique partout, sans curation d'image possible.

## 3.1 Pays — drapeaux embarqués

Jeu de drapeaux SVG livré avec l'application, indexé par code ISO 3166-1 alpha-2. Contrairement aux logos de marque, **les drapeaux nationaux ne sont pas des marques déposées** : les embarquer ne pose aucun problème (§13).

La curation d'un pays ne porte donc que sur **le nom et les alias**, jamais sur l'image. Le terme porte son code ISO ; le drapeau en découle.

Un pays sans code ISO reconnu (`Nürburgring Land`, valeur fantaisiste d'un mod) reçoit un emblème neutre et apparaît dans les propositions de fusion.

## 3.2 Catégories — icônes embarquées

Jeu d'une douzaine de silhouettes SVG livrées avec l'application : berline de rue, coupé sportif, classique, voiture de course à aileron, prototype fermé, monoplace, drift, rallye, et un glyphe neutre pour `Non classé`.

L'utilisateur **choisit** l'icône d'une famille dans ce jeu ; il n'en fournit pas. Une famille inventée par l'utilisateur reçoit le glyphe neutre tant qu'il n'en a pas assigné une.

Trait uniforme, remplissage `#d4d5d7`, hauteur de rendu 34 px dans les tuiles d'index.

---

# 4. Sélection automatique du logo canonique

*Marques uniquement. Le logo affiché sur une voiture reste le sien (§3).*

**Le gros du travail doit se faire sans l'utilisateur.** Cent marques à curer à la main, personne ne le fera. La curation manuelle est un rattrapage, pas une corvée d'installation.

Quand plusieurs mods fournissent un `badge.png` pour la même marque, le variant retenu est choisi dans cet ordre :

| # | Critère | Pourquoi |
|---|---|---|
| 1 | **Fond transparent** | le plus discriminant — c'est le cas Porsche, dont le logo Kunos porte un fond blanc cuit |
| 2 | Résolution la plus élevée | |
| 3 | Vote majoritaire | le variant utilisé par le plus de voitures est probablement le canonique |
| 4 | Identifiant de mod, ordre alphabétique | départage déterministe : deux lancements donnent le même résultat |

**Le vote majoritaire s'adapte tout seul à la collection.** Une bibliothèque à dominante moderne élit le logo moderne ; une collection des années 60 élit celui d'époque. Aucune règle de récence n'est nécessaire, et il ne faut pas en ajouter : elle imposerait l'identité actuelle d'une marque à quelqu'un qui ne possède que ses voitures anciennes.

**Détection du fond opaque.** Échantillonner l'anneau de bordure de l'image : si plus de 95 % des pixels du pourtour sont opaques et de teinte quasi uniforme, le logo est classé « fond cuit ». Seuil en dur dans le code, pas exposé.

Cette détection sert deux fois : au critère 1 ci-dessus, et au mode de rendu (§5).

---

# 5. Rendu d'un logo à fond cuit

Un logo à fond blanc opaque bave sur une interface sombre. Il est alors affiché dans une **pastille claire arrondie** — fond `#ececed`, rayon 2 px, `object-fit: contain`, marge intérieure 2 px. Ça le fait passer pour un choix graphique au lieu d'un défaut.

**Le mode de rendu est une propriété du fichier, pas du terme.** Détecté automatiquement (§4) et **appliqué partout où ce fichier s'affiche** — y compris sur la carte de grille et la fiche de détail, où le logo n'est pourtant pas curé.

C'est ce qui règle le cas Porsche sans toucher au niveau voiture : le badge Kunos à fond blanc reste celui de la voiture, mais il est rendu correctement.

Le mode reste corrigeable manuellement pour le logo canonique — un fond peut être détecté à tort. Au niveau voiture, la détection est automatique et non modifiable ; corriger 312 badges à la main n'a pas de sens.

Sans objet pour les pays et les catégories, dont les emblèmes sont embarqués et transparents par construction.

---

# 6. Les trois onglets de l'Atelier

`Marques`, `Pays` et `Catégories` rejoignent Règles, Importer, Profils et Maintenance. **L'Atelier passe à sept onglets, à plat, sans sous-rubrique** — la contrainte de la spec navigation, REFONTE§3, est respectée.

> **Seuil.** Sept onglets restent lisibles. C'est la limite : le prochain outil ajouté impose de passer à une liste latérale dans l'écran plutôt qu'à un huitième onglet.

## 6.1 Liste — structure commune

Une ligne par terme :

```
[emblème 20px]  Alfa Romeo        42 voitures    3 variants     ›
[emblème 20px]  Porsche           61 voitures    5 variants  ⚑  ›
[plaque      ]  Abarth             4 voitures    1 variant      ›
```

Colonnes : emblème, nom canonique, nombre de mods rattachés, nombre de variants (marques seulement), fanion de curation manuelle, chevron.

Tri par nombre de mods décroissant : les termes qui comptent sont ceux qu'on voit le plus.

**Colonne supplémentaire pour les catégories :** le nombre de tags rattachés à la famille.

## 6.2 Détail d'un terme

Cliquer une ligne la déplie :

| Champ | Marque | Pays | Catégorie |
|---|---|---|---|
| Nom canonique, éditable | ✓ | ✓ | ✓ |
| Alias | chaînes de marque | chaînes de pays | **tags `#…`** |
| Code ISO | — | ✓ | — |
| Variants de logo, cliquables | ✓ | — | — |
| Dépôt d'un fichier personnel | ✓ (§9) | — | — |
| Choix d'icône dans le jeu embarqué | — | — | ✓ |
| Mode de rendu `direct` / `plaque` | ✓ | — | — |
| Rétablir le choix automatique | ✓ | ✓ | ✓ |

## 6.3 Prévisualisation aux tailles réelles

**Obligatoire pour les marques.** L'éditeur montre le logo retenu aux tailles d'usage — 13, 18, 20 et 32 px — et sur les deux fonds, sombre et pastille claire.

Un logo choisi sur une grande prévisualisation se révèle illisible à 13 px une fois sur trois. Il faut voir la vraie taille au moment de choisir, pas après.

---

# 7. Fusion des variantes de nom

Mécanique identique pour les trois taxonomies.

## 7.1 Ce qui fusionne tout seul

**Uniquement les différences de casse, d'espaces et d'accents.** `PORSCHE` / `Porsche` / `porsche`, `Japan` / `japan` sont la même chaîne ; les fusionner n'est pas une décision.

**Exception pays :** un code ISO valide fusionne aussi automatiquement avec le nom du pays correspondant. `JP` → `Japon` n'est pas un jugement, c'est une table normalisée.

## 7.2 Ce qui est proposé

Tout le reste. Pit Box classe les candidats par proximité — préfixe commun, inclusion, distance d'édition faible — et affiche un bandeau en tête de l'onglet :

```
⚑  3 fusions proposées
    Alfa  →  Alfa Romeo          38 + 4 voitures      [ Fusionner ]  [ Ignorer ]
    Mercedes  →  Mercedes-Benz   12 + 47 voitures     [ Fusionner ]  [ Ignorer ]
    Nismo  →  Nissan              6 + 61 voitures     [ Fusionner ]  [ Ignorer ]
```

**Jamais de fusion silencieuse au-delà de §7.1.** Elle détruirait des distinctions que l'utilisateur veut peut-être garder — `BMW` et `BMW Motorsport`, `Nissan` et `Nismo` — et elle serait invisible : impossible de comprendre pourquoi un filtre remonte quarante voitures au lieu de douze.

`Ignorer` est **définitif et mémorisé** : la proposition ne réapparaît pas au prochain scan. Réversible depuis le détail du terme.

## 7.3 Le cas des catégories est différent

Pour les marques et les pays, fusionner signifie *ces deux valeurs sont la même*. Pour les catégories, rattacher un tag à une famille signifie *ce tag appartient à cette famille* — une relation de parenté, pas d'identité.

```
Prototype  ←  #prototype, #group-c, #lmp1, #lmp2, #lmp3, #lmh, #gtp
Race       ←  #gt3, #gt2, #dtm, #touring, #btcc, #nascar
```

Conséquences :

- **Le tag reste utilisable seul** comme filtre. Rattacher `#lmp1` à `Prototype` ne le fait pas disparaître ; il reste dans le filtre `Tag`.
- **Un tag n'appartient qu'à une famille.** Le rattacher ailleurs l'y déplace. Sinon les compteurs d'index deviennent incompréhensibles.
- **Les propositions se font sur le vocabulaire du domaine**, pas sur la distance d'édition seule : une table de départ livrée avec l'application couvre les tags courants d'Assetto Corsa, et l'utilisateur complète.

## 7.4 Bénéfices en cascade

Une fusion acceptée produit, d'un seul coup :

- une liste de filtre plus courte et plus propre ;
- un index plus lisible (spec index) ;
- pour les marques, la règle de retrait du préfixe dans le nom (GRILLE§3.4) qui mord sur toutes les variantes, et le logo appliqué à toutes les voitures fusionnées.

---

# 8. Migration depuis `Règles`

La correction des marques quitte `Règles` pour l'onglet Marques. **Même chose pour toute correction de pays ou de catégorie qui y figurerait.**

**Raison :** ce n'est pas la même mécanique. `Règles` est un moteur *conditions → actions*, appliqué à des critères flous — « si le dossier contient *drift*, alors tag drift ». Une correction de terme est une **table de correspondance exacte** : une chaîne, un terme canonique. Ce n'est pas une règle, c'est un index.

**Conversion au premier lancement de la version qui introduit les onglets :** chaque correction existante devient un alias du terme canonique cible. Les règles correspondantes sont retirées de `Règles`, et un message unique, non bloquant, signale le déplacement.

**Rien ne doit être perdu.** Si une correction ne peut pas être convertie — cible ambiguë, condition composite — elle reste dans `Règles` et est listée dans le message.

---

# 9. Fichier personnel de logo

*Marques uniquement.*

Formats acceptés : PNG et SVG. Le PNG doit faire au moins 64 px sur son plus petit côté.

**Trois contraintes :**

**Il est copié dans le dossier de données de Pit Box, jamais écrit dans le dossier Assetto Corsa.** Un gestionnaire de mods qui modifie les dossiers qu'il gère se met en position de casser une réinstallation, et l'utilisateur ne s'attend pas à ce que consulter une marque touche à son jeu.

**Il survit à la suppression de tous les mods de cette marque.** Sinon le travail de curation est perdu en désinstallant un pack. Un terme sans mod rattaché passe en `hidden`, reste en base, et réapparaît si un mod le réutilise. Un filtre `Afficher les termes sans mod` le montre.

**Il est prévisualisé aux tailles réelles avant validation** (§6.3), avec détection automatique du mode de rendu, modifiable.

---

# 10. Où les emblèmes apparaissent

| Emplacement | Taxonomie | Niveau | Taille |
|---|---|---|---|
| Surtitre d'une carte de voiture | marque | voiture | 13 px |
| Surtitre d'une carte de circuit | pays | — | 14 × 10 px |
| Colonne de session, ligne de source | marque | voiture | 13 px |
| Fiche de détail d'une voiture | marque | voiture | 32 px |
| Tuile d'index par marque | marque | **terme** | 32 px |
| Tuile d'index par pays | pays | — | 44 × 29 px |
| Tuile d'index par catégorie | catégorie | — | 34 px de haut |
| Éditeur d'un filtre — suggestions et jetons | toutes | **terme** | 18 px |
| Onglets de l'Atelier | toutes | **terme** | 20 px |
| **Résumé d'une puce de filtre** | toutes | **terme** | 16 × 11 px, **une seule valeur** |

**L'emblème dans une puce n'apparaît que si la puce porte une valeur unique.** Dès qu'elle en porte plusieurs (`Catégorie : ET Prototype, Race`), les emblèmes sont omis : ils rendraient la puce illisible et casseraient sa hauteur de 26 px.

**Chemin vers l'éditeur :** sur la fiche de détail, le nom de la marque porte une action discrète qui ouvre `Atelier › Marques` positionné dessus. La fiche n'affiche pourtant pas le logo canonique — mais c'est là qu'on pense à la marque, donc c'est un point de départ naturel.

**Pas d'édition dans le filtre.** Le popover porte déjà l'inclusion et l'exclusion ; y ajouter de la curation mélangerait deux modes dans un espace étroit, et obligerait à dupliquer le même éditeur.

---

# 11. Repères visuels

| Rôle | Valeur |
|---|---|
| Pastille claire (`render: 'plaque'`) | fond `#ececed`, rayon 2 px, marge intérieure 2 px |
| Icône de catégorie | remplissage `#d4d5d7`, `#fff` au survol |
| Terme curé manuellement (fanion) | `--accent-dim` `#7e2415` |
| Bandeau de fusions proposées | fond `#1b1811`, filet gauche 2 px `#c88a2a` |
| Emblème manquant | glyphe neutre `--txt-4`, jamais de case vide |

---

# 12. Localisation

**Se traduisent :** libellés de colonne, textes de fusion, mentions `fond transparent` / `fond cuit`, boutons, **les noms de pays** et **les noms de famille de catégorie**.

**Ne se traduisent pas :** les noms de marque — ce sont des noms propres — ni les tags bruts.

Les noms de pays se traduisent à partir du code ISO, pas de la chaîne du mod : `JP` donne `Japon`, `Japan`, `Japón` selon la locale. C'est une raison de plus de stocker le code plutôt que le libellé.

Les familles de catégorie livrées avec l'application sont traduites ; une famille créée par l'utilisateur garde le nom qu'il lui donne, dans toutes les langues.

---

# 13. Hors périmètre

**Un catalogue de logos de marque embarqué** est exclu : ce sont des marques déposées, et les redistribuer est un risque que le produit n'a pas à prendre. Tous les logos viennent des mods de l'utilisateur ou de ses propres fichiers. Les drapeaux et les icônes de catégorie, eux, sont embarqués sans difficulté (§3.1, §3.2).

**La normalisation des noms de modèle** — au-delà du retrait du préfixe de marque et de pack (GRILLE§3.4) — n'est pas couverte.

**Les catégories de circuits.** Cette spec rattache les tags aux familles pour les voitures. Les circuits ont aussi des tags ; le même mécanisme s'y appliquerait, mais la valeur est moindre puisque le pays y suffit comme axe de parcours. À évaluer séparément.

---

*Pit Box · spécification taxonomies · remplace la spec « marques et logos » · à lire avec les specs index, grille, filtres et navigation*
