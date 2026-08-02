# Aperçu 3D voiture (fiche détail) — recherche de faisabilité

> Branche `feature/showroom-3d-preview`. Sujet : remplacer la preview image
> statique d'une voiture par un rendu 3D. Deux pistes explorées et évaluées
> avant tout code — voir verdict en fin de document.

## Demande initiale

Dans Réglages, une option remplacerait la preview image de la fiche détail
voiture par le **showroom** (celui de Content Manager, preset "fast" si un
tel preset existe par défaut), avec l'intuition qu'il faudrait "embed le
process dans la page".

## Piste 1 — Embarquer la fenêtre du Custom Showroom de Content Manager

### Ce qui est confirmé

**Le showroom tourne dans le process de CM, pas dans un exécutable séparé.**
Contrairement à ce que suggère le dépôt GitHub `gro-ove/actools` (qui contient
un outil `CustomShowroom` buildable en `.exe` autonome), la version installée
de Content Manager charge ce code **en interne** (`CustomShowroomWrapper.
StartAsyncInner()`, log confirmé : `[CustomShowroomWrapper:84] StartAsyncInner():
Custom Showroom: Magick.NET IsSupported=True`). Aucun process séparé
n'apparaît dans `Get-CimInstance Win32_Process` pendant que le showroom est
ouvert — seul `Content Manager.exe` tourne.

**Mais c'est bien une fenêtre top-level séparée, pas une sous-fenêtre WPF.**
Énumération Win32 (`EnumWindows` + filtre par PID du process CM) :

```
hwnd=0x4E610B4 class='WindowsForms10.Window.8.app.0.ae0365_r7_ad1'
  title='Toyota Celica ST185 4WD Turbo (FPS: 81)' visible=True
```

- Classe **WinForms** (`FormWrapperBase`, confirmé dans les logs : `[FormWrapperBase:86]
  UpdateSize(): 1600×900 (AcTools)`), distincte des fenêtres WPF du reste de CM
  (classe `HwndWrapper[Content Manager.exe;;<guid>]`).
- **Titre prévisible** : `"{nom affiché de la voiture} (FPS: {n})"` — exploitable
  pour la retrouver par pattern matching une fois ouverte.
- Étant une vraie fenêtre top-level (pas un enfant de la fenêtre principale CM),
  un `SetParent()` Win32 classique pour l'embarquer dans une autre appli
  (donc dans Pit Box) est **en théorie jouable** sans avoir à embarquer toute
  l'UI de CM.

### Ce qui bloque : aucun déclenchement programmatique trouvé

Objectif : ouvrir le showroom **pour une voiture donnée, depuis Pit Box**,
sans repasser par les clics manuels dans CM. Deux mécanismes testés/vérifiés,
tous deux des impasses **confirmées par le code source**, pas de simples
absences de documentation :

1. **Protocole `acmanager://`** (celui déjà utilisé par Pit Box pour lancer
   les sessions, `acmanager://race/quick?presetFile=...`, voir
   `L4-cm-launch-research.md`). Liste complète des commandes supportées,
   lue directement dans `AcManager/Tools/ArgumentsHandler.Commands.cs`
   (dépôt `gro-ove/actools`, source réelle de CM) :
   `batch`, `launch`, `race/quick`, `race/config`, `race/online`,
   `race/online/join`, `race/csp`, `race/raceu`, `race/worldsimseries`,
   `race/worldsimseries/login`, `setsteamid`, `loadgooglespreadsheetslocale`,
   `install`, `importwebsite`, `cup/registry`, `live`, `lobby`, `csp/install`,
   `csp/preview`, `replay`, `rsr`, `rsr/setup`, `tool/update-car-preview`,
   `tool/script`, `shared`.
   **Aucune commande showroom.** Testé en direct sur la machine (CM ouvert,
   voiture Celica déjà en showroom) :
   ```
   Start-Process "acmanager://showroom/open?car=ks_toyota_celica_st185"
   ```
   Log CM : `[ArgumentsHandler.Commands:290] ProcessUriRequest(): Not supported
   request: "showroom/open"`. CM reçoit et rejette explicitement la requête.

