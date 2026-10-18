# Pit Box — index de bibliothèque

## Spécification d'implémentation

*L'état par défaut des écrans Circuits et Voitures devient un index visuel. Ne remplace rien : complète les specs filtres, grille et taxonomies.*

> **Deux maquettes accompagnent cette spec** — `maquettes/pitbox-index-pays.html` et `maquettes/pitbox-index-voitures.html`. Elles servent à juger les partis pris d'interaction, **pas de référence graphique**, et **l'une d'elles contient une section à ne pas construire** : voir §6.3. En cas de divergence, la spec gagne.

---

# 1. Objet

Les puces de filtre servent à **réduire** : je sais à peu près ce que je cherche, je resserre. Il manque l'autre moitié — **parcourir** : je ne sais pas ce que je veux, montre-moi ce que je possède.

Arriver sur Circuits ou Voitures sans filtre afficherait un mur de deux à trois cents vignettes, classées alphabétiquement : un état par défaut qui ne sert à rien.

Il devient un **index** : une grille de tuiles emblématiques qui donne une vue d'ensemble de la collection, et qui sert de porte d'entrée vers le filtre.

---

# 2. Deux règles fondatrices

## R1 — L'index est grossier, le filtre est fin

**Un index ne dépasse jamais un écran de tuiles.** Sinon on a remplacé un mur par un autre mur.

En pratique : une dizaine de familles de catégorie, une vingtaine de pays, les marques significatives seulement (§6.2). Le détail — `#LMP1`, `#group-c`, `#LMH` — reste accessible par les tags, croisables une fois dans la liste.

Qui cherche précisément une LMP1 ne parcourt pas : il sait ce qu'il veut, il tape. **L'index sert à celui qui ne sait pas.**

## R2 — La tuile pose une puce

**C'est le point critique de cette spec.**

Cliquer une tuile n'ouvre pas un mode, ne change pas d'écran, ne construit pas un état parallèle. Elle **écrit dans l'état de filtre existant** — la `FilterMap` de la barre de filtres, `SPEC.md` §7.1 — exactement comme si l'utilisateur avait ajouté la valeur depuis l'éditeur d'une puce.

Tout ce qui suit continue donc de fonctionner sans code supplémentaire : croiser avec une année, exclure un tag, l'opérateur `ET`/`OU`, « Tout effacer », le compteur de résultats, l'épinglage.

**À ne pas faire :** un second système de sélection qui filtrerait la liste en parallèle des puces. Les maquettes montrent un tableau `chips` autonome parce qu'elles sont autonomes ; dans l'application, il n'y a qu'un seul état de filtre.

---

# 3. Quand l'index s'affiche

| Condition | Écran |
|---|---|
| Aucune puce active **et** champ de recherche vide | **index** |
| Au moins une puce, ou du texte dans la recherche | liste |

Retirer la dernière puce ramène à l'index. « Tout effacer » aussi. Il n'y a pas de bouton « retour à l'index » : la sortie est la même que pour n'importe quel filtre.

**L'index n'est pas mémorisé et ne se replie pas.** C'est un état, pas une préférence.

**La bascule de vue a quatre positions, toujours** : index, puis les trois densités de la liste (dense, aérée, tableau). Elle en changeait — index / liste sans filtre, les trois densités avec — et la densité qu'on avait choisie était un clic plus loin qu'avant l'index (signalé à l'usage, 2026-10-03). Une densité quitte l'index pour la liste entière (« voir tout ») ; l'index ne s'affichant que sans filtre, son bouton reste visible mais grisé quand un filtre est posé, plutôt que d'effacer ce qui l'est.

---

# 4. Anatomie d'une tuile

```
┌──────────────┐
│   [emblème]  │
│   Japon      │
│  34 circuits │
└──────────────┘
```

Un emblème (§4.1), le nom (deux lignes possibles) et le compteur, centrés. Fond, bordure, survol et rayon : ceux du design system.

## 4.1 Emblème par taxonomie

| Taxonomie | Emblème | Taille |
|---|---|---|
| Pays | drapeau du jeu (TAXO§3.1) | 44 × 29 px |
| Catégorie | silhouette ou pictogramme embarqué (TAXO§3.2, TAXO§3.3) | 34 px de haut |
| Marque | logo élu de la marque (TAXO§4) | 32 px |

Les emblèmes viennent de la spec taxonomies. Une marque sans logo reçoit ses initiales dans la pastille ; jamais une case vide.

## 4.2 Largeur de tuile par grille

| Grille | `minmax` | Raison |
|---|---|---|
| Catégories | 134 px | le nom peut être long (`Open-wheel`) et la silhouette est large |
| Pays | 124 px | |
| Marques | 112 px | plus nombreuses, emblème plus petit |

