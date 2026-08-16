# KN5 — ce que le format fait vraiment

Journal des points de format **vérifiés sur des fichiers réels**, en complément
de `SPEC-preview-3d-kn5.md` §3 (qui décrit le layout attendu) et §12 (qui liste
les questions laissées ouvertes). Chaque entrée dit **comment** la réponse a été
obtenue : une affirmation sans méthode n'a pas sa place ici.

Sauf mention contraire, la mesure vient d'un `kn5-tool scan` sur la
bibliothèque de référence de la machine de développement — **201 dossiers de
`content/cars`**, dont 198 avec un modèle exploitable (voir « Corpus » en fin de
document).

---

## Écart n°1 — une entrée de texture de type 0 ne contient rien du tout

**Spec §3.2** décrit chaque entrée de la section Textures comme
`type / name / size / data`, avec le commentaire « 1 = actif/embarqué ;
0 rencontré = pas de données ». Lu ainsi — un nom et une taille présents, un
blob vide — la section se désynchronise.

**Réel** : quand `type == 0`, l'entrée s'arrête là. Quatre octets, pas de nom,
pas de taille, pas de blob.

**Méthode** : `rss_gtm_lanzo_v8` était le seul échec du premier scan complet
(`texture_size` à 1 124 073 472). Vidage hexadécimal du début de sa section
Textures :

```
offset 14 : 89 00 00 00              texture_count = 137
offset 18 : 00 00 00 00              type = 0        ← marqueur, fin de l'entrée
offset 22 : 01 00 00 00              type = 1        ← entrée suivante
offset 26 : 0e 00 00 00              name_len = 14
offset 30 : "COCKPIT_LR.png"
offset 44 : 64 7d 0a 00              size = 687 972
offset 48 : 89 50 4e 47              magie PNG ✔
```

En lisant un nom après le type 0, le `size` tombait sur `0x43000000` — soit
exactement l'entier aberrant rapporté. En s'arrêtant au type 0, **chaque entrée
suivante retombe pile sur sa magie PNG/DDS**, et le fichier se parse
intégralement. C'est cet alignement qui fait la preuve, pas la simple
disparition de l'erreur.

**Fréquence** : 1 entrée de ce type sur 14 297 dans la bibliothèque de
référence. Rare, mais un seul cas suffisait à rendre une voiture illisible.

**Verrouillé par** `type_zero_texture_entry_holds_no_name_and_no_blob`
(`crates/kn5/src/parse.rs`).

---

## §12 q2 — le `blend_mode` des matériaux : deux octets, pas un `i16`

**Spec §3.3** : `blend_mode : i16 // 0=opaque, 1=alpha blend, 2=alpha to
coverage (à confirmer)`.

**Réel** : ce sont **deux `u8` indépendants** — un mode de fusion (`0` opaque,
`1` alpha blend) puis un booléen « alpha testé ». La valeur `2` n'existe pas.

**Méthode** : sur 10 083 matériaux, seules trois valeurs de l'`i16` apparaissent —
`0` (7 298×), `1` (1 984×) et `256` (801×). `256` vaut `0x0100` : octet bas à 0,
octet haut à 1. Croisement avec le nom du shader (`kn5-tool inspect
--materials`) :

| Valeur | Décodage | Matériaux concernés |
| --- | --- | --- |
| `0` | opaque | carrosserie, plastiques, jantes pleines |
| `1` | alpha blend | `EXT_Glass`, `INT_Glass`, `DAMAGE_GLASS`, décalcomanies |
| `256` | opaque + alpha testé | **uniquement** des shaders `*_AT` : `ksPerPixelAT`, `ksPerPixelMultiMap_AT` |

`AT` = *alpha test* dans la nomenclature Kunos : la corrélation est exacte, dans
les deux sens, et la combinaison `257` (blend **et** test) n'apparaît jamais —
ce qui est cohérent, les deux techniques s'excluant.

**Conséquence pour §6.1** : `alpha_tested` est le signal fiable pour
`alphaMode: "MASK"`, plus fiable que « `ksAlphaRef` > 0 ».

---

## §12 q1 — ordre des trois octets de flags d'un nœud mesh

**Spec §3.4** : `cast_shadows`, `is_visible`, `is_transparent`, « ordre à
confirmer empiriquement ».

**Réel** : l'ordre annoncé est le bon. Confirmé, pas seulement plausible.

**Méthode** : deux mesures convergentes sur 38 806 meshes.

