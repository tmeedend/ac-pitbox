# Lancement de session

Comment on choisit ce qu'on va piloter, comment on règle la session, et comment
elle part dans le jeu. **Le code y renvoie par l'étiquette `SESSION§`** ;
renuméroter ici casse `npm run check`, et c'est voulu. Dans ce fichier aussi,
**un `§` nu désigne `SPEC.md`**. Ce qu'on a lu dans le code source de Content
Manager pour fixer ces règles est dans `L4-cm-launch-research.md`.

## 1. La bibliothèque est le sélecteur

Pas d'écran séparé de sélection : la voiture et le circuit choisis dans la
bibliothèque sont ceux de la session. La **colonne de session** montre le duo
sur les quatre écrans de la zone session (§7.2) — bibliothèques, Pilote,
réglages de la session.

**La colonne est la session, et ses parties sont des destinations** (§7.2) : la
carte circuit ouvre la bibliothèque des circuits, la carte voiture celle des
voitures, la ligne `PILOTE` l'écran Pilote, un type de session ses réglages.
Elle porte le type de session (SESSION§1.1), **deux blocs à l'anatomie
identique** — vignette, nom, source, puis les champs — et le lancement :

```
CIRCUIT
  [ vignette ]                     ← cliquable : ouvre la bibliothèque
  Imola
  Kunos
  LAYOUT     Imola — layout unique     (statique : une seule option)
  SKIN       Celui d'origine     ▾
VOITURE
  [ vignette ]
  Porsche 718 Boxster S
  Porsche · 2016
  LIVRÉE        ▪ Miami Blue    ▾
  PILOTE          Mon pilote    ›
  PERFORMANCE     D'origine     ▾  ← repli : lest, bride, ABS, antipatinage
SESSION
  Essais
  Hotlap
  Course                           ← sélectionné
    Adversaires  6 IA · 87 % ± 3   ← sous-entrée, filet d'attache
  Track day
  [ ▶ DÉMARRER LA SESSION      ]
```

**L'ordre suit celui de la décision** : le circuit, la voiture, le genre de
séance, puis le bouton de lancement, juste sous les types.

**La colonne ne défile jamais**, y compris dans le cas le plus chargé (Course
sélectionnée, sous-entrée affichée, 1920 × 1080). Deux dispositifs y
suffisent : le repli `PERFORMANCE` et des **vignettes adaptatives** — sous un
seuil de hauteur (une requête de conteneur à 960 px, jamais de média), les
photos de voiture et de circuit passent **en fond de leur bloc**, assombries
et floutées, sous le nom et les champs ; elles ne sont jamais rognées en
bandeau. Seul le nom mène alors à la bibliothèque. **La liste des types ne se
replie jamais.**

### 1.1 Le type de session EST la navigation

Les quatre types — Essais, Hotlap, Course, Track day — forment une liste
**toujours dépliée**. Cliquer un type le sélectionne **et** ouvre ses réglages.

- **Deux marques** : le libellé en pleine lumière = le type qui partira, sur
  tous les écrans ; le filet rouge = on le regarde, sur l'écran de session
  seulement.
- **La sous-entrée `Adversaires`** ne paraît que sous Course et Track day, sous
  le type choisi. Elle porte un **résumé** — `6 IA · 87 % ± 3`, nombre d'IA puis
  difficulté en centre ± écart —, puisqu'on peut lancer sans ouvrir la page.
- **Les alertes de la page adversaires remontent sur l'entrée** (vivier trop
  maigre, deux pilotes sous la même identité) : le résumé passe en rouge et
  prend un marqueur.
- **L'entrée parente sert de retour** depuis `Adversaires` : cliquer `Course`
  ramène aux réglages sans changer de type. Le filet vertical qui rattache la
  sous-entrée passe au rouge quand elle est l'entrée retenue.
- **Changer de type ne détruit jamais rien** : le type décide de ce qui est
  affiché et envoyé au jeu, jamais de ce qui est mémorisé. Revenir à Course
  restitue tous ses réglages, plateau compris ; quitter Course pour un type
  sans plateau referme la page adversaires.
- **Une session se lance sans avoir ouvert la page adversaires** : un type
  Course ou Track day vierge remplit le plateau au hasard.
- Le type vit dans un état partagé (`sessionNav.svelte.ts`), que l'écran de
  réglages recopie ; seul un **chargement** (son montage, une session
  enregistrée) réécrit dans cet état.

### 1.2 Le repli `PERFORMANCE`

