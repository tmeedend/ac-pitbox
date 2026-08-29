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
`ksWindscreen`. Verrouillé par
`windscreen_does_not_use_its_dirt_map_as_colour`.

**Suite : sur ce shader, `ksDiffuse` n'est pas non plus une opacité.** Le
matériau retombait alors sur l'opacité tirée de `ksDiffuse`, comme tout le
vitrage — et c'était encore faux. Symptôme remonté par l'utilisateur : un
**voile blanc sur tout l'habitacle** de `ks_toyota_supra_mkiv`, qui lavait les
sièges et la planche de bord.

La valeur trahit sa nature : `ksDiffuse = 0,45` sur `ks_toyota_supra_mkiv`,
`ks_mazda_mx5_cup`, `ks_ford_gt40` et `abarth500`, 0,75 sur
`ks_ferrari_488_gt3`. C'est une constante de la famille de shaders, pas un
réglage de vitre — prise pour une opacité, elle pose une vitre blanche opaque
à 45 % devant l'intérieur. Les noms des matériaux disent la même chose :
`INT_Glass_REFLEX`, `INT_Vetro`, `Windshield`. C'est la couche de **reflet**
du pare-brise ; la vitre elle-même est claire, et ce qui doit s'y voir est
l'environnement réfléchi. Opacité fixée à 0,1, verrouillée par
`a_windscreen_stays_clear_whatever_its_ksdiffuse_says`.

**Et son `ksSpecularEXP` n'est pas un exposant utilisable non plus.** Une fois
le voile blanc retiré, la vitre restait « sale » : `ksWindscreen` annonce
`ksSpecular = 0` et `ksSpecularEXP = 10`, dont la formule générale de rugosité
(§6.1) tire **0,8** — du verre dépoli. Comparaison qui tranche, sur la même
voiture : la vitre extérieure (`ksPerPixelReflection`) annonce, elle,
`ksSpecularEXP = 500`, soit une surface lisse. Une vitre est lisse quoi
qu'annonce son matériau ; rugosité plafonnée à 0,08 sur toute la famille
`GLASS_MARKERS`, verrouillée par `glass_is_smooth_whatever_its_exponent_says`.

> Trois lectures fausses sur le **même** matériau — la texture, l'opacité, la
> rugosité — et toujours la même racine : `ksWindscreen` renseigne ses champs
> standard avec des valeurs que son shader n'utilise pas comme les autres. Sur
> un shader qui porte un nom de rôle plutôt qu'un nom de technique, se méfier
> de **tous** ses champs, pas seulement de celui qui vient de trahir.

---

## Écart n°8 — la vitre brisée est toujours dans le modèle

**Symptôme** : après les trois correctifs de l'écart n°6, le pare-brise de
`ks_toyota_supra_mkiv` restait « sale » — un voile gris marbré par-dessus
l'habitacle. C'était le quatrième signalement du même défaut apparent, et la
cause n'avait rien à voir avec les précédentes.

**Réel** : un maillage distinct, de shader **`ksBrokenGlass`**, double le
vitrage en permanence — cinq maillages sur la Supra. Sa `txDiffuse` est un
aplat gris de 16×16 et sa `txNormal` porte le réseau de **fissures**. AC ne
l'affiche qu'une fois la vitre effectivement brisée ; dessiné tel quel, à 40 %
d'opacité, il pose un voile gris et une toile de craquelures sur le pare-brise.

**Même mécanisme que l'écart n°4** (la carte de dégâts), mais **sur un maillage
entier au lieu d'un slot de texture** — d'où quatre passages à côté : on
cherchait une texture fautive sur le matériau du pare-brise, alors que le
coupable était un autre objet posé devant.

**Correctif** : les maillages dont le matériau est `ksBrokenGlass` ne sont pas
convertis du tout (`geometry::classify`, `Skip::BrokenGlass`, compté dans les
statistiques de `kn5-tool convert`). Verrouillé par
`broken_glass_is_never_drawn`.

> **Le motif s'étend donc d'un cran.** AC ne range pas seulement un état dans un
> slot de texture : il le range aussi dans un **maillage**. Devant un défaut
> visuel qui résiste aux textures du matériau soupçonné, regarder ce qui est
> dessiné **par-dessus**.