1. **Distribution des combinaisons** — le deuxième octet vaut 1 sur 38 805
   meshes (une seule exception, un mesh également non visible côté rendu).
   Un drapeau « visible » quasi toujours vrai est exactement ce qu'on attend ;
   ni le premier ni le troisième n'ont ce profil.
2. **Croisement du troisième octet avec le shader du matériau** — les meshes
   marqués par ce troisième octet sont massivement du verre et des surfaces
   ajourées : `ksPerPixelReflection` (1 848), `ksBrokenGlass` (1 174),
   `ksWindscreen` (509), et toute la famille `*_AT`. Un octet qui sélectionne
   les vitres est `is_transparent`, pas `cast_shadows`.

Le premier octet est donc `cast_shadows` par élimination — 30 918 meshes à
`1 1 0`, soit l'immense majorité opaque projetant une ombre, ce qui est le
comportement par défaut attendu.

---

## §12 q5 — les 36 octets après chaque propriété de matériau

**Statut : sans intérêt pratique, question close.**

**Réel** : 9 flottants, **nuls dans 99,8 % des cas** — 218 propriétés non nulles
sur 125 850. Rien n'indique une valeur vectorielle réellement exploitée.

**Méthode** : `kn5-tool` lit ces 36 octets comme `[f32; 9]` (au lieu de les
sauter) et compte les propriétés dont au moins une composante est non nulle.
Le champ reste lu et conservé (`Kn5MaterialProperty::extra`) — il ne coûte
rien et évite d'avoir à refaire la mesure. Il n'est **pas** utilisé pour le
rendu.

---

## Écart n°2 — une texture sur huit est un DDS non compressé

**Spec §5.4** prévoit `image_dds` pour décoder « BC1–BC7 ». C'est nécessaire mais
**pas suffisant**.

**Réel** : une part importante des textures AC ne sont pas compressées par blocs
du tout. Ce sont des DDS hérités dont le format n'est décrit **que par les
masques de bits** de leurs canaux dans le bloc `DDS_PIXELFORMAT` — ni FourCC, ni
tag DXGI. `image_dds` les refuse toutes, avec le message
`DdsFormatInfo { dxgi: None, d3d: None, fourcc: None }`.

**Fréquence mesurée** sur 12 voitures (938 textures) : **117 échecs, soit 12 %**,
et jusqu'à **26 % sur `ks_ford_gt40`**. Sans correctif, ces textures manquent
purement et simplement à l'affichage.

**Correctif** : décodeur générique par masques dans `kn5-gltf`, en repli quand
`image_dds` échoue. Il ne connaît pas de formats nommés — il lit `rgb_bit_count`
et les quatre masques, ce qui couvre d'un coup A8R8G8B8, X8R8G8B8, R8G8B8,
A1R5G5B5, X1R5G5B5, R5G6B5, A4R4G4B4, L8, A8L8 et leurs variantes. Après
correctif : **0 échec** sur les mêmes 12 voitures (938 → 0).

⚠️ **Piège à ne pas refaire** : un canal de 5 bits ne se convertit pas en 8 bits
par un décalage. `value << 3` plafonne à 248 au lieu de 255 — un blanc devient
gris, sur toute la texture. Il faut une mise à l'échelle
(`value * 255 / (2^n - 1)`). Verrouillé par
`narrow_channels_stretch_to_full_range`.

**Vérification visuelle** : `leather_nm.dds` de `ks_mazda_mx5_cup` (format
X1R5G5B5, précédemment en échec) ressort en normal map bleu-violet canonique ;
`leather.dds` (luminance) ressort en cuir gris correct.

---

## §12 q4 — aucune conversion de repère, et aucune inversion de V

**Réponse : l'identité.** Ni négation d'axe, ni inversion de la coordonnée V,
contrairement à ce que demande le §4.4. Il a fallu **deux erreurs successives**
pour l'établir, et c'est cette histoire qui vaut d'être consignée.

### Ce que disaient les mesures numériques

Deux mesures indépendantes concluaient « identité » dès le lot 3 :

- **Les noms de roues.** `WHEEL_LF`/`WHEEL_RF` placés dans un repère droitier
  satisfont la relation `gauche = haut × avant` sans aucune négation.
- **L'accord enroulement/normales.** `(p1-p0) × (p2-p0)` en règle droitière
  s'accorde avec la normale stockée sur **100 % de 1,3 million de triangles**.

