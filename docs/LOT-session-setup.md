# Instruction Claude Code — Session setup : corrections et réorganisation

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> Ce lot est **livré**. Le document est conservé parce que le code y renvoie
> (`SETUP§…`) et parce qu'il porte les *arguments* derrière les choix — la
> partie qu'on ne retrouve pas deux fois. `SPEC-session.md` décrit ce que
> l'écran **est** aujourd'hui ; celui-ci dit ce qui a été demandé et pourquoi.
> En cas d'écart, `SPEC-session.md` fait foi.


**Périmètre : lots 1 et 2 uniquement.** Les lots 3 et 4 sont listés en fin de document pour contexte, ils ne sont pas à traiter ici.

Toutes les chaînes d'interface sont en anglais. Les valeurs et règles ci-dessous sont à appliquer telles quelles : là où une valeur n'est pas donnée, c'est qu'elle est à relever dans le code existant — ne pas l'inventer.

---

## Règles générales applicables aux deux lots

- **Système de gris** : deux niveaux seulement. `--text-secondary` pour une information lisible, `--text-disabled` pour un état vide ou inactif. Ne pas introduire de troisième niveau.
- **Bordures** : une seule couleur, `--border`.
- **Typographie** : mono pour les données, libellés et nombres ; sans-serif réservé à la prose.
- **Encarts jaunes** : réservés aux problèmes de configuration courante appelant une action de l'utilisateur. Jamais pour une explication permanente — celles-ci passent par l'icône ⓘ, comme sur ABS et Traction control.
- **Aucun réglage visible et modifiable ne doit être sans effet.** S'il est neutralisé par le contexte, il est désactivé et grisé, avec un ⓘ expliquant pourquoi. S'il n'a aucun sens dans le type de session courant, il est retiré de l'écran, pas grisé.

---

## LOT 1 — corrections sans dépendance

Aucun arbitrage dans ce lot, il peut partir en premier.

### 1.1 Convention de libellé dans SESSION OPTIONS

Une seule convention dans tout le bloc : libellé **au-dessus** du champ, mono, majuscules, `--text-secondary`.

À corriger : `LAPS`, actuellement placé à droite de son champ.

### 1.2 Durées de qualification et d'essais libres

Chaque case et sa durée sur une même ligne :

```
[x] Qualifying       [ 10 ] min
[x] Free practice    [ 20 ] min
```

- Le champ de durée est désactivé et grisé quand la case est décochée.
- La valeur est **conservée** quand on décoche, pas remise à zéro.
- Les deux spinners actuellement sans libellé disparaissent au profit de ces deux lignes.

### 1.3 Penalties

`Penalties` quitte le bloc SESSION OPTIONS (où elle flotte sous la colonne grip) et rejoint **SIMULATION**, sur la même ligne que `Tyre blankets`.

`Ideal line` ne bouge pas : elle reste dans DRIVING AIDS avec ABS et Traction control.

### 1.4 Champ Date

Le champ `Date` sort de la rangée des saisons : il n'est pas un toggle et ne doit pas avoir le gabarit d'une tuile.

- Ligne propre au-dessus de la rangée, libellé `DATE`.
- La rangée des saisons ne contient plus que les cinq tuiles : None / Spring / Summer / Autumn / Winter.

### 1.5 Chaîne tronquée en bas de grille

`+ Choose from the 3...` est coupée. Chaîne complète, sans ellipse :

```
+ Choose from the pool · 3 cars
```

Singulier géré : `· 1 car`.

### 1.6 Aperçu voiture au survol d'une ligne

Le panneau d'aperçu ne doit jamais recouvrir les lignes situées **au-dessus** de la ligne survolée : ce sont celles que l'utilisateur est en train de comparer.

- Bord supérieur aligné sur le haut de la ligne survolée, le panneau déborde vers le bas.
- Bord gauche à droite de la vignette de la ligne.
- Hauteur maximale 240 px.
- Si la fenêtre n'a pas la place en bas, remonter du strict nécessaire pour rester dans la fenêtre.

### 1.7 En-têtes de colonnes de la grille

Distinguer les colonnes éditables des colonnes en lecture seule, en utilisant uniquement les deux gris existants :

- En-têtes des colonnes éditables (`Driver name`, `Nat.`, `Str.`, `Ballast`, `Restr.`) : `--text-secondary`.
- En-têtes des colonnes en lecture seule (`Name`) : `--text-disabled`.

Les cellules elles-mêmes ne changent pas : les champs continuent d'apparaître au survol.

### 1.8 Encart d'explication Strength / Difficulty

