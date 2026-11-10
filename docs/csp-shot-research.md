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

## Pistes pour l'éclairage, de la moins à la plus coûteuse

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
