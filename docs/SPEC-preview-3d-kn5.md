# SPEC — Aperçu 3D natif des voitures Assetto Corsa (KN5)

Ce que fait l'aperçu 3D de la fiche voiture, et les règles qu'il faut connaître
pour le modifier sans le casser. **Les mesures, les bancs d'essai et les pistes
essayées puis écartées sont dans `preview3d-research.md`** ; ce que le format
KN5 fait réellement quand il s'écarte du §3, dans `kn5-format.md`. Ce que
l'utilisateur voit de l'aperçu (zone héros, bascule photo/3D, coexistence avec
le showroom de CM) est décrit au SESSION§4.

---

## 0. Contexte et contraintes

À la place de l'image `preview.jpg`, la fiche voiture affiche un **aperçu 3D
interactif** du modèle, sur un plateau qui tourne.

- **Aucun processus externe** : ni `acShowroom.exe`, ni Custom Showroom, ni
  binaire tiers. Tout est natif à l'application.
- Le rendu vit **dans la webview** (panneau redimensionnable, défilement,
  thème), jamais dans une fenêtre native superposée.
- Seuls les fichiers du jeu **installé par l'utilisateur** sont lus ; rien de
  Kunos n'est redistribué.
- **Non-objectifs** : la fidélité aux shaders AC/CSP (le rendu vise un PBR
  propre et flatteur), les animations hors braquage (suspension, portes,
  dégâts), les circuits, et le déchiffrement des KN5 protégés.

---

## 1. Décision d'architecture (ADR-001)

**Le KN5 est lu en Rust, converti en glTF, et rendu par three.js dans la
webview.** Une surface native superposée ne participe pas au layout (ni
défilement, ni coupe, ni coins arrondis), et des images rendues hors écran
puis envoyées à la webview coûtent trop cher en IPC pour de l'interactif. La
webview a WebGL2, three.js apporte l'orbite, l'éclairage par carte
d'environnement et le tone mapping, et le glTF est un **intermédiaire
inspectable** : un modèle converti s'ouvre dans Blender, ce qui rend le
convertisseur vérifiable sans l'interface. La conversion se fait une fois par
voiture et se met en cache.

---

## 2. Références externes et statut juridique

Le format KN5 n'est pas documenté officiellement. **La description d'un format
se réimplémente librement ; le code d'un tiers est soumis à sa licence.** Le
parseur est donc écrit de zéro, et les projets publics ne servent qu'à lever
une ambiguïté :

