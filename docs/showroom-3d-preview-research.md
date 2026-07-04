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
   les sessions, `acmanager://race/config?configFile=...`, voir
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
