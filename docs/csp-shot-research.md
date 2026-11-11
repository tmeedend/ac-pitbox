# Vignettes des voitures protégées — preuve de concept du rendu dans le jeu

2026-11-10, branche `poc/vignettes-dans-le-jeu`. Lot 1 du découpage : une app
Lua CSP, `ac.GeometryShot`, une voiture protégée, un PNG enregistré, le jeu
refermé. Critère de go/no-go : l'écart d'éclairage avec le rendu Pit Box.

## Ce qui a été construit

- `src-tauri/resources/csp/pitbox_shot/` — l'app Lua (`LAZY = NONE`, inerte
  sans fichier de tâches). Elle lit `Documents\Assetto Corsa\pitbox_shot\job.json`,
  attend 8 s après le chargement de la session, rend chaque voiture du lot hors
  écran sous plusieurs éclairages, écrit les PNG et un `done.json`, puis appelle
  `ac.shutdownAssettoCorsa()`.
- `scripts/poc-csp-shot.ps1` — outil de développeur, jamais livré : pose l'app
  par une **jonction** dans `apps\lua` (aucun fichier du jeu écrit ni remplacé ;
  `-Remove` la retire), écrit le fichier de tâches et lance une séance d'essais
  Quick Drive par CM (`acmanager://race/quick?presetFile=`), départ au stand.
  CM reste maître du `race.ini`.

Essayé sur CSP 0.3.2-preview2 (build 4305), sur `ddm_honda_s2000_ap1`
(protégée) et `ks_toyota_supra_mkiv_drift` (non protégée, comparée à sa
vignette Catalogue Pit Box). Trois tours, cinq sessions.

## Ce qui est tranché

| Question du document de départ | Réponse |
| --- | --- |
| Le jeu rend-il une voiture protégée hors écran ? | **Oui**, entière, intérieur compris, au premier essai. |
| Fond transparent | **Oui** : `setClearColor(rgbm(0,0,0,0))` + `setOpaqueAlphaFix(true)` + `setTransparentPass(true)` (vitres). |
| Résolution indépendante de l'écran | **Oui** : 2048 × 1152 rendus, le canevas ne dépend pas de la fenêtre. |
| Quitter le jeu depuis Lua | `ac.shutdownAssettoCorsa()` : le jeu se ferme proprement, CM aussi rend la main. |
| Boîte englobante | `car.aabbCenter` / `car.aabbSize` (LOD D ou collisionneur, repère du modèle) : juste à quelques cm. **Pas** `carRoot:getLocalAABB()`, qui descend 1,2 m sous le sol (un nœud caché sous la voiture) et laissait la voiture petite et haute. |
| Même cadrage que Pit Box | **Oui**, à l'œil : même formule que `framing.ts` (orbite ajustée sur les huit coins), même angle, même taille dans le cadre. Le décentrage de l'objectif passe par les matrices brutes de `GeometryShot:update(view, proj, pos)` ; l'entrée qui porte le décalage (`row3.x`) a été trouvée par sondage. Pas d'effet miroir : la voiture regarde du même côté que chez Pit Box. |
| Voiture immobile | Départ au stand : 0,0002 km/h, pas besoin de serrer le frein. **Mais la portière conducteur est ouverte** à l'arrêt au stand : `ac.setDriverDoorOpen(i, false, true)` la ferme. |
| LOD | `activeLOD = 0` après 8 s, sans rien forcer. |
| Où Lua peut écrire | Partout où on a essayé : `Documents\Assetto Corsa`, `%LOCALAPPDATA%\com.pitbox.app`, `%TEMP%`. Pit Box pourra donc faire écrire directement dans son cache. |
| Durée | ~9 s de session pour une voiture et quatre variantes, plus le lancement par CM (~30 s). |

## Ce qui ne l'est pas : l'éclairage

C'était annoncé comme le point faible, et ça l'est.

- **L'éclairage neutre de `GeometryShot`** (celui par défaut) brûle les hautes
  lumières et bleuit les peintures claires : un reflet en dégradé
  blanc → noir, fort, qu'on n'éteint pas. Faire varier la lumière ambiante de
  1,5 à 0,6 ne change presque rien à l'image ; le reflet domine.
