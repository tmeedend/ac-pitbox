# SPEC — Refonte de la navigation et des fiches de détail

> Complément à SPEC.md. Ce document ne réécrit pas la spec principale : il énonce ce qui
> change, ce qui reste, et ce qui devient caduc. Chaque section indique les §§ de SPEC.md
> qu'elle amende.
>
> Statut : décisions arrêtées en session de conception. Les points listés en §14 sont
> explicitement non tranchés et ne doivent pas être implémentés par défaut.

---

## 1. Cadrage

Trois problèmes, constatés sur les écrans existants.

**La navigation classe par mécanique d'installation.** « Add-ons voiture », « Add-ons
circuit » et « Compléments » décrivent la façon dont un mod est posé, pas ce que
l'utilisateur vient y faire. C'est précisément la complexité que l'application existe pour
absorber.

**Les fiches de détail n'ont pas de grammaire commune.** Une fiche voiture a un en-tête
riche, une fiche d'app a un titre en monospace suivi de trois métadonnées à plat, une fiche
de police n'a rien. Le titre y est souvent un nom de fichier.

**Certaines fiches n'ont rien à dire** et affichent du vide, alors que l'information qui
compte pour ces mods (où ils atterrissent, ce qu'ils contestent) existe déjà côté Rust.

Principe directeur retenu : **la classification n'a pas besoin d'être exacte, elle a besoin
d'être sans conséquence quand elle se trompe.** Toute déduction est corrigeable, et aucun
mod ne devient introuvable si une déduction échoue.

---

## 2. Modèle : deux axes indépendants

Ce sont des attributs **de chaque mod**, pas de son type. Un modèle de pilote est autonome
dans le cas général et greffé quand il est livré avec une voiture.

### 2.1 Rattachement

Sur quoi le mod se greffe. Valeurs : `voiture(id)`, `circuit(id)`, `app(id)`, `le jeu`,
`autonome`. Un mod peut être rattaché à plusieurs entités (cf. §14.2).

Déduit, par ordre de force décroissante :

1. Le chemin posé contient l'identifiant d'une entité connue
   (`content/cars/ks_toyota_ae86/…`). Quasi certain.
2. Le mod est une couche : l'hôte est connu par construction. Certain.
3. Config CSP nommée d'après une entité (`la_canyons__hide_pit_crew.ini`,
   `policeman__ext_config.ini`). Fort.
4. Arrivé dans la même archive qu'un contenu (les polices du pack RSS). **Conjecture.**

Aucun signal ⇒ `le jeu` ou `autonome`.

### 2.2 Nature

Ce que le mod fait. Valeurs : `contenu`, `apparence`, `comportement`, `dépendance`,
`non reconnu`.

Cet axe ne structure aucun écran. C'est une facette, et c'est lui qui rend praticable le
volume rattaché à `le jeu`.

### 2.3 Correction

Le rattachement déduit est **corrigeable par l'utilisateur** (« Rattacher à… » dans le ⋮).
La correction est stockée dans l'overlay, survit à la mise à jour du mod et au réindex,
selon le mécanisme déjà en place pour `display_name_user` (§5bis.3) et l'appariement
Wikipédia (§7.6 de SPEC-wikipedia).

---

## 3. Navigation — le rail

**Amende** : SPEC.md §7 (organisation des écrans), §8 (écran Compléments).

Deux rangs, séparés par un intitulé de groupe :

| Rang | Entrées | Critère |
|---|---|---|
| **LA SESSION** | Voitures · Circuits · Pilote | Se décide à chaque session |
| **LE JEU** | Apps · Compléments | Reste vrai jusqu'à nouvel ordre |
| *(hors rang)* | Atelier | |

### 3.1 Écrans supprimés

- **Add-ons voiture** et **Add-ons circuit** : fusionnés dans l'écran des compléments.
- **Compléments** dans son acception actuelle de fourre-tout : remplacé par cet inventaire,
  qui reprend le nom pour son sens littéral (§4).