2. **`tool/script`** — semblait prometteur (exécution de script piloté par
   URI) mais **inadapté à l'usage** : lit le code réel du handler
   (`ArgumentsHandler.Commands.cs`) — il ne fait qu'exécuter un fichier
   `.bat` externe présent dans un dossier `Scripts` fixe, et seulement si
   CM a été démarré avec le flag `--allow-data-scripts`. Un `.bat` externe
   n'a aucun moyen d'appeler une méthode C# privée à l'intérieur du process
   CM déjà lancé (`CustomShowroomWrapper` n'est pas exposé). Impasse
   confirmée par la lecture du code, pas juste par absence de doc.

**Il existe une commande statique proche** : `tool/update-car-preview`
(paramètres `car`, `skin`, `preset`) — mais elle régénère une **capture
d'écran fixe** (les presets `.cmpreset` de "Custom Previews"), pas un rendu
interactif live. Ce n'est pas la même fonctionnalité malgré le nom proche.
Sur le "preset fast" mentionné dans la demande initiale : aucune preuve
qu'il s'agisse d'un preset nommé du Custom Showroom — plus probablement un
niveau de qualité/anti-aliasing (le renderer standalone de `actools` a des
options MSAA/FXAA/SSAA), à vérifier au cas par cas sur l'installation CM
de l'utilisateur.

### Verdict piste 1

Techniquement embarquable (fenêtre séparée, `SetParent` jouable), mais
**pas déclenchable automatiquement pour une voiture donnée** sans un des
deux choix suivants, aucun des deux retenu pour l'instant :
- Automatisation d'UI (simuler des clics dans la fenêtre CM) — fragile,
  dépendant de la version/thème de CM, non exploré en détail.
- v1 pragmatique : l'utilisateur ouvre le showroom **manuellement** dans CM,
  Pit Box détecte la fenêtre par pattern de titre et propose un bouton
  "Attacher ici" (`SetParent` + synchro taille/position) dans la fiche
  détail, avec un bouton "Détacher" pour la rendre flottante. Reste risqué
  (plein écran exclusif à proscrire, focus clavier/souris à gérer, fermeture
  propre du process enfant à la charge de Pit Box, décalages DPI possibles)
  mais réaliste — **non implémenté**, en attente de décision.

## Piste 2 — Rendu 3D maison (parser KN5 + WebGL/Three.js), sans dépendre de CM

Idée alternative : lire directement le modèle 3D natif du mod (`.kn5`) et le
rendre nous-mêmes dans la webview (Three.js), sans process externe ni
Content Manager du tout.

### Le format KN5

Non documenté officiellement par Kunos, mais rétro-ingénierié par la
communauté — **3 implémentations indépendantes s'accordent sur la
structure**, ce qui la rend fiable malgré l'absence de spec officielle :

- **RaduMC/kn5-converter** (C#, référence la plus utile, code lu directement) :
  magique `"sc6969"` + version (int), puis sections **dans l'ordre** :
  1. **Textures** — nombre, puis par texture : type, nom de fichier, blob
     binaire brut **embarqué** (pas de fichiers externes à gérer).
  2. **Matériaux** — nom, nom du shader, propriétés flottantes (`ksAmbient`,
     `ksDiffuse`, `ksSpecular`, `diffuseMult`, `normalMult`…), références aux
     textures (diffuse/normal/detail) avec offset/scale UV.
  3. **Arbre de nœuds** — hiérarchie récursive (nœuds vides / meshes / meshes
     animés), chacun avec sa matrice de transformation, ses enfants, et pour
     les meshes : sommets, normales, UV, buffer d'indices, référence au
     matériau.
- **MarvinSt/kn5-obj-converter** (Python, port de la même logique, licence
  permissive type MIT) — sert de recoupement indépendant.
- **SeizureSaladd/Kn5Decrypt** (C#) — gère la variante "chiffrée" (obfuscation
  légère type XOR) que certaines voitures **officielles Kunos** utilisent ;
  pertinent seulement si on veut lire le contenu de base, pas les mods tiers.
- **gro-ove/actools** — namespace `AcTools.Kn5File` confirmé
  (`Kn5.cs`, `Kn5Header.cs`, `Kn5Node.cs`, `Kn5Material.cs`, `Kn5Texture.cs`),
  c'est le vrai lecteur de production de CM. Corps de l'implémentation non
  récupéré (façade publique seulement côté GitHub raw), mais la forme des
  fichiers confirme la même structure que RaduMC.

**Aucun parser Rust ni JS/TS/Three.js existant** (recherché sur crates.io,
npm, GitHub — zéro résultat pertinent). Ce serait entièrement à écrire côté
Pit Box, mais sans zone d'ombre majeure sur le format lui-même.

### Textures DDS dans Three.js

- `DDSLoader` de Three.js (`three/examples/jsm/loaders/DDSLoader.js`, code
  lu directement) gère **DXT1/DXT3/DXT5** nativement via l'extension WebGL
  `WEBGL_compressed_texture_s3tc` — **aucun décodage CPU nécessaire**. Ce
  sont les formats DDS les plus courants sur les skins AC.
- **BC7 non géré** par ce loader (branche DX10 lue seulement pour BC6H).
  Certains mods récents haute qualité utilisent BC7 — ces textures
  échoueraient au chargement sans travail supplémentaire (décodage BC7→RGBA
  côté Rust avant transmission au front, ou extension du loader pour
  `EXT_texture_compression_bptc`).

### Fidélité de rendu

AC utilise des shaders propriétaires Kunos (`ksPerPixel`,
`ksPerPixelMultiMap`, `ksPerPixelMultiMap_NMDetail`,
`ksPerPixelMultiMap_damage`…), pas du PBR standard. Un matériau Three.js
standard (`MeshStandardMaterial`/`MeshPhysicalMaterial`) approximé à partir
de `ksDiffuse`/`ksSpecular` donnerait une voiture **reconnaissable et
plausible**, mais pas identique au rendu showroom de CM (pas de vernis
multi-couches, flocons de peinture, detail maps fidèles). C'est un vrai
"aperçu 3D", pas un clone du showroom CM.

### Prior art

Aucun viewer web AC public trouvé (recherché : "kn5 three.js", "ac liveries
viewer web", "kn5 to gltf"). Les outils existants sont tous des
**convertisseurs desktop hors-ligne** : `RaduMC/kn5-converter` (KN5→OBJ/FBX),
`MarvinSt/kn5-obj-converter` (idem, Python), `moppius/blender-assetto-corsa-tools`
(addon Blender), `IOBYTE/kn5toac` (sens inverse, C++). Aucun ne vise le web/
WebGL directement — terrain neuf pour cette partie-là, mais le format
lui-même n'est pas un territoire inconnu.

### Verdict piste 2

**Effort réaliste : plusieurs semaines en solo, pas plusieurs mois**, pour un
résultat "correct" (pas fidélité showroom). Le parsing binaire n'est pas le
risque principal (3 implémentations de référence à porter/recouper). Les
vrais risques :
1. Fidélité shader/matériau — problème ouvert, rendements décroissants au-delà
   d'un rendu "plausible".
2. Textures BC7 — gap concret à combler pour certains mods.

**Gros avantage architectural vs la piste 1** : aucune dépendance à Content
Manager (fonctionne même sans CM installé), pas de hack de fenêtre Windows
fragile, pas d'étape manuelle utilisateur avant que l'aperçu s'affiche.

## Décision (2026-07-04)

Recherche mise en pause pour être consignée (ce document) avant de trancher
entre les deux pistes. Proposition retenue en discussion : commencer par une
**Phase A d'un jour ou deux** — parser KN5 en Rust + commande de diagnostic
qui lit un vrai `.kn5` de la bibliothèque et affiche le nombre de
meshes/matériaux/textures trouvés, sans toucher au rendu. Valide les
fondations avant d'investir dans l'intégration Three.js. **Non commencé** —
à reprendre plus tard.

## Piste 3 — `acShowroom.exe` natif (racine de l'install AC), pas Content Manager

Piste proposée par l'utilisateur, **beaucoup plus prometteuse que la piste 1**.
Vérifié directement sur la machine (install réelle :
`D:\SteamLibrary\steamapps\common\assettocorsa`).

### Ce qui est confirmé

- **`acShowroom.exe` existe** à la racine de l'install AC, à côté de `acs.exe`
  et `AssettoCorsa.exe` — un vrai exécutable natif Kunos séparé (17 Mo,
  DirectX11, moteur de rendu propre avec ses propres classes `ShowroomGUI`,
  `ShowRoomCameraManager`, `ShowroomSkinManager`, `ShowroomMirrorCamera`…,
  identifiées via les symboles embarqués dans le binaire).
- **Configuration par fichier INI, pas par CLI** — même principe que
  l'ancien `race.ini` (abandonné depuis pour le lancement de session, voir
  `L4-cm-launch-research.md`, mais le pattern « écrire l'INI, lancer, l'appli
  le lit » reste valable ici). Le fichier
  `cfg/showroom_start.ini` existe dans `Documents/Assetto Corsa/cfg/` **et**
  dans le dossier d'install (template par défaut). Contenu réel capturé
  pendant que le showroom tournait sur la Celica :
  ```ini
  [SHOWROOM]
  CAR=ks_toyota_celica_st185
  SKIN=00_racing_3
  ALLOW_SELECT_SKIN=1
  TRACK=showroom
  SELECTED_SKIN=1
  CAR_ID=0

  [PREVIEW_MODE]
  LOOK_AT=0,0.6,0
  CUSTOM_CAMERA_POSITION=-0.366574,0.775145,-6.12493
  USE_CUSTOM_CAMERA=1
  CUSTOM_CAMERA_ROLL=0
  CUSTOM_CAMERA_EXPOSURE=94.5

  [SETTINGS]
  ROTATION_SPEED=1.0
  CAMERA_DISTANCE=6
  CAMERA_HEIGHT=1.5
  CAMERA_FOV=30
  CAMERA_EXPOSURE=30
  SUN_ANGLE=-50
  ...
  ```
  `TRACK=showroom` référence une des scènes disponibles dans
  `content/showroom/` (`showroom`, `beach`, `Hangar`, `industrial`,
  `studio_white`, chacune avec son `ui_showroom.json`). Écrire ce fichier
  avant de lancer `acShowroom.exe` (aucun argument requis) devrait suffire à
  cibler une voiture + skin précis — **le même pattern « écrire l'INI puis
  lancer »**, sans dépendre de Content Manager.
- Le binaire utilise `CommandLineToArgvW` (accepte des arguments CLI) et logue
  `checkShowroomINI` — cohérent avec un chargement du fichier INI ci-dessus
  au démarrage.
- **Redimensionnement dynamique supporté** : symboles `OnWindowResize` /
  `OnWindowResizeEvent` présents dans le binaire — suggère que la fenêtre
  peut être redimensionnée à chaud, utile pour synchroniser la taille avec
  la zone preview de Pit Box une fois embarquée.

### Le point bloquant : mode fenêtré partagé avec le vrai jeu

Pas de flag CLI dédié trouvé pour forcer le mode fenêtré (`-window`,
`-windowed`… absents des chaînes du binaire). Les logs internes
(`"WARNING: Suitable video mode not found, but windowed mode requested..
continuing"`, `IsFullscreen: %d`, `Windowed: %d`) suggèrent que
`acShowroom.exe` lit la **même config vidéo que le jeu réel** :
`Documents/Assetto Corsa/cfg/video.ini`, section `[VIDEO]` :
```ini
[VIDEO]
FULLSCREEN=1
WIDTH=3840
HEIGHT=2160
REFRESH=144
...
```
C'est le fichier qui pilote aussi les vraies séances de conduite (résolution,
taux de rafraîchissement, anti-aliasing…) — **on ne peut pas se contenter de
le modifier en dur** sans risquer de casser l'expérience de jeu réelle si
Pit Box plante avant de restaurer la valeur d'origine.

**Approche à valider avant d'implémenter** : sauvegarder `video.ini`,
basculer temporairement `FULLSCREEN=0` (+ une résolution raisonnable pour
l'aperçu) juste avant de lancer `acShowroom.exe`, puis restaurer le fichier
d'origine dès la fermeture du process (et prévoir une restauration défensive
au démarrage de Pit Box si une sauvegarde orpheline traîne suite à un crash).
Risque non nul sur un fichier qui ne concerne pas que la fonctionnalité
d'aperçu — nécessite un accord explicite avant de coder, contrairement aux
pistes 1/2 qui ne touchaient à aucun fichier appartenant au jeu.

### Verdict piste 3

**La plus prometteuse des trois.** Contourne complètement les impasses de la
piste 1 (Content Manager) : process natif séparé, configuration par simple
fichier INI (pattern déjà maîtrisé côté Pit Box), pas de dépendance à CM.

### Phase A implémentée (2026-07-04)

Décision utilisateur : sauvegarder/restaurer `video.ini` automatiquement.
Implémenté dans `src-tauri/src/showroom.rs` :
- `write_showroom_ini_at` écrit `showroom_start.ini` (voiture + skin).
- `backup_and_force_windowed_at` sauvegarde `video.ini` puis force
  `FULLSCREEN=0` (seule cette clé est touchée, tout le reste — résolution,
  refresh, anti-aliasing — reste intact).
- `restore_video_ini_at` restaure depuis la sauvegarde ; no-op si absente.
- `open_native_showroom` écrit l'INI, sauvegarde/bascule `video.ini`, lance
  `acShowroom.exe`, puis restaure `video.ini` dès la fermeture du process
  (thread qui attend `child.wait()`) — que la fenêtre soit fermée par
  l'utilisateur ou le process tué.
- `restore_orphaned_video_ini` est appelé une fois au démarrage de l'app
  (filet de sécurité si une sauvegarde traîne suite à un crash).
- Bouton « Aperçu 3D » sur la fiche voiture (`DetailPage.svelte`), ouvre le
  showroom sur le skin actuellement sélectionné.
- 3 tests unitaires (écriture INI, transformation `FULLSCREEN`,
  sauvegarde/restauration en aller-retour).

**Non testé en conditions réelles au moment du commit** — le clic sur le
bouton n'a délibérément pas été automatisé (lance un vrai process externe et
modifie un vrai fichier de config du jeu). À valider manuellement.

