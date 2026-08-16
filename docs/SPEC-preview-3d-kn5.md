# SPEC — Aperçu 3D natif des voitures Assetto Corsa (KN5)

> Document de spécification destiné à Claude Code.
> À placer dans le dépôt sous `docs/SPEC-preview-3d-kn5.md` et à référencer depuis `CLAUDE.md`.

---

## 0. Contexte et contraintes

**Application cible** : gestionnaire de mods Assetto Corsa, Rust + Tauri.

**Objectif** : quand l'utilisateur sélectionne une voiture dans la liste, afficher un **aperçu 3D interactif** du modèle (orbite, zoom) à la place de l'image statique `preview.jpg` utilisée par Content Manager.

**Contraintes dures** :

- **Aucun processus externe.** Pas de `acShowroom.exe`, pas de Custom Showroom, pas de binaire tiers lancé en sous-processus. Tout doit être natif à l'application.
- Le rendu doit s'intégrer **dans le layout de la webview** (panneau redimensionnable, scroll, thèmes), pas dans une fenêtre native superposée.
- Les fichiers lus sont ceux du jeu **installé localement par l'utilisateur**. Aucune redistribution d'assets Kunos.
- Cible principale : **Windows** (AC est Windows-only), donc WebView2 / Chromium — WebGL2 est disponible de façon fiable.

**Non-objectifs (v1)** :

- Fidélité visuelle au rendu in-game (shaders AC / CSP). On vise un rendu PBR « propre et flatteur », pas une reproduction.
- Animations, suspension, portes, driver, damage.
- Rendu des circuits (`track.kn5`) — l'architecture doit toutefois rester réutilisable.
- Déchiffrement des KN5 protégés par CSP (voir §4.5).

---

## 1. Décision d'architecture (ADR-001)

### Options évaluées

| # | Approche | Verdict |
|---|---|---|
| A | Rendu natif **wgpu** dans une surface enfant (HWND/NSView) superposée à la webview | ❌ |
| B | Rendu natif **wgpu offscreen** → frames streamées vers la webview | ❌ |
| C | **Parsing KN5 en Rust → conversion glTF → rendu three.js dans la webview** | ✅ **retenu** |

**Pourquoi pas A** : une surface native superposée ne participe pas au layout DOM. Elle ne scrolle pas, ne se clippe pas, ignore le z-index, casse les animations de panneau et les coins arrondis. Sur Windows il faut gérer le redimensionnement et l'ordre de composition à la main. Coût d'intégration élevé pour un panneau d'aperçu.

**Pourquoi pas B** : encoder puis transférer ~60 images/s en IPC est coûteux en CPU et en latence, et l'interaction (orbite à la souris) devient molle. Acceptable pour une capture ponctuelle, pas pour de l'interactif.

**Pourquoi C** :

- La webview embarque déjà un moteur WebGL2 mature. Zéro problème de compositing : le canvas est un élément DOM comme un autre.
- three.js fournit gratuitement OrbitControls, l'environment mapping IBL, le tone mapping ACES, le frustum culling — tout ce qui fait qu'un aperçu voiture est joli.
- Le glTF est un **artefact intermédiaire inspectable** : on peut ouvrir le `.glb` produit dans Blender ou n'importe quel viewer glTF pour valider le parser, indépendamment de l'UI. Cela rend le développement testable par étapes.
- Le résultat est **cachable** : la conversion se fait une fois par voiture, puis on ne charge qu'un fichier.
- Le crate de parsing reste réutilisable plus tard (extraction de miniatures, outils CLI, rendu de circuits).

**Coût accepté** : un temps de conversion au premier affichage d'une voiture (cible < 2 s), masqué par un skeleton + fallback sur `preview.jpg`.

---

## 2. Références externes et statut juridique

Le format KN5 est propriétaire et non documenté officiellement. Plusieurs rétro-ingénieries publiques existent. **Distinction importante** : la *description d'un format de fichier* (offsets, types, ordre des champs) relève du fait technique et peut être réimplémentée librement ; le *code source* d'un tiers est soumis à sa licence.

