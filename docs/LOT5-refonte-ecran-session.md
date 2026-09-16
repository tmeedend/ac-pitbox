# Instruction Claude Code — Lot 5 : refonte de l'écran de préparation de session

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> Ce lot est **livré**. Le document est conservé parce que 77 renvois du code y
> pointent (`L5§2.3`, `L5§4.1`…) et surtout parce qu'il porte les *arguments* —
> « un réglage se range selon sa portée, jamais selon sa fréquence d'usage »,
> « le panneau gauche ne doit jamais défiler » et ses trois recours dans
> l'ordre. `SPEC-session.md` décrit ce que l'écran **est** aujourd'hui ; celui-ci
> dit ce qui a été demandé et pourquoi. En cas d'écart, `SPEC-session.md` fait foi.
>
> Il avait vécu hors du dépôt, ce qui laissait ses 77 renvois pointer dans le
> vide sans que rien ne le signale — exactement ce que `scripts/check-refs.mjs`
> empêche désormais.
>
> Il cite deux documents frères, **lot 1** (« la règle 1.2 du lot 1 ») et
> **lot 2** (SETUP§2.6), plus un **lot 4** hors périmètre. Ceux-là
> manquent encore, et une quarantaine de renvois du code les désignent.

**Nature du lot** : réorganisation structurelle. Peu de nouveaux réglages, beaucoup de déplacements. Aucun réglage existant ne disparaît, aucune valeur envoyée au jeu ne change.