- **`setBestSceneShotQuality()`** éclaire la voiture avec le soleil de la piste :
  `setAmbientColor` n'a alors plus d'effet (1,5 et 0,9 donnent la même image),
  et `setOriginalLighting(true|false)` non plus. Couleurs saturées, l'orange
  vire au jaune.
- **YEBIS** (le post-traitement complet) recompose la scène floutée derrière la
  voiture et perd le fond transparent. **SimplifiedYEBIS** tonemappe mais rend
  la voiture à moitié transparente.

À côté de la vignette Pit Box de la même Supra, aucune variante n'est
« indiscernable » : la nôtre est mate et retenue, celle du jeu brillante et
saturée. Dans une grille mixte, les voitures protégées se reconnaîtraient.

## Tour 4 : HDR, tonemapping maison et reflets du showroom

Diagnostic des tours 1 à 3 : le PNG contenait les valeurs HDR brutes, écrêtées
à 1, sans tonemapping ni encodage sRGB — d'où les hautes lumières brûlées et
l'orange devenu jaune. Ce que fait YEBIS d'ordinaire, mais YEBIS perd la
transparence.

Ce tour rend donc en texture flottante (`R16G16B16A16.Float`), puis applique
dans le jeu, par un shader (`ExtraCanvas:updateWithShader`), la même courbe que
Pit Box (`NeutralToneMapping` de three.js) et l'encodage sRGB. La transparence
est conservée, l'exposition se règle (vérifié : ×0,6 / ×1 / ×1,6 changent bien
la luminosité moyenne).

- **Les reflets du showroom du jeu** (`overrideReflectionCubemap('menu')`) sont
  ce qui change tout : orange crédible, argent qui ressemble à de l'argent. Le
  dégradé de reflet par défaut, lui, délave toutes les couleurs en pastel.
- **Avec en plus l'étalonnage du preset Catalogue** (saturation 75 %, teinte
  froide, appliqué ici après coup en Python), la Supra du jeu n'est plus loin de
  celle de Pit Box : même cadrage, même famille de couleur. Restent une teinte
  plus rouge et un flanc plus sombre.
- **Les lampes du gabarit, posées avec `ac.LightSource`, n'ont aucun effet
  visible** : images identiques à 0, 1 et 3 fois leur intensité. Ces lumières
  ne semblent pas atteindre une `GeometryShot`. C'est ce qui manque pour le
  flanc : il n'y a pas de lampe principale.
- **Le soleil de la piste** reste inutilisable : surexposé même à ×0,6.
- **Le skin** est imposé en réécrivant `SKIN=` dans `[CAR_0]` du `race.ini`
  juste après l'écriture de CM, comme le lancement de Pit Box. La Supra est
  bien sortie en `00_orange` (relu par `ac.getCarSkinID`), mais **c'est CM qui
  l'avait choisi** : le patch cherchait la voiture dans `[CAR_0]`, où CM écrit
  `MODEL=-`, et n'a donc jamais rien réécrit. Corrigé ensuite (la voiture se lit
  dans `[RACE]`, comme `raceini::player_car_model`), pas encore réessayé.

Un plantage d'AC au chargement pendant ce tour ne venait pas de l'app : Windows
avait épuisé sa mémoire réservable (127 Go engagés), dont 71 Go par un seul
processus Firefox. CSP le dit en clair dans `logs/log.txt` (« out of memory,
even though 4.2 GB of RAM is free »). À savoir pour le lot 2 : un échec de ce
genre doit se signaler comme un échec du jeu, pas de la voiture.

## Tours 5 et 6 : les lampes du gabarit, ajoutées par nous

Le jeu ne laisse poser aucune lampe dans une `GeometryShot` : `ac.LightSource`
reste sans effet sous l'éclairage neutre (tour 4) **comme sous l'éclairage
d'origine** (tour 5, images identiques avec et sans). Les lampes sont donc
ajoutées par notre shader, toujours dans le jeu :