L'encart jaune « A strength set on a line does not replace the difficulty range: the two multiply. Move the range and these lines move with it. » est **supprimé**.

Son contenu est repris tel quel dans un ⓘ placé à côté du libellé `DIFFICULTY`. Le texte définitif sera révisé au lot 3 ; ne pas le reformuler ici.

L'encart « Only 3 cars in the pool — widen the filter for more variety. » **reste en jaune** : c'est bien une alerte de configuration.

### 1.9 Doublons de pilote dans la grille

La grille produit actuellement deux lignes identiques (`59 Juan`).

- Quand `Driver name`, `Nat.` ou le numéro sont en `Auto`, la génération garantit l'unicité du couple (numéro, nom) au sein d'une même grille.
- Si l'utilisateur force manuellement un doublon, afficher un encart jaune : `Two drivers share the same number.`

---

## LOT 2 — réorganisation de l'écran

Ce lot se juge sur le rendu réel, pas sur une maquette : fais tourner l'application et compare avant/après.

### 2.1 Rail droit

Le rail droit devient le bloc des **conditions de course**, et rien d'autre :

```
Track condition   (voir 2.2)
Weather           air / track / wind, time, season
```

`Saved sessions` quitte le rail : il part dans l'en-tête de page, sous forme de deux boutons ouvrant une modale (voir 2.11). Le rail commence donc directement par Track condition.

Deux propriétés à préserver :

- **Le rail est identique dans les quatre types de session.** C'est lui qui donne à l'écran sa silhouette constante pendant que la colonne centrale grandit ou rétrécit.
- **Track condition et Weather sont voisins par nécessité**, pas par commodité de mise en page : l'entrée `Auto (set by weather)` n'a de sens qu'à côté de la météo qui la pilote. Ne pas les séparer ni intercaler un bloc entre les deux.

En Practice, où la colonne centrale est courte, le rail porte l'essentiel du réglage — on est seul en piste, la météo et l'état de la piste *sont* la session. Le déséquilibre visuel est donc un signal juste, pas un défaut à compenser (voir 2.10).

### 2.2 Track condition

`Grip evolution` quitte SESSION OPTIONS et devient un bloc du rail droit, sous le header `TRACK CONDITION`.

- **Composant** : un select (dropdown) listant les sept entrées dans l'ordre de Content Manager — Auto (set by weather), Dusty, Old, Green, Slow, Fast, Optimum. La liste verticale actuelle à sept lignes disparaît.
- **Sous le select**, une ligne de lecture en mono, `--text-secondary`, affichant les quatre valeurs réelles du preset sélectionné, avec le vocabulaire de CM :

```
INITIAL GRIP 95%  ·  GRIP TRANSFER 50%  ·  RANDOMIZATION 2%  ·  LAP GAIN 132 laps
```