Gouttière 9 px, `auto-fill`, dans tous les cas.

## 4.3 Tuile de valeur absente

Dernière position, bordure **pointillée**, emblème neutre :

- `Non renseigné` sur l'index des pays ;
- `Non classé` sur l'index des catégories.

Elle rend le trou de données visible au lieu de le cacher, et c'est l'entrée naturelle vers la curation : ces mods existent, ils sont seulement introuvables par cet axe.

Elle se comporte comme les autres : elle pose une puce.

---

# 5. Écran Circuits

Deux index empilés, **dans cet ordre** : catégories (§5.2), puis pays.

```
Parcourir par pays            21 pays représentés        [ Voir tous les circuits ]
────────────────────────────────────────────────────────────────────────────────
 🇩🇪 Allemagne   🇧🇪 Belgique    🇪🇸 Espagne      🇺🇸 États-Unis    🇫🇷 France
 29 circuits     6 circuits     9 circuits       21 circuits      17 circuits
 …
 ⬚ Non renseigné
 14 circuits
```

**Pourquoi le pays fonctionne ici.** Un circuit *est* son lieu. Le drapeau est le symbole le plus universellement reconnaissable qui existe, la vingtaine de pays tient dans une grille lisible, et la répartition est raisonnablement équilibrée.

**Par ordre alphabétique**, sur le nom affiché (donc traduit), et Non renseigné toujours en dernier. Les pays se classaient d'abord par nombre de circuits ; mais un circuit se cherche par son pays, et l'ordre par nombre éparpillait la liste (demandé à l'usage).

## 5.1 Voir tous les circuits

Lien en tête de la **première** section, aligné à droite — celle des catégories quand il y en a, sinon celle des pays : c'est la sortie de l'index, elle ne doit pas tomber sous la ligne de flottaison. **Obligatoire** : personne ne doit être forcé de passer par un pays pour chercher Spa.

Il n'ajoute aucune puce — il bascule simplement en vue liste, sans filtre. Techniquement, c'est le seul chemin vers la liste complète qui ne passe pas par une puce ; il mérite donc son propre état, distinct de « index » et de « liste filtrée ».

Dans cette vue, le drapeau du surtitre de chaque carte devient une information réelle, puisque les pays se mélangent.

## 5.2 Parcourir par catégorie

Une tuile par catégorie de circuit : son pictogramme (TAXO§3.3), son nom tel que Familles › Circuits le donne (TAXO§6.4), et le nombre de circuits qui la portent. Ordre par nombre décroissant, comme les familles de voitures ; tuile `Non classé` en pointillés à la fin (§4.3), pour les circuits qui n'en portent aucune.

Une tuile pose la puce **Catégorie**, multi-valuée pour les circuits, avec l'opérateur ET/OU déjà en place (§7). Les décomptes ne s'additionnent pas — un circuit peut être à la fois circuit et drift —, et le sous-titre de la section le dit, comme pour les familles.

Les pictogrammes apparaissent aussi dans l'éditeur du filtre Catégorie, à côté de chaque valeur, et dans une puce qui ne porte qu'une catégorie ; au-delà d'une, la puce n'aligne que les noms (même règle que les logos de marque, TAXO§10). Le filtre « Track category » de la page Online fait de même.

---

# 6. Écran Voitures

Deux index empilés, **dans cet ordre** : catégories, puis marques.

L'ordre n'est pas arbitraire. Les catégories sont plus grossières, plus visuelles, moins nombreuses — elles tiennent au-dessus de la ligne de flottaison, et c'est la question la plus fréquente (« je veux rouler en GT ce soir »).

## 6.1 Parcourir par catégorie

Une dizaine de familles (TAXO§7.3), plus `Non classé`.

**Les compteurs ne s'additionnent pas.** Sur l'installation de référence : 68 + 6 + 22 + 11 + 103 + 4 + 118 + 203 = **535 appartenances pour 287 voitures**. Une 250 GTO est Classic, Sportscars *et* Race.

Deux conséquences d'interface :

**Le sous-titre de section le dit** : *« les familles se recoupent — une voiture peut en porter plusieurs »*. Sans cette ligne, un utilisateur qui additionne croit à un défaut.

**Cliquer une seconde catégorie croise au lieu de remplacer** (§7).

## 6.2 Parcourir par marque

**Seules les marques significatives sont affichées**, plus un lien `Toutes les marques (N)` qui déplie la liste entière, **toute par ordre alphabétique** (les significatives ne restent pas en tête), et `Réduire` qui revient aux significatives, par nombre.

Soixante-dix marques dont la moitié ne mène qu'à une seule voiture : une grille complète serait surtout du bruit, et beaucoup de clics pour rien.

**Seuil dynamique — pas de constante en dur :**

```
Afficher les marques qui, cumulées par compte décroissant,
couvrent 80 % de la bibliothèque.
Borné à [8, 24] tuiles.
```

Sur l'installation de référence, ça donne quatorze marques. Sur une bibliothèque de quarante voitures, ça en donne huit ; sur une de deux mille, vingt-quatre. **Un « top 18 » figé casserait dans les deux cas.**

L'état déplié n'est pas mémorisé.

## 6.3 Pas d'index par pays pour les voitures

**Décision explicite. La maquette `pit-box-index-voitures.html` contient une section « Parcourir par pays », activable par un bouton — elle existe pour permettre de juger, et la conclusion est de ne pas la construire.**

Deux raisons :

**Trois grilles empilées redeviennent un mur.** C'est exactement ce que l'index remplace.

**L'axe est faible pour une voiture.** « Japon » redit ce que `#jdm` dit déjà, en moins bien : le tag exprime un genre, le pays n'exprime qu'un lieu d'immatriculation. Pour un circuit c'est l'inverse — le lieu *est* l'identité.

Le pays des voitures reste un filtre ordinaire, atteignable par `+ Filtre`.

---

# 7. Cumul — catégorie croise, marque et pays remplacent

| Taxonomie | Poser une seconde valeur | Raison |
|---|---|---|
| **Catégorie** | **s'ajoute**, opérateur `ET` | une voiture porte plusieurs catégories (§6.1) |
| Marque | remplace | une voiture porte une marque |
| Pays | remplace | un circuit porte un pays |

`Prototype` puis `Race` a un sens et remonte les prototypes de course. `Japon` puis `Italie` n'en aurait aucun.

Cette asymétrie découle de la cardinalité déclarée dans la spec taxonomies, TAXO§2.1 — **elle n'est pas à coder en dur par taxonomie**, mais à déduire de `cardinalité = 1 | N`.

Pour combiner des catégories en `OU` plutôt qu'en `ET`, l'utilisateur passe par l'éditeur de la puce, qui porte déjà la bascule. L'index ne propose que le cas courant.

---

# 8. Les deux index et les taxonomies

Les tuiles montrent les **termes canoniques** des taxonomies (`SPEC-taxonomies.md`) : `Alfa` et `Alfa Romeo` fusionnés en une tuile, `Japan` et `japan` aussi. L'index par famille repose sur la table famille → tags.

---

# 9. Clavier et accessibilité

- Chaque tuile est un `<button>`, atteignable au `Tab`, activable à `Entrée` et `Espace`.
- Flèches directionnelles pour circuler dans une grille de tuiles, comme dans une grille de cartes.
- Focus visible, en **rouge de trait** (niveau 2 du barème d'accent) — une exception au focus jaune du reste de l'app (§7.2ter de `SPEC.md`), signalée dans `CHANTIERS.md`.
- L'`aria-label` porte le nom **et** le compteur : *« Japon, 34 circuits »*. Le compteur est une information, pas une décoration.
- Les sections d'index sont des `<section>` avec un titre de niveau approprié, pour que la navigation par en-têtes fonctionne.

---

# 10. Repères visuels

Les valeurs (fonds, bordures, gouttières, écarts, liens de section) sont celles du design system.

**Aucun accent rouge dans l'index au repos.** Le survol monte au rouge éteint, le focus au rouge de trait. Le barème de la spec accent s'applique sans exception.

---

# 11. Localisation

Se traduisent : titres et sous-titres de section, liens, `Non renseigné`, `Non classé`, compteurs (`34 circuits` / `34 Strecken`), noms de pays, noms de familles de catégorie livrées.

Ne se traduisent pas : noms de marque, tags bruts.

Point de largeur : le nom d'une tuile dispose de deux lignes avant troncature. `Royaume-Uni` et `Vereinigtes Königreich` doivent tous deux tenir — c'est la contrainte qui fixe la largeur minimale des tuiles de pays à 124 px.

---

# 12. Hors périmètre

**Le contenu des listes** — cartes, tri, densité — est couvert par la spec grille. Cette spec ne décrit que l'état d'index et son passage vers la liste.

**Un index pour les écrans d'inventaire** (add-ons, autres mods) n'est pas prévu : ils ont leurs propres onglets et un volume moindre.

**La mémorisation d'un axe préféré** — ouvrir directement sur les marques plutôt que sur les catégories — n'est pas prévue. Si l'usage montre que quelqu'un scrolle systématiquement jusqu'aux marques, ce sera le signe qu'il faut inverser l'ordre pour tout le monde, pas ajouter un réglage.

---

*Pit Box · spécification index de bibliothèque · à lire avec les specs taxonomies, filtres, grille et accent*
