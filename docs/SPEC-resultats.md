# Résultats de session

Ce que Pit Box retient des sessions roulées, et ce qu'il montre quand le jeu se
ferme. Étiquette des renvois : `RESULTAT§`. La lecture du fichier de résultat
du jeu elle-même est au SESSION§2.6.

## 1. Le principe

- **Un journal des lancements** dit qui a lancé quoi ; c'est lui, et lui seul,
  qui reconnaît les copies que Content Manager garde de chaque session.
- **Les sessions de l'utilisateur sont enregistrées**, quel que soit le
  programme qui les a lancées, une fois chacune.
- **Le lien d'une session à son replay et à ses screenshots se calcule**, il
  n'est jamais stocké : une session a un début et une fin, un fichier a une date
  et un combo.

## 2. Le journal des lancements

Chaque jeu que Pit Box démarre est noté **au départ**, avec son heure et sa
nature : `session` (l'écran de session), `gameshot` (les vignettes,
GRILLE§7.1), `story`, `online`, `replay`. Une session y garde **le réglage tel
qu'il est parti et le preset avec son bloc `PitBox`**, celui que
`quickdrive::preset_value` construit (SESSION§3.6) : c'est ce qui permet de la
relancer. Un serveur online y est noté sans son mot de passe.

- Le support est une table d'`overlay.sqlite` (`launch_log`), créée par
  `CREATE TABLE IF NOT EXISTS` : le journal se lit avec les sessions qu'il
  rattache, et il est écrit sous le verrou que le lancement tient déjà. Ce
  n'est pas un réglage d'écran : la règle d'or n°6 ne le concerne pas.
- **Une fermeture du jeu termine tous les lancements ouverts** — un seul jeu à
  la fois (SESSION§2.4). Au démarrage, un lancement resté ouvert (Pit Box fermé
  pendant la partie) est terminé sans écran.
- **Un lancement n'est celui d'un jeu que s'il l'a précédé de trois minutes au
  plus.** Un lancement dont le jeu n'est jamais venu (CM l'a refusé) ne
  s'approprie pas la session que l'utilisateur lance ensuite depuis CM : il est
  terminé, sans rien enregistrer.
- Une écriture ratée du journal n'empêche pas la session, déjà partie : elle
  est journalisée.

## 3. Les sessions enregistrées

### 3.1 Un lancement de Pit Box

Quand le jeu se ferme après une session `session` ou `online`,
`race_out.json` est lu (seulement s'il est plus récent que le lancement) et la
session est enregistrée avec son lancement — si le joueur y est bien dans la
voiture envoyée ; sinon ce n'est pas la session lancée, et c'est la copie de CM
qui l'enregistrera comme une session de CM.

### 3.2 Les copies de Content Manager

CM garde une copie de chaque session dans
`%LOCALAPPDATA%\AcTools Content Manager\Progress\Sessions\<aammjj-hhmmss>.json`.
Ce dossier est **lu, jamais écrit** (hors §5), hors du fil de la fenêtre, au
démarrage puis 5 s et 25 s après chaque fermeture du jeu. **Une copie déjà lue
ne l'est plus jamais** (`cm_session_files`, avec ce qu'elle est devenue).

Constaté sur les 105 copies de la machine de développement (août à novembre
2026) :

- **La copie est le fichier du jeu, plus `__raceIni` et `__quickDrive`.** Son
  nom est l'heure où CM l'a écrite, une seconde environ après le jeu.
- **`__quickDrive` est la réécriture du preset par CM** : ses clés sont celles
  de CM, et **le bloc `PitBox` du preset passé par Pit Box n'y est plus**. Rien
  dans une copie ne dit qui a lancé la session : le journal est la seule
  reconnaissance, et marquer les presets de prise de vue n'apporterait rien.
- **Une session online n'a pas de `__quickDrive`** ; son `race.ini` porte
  `[REMOTE] ACTIVE=1`. La voiture 0 reste le joueur.
