# Pit Box — Cible de l'écran de réglages de session

> **Note ajoutée au versionnement, le reste du document est inchangé.**
>
> **Livré.** Conservé parce que le code y renvoie (`CIBLE§3.3`, `CIBLE§5.1`…)
> et parce qu'il porte les *arguments* — « le filtre définit le vivier, jamais
> le plateau » est une décision de conception, pas un détail d'écran.
> `SPEC-session.md` décrit ce que l'écran **est** ; celui-ci dit ce qui a été
> demandé et pourquoi. En cas d'écart, `SPEC-session.md` fait foi.


Ce document décrit **l'état visé complet** de l'écran de réglages de session, et le raisonnement
qui y mène. Il remplace les instructions de lot précédentes pour tout ce qui concerne le bloc
Adversaires.

Maquette de référence : `pitbox-session-global.html`.
Note annexe, pour plus tard, hors périmètre : `note-correction-performance-couche-mod.md`.

---

## 0. Comment lire ce document

**La maquette est à l'échelle réelle** — base 13 px, zoom 100 %, barème de `global.css`
(`.lbl` 9 px, `.lbl-key` 8 px, contrôles 10-12 px, body 13 px). Le bloc Adversaires y est dessiné
à **600 px**, la largeur qu'il reçoit réellement. Les zones hachurées sont des composants
existants, volontairement non redessinés. La maquette porte l'intention, pas le gabarit :
proportions et couleurs viennent du thème.

**Aucune affirmation n'est faite ici sur les noms de fichiers, de composants ou de champs
existants.** Les constater, puis les nommer dans le compte rendu. Les inexactitudes du premier
lot venaient toutes d'affirmations non vérifiées ; signaler explicitement celles que ce document
contiendrait encore.

**Règles transverses, valables partout dans ce document :**
- Barème rouge : le fond plein reste réservé au bouton de lancement. Toute sélection est de
  niveau 2 (texte rouge, fond éteint, filet 2 px).
- Persistance : l'état de chaque réglage est mémorisé **par type de session** ; les valeurs ne
  se propagent jamais d'un type à l'autre. Les migrations sont des boucles — une entrée par type
  dans `launch_state.json`, plus une par session enregistrée.
- Aucune donnée inventée présentée comme un fait : une valeur illisible s'affiche `—`, jamais
  estimée.
- L'app n'écrit jamais `race.ini` ; tout passe par le preset envoyé à Content Manager.
- Focus clavier jaune, navigation manette vérifiée, `prefers-reduced-motion` respecté.

---

## 1. Pourquoi — ce qui distingue Pit Box de Content Manager

Content Manager est un tableau de bord : vingt contrôles à plat, et l'utilisateur reconstitue son
intention à partir d'eux. Sa vraie limite n'est pas esthétique : **il ne connaît pas la
bibliothèque**. Son sélecteur d'adversaires est un arbre de noms parce qu'il n'a pas d'autre
matière.

Pit Box a une bibliothèque indexée — catégories, auteurs, années, pays, tags, état, specs. C'est
le seul avantage que CM ne peut pas rattraper, et c'est ce que cette cible exploite :

1. un plateau construit à partir d'un **vrai filtre**, combinable, au lieu de trois modes figés ;
2. des **specs réelles** (poids/puissance) pour composer un plateau cohérent ;
3. l'app **dit ce qu'elle sait** — vivier trop maigre, adversaire non activé, valeur illisible —
   au lieu de laisser deviner.

---

## 2. Ce qui est déjà livré et qui ne bouge pas

- Blocs `Session type`, `Session options`, `Simulation` : zones fixes, segmentés horizontaux,
  aides à trois états, info-bulles, ligne « Factory » contextuelle.
- Modale de sélection d'adversaire : deux contextes (ajouter / remplacer), barre de filtres
  partagée avec la bibliothèque, jeton « jouable » épinglé, sélection multiple, pied avec
  compteur.
- Garde d'activation de l'écran de session.
- Plateau : vignettes `preview.jpg` en paysage, bulle au survol, bouton `Regenerate`, valeurs de
  difficulté accrochées aux poignées, colonne réservée à droite du nom.