Elles avaient raison. Elles ont pourtant été écartées.

### Pourquoi elles ont été écartées à tort

Le rendu de `ks_mazda_mx5_cup` semblait exiger une négation de X *et* une
inversion de V : dans cette combinaison, le numéro `55` et `MX-5 CUP` se
lisaient correctement. **C'était une double coïncidence propre à cette
voiture** :

1. **L'îlot UV du flanc est tourné à 90°.** L'inversion de V agit donc
   *horizontalement* sur la carrosserie et **annule** le miroir géométrique.
   Le texte redevient lisible alors que le modèle est bel et bien en miroir.
2. **L'atlas range ses deux flancs côte à côte, quasi identiques.** Inverser V
   fait échantillonner le flanc gauche à la place du droit — invisible à l'œil.

### Ce qui a cassé l'illusion

`abarth500`. Son atlas place la **photo du compartiment moteur** exactement là
où l'inversion de V envoie la portière. Symptôme : une voiture qui paraît
translucide, alors qu'elle affiche simplement le mauvais morceau de son atlas.

Diagnostic par élimination, chaque étape écartant une hypothèse :

| Test | Résultat | Conclusion |
| --- | --- | --- |
| Tous matériaux en blanc opaque | silhouette pleine et parfaite | la géométrie est complète |
| Rendu double-face | inchangé | pas un problème de faces culées |
| Masquage des meshes transparents | inchangé | pas un problème de transparence |
| Superposition des îlots UV sur l'atlas | formes correctes | le découpage UV est bon |
| **Lancer de rayon sur la portière** | `uv=(0.515, 1.619)` | **62 % de la hauteur de l'atlas : le moteur.** Sans inversion : 38 %, le panneau |

### L'explication de fond

DirectX **et** glTF placent tous deux l'origine des textures **en haut à
gauche**. L'inversion de V est nécessaire pour aller vers OpenGL, pas vers
glTF. Le §4.4 confond les deux conventions.

⚠️ **Leçon de méthode** : valider une conversion sur une voiture dont l'atlas
est symétrique ne prouve rien. Le test doit porter sur **du texte** *et* sur une
zone d'atlas que la transformation fautive déplacerait visiblement. Le contrôle
automatique de `kn5-tool convert` est calibré sur `abarth500` pour cette raison.

---

## Écart n°3 — l'alpha d'une texture diffuse n'est pas de la transparence

**Mesuré sur `abarth500`** : `SkinBase_DEFAULT.dds` a **82,5 % de ses pixels à
alpha nul**, et le RVB moyen sous ces pixels vaut **[163, 159, 159]** — la
peinture de la carrosserie. Le canal alpha y transporte autre chose (masque de
spécularité selon les matériaux), pas une découpe.

**Conséquence si on le conserve** : le navigateur prémultiplie le RVB par
l'alpha à l'envoi vers le GPU. La carrosserie est effacée par son propre canal
alpha, et la voiture paraît transparente — alors que `alphaMode` vaut pourtant
`OPAQUE`.

**Correctif** : l'alpha n'est conservé que si un matériau l'exploite réellement
(mode `MASK` ou `BLEND`), ou si la texture sert aussi de carte de données. Sinon
il est forcé à l'opacité, ce qui fait au passage repasser la texture en JPEG —
`abarth500` passe de 12,0 à 9,8 Mo.

**Retombée sur la détection du verre** : le même signal sert à distinguer une
vitre d'une décalcomanie, **sans dépendre du nom** — le verre de l'Abarth
s'appelle `CAR_Vetro`, `INT_Vetro`, `INT_Vetro_Laterale`. Un matériau en fondu
dont la texture porte un alpha exploitable tire sa découpe de cet alpha
(décalcomanie, flou de jante, couture) ; un matériau en fondu sans alpha
exploitable, ou sans texture, est du verre et reçoit une opacité approximée
depuis `ksDiffuse`.

---

## Écart n°4 — sur un shader de dégâts, `txNormal` est la carte de dégâts

**Symptôme** : carrosserie qui paraît cabossée en permanence, sur une voiture
pourtant intacte.

**Réel** : les shaders de la famille `*_damage*` réservent `txNormal` à la
déformation des tôles, qu'AC ne mélange qu'à proportion des dégâts subis —
nulle sur une voiture neuve. Appliquée à pleine intensité comme une normal map
ordinaire, elle froisse définitivement la carrosserie.