Lest, bride, ABS et antipatinage sous **une ligne unique** de la carte voiture.
ABS et antipatinage sont des **capacités de la voiture**, d'où leur place ; la
ligne idéale reste dans les règles de la course (SESSION§3).

**La ligne affiche toujours son état, repliée ou non** : *D'origine* en gris
italique quand rien n'est posé, sinon la liste des réglages non neutres en
rouge (`Lest 50 kg · Bride 10 %`, `ABS Off`). ABS et antipatinage ne comptent
comme posés que s'ils diffèrent de `Factory`. Le repli n'est pas mémorisé.

Règles communes aux champs de la colonne :

- **L'intitulé est une colonne, pas une ligne au-dessus** : largeur fixe à
  gauche, valeur alignée, ligne de 30 px. La largeur est partagée par les
  champs et **mesurée une fois par langue**, plafonnée à 88 px.
- **L'état vide est une valeur, pas un intitulé** : l'intitulé ne bouge jamais
  (`SKIN`), et le défaut s'écrit *Celui d'origine* en italique grise.
  **Partout où un défaut existe, il se dit de la même façon** — un possessif et
  une italique, jamais une négation.
- **La vignette d'un champ prend toute la hauteur de la ligne** (28 px) :
  livrée, tracé et pilote se reconnaissent d'un coup d'œil.
- **Un sélecteur à une seule option n'en est pas un** : le champ devient une
  ligne statique, sans bordure ni chevron, valeur en gris, avec la raison
  (`Imola — layout unique`). La ligne reste : c'est le contrôle qui disparaît,
  pas le fait.
- **Le chevron dit où l'on va** : `▾` s'ouvre sur place, `›` est une
  destination.
- **La source n'est pas un résumé** : `Porsche · 2016`, `Kunos` — jamais une
  information déjà affichée à quelques pixels.
- **Une liste ouverte n'est jamais rognée** par la colonne : elle grandit
  jusqu'à son plus long libellé, et le déclencheur montre le nom complet au
  survol et au focus.

**Chaque emplacement a trois états**, et aucun bouton « Changer » :

- **Choisi** : photo, nom, métadonnée. Au survol et au focus clavier, un voile
  plein porte « ✎ Changer de voiture / de circuit ».
- **Vide** : trame diagonale, « ＋ Choisir une voiture ». **L'app ne choisit
  jamais une voiture à la place de l'utilisateur** au premier démarrage. Tant
  que le duo est incomplet, « Démarrer la session » est désactivé, son fond
  rouge en opacité réduite.
- **Impasse** : Assetto Corsa introuvable ou chemin mal renseigné — « Aucune
  voiture détectée », et le clic ouvre **Réglages › Chemins**.

**Un clic ouvre la bibliothèque, deux ouvrent la fiche**, et les deux gestes
sont **comptés par l'écran**, jamais confiés à l'événement `dblclick` du
navigateur : le premier clic changerait d'écran entre les deux moitiés du
geste, et l'historique enregistrerait un écran que personne n'a regardé. Le
clic simple attend donc un quart de seconde. Un emplacement vide ou en impasse
n'a pas de fiche : le double-clic y vaut le simple.

**Un clic qui vise un menu de la carte** (livrée, layout, skins de circuit,
pilote) **n'éjecte jamais vers la bibliothèque** : la zone qui navigue est un
bouton frère de ces menus, jamais leur parent.

`Ouvrir Content Manager` vit au pied du rail (§7.2), pas dans cette colonne.

## 2. Pilotage par preset Quick Drive CM

L'app pilote CM par `acmanager://race/quick?presetFile=…` : un **preset Quick
Drive** (le format `SaveableData` de CM) est écrit dans un fichier jetable à
chaque lancement, puis passé à `Content Manager.exe`. C'est le chemin du
bouton « DRIVE » de CM, **condition pour que CM télécharge de lui-même ce qui
manque à CSP**. `race/config` a été abandonné pour cette raison
(`L4-cm-launch-research.md`).

- La durée d'une session Practice n'est pas transmise : Quick Drive n'a pas ce
  champ.
- **L'état de piste** part en quatre nombres, jamais en nom. L'écran offre
  « Auto » puis les six états du jeu (ceux de `cfg/templates/tracks.ini`, avec
  ses noms et dans son ordre). « Auto » est le drapeau `WeatherDefined` de CM :
  la météo décide, et les quatre valeurs de Verte partent quand même. `s`, `t`,
  `r` sont des pourcentages divisés par cent, `g` le `LAP_GAIN` brut. Un preset
  ancien qui porte une valeur disparue est recalé sur l'état le plus proche, le
  plus adhérent à égale distance.