- **Couches et extensions** en écran dédié : n'existe pas. « Est une couche » est une
  valeur de facette, pas un lieu. *(Cette décision annule l'arbitrage intermédiaire pris
  en faveur d'un écran dédié.)*

### 3.2 Écran Apps

**Apps est un écran à part entière**, au même titre que Voitures, Circuits et Pilote. Une
app a un nom, une identité, on l'installe volontairement et on l'active ; elle n'est la
dépendance de rien.

Les composants de liste sont ceux de l'écran des compléments (barre d'outils, ligne à deux
niveaux, ⋮), mais l'écran est distinct et **les apps ne figurent pas dans l'inventaire**,
exactement comme les voitures et les circuits n'y figurent pas.

*(Une solution intermédiaire — Apps comme vue filtrée épinglée de l'inventaire — a été
écartée : elle ne résolvait qu'un problème de nommage créé par le maintien des apps dans
l'inventaire.)*

---

## 4. Écran des compléments

Inventaire **exhaustif de tout ce qui n'est pas un contenu autonome** : tout ce qui se
greffe, s'ajoute ou se superpose au jeu y figure, y compris ce que l'application n'a pas su
reconnaître. Cet écran n'organise pas, il inventorie.

**N'y figurent pas** les quatre contenus autonomes qui ont leur écran : voitures, circuits,
modèles de pilote, apps.

**Nom de l'écran.** « Installé » promettrait plus qu'il ne tient, puisque quatre types de
contenus installés en sont exclus. **« Compléments »** y retrouve son sens littéral — ce que
le mot n'a jamais eu dans l'application actuelle, où il désignait un fourre-tout. À
confirmer au moment de l'implémentation.

### 4.1 Facettes

| Facette | Valeurs |
|---|---|
| Greffé sur | Une voiture · Un circuit · Une app · Le jeu · Autonome |
| Nature | Apparence · Comportement · Dépendance · Non reconnu |
| État | Actif · Inactif · A une note |

Chaque valeur porte son compteur. Comportement tri-état conforme au système de filtres
existant.

### 4.2 Ligne d'inventaire

```
[vignette 30px] Nom lisible ✎          → Rattachement    NATURE    ● État    ⋮
                identifiant technique
```

- **Deux lignes** : nom d'affichage en haut, identifiant technique en mono en dessous.
  C'est ce qui permet de renommer sans rien perdre.
- Le rattachement est un lien vers la fiche de l'entité, en bleu-lien, couleur réservée à
  cet usage (motif déjà présent sur le nom de voiture dans la liste des skins).
- **Aucun bouton exposé.** Ouvrir le dossier, Priorité, Désactiver, Supprimer passent dans
  le ⋮. La priorité reste visible sous forme d'**état** (une étoile) quand elle est posée,
  jamais sous forme de bouton.
- Un indicateur discret signale la présence d'une note.

### 4.3 Remontée sur la fiche de l'hôte

Un mod rattaché **apparaît en plus** sur la fiche de l'entité qu'il modifie, avec son
interrupteur. « Masquer le personnel de stand » apparaît sur la fiche de LA Canyons.

C'est ce qui rend le non-dogmatisme sûr : une déduction ratée coûte un raccourci manquant,
jamais un mod introuvable.

### 4.4 Groupement

Groupement **par archive d'origine** disponible, avec en-tête portant le nom de l'archive,
le nombre d'éléments, le poids, la date d'import et « Tout supprimer ». Actif par défaut
uniquement là où il répare quelque chose (contenus arrivés en pack). Réglage mémorisé par
vue.

### 4.5 Rattachement multiple

Un mod rattaché à plusieurs entités apparaît une seule fois dans l'inventaire. La mention
des autres rattachements est **explicite et cliquable** (« aussi dans … »), jamais réduite
à un jeu de badges qu'il faut savoir décoder.

*(Corrige le cas `DORIKIN_DRIVER_MOD`, aujourd'hui listé deux fois sans que rien ne le
dise.)*

---

## 5. Écran Pilote

**Règle générale : l'inventaire d'un type vit avec son sélecteur quand il en existe un.**

C'est déjà vrai pour les livrées, dont le sélecteur est sur la fiche voiture. Les modèles
de pilote étaient la seule exception, d'où le doublon entre l'écran Pilote et
Compléments > Pilotes.

L'écran Pilote réunit donc :

- **À gauche** : le mannequin en scène, et la tenue (modèle, casque, combinaison, gants).
- **À droite** : l'inventaire des modèles installés, en grille, avec recherche.

### 5.1 Grille ou liste

Les zones dont le contenu est **visuel** (modèles de pilote, showrooms, filtres PP,
polices) se présentent en grille ; les zones de **fichiers** (extensions, non reconnus) en
liste. Un mannequin ne se reconnaît qu'à sa géométrie, vingt lignes de texte n'en montrent
rien.

Le mat de la grille est celui de la grille de voitures (§7.4), pas un nouveau traitement.
La même image alimente la tuile d'en-tête de la fiche.

---

## 6. La coquille de fiche

**Amende** : SPEC.md §6, §7.3, §8. Une seule anatomie, deux contenants.

### 6.1 En-tête, identique pour tous les types

```
←  [tuile]  Nom d'affichage ✎  [Catégorie]              ● État   ♡   ⋮
            Sous-titre lisible : marque · année · par auteur
```

- **La tuile montre la chose réelle** quand elle existe : badge de marque pour une voiture,
  tracé pour un circuit, vignette du mannequin pour un modèle de pilote, aperçu rendu depuis
  l'atlas pour une police. Pictogramme de type sinon. **Jamais deux lettres tirées du nom.**
- Le **nom d'affichage** est éditable sur tous les types, pas seulement les voitures
  (extension de §5bis.3).
- **L'auteur est dans le sous-titre.** C'est une propriété du mod, elle ne descend pas en
  bas de colonne.
- L'identifiant technique et l'archive d'origine ne sont **jamais** le titre : ils vivent
  dans la carte Origine de l'onglet Installation.
- **Un seul vocabulaire d'état** (cf. §12).

### 6.2 Contenants

| Contenant | Quand |
|---|---|
| Page pleine | Contenus autonomes, couches, et tout mod dont un bloc dépasse une dizaine de lignes |
| Panneau latéral | Mods greffés simples (police, config, ressource) |

Le panneau porte un lien **« Ouvrir la fiche complète »**, affiché uniquement quand le
contenu le justifie : présence de couches, ou plus d'une dizaine d'ajouts au jeu. Une app y
bascule presque toujours, une police jamais.

Ce panneau ne double aucune page pleine : il **est** la fiche. La suppression du panneau
latéral décidée en §6 de SPEC.md visait un panneau redondant et ne s'applique pas ici.

### 6.3 Corps

Grille à deux colonnes. **Les cartes absentes ne laissent pas de trou** : sans fiche
technique ni courbe, la colonne droite se réduit et la gauche prend la place. La grille
n'est plus dictée par le type le plus riche.

---

## 7. Fiche voiture et fiche circuit

### 7.1 Trois onglets

| Onglet | Contenu |
|---|---|
| **La voiture** / **Le circuit** | Aperçu, sélecteur, fiche technique, courbe, son, bloc textuel |
| **Médias et documents** | Screenshots, Replays, Backgrounds, Ressources |
| **Installation** | Origine, couches, décisions d'import, ajouts au jeu, étiquettes, extensions CSP |

Suppression des onglets de premier niveau **Ressources** et **Ajouts au jeu** : le plus
souvent vides, et il fallait cliquer pour le découvrir. Ajouts au jeu ne redescend pas en
colonne (ce que §4.5.5 interdit à raison), il rejoint Installation, qui a la place pour ses
34 dossiers.

Fusion de Screenshots, Replays et Backgrounds : trois onglets pour trois galeries souvent
vides.

**Ressources et Ajouts au jeu sont deux notions opposées**, à ne jamais réunir dans un même
bloc. Les ajouts au jeu sont posés *dans* Assetto Corsa, hors du dossier du mod. Les
ressources sont exactement ce qui n'y entre pas : elles sont mises de côté. Elles rejoignent
donc Médias et documents, pas Installation.

### 7.2 Ordre de la colonne principale

1. **Aperçu**
2. **Sélecteur** (livrée / tracé) — collé sous l'aperçu
3. Contenu propre à l'objet
4. **Bloc textuel** — en dernier

Le sélecteur est un **contrôle**, pas de la documentation : il agit sur l'image du dessus
et sur ce qui partira en session. Il ne doit jamais exiger de faire défiler.

### 7.3 Sélecteur compact

```
LIVRÉE  [vignette]  Deep Navy  1 / 7      ‹  ›      [ Voir les 7 ]
```

- **Flèches** : défiler d'une livrée à l'autre en gardant l'œil sur l'aperçu. Geste le plus
  fréquent.
- **Nom** : ouvre la liste déroulante, pour chercher par nom.
- **Bouton** : déplie la grille de vignettes en place, pour chercher par apparence. Une
  liste déroulante seule ne suffit pas : beaucoup de livrées de mods s'appellent `skin_01`.
- **La grille se replie à chaque ouverture de fiche.** Pas de persistance.

Identique pour le tracé d'un circuit, qui change en outre la miniature, la longueur et
l'odomètre.

### 7.4 Bloc textuel à sous-onglets

**Amende** : SPEC-wikipedia §7.1, qui prévoyait deux onglets.

```
Description   |   Le modèle réel   |   Notes ●
```

- Hauteur constante, c'est le contenu qui change. La longueur de l'article Wikipédia cesse
  d'être un problème de mise en page.
- L'onglet Wikipédia est **absent** si aucun article n'est apparié (inchangé).
- L'onglet Notes porte un **marqueur** quand une note existe.
- **Notes n'est jamais l'onglet par défaut.**
- La règle des 150 caractères (§7.2 de SPEC-wikipedia) reste ; noter qu'elle s'appliquera à
  la majorité des circuits du jeu de base, dont la description tient en trois mots.

### 7.5 Étiquettes et extensions CSP

- **La catégorie remonte près du titre.** C'est le seul tag qui fait un travail, la
  composition de plateau.
- **Les extensions CSP quittent les tags** et descendent dans Installation, sur une ligne
  grise, tronquée (« displays, lightingfx, rainfx +3 »). Elles décrivent l'installation,
  pas le contenu.
  **Attention :** la détection des extensions CSP n'existe aujourd'hui que pour les
  circuits. L'étendre aux voitures est souhaitable mais constitue un lot distinct (§15),
  et non un pré-requis de cette refonte.
- Les tags manuels et le champ de saisie vivent dans Installation. Le champ d'édition
  n'occupe pas de carte permanente.

**Pourquoi les tags ne sont pas dans le premier onglet.** Il faut distinguer la matière
première du résultat. Les tags servent à dériver la catégorie et à alimenter la fiche
technique ; ce que l'utilisateur consulte en premier onglet, c'est ce résultat. Il ouvre les
tags bruts quand la dérivation s'est trompée — mauvaise catégorie, fiche vide — c'est-à-dire
au même moment que l'origine, la version et les décisions d'import.

Pour qui les cherche depuis le premier onglet, **la puce de catégorie près du titre est un
lien** vers le bloc Étiquettes.

### 7.6 Unités

Les valeurs converties affichent **la valeur d'origine en gris à côté**, uniquement
lorsqu'elles diffèrent réellement (`327 ch  327 л.с.` / `1 196 kg  1195.7`). Ne pas masquer
ce que le mod déclare.

### 7.7 Tracés issus d'une couche

La carte Tracés montre **l'état composé**, celui que l'utilisateur verra au lancement. Les
tracés venus d'une couche portent une marque d'origine, et la fiche indique « 2 dont 1
ajouté ».

---

### 7.8 Onglet Médias et documents

Réunit quatre blocs existants, **sans modifier aucun d'eux** (cf. §17), en deux groupes.

**Ce que tu as produit**

| Bloc | Forme | Portée |
|---|---|---|
| Screenshots | Grille de vignettes, badge du circuit, horodatage | Voitures et circuits |
| Replays | Liste : nom de fichier, date, durée, circuit, « Lire dans CM », suppression | Voitures et circuits |

**Ce qui est livré avec le mod**

| Bloc | Forme | Portée |
|---|---|---|
| Ressources | Liste de fichiers **et visionneuse intégrée** (§17.1) | Tous types |
| Backgrounds | Bande d'images + phrase expliquant le rôle de repli | Circuits uniquement |

Le nom de l'onglet dit « et documents » parce qu'une notice PDF, un template de livrée ou un
fond d'écran ne sont pas des médias au sens des captures. C'est aussi le contenu réel de ce
bloc dans la pratique.

Les actions **Ouvrir le dossier** et **Lier un fichier…** restent au pied de chaque bloc,
comme aujourd'hui : elles portent sur un dossier différent pour chacun.

Un bloc vide n'est pas masqué : ses actions sont sa seule voie d'accès pour en ajouter. Il
affiche sa phrase d'état vide.

Le compteur de l'onglet est la **somme** des quatre.

## 8. Couches et extensions

### 8.1 Trois emplacements, trois rôles

| Où | Quoi |
|---|---|
| Fiche de l'hôte, onglet Installation | Liste des couches posées, activation, **réglage de l'ordre** |
| Écran Installé | Les couches parmi le reste, via facette. Ordre **affiché**, non réglé |
| Fiche de la couche | Le détail complet |

Le réglage du rang se fait là où l'ordre a un sens, c'est-à-dire sur l'hôte. Un inventaire
trié par nature ou par état n'a pas de place pour des flèches monter/descendre.

### 8.2 Fin du déversement de fichiers

Les écrans Add-ons déversaient la liste complète des fichiers d'une couche (392 lignes en
monospace, poids par fichier). **Supprimé.** Ce contenu vit dans la fiche de la couche.