- deux passes de plus, depuis la même caméra : la couleur de surface
  (`ShadersType.SampleColor`) et la normale en repère monde
  (`ShadersType.SampleNormal`, encodée `n·0,5+0,5` — mesuré : capot y 0,95,
  flanc x 0,9), sans les vitres pour ne pas masquer l'habitacle ;
- chaque lampe ajoute `couleur × max(0, n·L) × intensité` à l'image HDR,
  avant le tonemapping. Les directions sont celles de `placeLights` : principale
  à +45° de l'azimut caméra et 35° de haut, complément à −135° et 20° (25 %),
  contre-jour à 175° et 12° (60 %) ;
- puis l'étalonnage Catalogue (teinte froide, saturation 75 %), comme Pit Box,
  cette fois dans le shader du jeu.

**Résultat** : à un gain de ×2, la Supra du jeu est à quelques unités de la
vignette Pit Box en couleur moyenne de carrosserie — (124, 77, 44) contre
(116, 74, 40), sans les lampes (89, 44, 27). La S2000 protégée sort avec le même
traitement, sans réglage propre. La couleur de surface s'utilise telle quelle :
la linéariser (décodage sRGB) donne un orange trop rouge et trop sombre.

Ce qui reste visible : pas d'ombre de contact, des vitres un peu plus sombres,
et ce n'est vérifié que sur deux voitures, une orange et une argent.

## Tours 7 et 8 : d'autres peintures, et le phare sombre

**Cinq peintures de plus**, toutes non protégées : noir (`bmw_m3_e30`), blanc
(`c13_porsche_959_87`), rouge (`ferrari_458`), bleu (`ks_ford_mustang_2015`),
jaune (`ks_corvette_c7_stingray`). Avec le même réglage que la Supra (lampes à
×2), aucune ne sort fausse : le noir garde ses reflets, le blanc ne brûle pas,
le jaune reste jaune. La comparaison chiffrée n'est possible que pour la
Ferrari : carrosserie (98, 46, 48) dans le jeu avec spéculaire ×0,8, contre
(111, 39, 40) chez Pit Box. Pour les trois autres, la vignette Pit Box retrouvée
par la silhouette était celle d'une variante du même modèle (BMW de course,
Corvette de police, Mustang orange) : le rapprochement par la forme seule ne
distingue pas les variantes d'une même carrosserie.

**Le phare gauche de la Supra, sombre dans le jeu, gris chez Pit Box.**
- Ce n'est pas un manque de lampe : un reflet spéculaire des trois lampes
  (Blinn-Phong, une brillance pour toutes les surfaces) ne l'éclaircit presque
  pas.
- Ce n'est pas son verre : rendu sans aucun verre, il reste aussi sombre.
- C'est l'environnement de reflet. Le réflecteur chromé a une couleur de
  surface quasi noire et ne vit que de reflets ; tourné vers la caméra, il
  reflète ce qui est derrière elle, et dans le showroom du jeu (`'menu'`)
  c'est sombre. Le phare droit, tourné de côté, reflète une zone claire et
  sort presque blanc. Avec `'basic'`, le gauche s'éclaircit à peine. Pit Box
  donne à toute la voiture le même environnement gris uniforme, d'où deux
  phares pareils.

Le jeu ne propose que quatre environnements (`menu`, `basic`, `white`,
`black`) et aucun moyen d'en fournir un. Piste non essayée : rendre la même
vue sous `white` puis sous `black`. Leur différence, pixel par pixel, dit
combien chaque surface reflète, et le shader pourrait alors y poser
l'environnement de Pit Box lui-même, comme il pose déjà ses lampes.

## Tour 9 : l'environnement de reflet de Pit Box, posé par nous

Essayée, la piste ci-dessus marche techniquement : deux passes de plus sous les
environnements `white` et `black` du jeu, leur différence donne la
réflectivité de chaque point, et le shader y pose ce qu'il veut. Deux
environnements comparés : un gris uniforme (0,5), et le studio de Pit Box
(`showroomEnvironment.ts` : murs à 0,01, panneau de plafond à 6, panneaux
latéraux à 0,05, le tout à 0,55), lu dans la direction réfléchie, à trois
intensités.