### 2.1 Une course, deux modes CM selon la qualification

Une course **sans** qualification part dans le mode `Race` de CM, une course
**avec** dans le mode `Weekend` ; Pit Box n'a qu'un type « Course ». Les essais
libres n'existent que dans `Weekend`, ils suivent donc la qualification.
**`PracticeLength: 0`, jamais `null`** : CM lit `null` comme 15 minutes
d'essais.

L'URI porte `&loadAssists=true`, sans quoi CM ignore en silence les aides du
preset (dégâts, carburant, pneus, aides, chauffe-pneus) et garde celles de son
interface.

**Le skin du joueur est réécrit dans `race.ini` juste après CM.** Ni le
protocole ni le format de preset ne transportent le skin du joueur : CM
retombe sur le dernier qu'il a mémorisé pour la voiture. Un fil de surveillance
(`raceini.rs`) guette la réécriture de `race.ini` par CM, remplace `SKIN=` dans
les sections du joueur seulement (`[RACE]`, `[CAR_0]`, les adversaires gardant
le leur), et substitue le fichier par un renommage atomique, avant que le jeu
ne le lise. **Seulement si la voiture du fichier est celle demandée.** Trop
tard, le skin de CM reste ; chaque abandon est journalisé. Écrire `race.ini`
avant de lancer CM ne sert à rien : CM le réécrit.

### 2.2 Track day

Quatrième type, par le mode `Trackday` de CM : même grille d'adversaires que
Course (SESSION§3), tours, faux départ et pénalités ; jamais de qualification
ni d'essais libres. La limite de vitesse du mode vaut toujours 0.

### 2.3 Steam doit tourner avant le lancement

C'est Steam qui démarre Assetto Corsa : Steam éteint, la session ne démarre pas
et aucune erreur ne remonte. **Le lancement vérifie donc que `steam.exe` tourne**
avant de construire le preset. Absent, un dialogue demande de démarrer Steam et
revérifie à la validation, et reste ouvert tant que Steam manque. Une erreur de
la vérification elle-même laisse passer le lancement.

### 2.4 Une session à la fois

**Pit Box ne lance rien tant qu'Assetto Corsa tourne**, quel que soit celui qui
l'a démarré : le lancement réécrit des fichiers de la voiture du jeu, et une
seconde demande pendant une course n'est pas de notre ressort. Content Manager
ouvert sans jeu ne gêne rien.

| État | Bouton de la colonne | Bandeau de l'écran de réglages |
| --- | --- | --- |
| aucun jeu | `▶ Démarrer la session` | — |
| demande partie, jeu pas encore vu | `Lancement…`, désactivé | « Session envoyée à Content Manager — Assetto Corsa démarre… » |
| jeu lancé | `Session en cours`, désactivé, infobulle | « Session en cours… », jusqu'à la fermeture du jeu |

L'attente du jeu est bornée à **30 s** ; au-delà, le bouton revient et le
bandeau devient un avertissement. **Le backend refuse de son côté**
(`errors.gameRunning`) : la porte du bouton n'est qu'un confort. **Rejoindre un
serveur en ligne et lire un replay** suivent la même règle, bouton et refus
compris.

### 2.5 Une voiture qui emprunte ses modèles à une autre

Le `lods.ini` d'une voiture peut nommer des `.kn5` d'un autre dossier : une
variante qui ne livre que sa physique n'existe pas sans sa voiture de base.
`lods.rs` répond à « à qui cette voiture emprunte-t-elle ses modèles ? », pour
une voiture sans modèle à elle. Deux endroits posent la question :

- **Le lancement** : la prêteuse est posée dans le jeu si elle est en
  bibliothèque ; introuvable, la session est refusée **en la nommant**
  (`errors.carModelsMissing`). Un adversaire dans ce cas est retiré de la
  grille.
- **La suppression et la mise en vitrine** : la confirmation nomme les voitures
  qui empruntent les modèles de celle qui part (sauf celles qui partent dans le
  même lot).

Une erreur destinée à l'utilisateur peut donc porter des valeurs :
`errors.clé{"name":"…"}`, relu par `$lib/errorKey` avant traduction.

### 2.6 Le résultat d'une session

À la fin d'une session, Assetto Corsa écrit
`Documents\Assetto Corsa\out\race_out.json`. `raceresult.rs` le lit en un
classement par session jouée, et `read_session_result(since_ms)` le rend
**seulement s'il est plus récent que le lancement** : un fichier plus ancien est
celui d'une autre session. Aucun écran ne s'en sert encore ; Story Mode en sera
le premier. Pour chaque voiture : position, tours, meilleur tour valide, temps
cumulé, et abandon.