- Point ouvert, **à ne pas traiter ici** : l'évolution du grip n'est toujours pas transmise au
  jeu.

**L'extraction de la barre de filtres du lot précédent est le socle de cette cible.** Elle va
maintenant servir une troisième fois, en ligne dans le bloc Adversaires.

---

## 3. Le bloc Adversaires — le filtre remplace les onglets

### 3.1 Le raisonnement

Les trois onglets `Same car` / `By category` / `Free` faisaient deux choses :

1. **définir un ensemble de voitures** — ce que la barre de filtres fait déjà, en mieux ;
2. **générer un plateau** à partir de cet ensemble.

Seule la seconde justifiait leur existence. La première était un doublon, et c'est ce doublon qui
obligeait à des règles de réconciliation (« retirer un jeton ne change pas l'onglet », « le
plateau passe en manuel ») : le symptôme, pas la cause.

Les onglets disparaissent. Le bloc Adversaires porte **la même barre de filtres** que partout
ailleurs.

### 3.2 Composition du bloc

Dans l'ordre vertical :

1. **Barre de filtres** — composant partagé, identique à la bibliothèque : champ de recherche,
   menu `+ Filter`, jetons tri-état, `Clear all`. À droite du bandeau de jetons, le compteur
   **`Pool · 42 cars`**.
2. **Trois puces de raccourci** — `Same car`, `Same category`, `Same performance`. Elles **posent
   un jeton et rien d'autre** ; ce ne sont plus des modes. Une puce paraît active quand son jeton
   est présent. Elles existent pour le geste d'un clic et pour la lisibilité au premier regard,
   que des jetons seuls n'offrent pas.
3. **Ligne de contrôles** — `Opponents` (nombre), bouton `Fill N at random`, `Difficulty`
   (fourchette), `Aggression` (curseur simple).
4. **Avertissement de vivier maigre** s'il y a lieu (§3.5).
5. **Le plateau** (§4).

### 3.3 Le filtre définit le vivier, jamais le plateau

C'est le point à ne pas se tromper. Entre le vivier et le plateau il y a toujours un geste, et il
y en a exactement deux, qui travaillent tous deux sur l'ensemble filtré :

- **`Fill N at random`** — tire N voitures au hasard dans le vivier et **remplace** le plateau.
  Le chemin de celui qui veut courir tout de suite.
- **`+ Choose from the N…`** — ouvre la modale existante sur **ce même filtre**, cases à cocher,
  et **ajoute** en fin de plateau. Le chemin de celui qui veut décider.

Le libellé du second porte le nombre du vivier : c'est ce qui dit que le filtre a servi à rendre
le choix praticable — 42 lignes à parcourir au lieu de 600.

**Conséquence sur le travail déjà fait :** la dérivation des jetons de vivier depuis l'onglet
actif disparaît. La modale ne *dérive* plus rien, elle **partage l'état de filtre du bloc**. Le
jeton « jouable » reste épinglé en plus, comme aujourd'hui. C'est plus simple que ce qui est en
place.

### 3.4 Le jeton `Performance`

Nouveau type de jeton dans la barre de filtres : **`Performance : ±15% of my car`**.

- Critère : rapport **poids / puissance** en kg/bhp, calculé depuis les specs.
- Écart **relatif** au ratio de la voiture pilotée, réglable de **±5 % à ±50 %**, pas de 5,
  défaut **±15 %**.
- Le jeton affiche, quand il est posé : la référence (voiture, puissance, poids, ratio), les
  bornes calculées en kg/bhp, et le nombre de voitures écartées faute de specs lisibles.
- Disponible **aussi dans la bibliothèque** : « montre-moi tout ce qui roule au niveau de ma
  488 » est une question qu'on se pose hors session.