### Test réel (2026-07-04) : deux bugs trouvés et corrigés

Premier essai utilisateur : le showroom démarre mais reste plein écran (à
l'œil) et l'écran est noir (voiture invisible). Diagnostic par comparaison
avec le fichier réel capturé plus tôt (piste 3, section "confirmé") :

1. **Écran noir** : `showroom_start.ini` écrit par Pit Box était tronqué —
   sections `[FADES]`/`[ANIMATION]` absentes, et surtout `NEAR_PLANE`/
   `FAR_PLANE`/les 3 `SHADOW_SPLIT*` manquants dans `[SETTINGS]`. Un plan de
   clipping caméra absent/à 0 produit une matrice de projection dégénérée →
   rien n'est rendu. Corrigé en reproduisant l'intégralité du fichier de
   référence (seuls `CAR`/`SKIN` changent).
2. **"Toujours plein écran"** : `FULLSCREEN` passait bien à `0`, mais
   `WIDTH`/`HEIGHT` restaient à la résolution du bureau (3840×2160) — une
   fenêtre sans bordure à cette taille est visuellement indiscernable du
   plein écran. `force_windowed()` réduit maintenant aussi `WIDTH`/`HEIGHT`
   à `1280×720` pendant la session (restaurés comme le reste à la fermeture).

Corrigé dans `src-tauri/src/showroom.rs`, tests mis à jour en conséquence.
**Toujours en attente d'un nouveau test réel** pour confirmer que la voiture
s'affiche correctement.

### Phase B implémentée (2026-07-04, expérimental)

Après confirmation que la Phase A fonctionne (voiture visible en fenêtré),
implémentation de l'intégration dans la page. Fenêtre identifiée en lançant
`acShowroom.exe` manuellement et en inspectant ses fenêtres via
`EnumWindows`/`GetClassName` (PowerShell + P/Invoke) : classe native
**`acShowroomW`**, titre "Assetto Corsa" (non spécifique à la voiture — la
recherche se fait par PID + classe, pas par titre), style initial
`WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS`.

Ajout du crate `windows` (0.61, déjà résolu transitivement par Tauri — pas de
conflit de version) pour `EnumWindows`/`SetParent`/`SetWindowLongPtrW`/
`SetWindowPos`/`PostMessageW`. `tauri::WebviewWindow::hwnd()` donne
directement notre propre HWND dans le même type `windows::Win32::Foundation::HWND`
que Tauri utilise en interne — pas besoin de `raw-window-handle`.

Flux : `open_native_showroom` renvoie le PID du process lancé (mémorisé dans
un état d'app `ShowroomState`) → `attach_native_showroom(x,y,w,h)` attend
l'apparition de la fenêtre (jusqu'à 10s), retire `WS_POPUP`, ajoute
`WS_CHILD`, `SetParent` dans la fenêtre principale, positionne. Le bouton
"Fermer l'aperçu" envoie `WM_CLOSE` à la fenêtre native — le thread de
Phase A qui attend déjà la fin du process s'occupe de restaurer `video.ini`.

Suivi de position côté front (`DetailPage.svelte`) : la fenêtre intégrée est
une vraie fenêtre OS, elle ne fait pas partie du rendu de la page — un
`ResizeObserver` sur la zone héros + des listeners `resize`/`scroll` (phase
de capture, pour attraper le scroll de n'importe quel ancêtre) recalculent
sa position (`getBoundingClientRect()` × `devicePixelRatio`) à chaque
changement de mise en page.

**Entièrement expérimental, non testé en conditions réelles.** Les points
les plus susceptibles de nécessiter un ajustement après un premier essai :
- Le calcul `devicePixelRatio` peut être décalé par le zoom d'interface de
  Pit Box (§ réglage ajouté précédemment) ou par un DPI Windows atypique.
- Le comportement de `acShowroomW` une fois transformé en `WS_CHILD` n'a
  jamais été observé (le rendu DirectX pourrait mal réagir à la perte du
  statut top-level, ou le contenu pourrait apparaître décalé/mal découpé).
- Fermeture/réattachement en changeant de voiture pendant qu'un aperçu est
  attaché : géré via un effet Svelte sur `id`, non testé.

### Test réel n°2 (2026-07-04) : fenêtre invisible — "problème d'espace aérien"

Premier essai réel de la Phase B : le process démarre (son entendu, visible
dans le gestionnaire des tâches), mais **rien ne s'affiche** dans la page.
Effet de bord noté par l'utilisateur : le clavier atteint quand même le jeu
en arrière-plan (bruits de portières en tapant) — la fenêtre existe et reçoit
des événements, elle n'est juste pas peinte à l'écran.

**Diagnostic confirmé en direct** (pas une supposition) : `EnumChildWindows`
sur la vraie fenêtre Pit Box montre `acShowroomW` bien reparenté (`WS_CHILD`,
`visible=True`, rect correcte), mais **superposé exactement** par la
hiérarchie de fenêtres de WebView2 (`WRY_WEBVIEW` → `Chrome_WidgetWin_0/1`
→ `Chrome_RenderWidgetHostHWND`, sur 2 process différents) qui couvre toute
la zone cliente. C'est le "problème d'espace aérien" (airspace) : la surface
composée par accélération GPU de WebView2/Chromium passe systématiquement
au-dessus de toute fenêtre native sœur, indépendamment de l'ordre Z Win32
classique. Limitation documentée de l'écosystème CEF/WebView2/Electron, pas
un bug de positionnement.

**Contournement validé en direct avant d'écrire le code Rust** : prototype
PowerShell + P/Invoke sur le process resté ouvert — création d'une fenêtre
"overlay" séparée (classe custom enregistrée via `RegisterClassW`, `WS_POPUP`
possédée par la fenêtre Pit Box via `CreateWindowExW(..., hWndParent=pitbox)`),
`acShowroomW` reparenté **dans cet overlay** plutôt que directement dans la
fenêtre principale. Confirmé fonctionnel côté Win32 (à confirmer visuellement
par l'utilisateur avec le vrai code).

Implémenté dans `src-tauri/src/showroom.rs` : `ShowroomState` porte
maintenant `{ pid, overlay: Option<isize> }` ; `attach()` crée l'overlay et y
reparente la cible ; `reposition()` déplace l'overlay en coordonnées écran
absolues (translation via `ClientToScreen` sur la fenêtre principale, le
front continue d'envoyer des coordonnées relatives à la zone cliente) et
resynchronise la taille de l'enfant ; `close()` détruit l'overlay après avoir
demandé la fermeture propre du showroom.

**Incident collatéral** : le crash du prototype PowerShell (tué avec
`Stop-Process -Force`) a laissé `video.ini` bloqué en mode fenêtré (1280×720,
`FULLSCREEN=0`) — le filet de sécurité au démarrage de l'app n'avait pas
encore eu l'occasion de tourner (l'app Pit Box n'avait pas redémarré depuis).
Restauré manuellement. Confirme que le filet de sécurité est utile mais ne
couvre que le redémarrage de **Pit Box**, pas un test ad hoc externe au
process — à garder en tête pour la suite des essais.

**Toujours en attente d'un test réel de cette version** (build + clic sur
Aperçu 3D dans l'app).

### Test réel n°3 (2026-07-04) : crash complet (pas juste invisible)

Test de la version overlay : fenêtre noire brève puis **disparition totale
du process** (plus dans le gestionnaire des tâches) — régression par rapport
au test n°2 où le process restait vivant (juste invisible).

**Diagnostic** : l'overlay était créé (`CreateWindowExW`) sur le thread qui
exécute la commande Tauri `attach_native_showroom` — un thread de pool,
recyclable/jetable, pas un thread persistant. Windows détruit automatiquement
les fenêtres dont le thread propriétaire se termine. Une fois l'overlay
détruit, son enfant (le showroom reparenté dedans) l'est aussi en cascade —
et contrairement à un `WM_CLOSE` propre, ce genre de destruction forcée fait
visiblement planter `acShowroom.exe` plutôt que de le laisser sortir
proprement (cohérent avec "fenêtre noire puis disparition", pas juste
"fenêtre fermée").

**Corrigé** : `attach()` bascule la création de l'overlay + le reparentage
sur le **thread principal** de Tauri via `AppHandle::run_on_main_thread` —
ce thread pompe les messages en continu pour toute la durée de vie de l'app,
donc les fenêtres qu'il crée ne sont jamais orphelines. Le résultat remonte
au thread appelant via un canal `std::sync::mpsc` (le poll d'attente de la
fenêtre du showroom, potentiellement long, reste hors du thread principal
pour ne pas geler l'UI).

**Toujours en attente d'un test réel de cette version.**

### Test réel n°4 (2026-07-04) : aucune différence perçue → test hors-app

Le fix "thread principal" n'a rien changé de visible. Plutôt que de
continuer à itérer via rebuild complet + clic manuel (boucle lente), création
d'un outil de test **autonome, hors Tauri/WebView2**
(`tools/showroom-embed-test/`, jamais committé — config machine en dur,
jetable) reproduisant juste la mécanique Win32 : fenêtre hôte native simple
+ message loop, spawn `acShowroom.exe`, reparenting, dans le même process/
thread donc pas de souci d'orphelinage.

**Résultat décisif** : la voiture s'affiche correctement, dans les deux
modes (reparent direct dans l'hôte, et overlay séparé comme dans l'app) —
confirme que la mécanique de fond (reparenting Win32 + survie du rendu
DirectX en tant qu'enfant) fonctionne bien. Un bug restait visible : la
fenêtre gardait sa **propre barre de titre Windows** (bordure, boutons
minimiser/maximiser/fermer, menu système) une fois transformée en enfant —
`WS_POPUP` seul ne suffit pas à la retirer.

**Corrigé** : il faut aussi retirer `WS_CAPTION`, `WS_THICKFRAME`,
`WS_SYSMENU`, `WS_MINIMIZEBOX`, `WS_MAXIMIZEBOX` du style avant le
`SetParent`. Validé visuellement sur l'outil de test (mode direct et mode
overlay), puis porté dans le vrai code de l'app
(`src-tauri/src/showroom.rs::attach()`).

**Utilité confirmée du test hors-app** : a permis d'isoler en quelques
minutes ce qui aurait pris plusieurs allers-retours de rebuild complet de
l'app + test manuel. À garder comme outil de diagnostic pour la suite
(ex. si le problème d'espace aérien WebView2 revient sous une autre forme,
ou pour affiner la position/taille avant de toucher au vrai code).

**Toujours en attente d'un test réel de cette version dans l'app.**

### Test réel n°5 (2026-07-04) : le vrai bug, enfin trouvé

Le correctif WS_CAPTION n'a rien changé côté utilisateur — retour au
symptôme "fenêtre noire puis disparition". Ajout de traces (`eprintln!`) à
chaque étape d'`attach()`/`reposition()` et sur la sortie du process, testé
en mode dev (`npm run tauri dev`, seul moyen de voir la console) :

```
[showroom] attach: ... (toutes les étapes, sans erreur)
[showroom] attach: terminé avec succès
[showroom] process 88892 terminé : Ok(ExitStatus(ExitStatus(0)))
```

**`attach()` réussit intégralement.** Le process sort ensuite avec le
**code 0** — une fermeture propre, pas un crash. Aucune entrée dans
l'observateur d'événements Windows (pas d'Application Error/WER), cohérent.

Hypothèse testée en premier (perte de focus Windows au profit de WebView2)
via l'outil hors-app : ajout d'un `--steal-focus` qui vole le focus vers
l'hôte juste après le reparenting. **Infirmée** : le process survit très
bien au vol de focus dans le test isolé. Fausse piste écartée en quelques
secondes plutôt qu'un aller-retour de rebuild complet — l'outil a de nouveau
justifié sa création.

**Le vrai bug était côté Svelte, dans `DetailPage.svelte`**, sans aucun
rapport avec Windows/WebView2 :
```js
$effect(() => {
  id;
  if (showroomAttached) closeShowroom();   // BUG : lit showroomAttached
});                                          // sans untrack()
```
Svelte 5 traque **toute** valeur réactive lue pendant l'exécution d'un
effet — pas seulement celles isolées en tête de fonction. Lire
`showroomAttached` dans le `if` en fait donc une dépendance de l'effet au
même titre que `id`. Résultat : dès que `showroomAttached` passe à `true`
(juste après un `attach()` réussi), l'effet se redéclenche et appelle
aussitôt `closeShowroom()` — l'app se refermait elle-même la fenêtre qu'elle
venait d'intégrer avec succès, dans le même tick.

**Corrigé** avec `untrack()` (import `svelte`) autour de la lecture de
`showroomAttached`, pour que seul `id` reste une dépendance suivie de cet
effet.

**Leçon** : deux pistes (thread principal, barre de titre) étaient de vrais
bugs Win32 réels et utiles à corriger, mais aucune n'expliquait le
symptôme observé — le bug était ailleurs depuis le début. Ne pas supposer
que la couche la plus "exotique" (Win32/WebView2) est forcément en cause
quand un effet Svelte ordinaire peut suffire à tout expliquer.

## Statut (2026-07-04) : fonctionnel, confirmé par l'utilisateur

L'aperçu 3D natif marche de bout en bout : bouton "Aperçu 3D" sur la fiche
voiture → `acShowroom.exe` lancé, ciblé sur la bonne voiture/skin, intégré
directement dans la zone héros à la place de l'image, sans barre de titre,
sans dépendance à Content Manager. `video.ini` sauvegardé/restauré
proprement autour de la session.

**Outil de diagnostic conservé et committé** : `tools/showroom-embed-test/`
(reparenting Win32 hors Tauri/WebView2, modes `--overlay` et
`--steal-focus`, config machine en dur dans `main.rs` — à adapter si le
dossier d'install AC change). Utile pour toute future évolution de ce
système sans repasser par un cycle de rebuild complet de l'app à chaque
essai.

**Améliorations envisagées, non détaillées pour l'instant** (l'utilisateur
a mentionné avoir des idées, à reprendre plus tard) — pistes déjà visibles
dans le code actuel qui pourraient en faire partie : choix de la scène
showroom (`TRACK=` actuellement fixé à `showroom`, alors que
`content/showroom/` propose aussi `beach`, `Hangar`, `industrial`,
`studio_white`), réglages caméra/rotation exposés à l'utilisateur plutôt que
codés en dur, et éventuellement appliquer le même principe aux circuits.

## Optimisations (2026-07-05 → 2026-07-06)

Série de retours utilisateur sur l'aperçu intégré, traités un par un.

### Scène `studio_white` — musique + vitesse (RÉSOLU, committé)

`TRACK=showroom` → **`TRACK=studio_white`** dans `showroom_start.ini`.
Découverte décisive en inspectant `content/showroom/` : le poids **et la
musique** viennent de la scène. Tailles réelles : `showroom` 101 Mo,
`Hangar` 301 Mo, `industrial` 234 Mo, `beach` 275 Mo, **`studio_white`
17 Ko**. `studio_white` n'a **ni `.bank` ni `.wav`** → chargement quasi
instantané et **aucune musique** à couper (règle d'un coup « pas de musique »
et « chargement plus rapide »). Contrepartie assumée : fond blanc (pas foncé).
Piste « scène foncée sur mesure » (petit `.kn5`) écartée pour l'instant
(chantier format KN5). Le showroom **« Empty (Fast) » de Content Manager** est
un preset de son **renderer maison** (fichier `AcTools Content Manager/Data/
Built-in Presets/Custom Showroom/Default/Empty (Fast).cmpreset`), **pas** une
scène `content/showroom/` → **non chargeable par `acShowroom.exe`**. Abandonné.

### Exposition trop sombre (RÉSOLU, committé) — vrai coupable = filtre PP

Symptôme : aperçu trop sombre. **Fausse piste** explorée puis annulée :
remonter `[SETTINGS] CAMERA_EXPOSURE` (30 → 80) dans `showroom_start.ini`.
**Vraie cause trouvée par l'utilisateur** : le **filtre post-traitement actif**
(Pure) assombrit le showroom. Correctif retenu : forcer **`[POST_PROCESS]
FILTER=photographic`** (filtre stock AC, « rend très bien »). Emplacement du
filtre actif = `Documents/Assetto Corsa/cfg/video.ini`, clé `[POST_PROCESS]
FILTER=` (unique dans le fichier, à côté de `ENABLED=1`). `video.ini` étant
**déjà sauvegardé/restauré** autour de la session (mode fenêtré), le
`FILTER=pure` d'origine est remis intact à la fermeture — **zéro nouveau
fichier touché**. `CAMERA_EXPOSURE` remis à 30 ; `force_windowed()` renommé
`patch_video_for_showroom()` (force fenêtré + impose le filtre). Réserve non
levée : si CSP/Pure réimpose son filtre via `extension/…` malgré `video.ini`,
il faudrait viser le réglage CSP — mais en pratique le correctif suffit.

### Cadre 16:9 fixe + suivi du déplacement de fenêtre (FAIT, committé)

- **Cadre** : le héros voiture (`DetailPage.svelte`) a désormais un
  `aspect-ratio: 16/9` fixe (`.row.top:not(.track) .hero`, `align-self:
  start`, `min-height:0`) → l'image et l'aperçu 3D occupent exactement la même
  boîte, plus de saut de hauteur à l'attache (le rendu natif 1280×720 est aussi
  en 16:9).
- **Déplacement** : la fenêtre native est posée en **coordonnées écran
  absolues** → elle ne suivait pas le déplacement de la fenêtre principale
  (seuls `resize`/`scroll` étaient écoutés, pas le move). Ajout de
  `getCurrentWindow().onMoved(...)` → `reposition` dans l'effet de suivi.

### Masquer le menu du showroom (NON RÉSOLU)

Objectif : cacher le menu/UI du showroom au démarrage. Trois tentatives, **toutes
infructueuses** :

1. Touche **`H`** (raccourci courant de masquage HUD/apps dans AC) : testée
   manuellement dans le showroom → **ne masque pas** le menu.
2. **Extraction des chaînes du binaire** `acShowroom.exe` (PowerShell,
   `ReadAllBytes` + regex `[\x20-\x7E]{4,}`, 237 023 chaînes — `strings` Unix
   indisponible dans l'environnement, d'où un premier faux négatif) : n'expose
   que des **noms de classes RTTI** (`ShowroomGUI`, `ShowRoomCameraManager`,
   `ShowroomSkinManager`, `ShowroomMirrorCamera`, `ShowroomScreen`,
   `ShowroomSuspensionAnimator`), **aucune** chaîne de raccourci clavier ni clé
   INI de type `HIDE_*`/`SHOW_*`/`*_GUI`/`MENU`.
3. **`ALLOW_SELECT_SKIN=0`** dans `showroom_start.ini` : testé → **aucun
   changement**, le menu reste affiché.

Statut : aucun levier trouvé. Le « menu » visible n'est piloté ni par les clés
INI connues, ni par `H`, ni par `ALLOW_SELECT_SKIN`. Pistes non encore
explorées : envoi d'autres touches candidates (via `PostMessage` après
attache), ou identification de l'élément d'UI réel via inspection de la fenêtre
enfant du showroom (classes filles ?).

### Fenêtre noire au démarrage (NON RÉSOLU — 1re tentative annulée)

`acShowroom.exe` crée sa fenêtre top-level **visible immédiatement** (noire
pendant l'init DirectX), affichée jusqu'à ce qu'`attach()` la reparente (poll
jusqu'à 10 s). D'où un flash noir au lancement de l'aperçu.

**Tentative 1 (ANNULÉE, régression)** : dans `attach()`, `SW_HIDE` de la
fenêtre dès sa détection + overlay créé **sans** `WS_VISIBLE` + affichage
`ShowWindow(enfant)` puis `ShowWindow(overlay)` seulement après reparentage +
poll resserré 250 ms → 80 ms (**4 changements d'un coup**). **Résultat :
régression** — la fenêtre noire restait visible **ET** le process survivait à
la fermeture de Pit Box (orphelin, plus dans le gestionnaire des tâches
auparavant). Diagnostic : la séquence hide/show a déstabilisé le reparentage
fragile — la fenêtre n'était plus reparentée dans l'overlay (donc restait
top-level & visible, et n'était plus détruite en cascade à la fermeture de la
fenêtre principale → process orphelin). **Revert intégral** à la version qui
marchait. Leçon : ne changer qu'**une** variable à la fois sur ce chemin Win32
fragile (idéalement via `tools/showroom-embed-test/`).

**Tentative 2 (idée utilisateur — ÉCHEC)** : réduire la fenêtre top-level à
**1×1 px** (`SetWindowPos`, position (0,0), taille 1×1) dès sa détection, sans
toucher à sa visibilité ni à l'overlay. **Ne change rien** : la fenêtre noire
reste visible en plein. Conclusion importante : `acShowroom.exe` **réasserte
lui-même la géométrie (et la visibilité) de sa fenêtre pendant l'init**
(cohérent avec les symboles `OnWindowResize`/`OnWindowResizeEvent` du binaire)
→ toute manipulation externe *après* la création de la fenêtre est écrasée.
Corollaire : le seul levier exploitable est l'**état initial de la fenêtre à la
naissance du process**, pas une correction a posteriori.

**Tentative 3 (STARTUPINFO SW_HIDE, implémentée 2026-07-06, EN ATTENTE DE
TEST)** : lancer `acShowroom.exe` via `CreateProcessW` avec
`STARTUPINFOW.dwFlags = STARTF_USESHOWWINDOW` et `wShowWindow = SW_HIDE` — la
fenêtre **naît masquée** (méthode Windows canonique, au bon niveau : avant même
le `WinMain` de l'exe). `attach()`, **strictement inchangé** par ailleurs vs la
version fonctionnelle, la ré-affiche via `ShowWindow(target, SW_SHOW)` juste
après le reparentage dans l'overlay. Passage de `std::process::Command` à
`CreateProcessW` : le suivi de fin de process (pour restaurer `video.ini`) se
fait désormais via `WaitForSingleObject` sur le handle du process (HANDLE
traversé entre threads via son entier brut, non-`Send`). Features du crate
`windows` ajoutées : `Win32_System_Threading`, `Win32_Security` (CreateProcessW
est gated derrière Security). **Réserve** : si acShowroom appelle lui-même
`ShowWindow(hwnd, SW_SHOW)` pendant l'init (ignorant `nCmdShow`), le flash
reviendra — au pire on retombe sur le comportement actuel, **sans régression**
(le filet `restore_orphaned_video_ini` couvre toujours `video.ini` au prochain
démarrage). Si ça échoue aussi : itérer via `tools/showroom-embed-test/` (sans
rebuild complet de l'app), ou accepter le flash.

## Bug intermittent : « handle de fenêtre non valide » (2026-07-10)

Retour utilisateur : `attach()` échoue **parfois, pas toujours**, avec
`0x80070578` (`ERROR_INVALID_WINDOW_HANDLE`).

**Diagnostic** : `attach()` trouve le HWND top-level du showroom sur le thread
appelant (`find_showroom_window`, poll jusqu'à 10 s), puis dispatch le
reparentage vers le **thread principal** via `run_on_main_thread` (nécessaire,
voir Test réel n°3). Entre les deux, un délai existe — le temps que Tauri
planifie l'exécution de la closure sur le thread principal. Or `acShowroom.exe`
peut **détruire et recréer sa fenêtre pendant l'initialisation DirectX**
(cohérent avec le flash noir déjà documenté plus haut, "Fenêtre noire au
démarrage") : le HWND capturé côté thread appelant devient alors obsolète avant
que la closure ne s'exécute, et tout appel Win32 dessus (`SetParent`,
`SetWindowLongPtrW`…) échoue avec `ERROR_INVALID_WINDOW_HANDLE`. Le caractère
intermittent s'explique par la variabilité du délai de planification du thread
principal (charge de rendu WebView2 au moment du clic).

**Corrigé** (`src-tauri/src/showroom.rs`) :
- Nouvelle fonction `wait_for_valid_window(pid, deadline, poll)` : combine
  `find_showroom_window` + `IsWindow` — ne renvoie un HWND que s'il est
  **encore vivant** au moment du test, sinon continue de sonder.
- La recherche initiale (thread appelant, budget 10 s) utilise désormais cette
  fonction au lieu de `find_showroom_window` seul.
- **Re-vérification côté thread principal** : juste avant les appels Win32
  mutants, `IsWindow(target)` est revérifié. S'il est devenu invalide (fenêtre
  détruite/recréée entre-temps), une **re-recherche bornée** (2 s, pas de
  50 ms — budget court car sur le thread principal, ne doit pas geler l'UI)
  retrouve la fenêtre définitive avant de poursuivre.
- Ne couvre que le flux `attach()` (celui où le bug a été signalé) — pas
  `reposition()`/`close()`, non concernés par le rapport utilisateur.

**Non testable unitairement** (nécessite un vrai process `acShowroom.exe` et
une vraie fenêtre native) — 3 tests existants (écriture INI, patch video.ini,
sauvegarde/restauration) inchangés et toujours verts. **En attente d'un test
réel** pour confirmer que l'erreur intermittente a disparu.

---

## Verdict final (2026-08-03) — intégration abandonnée

Malgré les correctifs successifs documentés ci-dessus, l'aperçu 3D **intégré à
la page** est resté trop fragile en usage réel. Décision : **on n'intègre plus
rien**.

Ce qui a été retiré de `src-tauri/src/showroom.rs` et de `DetailPage.svelte` :
- la fenêtre overlay `PitboxShowroomOverlay`, le reparentage (`SetParent`,
  bidouille de styles `WS_CHILD`), le suivi resize/scroll/`onMoved` ;
- le spawn masqué (`STARTF_USESHOWWINDOW` + `SW_HIDE`) et la recherche de
  fenêtre par classe `acShowroomW` ;
- la **réécriture de `video.ini`** (mode fenêtré 1280×720 + filtre
  `photographic`) et sa restauration — le fichier du vrai jeu n'est plus touché
  du tout ; seule subsiste `restore_orphaned_video_ini()`, filet de sécurité
  pour les sauvegardes laissées par les versions précédentes ;
- les commandes `attach_native_showroom` / `reposition_native_showroom` /
  `close_native_showroom`, l'état `ShowroomState`, et la dépendance `windows`
  du `Cargo.toml` (plus aucun appel Win32) ;
- l'option de réglage « aperçu 3D par défaut ».

Ce qui reste : le bouton « Aperçu 3D » lance `acShowroom.exe` en **process
indépendant**, avec les réglages vidéo du jeu (donc plein écran si c'est ainsi
qu'AC est configuré). L'utilisateur ferme le showroom lui-même pour revenir à
Pit Box. Nouveau réglage à la place de l'ancien : le **décor** utilisé
(`content/showroom/<id>`, liste déroulante des showrooms installés, défaut
`studio_white`).