- **La voiture 0 est le joueur.**
- **L'ordre d'une course est celui du jeu** ; hors course, c'est le meilleur
  tour, les voitures sans tour valide après, dans l'ordre d'engagement.
- **Le meilleur tour est celui du jeu**, qui écarte un tour coupé.
- **L'abandon se déduit** : un pilote qui finit passe la ligne après le
  vainqueur, donc son temps cumulé est au moins celui du vainqueur ; un temps
  plus court veut dire arrêté avant le drapeau. Une voiture sans aucun tour n'a
  pas fini.

## 3. Écran de réglages

Pas de rappel du duo en haut. Le titre **suit la navigation** (`Course`, puis
`Course · Adversaires`) ; `Enregistrer` et `Charger` sont à sa droite. **Fond
photo** derrière l'interface, dans l'ordre de repli du §6.2. Tant que le
chargement initial n'est pas fini, le corps de l'écran reste derrière un
indicateur de chargement : les champs ne se réajustent pas sous les yeux.

**Un réglage se range selon sa PORTÉE, jamais selon sa fréquence d'usage** —
le critère de fréquence envoie tout ce qui est rare au même endroit, qui
devient un fourre-tout.

| Zone | Objet | Contenu |
| --- | --- | --- |
| Panneau gauche | la voiture et la piste | livrée, pilote, lest, bride, ABS, antipatinage |
| Colonne centrale | la session et ses règles | durées, dégâts, carburant, usure, pénalités, ligne idéale, départ |
| Rail droit | les conditions | météo, températures, vent, état de piste, heure, saison |
| Page dédiée | les adversaires | filtres, générateur, plateau |

**Communs aux quatre types** : « Simulation » (dégâts, consommation, usure, puis
chauffe-pneus, pénalités et ligne idéale) et les conditions. **Options de
session** ne porte que ce qui dépend du type : départ en Practice ; ghost car
et son avance en Hotlap ; tours, qualification, essais libres, faux départ et
position de départ en Course ; faux départ en Track day. Ordre invariant :
Options de session, puis Simulation. Seul bandeau possible en tête :
« ce circuit n'est pas catégorisé comme circuit fermé », posé au niveau de la
page parce que c'est le couple type + circuit qui est en cause.

**Les durées : trois champs du même gabarit, et 0 désactive sa séance.**

```
TOURS  [  5 ]      QUALIFICATION  [ 10 ] min      ESSAIS LIBRES  [ 20 ] min
```

Un `0` s'affiche éteint. Les booléens de mode se **déduisent** des durées. Le
champ des essais libres est éteint tant que la qualification vaut 0, et dit
pourquoi.

- **Un bloc prend la largeur, ses contrôles gardent leur gabarit**, calés à
  gauche.
- **Le bloc d'options a la même hauteur dans les quatre types**, pour que
  Simulation ne saute pas quand on change de type.
- **Un champ sans objet dans le type courant est retiré, jamais grisé.** Le
  grisé est réservé aux **dépendances internes** (la qualification éteint les
  essais libres et la position de départ). Dans les deux cas la valeur est
  conservée.
- **Position de départ** (Course) : Au hasard (défaut), 1er, 2e, Dernier, après
  le faux départ ; le rang se résout à la construction du preset (« Dernier » =
  adversaires + 1). **Écart assumé avec CM** : une qualification non nulle
  l'**éteint** au lieu de la retirer — chez CM les deux vivent dans deux vues,
  ici à côté, et un réglage qui disparaît quand on coche une case voisine se
  lit comme un bug.
- **Les aides au pilotage ont trois états** : `Off` / `Factory` / `On`, le
  vocabulaire et les valeurs du jeu (0, 1, 2). **`Factory` garde l'équipement
  réel de la voiture, donc `On` ne peut rien ajouter à une voiture qui n'en a
  pas** — ce que disent les infobulles. Elles vivent dans le repli
  `PERFORMANCE` (SESSION§1.2). Un réglage enregistré en booléen se relit
  `true → factory`, `false → off`, absent → `factory` : l'ancien booléen
  envoyait déjà `1`. La ligne idéale reste une case.
