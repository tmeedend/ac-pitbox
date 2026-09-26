# Lancement de session

Comment on choisit ce qu'on va piloter, comment on règle la session, et comment
elle part dans le jeu. Extrait du §9 de `SPEC.md`, qui en portait 38 % à lui
seul et dont la numérotation avait dégénéré en `bis`/`ter`/`quater` — un
symptôme, pas un tic : renuméroter cassait en silence les renvois du code, donc
personne ne renumérotait.

**Le code y renvoie par l'étiquette `SESSION§`** (voir `docs/README.md`), et
`npm run check` vérifie que chaque renvoi tombe sur une section qui existe.
Renuméroter ici casse donc le contrôle — c'est voulu.

Dans ce fichier aussi, **un `§` nu désigne `SPEC.md`** : un renvoi d'une
section d'ici vers une autre porte donc `SESSION§`, comme dans le code. Sans
quoi « voir §3.2 » et « voir §4.5.3 » ne parleraient pas du même document sans
que rien ne le dise.

## 1. La bibliothèque est le sélecteur

Pas d'écran séparé de sélection : la voiture/le circuit sélectionnés dans la bibliothèque sont ceux de la session. La **colonne de session** montre en permanence le duo courant. La page « Démarrer une session » ne contient aucune sélection de voiture/circuit — seulement les réglages + Lancer.

**La colonne répond à une seule question : *qu'est-ce que je lance ?*** Tout ce qui n'y répond pas en est sorti — les boutons de navigation dans le rail (§7.2), les trois menus de tenue dans l'écran Pilote (SESSION§5). Restent **le type de session, qui est la navigation** (SESSION§1.1), **deux blocs à l'anatomie strictement identique** — vignette, nom, source, puis les champs — et deux actions :

```
CIRCUIT
  [ vignette ]                     ← cliquable : ouvre la bibliothèque
  Imola                            ← cliquable, même zone
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
    Adversaires  6 IA · 87 % ± 3   ← sous-entrée, indentée, filet d'attache
  Track day
  [ ▶ DÉMARRER LA SESSION      ]
```

**L'ordre suit celui de la décision** : le circuit, puis la voiture qu'on y
emmène, puis le genre de séance qu'on y fait — et le bouton de lancement tombe
juste sous la liste des types, qui est le dernier choix avant de partir.
`Ouvrir Content Manager` a quitté cette colonne pour le pied du rail (§7.2).

**La colonne ne doit jamais défiler**, et le cas à tenir est le plus chargé :
Course sélectionnée, sous-entrée affichée, en 1920 × 1080. Deux dispositifs y
suffisent — le repli `PERFORMANCE`, qui ramène quatre réglages à une ligne, et
des **vignettes adaptatives** : sous le seuil de hauteur, la photo de voiture et
le plan de circuit tombent à 68 px. Le seuil est une requête de **conteneur**
(`container: sidecol / size`, 1020 px) et non de média : une requête de média
interroge la fenêtre, que le zoom d'interface ne touche pas — à 150 %, une
fenêtre de 1080 px n'offre plus que 720 px de mise en page et la règle ne se
déclencherait pas. Le nombre est une mesure, pas une valeur ronde : la colonne
la plus chargée fait un millier de pixels. **La liste des types ne se replie
jamais** — c'est la seule chose de cette colonne qui ne soit pas négociable.

### 1.1 Le type de session EST la navigation

Les quatre types — Essais, Hotlap, Course, Track day — forment une liste en tête
de la colonne, **toujours dépliée** : ils annoncent ce que l'application sait
faire. Cliquer un type le sélectionne **et** ouvre ses réglages ; c'est ce
double geste qui a remplacé le bouton `PARAMÉTRAGE DE LA SESSION` et le
segmenté `TYPE DE SESSION` de l'écran de réglages, qui disaient la même chose à
deux endroits sans qu'aucun ne dise ce que l'app savait faire.

- **Deux marques, et elles ne disent pas la même chose.** Le libellé en pleine
  lumière = le type qui partira, visible depuis n'importe quel écran ; le filet
  rouge d'attaque = on le regarde, donc seulement sur l'écran de session. Sans
  la première, la sous-entrée `Adversaires` pendait sous quatre lignes
  identiques dès qu'on quittait l'écran, sans qu'on voie à laquelle elle
  appartenait.
- **La sous-entrée `Adversaires`** ne paraît que sous Course et Track day, et
  seulement sous le type choisi. En Essais et en Hotlap, la liste fait
  quatre lignes.
- **Elle porte une ligne de résumé** — `6 IA · 87 % ± 3`, le nombre d'IA puis la
  difficulté en centre ± écart. Ce n'est pas un ornement : sans elle, on ne peut
  plus savoir combien d'adversaires on affronte sans changer de page, alors
  qu'on peut lancer la session sans y être allé.
- **Les alertes de la page adversaires remontent sur l'entrée** — vivier trop
  maigre pour le nombre demandé, deux pilotes sous la même identité : le résumé
  passe en rouge et prend un marqueur. Une alerte sur une page qu'on ne regarde
  pas ne vaut pas mieux que pas d'alerte.
- **L'entrée parente sert de retour** depuis `Adversaires` : cliquer `Course`
  ramène aux réglages sans changer de type. L'indentation seule ne dirait pas
  « remontée » — c'est le **filet vertical** qui rattache la sous-entrée à son
  parent qui le dit, et il passe au rouge quand elle est l'entrée retenue.
  Compromis assumé de cette structure : le retour se fait en cliquant un type
  déjà sélectionné. La solution de repli, si le geste ne se lit pas à l'usage,
  est de rétablir une entrée `Paramétrage` distincte, au prix d'une ligne.
- **Changer de type ne détruit jamais rien.** Passer de Course à Essais puis
  revenir restitue l'intégralité des réglages Course, plateau compris — le type
  décide de ce qui est **affiché** et de ce qui est **envoyé au jeu**, jamais de
  ce qui est **mémorisé**. Quitter Course pour un type sans plateau referme
  simplement la page adversaires.
- **Une session est lançable sans avoir ouvert la page adversaires** : ouvrir un
  type Course ou Track day vierge remplit le plateau au hasard (`OpponentGrid.fill`).
- Le type vit dans un **store partagé** (`sessionNav.svelte.ts`) et non dans
  l'écran de réglages : la liste est à l'écran en permanence, l'écran de
  réglages n'est monté que pendant qu'on le regarde. Même circulation à sens
  unique que le lest et la bride — le store est la valeur vivante, l'écran la
  recopie, et seul un **chargement** (son propre montage, une session
  enregistrée) réécrit dans le store.

### 1.2 Le repli `PERFORMANCE`

Lest, bride, ABS et antipatinage sous **une ligne unique** de la carte voiture,
au gabarit de `LIVRÉE` et `PILOTE`. Les deux assistances viennent de l'écran de
réglages, où elles vivaient parmi les règles de la course : ce sont des
**capacités de la voiture**, et le réglage n'existe que parce que la voiture les
possède — ce que dit déjà la ligne « *Factory — la 1M a l'ABS et
l'antipatinage* », qui les suit ici. La ligne idéale (SESSION§3) ne suit **pas** :
elle n'est pas une capacité de la voiture mais une assistance d'affichage.

**La ligne affiche toujours son état, repliée ou non** : rien n'est masqué,
seulement rendu non modifiable — « absent » et « à zéro » ne doivent pas se
ressembler. Le résumé vaut *D'origine* en gris italique quand rien n'est posé,
et la liste des réglages non neutres en rouge sinon (`Lest 50 kg · Bride 10 %`,
`ABS Off`) : le rouge au sens du barème (§7.2ter), un réglage qui change la
course. ABS et antipatinage ne comptent comme posés que s'ils diffèrent de
`Factory`.