**Maquette de référence** (structure uniquement, proportions indicatives, valeurs d'exemple) : https://claude.ai/artifact/6zvBBsmZYDXoddQasfsHJq

**Principe directeur, à appliquer en cas de doute sur un placement** : *un réglage se range selon sa portée, jamais selon sa fréquence d'usage.* Quatre zones, quatre objets :

| Zone | Objet | Contenu |
|---|---|---|
| Panneau gauche | la voiture et la piste | livery, driver, ballast, restrictor, ABS, traction control |
| Colonne centrale | la session et ses règles | durées, dégâts, carburant, usure, pénalités, ideal line, départ |
| Rail droit | les conditions | météo, températures, vent, état de piste, heure, saison |
| Page dédiée | les adversaires | filtres, générateur, grille |

Le critère de fréquence est à écarter explicitement : il envoie tout ce qui est rare au même endroit, et cet endroit devient un fourre-tout.

---

## 1. Navigation : le type de session devient la navigation

Le bouton `SESSION SETUP` du panneau gauche disparaît. Le segmented control `SESSION TYPE` de la colonne centrale disparaît également. Les deux sont remplacés par une liste de navigation en tête du panneau gauche, sous l'en-tête existant `THE SESSION` :

```
THE SESSION
  Practice
  Hotlap
  Race                              ← sélectionné
    Opponents   6 AI · 87% ± 3      ← sous-entrée, indentée
  Track day
```

### 1.1 Règles de la liste

- Les quatre types sont **toujours visibles**, jamais repliés derrière un sélecteur. Ils annoncent ce que l'application sait faire.
- Cliquer un type le sélectionne **et** affiche sa page de réglages.
- La sous-entrée `Opponents` n'apparaît **que sous Race et Track day**, et uniquement quand ce type est sélectionné. En Practice et Hotlap, la liste fait quatre lignes.
- L'entrée parente (`Race`) sert de retour depuis `Opponents` : cliquer dessus ramène aux réglages de session sans changer de type. L'indentation doit rendre ce geste lisible comme une remontée — soigner ce point, c'est la faiblesse connue de cette structure.
- Une seule entrée surlignée à la fois : soit le type, soit sa sous-entrée.

### 1.2 Ligne de résumé sur `Opponents` — obligatoire

Format : `6 AI · 87% ± 3` (nombre d'IA, puis difficulté au format centre ± écart).

Ce n'est pas un ornement : sans elle, on ne peut plus savoir combien d'adversaires on affronte sans changer de page, alors qu'on peut lancer la session sans y être allé.

### 1.3 Remontée des alertes

Toute alerte vivant sur la page adversaires (aujourd'hui : pool insuffisant, doublons de pilote) doit être **signalée sur l'entrée de navigation** : la ligne de résumé passe en rouge et porte un marqueur. Une alerte sur une page qu'on ne regarde pas ne vaut pas mieux que pas d'alerte.

### 1.4 Changer de type ne détruit jamais rien

- Passer de Race à Practice, puis revenir à Race, restitue **l'intégralité** des réglages Race, grille comprise.
- Si l'utilisateur est sur la page `Opponents` et change de type pour Practice ou Hotlap, il arrive sur la page de réglages de ce type. La grille précédente est conservée en mémoire.
- Le type détermine ce qui est **affiché** et ce qui est **envoyé au jeu**, jamais ce qui est **mémorisé**.

### 1.5 Une session doit être lançable sans avoir ouvert la page adversaires

Les valeurs par défaut de la grille doivent être jouables telles quelles. `FILL n AT RANDOM` couvre déjà ce besoin ; vérifier qu'il s'applique bien à l'ouverture d'un type Race ou Track day vierge.

### 1.6 Barre de titre

`Save session…` et `Load session…` restent où ils sont. Le titre de page suit la navigation : `Race`, puis `Race · Opponents`.

---

## 2. Panneau gauche : la voiture et la piste

### 2.1 Repli `Performance`

Ballast, restrictor, ABS et traction control sont regroupés sous **une ligne unique** de la carte voiture, au même gabarit que `LIVERY` et `DRIVER`, avec chevron :

```
PERFORMANCE    Stock                            ›     ← rien de posé
PERFORMANCE    Ballast 50 kg · Restr. 10%       ›     ← en rouge (--accent)
```

- **La ligne affiche toujours son état**, replié ou non. Rien n'est jamais masqué : « absent » et « à zéro » ne doivent pas se ressembler.
- Résumé quand rien n'est posé : `Stock`, en `--text-secondary`.
- Résumé quand au moins un réglage est actif : liste des réglages non neutres, en `--accent`. ABS et TC ne comptent comme actifs que s'ils diffèrent de `Factory`.
- Déplié : ballast, restrictor, ABS, traction control, aux gabarits et bornes déjà spécifiés (ballast 0–200 kg, restrictor 0–100 %, champs numériques, pas de sliders).

### 2.2 ABS et Traction control quittent la colonne centrale

Le bloc `DRIVING AIDS` disparaît. Motif : ce sont des **capacités de la voiture**, pas des règles de session — le réglage n'existe que parce que la voiture les possède, ce que dit déjà la ligne d'aide existante (« the Audi TT Cup has both ABS and traction control »). Cette ligne d'aide suit dans le repli.

`Ideal line` **ne suit pas** : ce n'est pas une capacité de la voiture mais une assistance d'affichage. Elle rejoint `Tyre blankets` et `Penalties` dans SIMULATION.

### 2.3 Hauteur du panneau — critère d'acceptation

**Le panneau gauche ne doit jamais défiler.** Cas le plus chargé à vérifier : Race sélectionné, sous-entrée `Opponents` affichée, en **1920 × 1080**.

Si le compte n'y est pas, réduire dans cet ordre :

1. **Vignettes adaptatives** : sous 900 px de **hauteur** de fenêtre, la photo de voiture et le plan du circuit passent à 50 % de leur hauteur actuelle. C'est la hauteur de la fenêtre qui commande, pas sa largeur.
2. Repli des lignes `LAYOUT` et `SKIN` de la carte piste sur le modèle de `PERFORMANCE`.
3. Marge entre `START THE SESSION` et `Open Content Manager`, aujourd'hui généreuse.

Ne pas replier la liste des types pour gagner de la place : c'est la seule chose qui n'est pas négociable dans ce panneau.

---

## 3. Colonne centrale

### 3.1 Durées : suppression des cases à cocher

Les cases `Qualifying` et `Free practice` disparaissent. **Une valeur à 0 désactive la session correspondante.** Une seule ligne, trois champs du même gabarit :

```
LAPS  [  5 ]      QUALIFYING  [ 10 ] min      FREE PRACTICE  [ 20 ] min
```

- Valeur `0` affichée en `--text-disabled` ; valeur non nulle en blanc.
- La règle 1.2 du lot 1 (griser le champ, conserver la valeur au décochage) devient caduque : il n'y a plus rien à griser.
- La dépendance `Qualifying → Starting position` (SETUP§2.6) devient : `Qualifying > 0` neutralise `Start`.

### 3.2 Bloc SIMULATION

Contenu final : Damage, Fuel consumption, Tyre wear, puis une ligne de trois cases — `Tyre blankets`, `Penalties`, `Ideal line`.

### 3.3 Largeur des contrôles

Règle à conserver et à préciser : **un bloc peut prendre la largeur disponible, ses contrôles internes gardent leur gabarit et restent calés à gauche.** Un slider de 900 px pour un réglage qu'on pose au pourcentage près est une régression, pas un gain.

---

## 4. Rail droit : un seul bloc `CONDITIONS`

Les blocs `TRACK CONDITION` et `WEATHER` fusionnent. Un seul header, dans cet ordre :

1. Les huit tuiles météo
2. Air / Track / Wind
3. **Track condition** : le select des sept entrées + la ligne des quatre valeurs (`INITIAL GRIP … · GRIP TRANSFER … · RANDOMIZATION … · LAP GAIN … laps`) + la mention de repli
4. **Time** : le curseur d'heure, avec la **bande jour/nuit directement en dessous**, poignée alignée sur celle du curseur — les deux forment un seul composant, la bande est la légende du curseur
5. **Season** : le champ `Date` puis les cinq tuiles de saison

### 4.1 Corrections à faire au passage

- **La date affichée à droite de la bande jour/nuit est supprimée** : elle double le champ `Date` du bloc saison.
- **Deux phrases décrivent le même repli** sous le select d'état de piste (« Track state specified by weather, or Green, in case weather doesn't specify track state » en blanc, puis « Falls back to Green if the weather doesn't set the track state. » en gris). Ne garder que la seconde.
- **`INITIAL GRIP 0%` est affiché pour `Auto`** alors que la liste annonce Green à 95 % et que la spec impose les valeurs de Green. Valeur non résolue ou conversion d'échelle fausse sur ce champ — à corriger, c'est exactement le champ que le lot 4 va alimenter avec les presets CM.
- **Deux icônes météo flottent dans la gouttière** entre le panneau gauche et la colonne centrale (un nuage, un flocon), visibles sur les captures. Positionnement absolu échappé de son conteneur.
- **`Random / First / 2nd / Last`** mélange deux conventions d'écriture. Retenir `Random / 1st / 2nd / Last`.

---

## 5. Page adversaires

### 5.1 Un seul enchaînement, pleine largeur

La page contient, dans l'ordre, sans césure de carte entre eux :

```
filtres (recherche, + Filter, chips, pool)
contraintes (Same car / Same category / Same performance)
générateur (Opponents, Fill n at random, Difficulty, Aggression)
bannières d'alerte
grille
```

Motif : le bloc du haut configure un générateur, la grille en est la sortie. Les deux étaient séparés par une gouttière, ce qui rendait invisibles trois liens réels — la bannière du pool explique le contenu de la grille, `FILL AT RANDOM` et `REGENERATE` font des choses voisines, et la colonne `Str.` réagit à un curseur hors de vue.

`GRID · n AI` devient un sous-titre à l'intérieur de la page, avec `COLUMNS` et `REGENERATE` sur la même ligne.

### 5.2 Le bouton ⤢ disparaît

La page dédiée rend l'élargissement en place sans objet. `COLUMNS` reste.

### 5.3 Hauteur de la table plafonnée

- Hauteur maximale équivalant à une dizaine de lignes, **défilement interne**, ligne d'en-tête figée.
- La hauteur de la page cesse ainsi d'être fonction du nombre d'IA — le cas normal d'une course GT3 est 24 adversaires.
- C'est le seul défilement imbriqué autorisé dans l'application : une table de données est précisément le composant pour lequel cette convention existe.

### 5.4 Moins de colonnes par défaut

Huit colonnes au repos, c'est trop. Afficher par défaut : vignette, `Name`, `Driver name`, `Str.`, actions. `Nat.`, `kg/bhp`, `Ballast`, `Restrictor` passent derrière `COLUMNS`.

En affichage large, abréviations levées : `Restr.` → `Restrictor`.

### 5.5 Ce qui ne migre pas

Damage, fuel, tyre wear, penalties et ideal line **restent sur la page de réglages** : ce sont les règles de la course, pas les adversaires. Vérifier qu'aucun contrôle ne reste orphelin d'un côté ou de l'autre après le découpage.

---

## Hors périmètre

- **Lot 4** (presets d'état de piste de Content Manager) : indépendant, le select de §4 est celui livré au lot 2 et ne change pas ici.
- **Refonte visuelle de la grille** (vignettes larges, mise en page plus illustrée) : la page dédiée rend cette évolution possible, elle n'est pas demandée ici. §5.3 et §5.4 suffisent à alléger la table.
- **SPEC.md** : à mettre à jour au présent, par tes soins, une fois le lot passé.

## Point à signaler si l'implémentation le contredit

Le retour depuis `Opponents` se fait en cliquant l'entrée parente, c'est-à-dire un type de session déjà sélectionné. C'est le compromis assumé de cette structure. Si, une fois en main, ce geste ne se lit pas, remonte-le — la solution de repli est de rétablir une entrée `Session setup` distincte, au prix d'une ligne supplémentaire.