### 8.3 Fiche de couche

- Statistiques en tête : ajoutés, **remplacés (en rouge)**, poids.
- **« Ce qui écrase la base »** en premier, en clair, avec la mention « base sauvegardée ».
  C'est la promesse de §4.4 rendue visible au seul endroit où elle inquiète. Dit ici, le
  bandeau bleu explicatif n'a plus à être répété sur chaque écran.
- **« Ce qui s'ajoute »** replié par dossier, avec compteur et poids **au niveau du
  dossier**. Le poids par fichier ne décide rien.
- Carte **Ordre** affichée seulement à partir de deux couches sur le même hôte (principe
  des lignes vides omises, §6).
- Notes et nom d'affichage éditables : une couche s'appelle `spa2022-release_V1-03.rar`,
  c'est exactement l'objet qui en a besoin.

Le nom d'affichage est **dérivé** (retrait de l'extension, des séparateurs, du préfixe de
l'hôte quand il le répète), donc faillible, donc corrigeable.

### 8.4 Navigation

Cliquer une couche depuis la fiche de l'hôte ouvre **sa fiche complète**. L'historique de
navigation (§7.2bis) permet le retour.

---

## 9. Notes

### 9.1 Portée

**Tous les types de mod**, sans exception. Exclure un type crée une règle à apprendre pour
une économie nulle. Ce qui varie n'est pas le droit d'en avoir, c'est la place occupée :