**Vérifié sur quatre voitures, sans exception** :

| Voiture | Matériau | `txNormal` |
| --- | --- | --- |
| `ks_toyota_supra_mkiv` | `supra_body` | `exterior_damage_NM.dds` |
| `ks_mazda_mx5_cup` | `EXT_Carpaint` | `Damage_NM.dds` |
| `abarth500` | `CAR_Livrea` | `NORMAL MAP DAMAGE_TEMP.dds` |
| `ks_ford_gt40` | `Car_body` | `damageNM.dds` |

La corrélation porte sur le **shader**, pas sur le nom de la texture : c'est le
critère retenu, il ne dépend d'aucune convention de nommage.

**Correctif** : aucune normal map exportée quand le nom du shader contient
`damage`. Verrouillé par `damage_shaders_do_not_export_their_normal_map`.

---

## Écart n°5 — la couleur de peinture est souvent dans la carte de détail

**Symptôme** : `ks_toyota_supra_mkiv` / `01_dark_green_pearl_met` ressortait
**blanche**, alors que `abarth500` / `black_red` était juste. Puis, une fois la
teinte posée : `ks_abarth500_assetto_corse` / `dark_blue` ressortait **blanche**
là où le jeu la rend bleu nuit.

**Réel** : la texture diffuse d'une carrosserie AC est un fond **gris neutre**
(la Supra : 200 à 230 sur les panneaux) qui porte la découpe des panneaux et
les décalcomanies — mais **aucune couleur**. La peinture vient de la petite
carte `txDetail` que le dossier du skin remplace, le plus souvent un aplat de
quelques dizaines de pixels. Le shader les combine à la manière du
`MODULATE2X` de Direct3D, où **un gris moyen est neutre** :

```
diffuse.rgb *= lerp(1, detail.rgb * 2, useDetail * (1 - diffuse.a))
```

Deux choses se lisent mal dans cette ligne et expliquent chacune un bug :

1. le facteur **×2** — les aplats « officiels » de Kunos valent 148 à 156 sur
   255, soit juste au-dessus du 128 que cette convention rend neutre ;
2. le terme `1 - diffuse.a` : **l'alpha de la diffuse est un masque de
   peinture**, pas une transparence. Alpha nul = carrosserie, à peindre ;
   alpha plein = décalcomanie, à laisser. C'est ce qui garde un numéro de
   course blanc sur une voiture bleu nuit — mesuré sur la livrée de
   `dark_blue` : carrosserie α=3, planche du 495 α=255, bandes bleues α=205.
   C'est aussi ce que voyait l'écart n°1 (« 82,5 % des pixels à alpha nul sur
   une carrosserie blanche ») sans le nommer.

**Vérifié sur toutes les combinaisons essayées**, sans exception :

| Voiture / skin | aplat `txDetail` | voiture rendue |
| --- | --- | --- |
| `ks_abarth500_assetto_corse` / `dark_blue` | (0, 16, 38) | bleu nuit |
| … / `red_yellow` | (239, 0, 0) | rouge |
| … / `white_grey` | (238, 238, 238) | blanche |
| … / `black_neon` | (8, 8, 8) | noire |
| `ks_toyota_supra_mkiv` / `01_dark_green_pearl_met` | (5, 105, 36) | vert foncé |
| … / `05_blue_pearl_met` | (35, 57, 161) | bleue |

**Correctif** : la peinture est cuite dans une **variante de la texture
diffuse** (`crates/kn5-gltf/src/paint.rs`), pas dans `baseColorFactor`. Deux
raisons, toutes deux rédhibitoires pour un facteur global : le masque est par
pixel, donc un facteur peindrait les décalcomanies avec la carrosserie ; et
glTF borne `baseColorFactor` à 1, donc il ne saurait pas porter la moitié
*éclaircissante* d'un `MODULATE2X` (un aplat blanc demande ×1,87). Les
variantes sont nommées d'après leur source et leur couleur, donc deux matériaux
qui demandent la même peinture partagent une image ; une variante qui
n'assombrit ni n'éclaircit rien (diffuse entièrement opaque) n'est pas écrite.