| Projet | Langage | Licence | Usage recommandé |
|---|---|---|---|
| [`gro-ove/actools`](https://github.com/gro-ove/actools) (Content Manager, namespace `AcTools.Kn5File`) | C# | **MS-PL** | Référence la plus complète et la plus à jour. MS-PL est permissive : la redistribution binaire d'un dérivé est autorisée, mais toute redistribution **sous forme de source** d'un dérivé doit rester en MS-PL. → **Lire pour comprendre, ne pas transcrire ligne à ligne.** |
| [`RaduMC/kn5-converter`](https://github.com/RaduMC/kn5-converter) | C# | **aucune** | Pas de licence = tous droits réservés. Consultation uniquement, **pas de copie**. |
| [`MarvinSt/kn5-obj-converter`](https://github.com/MarvinSt/kn5-obj-converter) | Python | **aucune** | Idem. Port du précédent, lisible ; utile pour vérifier une compréhension du layout. |
| [`atirutw/atirut.kn5-importer`](https://github.com/atirutw/atirut.kn5-importer) | GDScript | **MIT** | Implémentation permissive, petite ; bon point de comparaison. |
| [`JosepOli/ac-kn5-viewer`](https://github.com/JosepOli/ac-kn5-viewer) | Go + three.js | **MIT** | Précédent le plus proche de notre architecture (parser natif + three.js). À étudier en priorité. |
| [`SeizureSaladd/Kn5Decrypt`](https://github.com/SeizureSaladd/Kn5Decrypt) | C# | **GPL-3.0** | Déchiffrement des KN5 protégés CSP. **Incompatible** avec une app propriétaire — ne pas intégrer. Hors périmètre. |
| [site.hagn.io — Kn5 Files](https://site.hagn.io/assettocorsa/modding/kn5-files) | — | doc | Vue d'ensemble textuelle du format (3 sections : textures / matériaux / nœuds). |
| [assettocorsamods.net — shaders & texture maps](https://assettocorsamods.net/threads/assetto-corsa-shaders-texture-maps-list.794/) | — | doc | Liste des shaders AC et de la sémantique des slots de texture. Indispensable pour §6. |

**Règle pour l'implémentation** : écrire un parser Rust *from scratch* à partir de la spec du §3 ci-dessous. Les dépôts ci-dessus servent à lever les ambiguïtés, pas de base de code.

---

## 3. Format KN5 — layout binaire

⚠️ Layout vérifié sur une implémentation de référence, mais **à valider par l'implémentation** contre un échantillon de voitures réelles (voir §12). Tous les entiers sont **little-endian**. Les chaînes sont préfixées par leur longueur : `i32 len` suivi de `len` octets **UTF-8** (pas de terminateur nul).

### 3.1 En-tête

```
magic       : [u8; 6]   // "sc6969"
version     : u32        // 5 ou 6 typiquement
if version > 5:
    extra   : u32        // champ additionnel, ignoré
```

Si `magic != "sc6969"` → erreur `NotAKn5File`.
Si `version > 6` → log d'avertissement, tentative de parse quand même.

### 3.2 Section Textures

```
texture_count : i32
répété texture_count fois :
    tex_type  : i32          // 1 = actif/embarqué ; 0 rencontré = pas de données
    name      : String       // ex. "car_paint.dds"
    size      : i32
    data      : [u8; size]   // blob brut
```

Le blob est **le plus souvent du DDS**, mais pas toujours (PNG/JPG possibles selon l'auteur du mod). **Ne pas se fier à l'extension du nom** : sniffer les magic bytes (`DDS ` / `\x89PNG` / `\xFF\xD8\xFF`).

### 3.3 Section Matériaux

```
material_count : i32
répété material_count fois :
    name        : String     // ex. "carpaint"
    shader      : String     // ex. "ksPerPixelMultiMap_damage_dirt"
    blend_mode  : i16        // 0=opaque, 1=alpha blend, 2=alpha to coverage (à confirmer)
    if version > 4:
        _reserved : i32      // observé à 0

    prop_count  : i32
    répété prop_count fois :
        prop_name  : String
        prop_value : f32
        _padding   : [u8; 36]   // valeur vectorielle (vec4 + reste) non utilisée en v1

    sampler_count : i32
    répété sampler_count fois :
        sampler_name : String   // "txDiffuse", "txNormal", ...
        slot         : i32
        texture_name : String   // clé vers la section Textures
```

**Propriétés scalaires courantes** : `ksAmbient`, `ksDiffuse`, `ksSpecular`, `ksSpecularEXP`, `ksEmissive`, `ksAlphaRef`, `diffuseMult`, `normalMult`, `useDetail`, `detailUVMultiplier`, `fresnelC`, `fresnelEXP`, `fresnelMaxLevel`, `isAdditive`.

**Slots de texture courants** : `txDiffuse`, `txNormal`, `txMaps`, `txDetail`, `txDetailR/G/B/A`, `txDetailNM`, `txMask`, `txVariation`.

> Conserver **toutes** les propriétés et tous les samplers dans une `HashMap<String, f32>` / `HashMap<String, String>`, pas seulement ceux listés. Les mods CSP en ajoutent, et on veut pouvoir les exposer plus tard dans un panneau de debug.

### 3.4 Section Nœuds (arbre, parcours préfixe récursif)

Un seul nœud racine, lu immédiatement après la section matériaux. Chaque nœud :

```
node_type      : i32   // 1 = dummy, 2 = mesh, 3 = skinned mesh
name           : String
children_count : i32
is_active      : u8
```

Puis, selon `node_type` :

**Type 1 — dummy / transform**
```
matrix : [f32; 16]   // 4x4, convention DirectX ligne-vecteur :
                     // la translation est en m[12..15] (dernière ligne)
```

**Type 2 — mesh**
```
cast_shadows   : u8
is_visible     : u8      // ordre des 3 flags à confirmer empiriquement
is_transparent : u8
vertex_count   : i32
répété vertex_count fois :        // stride = 44 octets
    position : [f32; 3]
    normal   : [f32; 3]
    uv       : [f32; 2]
    tangent  : [f32; 3]
index_count : i32
indices     : [u16; index_count]  // triangle list
material_id : i32                 // index dans la section Matériaux
// puis 29 octets :
layer                  : u32
lod_in                 : f32
lod_out                : f32
bounding_sphere_center : [f32; 3]
bounding_sphere_radius : f32
is_renderable          : u8
```

**Type 3 — skinned mesh**
```
cast_shadows, is_visible, is_transparent : u8 x3
bone_count : i32
répété bone_count fois :
    bone_name : String
    inverse_bind_matrix : [f32; 16]
vertex_count : i32
répété vertex_count fois :        // stride = 76 octets
    position       : [f32; 3]
    normal         : [f32; 3]
    uv             : [f32; 2]
    tangent        : [f32; 3]
    bone_weights   : [f32; 4]
    bone_indices   : [f32; 4]     // stockés en float
index_count : i32
indices     : [u16; index_count]
material_id : i32
// puis 12 octets : layer(u32), lod_in(f32), lod_out(f32)
```

**Enfants** : après le corps du nœud, lire récursivement `children_count` nœuds. En pratique seuls les nœuds de type 1 ont des enfants, mais le parser ne doit pas le supposer.

**Transformations** : convention ligne-vecteur, donc `world = local × parent_world` (et non l'inverse). Les nœuds mesh n'ont pas de matrice propre : ils héritent de celle de leur parent. En v1, on peut **aplatir** l'arbre et pré-transformer les positions/normales en espace monde à l'écriture du glTF — plus simple, et suffisant pour un aperçu statique. Conserver malgré tout la hiérarchie dans le modèle intermédiaire.

### 3.5 Nœuds à ignorer

Filtrer à la conversion :

- `is_renderable == 0` ou `is_visible == 0`
- nom commençant par `AC_` (helpers : `AC_POS_0`, `AC_START_0`, `AC_PIT_0`…)
- noms contenant `COLLIDER`, `_SHADOW`, `AC_CRASH`, `DAMAGE_GLASS`
- meshes à `vertex_count == 0` ou `index_count == 0`

Ce filtrage doit être **paramétrable** (liste de patterns dans la config du convertisseur), pas codé en dur au milieu du parser.

---

## 4. Résolution des assets côté Assetto Corsa

### 4.1 Arborescence

```
<AC_ROOT>/content/cars/<car_id>/
    <model>.kn5              ← modèle principal (LOD A)
    <model>_lodb.kn5, _lodc.kn5, _lodd.kn5   (parfois)
    collider.kn5             ← à ignorer
    data.acd                 ← archive chiffrée (ou dossier data/ en clair)
    ui/ui_car.json, badge.png
    skins/<skin_id>/
        *.dds                ← surcharges de textures
        preview.jpg, livery.png, ui_skin.json
```

### 4.2 Choix du fichier modèle

Ordre de résolution :

1. Si `data/lods.ini` est présent en clair, lire `[LOD_0] FILE=`.
2. Sinon heuristique : tous les `*.kn5` de la racine du dossier voiture, **hors** `collider.kn5` et hors ceux dont le nom se termine par `_lodb/_lodc/_lodd`. S'il en reste plusieurs, prendre le plus volumineux.
3. Exposer une option de config pour forcer un LOD inférieur (aperçu plus rapide sur machines modestes).

Ne pas déchiffrer `data.acd` en v1 — l'heuristique suffit dans l'immense majorité des cas.

### 4.3 Skins

Le skin sélectionné surcharge les textures embarquées **par nom de fichier**. À la conversion, pour chaque texture référencée par un matériau : si `skins/<skin_id>/<texture_name>` existe sur disque, utiliser ce fichier ; sinon utiliser le blob embarqué dans le KN5.

Conséquence : **la clé de cache inclut le skin** (§5.3). Le skin par défaut est le premier par ordre alphabétique, ou celui indiqué par la config de l'app.

### 4.4 Système de coordonnées

AC utilise un repère **main gauche, Y vers le haut**. glTF utilise **main droite, Y vers le haut, +Z vers l'observateur**.

Conversion à appliquer : **négation d'un axe + inversion du winding des triangles** (sinon toutes les faces sont retournées).

⚠️ Le choix de l'axe à négliger (X ou Z) détermine si le modèle est *miroir*. **Ne pas deviner : valider empiriquement.** Test d'acceptation :

- charger une Kunos à conduite à gauche (ex. `ks_mazda_mx5_cup`) → le volant doit être **à gauche** vu de derrière la voiture ;
- charger une voiture dont le skin porte du texte (numéro, sponsor) → le texte doit être **lisible**, pas en miroir.

Si le test échoue, changer d'axe. Documenter le résultat dans un commentaire au-dessus de la constante.

**UV** : la coordonnée V est inversée par rapport à la convention glTF → appliquer `v = 1.0 - v`.

### 4.5 KN5 protégés (CSP)

Certains mods payants publient des KN5 chiffrés. Détection : magic absent ou incohérent, ou données de section aberrantes.

**Comportement attendu** : ne pas tenter de déchiffrer. Le convertisseur retourne `Kn5Error::Protected`, l'UI retombe silencieusement sur `preview.jpg` avec un petit badge « aperçu 3D indisponible » et une infobulle explicative. Aucun message d'erreur agressif.

---

## 5. Pipeline de conversion (Rust)

### 5.1 Découpage en crates

Le workspace doit isoler le parsing de Tauri, pour pouvoir tester sans lancer l'app.

```
crates/
  kn5/            # parsing pur, zéro dépendance à Tauri, zéro I/O réseau
                  #   lib: parse(bytes) -> Kn5Model
  kn5-gltf/       # Kn5Model -> GLB (+ transcodage textures)
  kn5-tool/       # binaire CLI de dev : inspect / convert / bench
src-tauri/        # commandes, cache, protocole custom
```

`kn5-tool` n'est pas livré à l'utilisateur : c'est l'outil de validation. Il doit exister **dès le premier lot** — c'est lui qui rend le reste testable.

### 5.2 Dépendances suggérées

| Besoin | Crate | Note |
|---|---|---|
| Lecture binaire | `byteorder` ou `binrw` | `binrw` rend le layout déclaratif et lisible ; préférable ici |
| Décodage DDS/BCn | `image_dds` | décode BC1–BC7 vers RGBA8 ; alternative plus limitée : `texpresso` |
| Images génériques | `image` | redimensionnement, encodage PNG/WebP |
| Écriture glTF | `gltf-json` + conteneur GLB écrit à la main | le conteneur GLB est trivial : header 12 o + chunk JSON + chunk BIN |
| Erreurs | `thiserror` | — |
| Parallélisme | `rayon` | transcodage des textures en parallèle |

**Sécurité du parser** : le KN5 vient d'un mod téléchargé par l'utilisateur, donc d'une source non fiable. Le parser doit être **robuste par construction** :

- aucun `unwrap()` / `panic!()` sur des données lues ;
- borner toute allocation dérivée d'un champ du fichier (`vertex_count`, `index_count`, `size` de texture, `len` de chaîne) par un plafond configurable avant d'allouer ;
- borner la profondeur de récursion de l'arbre de nœuds (ex. 256) → sinon un fichier malveillant provoque un stack overflow ;
- valider que `material_id` est dans les bornes ;
- fuzzing recommandé (`cargo-fuzz`) sur `parse()` une fois l'API stabilisée.

### 5.3 Cache

Chemin : `app_cache_dir()/previews/<hash>.glb`

Clé de hash (BLAKE3 ou xxhash) sur : `chemin absolu du kn5` + `mtime` + `taille` + `skin_id` + `version du convertisseur`.

Le champ **version du convertisseur** est indispensable : il invalide tout le cache quand le mapping matériaux évolue.

Prévoir une commande de purge (`clear_preview_cache`) et un plafond de taille du cache (ex. 2 Go, éviction LRU).

### 5.4 Textures

1. Sniffer le format réel du blob.
2. Décoder en RGBA8.
3. Redimensionner : côté max **2048** pour `txDiffuse`, **1024** pour les autres. Un aperçu n'a pas besoin de 4K, et cela divise le poids du GLB par 4 à 16.
4. Encoder :
   - couleur (`txDiffuse`) → **WebP qualité 85**
   - normal maps → **PNG** (le WebP lossy dégrade visiblement les normales)
5. Embarquer dans le GLB (buffer views), pas de fichiers annexes.

> KTX2 + Basis serait plus efficace mais ajoute une dépendance de transcodage côté web et le support S3TC n'est pas garanti partout (notamment GPU Apple). Rester sur WebP/PNG en v1.

Déduplication obligatoire : une même texture est référencée par plusieurs matériaux, elle ne doit être transcodée et stockée **qu'une fois**.

---

## 6. Mapping matériaux KN5 → glTF PBR

C'est la partie où la qualité visuelle se joue, et **la partie la plus incertaine**. Le shader AC n'est pas metallic/roughness : la conversion est une approximation assumée.

### 6.1 Règles de base

| Source KN5 | Cible glTF | Règle |
|---|---|---|
| `txDiffuse` | `pbrMetallicRoughness.baseColorTexture` | sRGB |
| `txNormal` | `normalTexture` | linéaire ; `scale` = `normalMult` si présent |
| `ksEmissive` > 0 | `emissiveFactor` | `[v, v, v]` clampé à 1.0 |
| `ksSpecular`, `ksSpecularEXP` | `roughnessFactor` | approximation : `roughness ≈ clamp(1 - sqrt(ksSpecularEXP / 250), 0.05, 1.0)` — **à calibrer visuellement**, ce n'est pas une conversion exacte |
| `ksAlphaRef` > 0 | `alphaMode: "MASK"`, `alphaCutoff` | typiquement les grilles, jantes ajourées |
| `blend_mode == 1` ou shader contenant `Glass`/`Windscreen` | `alphaMode: "BLEND"`, `doubleSided: false` | + `KHR_materials_transmission` optionnel |
| `txMaps` | — | **sémantique des canaux à déterminer** (voir ci-dessous) |

### 6.2 `txMaps` — à investiguer

Dans les shaders `ksPerPixelMultiMap*`, `txMaps` encode plusieurs masques dans les canaux R/G/B/A (typiquement autour de la spécularité, de la glossiness et de l'AO), mais **la répartition exacte des canaux doit être vérifiée**, pas supposée.

Démarche imposée à l'implémentation :

1. Consulter la liste de shaders d'assettocorsamods.net et le code de `AcTools.Render.Kn5Specific.Materials`.
2. Écrire la conclusion dans `docs/kn5-shaders.md` avec la source.
3. Seulement ensuite, mapper vers `metallicRoughnessTexture` (canal G = roughness, canal B = metallic selon la convention glTF) et éventuellement `occlusionTexture`.

Tant que ce n'est pas tranché : `metallicFactor = 0.0`, `roughnessFactor` issu de `ksSpecularEXP`, pas de `metallicRoughnessTexture`. Un rendu diffus correct vaut mieux qu'un rendu métallique faux.

### 6.3 Shaders spécifiques

- `ksPerPixelReflection`, `ksPerPixelMultiMap_damage_dirt` → traiter comme le cas standard.
- `ksTyres` → roughness élevée (≈ 0.9), metallic 0.
- Matériaux de vitre → `alphaMode: BLEND`, `roughness` basse ; leur ordre de rendu est géré côté three.js (§9).
- Shaders inconnus → **ne jamais échouer** : matériau standard par défaut + log `warn` listant le nom du shader. Collecter ces logs, ils orienteront les itérations suivantes.

---

## 7. Intégration Tauri

### 7.1 Commandes

```rust
#[tauri::command]
async fn prepare_car_preview(
    car_id: String,
    skin_id: Option<String>,
) -> Result<PreviewHandle, PreviewError>;

pub struct PreviewHandle {
    pub url: String,          // "carpreview://<hash>"
    pub triangle_count: u32,
    pub material_count: u32,
    pub from_cache: bool,
}

pub enum PreviewError {
    ModelNotFound,
    Protected,       // KN5 chiffré → fallback UI
    ParseFailed(String),
    Unsupported(String),
}
```

```rust
#[tauri::command]
async fn clear_preview_cache() -> Result<u64, String>; // octets libérés
```

### 7.2 Transport du GLB — point critique

**Ne pas renvoyer le GLB via l'IPC.** Sérialisé en JSON/base64, un modèle de 30 Mo devient ~40 Mo de chaîne à parser côté JS : blocage de l'UI et pic mémoire.

Enregistrer un **protocole custom** (`tauri::Builder::register_uri_scheme_protocol`) qui sert le fichier depuis le cache disque en octets bruts, avec :

- `Content-Type: model/gltf-binary`
- support de `Range` (permet à la webview de streamer)
- `Cache-Control: immutable` (le hash est déjà la clé de version)

Le front reçoit une URL et laisse `GLTFLoader` faire le fetch. Zéro copie inutile.

### 7.3 Concurrence

- La conversion est **bloquante et CPU-bound** → l'exécuter sur `tauri::async_runtime::spawn_blocking`, jamais sur le thread principal.
- Si l'utilisateur parcourt la liste rapidement, plusieurs conversions peuvent être demandées : **annuler celles devenues obsolètes** (garder un token de génération par sélection) et sérialiser les conversions (une seule à la fois, ou un pool de 2).
- Émettre un événement de progression (`preview://progress`) pour alimenter le skeleton UI.

---

## 8. Front-end — composant viewer

Framework-agnostique ; adapter au framework déjà utilisé dans le projet.

### 8.1 Setup three.js

- `WebGLRenderer` : `antialias: true`, `powerPreference: "high-performance"`, `toneMapping: ACESFilmicToneMapping`, `outputColorSpace: SRGBColorSpace`.
- Éclairage : **environment map IBL** via `RoomEnvironment` + `PMREMGenerator` (aucun asset externe à embarquer, et c'est ce qui donne un rendu de carrosserie crédible). Éventuellement une `DirectionalLight` faible pour l'accent.
- `OrbitControls` : `enableDamping: true`, `dampingFactor: 0.08`, `minDistance`/`maxDistance` dérivés du rayon de la bounding box, `enablePan: false`, angle polaire borné pour empêcher de passer sous le sol.
- Cadrage automatique : calculer la bbox du modèle chargé, positionner la caméra à ~2,2 × le rayon, léger angle de 3/4 avant (le cadrage le plus flatteur pour une voiture). Le cadrage doit être **calculé**, pas codé en dur — les mods ont des échelles variables.
- Ombre de contact : un simple plan avec une texture radiale, ou `ContactShadows`. Pas de shadow map en v1.

### 8.2 Transparence

Les vitres sont le principal piège visuel. Traitement :

- matériaux `BLEND` → `depthWrite = false`, `transparent = true`
- forcer `renderOrder = 1` sur les meshes transparents pour qu'ils passent après l'opaque
- si des artefacts persistent, envisager `side: FrontSide` strict et un tri par distance caméra

### 8.3 Cycle de vie — obligatoire

Les fuites mémoire GPU sont la première cause de crash dans ce type de composant : l'utilisateur parcourt 200 voitures, chacune laisse ses géométries et textures sur le GPU.

Au démontage **et à chaque changement de voiture** :

- `geometry.dispose()` sur toutes les géométries
- `material.dispose()` et `texture.dispose()` sur tout le graphe
- `renderer.dispose()` + `renderer.forceContextLoss()` au démontage
- annuler la boucle `requestAnimationFrame`

Écrire une fonction `disposeScene(root)` unique et l'appeler systématiquement. **Ajouter un test manuel documenté** : parcourir 50 voitures d'affilée et vérifier dans le Task Manager que la mémoire GPU se stabilise.

### 8.4 Boucle de rendu

Ne pas rendre en continu à 60 fps sur un panneau statique. Rendre **à la demande** : sur interaction OrbitControls (`change`), sur redimensionnement, et pendant l'inertie du damping. Cela divise la consommation CPU/GPU de l'app au repos.

### 8.5 États UI

| État | Affichage |
|---|---|
| Chargement | `preview.jpg` en fond flouté + spinner/skeleton |
| Succès | canvas 3D, fondu depuis l'image |
| `Protected` / `ModelNotFound` / `ParseFailed` | `preview.jpg` seule + badge discret « aperçu 3D indisponible » |
| WebGL indisponible | `preview.jpg` seule, silencieusement |

Le fallback ne doit **jamais** ressembler à une erreur bloquante. C'est un bonus visuel, pas une fonctionnalité critique.

---

## 9. Plan d'implémentation par lots

Chaque lot doit être livré **avec ses tests** et être vérifiable indépendamment de l'UI.

### Lot 0 — Fixtures et outillage
- Workspace `crates/kn5`, `crates/kn5-gltf`, `crates/kn5-tool`.
- `kn5-tool inspect <file.kn5>` : affiche version, nombre de textures / matériaux / nœuds, arbre des nœuds, liste des shaders rencontrés, total de triangles.
- **Critère d'acceptation** : `inspect` fonctionne sur 5 voitures Kunos et 5 mods communautaires sans panic.

### Lot 1 — Parser
- Implémentation complète du §3, structures `Kn5Model`, `Kn5Material`, `Kn5Node`.
- Bornes d'allocation, limite de récursion, zéro `unwrap`.
- **Critères** : round-trip des compteurs cohérent ; fichier tronqué à un offset arbitraire → `Err`, jamais de panic ; test dédié sur un fichier de 100 octets aléatoires.

### Lot 2 — Textures
- Sniff de format, décodage DDS/BCn, redimensionnement, encodage WebP/PNG, déduplication.
- `kn5-tool extract-textures` écrit les textures sur disque.
- **Critère** : les textures extraites d'une voiture Kunos s'ouvrent correctement dans une visionneuse et correspondent visuellement au skin du jeu.

### Lot 3 — Export glTF
- `kn5-tool convert <car_dir> -o out.glb`.
- Aplatissement des transforms, conversion de repère, winding, UV, mapping matériaux minimal (§6.1, sans `txMaps`).
- **Critères** : le `.glb` s'ouvre dans Blender **et** dans un viewer glTF en ligne ; le test du volant/texte du §4.4 passe ; aucune face retournée visible.

### Lot 4 — Cache + intégration Tauri
- Cache hashé, protocole custom, commandes, `spawn_blocking`, annulation.
- **Critères** : deuxième affichage d'une même voiture < 150 ms ; parcourir la liste rapidement ne laisse aucune conversion orpheline ni ne fige l'UI.

### Lot 5 — Viewer three.js
- Composant complet §8, avec états UI et `disposeScene`.
- **Critères** : 50 voitures parcourues d'affilée sans croissance monotone de la mémoire ; l'orbite reste fluide (> 50 fps) sur un modèle LOD A typique.

### Lot 6 — Finitions
- Sélection du skin avec rechargement, choix du LOD en config, `txMaps` une fois la sémantique documentée, ombre de contact, purge du cache dans les réglages.

---

## 10. Pièges connus — checklist

- [ ] **Winding inversé** après changement de repère → voiture « creuse », on voit l'intérieur.
- [ ] **UV non inversées** → textures à l'envers, très visible sur les liveries.
- [ ] **Transposition de matrice oubliée** (DirectX ligne-vecteur ↔ glTF colonne-vecteur) → pièces éparpillées dans l'espace.
- [ ] **Ordre `local × parent`** inversé → hiérarchie correcte mais positions fausses.
- [ ] **Fuite GPU** au changement de voiture (§8.3).
- [ ] **GLB passé par l'IPC** au lieu du protocole custom (§7.2).
- [ ] **Textures 4K non redimensionnées** → GLB de 200 Mo, conversion de 20 s.
- [ ] **`ksAlphaRef` ignoré** → jantes ajourées et calandres rendues pleines.
- [ ] **Vitres sans `depthWrite: false`** → intérieur invisible à travers le pare-brise.
- [ ] **Panic sur un mod mal formé** → crash de toute l'application pour un seul mauvais fichier.
- [ ] **Cache non versionné** → après une amélioration du mapping matériaux, les anciens rendus restent affichés.

---

## 11. Tests

**Corpus de test** (chemins à configurer, jamais commités) :

- 3 Kunos vanilla (une moderne, une historique, une monoplace)
- 3 mods communautaires de qualité variable
- 1 mod avec KN5 protégé CSP
- 1 fichier volontairement corrompu (KN5 valide tronqué à 60 %)
- 1 fichier de bruit aléatoire

**Tests unitaires** : parsing de chaînes, bornes d'allocation, limite de récursion, sniff de format d'image, conversion de matrices (vérifier une transformation connue à la main).

**Tests d'intégration** : `convert` sur tout le corpus ; assertions sur le nombre de meshes, la présence de textures, et la bbox (une voiture doit faire entre 1 et 8 m de long — un bon détecteur d'erreur d'échelle ou de repère).

**Bench** : `kn5-tool bench` mesure parse / transcodage textures / écriture GLB séparément. Cible totale < 2 s pour un LOD A typique sur une machine de milieu de gamme.

---

## 12. Questions ouvertes à trancher pendant l'implémentation

1. Ordre exact des 3 octets de flags des nœuds mesh (`cast_shadows` / `is_visible` / `is_transparent`) — à confirmer en croisant plusieurs voitures.
2. Sémantique du `i16` `blend_mode` des matériaux.
3. Sémantique des canaux de `txMaps` (§6.2).
4. Axe à négliger pour la conversion de repère (§4.4).
5. Contenu des 36 octets de padding après chaque propriété de matériau — probablement une valeur vectorielle ; sans intérêt en v1 mais à documenter.

Chaque réponse trouvée doit être consignée dans `docs/kn5-format.md` avec la méthode de vérification, pas seulement dans un commentaire de code.

---

## 13. État d'avancement (mis à jour au fil des lots)

> Section ajoutée **après** la rédaction initiale. Le corps de ce document
> reste la spécification de départ, y compris là où l'implémentation l'a
> contredite : les écarts sont listés au §14, avec leur raison. Quelqu'un qui
> reprend le chantier lit cette section-ci en premier.

Branche `feature/3dpreview`.

| Lot | État | Où |
| --- | --- | --- |
| 0 — outillage | ✅ | `crates/kn5`, `crates/kn5-tool` |
| 1 — parser | ✅ (fondu dans le lot 0) | `crates/kn5` |
| 2 — textures | ✅ | `crates/kn5-gltf/src/texture.rs` |
| 3 — export glTF | ✅ | `crates/kn5-gltf/src/{geometry,material,glb}.rs` |
| 4 — cache + Tauri | ✅ | `src-tauri/src/preview.rs`, `commands/preview.rs` |
| 5 — viewer three.js | ✅ | `src/lib/components/detail/CarPreview3D.svelte` |
| 6 — finitions | ⏳ **en cours** | voir §15 |

**Validé à l'écran par l'utilisateur** : l'aperçu s'affiche dans la fiche
voiture, tourne sur son socle, se manipule à la souris, et les textures se
posent au bon endroit.

**Corpus** : 198 voitures sur 201 de la bibliothèque de référence se parsent
et se convertissent sans échec (les 3 restantes n'ont pas de modèle).

---

## 14. Écarts assumés par rapport à cette spécification

Chacun a été pris en connaissance de cause, avec sa raison. Ne pas les
« corriger » vers la spec sans relire la raison.

### 14.1 Ajouts demandés par l'utilisateur en cours de route

- **Plateau tournant.** C'était le *but* du chantier, non écrit dans la
  demande initiale : la voiture tourne lentement sur elle-même, comme sur un
  socle de salon (un tour en ~28 s). C'est la voiture qui tourne, pas la
  caméra : le cadrage reste stable et l'état d'`OrbitControls` n'est jamais
  contrarié quand l'utilisateur prend la main.
  **Contredit le §8.4** (« rendre à la demande, pas en continu »), et les deux
  sont inconciliables. Contrepartie payée là où elle se voit : rotation
  suspendue hors écran, en arrière-plan, et pendant la manipulation (reprise
  après 4 s) ; désactivée si le système demande de réduire les animations ;
  avance calculée sur le temps écoulé, pour qu'un écran 144 Hz ne double pas
  la vitesse.
- **Manipulation à la souris** conservée par-dessus le plateau : orbite et
  zoom, sans panoramique, angle polaire borné pour ne pas passer sous le sol.
- **Coexistence avec le showroom natif** (`acShowroom.exe`, §9.4 du SPEC
  principal) au lieu du remplacement : les deux ne rendent pas le même
  service. Une bascule photo/3D mémorisée vit dans la zone héros.
- **Photo de repli non floutée** pendant la préparation : c'est déjà l'aperçu
  habituel de la fiche, la flouter la rendait illisible au moment où elle sert.

### 14.2 Écarts techniques

| Spec | Fait | Raison |
| --- | --- | --- |
| §4.4 négation d'un axe + inversion du winding | **Aucune conversion de repère** | Prémisse fausse. Voir `kn5-format.md` §12 q4 : deux mesures numériques le disaient dès le lot 3, une validation sur une voiture à l'atlas symétrique les a fait écarter à tort. |
| §4.4 `v = 1.0 - v` | **UV reprises telles quelles** | DirectX et glTF placent tous deux l'origine des textures en haut à gauche. L'inversion vaut pour OpenGL. |
| §5.2 `binrw` | Lecteur binaire écrit à la main | Chaque compteur doit être validé *avant* d'allouer ; l'exprimer en attributs coûtait plus que 130 lignes, pour une dépendance de plus. |
| §5.2 `gltf-json` | Document glTF écrit avec `serde_json` | Sous-ensemble petit et figé ; le test d'acceptation est empirique de toute façon. |
| §5.4 WebP q85 | **JPEG q85**, PNG si alpha utile | Le glTF de base n'accepte que PNG et JPEG ; WebP exige `EXT_texture_webp`, ce qui casse le critère « s'ouvre dans Blender ». `image` 0.25 n'encode plus le WebP. |
| §7.1 `PreviewHandle` | `CarPreview` | Nom déjà pris par `music::PreviewHandle`. |
| §5.1 `crates/` à la racine | `src-tauri/crates/` | Garde `target/`, `Cargo.lock` et les chemins CI où ils étaient. |
| §5.4 décodage BC1–BC7 | + **décodeur DDS par masques** | 12 % des textures AC sont des DDS non compressés qu'`image_dds` refuse. |
| — | **L'alpha des textures diffuses est retiré** quand aucun matériau ne l'exploite | Chez AC il ne code pas la transparence (82,5 % des pixels à alpha nul sur une carrosserie blanche). Le conserver efface la carrosserie. |

---

## 15. Reste à faire — lot 6

Points remontés par l'utilisateur après validation à l'écran, par ordre de
gêne constatée :

1. ~~**La carrosserie paraît cabossée.**~~ ✅ **Corrigé.** C'était bien une
   carte de dégâts appliquée à tort : sur un shader `*_damage*`, `txNormal`
   est la déformation des tôles, qu'AC ne mélange qu'à proportion des dégâts.
   Vérifié sur quatre voitures — voir `kn5-format.md`, écart n°4.
   *Reste possible plus tard* : exporter le `TANGENT` du KN5 (three.js
   reconstruit aujourd'hui les tangentes par dérivées d'écran), et vérifier le
   canal vert des normal maps DirectX. Non nécessaire pour ce défaut-ci.
2. **Couleur de peinture** ✅ **améliorée, pas résolue.** La teinte vient de la
   carte de détail du skin, pas de la diffuse — voir `kn5-format.md`, écart
   n°5. La Supra verte ressort désormais verte. La nuance exacte reste
   approchée : un facteur d'amplification calibré à l'œil compense une
   amplification du shader qu'on ne sait pas encore reproduire (§12 q3).
3. **Réglages à exposer** (écran Réglages) : aperçu photo ou 3D par défaut,
   niveau de zoom, angle de caméra autour de l'axe vertical, hauteur de
   caméra, vitesse de rotation. La voiture est aujourd'hui cadrée trop bas :
   quand le plateau la présente de face, l'avant est coupé.
4. **Éclairage et cadrage calés sur les `preview.jpg` Kunos**, pour que le
   passage de la photo à la 3D ne saute pas à l'œil. C'est le point qui
   demande le plus de tâtonnement visuel.

Restent aussi, hérités du plan initial : `txMaps` une fois sa sémantique
documentée (§6.2, §12 q3 — **toujours ouverte**), choix du LOD en config,
purge du cache depuis les Réglages, et l'aperçu dans `ModDetail.svelte`
(panneau latéral), qui n'a jamais été branché.