| Type | Présentation |
|---|---|
| Voiture, circuit, app, modèle de pilote | Sous-onglet permanent du bloc textuel |
| Tous les autres (police, config, couche, ressource…) | Bouton « Ajouter une note », déplie le champ |

### 9.2 Sémantique

**Distincte de la description.** La description est celle du mod, éditable en surcharge, et
la vider signifie « reviens au fichier » (§5bis.3). Une note n'a pas de valeur d'origine :
vide veut dire vide. Les fusionner rendrait le geste « effacer » ambigu.

### 9.3 Comportement

- Texte brut multi-lignes. **Pas de markdown** : un rendu à moitié interprété est pire que
  rien.
- Sauvegarde à la perte du focus.
- Pendant la saisie, sous le champ : « Enregistré dans Pit Box, pas dans les fichiers du
  mod » — même convention que le renommage.

### 9.4 Données

Colonne `notes_user` dans l'overlay, même logique que `display_name_user` : survit à la
mise à jour du mod et au réindex. À porter sur toutes les tables d'entités (mods, sub_mods,
apps, autres, couches).

### 9.5 Recherche

Trois mécanismes, à livrer **ensemble**, sans quoi la note est en écriture seule :

1. Les notes entrent dans la recherche plein texte du champ global.
2. Jeton `note:` pour cibler, et facette booléenne « A une note ».
3. **Indicateur visuel sur la ligne ou la carte** quand une note existe.