⚠️ **Correction d'une conclusion antérieure.** Cette section affirmait que la
carte de détail de la Supra verte était un « vert d'eau » très peu saturé, ce
qui avait justifié un facteur d'amplification calibré à l'œil
(`DETAIL_TINT_BOOST = 3.0`, supprimé). C'était une mesure fausse : la carte
vaut (5, 105, 36), un vert franc et sombre. Le rendu trop pâle ne venait pas
d'une carte fade mais de la normalisation à luminance constante, qui
interdisait par construction à une voiture d'être foncée — et faisait tomber
les peintures les plus sombres sous le garde-fou `luminance <= 0.02`, d'où une
carrosserie restée blanche. Méthode de vérification : décodage direct du DDS
hors pipeline (moyenne par canal) et comparaison au `preview.jpg` du skin.

---

## Écart n°6 — sur `ksWindscreen`, `txDiffuse` est une carte de saletés

**Symptôme** : vitrage constellé de taches, comme un pare-brise abîmé.

**Réel** : même mécanisme que l'écart n°4, sur un autre slot. `ksWindscreen`
n'utilise pas sa `txDiffuse` comme une couleur : c'est une carte de **rayures
et de poussière**, qu'AC ne mélange qu'à proportion de la saleté du pare-brise
— nulle sur une voiture propre.

**Vérifié sur trois voitures** : `ks_mazda_mx5_cup` et `abarth500`
(`INTERNAL_glass.dds`, un fond gris rayé et piqueté), `ks_toyota_supra_mkiv`
(`Interior_windscreen_diff.dds`).

**Correctif** : aucune texture de couleur exportée pour un matériau
`ksWindscreen`. Le matériau retombe alors sur l'opacité tirée de `ksDiffuse`,
qui est le bon comportement pour du verre. Verrouillé par
`windscreen_does_not_use_its_dirt_map_as_colour`.