- **Ce que `Factory` vaut pour la voiture** se lit sous les deux segmentés :
  « *Factory — la Ford Mustang Mach 1 428 n'a ni ABS ni antipatinage.* » La
  présence vient de la **fiche technique** (FICHE§5 — `[ABS]` et
  `[TRACTION_CONTROL]` d'`electronics.ini`, lus section par section). La phrase
  nomme *quelle* aide la voiture possède ; une voiture qui ne déclare ni l'une
  ni l'autre section n'affiche **aucune ligne**, jamais un « inconnu ».
- **Course et Track day** ont une page **Adversaires** (SESSION§3.1) ; faux
  départ et pénalités leur sont communs ; les **tours** ne valent qu'en Course
  (une session Track day ne se termine jamais sur un décompte de tours).
- **Practice** : départ Stand / Piste / Position de chrono, sans durée ni tours.
- **Hotlap** : ghost car et son **avance**, sur une seule ligne.

**L'écran est responsive, et centré.** La colonne de droite passe dessous quand
la largeur manque (sous ~380 px elle ne tient plus), le contenu est plafonné
et centré, et le seuil est une requête de **conteneur**.

### 3.1 Le vivier, le plateau, et les deux gestes entre les deux

**Le filtre définit le vivier, jamais le plateau.** Le bloc Adversaires porte
la barre de filtres de la bibliothèque et un compteur `Vivier · N voitures`, ce
qui permet de **combiner** (`#gt3` et 2010-2016 et sauf Kunos).

**Trois puces de raccourci** — Même voiture, Même catégorie, Même performance —
posent **un jeton et rien d'autre**. Une puce paraît active quand son jeton est
là, même posé à la main ; la recliquer le retire. Une puce sans référence (pas
de voiture, pas de catégorie, specs illisibles) est éteinte et **dit
pourquoi** (`aria-disabled`, pour que l'infobulle reste).

- `Modèle` et `Catégorie` posent une **valeur** : un instantané, qui ne suit
  pas un changement de voiture.
- `Performance` pose une **tolérance** sur le rapport poids / puissance
  (kg/bhp) **autour de la voiture pilotée**, qui suit la voiture. Le jeton existe
  aussi dans la bibliothèque. Lecture **conservatrice** des specs : une
  puissance aux roues (`whp`), en chevaux métriques ou absente rend la voiture
  illisible — elle **sort du vivier** et affiche `—`, jamais une valeur estimée.

**Entre le vivier et le plateau il y a exactement deux gestes** : `Tirer N au
hasard` **remplace** le plateau ; `Choisir dans le vivier · N voitures` ouvre la
modale sur **ce même filtre** et **ajoute** en fin de plateau. La modale partage
l'état de filtre du bloc. Un composant, deux modes : « Ajouter » (cases,
`Maj+clic` pour étendre, ajout dans l'ordre affiché) et « Remplacer » (sélection
simple, ouverte sur la voiture de la ligne, double-clic pour valider, la force
de la ligne conservée, le skin retiré au sort dans la nouvelle voiture).

- **Aucun repli quand le vivier est vide** : `Vivier · 0`, les deux boutons
  éteints. Un vivier plus maigre que le nombre demandé se signale sans
  bloquer — le plateau répète des voitures avec d'autres livrées.
- **Le jeton « jouable » n'est pas épinglé** : la garde d'activation (§7.4) le
  dit au-dessus du bouton de lancement et répare d'un clic.
- **Le plateau ne change que sur un geste**, jamais tout seul au changement de
  voiture pilotée.
- **Deux pilotes ne portent jamais la même identité par accident** : le jeu
  nomme une IA d'après le `ui_skin.json` de sa livrée, donc le tirage évite les
  livrées dont le couple numéro + nom est déjà pris **sur tout le plateau**. Un
  doublon forcé à la main est signalé, jamais corrigé dans le dos.

### 3.2 Le plateau

**Chaque cellule éditable vaut `Auto` ou une valeur explicite.** `Auto` est
l'absence de surcharge — `-1` dans un tableau numérique du format de CM, `null`
dans un tableau de texte. Une valeur posée à la main n'est jamais écrasée par
un tirage, et **vider le champ la rend à `Auto`**.

- Une **force** en `Auto` est tirée **par le jeu** dans la fourchette : l'écran
  affiche `Auto`, pas un nombre. Un **nom** ou une **nationalité** en `Auto`
  affiche ce que la livrée déclare, ce que le jeu emploiera.
- **Une force explicite remplace, elle ne multiplie pas** : une ligne à 95 dans
  un plateau réglé 84-90 est légitime et n'est pas signalée.
- **La fourchette part dans le preset** : `AiLevel` est le haut, `AiLevelMin` le
  bas.
- **Une agressivité par pilote existe, sans colonne à l'écran** (0-100, `Auto`
  par défaut), envoyée dans `AiAggressions` comme la force dans `AiLevels` ;
  elle sert aux grilles construites par le code (Story Mode). Une grille
  enregistrée avant elle se relit en `Auto` partout.