(valeurs d'exemple — afficher celles du preset retenu)

- **Pour `Auto (set by weather)`** : même ligne, avec les valeurs de `Green`, plus une mention en dessous : `Falls back to Green if the weather doesn't set the track state.`
- **Lecture seule.** L'édition et les presets utilisateur sont hors périmètre (lot 4).
- Le détail actuellement affiché au survol est supprimé : il devient redondant.

**Modèle de données** : traiter un état de piste comme un objet nommé porteur de quatre valeurs, pas comme un enum à sept cas. Le lot 4 ajoutera des états de provenances différentes sans que ce modèle ait à changer.

### 2.3 Largeur des blocs

Le bouton ⤢ élargit actuellement toute la page. Il ne doit élargir que la grille.

- **Max-width sur tous les blocs de la colonne centrale** (Session type, Session options, Simulation, et l'en-tête d'Opponents), calés à gauche. Valeur : la largeur qu'ils ont aujourd'hui en mode normal — la relever dans le code, ne pas la recalculer.
- **Seul le bloc `GRID`** prend la largeur disponible en mode élargi.

Motif : les sliders de SIMULATION étirés sur toute la largeur ont une course souris disproportionnée pour un réglage qu'on pose au pourcentage près, et le bloc ne ressemble plus au même composant d'un mode à l'autre.

### 2.4 Colonnes de la grille en mode élargi

- La largeur gagnée sert à afficher **plus de colonnes**, pas à étirer les existantes.
- Cap de largeur sur `Name` : l'écart entre `Name` et `Driver name` ne doit pas être assez large pour qu'on perde la ligne en la parcourant.
- Abréviations levées en mode élargi : `Restr.` → `Restrictor`. Même mot que dans la carte voiture du joueur (voir 2.7), pour que le lien entre les deux réglages soit visible.

### 2.5 Contenu variable de SESSION OPTIONS selon le type

**Ordre des blocs de la colonne centrale, invariant dans tous les types :**

```
Session type      toujours, hauteur fixe
Session options   toujours, contenu variable
Simulation        toujours, hauteur fixe
Opponents / Ghost variable — toute la variabilité est ici, en bas
```

Rien de ce qui bouge ne doit se trouver au-dessus de ce qui ne bouge pas.

Par type :

| Type | SESSION OPTIONS | Bloc de bas de colonne |
|---|---|---|
| Practice | contenu actuel, inchangé | aucun |
| Hotlap | Ghost car + Advantage (voir 2.8) ; ni laps, ni qualifying/free practice, ni starting position | aucun |
| Race | laps, jump start, starting position, qualifying + durée, free practice + durée | Opponents |
| Track day | contenu actuel, inchangé | Opponents |

Un champ sans objet dans le type courant est **retiré**, pas grisé. Le grisé reste réservé aux dépendances internes (case décochée → durée grisée).

### 2.6 Starting position (Race uniquement)

- Segmented control à quatre segments, même gabarit que `Jump start`, placé immédiatement après lui dans SESSION OPTIONS.
- Valeurs : `Random` / `1st` / `2nd` / `Last`. Défaut : `Random`.
- **À vérifier avant implémentation** : ce que fait Content Manager quand `Qualifying` est coché. Si CM neutralise le réglage (grille déterminée par les résultats de qualification), faire de même — champ grisé, ⓘ : `Grid is set by qualifying results.` Aligner le comportement sur CM, ne pas trancher seul.

### 2.7 Ballast et Restrictor du joueur

Ces deux réglages valent pour **tous les types de session**. Ils ne dépendent pas du type et ne doivent donc pas figurer dans SESSION OPTIONS : leur place est dans la **carte voiture du panneau gauche**, deux lignes sous `LIVERY` et `DRIVER`, au même gabarit de ligne.

```
BALLAST       [   0 ] kg
RESTRICTOR    [   0 ] %
```

- `BALLAST` : entier, 0–200, pas de 1, défaut 0.
- `RESTRICTOR` : entier, 0–100, pas de 1, défaut 0.
- **Champs numériques, pas de sliders** : ce sont des valeurs qu'on pose, pas qu'on tâtonne.
- **Valeur 0** affichée en `--text-disabled`. **Valeur non nulle affichée en rouge**, pour qu'un handicap oublié se voie sans lire la ligne.
- **Pas de chevron** sur ces deux lignes. `LIVERY` et `DRIVER` en portent un parce qu'elles ouvrent un sélecteur ; ici la valeur s'édite sur place. Même gabarit de ligne, affordance différente.
- **Ces deux valeurs font partie de la configuration sauvegardée** d'une session, au même titre que celles de SESSION OPTIONS. Leur place dans le panneau gauche ne les en sort pas : une session rechargée doit restituer le ballast et le restrictor enregistrés.

*(Note : une consigne orale antérieure demandait de ne pas traiter ce point. Elle est levée — 2.7 est à implémenter tel qu'écrit ici.)*

### 2.8 Ghost car (Hotlap uniquement)

Dans SESSION OPTIONS, une seule ligne :

```
[ ] Ghost car     Advantage [ 0.00 ] s
```

- `Advantage` : 0.00–5.00, pas de 0.10, deux décimales, défaut 0.00.
- Spinner, pas de slider.
- Champ désactivé et grisé tant que la case est décochée ; valeur conservée au décochage.

### 2.9 Difficulty et Aggression : modèle centre ± écart

Les deux réglages passent au même modèle et partagent **un seul composant, instancié deux fois**.

- **Manipulation** : une poignée principale sur le curseur (le centre) + un champ numérique `±` à côté.
- **Déplacer le centre conserve l'écart.** C'est le geste fréquent (monter tout le plateau de quelques points) et il ne doit pas demander deux manipulations.
- **Libellé** : `95% ± 3 (92–98)`.
- **Bornage** : appliqué à l'affichage **et** à l'envoi au jeu. Le libellé affiche la plage **bornée**, jamais la plage théorique — un centre de 3 avec ± 5 donne `3% ± 5 (0–8)`, pas `(-2–8)`.
- **± 0 doit rester atteignable sans bagarre au pointeur** : c'est le champ numérique qui le garantit.
- **Bornes de chaque réglage** : inchangées, celles en vigueur aujourd'hui.
- **Stockage** : centre + écart, pas min + max.
- **Migration des sessions sauvegardées** : centre = (min + max) / 2, écart = (max − min) / 2 ; si (max − min) est impair, arrondir l'écart à l'entier supérieur puis borner.

### 2.10 Practice : ne rien compenser

En Practice, la page est plus courte. C'est le comportement voulu.

- **Ne pas réserver d'espace vide** en attente de contenu qui ne viendra pas.
- **Ne pas centrer verticalement** : les blocs restent ancrés en haut.
- **Ne pas changer le nombre de colonnes** selon le type de session. Un bloc qui change de largeur change aussi ses retours à la ligne et la position de ses libellés : l'écran ne se ressemble plus d'un mode à l'autre.
- Le bouton `START THE SESSION` étant en permanence dans le panneau gauche, une page courte ne laisse personne sans action suivante.

### 2.11 Sessions sauvegardées : passage en modale

Sauvegarder et recharger une configuration nommée est le même geste pour une grille et pour une session. Les deux doivent donc avoir la même grammaire d'interface, et c'est celle de la grille qui est retenue : deux boutons, une modale.

**Remplacement du bloc `SAVED SESSIONS`** (aujourd'hui en tête du rail droit) par deux boutons `SAVE SESSION…` et `LOAD SESSION…`, au même gabarit que `SAVE GRID…` / `LOAD GRID…`.

**Placement : dans la barre de titre, à droite de « Session setup ».** Pas à côté de Session type — ça suggérerait que la sauvegarde est rattachée au type de session, alors qu'elle porte sur toute la configuration de la page. Règle générale à appliquer ici : *l'action se place au niveau de ce qu'elle sauvegarde.* Save/Load grid restent en bas de la grille, Save/Load session vont dans l'en-tête de page.

**Le compteur** aujourd'hui affiché à droite du header passe sur le bouton : `LOAD SESSION… (12)`.

**Modale `LOAD A SESSION`** : même structure que `LOAD A GRID` — champ de recherche, liste, croix de suppression par entrée.

**Filtre par type de session : à supprimer.** La liste ne doit plus être restreinte aux sessions du type courant. Motif : la restriction est invisible. Un utilisateur qui a enregistré une session en Race et la cherche depuis Practice ne voit pas une liste filtrée, il voit une liste vide — et il en conclut que sa sauvegarde a échoué, pas qu'un filtre s'applique.

À la place, **le type devient une propriété affichée** de chaque entrée, sur le modèle du `6 AI` déjà affiché sous `VRC` dans la modale de grille :

```
GT3 at Spa
Race · Spa · 2026-08-28 09:10
```

- **Tri** : les sessions du type courant en tête, les autres à la suite. Le bénéfice pratique du filtre est conservé sans que rien ne soit masqué.
- **Recherche** : porte sur toutes les entrées, tous types confondus.
- Si un filtre explicite s'avère nécessaire plus tard, il prendra la forme d'une puce visible et effaçable (`Race ×`) en haut de la modale — jamais d'un masquage silencieux.

**À confirmer avant implémentation** : charger une session bascule-t-il bien le type de session, puisque le type fait partie de ce qui est sauvegardé ? Si oui, charger une session Race depuis Practice est une opération valide et il n'y a aucune raison de la masquer. Si non, c'est ce comportement-là qu'il faut corriger, pas la liste.

**Points de suspension** : `SAVE SESSION…` et `LOAD SESSION…` les portent, au sens classique « ouvre une boîte de dialogue » — cohérent avec les boutons de grille existants. Raison supplémentaire de corriger la troncature de `+ Choose from the 3...` (voir 1.5), qui ressemble à cette convention sans en être une.

---

## Hors périmètre de cette instruction

- **Lot 3 — sémantique Strength / Effective.** Bloqué sur la vérification ci-dessous.
- **Lot 4 — état de piste.** Édition des quatre valeurs et lecture des presets utilisateur de Content Manager (en lecture seule, en groupe séparé sous les sept entrées natives, jamais en remplacement de celles-ci).
- **Ordre des onglets de type de session** : non décidé, ne pas y toucher.
- **SPEC.md** : à mettre à jour au présent, par tes soins, une fois les lots passés.

## Question à traiter avant le lot 3

Dans Content Manager, la valeur `Strength` posée sur une ligne **multiplie-t-elle** le niveau tiré de la plage de difficulté, ou le **remplace-t-elle** pour cette voiture ?

Et le comportement actuel de Pit Box : est-il repris de CM, ou choisi à l'implémentation ?

L'enjeu : si CM remplace et que Pit Box multiplie, un utilisateur qui reproduit une grille connue obtiendra des IA plus lentes sans jamais soupçonner le réglage.