### 9.6 Export

Les notes entrent dans l'export/import. C'est la donnée la plus personnelle de la base.

---

## 10. Listes — grammaire commune

Une ligne de liste est une projection compressée de l'en-tête de fiche. Même barre d'outils
partout :

```
[ Recherche : nom, archive, chemin, note… ]   GROUPER [ … ]   TRIER [ A→Z | Taille | Import ]   n / N
```

- Les onglets de zone vides restent **visibles et grisés** : une barre d'onglets dont la
  taille change se relit à chaque visite.
- Le paragraphe explicatif de tête (junctions, liens de fichiers) passe derrière un lien.
  Il est juste, mais on ne le lit qu'une fois.

---

## 11. Fiches de mods greffés simples

Le vide constaté (une police affiche un titre, trois métadonnées et « aucun fichier
annexe ») se comble sans rien inventer, avec des données que le parcours de fichiers
produit déjà pour la détection de conflits :

- **Emplacements** : arbre des chemins posés, avec compteurs.
- **Conflits** : quels autres mods visent les mêmes fichiers, et qui gagne.
- **Notes**.
- **Origine** : archive, chemin dans l'archive, date d'import.

---

## 12. Vocabulaire des états

Quatre formulations coexistent aujourd'hui (`Active` + pastille verte, `Stock`, `ACTIVE` en
mono vert, badge rouge `INSTALLED`). Elles recouvrent **deux notions distinctes** qu'il faut
séparer et nommer une fois pour toutes :