Le repli n'est pas mémorisé, et n'a pas à l'être : la ligne disant déjà tout,
l'ouvrir ne révèle rien qu'on ne sache. Le chevron est `▾` et non le `›` de la
maquette — dans cette colonne, `›` annonce une destination (l'écran Pilote) et
`▾` un dépliement sur place.

**L'intitulé d'un champ est une colonne, pas une ligne au-dessus.** Largeur fixe à gauche, valeur alignée à droite, la ligne restant à 30 px : **coût en hauteur zéro**, là où un intitulé posé au-dessus aurait coûté 14 px par champ pour le même service. Effet recherché : les valeurs s'alignent verticalement et la colonne se lit comme une fiche technique, pas comme une pile de menus. Cet alignement **est** tout l'intérêt du dispositif, donc la largeur est partagée par les quatre champs (variable CSS `--sess-lblw`) et **mesurée une fois par langue** — 60 px conviennent au français, l'allemand demande plus (`LACKIERUNG`), 88 px plafonnent et l'intitulé tronque au-delà. La mesure divise par `zoomFactor()` avant d'écrire, comme toute mesure de pixels qui repart dans un style (§13).

**L'état vide est une valeur, pas un intitulé.** Piège réel : `Aucun skin de circuit` se nomme parfaitement lui-même — tant qu'il est vide. Dès qu'un skin est coché, la ligne affiche `Gulf Racing` et la nature du champ disparaît. L'état vide rassure le concepteur, l'état rempli perd l'utilisateur. Donc l'intitulé ne bouge jamais (`SKIN`), et le défaut devient une valeur : *Celui d'origine*, italique grise — **partout où un défaut existe dans le produit, il se dit de la même façon**, un possessif et une italique, jamais une négation (même formulation que « Celle de la livrée » de l'écran Pilote, SESSION§5).

**La vignette d'un champ prend toute la hauteur de la ligne.** Elle a longtemps été une pastille de 13 px, au motif que d'une livrée on ne lit que la teinte — sauf qu'une livrée, un tracé et un pilote sont justement ce qu'on cherche à reconnaître d'un coup d'œil, et à 13 px aucun des trois ne se reconnaissait (signalé à l'usage). Elle fait donc 28 px : les 30 px de la ligne moins ses deux filets, soit le maximum **à hauteur de ligne inchangée** — c'est l'alignement des valeurs qui coûterait cher, pas les pixels d'image. Sans cadre à elle, qui doublerait celui de la ligne à un pixel de distance, et sans gouttière intérieure pour un tracé, qui lui prendrait un septième de sa hauteur. Les trois composants qui posent ces lignes (`ImageSelectDropdown`, `TrackSkinChecklistDropdown`, le champ nu de `SessionColumn`) partagent la même gouttière de 8 px : deux d'entre eux étaient à 9, ce qui décalait le début de leur valeur de 2 px — invisible tant que la vignette était une pastille.

**Un sélecteur à une seule option n'en est pas un.** Un menu qui affiche `Imola` sous un titre `Imola` ne propose rien et fait douter. Le champ devient alors une **ligne statique** : même colonne d'intitulé, 26 px au lieu de 30, ni bordure ni fond ni chevron, valeur en gris, plus une mention qui dit pourquoi il n'y a rien à choisir (`Imola — layout unique`). La règle vaut pour tous les champs — une voiture à livrée unique n'a pas de menu non plus. **La ligne n'est pas masquée pour autant** : elle dit ce que la session utilisera, c'est le contrôle qui disparaît, pas le fait.

**Le chevron dit où l'on va** : `▾` pour un menu qui s'ouvre sur place, `›` pour une destination — le champ `PILOTE` ouvre l'écran Pilote, pas une liste. Ténu, mais constant dans tout le produit.

**La source n'est pas un résumé.** `Porsche · #sportscars · skin: Miami Blue` répétait la livrée affichée juste dessous ; elle devient `Porsche · 2016`. `Imola · Kunos` répétait le nom écrit au-dessus ; elle devient `Kunos`. Même principe que le retrait de la marque dans le nom des cartes : ne pas payer des caractères pour une information présente à quelques pixels. Le tag quitte la colonne — c'est un critère de recherche, sa place est dans la bibliothèque et sur la fiche.

**Chaque emplacement a trois états**, et aucun bouton « Changer » : c'est la vignette et le nom qui ouvrent la bibliothèque.
- **Choisi** : photo, nom, métadonnée. Le libellé d'action n'est pas supprimé, il est **différé** — au survol *et au focus clavier*, un voile plein (`rgba(8,8,10,.72)`, 130 ms) porte « ✎ Changer de voiture / de circuit ». Le voile est plein parce qu'au moment où l'on décide de changer, la photo n'est plus l'information utile, et un voile partiel rendrait le libellé illisible sur une image imprévisible. Le bouton porte un `aria-label` complet (« Porsche 718 Boxster S — changer de voiture »).
- **Vide** : trame diagonale, bordure pointillée, « ＋ Choisir une voiture », ni nom ni métadonnée. **L'app ne choisit jamais une voiture à la place de l'utilisateur** au premier démarrage : « la première du catalogue » est une voiture arbitraire, et démarrer sur un bouton rouge qui lancerait une session au hasard installe le mauvais modèle mental. Tant que le duo est incomplet, « Paramétrage » et « Démarrer la session » sont **désactivés** — ce dernier gardant son fond rouge en opacité réduite : il reste la destination visible de l'écran, le griser effacerait le but à atteindre. C'est aussi ce premier démarrage qui enseigne le geste : sans autre chemin que le bloc, l'utilisateur apprend qu'il est cliquable, et le bouton permanent n'avait plus rien à enseigner ensuite.
- **Impasse** : Assetto Corsa introuvable ou chemin mal renseigné (`validate_config`, relu en quittant les Réglages). Même trame que l'état vide, mais « Aucune voiture détectée » + « Vérifiez le chemin d'installation », et le clic ouvre **Réglages › Chemins** au lieu d'une bibliothèque nécessairement vide — le problème à résoudre n'est pas le même.

**Ouvrir Content Manager** vit au pied du rail (§7.2), plus dans cette colonne. Le libellé ne dit **pas** « ouvrir *dans* » : CM ne reçoit ni la voiture ni le circuit, il s'ouvre sur son propre état, et « dans » annoncerait un transfert de contexte qui n'a pas lieu. Si CM n'est pas détecté au chemin configuré (`validate_config`), **l'entrée ne s'affiche pas du tout** — ni bouton grisé ni message d'erreur au clic : une sortie vers un outil absent n'a pas à occuper une ligne.

**Deux chemins vers les bibliothèques, deux intentions.** L'entrée du rail dit *je vais parcourir ma collection* ; le clic sur la vignette dit *je change la voiture de cette session*. Ce n'est pas une redondance. En l'absence de décision sur un cadrage différent (positionner la liste sur la voiture courante ?), les deux chemins sont strictement identiques — défaut acceptable.

**Un clic ouvre la bibliothèque, deux ouvrent la fiche — et les deux gestes sont comptés par l'écran, jamais confiés à l'événement `dblclick` du navigateur.** Les deux étaient posés côte à côte sur le même bouton, le clic simple agissant tout de suite, et il en découlait deux défauts dont l'un se voyait :

- **Le double-clic n'ouvrait pas la fiche** quand on n'était pas déjà sur la bibliothèque visée (signalé à l'usage). Le premier clic changeait de section, donc remontait tout l'écran principal et remplaçait le contenu **entre les deux moitiés du geste** — or un `dblclick` ne part que si les deux clics tombent sur le même élément, dans le temps système et sans que la cible ait bougé. Non reproductible en entrée synthétique, où les deux clics partent avant le moindre rendu : c'est exactement le genre de course qu'on ne corrige pas en la retentant.
- **L'historique enregistrait un écran que personne n'a regardé** : la liste, ouverte par le premier clic, puis la fiche. « Précédent » y ramenait, ce que `openInSection` existe justement pour éviter (§7.2bis).

Compter les clics dans l'écran règle les deux d'un coup : rien ne part avant que le geste ne soit fini, donc aucun rendu ne s'intercale et aucun écran intermédiaire n'existe. Le prix est un quart de seconde d'attente sur le clic simple — le prix habituel d'une cible qui porte deux gestes. Un emplacement vide ou en impasse n'a pas de fiche à ouvrir : le double-clic y vaut le simple.

**Frontière des zones cliquables.** Le bloc porte aussi le menu de livrée, celui de layout, la liste des skins de circuit et la ligne « Mon pilote » : un clic qui visait un menu ne doit jamais éjecter vers la bibliothèque. La zone qui navigue est donc un `<button>` **frère** de ces menus, jamais leur parent — un bouton contenant des contrôles interactifs est de surcroît invalide en HTML et casse la navigation clavier.

**`ImageSelectDropdown.svelte` (sélecteur de skin/layout du bloc Session) échappe au clip de la barre latérale.** La barre latérale (`.side`) défile verticalement (`overflow-y: auto`) — et une règle CSS fait qu'un seul axe posé à `auto` calcule l'autre à `auto` aussi, donc `.side` rogne également tout ce qui déborde en largeur. Un `position: absolute` classique en aurait fait les frais dès qu'un libellé de layout dépassait la largeur de la barre latérale : la liste ouverte restait aussi étroite que le déclencheur, ellipsée à mi-mot, sans le moindre moyen de lire le nom en entier. Corrigé sur deux fronts, indépendants l'un de l'autre :
- **La liste ouverte passe en `position: fixed`**, positionnée en JS depuis le rectangle du déclencheur (`getBoundingClientRect`), avec `width: max-content` (elle grandit jusqu'à son plus long libellé, plafonnée à `min(420px, 100vw - 16px)`) plutôt que calée sur la largeur du déclencheur. `fixed` échappe au clip de n'importe quel ancêtre à `overflow` — aucun n'y pose de `transform`/`filter`/`will-change`, ce qui aurait recréé un cadre de référence local et annulé l'échappée — et se repositionne au plus près du bord droit de la fenêtre si son plus long libellé la ferait déborder, une fois sa largeur réelle connue après rendu. Se ferme sur un défilement de n'importe quel ancêtre (sans ça, une liste `fixed` resterait figée pendant qu'un `.side` défilerait sous elle) — sauf le sien propre, sans quoi parcourir une longue liste la refermerait avant qu'on ait pu cliquer. **Piège vérifié plutôt que supposé** : un élément `position: fixed` reste un `offsetParent` valide pour ses propres enfants (seul lui-même, interrogé directement, renvoie `offsetParent === null`) — la navigation manette (`gamepadNav.ts`, filtre `offsetParent !== null`) continue donc de trouver les boutons de la liste sans adaptation.
- **Le déclencheur montre le nom complet au survol/focus**, à la place de l'ancienne infobulle native figée sur le texte du placeholder (« Choisir un layout ») : une bulle maison (même mécanique CSS que `Tooltip.svelte` — `:hover`/`:focus`, pas de JS — mais pas le composant lui-même, qui enveloppe son déclencheur dans un `inline-flex` incompatible avec le `width: 100%` du bouton ici) affiche le libellé complet, alignée à gauche et plafonnée en largeur (200px, texte qui s'enroule) pour la même raison de clip que ci-dessus. `:focus`, pas `:focus-within` : ne réagit qu'au déclencheur lui-même, jamais à un bouton de la liste ouverte (qui montre déjà les noms en entier) — et la bulle disparaît entièrement tant que la liste est ouverte, pour ne pas s'y superposer. Fonctionne aussi bien au focus posé par la manette (`gamepadNav.ts` appelle un vrai `.focus()` DOM) qu'au survol souris, contrairement à l'attribut `title` natif, qui ne réagit qu'au survol réel.

## 2. Pilotage par preset Quick Drive CM

L'app pilote CM via son protocole `acmanager://race/quick?presetFile=…` : un **preset Quick Drive** (JSON, format `SaveableData` de CM) est généré à chaque lancement dans un fichier temporaire jetable, puis passé à `Content Manager.exe`. C'est le même chemin (`QuickDrive.ViewModel.Go()`) que le bouton « DRIVE » de l'UI Quick Drive native de CM — condition nécessaire pour que le téléchargement CSP automatique (VAO/config manquants) se déclenche.

> L'ancien mécanisme `race/config?configFile=` (race.ini brut via `PreparedConfig`) a été abandonné : il ne peuple pas `StartProperties.BasicProperties`, dont dépend le check CSP auto-load côté CM — bug confirmé empiriquement, détail en `docs/L4-cm-launch-research.md`.

Limite connue du preset Quick Drive : durée de session Practice non appliquée (sessions à durée libre par design Quick Drive, pas de champ correspondant dans le schéma).

**L'évolution du grip, elle, part bien** — elle ne partait pas, et le champ existait pourtant. Les dix presets de référence portent tous le même `TrackPropertiesData`, ce qui avait été lu comme « pas de champ dédié » alors qu'ils avaient simplement tous été sauvegardés sur une piste optimale. Ce qui a tranché est la table de presets **que le jeu embarque**, `cfg/templates/tracks.ini` : son entrée `OPTIMUM` (`SESSION_START=100`, `SESSION_TRANSFER=100`, `RANDOMNESS=0`, `LAP_GAIN=1`, « Perfect track for hotlapping. ») reproduit exactement le `{"s":1.0,"t":1.0,"r":0.0,"g":1,"d":…}` des presets. D'où la lecture des clés abrégées : `s` et `t` sont des pourcentages divisés par 100, `g` le `LAP_GAIN` brut, `d` une description que CM affiche et que le jeu ne lit pas — son `[DYNAMIC_TRACK]` n'a pas de clé correspondante.

**L'écran offre « Auto » puis les six états du jeu**, ceux de `tracks.ini`, avec ses noms et dans son ordre — la même liste que propose Content Manager. « Auto » est le `WeatherDefined` de CM (`w` du preset) : la météo décide, et les quatre nombres partent quand même, aux valeurs de Verte — `ToProperties()` les écrit dans tous les cas, donc une météo muette sur la piste la laisse verte, ce que la description de CM énonce mot pour mot. Côté `RaceSetup`, c'est la sentinelle `grip = 0` : l'écran n'offre qu'un choix parmi sept, et deux champs pour une seule décision finissent par se contredire.

**Le nom de l'état ne part jamais, seuls les nombres comptent.** « Green » est un libellé d'interface pour une combinaison de quatre valeurs ; rien dans le preset ne le transporte, et le `d` qui l'accompagne est une description libre que CM affiche et qu'AC ne lit pas. Le panneau « Track state » de CM n'est donc pas une information mais **le paramétrage lui-même** : `TrackStateViewModelBase.SaveableData` sérialise `s`/`t`/`r`/`g` en *GripStart / GripTransfer / GripRandomness / LapGain*, et `ToProperties()` fait `SessionStart = GripStart * 100` avant que CM n'écrive `SESSION_START=95` dans le `[DYNAMIC_TRACK]` du `race.ini`. Le grip de départ **est** l'identifiant de l'état : c'est le seul des quatre paramètres que l'écran retienne, `quickdrive.rs` retrouve les trois autres à partir de lui et les envoie tous. Un preset antérieur à cet alignement peut porter une valeur qui n'y figure plus (92 % a existé, inventé) : il est recalé sur l'état le plus proche, le plus adhérent l'emportant à égale distance — pas de migration à écrire.

**L'échelle de `r` est tranchée, et c'est le panneau de CM qui l'a fait.** Elle ne pouvait pas l'être depuis les presets de référence, dont le `RANDOMNESS` nul donne `0.0` dans les deux hypothèses. Sur l'état `GREEN`, CM affiche « Grip at the start 95 % », « Grip transfer 90 % » et « Randomness 2 % », là où le fichier du jeu écrit `95`, `90` et `2` : les trois sont donc le même pourcentage divisé par cent, l'aléa compris. Seul `LAP_GAIN` reste brut — et il n'est d'ailleurs pas affiché en pourcentage.

### 2.1 Une course, deux modes CM selon la qualification

Le mode `QuickDrive_Weekend.xaml` n'a pas d'état « pas de qualification » : son curseur est borné à `[5, 90]` min et son `Save()` n'écrit jamais de durée nulle. Une course **sans** qualification passe donc par l'autre mode course de CM, `QuickDrive_Race.xaml`, dont le `ModeData` ne porte aucune durée — c'est le même contenu que Weekend moins `PracticeLength`/`QualificationLength` (schéma confirmé sur un preset réel sauvegardé depuis l'UI de CM). Côté Pit Box, un seul type de session « Course » : c'est la case Qualification qui décide du mode envoyé.

Les essais libres n'existent que dans Weekend, ils suivent donc la qualification.

**`PracticeLength: 0`, jamais `null`** : le `Load()` de `QuickDrive_Weekend.xaml.cs` fait `r.PracticeLength ?? 15`, donc `null` ne saute pas la phase — il rend 15 minutes d'essais par défaut. Seul `0` la saute (curseur `[0, 90]`, libellé « Skip session » côté CM). Bug réel, constaté en jeu avant d'être retrouvé dans la source.

L'URI de lancement porte `&loadAssists=true`, qui correspond au flag `forceAssistsLoading` lu par `ArgumentsHandler.Race.cs::ProcessRaceQuick` (code source de CM) et force le chargement de l'`AssistsData` du preset (dégâts/carburant/pneus/aides/chauffe-pneus), **indépendamment** du réglage global de CM « Charger assistances avec préréglage de course rapide » (désactivé par défaut chez CM). Sans ce flag, CM ignore silencieusement (pas d'exception, pas de log) l'`AssistsData` de n'importe quel preset Quick Drive — y compris ceux sauvegardés par l'utilisateur lui-même dans CM — et garde les assistances actuellement actives dans son UI. Confirmé en lisant `QuickDrive.xaml.cs`/`ArgumentsHandler.Race.cs` (`gro-ove/actools`). `TrackPropertiesData` (grip) n'a pas de garde équivalente côté CM — toujours chargé, indépendamment de ce flag.

**Skin joueur — réinjecté dans le `race.ini` juste après CM.** Le protocole `race/quick` ne transporte aucun skin joueur : `ArgumentsHandler.Race.cs::ProcessRaceQuick` ne lit qu'un preset + des assists et n'en transmet aucun à `QuickDrive.RunAsync()` (qui a pourtant un paramètre `carSkinId`, jamais alimenté depuis l'URI), et le format de preset lui-même n'a pas de champ skin — mesuré, pas déduit : deux `.cmpreset` sauvegardés par CM avec deux skins différents sont identiques octet pour octet. CM retombe donc sur `CarObject.SelectedSkin`, sa mémoire par voiture.

Pit Box écrit donc **après** lui. CM réécrit `Documents\Assetto Corsa\cfg\race.ini` à l'instant où il lance `acs.exe`, mais le jeu ne lit ce fichier que quelques centaines de ms plus tard, pendant son chargement. Un fil de surveillance (`raceini.rs`) guette cette réécriture, remplace `SKIN=` dans `[RACE]` et `[CAR_0]` — les deux sections du joueur, les adversaires (`[CAR_1]`…) gardant les skins que CM vient d'écrire depuis notre grille — puis substitue le fichier par `fs::rename` (atomique : le jeu voit l'ancienne version ou la nouvelle, jamais une moitié). Mesuré sur un lancement réel : écriture de CM à +1694 ms après l'URI, réinjection à +1702 ms, skin confirmé chargé par le jeu dans son propre `logs\log.txt`.

Best-effort par construction, comme tout le reste du lancement : arriver trop tard laisse simplement le skin de CM, l'état d'avant ce mécanisme. Un garde vérifie que `[RACE] MODEL=` correspond bien à la voiture demandée avant de toucher au fichier — on ne tamponne pas un skin sur la session de quelqu'un d'autre — et chaque abandon est journalisé (`log::warn!`). Écrire directement dans le cache interne de CM (`Values.data`) a été écarté : fichier compressé au format propriétaire, réécrit par CM à sa fermeture, et skin mis en cache mémoire dès la première lecture.

> Écrire `race.ini` **avant** de lancer CM ne sert à rien : `Game.StartAsync` charge le fichier existant, le nettoie, puis `BasicProperties.Set()` réécrit `[RACE] SKIN` et reconstruit `[CAR_0]` intégralement. Vérifié : un skin sentinelle écrit avant lancement est effacé 0,26 s après l'envoi de l'URI.

### 2.2 Track day

Quatrième type de session, à droite de Course. Passe par son propre mode CM, `QuickDrive_Trackday.xaml` — schéma confirmé sur un preset réel sauvegardé depuis l'UI de CM (`pitbox-trackday.cmpreset`) : même grille manuelle d'adversaires que Course (SESSION§3), tours, faux départ et pénalités, plus `SpeedLimit` (pas de champ correspondant côté Pit Box pour l'instant — toujours 0, comme le seul preset de référence vu). Contrairement à Course, aucun mode Weekend équivalent : jamais de qualification ni d'essais libres, quel que soit le réglage.

### 2.3 Steam doit tourner avant le lancement

Assetto Corsa est un jeu Steam : c'est Steam qui le démarre, quel que soit le `Starter` retenu par CM. Steam éteint, l'échec se produit **après** que Pit Box a rendu la main — aucune erreur ne remonte à l'app, l'utilisateur voit seulement une session qui ne démarre pas.

Le lancement vérifie donc la présence du process `steam.exe` (`launch::steam_running`, scan ponctuel `sysinfo`, même mécanique que la surveillance du jeu en §16.2) **avant** de construire le preset. Absent : un dialogue demande de démarrer Steam et de valider, la validation revérifie, et tant que Steam manque le dialogue reste ouvert en le signalant. Une erreur de la vérification elle-même laisse passer le lancement — le pire cas redevient simplement l'échec côté CM.

**Bouton « Ouvrir dans CM »** : lance CM sans argument de session, sélection active, pour les réglages fins (échappatoire power-user).

## 3. Écran de réglages

Pas de rappel du duo en haut (déjà dans la barre latérale). Le titre **suit la
navigation** : `Course`, puis `Course · Adversaires` (SESSION§1.1) ; `Enregistrer` et
`Charger` restent à sa droite — ils portent sur toute la configuration, pas sur
le type. **Fond photo** derrière l'interface : voir §6.2 pour l'ordre de repli
(screenshot du combo → screenshot du circuit → background officiel → fond
neutre).

**Un réglage se range selon sa PORTÉE, jamais selon sa fréquence d'usage.**
C'est le principe qui décide de tout placement sur cet écran, et le critère de
fréquence est écarté explicitement : il envoie tout ce qui est rare au même
endroit, et cet endroit devient un fourre-tout. Quatre zones, quatre objets :

| Zone | Objet | Contenu |
| --- | --- | --- |
| Panneau gauche | la voiture et la piste | livrée, pilote, lest, bride, ABS, antipatinage |
| Colonne centrale | la session et ses règles | durées, dégâts, carburant, usure, pénalités, ligne idéale, départ |
| Rail droit | les conditions | météo, températures, vent, état de piste, heure, saison |
| Page dédiée | les adversaires | filtres, générateur, plateau |

**Communs à tous les types de session** — regroupés dans « Simulation » (dégâts,
conso carburant, usure pneus, puis chauffe-pneus, pénalités et ligne idéale sur
une ligne de trois cases) et dans le rail droit (« Conditions », SESSION§3.3) :
ces réglages sont envoyés au preset Quick Drive quel que soit le type
(`Penalties` figure dans les trois `ModeData` ; `TrackPropertiesData` et
`AssistsData` sont au niveau racine du preset, pas dans `ModeData`), rien ne
justifie de les cantonner à Course. Lest, bride, ABS et antipatinage sont
communs eux aussi, mais leur place est la carte voiture du panneau gauche
(SESSION§1.2).

**Options de session ne porte que ce qui dépend du type** : départ en Practice ;
ghost car et son avance en Hotlap ; tours, durées de qualification et d'essais
libres, faux départ et position de départ en Course ; faux départ en Track day.
Le bloc a longtemps eu **deux zones**, la droite portant l'évolution du grip et
les pénalités pour qu'elles restent à la même place dans les quatre types. Les
deux en sont sorties — le grip vers le rail droit (SESSION§3.3), les pénalités
vers Simulation — et la seconde zone avec elles : ce qu'elle protégeait est
obtenu mieux en sortant du bloc ce qui n'y dépend de rien.

**Les durées : trois champs du même gabarit, et une valeur à 0 désactive sa
séance.**

```
TOURS  [  5 ]      QUALIFICATION  [ 10 ] min      ESSAIS LIBRES  [ 20 ] min
```

Les deux cases à cocher ont disparu. Une case et une durée étaient un seul
contrôle dessiné en deux, et la paire portait une règle à elle — griser le
champ, conserver la valeur au décochage — qu'un champ unique rend sans objet :
il n'y a plus rien à griser, et la valeur qu'on retrouve est celle qui est à
l'écran. Un `0` s'affiche **éteint**, une valeur non nulle en blanc. Les
booléens restent dans `RaceSetup` — ce sont eux qui choisissent le mode de
Content Manager — mais ils se **déduisent** des durées. Les essais libres
n'existant que dans le mode Weekend, celui que porte la qualification, leur
champ est éteint tant que la qualification vaut 0 et dit pourquoi.

**Un bloc peut prendre la largeur disponible ; ses contrôles internes gardent
leur gabarit et restent calés à gauche.** Un curseur de 900 px pour un réglage
qu'on pose au pourcentage près est une régression, pas un gain.

**Le bloc a la même hauteur dans les quatre types.** Il en changeait à chaque
fois — un segmenté est plus court qu'un champ numérique, et la case du ghost
n'a pas d'intitulé au-dessus d'elle — si bien que SIMULATION, juste dessous,
sautait de quelques pixels quand on changeait de type. Un plancher égal à la
rangée la plus haute (intitulé + champ de 32 px) le fige, et l'alignement par le
bas cale les contrôles plus courts sur la même ligne de base que les autres.

**Un champ sans objet dans le type courant est retiré, jamais grisé.** Le grisé
est réservé aux **dépendances internes** — une case décochée éteint sa durée, la
qualification éteint la position de départ. La distinction se lit : ce qui est
absent n'existe pas ici, ce qui est éteint existe et attend qu'on lève ce qui le
neutralise. Dans les deux cas la valeur est **conservée** : décocher puis
recocher retrouve ce qui avait été réglé.

**Ordre des blocs de la colonne centrale, invariant dans les quatre types** :
Options de session, puis Simulation. Le bloc « Type de session » a disparu — le
type est la navigation de la colonne de gauche (SESSION§1.1) — et les adversaires
ont leur page (SESSION§3.2). Le seul bandeau qui puisse s'ajouter en tête est
l'avertissement « ce circuit n'est pas catégorisé comme circuit fermé », posé au
niveau de la page parce que c'est le couple type + circuit qui est en cause, pas
un réglage.

**Position de départ** (Course uniquement) : quatre segments — Au hasard, 1er,
2e, Dernier — placés juste après le faux départ. Au hasard par défaut, et le
seul des quatre qui ne fixe rien. Le rang réel se résout à la construction du
preset, où la taille du plateau est connue pour de bon : « Dernier » vaut
`adversaires + 1`, le joueur comptant pour une voiture.

**Écart assumé avec Content Manager, à ne pas « corriger » par mégarde.**
Une qualification non nulle neutralise la position de départ — la grille vient alors
des résultats de qualif. Chez CM le contrôle est purement **absent** dans ce
cas : relevé dans son binaire, il n'est lié que dans la vue `QuickDrive_Race`,
et `QuickDrive_Weekend` — celle qui porte la qualification — n'en a aucune
trace. Pit Box l'**éteint** au lieu de le retirer, et c'est délibéré : chez CM
la qualification et la position vivent dans deux vues, chez nous dans le même
écran, à quelques centimètres l'une de l'autre. Un réglage qui disparaît quand
on coche une case voisine se lit comme un bug ; éteint, il dit ce qui le
neutralise. C'est la règle des dépendances internes ci-dessus, et elle prime
ici sur l'alignement sur CM.

**Les aides au pilotage ont trois états, pas deux** — elles vivent depuis le
lot 5 dans le repli `PERFORMANCE` de la carte voiture (SESSION§1.2), leur règle
restant celle-ci : `Off` / `Factory` / `On`,
le vocabulaire d'Assetto Corsa lui-même — et ses valeurs, relevées sur un
fichier livré par le jeu (`launcher/themes/default/index.html` :
`data-slidervalues="0,1,2"` en face de `data-slidertextvalues="Off,Factory,On"`,
lié à la clé `ABS` de `cfg/assists.ini`, celle que porte l'`AssistsData` d'un
preset Quick Drive). Une case à cocher n'en portait que deux, et « cochée »
envoyait `1`, c'est-à-dire `Factory` — le réglage avait donc trois états en jeu
et deux à l'écran, dont aucun ne s'appelait par son nom. `Factory` garde
l'équipement réel de la voiture, ce qui veut dire que **`On` ne peut rien
ajouter à une voiture qui n'en a pas** : c'est le piège que les deux
info-bulles expliquent. La ligne idéale reste une case — elle n'a que deux
états.

> **Migration** d'un réglage enregistré avant ces trois états : `true →
> factory`, `false → off`, absent → `factory`. Pas `true → on`, malgré
> l'apparence : l'ancien booléen envoyait déjà `1`, et une migration ne change
> pas ce qui part en jeu. La conversion passe sur **toutes** les entrées
> (chaque type de session a son preset, chaque session enregistrée son propre
> `setup`), pas sur le preset courant seul.

**Ce que `Factory` vaut pour la voiture en session** se lit sous les deux
segmentés, dans le repli `PERFORMANCE` du panneau gauche (SESSION§1.2) : « *Factory — la Ford Mustang Mach 1 428 n'a ni ABS ni
antipatinage.* » C'est la troisième chose que Pit Box sait et que Content
Manager ne montre pas, après le vivier et les sessions enregistrées : CM
affiche trois valeurs opaques, l'app lit les specs de la voiture. La présence
d'usine est déclarée par `PRESENT` sous `[ABS]` et `[TRACTION_CONTROL]` de
**`electronics.ini`**, à l'intérieur du `data.acd` (`electronics.rs` ; le
déchiffrement est celui d'`acd.rs`). Trois précisions qui ne se retrouvent pas
deux fois :

- **Ce n'est pas `drivetrain.ini`**, qui porte la boîte et le différentiel.
- **La présence du fichier ne prouve rien** : il existe sur toutes les voitures
  de l'install de référence, y compris celles qui n'ont aucune aide, où il dit
  simplement `0`. Seul son contenu répond.
- **La lecture est bornée à la section.** `PRESENT` apparaît aussi sous
  `[EDL]` : une lecture à plat créditerait d'un antipatinage toute voiture
  ayant l'ABS.

Mesuré sur les 311 voitures de l'install : 142 ont les deux, 31 l'ABS seul, 17
l'antipatinage seul, 111 ni l'un ni l'autre, et **10 ne déclarent aucune des
deux sections**. Ces dix-là n'affichent **aucune ligne** — jamais une ligne au
conditionnel ni un « inconnu » : aucune donnée inventée présentée comme un
fait. Les 48 asymétriques sont la raison pour laquelle la phrase nomme *quelle*
aide la voiture possède au lieu de répondre oui ou non.

**Course et Track day** (absents des schémas Quick Drive Practice/Hotlap : pas de
grille, pas de phase weekend) :
- **Adversaires** : une **page dédiée**, atteinte par la sous-entrée de la liste des types (SESSION§1.1). Le vivier y est **un filtre**, celui de la bibliothèque (SESSION§3.1). Trois onglets (Même voiture / Par catégorie / Libre) faisaient ce travail avant lui et le faisaient moins bien : ils ne savaient pas combiner, et ils doublaient une barre de filtres qui existait déjà — c'est ce doublon qui obligeait à des règles de réconciliation entre l'onglet et les jetons.
- Faux départ, pénalités — communs à Course et Track day. **Tours** : Course uniquement — envoyé dans le `ModeData` de Track day aussi (schéma confirmé sur un preset CM réel), mais sans effet en jeu : une session Track day ne se termine jamais sur un décompte de tours (testé).

### 3.1 Le vivier, le plateau, et les deux gestes entre les deux

**Le filtre définit le vivier, jamais le plateau.** Le bloc Adversaires porte la
barre de filtres de la bibliothèque — troisième consommateur après les deux
écrans de bibliothèque — et un compteur `Vivier · N voitures`. Ce qu'on y gagne
est la **combinaison** : `#gt3` **et** 2010-2016 **et** sauf Kunos est trivial
avec des jetons et était impossible avec trois onglets.

**Trois puces de raccourci** — Même voiture, Même catégorie, Même performance —
posent **un jeton et rien d'autre**. Une puce paraît active quand son jeton est
présent, y compris posé à la main ; la recliquer le retire. Il n'y a donc pas de
second état à tenir d'accord avec le premier, ce qui est tout l'intérêt. Une
puce dont la référence ne peut rien fournir (pas de voiture choisie, pas de
catégorie déclarée, specs illisibles) est éteinte et **dit pourquoi** — par
`aria-disabled` et non `disabled`, un bouton désactivé ne recevant aucun
événement de survol, donc n'affichant jamais son explication.

**Asymétrie assumée entre les trois.** `Modèle` et `Catégorie` posent une
**valeur** : changer de voiture ne déplace pas le jeton, c'est un instantané.
`Performance` pose une **tolérance**, dont la référence est lue dans le
contexte : la bande suit la voiture pilotée. Une bande nommée « ±15 % de ma
voiture » qui continuerait de mesurer sur une voiture qu'on ne pilote plus
mentirait ; un jeton de catégorie qui cesserait de dire ce qu'il affiche serait
le comportement des onglets revenu par la fenêtre.

**Le jeton `Performance`** filtre sur le rapport **poids / puissance** en
kg/bhp, à un écart relatif réglable autour de celui de la voiture pilotée. Il
est disponible **aussi dans la bibliothèque** : « montre-moi tout ce qui roule au
niveau de ma 488 » est une question qu'on se pose hors session. La lecture des
specs est délibérément **conservatrice** — mesurée sur les 311 voitures de
l'install, 291 donnent un ratio et 20 sont écartées : six annoncent une
puissance aux roues (`whp`, une autre grandeur, pas un facteur), neuf des
chevaux métriques (`ps`, `л.с.` — le facteur est exact mais l'intention de
l'auteur ne l'est pas), cinq écrivent `--`. Une voiture illisible **sort du
vivier** et affiche `—`, jamais une valeur estimée.

**Entre le vivier et le plateau il y a toujours un geste, et il y en a
exactement deux**, travaillant tous deux sur l'ensemble filtré : `Tirer N au
hasard` **remplace** le plateau — le chemin de qui veut courir tout de suite —
et `Choisir dans le vivier · N voitures` ouvre la modale sur **ce même filtre**
et **ajoute** en fin de plateau — le chemin de qui veut décider. Le nombre porté
par le second dit ce que le filtre a acheté : N lignes à lire au lieu de toute
la bibliothèque.

**La modale partage l'état de filtre du bloc**, elle n'en dérive plus. Il n'y a
qu'un vivier, et elle en est la vue détaillée : y retirer un jeton élargit aussi
ce dans quoi le tirage pioche. **Un composant, deux modes** — « Ajouter » :
sélection multiple à cases, `Maj+clic` pour étendre, ajout en fin de plateau
dans l'ordre affiché ; « Remplacer » : sélection simple, ouverte défilée sur la
voiture de la ligne, double-clic pour valider, la force de la ligne conservée et
le skin retiré au sort dans ceux de la nouvelle voiture.

**Aucun repli quand le vivier est vide.** Un filtre qui ne garde rien rend un
plateau vide, pas un tirage fait ailleurs : le compteur dit `Vivier · 0` et les
deux boutons sont éteints. Le repli d'avant venait des onglets, dont le vivier
pouvait être vide sans que rien ne le dise. Un vivier plus maigre que le nombre
d'adversaires demandé se signale sans bloquer — le plateau reste jouable, il
répète simplement des voitures avec d'autres livrées.

**Le jeton « jouable » n'est pas épinglé.** Il l'a été, pour qu'un tirage ne
puisse pas produire un plateau injouable. Il coûtait une puce permanente dans
une barre de 600 px pour prévenir un cas rare, et quand ce cas se présente la
garde d'activation le dit au-dessus du bouton de lancement et le répare d'un
clic. Un avertissement qui apparaît quand le cas se produit vaut mieux qu'une
puce qui prend de la place le reste du temps.

**Le plateau ne se régénère plus tout seul au changement de voiture pilotée.**
La règle était « régénérer sauf en mode libre, sauf si le plateau a été touché à
la main » — trois conditions pour deviner s'il appartenait encore à
l'utilisateur ou au vivier. Le vivier étant un filtre qu'un changement de
voiture ne déplace pas, régénérer jetterait un plateau au profit d'un tirage
dans le **même** vivier. Le plateau ne change donc plus que sur un geste, et le
marqueur « plateau manuel » a disparu avec la règle qu'il servait.

**`Régénérer` n'est pas `Tirer` sous un autre nom** : il garde les voitures et
retire au sort ce qui avait été tiré **sur** elles — la livrée, et avec elle le
nom de pilote laissé en `Auto`. « Le plateau est bon mais les livrées se
répètent » et « le plateau n'est pas le bon » sont deux gestes qu'on veut
séparément.

**Deux pilotes ne portent jamais la même identité par accident.** Le jeu nomme
une IA d'après le `ui_skin.json` de sa livrée — pilote, numéro, pays, présents
sur 393 à 400 livrées d'un corpus de 400. Le tirage évite donc les livrées dont
le couple numéro + nom est déjà pris **sur tout le plateau**, pas seulement par
voiture : deux livrées différentes peuvent parfaitement déclarer le même « 59
Juan », et deux lignes indistinguables étaient le défaut visible. Un doublon
forcé à la main est signalé, jamais corrigé dans le dos.

### 3.2 Le plateau

**Chaque cellule éditable vaut `Auto` ou une valeur explicite.** `Auto` n'est
pas une valeur qu'on tire : c'est **l'absence de surcharge**, et le format de
Content Manager la connaît déjà — `-1` dans un tableau numérique, `null` dans un
tableau de texte, les deux relevés sur un preset de grille réel où un `"0"`
explicite voisine un `"-1"`. Une valeur posée à la main n'est donc jamais
écrasée par un tirage, et **vider le champ la rend à `Auto`** : le geste naturel
pour dire « je ne décide pas », et la raison pour laquelle aucun menu de ligne
n'est nécessaire.

Une force laissée en `Auto` est tirée **par le jeu** dans la fourchette de
difficulté, à l'exécution : elle n'est pas connue au moment où l'on configure, et
l'écran affiche donc le mot `Auto` plutôt qu'un nombre qui ne serait pas
celui-là. Un nom ou une nationalité en `Auto` affiche en revanche **ce que la
livrée déclare**, qui est exactement ce que le jeu emploiera.

**Le survol d'une ligne montre ce qui est tronqué, pas une photo.** Un aperçu
en grand s'y ouvrait : il recouvrait les lignes voisines — précisément celles
qu'on est en train de comparer — pour montrer ce que la vignette de la ligne
montrait déjà. Ce qui manque vraiment, c'est le texte que la colonne élide : le
nom complet de la voiture et de sa livrée, et le nom du pays. Ils sont donc en
infobulle.

**La vignette ouvre le choix de livrée.** Il n'y en avait aucun : le `+`
dupliquait une ligne avec une autre livrée, `Regenerate` les retirait toutes au
sort, mais rien ne permettait d'en désigner une. La vignette est la cible
naturelle — c'est l'image de la livrée, donc l'endroit où l'on pense à la
changer — et elle évite un bouton de plus dans une ligne qui en porte déjà deux.
Le reste de la ligne ouvre le choix de **voiture** : deux questions, deux cibles.
Le popover liste les livrées par leur `livery.png` d'abord, la photo en repli —
l'inverse de la vignette de ligne, qui répond à « quelle voiture ? » et non à
« quelle peinture ? ».

**La nationalité n'est pas un champ libre.** Le jeu en tient la liste —
`$.Nationalities` dans `launcher/themes/.base/ac.utils.js`, **221 entrées**
actives, code ISO 3166-1 alpha-3 vers nom anglais, plus vingt-huit territoires
qu'AC y a commentés et qui restent donc écartés. Chacune a son drapeau dans
`content/gui/NationFlags/<CODE>.png` : 222 fichiers, les 221 entrées **toutes
pourvues** plus `AC.png`, le repli du jeu. La cellule ne montre **que le drapeau** : écrit en toutes lettres,
« Brunei Darussalam » prenait un cinquième de la largeur du plateau pour ce
qu'un drapeau dit d'un coup d'œil. Le nom reste là où on le cherche — en
infobulle, et dans le menu au moment de choisir.

**Le menu est maison, pas un `<select>`.** Un `<option>` natif ne peut pas
porter d'image : le menu déroulé est dessiné par le système, pas par la webview,
donc le drapeau n'y apparaissait jamais — alors que c'est lui qui fait
reconnaître un pays d'un coup d'œil. Ce qu'on perd est la recherche à la frappe
du système ; ce qu'on gagne est un vrai champ de recherche, et sur **220 pays**
il vaut mieux : il cherche n'importe où dans le nom et pas seulement au début,
donc « guinea » rend les trois Guinées.

Ce qui est **stocké reste le nom entier**, jamais le code : c'est ce que dit
`ui_skin.json` et ce qu'écrit un preset de grille CM (« Brunei Darussalam » y a
été relevé) ; le code ne sert qu'à trouver l'image. La liste est lue dans
l'installation du jeu plutôt que recopiée — les drapeaux en viennent déjà, et
les deux restent ainsi alignés. Installation illisible : liste vide et retour à
la saisie libre, un menu vide empêcherait d'éditer.

**Un piège, et il est dans la table du jeu** : « Congo » y apparaît deux fois,
`COD` et `COG` sous le même libellé. Le nom ne peut donc pas désigner un drapeau
sans ambiguïté. C'est l'ambiguïté d'AC et non la nôtre — on n'invente pas un
libellé qu'il ne connaît pas : la liste est dédoublonnée par nom (220 entrées
offertes) et le premier code gagne pour le drapeau.

**Une force explicite remplace, elle ne multiplie pas.** `race.ini` écrit un
`AI_LEVEL` absolu par voiture : il n'y a aucune transformation entre ce qu'on
pose et ce que le jeu reçoit. Une ligne à 95 dans un plateau réglé 84-90 est
donc **légitime** — c'est même le cas d'usage, poser une IA rapide dans un
plateau moyen — et rien dans l'écran ne la signale comme une incohérence. Ce
qu'on observe en bougeant la fourchette, c'est le **recalcul des lignes `Auto`**,
qui ressemble à une multiplication sans en être une.

**La fourchette part enfin dans le preset.** `AiLevel` / `AiLevelMin` y étaient
codés en dur sur 95/85 : le réglage de l'écran n'atteignait jamais le jeu — et
une ligne `Auto` n'aurait rien voulu dire, puisque c'est précisément dedans que
le jeu tire. `AiLevel` est le **haut** de la fourchette et `AiLevelMin` le bas,
relevé sur un preset réel.

**Colonnes : toutes, tout le temps, et aucune abréviation.** Le menu de
colonnes est parti, et avec lui la préférence qu'il gardait. Il existait parce
que le plateau vivait dans une colonne d'écran : à 700 px, huit colonnes ne
tenaient pas, et il fallait choisir. La page dédiée a supprimé la contrainte —
la table a toute la largeur — donc aussi la question. Un menu qui cache des
colonnes dont on a la place est un geste de plus pour un problème qui n'existe
plus, et un réglage à retrouver quand on cherche une valeur qui « a disparu ».
Corollaire : plus de largeur à économiser, donc `Nat.` et `Str.` s'écrivent
`Nationalité` et `Force` — elles ne se lisaient que parce qu'on savait déjà ce
qu'elles disaient. Même raison pour le **nom du pays affiché à côté de son
drapeau** : il vivait en infobulle, c'est-à-dire à peu près nulle part, et un
drapeau seul se reconnaît mal au-delà d'une dizaine de pays.

Le lest et la bride n'ont pas d'`Auto` : « rien » s'y dit par 0, comme dans le
preset. Les en-têtes distinguent les colonnes éditables des colonnes en lecture
seule par les **deux gris** de l'app, sans en introduire un troisième.

**Une seule police pour toute la rangée d'en-tête**, et elle est posée sur la
rangée. Chaque intitulé héritait de la taille de SA colonne — 10,5 px pour le
nom, 9 pour le kg/bhp, 8 pour les autres : trois tailles sur une même ligne, ce
qui se voit avant même qu'on lise les mots. Deux pièges de mise en œuvre, tous
deux constatés à l'écran : la couleur doit être reposée là aussi (`.lbl-key`
est globale, donc moins spécifique que les règles de colonne du composant — la
colonne « Force » ressortait en blanc), et la case vide qui tient lieu de
vignette dans l'en-tête doit avoir **exactement** la largeur de la vignette
d'une ligne, sans quoi toute la rangée est décalée de la différence et chaque
intitulé désigne la colonne d'à côté.

**Le vivier et le plateau ont leur page, et rien ne les sépare.** Dans l'ordre :
le bandeau (vivier et tirage), les bannières d'alerte, le plateau — un seul
enchaînement, pleine largeur, sans césure de carte.

**Le bandeau : deux groupes côte à côte, un filet entre eux.** Le vivier et le
tirage occupaient trois rangées empilées — la barre de filtres, les trois
puces, puis le générateur — sur une page qui a toute la largeur de l'écran. Ils
tiennent côte à côte, et le filet dit ce que l'empilement disait mal : à gauche
**à qui on a le droit de piocher**, à droite **ce qu'on en tire**. Le groupe de
gauche est dimensionné par son contenu, pas étiré : le groupe de droite se lit
accolé au filet et non renvoyé au bord opposé de l'écran. Chaque groupe se
replie sur lui-même quand la largeur manque, et le filet disparaît quand les
deux s'empilent — un trait vertical entre deux blocs superposés ne sépare plus
rien.

**Le décompte du vivier se lit dans la coulée du filtre**, juste après les
puces, et non calé à droite de la barre. Le bloc de droite de `FilterBar` est
fait pour cohabiter avec des contrôles de vue : à la bibliothèque, le décompte
y voisine la bascule grille/tableau et se lit comme une information d'écran.
Cette page n'en a aucun, et le décompte s'y retrouvait seul à l'autre bout de la
ligne, sans rien pour dire qu'il est la **conséquence du filtre** — signalé à
l'usage. Collé aux puces, il n'a plus besoin de le dire. Le bloc du haut configure un
générateur, le plateau en est la **sortie** ; séparés par une gouttière, trois
liens réels devenaient invisibles — la bannière de vivier explique le contenu du
plateau, `Tirer au hasard` et `Régénérer` font des choses voisines, et la
colonne « Force » réagit à un curseur hors de vue. `PLATEAU · N IA` est un
sous-titre à l'intérieur de la page, avec `Colonnes` et `Régénérer` sur la même
ligne.

**La hauteur de la table est plafonnée** à une dizaine de lignes, avec
**défilement interne** et ligne d'en-tête figée : la hauteur de la page cesse
ainsi d'être fonction du nombre d'IA — le cas normal d'une course GT3 en aligne
24. C'est le **seul défilement imbriqué autorisé** dans l'application : une table
de données est précisément le composant pour lequel cette convention existe.

> Piège de mise en œuvre, constaté à l'écran : la rangée d'en-tête porte aussi
> la classe des lignes, qui se pose en `position: relative`. À spécificité
> égale, c'est l'ordre dans la feuille qui tranchait — et il donnait `relative`,
> donc un en-tête qui sortait par le haut au bout de trois lignes.

**Il n'y a pas de mode « plateau élargi ».** Un bouton `⤢` a existé : il donnait
au plateau toute la largeur du contenu en faisant passer la colonne de droite
dessous. La page dédiée rend l'élargissement en place sans objet — la table a
déjà toute la largeur. Qui manque de place retire une colonne : le menu est à un
clic, et c'est le même geste que dans la bibliothèque.

**Ce qui ne migre pas sur cette page** : dégâts, carburant, usure, pénalités et
ligne idéale restent sur la page de réglages. Ce sont les règles de la course,
pas les adversaires.

**Le nom est plafonné** : sans quoi la largeur gagnée y va toute, et l'écart
entre lui et le nom de pilote devient assez grand pour qu'on perde la ligne en la
parcourant des yeux. La place restante va au vide en fin de ligne plutôt qu'à une
colonne arbitraire. `Restrictor` s'écrit en toutes lettres, seul des trois à ne
pas s'abréger : c'est le mot exact de la carte voiture du joueur, et c'est ce qui
fait voir que les deux réglages sont le même — `Nat.` et `Str.` n'ont pas ce
voisin et ne se confondent avec rien.

**L'écran est responsive, et centré.** La colonne de droite **passe dessous**
quand la largeur manque, plutôt que de se comprimer : en dessous d'environ 380 px
elle ne sait plus afficher la bande jour/nuit ni les quatre valeurs de l'état de
piste sur une ligne. Le contenu est plafonné et centré dans les deux régimes,
jamais collé à un bord — sans plafond, un curseur de difficulté long de 900 px a
une course souris disproportionnée pour une valeur qu'on pose au pourcentage
près, et une barre de filtres étalée ne ressemble plus à celle de la
bibliothèque. Le seuil est une requête de **conteneur** et non de média : ce qui
décide est la largeur réellement reçue par le corps de l'écran — le rail de
navigation et la colonne de session ont déjà pris la leur — et le zoom
d'interface déplace la largeur de la fenêtre sans rien changer à celle-là.

**Difficulté et agressivité : un centre et un écart**, et un seul composant
instancié deux fois — ce sont les deux réglages qui décident du caractère de la
course, ils ne peuvent pas se manipuler autrement l'un que l'autre. Le geste
fréquent est de monter tout le plateau de quelques points sans en changer la
dispersion : avec un minimum et un maximum il demande deux manipulations, et
rater l'une des deux resserre la fourchette sans qu'on s'en aperçoive. Déplacer
le centre conserve l'écart par construction. L'écart est un **champ** et non un
second curseur, parce que `± 0` — tout le plateau à la même force — est une
valeur qu'on veut poser exactement et qu'un pointeur n'atteint qu'à la bagarre.
Le libellé affiche la plage **bornée** (`3% ± 5 (0–8)`, jamais `(-2–8)`), et le
bornage s'applique aussi à ce qui part en jeu : sinon l'écran annonce une plage
et le jeu en reçoit une autre. Le stockage est centre + écart ; presets par type
et sessions enregistrées d'avant ce modèle sont **convertis**, jamais repliés
sur le défaut.

**Lest et bride du joueur** : dans la carte voiture du panneau gauche, sous le
repli `PERFORMANCE` (SESSION§1.2). Leur place n'est pas dans les options de session
parce qu'elles valent pour les quatre types, alors que tout le contenu de ce
bloc en dépend ; ça ne les sort pas de la configuration enregistrée pour autant.
Zéro s'affiche éteint, toute autre valeur en rouge, pour qu'un handicap oublié
se voie sans lire la ligne — et le résumé du repli le redit, la ligne affichant
toujours son état. Ils partent dans le preset des quatre types — au niveau de la grille pour une course, dans le
`ModeData` pour les modes solo, les deux emplacements relevés sur des presets
réels.

### 3.3 Les conditions — un bloc unique du rail droit

**Le rail droit porte les conditions de course, et rien d'autre**, identiques
dans les quatre types de session. C'est lui qui donne à l'écran sa silhouette
constante pendant que la colonne centrale grandit ou rétrécit. En Practice, où
cette colonne est courte, le rail porte l'essentiel du réglage — on est seul en
piste, la météo et l'état de la piste *sont* la session. Le déséquilibre est
donc un signal juste : rien n'est centré verticalement, aucun espace n'est
réservé, et le nombre de colonnes ne change pas avec le type.

**Un seul bloc `CONDITIONS`, un seul en-tête**, dans cet ordre : les huit tuiles
météo, la rangée air / piste / vent, l'état de piste, l'heure, la saison.
`ÉTAT DE PISTE` et `MÉTÉO` ont été deux cartes, et ce n'en a jamais été deux :
la première entrée de l'état de piste est « Auto (posé par la météo) », et une
entrée qui nomme sa voisine ne se lit que si cette voisine est sous les yeux.
Elles n'étaient pas voisines par commodité de mise en page — elles ne faisaient
qu'un. Rien ne s'intercale entre elles.

**L'heure et la bande jour/nuit sont un seul contrôle** : la bande est posée
directement sous le curseur, poignée alignée sur la sienne, et elle en est la
légende. La date affichée à sa droite a été retirée — elle doublait le champ
`Date` du bloc saison.

**Un select, et les quatre vraies valeurs sous lui.** Sept lignes nommées
coûtaient la hauteur du bloc météo pour un réglage qu'on choisit une fois, et
les quatre nombres qui décident réellement de l'évolution de la piste n'étaient
lisibles qu'au survol — c'est-à-dire nulle part. Ils s'affichent maintenant avec
le vocabulaire de Content Manager (`INITIAL GRIP`, `GRIP TRANSFER`,
`RANDOMIZATION`, `LAP GAIN`) : quelqu'un qui a réglé un état de piste là-bas doit
reconnaître ce qu'il lit ici. Lecture seule.

**Un état de piste est un objet nommé porteur de quatre valeurs, pas un cas
d'énumération.** L'écran ne connaît plus la liste, il la reçoit — et c'est ce
qui a permis d'y ajouter les presets de Content Manager sans rien changer à
l'écran. `Auto` n'est pas un état mais le drapeau `WeatherDefined` : ses quatre
valeurs sont celles de Green, sur lesquelles le jeu retombe quand la météo ne
dit rien de la piste, et **une seule phrase le dit** — la mention de repli, en
gris. La description que le jeu donne à cette entrée (« *Track state specified
by weather, or Green…* ») disait mot pour mot la même chose, en blanc, juste
au-dessus : deux phrases pour un même repli se lisaient comme deux règles.

> **Ses quatre valeurs sont celles de Green, `INITIAL GRIP` compris.** Le champ
> a porté 0 %, qui est la sentinelle du réglage hérité (`RaceSetup::grip`) et
> non un grip. Ça se voyait deux fois : la liste annonçait 95 % et l'écran
> affichait 0, et surtout — l'état voyageant désormais **entier** depuis
> l'écran (SESSION§3.4) — ce 0 partait tel quel dans le preset, donc une
> session « Auto » dont la météo ne disait rien de la piste roulait sur une
> piste à 0 % d'adhérence là où Content Manager écrit du vert. Ce qui identifie
> « Auto » est le drapeau `weather_defined`, jamais le grip ; la sentinelle
> reste dans `grip`, qui n'est plus relu que pour les configurations
> antérieures à ce modèle.

**Deux groupes, et les natifs ne sont jamais masqués.** Les presets d'état de
piste que l'utilisateur a créés dans Content Manager s'ajoutent **après** les
sept entrées du jeu, par ordre alphabétique, jamais à leur place : celles-ci
viennent de la table du jeu, ce sont les noms que tout le monde emploie, et
quelqu'un qui s'est fabriqué une piste verte humide veut quand même pouvoir
choisir `Optimum`. Le second groupe est simplement **absent** quand il n'y en a
aucun — pas de message, pas d'état vide, c'est le cas nominal. Lecture seule :
Pit Box n'écrit rien dans le dossier de CM, et n'ouvre pas l'édition des quatre
valeurs. Qui veut composer un état le fait dans CM et le retrouve ici.

**La lecture a lieu à l'ouverture de l'écran**, pas au démarrage de l'app : le
scénario réel est de créer un preset dans CM puis de revenir dans Pit Box, et
une lecture au démarrage obligerait à relancer.

**La description est la partie la plus utile du lot.** Elle s'affiche sous la
ligne des quatre valeurs, en prose (sans-serif) : ce sont les mots de
l'utilisateur pour décrire l'état qu'il a composé, et c'est exactement ce qui
manquait pour départager deux états proches. Les sept natifs en portent une
aussi, celle du jeu, affichée par le même chemin. Absente, rien n'est affiché.

**Le format est relevé sur un preset réel.** Un `.cmpreset` de
`…\Presets\Track States` est exactement l'objet d'état, JSON brut sans
en-tête — comme un `.cmpreset` de `Race Grids` est exactement l'objet
`RaceGrid` :

```json
{"s":0.89,"t":0.8,"r":0.03,"g":50,"d":"Old tarmac. Bad grip won't get better soon.","w":false}
```

`s`, `t` et `r` sont des pourcentages divisés par cent, `g` le `LAP_GAIN` brut.
C'est exactement ce que `build_track_properties` écrit déjà : **le lecteur en est
l'inverse**, ce qui est la meilleure garantie que les deux restent d'accord.

**L'échantillon est un duplicata de l'état natif `Old`, ce qui en fait un
témoin** : ses quatre valeurs doivent relire 89 / 80 / 3 / 50, les nombres que
`cfg/templates/tracks.ini` donne à cette entrée. L'échelle n'est donc pas
seulement supposée cohérente, elle est vérifiée contre des valeurs connues, et
un test rejoue le fichier verbatim.

**Les pourcentages s'arrondissent, ils ne se tronquent pas.** `0.29 × 100` vaut
`28.999999999999996` en flottant : une troncature rendrait 28, soit un état relu
un point plus glissant qu'il n'a été composé. Le fichier de référence n'expose
pas le défaut — `0.8` tombe du bon côté — donc rien n'aurait signalé sa
réintroduction, d'où un test dédié.

L'identité d'un état est son **nom de fichier** : c'est ainsi que CM lui-même le
désigne (`TrackPropertiesPresetFilename`), y compris pour les natifs —
`Optimum.cmpreset`, qui n'existe pourtant pas sur disque, les natifs étant
virtuels.

**Bornage volontairement large, et c'est un écart assumé.** Les plages de
l'éditeur de CM (grip initial 85-100, lap gain 0-700) n'ont pas pu être
confirmées. Un plancher non confirmé à 85 réécrirait en silence un état composé
à 70 — précisément ce que ce lot est censé rendre à l'utilisateur. On s'en tient
donc au sens physique : un pourcentage entre 0 et 100, un lap gain d'au moins 1.
Un fichier illisible ou incomplet est ignoré **en silence** : c'est un
enrichissement optionnel, il ne doit jamais empêcher de régler ni de lancer.

### 3.4 Ce qu'une session retient d'un état de piste

**Origine + nom, jamais le nom seul.** Rien n'empêche de nommer son preset
`Green`. À l'écran, l'appartenance au groupe suffit à distinguer ; au stockage,
non.

**Et les quatre valeurs résolues, en plus de la référence.** Ce qui part au jeu,
ce sont les nombres — le nom n'est qu'une étiquette. Trois conséquences, toutes
voulues :

- un preset supprimé ou renommé dans CM ne modifie **jamais silencieusement**
  une session déjà enregistrée ;
- **référence introuvable au chargement** : les quatre valeurs mémorisées sont
  appliquées telles quelles, et le select garde l'entrée sous son nom suivi de
  `(introuvable)` plutôt que de sauter en silence sur un autre état. Aucun
  blocage, aucune boîte de dialogue — la session reste jouable telle qu'elle a
  été réglée ;
- **référence trouvée mais valeurs différentes** : celles du preset gagnent.
  Modifier son preset dans CM est un geste intentionnel, et on attend que ses
  sessions suivent. L'autre branche se défend (la session prime, le preset n'est
  qu'un point de départ figé) ; rien dans l'implémentation n'a fait apparaître de
  raison de trancher autrement.

Le réglage n'était qu'un **pourcentage de départ**, qui servait d'identifiant :
ça ne tient plus dès que deux états peuvent partager le même. Un preset ou une
session d'avant ce modèle ne porte que ce pourcentage — l'état natif le plus
proche est repris, ce que faisait déjà l'ancien select.

### 3.5 Grilles enregistrées et import Content Manager

**Une grille et une session sont deux objets, et la distinction n'est pas une
nuance.** Une session enregistre tout — duo, météo, heure, options — dont une
**copie** de sa grille ; une grille n'enregistre que les adversaires et ce qui
fait le caractère du plateau (fourchette, agressivité), et c'est ce qui la rend
rejouable ailleurs : le même plateau GT3 sur dix circuits. La charger dans une
session déjà configurée ne touche donc ni à la météo, ni à l'heure, ni au type.

**Une copie, jamais un lien** : sinon modifier une grille changerait en silence
toutes les sessions qui la citent. C'est cette règle seule qui justifie les deux
objets. Liste plate avec recherche, pas d'arborescence — un dossier est une
taxonomie qu'il faut inventer et maintenir, un nom cherchable ne demande rien.
Fichier JSON dédié, jamais rangé par type de session : une grille ne dépend pas
du type de course qu'on fera avec.

**Import des presets Content Manager, jamais synchronisation.** On lit une fois,
on convertit en objet Pit Box, on ne dépend plus du fichier — une passerelle
vivante rendrait l'app dépendante du format de CM. Deux dossiers sont parcourus
récursivement, CM rangeant ses presets en arborescence : `Race Grids\`, dont un
`.cmpreset` est exactement l'objet `RaceGrid`, et `Quick Drive\`, qui l'enfouit
sous deux couches de JSON-dans-une-chaîne. La grille d'un preset de session
s'importe donc aussi.

**Seul `ModeId: "manual"` devient une grille.** Les autres modes de CM
(`same_car`, `similar_p_w_ratio`, `same_subclass_only`…) décrivent une **règle
de tirage** : leurs `CarIds` sont des candidats, pas des lignes, et en faire un
plateau inventerait ce que personne n'a composé. Ils sont nommés dans le
rapport — et Pit Box exprime déjà ces règles, en mieux, avec ses jetons.

**L'import aboutit toujours** : un fichier corrompu est nommé et n'arrête rien,
une voiture absente de la bibliothèque est retirée en le disant, une voiture
simplement désactivée reste et la garde d'activation la rallume d'un clic. La
proposition d'import et le bouton permanent vivent tous deux sur la page
d'import de mods ; un refus est définitif.

**Piège du format à ne pas réintroduire** : le même fichier mélange deux
écritures des nombres — les tableaux par ligne en **chaînes** (`"74"`), les
valeurs globales en **flottants** (`95.0`). Un lecteur qui n'accepte que les
entiers rend `None` sur les secondes, et la fourchette de difficulté d'un preset
importé retombe alors sur le défaut sans un mot.


**Practice** : pas de champ durée (non applicable — session à durée libre par design Quick Drive, voir SESSION§2 ; pas de champ correspondant côté Pit Box), ni tours/faux départ (absents du schéma `QuickDrive_Practice.xaml`, réservés à Course/Track day). Départ (Stand/Piste/Position de chrono → `StartType` du `ModeData`, trois valeurs) : "Piste" non vérifiée sur un preset réel, voir commentaire `PracticeStart`.

**Hotlap** : ghost car et son **avance**, sur une seule ligne — case et valeur
forment un contrôle, comme les deux phases de la course. `GhostCarAdvantage`
était codé en dur à 0 dans le preset, donc le réglage n'existait nulle part.

**Météo** : conditions en **icônes SVG stylisées** (thème, libre de droits) — Beau, Quelques nuages, Couvert, Brouillard, Pluie légère, Pluie, Orage. **Température, vent et heure implicites** sur une même ligne (heure modifiable, température/vent recommandés par condition + heure + stack SOL/CSP, tous corrigeables à la main). **Saison** optionnelle : un champ date natif (en premier, avant les 4 cartes saison) qui affiche/permet de corriger précisément la date associée — sélectionner une saison y reporte automatiquement la date calculée (milieu de saison), la modifier à la main ne désélectionne pas la saison affichée.

**Heure de session** : curseur sur la journée entière (une session de nuit est légitime, CSP/Sol gèrent l'éclairage), **au pas de 10 minutes** — au bord d'un coucher, le ciel change complètement en dix minutes, et une demi-heure passe à côté de la lumière qu'on vient chercher. Dernier cran à 23:50, pas de 24:00.

**Bande jour/nuit sous le curseur d'heure**, **datée** : la date effective est écrite au-dessus de la bande, jamais seulement en infobulle — sans saison choisie le champ date reste vide alors que le jeu, lui, prend la date du jour, et un champ vide laisserait croire qu'aucune date ne s'applique. Le dégradé du ciel sur ce circuit — nuit, crépuscule civil, plein jour — avec deux repères **cliquables** portant l'heure exacte du lever et du coucher (les poser d'un clic, ce qu'aucun pas de curseur ne permet d'atteindre au hasard). Rien n'est inventé : ce sont les entrées que **CSP** lit lui-même (`src-tauri/src/sun.rs`).

- **Position et fuseau** : `extension/config/data_track_params.ini`, une section par identifiant de circuit (`LATITUDE`, `LONGITUDE`, `TIMEZONE=Europe/Berlin` — pas d'entrée par tracé, un layout suit son circuit). Repli sur les `geotags` du `ui_track.json` du mod pour un circuit que CSP ne connaît pas ; le fuseau est alors approché d'après la longitude, et le survol le dit. Les `geotags` des circuits Kunos valent littéralement `["lat", "lon"]` : ce ne sont pas des coordonnées, et ils sont ignorés comme tels. Sans position nulle part, pas de bande — jamais une bande fausse.
- **Fuseau, pas longitude** : l'heure d'un lever se lit sur l'horloge locale du circuit, heure d'été comprise (`chrono-tz`). Un simple `longitude / 15` placerait Barcelone à UTC+0, soit deux heures d'erreur en été — la même classe d'erreur que celle qu'on affichait sur la date des sessions enregistrées.
- **Quelle date le soleil suit** : `[SEASONS] ALLOW_ADJUSTMENTS` de `track_adjustments.ini` (valeur utilisateur dans `Documents/Assetto Corsa/cfg/extension/`, défaut livré dans `<AC>/extension/config/`), dont le fichier livré documente lui-même les valeurs — `0` « jamais, trajectoire de plein été », `0.5` « jamais, trajectoire réelle », `1` (défaut) « avec une date posée », `2` « toujours, date du jour à défaut ». **Seul `0` détache le soleil du calendrier** ; toutes les autres valeurs suivent la date que la session porte (celle que Pit Box écrit en `udt`/`dtv`, SESSION§3.3), et **la date du jour** à défaut. Ce dernier point est mesuré en jeu, pas déduit : le libellé de `1` laisse croire que sans date rien ne s'ajuste, mais à Barcelone sans saison, le 29 août, le soleil s'est levé à 07:14 — le lever de ce jour-là, pas celui du solstice (06:17). Voir le commentaire de `effective_date`.
- **Le script météo n'y change rien** : CSP calcule la direction du soleil nativement, Sol et Pure ne font qu'y réagir — les deux installs donnent le coucher à la même minute.

**Presets de session par type** : chaque type (Practice/Hotlap/Course/Track day) a un preset mémorisé ; toute modif est persistée pour les prochaines sessions du même type. **Persistance** (`src-tauri/src/session_state.rs`, `app_config_dir/launch_state.json`) : fichier écrit côté Rust, même mécanisme et même raison que le duo de session (§7.4) — `localStorage` n'est pas garanti synchrone sur disque côté WebView2, ce qui pouvait perdre les presets et la dernière sélection (type de session, adversaires) à la fermeture de l'app. Fichier dédié, distinct de `session.json` (chaque commande réécrit tout son fichier ; les mélanger ferait que sauvegarder le duo de session écrase les presets, et inversement). Migration silencieuse au premier démarrage après la mise à jour, même schéma qu'en §7.4. **Une écriture ratée ne passe pas inaperçue** (règle d'or n°6) : elle est réessayée une fois avec l'état le plus récent — jamais avec l'état qui a échoué, qui pourrait écraser une écriture plus récente réussie entre-temps —, journalisée côté Rust (`warn`), puis signalée par la même notification que `ui_prefs.json` (« Vos réglages ne sont pas enregistrés »), avec sa propre phrase et sa raison technique.

**Sessions enregistrées : deux boutons dans la barre de titre, et une modale.**
Enregistrer et recharger une configuration nommée est le même geste pour une
grille et pour une session ; les deux partagent donc le même composant. Le
placement suit une règle générale : *l'action se place au niveau de ce qu'elle
enregistre* — `Enregistrer la grille…` en bas de la grille, `Enregistrer la
session…` en en-tête d'écran. Pas à côté du type de session, ce qui suggérerait
que la sauvegarde y est rattachée alors qu'elle porte sur toute la page. Le
décompte est porté par le bouton, qui est ce dont il parle.

**Le filtre par type est supprimé, parce qu'il était invisible.** Qui avait
enregistré une session en Course et la cherchait depuis Practice ne voyait pas
une liste filtrée : il voyait une liste vide, et en concluait que sa sauvegarde
avait échoué. Charger une session **bascule** le type — il fait partie de ce qui
est enregistré —, donc la charger depuis un autre type est valide et il n'y
avait rien à masquer. Le bénéfice du filtre passe dans le **tri** : type courant
en tête, rien de caché. Le type devient une propriété affichée, à côté du
circuit et de la date. Si un filtre explicite devenait nécessaire, il prendrait
la forme d'une puce visible et effaçable — jamais d'un masquage silencieux.

**Un nom, un fichier** : une sauvegarde est un `.cmpreset` (SESSION§3.6), donc
la clé `<type>::<nom>` d'avant a disparu avec le fichier unique qui la portait.
Deux types ne peuvent plus avoir une sauvegarde du même nom — enregistrer
« Test » en Practice remplace le « Test » de Course, après la confirmation
d'écrasement que la modale demande déjà. C'est le prix du format, et il est
assumé : un fichier que Content Manager affiche doit porter le nom qui a été
tapé, pas `race::Test`. La suppression, elle, lit toujours l'entrée et jamais
l'écran, puisque la liste n'est pas filtrée.

**Contenu d'une session enregistrée** : les réglages (météo, adversaires, options) **et le duo de session** — voiture pilotée avec son skin, circuit avec son tracé et ses **skins de circuit actifs** (§8, seul élément hors `setup` : c'est un état de déploiement, d'où un champ `trackSkins` à part). Le chargement rétablit le tout en passant par le duo de session (SESSION§1), qui reste la source de vérité : voiture et circuit sont reposés via `pickSession`, pas écrits directement dans le setup. Les skins de circuit sont remis **à l'identique** — ceux qui manquent sont activés, ceux en trop désactivés, sinon un skin resté actif d'une session précédente changerait l'apparence du circuit sans que rien ne le signale. Une sauvegarde antérieure au champ `trackSkins` (`undefined`, distinct d'une liste vide) n'y touche pas du tout.

**Chargement partiel** : un mod supprimé depuis la sauvegarde n'interrompt jamais le chargement — la sélection courante est conservée pour ce qui manque (voiture, circuit), le tracé retombe sur celui par défaut, et un **bandeau d'avertissement jaune** en tête d'écran (même emplacement que le retour de lancement, jamais une popup : il n'y a rien à décider) énumère ce qui n'a pas pu être rétabli. Il s'efface au chargement suivant et au lancement de la session.

**Écran non prêt** : le corps de l'écran (colonnes de réglages) reste masqué derrière `LoadingState` (même indicateur que les listes de mods) tant que le chargement initial n'est pas terminé (bibliothèque, presets/sélection persistés, duo de session, météo par défaut) — évite que l'utilisateur voie les champs se réajuster au fur et à mesure que les valeurs mémorisées arrivent.

**Choix layout/skin de circuit** : sur la fiche/bibliothèque circuit, image d'aperçu (`preview.png`) avec le tracé du layout (`outline.png`/`map.png`) par-dessus, infos (longueur, virages, CSP).

### 3.6 Une session enregistrée est un preset Content Manager

**Le format de sauvegarde est celui de CM**, `.cmpreset`, écrit dans son propre
dossier : `%LocalAppData%\AcTools Content Manager\Presets\Quick Drive\Pit Box\`.
Une session réglée dans Pit Box se lance donc aussi depuis Content Manager,
sans export ni conversion — et les presets que l'utilisateur a composés dans CM
apparaissent dans la liste de Pit Box. Les deux applications cessent d'être deux
mondes.

Le fichier est produit par **le constructeur du lancement lui-même**
(`quickdrive::preset_value`, SESSION§2) : ce qui est enregistré et ce qui est
lancé ne peuvent pas diverger, parce qu'il n'y a qu'un producteur.

**Le bloc `PitBox`, et pourquoi un aller-retour ne suffit pas.** Le schéma Quick
Drive ne porte pas tout : **le skin du joueur n'y a aucun champ** (mesuré, pas
supposé — SESSION§2), ni l'intention météo, ni les skins de circuit actifs, ni
les jetons du vivier, ni la tenue du pilote. Relire un preset perdrait donc
exactement ce sur quoi on a passé du temps. Nos fichiers portent en plus une
clé de premier niveau `PitBox` contenant l'instantané complet, et c'est **elle
seule** qui est relue : aucun aller-retour dégradant. Content Manager ignore les
clés qu'il ne connaît pas, le fichier reste un preset parfaitement ordinaire
pour lui. S'il réécrivait un de nos fichiers et emportait le bloc, rien ne
casse : le fichier se relit alors comme n'importe quel preset de CM.

**Les presets de Content Manager sont listés, avec un badge.** Ils se chargent
au mieux de ce que le format porte. Ce qu'il ne porte pas est **dit** dans le
bandeau jaune du chargement partiel, jamais deviné : le skin du joueur d'abord
(la session garde celui en place, exactement ce que fait CM), et le plateau
quand le preset **tire** ses adversaires au lieu de les énumérer — seul
`ModeId: "manual"` est un plateau, même règle qu'à l'import des grilles
(SESSION§3.5). La température de piste ne revient pas non plus : `crt` dit
qu'une valeur a été posée, jamais laquelle.

**Les trois modes sans équivalent** — Drag, Drift, Time attack — sont **nommés**
dans la liste, en ligne grisée avec leur raison, pas masqués : qui a dix presets
et en voit huit se demande lesquels ont disparu. Ils ne comptent pas dans le
décompte du bouton, qui ne promet que ce qui se charge.

**Un preset qu'on n'a pas écrit ne se supprime pas** — la croix ne s'affiche que
sur les nôtres, et le backend refuse de toute façon un fichier sans bloc
`PitBox`. C'est le corollaire de la règle sur les fichiers du jeu, appliqué au
dossier de CM.

**Sans Content Manager**, le dossier n'existe pas : les presets vont alors dans
`app_config_dir/Quick Drive/Pit Box/`, même format, même code. Enregistrer une
session ne dépend pas de CM, seul le partage avec lui en dépend.

**Migration** : `saved_sessions.json` est relu une fois, chaque entrée réécrite
en preset, puis le fichier renommé — une session supprimée depuis ne ressuscite
pas au démarrage suivant. Une entrée dont le `setup` ne se relit plus est
journalisée et sautée, jamais bloquante.

## 4. Aperçu 3D des voitures

Deux aperçus 3D **coexistent**, parce qu'ils ne rendent pas le même service.

**Aperçu intégré à la fiche** (docs/SPEC-preview-3d-kn5.md). Le modèle `.kn5` de la voiture est lu, converti en glTF et affiché **dans la zone héros**, à la place de la photo. La voiture se **reflète au sol**, floutée et s'éteignant à courte distance, comme sur le sol laqué d'un salon. La voiture **tourne lentement sur elle-même**, comme sur un socle de salon (un tour en ~28 s) : c'est la présentation par défaut, l'orbite et le zoom à la souris restent disponibles par-dessus. À l'apparition du modèle, le plateau peut s'ébranler en douceur ou partir lancé puis ralentir, selon le réglage (§11). Le plateau s'arrête dès qu'on attrape le modèle et **ne repart pas tout seul** : c'est le bouton de remise en place qui le relance. Il repartait après quelques secondes d'inactivité, et c'était une gêne plutôt qu'un service — on règle un cadrage en regardant la voiture, et elle se remettait à tourner sous les doigts. Il s'arrête aussi quand la fiche quitte l'écran ou que la fenêtre passe en arrière-plan, et ne démarre pas du tout si le système demande de réduire les animations. Cadrage trois-quarts avant calculé sur les dimensions réelles du modèle. Actif par défaut sur les fiches voiture ; une bascule discrète en bas à droite du héros permet de revenir à la photo, et ce choix est mémorisé. Pendant la préparation, **rien d'autre n'est affiché** : la zone reste vide avec un témoin de chargement au centre. La photo servait de patience, mais montrer une voiture pour en montrer une autre trois secondes plus tard fait deux images là où on en attend une ; elle reste, entière, quand la 3D ne peut pas aboutir. Un **changement de skin**, lui, garde le badge discret en haut à droite : le modèle précédent est toujours à l'écran et c'est lui qu'on regarde. Si le modèle est introuvable, protégé (KN5 chiffré) ou si la machine n'a pas de WebGL, l'aperçu retombe **silencieusement** sur la photo — badge discret « aperçu 3D indisponible » seulement quand il y a une raison à donner. Ce n'est jamais une erreur bloquante : c'est un bonus visuel. Le skin sélectionné sur la fiche est appliqué au modèle. **Changer de skin ne repasse pas par la photo** : le modèle en place continue de tourner pendant que le nouveau se prépare, et celui-ci reprend le plateau et la caméra exactement où le précédent les avait laissés — la voiture change de peinture sans interrompre sa rotation. Si le nouveau skin ne peut pas être converti, l'ancien modèle reste à l'écran avec sa peinture précédente plutôt que de retomber sur la photo. **Les mods de préparation qui étendent leur voiture par CSP sont assemblés avant affichage** : beaucoup livrent un `.kn5` volontairement incomplet et rangent ailleurs les pièces qui changent d'un skin à l'autre — jantes, boucliers, optiques. Ces pièces sont greffées sur le modèle en lisant l'`ext_config.ini` de la voiture et celui du skin, si bien que l'aperçu montre la voiture telle qu'elle roule et non trouée (détail et limites dans PREVIEW§4.5ter). La conversion est mise en cache sur disque : le deuxième affichage d'une même voiture est immédiat. **Une couche activée ou retirée rafraîchit tout** — modèle, liste des skins et vignettes : elle recompose `content/` sans changer un seul chemin, si bien que le navigateur resservait ses images en cache et que l'aperçu, ne voyant bouger ni la voiture ni le skin, gardait le modèle précédent. Le skin regardé se retrouve alors par son identité et non par son rang, faute de quoi il désigne une autre livrée dès que la couche en ajoute ou en retire. **Le pilote peut être affiché au volant** (option de l'écran Réglages à trois valeurs — toujours, au démarrage du moteur, jamais — le milieu par défaut) : AC range un pilote en trois morceaux — le mannequin 3D nommé par la voiture, la tenue (combinaison, gants, casque) nommée par le skin, et la place assise donnée par la ligne `DRIVEREYES` de la voiture — et l'aperçu les assemble. **Au démarrage du moteur**, il s'installe en fondu quand une clé de contact tourne sur la fiche et repart quand elle se coupe : il ne marche pas jusqu'à la voiture, la conversion le porte déjà et seule la vue le montre ou le retire. La pose et la place assise viennent du moddeur, qui les a réglées pour ce cockpit-là — rien n'est calculé, donc rien ne s'adapte à un volant redimensionné après coup. Le pilote entre dans la clé de cache, donc passer de « jamais » à l'un des deux autres demande une conversion, une seule. Le pilote qu'on voit au volant — son corps et sa tenue — se choisit sur son propre écran (SESSION§5).

**Le braquage est réglable, et il tourne la voiture entière** : les roues avant, le volant du poste de pilotage et, quand il y en a un, les bras du pilote — que l'animation de braquage embarquée par la voiture pose au même angle. Le réglage est gradué **aux roues** (±35°), pas au volant : c'est ce qui se voit sur une voiture à l'arrêt. Le volant tourne d'autant plus que la voiture est démultipliée et s'arrête à la course qu'elle déclare ; les roues, elles, vont où le réglage demande — la course déclarée par AC est le débattement utile d'un volant de simulation, pas la butée mécanique, et s'en servir laissait une GT3 quasi droite. Le réglage vaut sans pilote comme avec : ce sont les roues de la voiture qu'il tourne. Le braquage est **gratuit et instantané** : la conversion ne le cuit pas dans le modèle, elle décrit ce qui tourne — pivot, axe, démultiplication — et la vue applique l'angle. Il n'entre donc pas dans le cache. Les bras du pilote suivent aussi : le mannequin est exporté **vivant** dans le modèle converti — squelette, peau et animation de braquage — donc poser ses bras revient à choisir une image, sans rien reconvertir. Plus rien, pilote compris, ne fait de l'angle une entrée de cache (détail dans PREVIEW§4.6bis).

**Aperçu natif** (bouton de la fiche, lance `acshowroom`) — conservé pour le rendu fidèle du jeu, que l'aperçu intégré n'imite pas. Le showroom est un **process indépendant**, affiché par-dessus l'app avec les réglages vidéo du jeu : l'utilisateur le ferme lui-même pour revenir à Pit Box. L'intégration de sa fenêtre dans la page a été tentée puis abandonnée (voir `showroom-3d-preview-research.md`). **Option de réglage** : le décor (`content/showroom/<id>`) chargé par `acshowroom`, choisi parmi ceux installés — défaut `studio_white`, le seul instantané et sans musique. Pendant le démarrage d'`acshowroom`, afficher une **animation de chargement en haut à droite** de l'image.

---

## 5. Écran Pilote

Choisir le pilote qu'on voit au volant : son **corps** (le mannequin 3D) et sa **tenue** en trois pièces — casque, combinaison, gants. Spécification complète dans `SPEC-ecran-pilote.md`, maquette dans `maquettes/pitbox-ecran-pilote.html`.

**Deux natures d'objet, deux comportements**, et cette asymétrie structure l'écran. Le corps est un modèle 3D que la voiture désigne dans `data.acd`, le conteneur qu'un serveur de course vérifie ; il ne se remplace donc pas en y touchant, mais par une surcharge CSP posée à côté (voir plus bas). Le casque, la combinaison et les gants ne sont que des images posées dessus, que la livrée choisit déjà — les choisir à sa place ne demande qu'un fichier de skin. D'où le corps au-dessus, séparé, et les trois autres en dessous : le corps **commande**, la tenue en découle.

**Point d'entrée** : une ligne dans la colonne de session, sous le sélecteur de livrée, qui dit ce que le pilote porte — *Tenue d'origine* quand rien n'est touché, le **nom de la tenue enregistrée** quand on en porte une, *Tenue personnalisée* sinon — plus un badge *Substitué* quand le corps n'est pas celui de la voiture. Un clic ouvre l'écran, jamais plus. Cette ligne remplace la bascule et les trois menus déroulants qui occupaient la colonne auparavant.

**L'écran** se partage en deux : un panneau d'essayage fixe à gauche — le pilote, puis les quatre pistes et la sortie — et une galerie défilante à droite, qui montre les options de la piste active. Le geste central est le **survol** : parcourir la galerie applique chaque option sur le pilote affiché, cliquer l'adopte. C'est la seule façon de juger une tenue, une texture à plat ne disant rien du résultat. La vignette d'une case est l'image plate fournie par le jeu, assumée comme telle : elle sert de repère pour retrouver et comparer, pas de promesse de résultat.

**Le choix se fait par voiture**, et c'est un revirement assumé sur la spec de l'écran, qui le voulait global. L'intention — se reconnaître d'une voiture à l'autre — était bonne, la conséquence ne l'était pas : un casque choisi une fois s'imposait à tout le parc, sur des voitures que l'utilisateur n'avait jamais ouvertes et sans que rien le lui dise. Une cascade à trois niveaux le remplace : **la tenue choisie pour cette voiture** gagne toujours ; sinon **la tenue par défaut de sa classe** — l'une des tenues enregistrées, désignée par l'utilisateur ; sinon **la livrée**. Il y a **deux tenues par défaut, pas une** : une pour les voitures de rue, une pour les voitures de course, parce que le mot n'y a pas le même sens — sur une voiture de course la tenue fait partie de la livrée, et beaucoup voudront la lui laisser tout en s'habillant sur une routière. Laisser « Aucune » d'un côté rend ces voitures-là à leur livrée. Tout ce qui n'est pas annoncé `race` compte comme rue, y compris les mods qui ne renseignent aucune classe : le défaut sûr est celui où la livrée n'a rien prévu. Activer l'option n'écrase donc jamais un choix fait à la main, et la désactiver ne perd rien. La bibliothèque des voitures porte un filtre à trois états « Pilote choisi », pour retrouver d'un coup d'œil celles qu'on a réglées.

**Le pilote choisi est posé en jeu au lancement de la session**, et retiré dès qu'on ne le choisit plus. Deux fichiers, aucun dans `data.acd` : **le corps** par une section `[DRIVER3D_MODEL]` dans l'`ext_config.ini` de la voiture — CSP surcharge une section de `data.acd` en préfixant le nom du fichier, sans toucher le conteneur —, **la tenue** par le `skin.ini` de la livrée, sous le nom du mannequin. Conséquence à ne pas rater : remplacer le corps rend la section de la livrée inopérante, la tenue est donc réécrite sous le **nouveau** nom de mannequin. Trois disciplines, chacune obligatoire : l'original est sauvegardé avant écriture et rendu à la reprise (§4.5.4) ; seule la section concernée est remplacée, le reste du fichier est conservé au caractère près (735 lignes d'`ext_config.ini` sur une NSX de l'installation) ; et le fichier est **effacé avant d'être réécrit**, sans quoi c'est la copie de bibliothèque qu'on modifierait à travers le hardlink. Voir `csp-driver-research.md`.

**Le plateau d'essayage montre le pilote en 3D**, seul, sans habitacle autour, mais **posé comme sa voiture le pose** : sa hiérarchie d'assise et son animation de braquage sont appliquées, seul l'ancrage sur les yeux tombe. Sans elles, le mannequin garde sa pose de modélisation, bras écartés de 55 cm — ce qui ressemble à une prise de volant sans en être une. Avec elles, l'écart tombe à 35–43 cm selon la voiture : le diamètre de son vrai volant. **Aucun volant n'est dessiné.** L'idée d'un tore générique posé entre les mains a été essayée deux fois — sur les poignets, puis sur les phalanges 13 cm plus en avant, ce qui le met pourtant au bon endroit — et abandonnée : elle ne convainc pas à l'écran, et le pilote seul se lit mieux qu'un pilote accompagné d'un objet approximatif. Ce qu'on essaie est une tenue, pas un habitacle. Le cadrage suit la piste active — plan large sur le corps, tête sur le casque, buste sur la combinaison, mains sur les gants — et **ne bouge jamais au survol** : une caméra qui se déplace pendant qu'on compare rend la comparaison impossible. Glisser fait pivoter le pilote, double-cliquer le remet de face.

**Adopter se voit.** Le survol échange une texture — instantané, rien à signaler —, mais adopter demande une conversion complète au backend, le temps de laquelle le mannequin précédent reste affiché. Un badge discret avec le même serpent que l'aperçu 3D d'une voiture le dit : sans lui, le clic ne produit rien de visible pendant une seconde et on clique une deuxième fois.

**Deux vitesses, et c'est ce qui rend le survol utilisable.** Habiller un mannequin côté backend est une conversion complète — une bonne seconde. Le survol ne passe donc jamais par là : la texture est échangée sur place, dans la scène déjà chargée, avec le `.jpg` qu'Assetto Corsa range à côté de chaque `.dds` de garde-robe — la même image aux mêmes dimensions, et déjà la vignette de la galerie. L'adoption, elle, redemande le modèle au backend et rétablit le rendu exact. Un essai est un essai : il ignore la légère teinte que la conversion applique. Le corps fait exception — ce n'est pas une texture, le changer demande une vraie conversion, donc seule son adoption la déclenche, le corps précédent restant affiché pendant ce temps. **Jamais de plateau vide.**

**La galerie des corps porte des vignettes en 3D**, faute d'échantillon plat qui veuille dire quelque chose : la texture de casque d'un corps est partagée par tous ceux de la même époque, sa combinaison par tous les corps — deux mannequins très différents donneraient la même case. Ce qui les distingue est leur géométrie, donc on la rend : un buste, du haut du casque à la poitrine, dans le même éclairage que le plateau. **Produites à la demande, une à la fois, et seulement pour ce qui entre dans le champ de vision** — il y en a 45 sur l'installation de référence et chacune coûte une conversion la première fois. Une vignette ne périme jamais le chargement du plateau, ni une autre vignette. **Le PNG rendu est conservé, hors du plafond du cache d'aperçus** : sans ça les 45 conversions écrivent 180 Mo dans un pool déjà plein, chacune évinçant une entrée plus ancienne — y compris les mannequins des vignettes précédentes, qu'il faut alors reconvertir à la visite suivante. Une vignette rangée à part ne demande son `.glb` qu'une fois dans la vie d'un corps. Elle porte le nom de l'entrée de cache du mannequin, donc elle se périme quand le mod change, sans invalidation à écrire.

**Si la 3D ne démarre pas**, le plateau affiche l'échantillon plat de la pièce retenue en grand et le dit en une ligne. La galerie et la sélection restent pleinement fonctionnelles : l'écran ne se bloque jamais sur l'absence de 3D.

**Ce qu'on peut porter n'est ni deviné ni codé en dur** : un dossier de garde-robe est retenu s'il contient une texture que le corps échantillonne comme couleur de base. Cette règle unique donne la bonne réponse pour les trois listes — combinaisons et gants passent partout, parce que les mannequins Kunos réclament tous les mêmes fichiers, tandis que les casques se filtrent d'eux-mêmes par époque. Un corps de mod qui nomme ses images à lui n'a donc aucun casque à proposer : l'écran l'énonce, plutôt que de proposer un choix sans effet. Le filtrage par époque est **automatique et non désactivable**, et le compteur en dit toujours la cause, jamais le filtre seul.

**Les corps sont rangés par pack, toujours, et sans commande pour en sortir.** Le pack est ce que le nom de fichier porte avant le premier `_` — seul regroupement que les données portent, un mannequin n'ayant ni fiche ni marque : `rss_*` (13 corps sur l'installation de référence), `rh_*` (5), `gt-m_*` (5). Le préfixe `driver` est écarté : tous les mannequins sont des pilotes, et les quinze fichiers qui commencent ainsi sont la famille livrée avec le jeu — ils retombent sur leur **époque** (celle des casques qu'ils acceptent), qui les sépare utilement là où « driver » les entasserait sous une étiquette vide. Chaque groupe porte ainsi un nom qui veut dire quelque chose, qu'il vienne d'un pack ou d'une époque. **Un groupe de moins de trois cases rejoint « Autres »** plutôt que de coûter une bannière pour une case. **Aucune piste n'offre plus de bascule de regroupement**, ni les corps ni les tenues : l'époque seule, le pack et la grille plate ont existé comme variantes, retirées après essai. Personne ne les rouvrait, et une grille plate n'est pas praticable passé quelques dizaines de cases — la barre d'outils a mieux à faire de sa place.

**Les corps qu'on ne peut pas prendre ne sont pas montrés** : un mannequin illisible, ou sans squelette — les variantes de LOD basse définition sont dans ce cas — n'apparaît pas dans la galerie, sans message. Une option qu'on ne peut pas choisir n'a rien à y faire.

**Substituer le corps supprime la référence « livrée »** : la tenue du skin est écrite sous le nom de l'ancien corps et n'a plus de destinataire. Une bannière unique, en tête de galerie, énonce ce qui tombe au moment où ça tombe — jamais trois messages séparés, et jamais une puce dont l'objet n'a pas réellement été perdu. La sortie du panneau change de sens avec le mode : elle remet tout sur la livrée tant que le corps est celui de la voiture, elle rétablit le corps sinon. Un seul bouton, un seul chemin de retour.

**Aucune voiture n'est mise à part.** Un écran intermédiaire prévenait autrefois que sur une voiture de course la tenue appartient à la livrée, et proposait de passer outre. Il est retiré : le corps s'y pose comme ailleurs, son texte était devenu faux, et un avertissement qu'un clic franchit sans conséquence n'avertit de rien.

**Le plateau se manipule à la souris comme l'aperçu d'une voiture** : glisser à l'horizontale fait pivoter le pilote, glisser à la verticale monte et descend le long de lui, la molette rapproche et éloigne, un double-clic remet de face et rend le cadrage d'origine. Les deux axes du glissé n'agissent pas sur le même objet : à l'horizontale c'est le **plateau** qui tourne (l'éclairage reste du côté du spectateur), à la verticale c'est la **caméra** qui se translate — la faire pivoter en site l'aurait fait plonger sur le crâne ou remonter sous le menton, alors qu'on veut passer du casque aux gants sans changer de point de vue. Le zoom s'applique **par-dessus** le cadrage de la piste et lui survit : se rapprocher une fois vaut pour les quatre pistes. Dans la galerie, les **flèches** parcourent les cases — le focus valant survol, elles essaient au passage — et la case visée est choisie par sa géométrie, ce qui traverse les groupes comme le fait l'œil plutôt que de suivre un index qui sauterait en biais.

---