**Conséquence immédiate, et il faut la connaître** : une fois la vitre brisée
retirée, il n'y avait plus de vitrage du tout à l'écran — le voile gris qu'on
prenait pour une vitre *était* la vitre brisée. Voir l'écart n°9.

---

## Écart n°9 — un alpha constant est une opacité, pas une découpe

**Symptôme** : plus aucune vitre visible après le correctif de l'écart n°8.

**Réel** : le vitrage d'AC est presque parfaitement transparent, et ce qu'on en
voit vient de la **réflexion** — `fresnelMaxLevel = 0.7` sur le pare-brise de
la Supra. Le `glass.dds` qu'il porte est un aplat de 64×64 dont l'alpha vaut
**13/255 partout**, soit 5 % d'opacité.

Notre conversion traitait tout alpha présent dans une texture diffuse comme une
**découpe** (décalcomanie, flou de jante, couture) et laissait donc cet alpha
piloter la transparence : 5 %, invisible. Or un alpha **constant** ne découpe
rien — c'est une opacité uniforme, exactement la grandeur que le shader donne
par ailleurs. La distinction utile n'est donc pas « la texture a-t-elle un
alpha » mais « **cet alpha varie-t-il** ».

**Correctif** : `PreparedTexture::alpha_varies` (l'alpha diffère-t-il d'un pixel
à l'autre) remplace `has_alpha` dans la décision de transparence. Un alpha
constant renvoie le matériau sur l'opacité tirée du shader, plancher à 0,15 —
faute de pouvoir lui rendre son reflet, on lui rend une présence
(`GLASS_MIN_OPACITY`, à ajuster avec `WINDSCREEN_OPACITY` si le vitrage paraît
trop ou pas assez marqué).

> **Motif à retenir pour la suite.** Quatre défauts visuels sur quatre avaient
> la même cause de fond : **AC range dans un slot standard une carte que son
> shader ne mélange qu'à proportion de quelque chose** — un état (dégâts,
> saleté) ou un masque (la peinture, sous l'alpha de la diffuse). Prise au
> premier degré, elle s'applique à 100 %, ou pas du tout. Devant un nouveau
> défaut « la voiture a l'air abîmée / sale / de la mauvaise couleur », c'est
> la première chose à vérifier : quel shader, et que met-il vraiment dans ce
> slot — **y compris dans le canal alpha**.

---

## Écart n°7 — dans `txMaps`, seul le vert porte un sens (et c'est la brillance)

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

**R et B ne portent rien d'exploitable — mesuré, et la question est close.**

Seconde campagne, sur **6 597 textures `txMaps`** de tout le parc (`kn5-tool
maps`, qui écrit une ligne de CSV par couple matériau/texture et dont c'est la
seule raison d'être). Trois mesures, convergentes :

| | Kunos (3 381) | mods (3 216) |
| --- | --- | --- |
| corrélation R↔B, médiane | **1,00** | 0,00 |
| R et B identiques sur **tous** les pixels | 49 % | 33 % |
| corrélation R↔V et B↔V, médiane | 0,06 | 0,00 |

Ce que ça dit, dans l'ordre :

1. **Chez Kunos, R et B sont la même donnée.** Une corrélation médiane de 1,00
   et la moitié des cartes identiques au pixel près : deux entrées distinctes
   d'un shader ne seraient pas des doublons sur la moitié du contenu officiel.
2. **Ni l'un ni l'autre ne suit la brillance.** Corrélation médiane 0,06 avec
   le vert, et la valeur balaie **−0,97 à +1,00** d'une voiture à l'autre.
   C'est la signature d'un canal que personne n'écrit délibérément, pas celle
   d'une grandeur.
3. **Les mods y écrivent tout autre chose** (corrélation R↔B médiane 0,00) sans
   que leurs voitures paraissent cassées en jeu. Le shader ne les lit donc pas
   davantage.

Conclusion : R et B restent inutilisés, et ce n'est plus une prudence en
attendant mieux — c'est une réponse. La question §12 q3 de la spec est
tranchée, par la négative.

**La métallicité qu'on en attendait vient d'ailleurs** : `fresnelC`, voir
l'écart n°10.

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

## Écart n°10 — la métallicité n'est pas dans une texture, elle est dans `fresnelC`

**Le problème qu'il fallait résoudre** : `metallicFactor` était tenu à zéro
depuis le début du chantier (§6.2), donc **aucun matériau n'était métallique** —
le chrome, les jantes, le métal nu et les optiques rendaient comme de la
peinture brillante. On l'attendait de `txMaps`, qui ne le donne pas (écart n°7).

**Méthode** : mêmes 6 597 lignes, en y ajoutant les propriétés scalaires des
matériaux. `fresnelC`, `fresnelEXP` et `fresnelMaxLevel` sont portées par
**82 %** des matériaux (13 695 sur 16 791) — c'est le seul groupe de propriétés
qui décrive la réflectivité.

**`fresnelC` est la réflectance à incidence normale**, c'est-à-dire F0 — la
grandeur même qu'encode le modèle métallique-rugosité de glTF. Médianes par
famille de surface, voitures Kunos seules :

| surface | `fresnelC` | `fresnelMaxLevel` | `fresnelEXP` |
| --- | --- | --- | --- |
| chrome | 0,20 (q3 0,50) | 0,60 | 1,45 |
| optiques | 0,15 | 0,40 | 1,90 |
| métal nu | 0,05 | 0,30 | 1,70 |
| peinture | 0,05 | 0,50 | 3,50 |
| jantes | 0,05 | 0,20 | 2,00 |
| carbone | 0,04 | 0,20 | 3,00 |
| plastique | 0,01 | 0,05 | 3,00 |
| cuir, tissu | 0,00 | 0,02 | 4,00 |

C'est l'ordre de la physique, et l'exposant le confirme : bas (1,4–2) là où le
reflet tient de face — un métal — haut (4) là où il n'apparaît qu'en rasant,
c'est-à-dire un diélectrique. L'approximation de Schlick utilise 5.

**Kunos ne dépasse jamais 0,5** (p99 = 0,40). Les moddeurs, si : 11 % de leurs
matériaux passent 0,5, certains écrivent 1,2 pour dire « le plus réfléchissant
possible », et l'un a laissé 100. La valeur est donc ramenée dans [0, 1] plutôt
qu'écartée — l'intention reste lisible.

**Un second champ sert de veto, et il a été trouvé par un bug.** Sur la seule
`fresnelC`, la 250 GTO ressortait avec un **tapis de sol**, des **coutures de
cuir** et des **étriers** métalliques : leur `fresnelC` vaut bien 0,20 à 0,40,
mais leur `fresnelMaxLevel` vaut **0,01 à 0,03** — « cette surface ne renvoie
rien ». Le chrome et les optiques sont à 0,40–1,00. Un plancher sur ce champ
supprime les trois faux positifs sans toucher au chrome. La peinture, elle,
passe le veto (0,50) mais reste diélectrique par sa `fresnelC` de 0,05 : c'est
exactement ce qu'est un vernis, très réfléchissant en rasant, transparent de
face.

**Correctif** : `kn5_gltf::material::metallic_of` — rampe de `fresnelC` entre
0,10 et 0,40, vetée sous `fresnelMaxLevel` 0,15, et jamais appliquée au vitrage
ni au caoutchouc. Vérifié sur cinq voitures : la 250 GTO ne garde que son
rétroviseur, la Mustang ses six chromes, la MX-5 son chrome et ses étriers, la
Supra son rétroviseur et son métal nu.

**Piège associé, et il annulait tout en silence** : glTF lit la métallicité
dans le **bleu** de la texture métallique-rugosité et la **multiplie** par
`metallicFactor`. La carte dérivée de `txMaps` écrivait ce bleu à zéro — donc
le facteur ne serait jamais arrivé jusqu'aux surfaces qui en ont une, c'est-à-
dire précisément celles qui portent une carte. Le canal est désormais ouvert à
255 (`crates/kn5-gltf/src/roughness.rs`).

**Limite connue** : chez un moddeur qui écrit des valeurs de chrome sur un
volant, on obtient un volant chromé. Rien dans le fichier ne dit le contraire —
c'est un choix d'auteur qu'aucune mesure ne distingue d'un vrai miroir.

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

⚠️ **Levé, négativement** : voir « Découverte — un magic KN5 valide n'est pas une
garantie de géométrie exploitable » ci-dessous. Le modèle en clair de
`ms_citroen_berlingo_2003_vts` n'est **pas** le modèle affiché en jeu — la
moitié de ses triangles est incohérente, un rendu réel dessine un carré bleu
qui clignote à la place de la voiture. « Complet et cohérent » plus haut
décrivait ce que **le parseur structurel** voyait (compteurs de mesh,
identifiants de matériaux tous valides) — pas ce qu'un moteur de rendu en
ferait. Les deux questions sont différentes, et seule la seconde compte pour
l'utilisateur.

---

## Découverte — un magic KN5 valide n'est pas une garantie de géométrie exploitable

Deux mods signalés par un utilisateur comme cassés dans l'aperçu 3D :
`ms_citroen_berlingo_2003_vts` (« un gros carré bleu qui clignote ») et
`gmp_w204_c63_c13` (« plein de petits polygones bleus »). L'hypothèse de
départ — un magic `sc6969` altéré par la protection CSP (§4.5) — ne tient pas :
les deux fichiers parsent **sans la moindre erreur**, `kn5::Kn5Error::NotAKn5File`
ne se déclenche jamais. La détection existante (comparaison du magic) est donc
aveugle à cette famille de mods cassés.

**Méthode** : le vrai signal était déjà calculé, juste jamais exploité —
`kn5_gltf::winding_consistency` (accord entre le sens d'enroulement d'un
triangle et sa normale stockée, une mesure de cohérence interne indépendante
de toute convention de repère). Comparé sur un échantillon :

| Mod | Triangles orientés comme attendu |
| --- | --- |
| `ferrari_599_gto` | 99,9 % |
| `ford_mustang_boss_302` | 99,5 % |
| `rss_gtm_lanzo_v8` | 100,0 % |
| `some1_acura_nsx_1992` | 100,0 % |
| `oneweek_corvette_c1` | 99,9 % |
| `c13_porsche_959_87` | 99,9 % |
| `ag_subaru_impreza_wrx_tuned` | 100,0 % |
| **`ms_citroen_berlingo_2003_vts`** | **50,0 %** |
| **`ms_citroen_berlingo_2003_hdi`** | **50,1 %** |
| **`gmp_w204_c63_c13`** | **50,0 %** |

Aucun fichier de l'échantillon ne tombe entre les deux groupes : c'est une
rupture nette, pas un dégradé. 50 % est très exactement ce que produit un pile
ou face — la moitié des triangles n'a plus aucun rapport cohérent avec sa
normale, ce qui est la signature attendue d'une partie de la géométrie
remplacée par du bruit avant l'empaquetage, plutôt qu'une simple erreur
d'export. Sur `ms_citroen_berlingo_2003_vts` spécifiquement, une partie des
dummies (`bodyshell`, `door_dside_f`, `Headlight_housing`, une trentaine
d'autres) portent en plus une échelle locale de ~10237× au lieu de ~1× — sans
rapport visible avec le taux de winding (le second mod cassé, `gmp_w204_c63_c13`,
n'a aucune transformée suspecte de ce genre), donc probablement un symptôme
distinct de la même corruption plutôt que sa cause.

**Ce qu'on en fait** : `kn5_gltf::is_geometry_sane`/`WINDING_SANITY_THRESHOLD`
(0,9 — la marge entre 99,5 % et 50 % est large, aucune anomalie
de réglage à craindre) transforme cette mesure en verdict. Le pipeline
d'aperçu (`preview::prepare`) l'applique **avant** `convert()`, juste après le
contrôle du magic, avec le même repli : `errors.previewProtected`, jamais de
tentative de rendu sur une géométrie qui ne veut rien dire. `kn5-tool
convert`, lui, continue de convertir et de simplement avertir — c'est un outil
de diagnostic, pas le chemin utilisateur.

On ne sait toujours pas s'il s'agit de la protection CSP (§4.5) ou d'une autre
forme de corruption qui laisse le magic intact : les deux produisent le même
symptôme côté utilisateur, donc le même traitement. Le libellé affiché reste
générique (« modèle protégé ») plutôt que de nommer une cause qu'on ne peut
pas confirmer.

## Écart n°11 — l'alpha d'une texture **multiplie** l'opacité du matériau

**Symptôme** : vitrage quasi absent sur presque toutes les voitures. Signalé
par l'utilisateur comme « il manque beaucoup de vitre », après que l'écart n°9
ait cru régler la question.

**Réel** : en glTF, la couleur de base vaut `baseColorFactor × baseColorTexture`
— **canal alpha compris**. Le plancher d'opacité posé par l'écart n°9 ne
*remplaçait* donc pas l'alpha de la texture : il se multipliait à lui.

Mesuré sur `ks_toyota_ae86_tuned` :

| | valeur |
| --- | --- |
| alpha de `glass.dds` | 13/255 partout |
| `baseColorFactor.a` du matériau `glass` | 0,15 |
| opacité réellement rendue | **0,0076**, soit 0,76 % |

Même mécanisme sur `tint_windows` (53/255 × 0,15 = 3 %) et sur
`blacked_windows` (181/255 × 0,15 = 10,6 %). Le correctif de l'écart n°9 était
juste dans son intention et inopérant dans les faits — le plancher était bien
posé, il ne pouvait simplement rien contre un facteur qui ne s'appliquait pas
là où on croyait.

**Correctif** : quand **aucun** utilisateur d'une diffuse ne se sert de son
alpha comme d'une découpe, l'alpha est retiré de l'image avant encodage
(`texture::prepare_one`). L'opacité calculée pour le matériau reprend alors
seule la main. Volontairement conservateur : l'alpha reste dès qu'un matériau
alpha-testé l'utilise (grille, jante ajourée — c'est de lui qu'elles vivent),
ou qu'un matériau en fondu voit son alpha **varier dans sa propre empreinte**
(décalcomanie, autocollant). Verrouillé par
`an_alpha_nobody_cuts_with_is_dropped_so_the_shader_opacity_can_apply` et
`a_real_cutout_keeps_its_alpha`.

**L'empreinte, et pourquoi ce n'est pas l'image entière.** La question « cet
alpha découpe-t-il ? » ne se pose pas au niveau de la texture : un atlas de
carrosserie est partagé par la peinture, les décalcomanies et les vitres, et
son alpha y est un **masque de peinture** (écart n°5) qui varie forcément
quelque part. Elle se pose au niveau du **matériau**, sur la zone qu'il
échantillonne réellement. Deux approximations ont été essayées et jetées :

- marquer toute la texture comme masque de peinture dès qu'un matériau la
  peint — ça effaçait les décalcomanies et les vis de
  `vrc_erc_1999_renoir_csp`, qui partagent l'atlas et découpent vraiment ;
- prendre le **rectangle englobant** des UV du matériau — sur le pare-brise du
  même Renoir il couvre 27 % de l'atlas, où il attrape évidemment de l'opaque
  comme du transparent, et ne mesure donc rien.

Ce qui marche est l'échantillonnage par **points** : un par sommet, un par
centre de triangle. Les sommets seuls ne suffisent pas — ils ne décrivent que
le contour, et un quadrilatère dont les quatre coins tombent sur des texels
identiques passerait pour uniforme alors que son intérieur est découpé.
`kn5-tool inspect --materials` affiche la mesure (`empreinte alpha min-max sur
N points`), et c'est avec elle qu'on tranche au lieu de deviner.

> **Le motif de l'écart n°9 se retourne.** Il disait « un alpha constant est
> une opacité, pas une découpe ». Le pendant manquait : **un alpha qui varie
> n'est pas forcément une découpe non plus** — sur un atlas partagé, il varie
> parce qu'il sert à autre chose. Et surtout : décider qu'un alpha « n'est pas
> une transparence » ne suffit pas, encore faut-il **le retirer**, sinon il
> continue de s'appliquer dans le dos de la décision.

---

## Écart n°13 — le verre que déclare un mod est du verre **physique**

**Le point de départ** : après l'écart n°11, le vitrage était visible mais
restait pâle et sans caractère. Le réflexe aurait été de monter l'opacité au
jugé.

**La source** : `<AC>/extension/config/cars/common/materials_glass.ini`, livré
avec Custom Shaders Patch. Ce n'est pas de la documentation, c'est
l'**implémentation** du template `[Material_Glass]` que les moddeurs
invoquent — et elle dit exactement ce qu'est le verre d'AC sous CSP :

```ini
[TEMPLATE: Material_Glass EXTENDS _Base_Material_Custom]
IOR = 1.5            ; index of refraction for glass, usualy, 1.5
FilmIOR = $IOR       ; redefine IOR for external film layer to increase reflections
ThicknessMult = 1.0  ; thicker glass passes less light through
SHADER = smGlass
FresnelC = $" _PBR_EstimateF0( $FilmIOR or $IOR ) "   ; approximation de Schlick
```

Donc : un indice de réfraction, une épaisseur, un Fresnel dérivé de l'IOR par
Schlick. **Pas un canal alpha, pas un fondu.** `smGlass` n'utilise pas non plus
sa `txDiffuse` comme une couleur — même situation que `ksWindscreen` (écart
n°6).

**Pourquoi le fondu était structurellement faux** : sous `alphaMode: BLEND`,
glTF atténue *toute* la réponse du matériau, **reflet spéculaire compris**. Une
vitre rendue à 15 % d'opacité ne renvoie donc que 15 % de son reflet — or c'est
le reflet qui fait qu'une vitre ressemble à une vitre. On rendait une vitre de
plus en plus pâle en croyant la rendre de plus en plus transparente.

**Correctif** : un matériau que le mod déclare en `[Material_Glass]` (ou ses
variantes `GlassSide`, `MultiEmissiveGlass`, `PhotoelasticGlass`, ou le
raccourci `ExteriorGlassMaterials`) sort en `KHR_materials_transmission` +
`KHR_materials_ior`, `alphaMode: OPAQUE`, sans texture diffuse. glTF dérive le
F0 de `ior` par la même formule de Schlick que CSP applique — la conversion
n'approxime donc rien, elle transcrit.

**Deux pièges côté viewer**, tous deux parce qu'un matériau transmissif n'est
**pas** `transparent` au sens de three.js, si bien que toute règle branchée sur
ce drapeau le rate :

- il tombe dans la branche « opaque » et se met à projeter une ombre **noire
  pleine** — le « pâté sombre sous la voiture » que le code prend soin d'éviter ;
- le plancher de rugosité anti-scintillement (0,15) s'y applique, et three
  floute l'image transmise avec cette même rugosité : toutes les vitres
  ressortent **dépolies**.

> **Où chercher, la prochaine fois.** Avant de régler une valeur au jugé,
> vérifier si CSP livre le template correspondant dans
> `extension/config/cars/common/`. Le wiki est incomplet et se dit lui-même en
> chantier ; ces fichiers-là, eux, sont la vérité exécutée par le jeu.

---

## Écart n°12 — `ksAlphaRef = 0` veut dire « non réglé », pas « ne découpe rien »

**Symptôme** : tout l'arrière de `j8_mitsubishi_gto_twin_turbo_91` uniformément
orange.

**Réel** : huit matériaux de cette voiture écrivent `ksAlphaRef = 0`, recopié
tel quel en `alphaCutoff`. En glTF, un fragment passe dès que
`alpha >= alphaCutoff` : **à zéro, plus rien n'est découpé**, y compris les
pixels parfaitement transparents. La texture des lignes de dégivrage de la
lunette (`window_heater_lines.dds`) est à **87,5 % à alpha nul**, avec du RVB
`[165, 83, 0]` — de l'orange — sous ces pixels. Résultat : un panneau orange
plein en travers de la lunette arrière. Le jeu, lui, découpe correctement :
sa valeur par défaut n'est pas zéro.

**Correctif** : un zéro explicite prend le **même** défaut qu'une valeur
absente (`material::alpha_cutoff_of`, `DEFAULT_ALPHA_CUTOFF = 0.5`). Verrouillé
par `a_zero_alpha_reference_falls_back_to_the_default_cutoff`.

> Troisième fois que la même famille de pièges se referme : une valeur qu'AC
> écrit sans s'en servir. `ksWindscreen` renseigne trois champs qu'il n'utilise
> pas (écart n°6), `fresnelC` porte la métallicité que `txMaps` ne porte pas
> (écarts n°7 et n°10), et ici un `0` veut dire « je n'ai rien mis ». Devant un
> champ à zéro, se demander si c'est une valeur ou une absence.

---

## Découverte — les UV ne sont pas normalisés dans [0, 1]

`vrc_erc_1999_renoir_csp` range **tout son V dans [-1, 0]** : le pare-brise y
va de `v = -0.800` à `v = -0.200`, la carrosserie de `-0.997` à `-0.003`.

Le **rendu** s'en moque, et c'est ce qui rend la chose sournoise :
l'échantillonnage de texture boucle, donc `v = -0.8` lit le même texel que
`v = 0.2` et la voiture s'affiche correctement. Mais toute **mesure** faite sur
ces coordonnées au premier degré tombe entièrement hors de l'image. C'est ce
qui a fait échouer silencieusement la première version du test d'empreinte de
l'écart n°11 : elle concluait « UV répétés » sur *tous* les matériaux de la
voiture et retombait sur l'image entière, sans rien mesurer du tout.

**À retenir** : avant d'utiliser une coordonnée UV pour autre chose que du
rendu, la ramener dans sa période (`u - u.floor()`). Ne jamais supposer
`[0, 1]`.

---

## Découverte — un `WHEEL_*` peut ne contenir aucune jante

**Le symptôme** : sur `ks_toyota_ae86_tuned` équipé de son layer de
préparation, l'aperçu montrait un pneu noir, un disque clair et un étrier
rouge — et un trou à la place de la jante. Le réflexe est de chercher une
erreur de rendu ; il n'y en a pas.

**La mesure** (`kn5-tool inspect --tree`) :

```text
[dummy] WHEEL_RF
  [dummy] M_Tire_Max_Toyo_Proxes_R888R_Small.001
    [mesh] ... — 1980 tris, material Tyre_TUned86
[dummy] SUSP_RF
  [dummy] 20_T    → material caliper
  [dummy] DISC_RF → 209_T, material disk
```

Le nœud de roue **ne contient que le pneu**. Le disque et l'étrier pendent de
la suspension, pas de la roue. La jante n'est nulle part dans le fichier :
elle est livrée à part, un seul modèle de roue
(`skins/00_panda/watanabe.kn5`, 0,39 m de diamètre), que CSP instancie quatre
fois via `[ReplaceRims]`.

**Ce qu'il faut en retenir** : devant une pièce manquante sur un mod de
préparation, vérifier d'abord si elle existe dans le KN5 avant de soupçonner
la conversion. `extension/*.kn5` et `skins/*/*.kn5` sont les deux endroits où
regarder. La règle de traitement est au §4.5ter de `SPEC-preview-3d-kn5.md`.

## Découverte — les conventions des configs CSP qui se devinent mal

Deux détails de `ext_config.ini` qu'aucune documentation n'énonce, et qui
produisent chacun un défaut silencieux.

**`?` est un joker de longueur quelconque, y compris nulle** — pas un
caractère unique comme dans un glob classique. Preuve locale et décisive : la
config de l'AE86 filtre sur `SKINS = ?07_topaz?` alors que le dossier de skin
s'appelle exactement `07_topaz`, donc le motif doit pouvoir ne rien capturer
des deux côtés. Le wiki CSP ne montre que des exemples (`RIM_?`, `red?`) sans
jamais énoncer la règle. Lu comme un caractère unique, ce filtre ne
sélectionne aucun skin et toute la section est silencieusement ignorée.

**`ROTATION` est heading, pitch, roll — pas X, Y, Z.** Soit une rotation
autour de la verticale, puis de l'axe transversal, puis de l'axe
longitudinal. Le template `[ReplaceRims]` écrit `ROTATION = 180, 0, 0` sur les
roues de droite : c'est un retournement **gauche/droite** de la jante, qui
n'est modélisée qu'une fois. Lu comme un XYZ, le même `180, 0, 0` la met la
**tête en bas** — un défaut qu'un contrôle numérique ne voit pas (la matrice
reste une rotation propre, déterminant +1, donc aucun avertissement
d'enroulement) et qu'il faut regarder une roue de près pour attraper.

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