| Notion | Valeurs | Où |
|---|---|---|
| Déploiement | ● Actif / ● Inactif / ● En attente | En-tête de fiche, ligne de liste |
| Provenance | Contenu de base / Importé le … | Carte Origine |

**« En attente » est un état existant, à conserver.** Il apparaît aujourd'hui en ambre dans
l'arbre des ajouts au jeu (« 1 waiting », « 2 waiting ») lorsqu'un emplacement est disputé
par un autre mod. Il ne doit pas être fondu dans « Inactif » : un mod inactif a été
désactivé, un mod en attente est actif mais perd l'arbitrage. La couleur ambre lui est
réservée.

---

## 13. Impacts techniques à prévoir

- Colonnes overlay : `notes_user` (toutes entités), `display_name_user` (étendu à toutes
  les entités), `rattachement_user`.
- Calcul et stockage du rattachement déduit, avec traçabilité du **signal** utilisé (pour
  pouvoir dégrader la confiance affichée si besoin).
- Calcul de la nature.
- Vues épinglées = filtres enregistrés, à brancher sur le système de jetons existant.
- Refonte des routes de navigation : la disparition de trois écrans touche l'historique
  (§7.2bis), les liens depuis les fiches et la mémorisation de vue par écran.

---

## 14. Points ouverts — ne pas implémenter par défaut