- **Le circuit et son tracé se lisent dans `race.ini`** (`[RACE] TRACK`,
  `CONFIG_TRACK`) : le champ `track` du fichier les joint d'un tiret, qu'un id
  de circuit peut aussi contenir.

### 3.3 Sans doublon

Pit Box lance par CM, donc CM a aussi une copie de chaque session de Pit Box.
**Une copie appartient à un lancement** quand elle a été écrite entre son
départ et deux minutes après la fermeture du jeu (un lancement jamais vu se
fermer s'arrête au suivant, au plus douze heures), sur le même circuit, le
joueur dans la même voiture — la voiture ne compte pas pour Story Mode, qui
peut faire rouler sa propre version de celle du joueur. Elle est alors **rattachée** à la session déjà
enregistrée, ou l'enregistre avec son lancement si le fichier du jeu n'a pas
été lu. Sinon, c'est une session lancée ailleurs, enregistrée avec le preset
de CM.

### 3.4 Ce qui n'est jamais enregistré

- les prises de vue des vignettes ;
- les sessions de Story Mode : elles peuvent faire rouler une autre version de
  la voiture du joueur (L4.5b de Story Mode), ce qui fausserait les records ;
- une session où **aucune voiture n'a commencé un tour** (aucun tour noté,
  aucun tour compté).

Les sessions de Story Mode et de vignettes d'avant le journal ne se
reconnaissent pas : une copie de vignettes n'a aucun tour, donc reste dehors ;
une session de Story Mode d'avant entre comme une session de CM.

### 3.5 Les données

`session_results` dans `overlay.sqlite`. Comme `usage`, elle tient aux mods par
leur id seulement et **survit à la suppression d'un mod** (§10 de SPEC.md).
Elle ne part pas encore dans un export de bibliothèque (EXPORT§4.1) : la
décision est au §7.

## 4. Le retour du jeu

### 4.1 Pas de popup

Quand le jeu se ferme après une session `session` ou `online`, **Pit Box revient
au premier plan** sur l'écran de résultat, dans la zone Session : il prend la
place de la bibliothèque, la colonne de session reste. Ce n'est pas une
modale : naviguer ailleurs le ferme, et il ne se rouvre jamais au démarrage. Si
Windows refuse le premier plan, le bouton de la barre des tâches clignote.

### 4.2 Ce qu'il montre

- **Course** (la dernière session est une course) : le classement, la ligne du
  joueur en évidence, l'écart au vainqueur (en tours pour un retardataire), les
  abandons.
- **Un tour valide** (essais, hotlap, journée piste, online) : le meilleur
  tour, l'écart au record perso sur ce couple voiture et tracé, la liste des
  tours, les coupés grisés et marqués. **La pastille « record perso » ne vient
  que si un record existait et qu'il est battu** : la première session sur un
  couple n'en a pas.
- **Aucun tour valide** : une ligne, le type et la durée (ou le nombre de
  tours).

**Le record perso** est le meilleur tour valide du joueur sur la voiture et le
tracé, **toutes sessions enregistrées confondues** : Pit Box, Content Manager,
online.

### 4.3 Les actions

- **Again**, l'action principale : la même session. Une session de l'écran de
  session repart avec le réglage enregistré au lancement ; une session online
  rejoint le même serveur avec la même demande (mot de passe compris, gardé en
  mémoire seulement : après un redémarrage de Pit Box, plus de Again online).
- **Change car** : le circuit reste, la bibliothèque des voitures s'ouvre.
- **Watch replay** et **Keep** : §6.1. Une bande de screenshots : §6.2.

### 4.4 Big Picture

Même écran ; Again a le focus, donc A le déclenche, et B revient en arrière
(`goBackOr`, la zone Session à défaut).

### 4.5 Online

Le jeu écrit bien `race_out.json` après une session online : **constaté sur les
copies de CM**, qui en sont la recopie (trois sessions online, la voiture 0 y
est le joueur). Même écran. Pas vu en direct sur un retour de Pit Box.