> **Motif à retenir pour la suite.** Quatre défauts visuels sur quatre avaient
> la même cause de fond : **AC range dans un slot standard une carte que son
> shader ne mélange qu'à proportion de quelque chose** — un état (dégâts,
> saleté) ou un masque (la peinture, sous l'alpha de la diffuse). Prise au
> premier degré, elle s'applique à 100 %, ou pas du tout. Devant un nouveau
> défaut « la voiture a l'air abîmée / sale / de la mauvaise couleur », c'est
> la première chose à vérifier : quel shader, et que met-il vraiment dans ce
> slot — **y compris dans le canal alpha**.

---

## Écart n°7 — dans `txMaps`, seul le vert est lisible (et c'est la brillance)

**Question ouverte depuis le début du chantier** (SPEC-preview-3d §6.2, §12 q3),
laissée de côté parce que « une spécularité fausse est pire qu'une surface
seulement diffuse ». Mesurée, elle l'est maintenant à moitié.

**Méthode** : décodage direct des textures liées à `txMaps`, moyenne et
écart-type par canal, croisés avec le nom du matériau qui les utilise et ses
`ksSpecular*`. Quatre voitures, une trentaine de textures.

**Le canal vert est la brillance**, sans exception rencontrée :

| Texture | surface | G |
| --- | --- | --- |
| `EXT_Chrome_MAP` (`ks_mazda_mx5_cup`) | chrome | 255 |
| `exterior_body_map` (`ks_toyota_supra_mkiv`) | peinture | 255 |
| `500_Abarth_Racing_skin_MAP` | peinture de course | 255 |
| `EXT_Rims_MAP` | jantes | 223 |
| `exterior_metal_map` | métal nu | 148 |
| `exterior_plastic_map` | plastique mat | 64 |
| `EXT_Loghi_MAP` | décalcomanies | 54 |
| `INT_LR_map` | cuir, planche de bord | 25 |

Ce qui compte autant que la découverte : **`ksSpecularEXP` ne sépare pas ces
surfaces**. Le chrome et le cuir de la MX-5 sont tous deux à `EXP = 100`, avec
G à 255 contre 25. La rugosité devinée depuis l'exposant seul rangeait donc une
carrosserie à 0,55 — un satiné, pas une peinture.

**R et B restent indéterminés.** Les auteurs écrivent très souvent la même
valeur dans les deux (`R == B` exactement sur la plupart des cartes mesurées,
et sur la Supra cette valeur suit l'AO de la diffuse), mais pas toujours : la
carrosserie de l'Abarth porte R=24 contre B=239. Ce sont donc **deux
grandeurs**, et rien de mesuré ne dit lesquelles. Elles restent inutilisées, et
`metallicFactor` reste à zéro — §6.2 demande un résultat plausible plutôt qu'un
résultat deviné.

**Correctif** : une texture métallique-rugosité dérivée par carte de surface —
glTF lit la rugosité dans le vert, donc une inversion suffit
(`crates/kn5-gltf/src/roughness.rs`). Par pixel et non en scalaire : un même
atlas sert couramment plusieurs finitions (écart-type de 70 sur le G de
`INT_Cockpit_OCC_Map`), qu'une moyenne effacerait.

Deux garde-fous, tous deux nés d'un cas réel :

- **`txMaps` qui pointe sur une texture de couleur** — `Grey.dds` sur le
  radiateur de la Supra, son propre atlas d'habitacle sur l'Abarth. Le vert
  d'une texture de couleur est un vert, pas une brillance. Écarté sur le rôle
  déjà attribué à la texture par le pipeline.
- **`NULL.dds`**, quatre pixels blancs, qui veut dire « rien à dire sur cette
  surface » : les sièges en tissu de la Supra pointent dessus et devenaient des
  miroirs. Détecté sur **les trois canaux saturés**, pas seulement le vert —
  une carte réellement brillante porte des valeurs ailleurs (R=24, B=239 sur
  l'Abarth). Le matériau garde alors son repli sur `ksSpecularEXP`.

---

## Découverte annexe — remorque chiffrée après l'arbre de nœuds

Deux voitures de la bibliothèque de référence (`ms_citroen_berlingo_2003_hdi` et
`_vts`) se parsent intégralement mais laissent 14 Mo et 23 Mo d'octets
**après** la fin de l'arbre de nœuds — d'où l'avertissement « trailing bytes »
émis par le parser.

**Ce que c'est** : une table `nom / taille / blob` appendue au fichier, avec
**l'offset de début de table écrit dans les 4 derniers octets du fichier**
(footer classique). 681 entrées, nommées d'après les meshes et les textures du
modèle, avec trois suffixes systématiques par mesh (`.i` 16 octets, `.k`
4 octets, `.x` taille variable) plus un par texture (`.d`) :

```
ver.Towbar.i        16 octets
ver.Towbar.k         4 octets
ver.Towbar.x      1104 octets
ver.towhook_bar.i   16 octets
…
```

16 octets = IV, 4 octets = identifiant/clé, blob = charge utile : la signature
d'un chiffrement par bloc. C'est très probablement la protection CSP des mods
payants (§4.5).

**Ce qu'on en fait : rien.** La partie KN5 en clair se lit normalement et
produit un modèle complet et cohérent (273 234 triangles, 72 textures,
identifiants de matériaux tous valides). La remorque est ignorée, conformément
à §4.5 — on ne déchiffre pas. L'avertissement du parser est conservé : il reste
le bon signal si un vrai décalage de section apparaissait un jour.

⚠️ **Non vérifié** : que le modèle en clair soit bien celui que le jeu affiche.
Il pourrait n'être qu'une version dégradée, le vrai modèle vivant dans la partie
chiffrée. Seul le rendu (lot 3+) le dira.

---

## Corpus de référence

201 dossiers dans `content/cars` d'une install réelle :

- **198 parsés sans erreur** (v6 ×197, v5 ×1 — le seul fichier v5 valide la
  branche `version > 4` de la section Matériaux).
- **3 sans modèle**, tous légitimement : `2K Skins` et `No Dust Skins` (dossiers
  de skins mal rangés, aucun `.kn5`), et `some1_acura_nsx_zanardi_1999` qui
  n'a qu'un `collider.kn5` — mod incomplet.
- **0 échec de parsing.**
- Résolution du modèle : 197 par heuristique (§4.2 étape 2), 1 par
  `data/lods.ini`. L'heuristique porte donc la quasi-totalité du travail —
  `lods.ini` est presque toujours enfermé dans `data.acd`.
- Conteneurs de texture : 13 682 DDS, 614 PNG, **aucun JPEG**. Le sniff de
  magie (§3.2) n'est pas une précaution théorique : **134 textures sur 14 296**
  (≈ 1 %) ont un conteneur réel qui contredit leur extension — un `.dds` qui
  est un PNG, ou l'inverse. Une sur cent, c'est assez pour qu'au moins une
  voiture de n'importe quelle bibliothèque soit concernée.

Ces fichiers ne sont **jamais** committés (assets Kunos et mods tiers). Pour
rejouer la mesure :

```bash
cargo run --release -p kn5-tool -- scan "D:/SteamLibrary/steamapps/common/assettocorsa/content/cars" --details
```
