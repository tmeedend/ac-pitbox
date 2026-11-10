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