**Lecture des specs — conservatrice.** Les valeurs d'`ui_car.json` sont sales : `"450bhp"`,
`"1200 kg"`, `"—"`, champ absent, valeur nulle. Extraire le nombre en tolérant l'unité collée ou
espacée ; **ne convertir aucune unité par inférence** (ch/PS/kW) — si l'unité n'est pas reconnue
avec certitude, traiter comme illisible. Une voiture illisible est **écartée du vivier**, jamais
estimée, et affiche `—` dans la colonne du plateau. Calcul mis en cache avec les specs, pas
recalculé à chaque frappe du curseur.

Si **la voiture pilotée** est illisible, la puce `Same performance` est désactivée et l'explique.
Pas de repli silencieux sur un tirage au hasard.

### 3.5 Avertissement de vivier maigre

Quand le vivier contient moins de voitures distinctes que le nombre d'adversaires demandé, une
ligne le dit :

- `Only 1 car in #gt1 — the grid repeats it with different skins.`
- `Only 3 cars in this band — widen it to get more variety.`

Couleur du fragment en gras : `--warn` (`#c8b06a`), la même que la garde d'activation. **Pas de
rouge** : ce n'est pas une erreur, le plateau reste jouable. C'est un constat, pas un blocage.

---

## 4. Le plateau

### 4.1 Cellules `Auto`

**C'est le mécanisme central, et il remplace le marqueur « plateau manuel ».**

Chaque cellule éditable d'une ligne vaut soit `Auto`, soit une valeur explicite :

- **`Auto`** — la valeur est retirée au sort à chaque `Fill` ou `Regenerate`.
- **Valeur explicite** — posée à la main, elle n'est **jamais** écrasée par un tirage.

S'applique à la force, au nom de pilote, à la nationalité, et à l'agressivité si elle existe par
ligne. Affichage : `Auto` en `--text-disabled`, valeur explicite en `--text`. Un menu de ligne
permet de repasser une cellule en `Auto`.

Conséquence : plus de marqueur global, plus de confirmation avant régénération, plus de règle
« changer la voiture pilotée régénère sauf si… ». La question se pose cellule par cellule. Le
comportement de régénération existant s'aligne sur ce modèle.

### 4.2 Colonnes

Par défaut : **voiture + skin**, **kg/bhp**, **force**. Les autres s'ajoutent via un bouton
`Columns`, **le même composant que la vue tableau de la bibliothèque** — même geste, même menu.

Colonnes disponibles : `Driver name`, `Nationality`, `Ballast`, `Restrictor`.

- **`Driver name`** — par défaut le pilote déclaré dans le `ui_skin.json` du skin, affiché en
  italique éteint comme une valeur `Auto`. Taper quelque chose l'écrase pour cette ligne, dans
  cette grille. Rien à voir avec les mods de tenue de pilote, qui sont une apparence 3D.
- **`Nationality`** — même origine, même comportement.
- **`Ballast` / `Restrictor`** — équilibrage d'un plateau hétérogène. Affichent la valeur
  effective ; si une correction permanente existe un jour au niveau du mod, elle s'affichera ici
  en `--warn` pour dire qu'elle est héritée (hors périmètre, voir la note annexe).

### 4.3 La contrainte des 600 px, et le bouton `⤢`

L'écran équivalent de CM fait ~1490 px de large et affiche huit colonnes de front. Le bloc
Adversaires en reçoit ~600. **Quatre colonnes optionnelles rentrent, pas davantage.**

Le bouton `⤢` de l'en-tête du plateau **élargit temporairement le bloc sur toute la largeur du
contenu**, la colonne météo passant dessous le temps qu'on compose. C'est la seule sortie
honnête : rogner les colonnes ou faire défiler horizontalement sont pires.

Le menu `Columns` indique combien de colonnes supplémentaires tiennent à la largeur courante, et
renvoie à `⤢` pour le reste. L'état élargi est mémorisé comme les autres réglages.

### 4.4 En-tête et pied du plateau

En-tête : `Grid · N AI`, puis à droite **`Start`** (position de départ), `Columns`, `⤢`,
`Regenerate`.

- **Position de départ** : `Last` (défaut), `First`, `Random`, `Position…`. Bornée par la taille
  du plateau + 1 ; réduire le nombre d'adversaires reborne la valeur. **Type `Race` uniquement.**