**14.1 Filtres PP, showrooms, météo, interface.** Rangés dans l'inventaire avec un
rattachement `le jeu`. À réexaminer une fois l'inventaire réel sous les yeux : un filtre PP
se *choisit* plus qu'il ne s'installe.

**14.2 Rattachement massif.** Un pack de configs visant trente circuits doit s'afficher
« 30 circuits » sans les énumérer. Reste à décider ce qu'ouvre ce lien.

**14.3 Mod greffé sur un contenu absent.** Le skin d'une voiture non installée doit avoir un
état d'attente. Sans cela il tombe dans `le jeu` et devient indéchiffrable.

**14.4 Volume de `le jeu`.** Sur une bibliothèque réelle, mesurer la part de l'inventaire
qui n'a aucun rattachement. Si elle domine, une troisième facette sera nécessaire,
probablement le dossier AC touché.

**14.5 Gestes du sélecteur compact.** Les flèches sont supposées être le geste le plus
fréquent. Si l'usage montre qu'on ne fait que déplier la grille, elles sont du bruit.

**14.6 Déséquilibre inverse des colonnes.** Sur un mod sans description ni article
Wikipédia, la colonne gauche se réduit à l'aperçu et au sélecteur. À vérifier sur un mod
pauvre avant de figer.

**14.7 Lien « 1 en attente ».** L'état ambre des ajouts au jeu indique qu'un emplacement est
disputé, mais pas par qui. Reste à décider si le nom du mod qui gagne l'arbitrage s'affiche,
et s'il est cliquable.

---

## 15. Lots de livraison suggérés

| Lot | Contenu | Dépend de |
|---|---|---|
| 1 | Notes : colonne overlay, UI sur toutes les fiches, recherche, indicateur, export | — |
| 2 | Coquille de fiche unique : en-tête, tuile, vocabulaire d'état, nom éditable partout | — |
| 3 | Fiche voiture et circuit : trois onglets, sélecteur compact, bloc textuel à sous-onglets, CSP démotées | 2 |
| 4 | Fiche de couche + fin du déversement de fichiers | 2 |
| 5 | Rattachement et nature : calcul, stockage, correction | — |
| 6 | Écran des compléments : fusion des trois écrans, facettes | 5 |
| 7 | Écran Pilote fusionné, écran Apps dédié, rail à deux rangs | 6 |
| 8 | Fiches de mods greffés simples : emplacements, conflits | 5 |
| 9 | Onglet Médias et documents : fusion des quatre blocs existants | 3 |
| 10 | Détection des extensions CSP pour les voitures (n'existe aujourd'hui que pour les circuits) | — |

Les lots 1 à 4 n'exigent pas la refonte de la navigation et peuvent être livrés seuls.

---

## 16. Maquette de référence

Un seul fichier HTML autonome, à joindre à cette spec : **`pitbox-maquettes.html`**. Il
illustre, il ne fait pas foi : en cas d'écart, le texte ci-dessus prime.

| Écran | §§ |
|---|---|
| 1 · Le rail, avant et après | 3 |
| 2 · Écran des compléments, inventaire à facettes | 4 |
| 3 · Apps, écran dédié | 3.2 |
| 4 · Écran Pilote | 5 |
| 5 · Fiche voiture — onglet « La voiture » | 7.1 à 7.4, 7.6 |
| 6 · Fiche voiture — onglet « Installation » | 7.1, 7.5, 8.1 |
| 7 · Fiche circuit | 7.3, 7.4, 7.7 |
| 8 · Fiche d'une couche | 8.3 |
| 9 · Mod greffé simple, en panneau | 6.2, 9.1, 11 |
| 10 · Fiche voiture — onglet « Médias et documents » | 7.8, 17.1, 17.3 à 17.5 |

Les sélecteurs de livrée et de tracé sont interactifs : le bouton « Voir les 7 » déplie la
grille.