| Projet | Licence | Usage |
|---|---|---|
| [`gro-ove/actools`](https://github.com/gro-ove/actools) (Content Manager) | MS-PL | référence la plus complète — lire pour comprendre, jamais transcrire |
| [`RaduMC/kn5-converter`](https://github.com/RaduMC/kn5-converter), [`MarvinSt/kn5-obj-converter`](https://github.com/MarvinSt/kn5-obj-converter) | aucune | consultation seulement, aucune copie |
| [`atirutw/atirut.kn5-importer`](https://github.com/atirutw/atirut.kn5-importer), [`JosepOli/ac-kn5-viewer`](https://github.com/JosepOli/ac-kn5-viewer) | MIT | points de comparaison |
| [`SeizureSaladd/Kn5Decrypt`](https://github.com/SeizureSaladd/Kn5Decrypt) | GPL-3.0 | déchiffrement des KN5 protégés — **jamais intégré** |

---

## 3. Format KN5 — layout binaire

La disposition que lit le parseur (`crates/kn5`). Tous les entiers sont
**little-endian** ; une chaîne est un `i32` de longueur suivi d'autant
d'octets **UTF-8**, sans terminateur. Chaque point ci-dessous a été confirmé
sur la bibliothèque de référence ; le détail des écarts avec ce qui était
attendu, et la méthode, sont dans `kn5-format.md`.

### 3.1 En-tête

```
magic       : [u8; 6]   // "sc6969"
version     : u32        // 5 ou 6 typiquement
if version > 5:
    extra   : u32        // ignoré
```

Magic absent → erreur `NotAKn5File`. Version supérieure à 6 → avertissement,
et la lecture est tentée quand même.

### 3.2 Section Textures

```
texture_count : i32
répété texture_count fois :
    tex_type  : i32          // 1 = embarquée ; 0 = entrée sans données
    name      : String       // ex. "car_paint.dds"
    size      : i32
    data      : [u8; size]
```

Le blob est le plus souvent du DDS, parfois du PNG ou du JPEG : **le format se
lit sur les premiers octets**, jamais sur l'extension du nom.

### 3.3 Section Matériaux

```
material_count : i32
répété material_count fois :
    name        : String     // ex. "carpaint"
    shader      : String     // ex. "ksPerPixelMultiMap_damage_dirt"
    blend_mode  : u8         // 0 opaque, 1 fondu (kn5-format.md, PREVIEW§12 q2)
    alpha_test  : u8         // booléen « alpha testé »
    if version > 4:
        _reserved : i32
    prop_count  : i32
    répété prop_count fois :
        prop_name  : String
        prop_value : f32
        extra      : [f32; 9]   // nul à 99,8 %, lu et gardé, inutilisé (PREVIEW§12 q5)
    sampler_count : i32
    répété sampler_count fois :
        sampler_name : String   // "txDiffuse", "txNormal", …
        slot         : i32
        texture_name : String   // clé vers la section Textures
```

**Toutes** les propriétés et tous les samplers sont conservés, pas seulement
ceux que le rendu utilise : les mods CSP en ajoutent.

### 3.4 Section Nœuds (arbre, parcours préfixe récursif)

Un seul nœud racine, lu juste après les matériaux. Chaque nœud :

```
node_type      : i32   // 1 = dummy, 2 = mesh, 3 = skinned mesh
name           : String
children_count : i32
is_active      : u8
```

**Type 1 — dummy / transform** : `matrix : [f32; 16]`, convention DirectX
ligne-vecteur (translation en `m[12..15]`).

**Type 2 — mesh** :
```
cast_shadows, is_visible, is_transparent : u8 x3   // ordre confirmé (PREVIEW§12 q1)
vertex_count : i32
répété vertex_count fois :        // 44 octets
    position [f32;3], normal [f32;3], uv [f32;2], tangent [f32;3]
index_count : i32
indices     : [u16; index_count]  // liste de triangles
material_id : i32
layer : u32, lod_in : f32, lod_out : f32,
bounding_sphere_center : [f32;3], bounding_sphere_radius : f32,
is_renderable : u8
```

**Type 3 — skinned mesh** : les trois drapeaux, puis `bone_count : i32` et
pour chaque os `bone_name : String` + `inverse_bind_matrix : [f32; 16]`, puis
des sommets de 76 octets (les champs du type 2 plus `bone_weights [f32;4]` et
`bone_indices [f32;4]`, stockés en flottants), les indices, `material_id`, et
`layer`, `lod_in`, `lod_out`.

**Enfants** : après le corps du nœud, `children_count` nœuds lus
récursivement — sans supposer que seuls les dummies en ont. **Transformations**
en ligne-vecteur : `world = local × parent_world` ; un mesh n'a pas de matrice
propre, il hérite de celle de son parent.

### 3.5 Nœuds à ignorer

Écartés à la conversion :

- `is_renderable == 0` ou `is_visible == 0` ;
- nom commençant par `AC_` (repères : `AC_POS_0`, `AC_PIT_0`…) ;
- nom contenant `COLLIDER`, `_SHADOW`, `AC_CRASH`, `DAMAGE_GLASS`, `BLUR` ;
- maillages sans sommet ou sans indice.

**Un motif de nom écarte le nœud et tout son sous-arbre ; un préfixe n'écarte
que le nœud.** Le nom qui trahit l'accessoire est porté par le groupe
(`RIM_BLUR_LF` contient `Object190`), alors que le seul nœud de voiture en
`AC_` de la bibliothèque, `ac_black_metal`, contient de la vraie garniture. La
liste est un paramètre du convertisseur, pas un littéral au milieu du parseur.

---

## 4. Résolution des assets côté Assetto Corsa

### 4.1 Arborescence

```
<AC>/content/cars/<car_id>/
    <model>.kn5, <model>_lodb.kn5…   collider.kn5 (ignoré)
    data.acd ou data/                ui/ui_car.json, ui/badge.png
    extension/ext_config.ini         driver_base_pos.knh
    animations/steer.ksanim
    skins/<skin_id>/  *.dds, preview.jpg, livery.png, ext_config.ini
```

### 4.2 Choix du fichier modèle

Le modèle est celui que nomme `[LOD_0] FILE=` de `data/lods.ini` quand le
dossier `data/` est en clair ; sinon le `.kn5` de la racine de la voiture, hors
`collider.kn5` et hors `_lodb/_lodc/_lodd/_lode`, et s'il en reste plusieurs le
plus volumineux. Le choix du LOD par l'utilisateur n'est pas implémenté
(`CHANTIERS.md`).

### 4.3 Skins

Le skin sélectionné surcharge les textures du KN5 **par nom de fichier** : une
texture présente dans `skins/<skin_id>/` remplace celle du modèle. Le skin
entre donc dans la clé de cache (§5.3).

### 4.4 Système de coordonnées

**Aucune conversion de repère et aucune inversion de V** : le KN5 se lit tel
quel en glTF. Une conversion (négation d'axe, `v = 1 - v`) est un défaut, pas
une correction oubliée — l'histoire de cette erreur, commise deux fois, est
dans `kn5-format.md` (PREVIEW§12 q4). `kn5-tool` porte un garde de régression
sur une voiture à conduite à gauche dont la livrée porte du texte.

### 4.5 KN5 protégés (CSP)

Un KN5 sans magic valide (`NotAKn5File` — le parseur ne distingue pas un
fichier chiffré d'un fichier corrompu) n'est **jamais déchiffré**.
`preview::prepare` rend `errors.previewProtected` et la fiche retombe sur la
photo, avec un badge discret « aperçu 3D indisponible » et une infobulle. Rien
d'agressif : l'aperçu est un bonus visuel.

### 4.5bis Magic valide, géométrie protégée

Certains mods se lisent sans erreur mais ont un **tampon d'index brouillé** :
sommets intacts, triangles qui relient n'importe quoi. `preview::prepare` le
détecte avant de convertir — la part des triangles dont l'enroulement s'accorde
à la normale stockée (`kn5_gltf::is_geometry_sane`) : ~100 % sur un modèle
sain, ~50 % sur un modèle protégé, seuil à 90 % — et applique le même repli
qu'au §4.5. **On ne cherche pas à rattraper ces modèles** : un rendu double
face a été essayé, il montre une toile de triangles. `kn5-tool convert`, lui,
convertit et avertit seulement, pour rester un outil d'inspection.

### 4.5ter Modèles étendus par CSP (`ext_config.ini`)

Beaucoup de mods livrent un KN5 volontairement incomplet et laissent CSP
greffer, skin par skin, les pièces qui changent (jantes, boucliers, optiques).
**Ces greffes sont appliquées avant la conversion**, pour que l'aperçu montre
la voiture telle qu'elle roule.

- **Fichiers lus**, du plus précis au plus général : l'`ext_config.ini` du
  skin, celui de la voiture, son `materials.ini`, puis la config que CSP livre
  pour la voiture (`<AC>/extension/config/cars/loaded/`), plus les fichiers
  frères qu'ils incluent par `[INCLUDE: …]`. Un `[INCLUDE: common/…]` (template
  de CSP) n'est pas suivi.
- **Ce qui est appliqué** : les sections `[MODEL_REPLACEMENT_*]` littérales
  (`ACTIVE`, `FILE`, `SKINS`, `HIDE`, `INSERT` avec `INSERT_IN` ou
  `INSERT_AFTER`, `OFFSET`, `ROTATION`, `SCALE`, `MULTIPLE`), le template
  `[ReplaceRims]` transcrit à la main, les `[Material_*]` (§6.3) et les
  `[REFRACTING_HEADLIGHT_…]`, qui disent qu'un maillage est une optique.
- **Ce qui ne l'est pas** : le langage de templates de CSP (expressions,
  générateurs, `read()`), et les `[SHADER_REPLACEMENT_*]`, qui changent l'aspect
  d'une surface et non son existence.
- **Deux limites, dans le sens « trop plutôt que pas assez »** : un `ACTIVE` qui
  est une expression est tenu pour actif (seul un `0` littéral désactive) ; et
  la taille de jante visée par CSP vit dans `data/tyres.ini`, que l'aperçu ne
  lit pas — à défaut d'un `Radius`/`Width` explicite, la jante garde la taille
  de son propre modèle.
- Les `ext_config.ini` entrent dans la **clé de cache** (`preview::cache_key`) :
  corriger une ligne de config reconvertit.

### 4.6 Le pilote

Assetto Corsa range un pilote en trois endroits, dont aucun n'est la voiture :
le **mannequin** 3D (`<AC>/content/driver/<nom>.kn5`, nommé par
`driver3d.ini` de la voiture), la **tenue** (`content/texture/driver_{suit,gloves,helmet}/…`,
nommée par le `skin.ini` de la livrée, sous le nom du mannequin) et la
**place assise** (`DRIVEREYES` de `car.ini`). La tenue surcharge le mannequin
par nom de fichier, comme un skin surcharge une voiture.

- **Seul le casque est lié au mannequin** : les mannequins Kunos réclament tous
  les mêmes combinaison et gants, mais un casque daté (`HELMET_2012`,
  `HELMET_1985`…). D'où l'écran Pilote et ses trois listes (`SPEC-ecran-pilote.md`).
- Le mannequin est lu, habillé, puis **greffé dans le modèle de la voiture
  après la passe CSP** (§4.5ter), sous une racine neuve, par la même fusion
  d'assets. `driver3d.ini` et `car.ini` se lisent dans `data.acd` déchiffré
  (`acd::read_text`) quand la voiture est packagée.
- **Non implémenté** : les `HIDE_OBJECT_*` de `driver3d.ini` (vue cockpit
  seulement), le `POSITION` de `driver3d.ini` (il ressemble à un réglage fin
  et n'en est pas un), la substitution de mannequin par `ext_config.ini`.
- **Réglage à trois valeurs** dans Réglages › Aperçu : toujours, jamais, ou
  **au démarrage du moteur** (le défaut) — le pilote s'installe en fondu de
  0,45 s quand une clé de contact tourne sur la fiche, et repart quand elle se
  coupe. « Toujours » et « au démarrage » partagent la même entrée de cache :
  seule la vue les distingue, le mannequin étant déjà dans le modèle (la vue le
  retrouve au préfixe `PITBOX_DRIVER:` de ses maillages). « Jamais » n'entre
  pas dans la clé.

### 4.6bis Les mains sur le volant

**Rien n'est calculé : la pose vient du moddeur.** L'assise est donnée par
`<voiture>/driver_base_pos.knh` (le rig entier, placé dans la voiture) et les
bras par `animations/steer.ksanim` (formats décrits dans `kn5::knh` et
`kn5::ksanim`). **Le fichier qui a placé le pilote fait foi** : la hiérarchie
d'abord, l'animation à défaut, `DRIVEREYES` en dernier recours — et c'est là
seulement que l'écart œil / os de tête (10 cm au-dessus, 8 cm devant) sert.
Le skinning est donc réel (normales et tangentes comprises), sans quoi le
corps resterait en arrière d'un rig posé.

**Le braquage est un réglage, en degrés aux roues avant, de −35° à +35°**, au
degré près. Il tourne trois choses :

- **les coins avant** — chacun en trois nœuds frères, `WHEEL_?F` (pneu et
  jante), `DISC_?F`, `SUSP_?F` (fusée et étrier), qui tournent ensemble autour
  du **centre de la roue**, mesuré avant le parcours de l'arbre ;
- **le volant** (`STEER_HR`/`STEER_LR`), de l'angle multiplié par la
  démultiplication de la voiture (`STEER_RATIO`), et lui seul s'arrête à la
  course déclarée (`STEER_LOCK`) ; l'axe de sa colonne est mesuré sur la
  géométrie, son pivot est le milieu de la géométrie et non l'origine du nœud ;
- **les bras du pilote**, à l'image de l'animation qui correspond à l'angle du
  volant.

**Aucune butée par voiture sur les roues** : `STEER_LOCK` est le débattement
utile d'un volant de simulation, pas la butée mécanique, et les voitures de
course y déclarent très peu (12° à la Huracán GT3).

**L'angle n'est jamais cuit dans le modèle**, et c'est ce qui rend le réglage
gratuit et instantané. La conversion **décrit** ce qui tourne — chaque nœud
braqué sort en nœud glTF à part, relatif à son pivot, l'axe et le facteur en
`extras` (`pitboxSteer`) — et le mannequin est exporté **vivant** : os en
nœuds, peau en `skin`, pièces rigides en enfants de l'os qui les porte,
animation de braquage en animation glTF. La vue n'écrit qu'une rotation et
choisit un instant du clip. L'angle n'entre donc pas dans la clé de cache ;
ce que la voiture déclare (`STEER_LOCK`, `STEER_RATIO`, les bornes mesurées
de l'animation) y entre, puisque c'est écrit dans le modèle. La fusion par
matériau sépare les nœuds braqués : deux roues ne partagent pas un pivot. Un
mannequin sans peau ni animation exploitable retombe sur une pose cuite.

---

## 5. Pipeline de conversion (Rust)

### 5.1 Découpage en crates

```
src-tauri/crates/
  kn5/        parsing pur : ni Tauri, ni réseau — parse(bytes) -> Kn5Model
  kn5-gltf/   Kn5Model -> glTF, textures, greffes CSP, pilote, braquage
  kn5-tool/   CLI de validation (inspect / convert / bench), jamais livrée
src-tauri/src/preview.rs   commandes, cache, protocole
```

### 5.2 Robustesse du parseur

Le KN5 vient d'un mod téléchargé : **un fichier mal formé ne fait jamais
tomber l'application.** Aucun `unwrap()` ni `panic!()` sur une donnée lue ;
toute allocation déduite d'un champ du fichier est bornée avant d'allouer ; la
profondeur de l'arbre de nœuds est bornée ; un `material_id` hors bornes est
une erreur, pas un accès.

### 5.3 Cache

- **Sur disque**, dans `app_cache_dir()/previews/` : une voiture déjà vue
  s'ouvre sans reconversion, y compris après un redémarrage.
- **La clé** porte le `.kn5` (chemin, date, taille), le skin, les
  `ext_config.ini`, le pilote, ce que la voiture déclare de sa direction et la
  **version du convertisseur** (`preview::CONVERTER_VERSION`), qui est aussi
  dans le **nom** de l'entrée : une entrée d'une autre version n'est jamais
  servie, et elle est **effacée à chaque passe d'éviction** — pas seulement au
  premier aperçu, sans quoi une autre instance ouverte en même temps repose
  ses entrées juste après le ménage.
- **Une entrée est un `.gltf` qui cite des blobs adressés par leur contenu**
  (`previews/blobs/<sha256>.<bin|png|jpg>`) : les skins d'une même voiture
  partagent leur géométrie et leurs textures communes. Les blobs s'écrivent
  **avant** le document, sous un nom temporaire renommé ensuite ; un blob ne
  s'efface qu'une fois partie la dernière entrée qui le cite, et le balayage
  relit les entrées survivantes plutôt que de se fier à ce qu'il a compté.
  `kn5-tool`, lui, produit un `.glb` autonome qui s'ouvre tel quel dans
  Blender.
- **Le plafond est un réglage** (Réglages › Aperçu › Cache), de 0,5 à 20 Go,
  2 Go par défaut, poussé au backend par le frontend, qui le borne lui-même.
  **Le baisser évince tout de suite.** L'écran affiche la place réellement
  occupée, et un bouton vide le cache.
- L'éviction garde les entrées les plus récentes, entrée et blobs cumulés,
  jusqu'au plafond.

### 5.4 Textures

1. Le format réel du blob est lu sur ses octets ; DDS compressés (BC1–BC7)
   **et** non compressés (une texture d'AC sur huit) sont décodés.
2. Taille ramenée à **2048** px de côté pour les cartes de couleur, **1024**
   pour les cartes de données.
3. Encodage : **JPEG** pour une carte de couleur sans alpha utile, **PNG**
   sinon ; une carte de données opaque passe en JPEG (sans sous-échantillonnage
   de chrominance) **seulement s'il est plus petit** que le PNG. Un canal alpha
   constant à 255 n'est pas écrit. Pas de WebP : il exigerait une extension que
   Blender et les visualiseurs glTF ne lisent pas.
4. Les textures sont **dédupliquées** : une texture citée par plusieurs
   matériaux n'est transcodée et stockée qu'une fois.

La géométrie est écrite en attributs quantifiés (`KHR_mesh_quantization`,
seule extension *requise* du projet : un lecteur qui l'ignorerait lirait des
entiers comme des flottants). `POSITION` reste en flottants.

---

## 6. Mapping matériaux KN5 → glTF PBR

Le shader d'AC n'est pas un modèle métal/rugosité : **la conversion est une
approximation assumée, et elle ne doit jamais échouer.**

### 6.1 Règles de base

| Source KN5 | glTF |
|---|---|
| `txDiffuse` | couleur de base (sRGB) |
| `txNormal` | carte de normales — **sauf sur un shader `*_damage*`**, où c'est la carte de dégâts (kn5-format.md, écart n°4) |
| `ksEmissive` > 0 | `emissiveFactor` |
| `ksSpecularEXP` | rugosité de base, par une approximation calibrée à l'œil |
| `ksAlphaRef` | découpe (`MASK`) ; `0` veut dire « non réglé » |
| `blend_mode = 1` | fondu **si** l'alpha s'étale entre 0 et 255 ; une découpe (alpha à 0 ou 255) passe en `MASK`, rendue avec l'opaque (§8.2) |

### 6.2 `txMaps` et la métallicité

- **Le canal vert de `txMaps` est la brillance** → rugosité par pixel.
- **R et B ne portent rien d'exploitable**, et c'est une réponse mesurée : ils
  restent inutilisés.
- **La métallicité vient de `fresnelC`** (la réflectance à incidence normale),
  vetée par `fresnelMaxLevel`.
- **La peinture** vient de la carte de détail du skin, masquée par l'alpha de
  la diffuse, et elle est **cuite dans une variante de la texture** plutôt que
  posée en facteur : le masque est par pixel, et glTF borne le facteur à 1.
- L'alpha d'une texture diffuse que **aucun matériau n'emploie comme découpe**
  est retiré avant encodage : chez AC il code le masque de peinture, pas la
  transparence.

Détail, chiffres et méthode : `kn5-format.md`, écarts n°3 à 27.

### 6.3 Shaders spécifiques

- Vitres (`ksWindscreen`…) : leur `txDiffuse` est une carte de saleté, pas une
  couleur, et le maillage de vitre brisée (`ksBrokenGlass`) est écarté.
- Un matériau déclaré `[Material_Glass]` par sa config CSP devient du verre
  **physique** (`KHR_materials_transmission` + `KHR_materials_ior`).
- `ksPerPixelAlpha` : son opacité est sa propriété `alpha`, qui l'emporte sur
  la texture.
- `ksTyres` : rugosité élevée, non métallique.
- **Shader inconnu : matériau standard et un `warn` au journal**, jamais une
  erreur.

---

## 7. Intégration Tauri

### 7.1 Commandes

`prepare_car_preview(car_id, skin_id)` convertit au besoin et rend l'adresse
du modèle (`CarPreview` : adresse, compteurs, indication « venu du cache ») ou
une erreur traduisible (modèle introuvable, protégé, illisible).
`clear_preview_cache` vide le cache et rend les octets libérés.

### 7.2 Transport du modèle

**Le modèle ne passe jamais par l'IPC** : sérialisé, un modèle de 30 Mo
gèlerait l'interface. Un protocole dédié (`carpreview`) sert les fichiers du
cache en octets bruts, et `GLTFLoader` les demande lui-même.

### 7.3 Concurrence

La conversion tourne **hors du fil de la fenêtre**. Parcourir la liste vite ne
laisse aucune conversion orpheline : une demande devenue obsolète est
abandonnée. Un événement de progression alimente l'indicateur de chargement.

---

## 8. Front-end — composant viewer

### 8.1 La scène

- **Un studio sombre à rampes zénithales**, procédural et sans asset
  (`showroomEnvironment.ts`) ; c'est `scene.environmentIntensity` qui le dose.
- **Le cadrage est calculé** sur les dimensions du modèle, pas codé en dur ;
  focale de 20° par défaut, trois-quarts avant **gauche** comme les photos du
  jeu.
- **Le sol** porte une flaque de lumière, l'ombre portée de la voiture (une
  lumière directionnelle d'intensité nulle n'existe que pour la projeter) et
  **le reflet de la voiture** (`floorMirror.ts`) : flouté, éteint radialement,
  composé par-dessus la flaque. Le reflet ne montre que la voiture ; sa cible
  suit la taille du tampon de rendu ; à 0 % la passe miroir n'existe pas.
- **La manipulation** : orbite et zoom à la souris, sans panoramique, angle
  polaire borné au-dessus du sol.

### 8.2 Transparence

Les matériaux en fondu sont rendus après l'opaque, sans écriture de
profondeur. **Mais le renoncement à la profondeur ne vaut que pour ce qui est
vraiment translucide** : une décalcomanie déclarée en fondu dont l'alpha ne
prend que ses deux extrêmes est rendue en découpe, dans la passe opaque, où la
profondeur arbitre — sinon l'ordre des matériaux décide de qui passe devant.
Les bords d'une découpe sont lissés par `alphaToCoverage`. Le verre en est
exempté.

### 8.3 Cycle de vie

À chaque changement de voiture et au démontage, **tout ce que la scène a mis
sur la carte graphique est libéré** (géométries, matériaux, textures, puis le
contexte au démontage) : parcourir deux cents voitures ne doit pas faire
croître la mémoire.

### 8.4 Boucle de rendu

Le plateau tourne en continu (un tour en ~28 s à 100 %), c'est la présentation
par défaut ; l'avance se calcule sur le temps écoulé, pour qu'un écran à
144 Hz ne double pas la vitesse. **Le rendu s'arrête quand rien ne bouge** :
plateau à l'arrêt, fiche hors de l'écran, fenêtre en arrière-plan.
**Le plateau s'arrête dès qu'on attrape le modèle et ne repart pas tout
seul** ; le bouton de remise en place le relance. Il ne démarre pas si le
système demande de réduire les animations.

### 8.5 États UI

| État | Affichage |
|---|---|
| Préparation | rien d'autre qu'un témoin de chargement au centre |
| Succès | le modèle |
| Changement de skin | le modèle précédent continue de tourner, badge discret ; le nouveau reprend plateau et caméra où l'ancien les a laissés ; un échec garde l'ancien |
| Protégé, introuvable, illisible | la photo, avec le badge « aperçu 3D indisponible » et sa raison |
| WebGL indisponible | la photo, silencieusement |

**Rien de tout cela ne ressemble à une erreur bloquante.**

---

## 10. Pièges connus — checklist

Chacun a déjà été commis ou failli l'être ; un correctif dans ces zones se
vérifie contre cette liste.

- [ ] Une **conversion de repère** ou une inversion de V réintroduite (§4.4).
- [ ] Une matrice transposée « pour la forme » : KN5 en ligne-vecteur rangé par
      lignes et glTF en colonne-vecteur rangé par colonnes donnent la même
      suite d'octets — ne rien transposer.
- [ ] L'ordre `local × parent` inversé.
- [ ] Une **fuite GPU** au changement de voiture (§8.3).
- [ ] Le modèle passé par l'IPC (§7.2).
- [ ] Des textures 4K non réduites (§5.4).
- [ ] `ksAlphaRef` ignoré : calandres et jantes ajourées rendues pleines.
- [ ] Une vitre qui écrit sa profondeur : habitacle invisible.
- [ ] Un **panic** sur un mod mal formé (§5.2).
- [ ] Une évolution du mapping sans incrément de `CONVERTER_VERSION` : les
      anciens rendus restent servis (§5.3).

---

## 11. Tests

Les tests du parseur couvrent les chaînes, les bornes d'allocation, la limite
de récursion, la détection du format des images et une transformation de
matrice vérifiée à la main. Les tests de corpus (`kn5-tool`, chemins locaux
jamais commités) convertissent toute la bibliothèque de référence et vérifient
qu'une voiture mesure entre 1 et 8 m — le meilleur détecteur d'une erreur
d'échelle ou de repère.

---

## 12. Questions ouvertes à trancher pendant l'implémentation

Toutes tranchées ; la réponse et sa méthode sont dans `kn5-format.md`, sous le
titre de la question.

1. **q1** — ordre des trois drapeaux d'un maillage : celui du §3.4, confirmé.
2. **q2** — `blend_mode` : deux octets, mode de fusion puis « alpha testé ».
3. **q3** — sens des canaux de `txMaps` : vert = brillance, R et B
   inexploitables ; la métallicité vient de `fresnelC` (§6.2).
4. **q4** — axe à inverser : aucun (§4.4).
5. **q5** — les 36 octets après chaque propriété : neuf flottants presque
   toujours nuls, sans usage.

---

## 15. Réglages et rendu de l'aperçu

Les réglages vivent dans **Réglages › Aperçu**, qui porte son propre aperçu
(la voiture de la session, à défaut la première de la bibliothèque) : on règle
en voyant le résultat. Ils sont dans `ui_prefs.json` ; les leviers de rendu
que l'utilisateur ne règle pas sont regroupés dans
`src/lib/preview3d/tuning.ts`. **Rien de ce qui suit n'entre dans la
conversion** — en changer s'applique à l'image suivante, sans reconvertir —
sauf le pilote (§4.6). Les numéros sont ceux du journal du lot 6
(`preview3d-research.md`), que le code cite.

1. **Carrosserie** : la carte de dégâts d'un shader `*_damage*` n'est pas
   appliquée (§6.1).
2. **Peinture** : cuite depuis la carte de détail du skin (§6.2).
3. **Vitrage** : la diffuse d'une vitre est une carte de saleté, la vitre
   brisée est écartée, et un alpha constant est une opacité, pas une découpe.
4. **Réglages exposés, et rien ne s'enregistre tout seul.** Les réglages
   s'appliquent à l'aperçu à l'instant, mais n'atteignent le disque qu'au clic
   sur Enregistrer ; Annuler revient sur l'enregistré, et quitter l'écran avec
   des changements en attente demande quoi en faire. **Seule exception : la
   bascule photo/3D de la fiche**, un interrupteur d'un clic, enregistré
   sur-le-champ.
6. **Rugosité par pixel** depuis le vert de `txMaps` (§6.2).
7. **Cadrage et éclairage** : focale 20° et trois-quarts avant gauche, calés
   sur les `preview.jpg` du jeu ; zoom 140 %, plongée 6°, hauteur −13 % et
   plateau à 50 % sont les valeurs choisies par l'utilisateur. Les défauts ne
   valent que pour une préférence jamais écrite : le bouton « rétablir » d'un
   groupe les fait apparaître. Le `preview.jpg` sert de référence pour le
   cadrage, **jamais pour la luminosité** : il est plus sombre que le rendu du
   jeu.
8. **Anticrénelage** : MSAA du contexte, filtrage anisotrope au maximum de la
   carte, `alphaToCoverage` sur les découpes, et le suréchantillonnage du
   point 17. Aucune passe de post-traitement : SMAA a été essayé, mesuré et
   retiré.
9. **Géométrie protégée** : refusée avant conversion (§4.5bis).
10. **Effet d'entrée du plateau** : *Aucun*, *Progressif* (le défaut, une
    montée sur 1,2 s) ou *Lancé* (cinq fois la vitesse réglée, qui décroît en
    ~2,6 s). Il démarre quand le modèle arrive à l'écran, jamais sur un
    changement de skin à chaud, jamais plateau à l'arrêt ni quand le système
    demande moins d'animations ; « replacer la voiture » le rejoue, et changer
    d'effet dans les Réglages aussi.
11. **Métallicité** depuis `fresnelC` (§6.2).
12. **Fusion des maillages par matériau** : un appel de dessin par matériau ;
    la clé de fusion inclut la transparence et sépare les nœuds braqués ; les
    index ne passent en 32 bits que quand il le faut.
13. **Reflet au sol** (§8.1), avec ses trois réglages : intensité, flou,
    portée. Défauts : reflet 85 %, flou 0,5, portée 75 %, flaque 85 %, ombre
    50 % ; la portée ne descend pas sous 20 %, en dessous le reflet s'éteint
    avant d'atteindre la voiture.
14. **Réglages de décor** : exposition, intensité de l'éclairage, focale — la
    focale recalcule la distance pour que la voiture garde sa taille. Les
    curseurs sont rangés en groupes, chacun avec son bouton de remise à zéro.
15. **L'aperçu est dans l'écran Réglages** ; la fiche voiture n'en garde qu'un
    raccourci vers l'onglet.
16. **Le reflet se rafraîchit à chaque image**, sur une cible à la taille du
    tampon de rendu, sans MSAA, avec un flou qui lit le niveau de mip dont le
    texel vaut le pas du flou. Devant un défaut de l'aperçu, la première
    question est : **est-il sur l'image fixe, ou entre deux images ?**
17. **Deux niveaux de qualité, et pas d'autre facteur** : Standard rend à la
    densité de l'écran, Élevée au double. Le navigateur réduit le canevas en
    une seule prise bilinéaire, qui n'est juste qu'au rapport 2 ; tout autre
    rapport dégrade l'image. Un budget de pixels fait redescendre le niveau,
    jamais le facteur. Une ancienne préférence « Ultra » se lit comme Élevée.
18. **Contre le scintillement en mouvement** : un flou de l'environnement de
    0,08 et un plancher de rugosité de 0,15, injecté dans le shader (et non sur
    `material.roughness`, que three multiplie par la carte). Le niveau de
    qualité n'a aucun effet sur le scintillement. L'anticrénelage spéculaire
    géométrique a été essayé et ne fait rien ici.

### 15.0quater Déduplication entre skins — le cache éclaté

Le rangement en document + blobs adressés par leur contenu est décrit au
§5.3 ; ses mesures (−65 % sur quatre skins d'une même voiture) et les deux
pièges rencontrés en l'écrivant sont dans `preview3d-research.md`.