- **Agressivité** : 0 à 100 %, pas de 5, défaut **0 %** — le défaut de CM, à ne pas « améliorer ».
  Sa place est la ligne de contrôles, à droite de la difficulté : ce sont les deux réglages qui
  décident du caractère de la course.

Pied : `Save grid…` et `Load grid…` (§5).

**À vérifier avant de câbler** : le preset Quick Drive porte-t-il des champs pour la position de
départ et pour l'agressivité ? Si l'un manque, **ne pas livrer le contrôle correspondant** et le
signaler. Un contrôle sans effet en jeu est exactement le défaut que l'évolution du grip traîne.

### 4.5 Un piège à dire à l'utilisateur

Constat remonté par des utilisateurs de CM : les forces posées par ligne **ne remplacent pas** le
curseur de difficulté global — les deux se multiplient. Modifier le curseur global a donc un effet
massif même sur des forces explicites, et il faut le maintenir à une valeur pivot pour obtenir des
résultats fidèles.

Ça fait perdre du temps à des gens depuis des années. Une ligne d'explication affichée au moment
où une force explicite est posée vaut mieux que dix fils de forum.

---

## 5. Grilles enregistrées

### 5.1 Deux objets distincts

| | Session enregistrée | Grille enregistrée |
|---|---|---|
| Contient | Tout : duo, météo, heure, options, **plus une copie de la grille** | Les seuls adversaires |
| Sert à | Rejouer une session complète | Rejouer un plateau **ailleurs** — le même GT3 sur dix circuits |
| Existe déjà | Oui (panneau en haut à droite) | Non |

**Une session enregistre une copie de sa grille, jamais un lien.** Sinon, modifier une grille
changerait silencieusement toutes les sessions qui la citent.

Un objet unique ne suffirait pas : appliquer une grille à une session **en cours de
configuration**, sans perdre la météo, l'heure et le type de session, est le cas réel — et les
presets de grille de CM n'ont aucune session attachée (§6).

### 5.2 Forme

**Liste plate avec recherche, pas d'arborescence.** CM range ses grilles dans une arborescence de
fichiers ; un dossier est une taxonomie qu'il faut inventer et maintenir, un nom cherchable ne
demande rien. Le champ de recherche et les jetons existent déjà.

Chaque entrée : nom, nombre d'IA, et un badge `From CM` si elle vient d'un import.

---

## 6. Import Content Manager

### 6.1 Deux moments, un seul mécanisme

- **Au premier lancement** : si des presets CM sont détectés, une proposition non intrusive —
  « X grilles et Y sessions trouvées, Pit Box peut les importer ». Deux actions : examiner et
  importer, ou refuser. Un refus est **définitif**, la proposition ne revient pas.
- **En permanence** : un bouton d'import dans la **page d'import de mods** existante. Pas dans
  l'écran de session.

### 6.2 Import, jamais synchronisation

Lire une fois, convertir en objet Pit Box, **ne plus jamais dépendre du fichier**. Une passerelle
vivante rendrait l'app dépendante du format de CM — ce que la spec refuse déjà pour la base en
ligne d'AcTools. CM garde ses fichiers, Pit Box a ses copies.