- **La vignette ouvre le choix de livrée** (par leur `livery.png`, la photo en
  repli) ; le reste de la ligne ouvre le choix de voiture.
- **Le survol d'une ligne montre ce qui est tronqué** — nom complet de la
  voiture et de la livrée —, jamais une photo qui recouvrirait les voisines.
- **La nationalité n'est pas un champ libre** : la liste vient du jeu
  (`$.Nationalities` de son launcher, drapeaux de `content/gui/NationFlags/`),
  lue dans l'installation et non recopiée. La cellule montre le **drapeau et le
  nom du pays** ; le menu est maison, avec un champ de recherche n'importe où
  dans le nom. **Ce qui est stocké est le nom entier**, jamais le code. La table
  du jeu porte « Congo » deux fois : elle est dédoublonnée par nom, le premier
  code gagne. Installation illisible : saisie libre.
- **Toutes les colonnes, tout le temps, sans abréviation** (`Nationalité`,
  `Force`, `Restrictor`) : il n'y a pas de menu de colonnes. Le nom est plafonné,
  la place restante va au vide en fin de ligne.
- Le lest et la bride n'ont pas d'`Auto` : « rien » s'y dit par 0. Les en-têtes
  distinguent colonnes éditables et en lecture seule par les deux gris de l'app,
  dans une seule police pour toute la rangée.
- **La table est plafonnée à une dizaine de lignes**, défilement interne,
  en-tête figé — le seul défilement imbriqué autorisé de l'application.

**La page** : le bandeau (vivier à gauche, tirage à droite, séparés d'un filet
qui disparaît quand les deux s'empilent ; le décompte du vivier dans la coulée
des puces), les bannières d'alerte, puis le plateau (`PLATEAU · N IA`), en un
seul enchaînement pleine largeur. Dégâts, carburant, usure, pénalités et ligne
idéale restent sur la page de réglages.

**Difficulté et agressivité : un centre et un écart**, un même composant deux
fois. Déplacer le centre conserve l'écart ; l'écart est un champ (`± 0` se pose
exactement). Le libellé affiche la plage **bornée** (`3% ± 5 (0–8)`), et le
bornage vaut aussi pour ce qui part en jeu. Les réglages d'avant ce modèle
(minimum et maximum) sont **convertis**, jamais repliés sur le défaut.

**Lest et bride du joueur** : dans le repli `PERFORMANCE` (SESSION§1.2). Zéro
éteint, toute autre valeur en rouge. Ils partent dans le preset des quatre
types, au niveau de la grille pour une course, dans le `ModeData` pour les
modes solo.

### 3.3 Les conditions — un bloc unique du rail droit

**Le rail droit porte les conditions, et rien d'autre**, identiques dans les
quatre types : c'est lui qui donne à l'écran sa silhouette constante. Rien n'y
est centré verticalement, aucun espace réservé.

**Un seul bloc `CONDITIONS`**, dans cet ordre : les tuiles météo, la rangée air
/ piste / vent, l'état de piste, l'heure, la saison. État de piste et météo ne
font qu'un — la première entrée de l'état de piste est « Auto (posé par la
météo) ».

- **Météo** : sept conditions en icônes (Beau, Quelques nuages, Couvert,
  Brouillard, Pluie légère, Pluie, Orage). Température, vent et heure sont
  proposés par condition + heure + stack météo, tous corrigeables.
- **L'état de piste** : un select, et **ses quatre vraies valeurs sous lui**,
  en lecture seule, avec le vocabulaire de CM (`INITIAL GRIP`, `GRIP TRANSFER`,
  `RANDOMIZATION`, `LAP GAIN`). Un état est un objet nommé porteur de quatre
  valeurs, que l'écran reçoit. « Auto » a les valeurs de Green, et **une seule
  phrase dit son repli**. Ce qui identifie « Auto » est le drapeau
  `weather_defined`, jamais un grip à 0.