Les chiffres, noms et proportions sont plausibles mais inventés. En particulier, la
répartition des facettes de l'inventaire (« Le jeu : 26 sur 61 ») est une hypothèse de
travail destinée à éprouver la lisibilité de l'écran, pas une mesure.

## 17. Ce qui ne doit pas être modifié

**Cette spec ne redessine que ce qu'elle nomme.** Tout bloc existant non cité ici et non
traité plus haut reste tel quel. Les blocs ci-dessous sont **déplacés** par la refonte
(d'un onglet à un autre, d'un écran à un autre) mais leur contenu, leur forme et leurs
actions ne changent pas.

### 17.1 Ressources

Liste des fichiers avec leur poids, **et visionneuse intégrée** : ouverture d'un fichier en
place, ajustement largeur / page, zoom réglable, pagination, fermeture, ouverture externe.

À conserver intégralement. Ne pas réduire à une liste de liens.

*Déplacé : onglet de premier niveau → onglet Installation (§7.1).*

### 17.2 Ajouts au jeu

Arbre **par dossier de destination**, chaque nœud dépliable, avec nombre de fichiers et
poids par dossier, et affichage des fichiers avec leur poids une fois déplié. État
**« en attente » en ambre** au niveau du dossier quand un emplacement est disputé.

Volume réel constaté : 34 dossiers pour une seule voiture. C'est ce qui justifie que ce bloc
ne redescende pas en colonne de fiche (§7.1) et que le seuil de bascule panneau → page
pleine existe (§6.2).

À conserver intégralement, y compris la phrase d'explication du bloc.

*Déplacé : onglet de premier niveau → onglet Installation (§7.1).*

### 17.3 Screenshots

Grille de vignettes, badge du circuit en surimpression, horodatage, actions
**Ouvrir le dossier** et **Lier un fichier…**.

*Déplacé : onglet de premier niveau → bloc de l'onglet Médias (§7.8).*

### 17.4 Replays

Ligne par replay : nom de fichier, date, durée, circuit, action **Lire dans CM**,
suppression. Actions **Ouvrir le dossier** et **Lier un fichier…**.

Ce n'est pas une galerie. Ne pas convertir en grille de vignettes.

*Déplacé : onglet de premier niveau → bloc de l'onglet Médias (§7.8).*

### 17.5 Backgrounds

Bande d'images, **circuits uniquement**, avec la phrase expliquant leur rôle de repli sur
l'écran de réglage de session.

*Déplacé : onglet de premier niveau → bloc de l'onglet Médias (§7.8).*

### 17.6 Construction de la tenue du pilote

Tout le mécanisme actuel de l'écran Pilote reste inchangé : sélection du modèle, du casque,
de la combinaison et des gants, prévisualisation du mannequin, et l'ensemble des états et
options qui l'accompagnent.

**Ce que la refonte ajoute, et rien d'autre :** l'inventaire des modèles installés, en
grille, dans la partie droite de l'écran (§5). **Ce qu'elle retire :** le doublon de cet
inventaire dans l'ancien écran Compléments.

La maquette simplifie volontairement la partie tenue pour ne montrer que la cohabitation des
deux zones. Elle ne décrit pas les contrôles existants et ne doit pas servir de référence
pour eux.

### 17.7 Autres blocs conservés sans changement

- **Historique** des versions et des imports.
- **Décisions d'import**, y compris le code couleur ambre.
- Toute la mécanique d'import, de composition, de junction et de lien de fichier.
- Le système de filtres à jetons, que cette spec étend sans le refondre.
- Tout le reste de SPEC.md non explicitement amendé ici.

### 17.8 Règle générale

En cas de doute entre « la maquette montre autre chose » et « l'écran actuel fait ceci »,
**c'est l'écran actuel qui gagne**, sauf si le présent document dit explicitement le
contraire. Les maquettes ont été dessinées à partir de captures partielles et ne montrent
pas tous les états existants.