Emplacements à constater plutôt qu'à supposer : les presets de CM vivent sous
`%LocalAppData%\AcTools Content Manager\Presets\`, les grilles dans un sous-dossier dédié et les
presets de session dans un sous-dossier voisin. **Vérifier les chemins et le format réels avant de
câbler.**

### 6.3 Un rapport d'import, pas un échec

Une grille CM cite des voitures par identifiant ; certaines seront absentes, d'autres désactivées.
**L'import aboutit toujours** et rend un rapport : ce qui est importé, ce qui manque, ce qui est
désactivé. Pour les mods simplement désactivés, la garde d'activation existante répare d'un clic.

---

## 7. Ordre de livraison indicatif

Ce découpage est une proposition, motivée par la capacité à relire et à changer d'avis entre deux
passes, pas par une contrainte technique. **Si un autre découpage paraît meilleur, le proposer
avant de commencer** plutôt que de l'appliquer en silence.

**Passe A — filtre et plateau.** Barre de filtres en ligne dans le bloc, suppression des onglets,
puces de raccourci, jeton `Performance`, `Fill` / `Choose`, cellules `Auto`, colonnes optionnelles
et `⤢`, position de départ, agressivité, avertissement de vivier maigre.

**Passe B — grilles et import.** Grilles enregistrées, import des presets CM, proposition au
premier lancement, bouton dans la page d'import.

Les deux passes n'ont presque rien en commun : la seconde est du stockage et de la lecture de
fichiers. Les séparer ne coûte aucun effort partagé.

---

## 8. Recette

1. Poser `Category : #gt3` **et** `Year : 2010–2016` **et** `except Kunos` : la combinaison
   fonctionne, le compteur de vivier est juste. C'était impossible avec les onglets — c'est le
   gain principal.
2. Cliquer une puce pose son jeton ; le retirer désactive la puce. Aucun autre effet de bord.
3. `Fill N at random` remplace le plateau ; `Choose from the N…` ouvre la modale sur **le même
   filtre** et **ajoute** en fin de plateau.
4. Poser une force explicite sur une ligne, puis `Regenerate` : cette ligne est intacte, les
   lignes `Auto` ont changé.
5. Changer la voiture pilotée : mêmes règles, aucune confirmation demandée, aucune valeur
   explicite perdue.
6. Jeton `Performance` : le compteur, les bornes et le nombre d'exclusions se mettent à jour sans
   saccade sur la bibliothèque complète.
7. Une voiture aux specs illisibles n'entre **jamais** dans un vivier de performance et affiche
   `—`. Tester sur un mod réellement cassé.
8. Piloter une voiture aux specs illisibles : la puce `Same performance` est désactivée et
   expliquée ; aucun tirage au hasard.
9. Ajouter quatre colonnes à 600 px : elles tiennent. En ajouter une cinquième : le menu renvoie à
   `⤢`, et `⤢` élargit sans rien casser.
10. Nom de pilote : la valeur par défaut vient du skin, en italique éteint ; la saisie l'écrase ;
    le menu de ligne la repasse en `Auto`.
11. Position de départ présente en `Race`, absente ailleurs, rebornée quand le nombre d'IA baisse.
12. Enregistrer une grille, la charger dans une session déjà configurée : **seuls les adversaires
    changent**, météo, heure et type de session sont intacts.
13. Modifier une grille enregistrée : les sessions qui l'avaient utilisée ne bougent pas.
14. Importer un preset CM citant un mod absent et un mod désactivé : l'import aboutit, le rapport
    les nomme, la garde d'activation répare le second.
15. Refuser la proposition d'import au premier lancement : elle ne revient plus.
16. Cloisonnement par type de session sur tous les nouveaux réglages.
17. Une session enregistrée **avant** ces passes se recharge sur les défauts, sans erreur, sur
    plusieurs types de session et pas seulement sur le preset actif.
18. Test §7.2ter : effondrer les neutres, aucun fond rouge plein hors bouton de lancement.
19. `L'écran bibliothèque voitures se comporte exactement comme avant` — la barre de filtres sert
    maintenant à trois endroits, une régression y passerait inaperçue.

---

## 9. Compte rendu attendu

- Noms réels des composants, structures et champs touchés.
- Les specs numériques (puissance, poids) étaient-elles exploitables ? Combien de voitures de la
  bibliothèque sont illisibles ?
- Le preset Quick Drive porte-t-il la position de départ et l'agressivité ? Si l'un manque, ce qui
  a été livré à la place.
- Chemins et format réels des presets CM constatés.
- Si un autre découpage a été retenu, lequel et pourquoi.
- **Toute affirmation de ce document qui s'est révélée inexacte** — la signaler explicitement,
  comme pour les lots précédents.

Aucune modification de SPEC.md : la spec est mise à jour côté conception une fois recetté.