| Carrosserie (moyenne) | Pit Box | showroom du jeu | gris 0,5 | studio ×0,5 | studio ×2 |
| --- | --- | --- | --- | --- | --- |
| Supra orange | 116, 74, 40 | 126, 80, 48 | 127, 81, 50 | **121, 74, 39** | 124, 78, 46 |
| Ferrari rouge | 111, 39, 40 | 94, 39, 41 | **96, 41, 43** | 85, 27, 28 | 91, 37, 38 |

- **La carrosserie de la Supra** n'a jamais été aussi proche qu'avec le studio ;
  **la Ferrari**, elle, s'en éloigne (plus sombre). Pas de gagnant net.
- **Les phares empirent avec le studio** : ceux de la Supra et de la Porsche
  deviennent noirs. Nos reflets sont des miroirs parfaits : un chrome tourné
  vers la caméra renvoie les murs presque noirs du studio. Chez Pit Box, la
  rugosité de chaque matériau floute ses reflets (PMREM) et mélange le panneau
  du plafond au reste, d'où un gris. Les passes du jeu ne portent pas cette
  rugosité, donc ce flou ne se reproduit pas point par point.
- Le gris uniforme donne à peu près l'image du showroom du jeu, phare gauche
  un peu moins sombre.

Le phare rejoint donc les deux limites connues des vignettes Pit Box elles-mêmes
(peintures sombres, reflets des vitres, `CHANTIERS.md`) : c'est un reflet, et
les reflets sont ce que deux moteurs différents reproduisent le moins bien.
Réglage retenu pour la suite, faute de mieux : showroom du jeu ou gris
uniforme, lampes à ×2, spéculaire ×0,3.

Le premier essai de ce tour a planté au chargement d'AC, avec Firefox à 76 Go
de mémoire engagée et Bash incapable de lancer un processus. Relancé après la
fermeture de Firefox, il est passé du premier coup.

## Tour 10 : le sol de Vitrine

Le sol se pose dans le shader de l'app, comme le rig de Pit Box le pose
(`floor.ts`, `studioFloor.ts`, `floorMirror.ts`) : un carré de six rayons de
voiture sous la boîte, la flaque en dégradé, l'assombrissement sous la caisse,
et le reflet. Deux techniques :