### 4.6 La fenêtre de résultats de Content Manager

Elle suit toujours le réglage « Skip results » de CM (SESSION§2.7). **Aucun
lancement ne peut demander de la passer** : `race/quick` ne lit que `preset`,
`assists`, `loadPreset` et `loadAssists` (code de CM, `ArgumentsHandler.Race.cs`),
et le format du preset n'a pas de clé pour cela. Le seul moyen serait d'écrire
les réglages de CM, ce que l'esprit de la règle d'or n°4 exclut.

## 5. Les sessions de vignettes dans les résultats de CM

Ces copies sont des données de CM, pas des fichiers du jeu : les règles d'or
n°2 et n°5 ne s'appliquent pas telles quelles. **Leur esprit vaut** : on ne
retire que ce que Pit Box a provoqué, ou ce que l'utilisateur a confirmé, et
toujours **à la corbeille**, jamais en suppression définitive. Chaque retrait
est journalisé.

### 5.1 À l'avenir

À la fin d'une session de vignettes, une fois CM relâché, **la copie qu'il
vient d'en écrire part à la corbeille** : écrite pendant la session, à
Magione, le joueur dans la première voiture du lot, autant de voitures que le
lot et toutes du lot. Pas exactement les mêmes voitures : CM a déjà été vu en
oublier et en doubler d'autres. La copie est attendue dix secondes : rien ne
retient le jeu fermé quand CM n'est pas celui de Pit Box (SESSION§2.7).
**Aucune copie, ou plusieurs, et rien n'est touché** ; le journal le dit.

### 5.2 Le passé

`Fichiers › Maintenance` liste les copies qui **ressemblent** à une prise de
vue, avec leur date et leurs voitures, et les met à la corbeille après
confirmation, chacune revérifiée au moment de partir. Ressembler : Magione sans
tracé, un preset Quick Drive d'essais ou de journée piste, douze voitures au
plus, aucun tour commencé. Une session de l'utilisateur dans ce cas (un essai à
Magione sans un tour) serait proposée aussi ; elle est de toute façon hors de
l'historique (§3.4).

## 6. Le replay et les screenshots d'une session

### 6.1 Le replay

**Constaté** : les autosaves du jeu sont dans `replay\temp\`
(`AC_<jjmmaa>-<hhmmss>_<type>_…`, l'heure étant la fin de la session concernée
du week-end), pas dans `replay\`. Le replay d'une session est **le plus récent
du combo daté entre son début et deux minutes après sa fin**, cherché dans les
deux dossiers ; dans un week-end, celui de la course. L'en-tête du fichier donne
le combo, le nom à défaut.

- **Watch replay** passe par `launch_replay`, avec la même garde (SESSION§2.4).
- **Keep** le sort de la rotation des autosaves en le déplaçant dans `replay\`
  sous le nom que CM donne à un replay gardé (`<voiture>_<circuit>_<date>`,
  celui que l'onglet Médias de la fiche reconnaît). Un nom déjà pris est refusé, jamais écrasé.
- L'écran le cherche à son ouverture puis six secondes après, comme l'onglet
  Médias de la fiche. Pas là : pas de bouton, et rien n'est dit. Un
  autosave écrasé depuis : la session n'a plus de replay.

### 6.2 Les screenshots

Ceux du combo (voiture et circuit dans le nom) datés entre le début et deux
minutes après la fin de la session, dans l'ordre de prise. Un clic ouvre la
visionneuse (`Lightbox`).

## 7. À venir (lot 2)

Le record perso à côté du bouton Start, le bloc « My times » des fiches,
l'onglet History de la zone Session (avec Edit, qui charge le preset enregistré
par le chemin du SESSION§3.5), le résultat d'une session sur sa ligne de replay.
L'export de bibliothèque devra alors emporter `session_results` (EXPORT§4.1),
sans le journal ni les noms de copies de CM, qui sont l'état de la machine.