- **Les presets d'état de piste de CM** (`…\Presets\Track States`) s'ajoutent
  **après** les sept entrées du jeu, par ordre alphabétique ; le groupe est
  absent quand il n'y en a aucun. Lecture seule — Pit Box n'écrit rien chez CM —,
  lue **à l'ouverture de l'écran**. Leur **description** s'affiche sous les
  quatre valeurs, comme celle des natifs. Les pourcentages **s'arrondissent**
  (une troncature rendrait 28 pour `0.29`). Bornage large, au sens physique (un
  pourcentage entre 0 et 100, un lap gain d'au moins 1). Un fichier illisible
  est ignoré en silence. L'identité d'un état est son **nom de fichier**, comme
  chez CM.
- **Heure** : la journée entière, au pas de **10 minutes**, dernier cran à
  23:50. **La bande jour/nuit est sous le curseur**, poignée alignée, et
  **datée** (la date effective écrite au-dessus, jamais seulement en
  infobulle) : nuit, crépuscule civil, plein jour, avec deux repères cliquables
  à l'heure exacte du lever et du coucher.
- **La bande ne s'invente pas** : ce sont les entrées que CSP lit (`sun.rs`).
  Position et fuseau viennent de `extension/config/data_track_params.ini`, puis
  de sa copie chez Content Manager, puis des `geotags` du `ui_track.json`
  (toutes les formes rencontrées ; ceux des circuits Kunos, littéralement
  `["lat", "lon"]`, sont ignorés), le fuseau étant alors approché d'après la
  longitude et le survol le disant. **Sans position, pas de bande.** L'heure se
  lit sur l'horloge locale du circuit, heure d'été comprise.
- **La date que suit le soleil** : `[SEASONS] ALLOW_ADJUSTMENTS` de
  `track_adjustments.ini`. Seul `0` détache le soleil du calendrier ; sinon il
  suit la date que porte la session, et **la date du jour** à défaut (mesuré en
  jeu). Le script météo n'y change rien.
- **Saison** optionnelle : un champ date (en premier) puis quatre cartes ;
  choisir une saison reporte la date du milieu de saison, corriger la date ne
  désélectionne pas la saison.

### 3.4 Ce qu'une session retient d'un état de piste

**L'origine et le nom, plus les quatre valeurs résolues.** Ce qui part au jeu,
ce sont les nombres :

- un preset supprimé ou renommé dans CM ne modifie **jamais en silence** une
  session enregistrée ;
- **référence introuvable** : les quatre valeurs mémorisées s'appliquent, et le
  select garde l'entrée sous son nom suivi de `(introuvable)` ;
- **référence trouvée, valeurs différentes** : celles du preset gagnent.

Un réglage d'avant ce modèle ne porte qu'un pourcentage de départ : l'état
natif le plus proche est repris.

### 3.5 Grilles enregistrées et import Content Manager

**Une grille et une session sont deux objets.** Une session enregistre tout,
dont une **copie** de sa grille ; une grille n'enregistre que les adversaires
et le caractère du plateau (fourchette, agressivité), donc se rejoue ailleurs.
La charger ne touche ni à la météo, ni à l'heure, ni au type. **Une copie,
jamais un lien.** Liste plate avec recherche ; fichier JSON dédié, pas rangé par
type.

**Import des presets Content Manager, jamais synchronisation** : on lit une
fois et on convertit. Deux dossiers sont parcourus récursivement : `Race
Grids\` et `Quick Drive\` (dont la grille s'importe aussi). **Seul `ModeId:
"manual"` devient une grille** : les autres modes de CM décrivent une règle de
tirage, et ils sont nommés dans le rapport. **L'import aboutit toujours** : un
fichier corrompu est nommé, une voiture absente retirée en le disant, une
voiture désactivée gardée (la garde d'activation la rallume). La proposition
d'import et le bouton permanent vivent sur la page d'import de mods ; un refus
est définitif. Le format mélange nombres en chaînes et en flottants : le
lecteur accepte les deux.

**Presets de session par type** : chaque type a son preset mémorisé, et toute
modification vaut pour les prochaines sessions du même type, dans
`launch_state.json` (écrit côté Rust, distinct de `session.json`). **Une
écriture ratée ne passe pas inaperçue** (règle d'or n°6) : réessayée une fois
avec l'état le plus récent, journalisée, puis signalée par la notification
« Vos réglages ne sont pas enregistrés ».

**Sessions enregistrées : deux boutons dans la barre de titre, et une modale.**
Grilles et sessions partagent la même modale. **L'action se place au niveau de
ce qu'elle enregistre** : `Enregistrer la grille…` en bas de la grille,
`Enregistrer la session…` en en-tête d'écran. La liste n'est **pas filtrée par
type** — un filtre invisible fait croire qu'une sauvegarde a échoué — mais
**triée** type courant en tête ; charger une session bascule le type.

**Contenu d'une session enregistrée** : les réglages **et le duo** — voiture et
skin, circuit, tracé et **skins de circuit actifs** (`trackSkins`). Le
chargement passe par le duo de session (`pickSession`), et remet les skins de
circuit **à l'identique** (ceux qui manquent activés, ceux en trop désactivés) ;
une sauvegarde sans le champ n'y touche pas.

**Chargement partiel** : un mod supprimé n'interrompt jamais le chargement — la
sélection courante est gardée pour ce qui manque, le tracé retombe sur le
défaut, et un **bandeau jaune** en tête d'écran énumère ce qui n'a pas pu être
rétabli, jusqu'au chargement suivant ou au lancement.

### 3.6 Une session enregistrée est un preset Content Manager

**Le format de sauvegarde est celui de CM**, `.cmpreset`, dans son dossier
(`%LocalAppData%\AcTools Content Manager\Presets\Quick Drive\Pit Box\`) : une
session réglée dans Pit Box se lance aussi depuis CM, et les presets composés
dans CM apparaissent dans Pit Box. **Un nom, un fichier** : deux types ne
peuvent plus avoir une sauvegarde du même nom, et la modale demande avant
d'écraser.

- Le fichier vient **du constructeur du lancement lui-même**
  (`quickdrive::preset_value`) : ce qui est enregistré et ce qui est lancé ne
  peuvent pas diverger.
- **Le bloc `PitBox`** : nos fichiers portent une clé de premier niveau avec
  l'instantané complet (skin du joueur, intention météo, skins de circuit,
  jetons du vivier, tenue du pilote — que le format de CM ne porte pas), et
  c'est **elle seule** qui est relue. CM l'ignore ; s'il la perdait, le fichier
  se relirait comme un preset de CM.
- **Les presets de CM sont listés, avec un badge**, et se chargent au mieux. Ce
  que le format ne porte pas est **dit** dans le bandeau du chargement partiel :
  le skin du joueur (celui en place est gardé), le plateau quand le preset
  **tire** ses adversaires, la température de piste.
- **Drag, Drift, Time attack** sont nommés en ligne grisée avec leur raison, et
  ne comptent pas dans le décompte du bouton.
- **Un preset qu'on n'a pas écrit ne se supprime pas** : la croix n'est que sur
  les nôtres, et le backend refuse un fichier sans bloc `PitBox`.
- **Sans Content Manager**, les presets vont dans
  `app_config_dir/Quick Drive/Pit Box/`, même format.
- **Migration** : `saved_sessions.json` est relu une fois, chaque entrée réécrite
  en preset, puis le fichier renommé ; une entrée illisible est journalisée et
  sautée.

## 4. Aperçu 3D des voitures

Deux aperçus **coexistent**, parce qu'ils ne rendent pas le même service.

**L'aperçu intégré** remplace la photo dans la **zone héros** de la fiche
voiture. Ce qu'il fait — plateau tournant, reflet au sol, greffes CSP, pilote,
braquage, cache, états de chargement et de repli — est dans
`SPEC-preview-3d-kn5.md`. Côté fiche :

- **Actif par défaut**, avec une bascule discrète en bas à droite du héros pour
  revenir à la photo ; ce choix est mémorisé.
- **Le skin sélectionné sur la fiche est appliqué au modèle**, sans repasser par
  la photo.
- **Une couche activée ou retirée rafraîchit tout** — modèle, liste des skins et
  vignettes — et le skin regardé se retrouve **par son identité**, pas par son
  rang.
- Le pilote affiché au volant se choisit sur son écran (SESSION§5).

**L'aperçu natif** (bouton de la fiche) lance `acshowroom`, pour le rendu fidèle
du jeu : un **processus indépendant**, par-dessus l'app, que l'utilisateur ferme
lui-même. Une animation de chargement en haut à droite de l'image accompagne son
démarrage. **Réglage** : le décor (`content/showroom/<id>`), parmi ceux
installés — défaut `studio_white`, le seul instantané et sans musique. Intégrer
sa fenêtre dans la page a été abandonné (`showroom-3d-preview-research.md`).

---

## 5. Écran Pilote

Choisir le pilote qu'on voit au volant : son **corps** (le mannequin 3D) et sa **tenue** en trois pièces — casque, combinaison, gants —, **par voiture**, avec deux tenues par défaut (rue, course), et le poser en jeu au lancement de la session. L'écran est décrit en entier dans `SPEC-ecran-pilote.md` (`PILOTE§`) ; on y entre par la ligne `PILOTE` de la colonne de session (SESSION§1).