- **Où un pixel touche le sol** : les quatre rayons des coins de l'image, tirés
  des matrices de la caméra du jeu (décalage d'objectif compris), interpolés
  par le shader puis coupés par le plan du sol.
- **Le reflet** : une passe de plus, vue par la caméra symétrique sous le sol,
  avec le décalage d'objectif inversé ; son image est le reflet retourné de
  gauche à droite. Flouté (25 prises) et éteint en s'éloignant de la voiture,
  teinté de 0,212 comme le `Reflector` de three.js.

Vu sur la Ferrari 458 : le reflet tombe sous la voiture, chaque roue sous sa
roue. Aucune vignette Vitrine de Pit Box n'existait sur la machine pour
comparer : le preset confortable de l'utilisateur est Officiel.

## Tour 11 : Officiel

Officiel ajoute trois choses au sol, toutes posées par le shader de l'app comme
le rig de Pit Box les pose :

- **le fond cuit** : le dégradé elliptique du mat (`backdropTexture`), ni
  exposé ni tone mappé ;
- **le disque de lumière** (`poolSize`) centré sur l'origine de la voiture sous
  la caméra du jeu, et le **sol sombre** du studio autour (`GROUND_SHADE`) ;
- **l'exposition automatique** : l'image entière rendue en 64×36 à
  l'exposition de base, relue côté CPU (`ExtraCanvas:accessData`, asynchrone),
  sa luminosité moyenne en lumière linéaire donnant l'exposition de l'image
  finale, avec la cible et les bornes de `look.ts`.

L'étalonnage passe en dernier, sur toute l'image, fond compris, comme Pit Box
étalonne son canevas fini.

Comparé sur la Ferrari 599 GTO à sa vignette Officiel Pit Box : cadrage
identique (la silhouette se superpose exactement), fond (3, 4, 6) des deux
côtés, sol (13, 14, 15) contre (14, 14, 15), image entière (25, 16, 16) contre
(28, 14, 15), exposition mesurée 0,74. La peinture sort un peu plus sombre et
moins brillante sur le capot : la limite des reflets, déjà vue.

Le journal d'erreurs (`error.txt`, et `lastError` dans `done.json`) a trouvé du
premier coup une erreur de compilation du shader que rien d'autre n'aurait
montrée.

## Tour 12 : plusieurs voitures dans une même session (branche d'essai)

Une séance d'essais ne porte qu'une voiture. Une **course** ou une **journée
piste** en porte plusieurs : la première au joueur, les autres en adversaires,
chacune dans son skin (`launch::studio_grid_setup`, preset produit par le code
de Pit Box). Dans l'app de prise de vue :

- **le jeu est mis en pause** dès qu'il l'accepte (`ac.tryToPause`), avant
  qu'une voiture ne bouge ;
- **aucun modèle simplifié** (`ac.overrideLODMultiplier(nil, math.huge)`), et
  `ac.forceDetailedCarLOD` sur chaque voiture pendant 30 images avant sa photo ;
- **pas de pilote** (`ac.setDriverVisible(i, false)`) : les adversaires en ont
  un assis au volant, visible par la vitre.

Mesuré : **4 voitures protégées, 12 images (Catalogue, Vitrine, Officiel), en
36 s** de bout en bout, lancement compris, contre environ 25 s par voiture une
par une. Toutes en LOD 0, dans leur skin, jeu bien en pause.

**Le blocage** : après une course ou une journée piste, Content Manager ouvre
sa fenêtre de résultats (il ne la saute qu'après une séance d'essais :
`SkipResults: Type=Practice`) et **ignore toute demande tant qu'elle est
ouverte** (« Waiting for extra windows to close »). Une session à plusieurs
voitures se termine donc sur cette fenêtre, et la suivante ne part pas. Magione
a 18 stands : jusqu'à 18 voitures par session.

**Suite (2026-11-11)** : même avec « Skip results » coché pour tous les types
de session, CM garde après une journée piste sa boîte d'attente (« Waiting for
extra windows to close ») ; son code (`GameDialog.OnResult`) est pourtant censé
la fermer. Contourné côté Pit Box (SESSION§2.7) : le CM que Pit Box a démarré
est prié de se fermer douze secondes après le jeu. La génération en masse
envoie les voitures par lots de douze, une journée piste par lot.

## Pistes pour l'éclairage, de la moins à la plus coûteuse

*Écrites avant les tours 4 à 6 ; la deuxième est celle qui a abouti, mais dans
le jeu et non dans Pit Box.*

1. **Piste studio** (prévue par le document de départ) : soleil orienté selon le
   gabarit, sans ciel ni décor, et `setBestSceneShotQuality()`. C'est le seul
   moyen d'agir sur l'éclairage d'origine, puisque les réglages de la
   `GeometryShot` sont ignorés dans ce mode. Demande de livrer une piste.
2. **Rendre les tampons, éclairer dans Pit Box.** `render.ShadersType.SampleColor`
   (couleur de surface) et `SampleNormal` (normale en repère monde) sont
   disponibles : le jeu ne ferait que décoder la voiture, et l'éclairage serait
   calculé par Pit Box avec les lumières du gabarit — même éclairage que les
   autres vignettes, par construction. Reste à voir ce qu'on perd (reflets,
   vitres, matériaux CSP).
3. Régler à la main un jeu de valeurs pour l'éclairage neutre : écarté, ses
   réglages n'ont presque pas d'effet visible.

## Autres choses apprises pour le lot 2

- **Le skin, c'est CM qui le choisit** quand le preset Quick Drive n'en dit
  rien (la Supra est sortie orange puis rouge). À imposer : le patch du
  `race.ini` existant (`raceini::spawn_player_skin_patcher`) ou
  `SceneReference:applySkin` côté Lua.
- **Une voiture par session**, en séance d'essais. Un lot de plusieurs voitures
  demande un autre mode (course, grille) ; non essayé.
