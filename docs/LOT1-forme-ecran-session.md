# Instruction Claude Code — Lot L1 : forme de l'écran de réglages de session

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> Ce lot est **livré**. Le document est conservé parce que le code y renvoie
> (`L1§…`) et parce qu'il porte les *arguments* derrière les choix — la
> partie qu'on ne retrouve pas deux fois. `SPEC-session.md` décrit ce que
> l'écran **est** aujourd'hui ; celui-ci dit ce qui a été demandé et pourquoi.
> En cas d'écart, `SPEC-session.md` fait foi.


Maquette de référence : `pitbox-session-l1.html` (ouvrir dans un navigateur, les onglets
sont cliquables — c'est le moyen le plus rapide de vérifier qu'aucun contrôle ne se déplace
d'un type de session à l'autre).

Spec concernée : SESSION§3 (écran de réglages), §7.2ter (barème de l'accent rouge),
SESSION§2.1 (couplage qualification / essais libres), SESSION§2.2 (Track day).

---

## 1. Périmètre

**Dans le lot :** les blocs `Session type`, `Session options`, `Simulation` et `Opponents`
de l'écran de réglages de session.

**Hors du lot, à ne pas toucher :**
- la colonne météo (icônes, température/vent/heure, bande jour/nuit, saisons) ;
- la carte `Saved sessions` ;
- la colonne de session de la barre latérale (SESSION§1), y compris `ImageSelectDropdown.svelte` ;
- le fond photo de l'écran (§6.2) ;
- la modale de sélection d'adversaire — elle est refaite en L2, elle reste telle quelle ici ;
- toute la couche backend Rust, sauf le point 4.1 ci-dessous.

**Un seul réglage change de nature dans ce lot** : ABS et antipatinage passent de deux à trois
états (point 3.12). Aucun autre réglage n'est ajouté ni supprimé — les contrôles existants
changent de place, de forme et d'habillage, rien d'autre.

**Point de vigilance séparé (hors L1) :** l'évolution du grip est peut-être non transmise au
preset Quick Drive (SESSION§2 la dit non mappée, SESSION§2.1 suggère l'inverse). Ce lot ne touche
pas à sa transmission. Ne pas « corriger » le câblage au passage : si le comportement est
constaté anormal pendant le travail, le signaler sans le modifier.

---

## 2. Fichiers

Le composant du bloc adversaires est `OpponentsBlock.svelte` (nommé en SESSION§3).
Le composant qui porte `Session type` / `Session options` n'est pas nommé dans la spec :
**l'identifier et le nommer dans le compte rendu final**, ne pas en créer un nouveau.

Le stepper numérique existant est `NumberStepper` : le réutiliser tel quel pour tous les
champs numériques de ce lot (`Race length`, `Opponents`, `Min year`, `Max year`, durées de
qualification et d'essais libres). Son comportement `emptyStart` décrit en SPEC §7.4 reste
inchangé.

Les valeurs CSS de la maquette sont indicatives : **utiliser les variables du design system
existant** (`--rosso`, `--rosso-border`, `--rosso-dim`, `--border`, `--text`,
`--text-secondary`, `--text-disabled`), jamais les hex en dur de la maquette.

---

## 3. Règles de forme

### 3.1 Barème rouge — un seul langage de sélection

Le fond rouge plein (niveau 1) est **retiré de tous les contrôles** de ces trois blocs. Il
reste réservé au bouton « Start the session », seul élément de niveau 1 de l'écran.

Tout contrôle sélectionné — onglet de type de session, onglet de mode d'adversaires, option
de `Grip evolution`, de `Jump start`, de `Start` — porte **exactement** ce traitement :

```
color            : var(--rosso)
background       : var(--rosso-dim)
border-bottom    : 2px solid var(--rosso)
```

Traitement au repos : `color: var(--text-secondary)`, fond transparent.
Traitement au survol d'un élément **non** sélectionné : éclaircissement du gris uniquement
(`background:#1b1b20`, `color:var(--text)`). **Jamais de rouge au survol** — SPEC §7.2ter.

Le focus clavier reste **jaune** (`:focus-visible`, `global.css`), inchangé.

Hiérarchie entre les deux niveaux d'onglets : `Session type` garde `font-size:14px` et
`padding:14px 10px`, les segmentés imbriqués (`Grip`, `Jump start`, `Start`, modes
d'adversaires) passent à `13px` / `9px 8px`. C'est la taille, pas la couleur, qui porte la
hiérarchie.

**Test de conformité à repasser en fin de lot** (SPEC §7.2ter) : effondrer tous les tons
neutres vers le fond, ne garder que `--rosso` et `--rosso-border`. Doivent rester visibles,
et rien d'autre : le filet supérieur de la fenêtre, le carré du logo, le filet de l'entrée
active du rail, la carte du duo en session, le bouton de lancement, et les éléments retenus
en rouge éteint. Joindre le résultat au compte rendu.

### 3.2 Deux zones, et la zone droite ne bouge jamais

`Session options` se divise en deux zones, en grille CSS :

```
grid-template-columns: minmax(0,1fr) 420px;
gap: 26px 30px;
```

- **Zone gauche** : les contrôles qui dépendent du type de session. Grille interne à
  2 colonnes égales.
- **Zone droite** : `Grip evolution` puis `Penalties`, empilés, **dans cet ordre, dans les
  quatre types de session, sans exception**. Ces deux contrôles ne changent jamais de place.

Table des créneaux, autoritative (elle prime sur le JS de la maquette, qui contient une
redondance sur Track day) :

| Type       | Zone gauche, dans cet ordre                              | Zone droite        |
|------------|----------------------------------------------------------|--------------------|
| Practice   | Start                                                     | Grip, Penalties    |
| Hotlap     | Ghost car                                                 | Grip, Penalties    |
| Race       | Race length, Jump start, Qualifying, Free practice        | Grip, Penalties    |
| Track day  | Jump start                                                | Grip, Penalties    |

Un contrôle absent d'un type n'est pas rendu (`{#if}`), il ne laisse pas de créneau vide dans
la zone gauche. La zone gauche peut donc paraître dégarnie en Practice et Hotlap : **c'est
voulu**, la stabilité de la zone droite est le service rendu.

Track day n'a pas de champ `Race length` : la valeur part bien dans le `ModeData` côté CM
mais n'a aucun effet en jeu (SESSION§2.2). Ne pas l'afficher.

### 3.3 Segmentés horizontaux

`Grip evolution`, `Jump start` et `Start` passent de piles verticales à des segmentés
horizontaux, même composant que `Session type`.

`Grip evolution` a quatre segments et le pourcentage passe **sous** le libellé, sur une
deuxième ligne, en mono, `font-size:11px`, `opacity:.7`. Ne pas tenter la mise sur une seule
ligne : elle demande ~560 px et déborde la zone droite.

### 3.4 Case à cocher et durée : un seul contrôle

`Qualifying` et `Free practice` fusionnent leur case et leur champ de durée dans un cadre
unique : `[☑ Run qualifying] │ [10] min`, séparateur interne `1px solid var(--border)`,
hauteur 40 px.

Décoché → le cadre entier passe à `opacity:.55` et le champ de durée à
`color: var(--text-disabled)`. Le champ reste lisible, il n'est pas masqué.

**Décocher `Qualifying` décoche `Free practice`** (SESSION§2.1 : sans qualification le preset
bascule sur le mode course sèche de CM, où aucune phase préparatoire n'existe). L'inverse
n'est pas vrai : décocher les essais libres ne touche pas la qualification. Le bornage
existant (qualification ≥ 5 min, `PracticeLength: 0` et jamais `null`) est inchangé.

### 3.5 Vignettes du plateau — `preview.jpg` d'abord, et en paysage

Format : **96×48 px**, `object-fit: cover`, dans la hauteur de ligne actuelle (~64 px).
La hauteur de ligne ne change pas.

Ordre de repli **dans le plateau d'adversaires uniquement** :
1. `preview.jpg` du skin de la ligne ;
2. `preview.jpg` du skin par défaut du mod ;
3. `livery.png` du skin ;
4. placeholder neutre.

C'est l'inverse de l'ordre actuel, et l'inversion est **locale à ce composant**. Le sélecteur
de livrée de la barre latérale garde son ordre actuel (`livery.png` d'abord) : la question
qu'il pose est « quelle peinture ? », celle du plateau est « quelle voiture ? ».

`preview.jpg` est en 16:9 avec beaucoup de fond autour de la voiture. Recadrer par
`object-fit: cover` + `object-position: center 60%` plutôt que de dézoomer : le cadrage Kunos
est constant et la plupart des mods le reprennent.

**Bulle de survol** : survoler ou focusser une ligne affiche la `preview.jpg` en 300 px de
large, ancrée au-dessus de la vignette. Aucune requête réseau, aucun appel backend, **jamais
l'aperçu 3D** — c'est un JPEG déjà sur disque, le coût doit être nul. Se ferme au défilement
de n'importe quel ancêtre. Sous `prefers-reduced-motion: reduce`, pas de transition
d'apparition.

### 3.6 Difficulté — les valeurs suivent les poignées

`MIN 92%` / `MAX 98%` ne sont plus posés aux extrémités de la piste mais **accrochés aux
poignées**, 38 px au-dessus, centrés sur chacune (`transform: translateX(-50%)`). Format :
la valeur en `var(--text)`, le mot `min`/`max` en `var(--text-secondary)` juste après.

Quand les deux poignées se rapprochent au point que les libellés se chevaucheraient, décaler
celui de gauche vers la gauche et celui de droite vers la droite plutôt que de les laisser se
superposer.

### 3.7 Force des IA

La valeur passe du vert à `var(--text)` — le vert était la seule occurrence de cette couleur
dans l'app. Elle reste éditable ligne par ligne, comportement inchangé, mais **elle doit dire
qu'elle l'est** : au survol de la ligne, la valeur prend `border: 1px solid var(--border)` et
`background:#0f0f13`. Au repos, ni bordure ni fond.

### 3.8 « Regenerate »

Ajouter un bouton `Regenerate` à droite de l'en-tête `Grid · N AI`, style bouton secondaire
neutre (bordure `--border`, texte `--text-secondary`, mono, 10.5px, majuscules). Il appelle
**la fonction de régénération déjà existante**, celle que déclenche aujourd'hui un changement
de voiture pilotée. Aucune nouvelle logique.

L'en-tête passe de `GRID GENERATED · 4 AI` à `Grid · 4 AI` : « generated » devient faux dès
qu'une ligne a été posée à la main.

### 3.9 Années masquées en mode « Same car »

`Min year` / `Max year` ne sont rendus qu'en modes `By category` et `Free`. En `Same car` ils
n'ont pas de sens. Même règle que le champ `Category`, qui n'apparaît que dans son mode.

### 3.10 Race length — bascule tours / minutes

Le libellé actuel `LAPS (OR DURATION IN MINUTES)` est ambigu : la valeur ne dit pas son unité.
Il devient un champ numérique **soudé** à une bascule `Laps | Min` (segmenté à deux segments,
collé au champ, `border-left: 0`).

**À vérifier avant de câbler** : le preset Quick Drive distingue-t-il réellement les deux
unités (champ dédié dans le `ModeData` de `QuickDrive_Race.xaml` / `QuickDrive_Weekend.xaml`) ?
- Si oui : câbler, et persister l'unité (point 4.1).
- Si non : **ne pas livrer la bascule**, garder le champ seul, et le signaler dans le compte
  rendu. Un contrôle sans effet en jeu est pire que le libellé ambigu qu'il remplace.

Ne pas trancher soi-même en inventant une convention.

### 3.11 Bloc Simulation — curseurs en ligne, et rangement corrigé

Les trois curseurs `Damage`, `Fuel consumption`, `Tyre wear` passent de trois lignes pleine
largeur à **trois colonnes sur une seule ligne** (`grid-template-columns: repeat(3,minmax(0,1fr))`,
`gap: 22px 30px`). Ce sont des réglages qu'on pose une fois ; ils occupent aujourd'hui autant
de hauteur que le plateau d'adversaires.

La valeur passe **en tête du curseur**, alignée à droite face au libellé, en mono
`font-size:14px`, `var(--text)` — elle ne flotte plus à droite de la piste.

`Tyre blankets` **quitte `Driving aids` pour rejoindre `Simulation`**, sous le curseur
`Tyre wear`. C'est l'état des pneus au départ, au même titre que l'usure, pas une aide au
pilotage. Répartition finale :

- **Simulation** : Damage, Fuel consumption, Tyre wear, Tyre blankets
- **Driving aids** : ABS, Traction control, Ideal line

`Driving aids` reste dans le même panneau, séparé par un filet `1px solid var(--border)` et
son propre intitulé mono — pas un nouveau panneau.

### 3.12 ABS et antipatinage — trois états

ABS et `Traction control` passent de la case à cocher à un segmenté à trois segments, dans
cet ordre exact : **`Off` · `Factory` · `On`**. `Factory` au milieu, l'ordre lit une
progression. Même composant et même habillage de sélection que les autres segmentés imbriqués
(point 3.1), `font-size:13px`.

`Ideal line` reste une case à cocher : elle n'a que deux états.

**Mapping vers `race.ini` `[ASSISTS]` : à confirmer, pas à supposer.** La convention attendue
est `0 = Off`, `1 = Factory`, `2 = On`, mais elle doit être **vérifiée contre le schéma du
preset Quick Drive et contre le code d'écriture de `race.ini` existant** avant câblage. Si la
vérification contredit cette convention, suivre le schéma réel et le signaler dans le compte
rendu. Ne pas trancher à l'intuition.

### 3.13 Info-bulles

ABS et `Traction control` portent chacun une info-bulle, déclenchée par une pastille `?` de
15 px placée après le libellé.

Contenu (texte exact, à traduire uniquement si l'app est traduite — l'UI est en anglais) :

> **ABS** — *Factory keeps whatever the real car had — no ABS on a 1970 muscle car, ABS on a
> GT3. On only forces it where the car is equipped; it cannot add ABS to a car that never had
> any.*
>
> **Traction control** — *Factory keeps the car's own setup. On forces traction control where
> the car is equipped, and does nothing otherwise.*

Ne **pas** écrire que Content Manager nomme cela ainsi : `Factory` est le vocabulaire
d'Assetto Corsa lui-même, pas celui de CM. La bulle explique le comportement, pas la
provenance du mot.

**Ouverture au survol ET au focus clavier** (`:hover` sur le conteneur, `:focus-visible` sur
la pastille). Une bulle uniquement au survol n'existe pas sur un écran pilotable à la manette.
La pastille est un `<button>` focusable portant `aria-describedby` vers la bulle, elle-même
en `role="tooltip"`. Pas de transition sous `prefers-reduced-motion: reduce`.

### 3.14 Ligne contextuelle « Factory » — sous réserve de faisabilité

Sous les deux segmentés, une ligne qui dit ce que `Factory` vaut **pour la voiture en
session** :

> *Factory — the Ford Mustang Mach 1 428 has neither ABS nor traction control.*

Trois formulations selon le cas : ni l'un ni l'autre ; l'un des deux seulement (le nommer) ;
les deux (`Factory — the <car> has both ABS and traction control.`). Le nom de la voiture est
son nom d'affichage complet. Couleur du mot `Factory` : `#c8b06a`, la même que l'avertissement
de vivier maigre — c'est le même registre, une information que l'app possède et que
l'utilisateur ne devrait pas avoir à deviner.

**Condition de faisabilité :** la présence d'usine de l'ABS et de l'antipatinage se lit dans
`drivetrain.ini`, à l'intérieur du `data.acd`. **Si le déchiffrement de l'acd n'est pas déjà
en place là où les specs de la voiture sont lues**, ne pas l'ajouter pour ce lot : livrer le
bloc sans cette ligne, et le signaler. Elle basculera en L3. Les info-bulles du point 3.13 ne
dépendent d'aucune donnée et sont livrées dans tous les cas.

En cas de donnée indisponible pour une voiture précise (acd illisible, `drivetrain.ini`
incomplet), **ne rien afficher** — pas de ligne au conditionnel, pas de « unknown ». SPEC :
aucune donnée inventée présentée comme un fait.

---

## 4. Emplacements réservés — à tenir libres

Deux lots suivants viendront remplir des emplacements que **ce lot doit créer vides**, pour
que rien ne se déplace ensuite :

- **Colonne à droite du nom de chaque ligne du plateau**, largeur 96 px, alignée à droite,
  vide. Elle recevra le rapport poids/puissance en L3.
- **Espace à droite de `Grid · N AI`**, entre le titre et le bouton `Regenerate`. Il recevra
  la position de départ en L3.
- **Espace à droite de la difficulté**, dans la ligne de contrôles du bloc adversaires. Il
  recevra l'agressivité des IA en L3.

Ne pas les remplir, ne pas les commenter dans l'UI, ne pas les supprimer parce qu'ils
paraissent inutiles.

### 4.1 Persistance

**Règle générale, valable pour tous les champs de cet écran sans exception :** l'état de
chaque réglage est mémorisé **par type de session**. Passer de Race à Practice restitue les
valeurs propres à Practice ; les valeurs ne se propagent jamais d'un type à l'autre.

Ce lot déplace et transforme des contrôles — c'est exactement la situation où un binding se
perd sans que ça se voie : le contrôle se retrouve branché sur un état global, ou réinitialisé
au défaut à chaque montage du composant. **Aucun réglage déplacé, redimensionné ou changé de
type par ce lot ne doit perdre son cloisonnement par type de session.** Concerne au minimum
`Tyre blankets` (change de bloc), ABS et `Traction control` (changent de type), et tous les
contrôles passés en segmentés horizontaux.

Si et seulement si le point 3.10 aboutit, l'unité (`laps` | `minutes`) est un nouveau champ :
- persisté dans le preset par type de session (`src-tauri/src/session_state.rs`,
  `launch_state.json`) ;
- persisté dans les sessions enregistrées (`src-tauri/src/saved_sessions.rs`) ;
- **absent d'une sauvegarde antérieure** (`undefined`, distinct de `0`) → repli sur `laps`,
  le comportement implicite d'avant ce champ. Même schéma de migration silencieuse que le
  champ `Category` et que `trackSkins` (SESSION§3).

**ABS et antipatinage changent de type** : booléen → énumération `off | factory | on`. À
persister dans `launch_state.json` et dans les sessions enregistrées.

**Migration des sauvegardes antérieures : `true → on`, `false → off`.** Pas `→ factory`, même
si `factory` est le défaut le plus juste pour une session neuve. Une migration ne doit jamais
« améliorer » un choix que quelqu'un a posé exprès : l'utilisateur qui avait décoché ABS
attend de le retrouver désactivé, pas remis au réglage d'usine de la voiture. Le défaut
`factory` ne s'applique qu'aux sessions créées après ce lot, et à une valeur absente
(`undefined`, distinct de `false`).

**La migration est une boucle, pas une conversion unique.** Puisque chaque réglage est
cloisonné par type de session (§4.1), il existe une valeur d'ABS et une valeur d'antipatinage
**par type** dans `launch_state.json` — soit quatre paires — plus une paire **par session
enregistrée** dans `saved_sessions.rs`. Toutes doivent être converties. Convertir le seul
preset actif, ou le seul type de session courant, laisserait des booléens orphelins qui
tomberaient silencieusement sur le défaut au prochain chargement.

Aucun autre champ de persistance n'est touché par ce lot.

---

## 5. Contraintes transverses

- **Navigation manette** (`gamepadNav.ts`) : la bulle de survol du plateau est en
  `position: fixed`, donc `offsetParent === null` la ferait écarter du test de visibilité.
  Elle n'est **pas** navigable et n'a pas à l'être — c'est un affichage, pas un contrôle.
  Vérifier en revanche que les segmentés horizontaux restent atteignables à la croix, et que
  la bascule `Laps | Min` ne casse pas le parcours du champ numérique voisin.
- **Mesures en pixels réinjectées dans un style** : diviser par `zoomFactor()` avant écriture
  (SPEC §13). Concerne le positionnement des libellés de difficulté et celui de la bulle.
- **Largeur des libellés mono** : ils sont au-dessus des groupes, pas en colonne — la règle
  de largeur partagée `--sess-lblw` de SESSION§1 ne s'applique pas ici, ne pas l'y importer.
- **`prefers-reduced-motion: reduce`** : toutes les transitions de ce lot sont désactivées.
- **Focus visible** sur chaque contrôle interactif, jaune, non négociable.
- **Responsive** : sous 1000 px de large, la grille à deux zones passe en une colonne et la
  colonne du rapport poids/puissance est masquée.

---

## 6. Recette

À vérifier avant de rendre la main, dans cet ordre :

1. Passer successivement par Practice, Hotlap, Race, Track day : `Grip evolution` et
   `Penalties` occupent **exactement les mêmes coordonnées** dans les quatre. Capture des
   quatre états jointe au compte rendu.
2. Aucun fond rouge plein nulle part dans ces trois blocs. Le test §7.2ter du point 3.1 passe.
3. Race : décocher `Qualifying` décoche `Free practice` ; décocher `Free practice` seul ne
   touche pas `Qualifying`.
4. Plateau avec **11 adversaires** et des mods aux noms longs : aucun débordement, aucune
   ellipse sur la force ou les actions, la hauteur totale reste comparable à l'actuelle.
5. Un mod dépourvu de `preview.jpg` retombe bien sur `livery.png` sans case vide ni image
   cassée.
6. Survol d'une ligne : bulle instantanée, aucun appel réseau ni backend dans les logs.
7. `Same car` : ni `Category` ni les champs d'année ne sont rendus.
8. Le sélecteur de livrée de la barre latérale affiche **toujours** `livery.png` en premier —
   l'inversion du point 3.5 ne doit pas avoir fuité hors du plateau.
9. Navigation complète au clavier et à la manette sur les deux blocs.
10. Les info-bulles ABS et antipatinage s'ouvrent au focus clavier, pas seulement au survol,
    et sont atteignables à la manette.
11. Une session enregistrée **avant** ce lot, avec ABS décoché, se recharge sur `Off` — pas
    sur `Factory`. À vérifier sur **plusieurs types de session** et sur au moins une session
    enregistrée, pas seulement sur le preset actif : la migration doit avoir traité toutes
    les entrées, pas une seule.
12. Changer la voiture en session met à jour la ligne contextuelle « Factory » (si livrée).
    Une voiture dont l'acd est illisible n'affiche **aucune** ligne, pas une ligne au
    conditionnel.
13. Cloisonnement par type de session préservé sur **tous** les contrôles touchés par ce lot
    (§4.1). Test type, à refaire sur `Tyre blankets`, ABS, `Traction control`, `Grip
    evolution`, `Jump start` et `Start` : poser une valeur dans un type, passer à un autre
    type (qui doit afficher sa propre valeur), revenir — la première valeur est intacte.

---

## 7. Compte rendu attendu

- Nom du composant portant `Session type` / `Session options`.
- Résultat du test de conformité §7.2ter (capture).
- Verdict du point 3.10 : la bascule tours/minutes a-t-elle un champ correspondant dans le
  preset ? Si non, ce qui a été livré à la place.
- Verdict du point 3.12 : le mapping `Off / Factory / On` vers `race.ini [ASSISTS]` est-il
  bien `0 / 1 / 2` ? Si non, la convention réelle constatée.
- Verdict du point 3.14 : le `data.acd` est-il déjà déchiffré là où les specs sont lues ? Si
  non, la ligne contextuelle est repoussée en L3 — le dire explicitement.
- Toute divergence assumée par rapport à la maquette, avec sa raison.

Aucune modification de SPEC.md dans ce lot : la spec est mise à jour côté conception une fois
le lot recetté.
