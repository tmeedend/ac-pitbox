# Aperçu 3D — relevé des mesures et des pistes écartées

Ce document n'est pas une spec : **`SPEC-preview-3d-kn5.md` dit ce que fait
l'aperçu**, et c'est lui qui fait foi. Ici vit tout ce qui a servi à le
décider — les diagnostics, les bancs de mesure, les chiffres relevés sur la
bibliothèque de référence, et surtout **les pistes essayées puis retirées**.
À lire avant de retenter quelque chose dans ces directions : chacune a déjà
coûté au moins une soirée.

Le texte est repris tel qu'il a été écrit au fil des lots, dans la spec
d'origine ; les renvois `PREVIEW§` désignent la spec, les numéros de point
(« point 16 ») ceux du PREVIEW§15, qui garde la même numérotation. Le format
des fichiers lui-même est consigné dans `kn5-format.md`.

---

## Géométrie protégée, modèles étendus par CSP

### Magic valide, géométrie protégée

Certains mods parsent sans la moindre erreur et ne s'affichent pas — un carré
bleu clignotant, une pluie de petits polygones. Le SPEC ne savait pas pourquoi ;
c'est mesuré depuis (`docs/kn5-format.md`, découverte sur l'enroulement) : leurs
**sommets sont intacts** — normales et tangentes unitaires à 100 %, dimensions
d'une voiture, identifiants de matériaux valides — et seuls leurs *triangles*
relient n'importe quoi. Quatre autres explications ont été éliminées une à une
(décalage de lecture, bandes de triangles, géométrie doublée, variante de
format).

C'est la signature d'un **tampon d'index brouillé**, donc d'une protection : le
fichier paraît sain à tout outil externe et ne se reconstitue qu'avec la clé.

**Détection** : `kn5_gltf::winding_consistency` donne la fraction de triangles
dont l'enroulement correspond à sa normale stockée ;
`kn5_gltf::is_geometry_sane`/`WINDING_SANITY_THRESHOLD` (0,9) en fait un
verdict — les modèles sains sont entre 99,5 % et 100 %, les protégés à ~50 %.

**Comportement attendu** : `preview::prepare` applique ce contrôle juste après
celui du magic, avant `convert()`. Même repli que PREVIEW§4.5 :
`errors.previewProtected`, badge, infobulle, rien d'agressif. **Ne pas essayer
de rattraper ces modèles** — un rendu en double face a été tenté puis retiré :
si l'assemblage est brouillé, il montre une toile de triangles à la place d'une
photo propre. `kn5-tool convert` n'est pas concerné : il continue de convertir
et de seulement avertir, pour rester utilisable comme outil d'inspection.

**Portée mesurée** : 28 voitures sur 70, groupées par préfixe d'auteur
(`art_`, `bati_`, `bksy_`, `ddm_`, `aegis_`) — des familles entières de mods
protégés, là où le SPEC n'en connaissait que deux cas.

### Modèles étendus par CSP (`ext_config.ini`)

Beaucoup de mods de préparation livrent un KN5 **volontairement incomplet** et
laissent Custom Shaders Patch y greffer, skin par skin, les pièces qui
changent. Mesuré sur `ks_toyota_ae86_tuned` : les nœuds `WHEEL_*` ne
contiennent **que le pneu** — aucune jante — et la barre d'optiques avant vit
dans `extension/TOYOTA_HALOGEN.kn5`. Rendre le fichier principal seul montre
donc une voiture trouée, ce qui n'est pas un défaut du parseur : la géométrie
est ailleurs. Le showroom léger de Content Manager a exactement le même
symptôme, pour exactement la même raison ; seul le jeu, avec CSP chargé,
affiche la voiture entière.

**Ce qui est implémenté** (`crates/kn5-gltf/src/extconfig.rs`) :

- les sections `[MODEL_REPLACEMENT_*]` **littérales** de
  `<voiture>/extension/ext_config.ini` **et** de
  `<voiture>/skins/<skin>/ext_config.ini` — filtres `ACTIVE` / `FILE` /
  `SKINS`, puis `HIDE`, `INSERT` avec `INSERT_IN` (l'insertion devient enfant
  du nœud, donc suit ses mouvements) ou `INSERT_AFTER` (frère suivant),
  `OFFSET` / `ROTATION` / `SCALE` / `MULTIPLE` ;
- le template `[ReplaceRims]`, transcrit à la main depuis
  `<AC>/extension/config/cars/common/custom_rims.ini` ;
- les sections `[Material_*]`, qui disent de quelle matière est une surface
  (PREVIEW§6.3), et les `[REFRACTING_HEADLIGHT_…]`, qui disent qu'un maillage est une
  optique — sans elles, une vitre de phare dont la diffuse est un gabarit gris
  sort en aplat noir (`docs/kn5-format.md`, écart n°25). Le `SURFACE` d'une
  optique nomme parfois un morceau que `[MESH_SPLIT_…]` fabrique en découpant
  un maillage : la déclaration retombe alors sur le maillage entier, faute de
  faire la découpe (écart n°26).

**Quels fichiers sont lus**, du plus spécifique au plus général : le
`ext_config.ini` du skin, celui de la voiture, son `materials.ini`, puis la
config que CSP livre pour la voiture (`<AC>/extension/config/cars/loaded/`).
S'y ajoutent les fichiers **frères** que ceux-là incluent par `[INCLUDE: …]`,
suivis récursivement et rangés juste derrière celui qui les inclut — un mod
découpe couramment sa config en `pbr.ini`, `lights.ini`, `refraction.ini`. Un
`[INCLUDE: common/…]` n'est pas suivi : il désigne un template de CSP, résolu
contre son propre dossier, et c'est précisément ce qu'on renonce à interpréter.

**Ce qui ne l'est pas, et pourquoi.** `ext_config.ini` n'est pas un INI : c'est
un langage à templates, avec expressions (`$" ... "`), générateurs `@GENERATOR`,
includes, et un `read()` qui va chercher dans `data.acd` chiffré. Un moteur
complet est hors périmètre. Mais tout template se réduit à la primitive
`MODEL_REPLACEMENT`, qui est petite — d'où le choix de traiter la primitive et
de coder à la main le seul template qui coûte ses roues à une voiture. Les
remplacements de **shader** (`[SHADER_REPLACEMENT_*]`) restent ignorés : ils
changent l'aspect d'une surface, pas son existence.

**Deux limites assumées**, toutes deux dans le sens « on en montre plutôt trop
que pas assez » :

- `ACTIVE` est parfois une expression (`$" read('csp/version', 0) >= 2261 "`,
  sur `vrc_erc_1999_renoir_csp`) et non un drapeau. Seul un `0` littéral
  désactive une section ; ce qu'on ne sait pas évaluer est tenu pour actif, ce
  que ces gardes de version valent de toute façon sur un CSP récent.
- CSP lit le rayon et la largeur de jante visés dans `data/tyres.ini`, donc
  dans le `data.acd` chiffré qu'on ne déchiffre pas (PREVIEW§4.2). À défaut d'un
  `Radius`/`Width` explicite, la jante garde la taille que son propre modèle
  déclare — un facteur d'échelle de 1, qui est la bonne réponse dès lors que le
  moddeur a dimensionné la jante pour cette voiture. Sur l'AE86 : 0,195 m
  déclarés contre 0,1905 m pour une jante de 15 pouces, soit 2 % d'écart.

**Clé de cache.** Les `ext_config.ini` décident des morceaux greffés : ils
entrent donc dans la clé au même titre que le `.kn5` (`preview::cache_key`).
Sans ça, corriger une ligne de config laisserait l'ancien aperçu troué servi
indéfiniment.

**Portée mesurée** sur une install réelle de 299 voitures : 106 ont un
`extension/ext_config.ini`, mais seules **14** y ont des `MODEL_REPLACEMENT` et
**8** un `[ReplaceRims]` dans un skin — 19 voitures distinctes au total. Les
autres traversent la passe sans que rien ne s'applique.
## Le pilote : où il s'assoit, comment il braque

### Le pilote

Assetto Corsa range un pilote **en trois endroits**, et aucun n'est la
voiture :

| | Où | Qui le choisit |
| --- | --- | --- |
| **mannequin** (3D) | `<AC>/content/driver/<nom>.kn5` | la voiture, `driver3d.ini` `[MODEL] NAME` |
| **garde-robe** (textures) | `<AC>/content/texture/driver_{suit,gloves,helmet}/…` | le skin, `skin.ini` |
| **place assise** | `car.ini` `[GRAPHICS] DRIVEREYES` | la voiture |

La surcharge se fait par nom de fichier, exactement comme un skin surcharge une
voiture (PREVIEW§4.3) — d'où sa gratuité, et d'où sa limite : les `.dds` d'un dossier
de garde-robe portent le nom exact que les matériaux du mannequin réclament, et
un dossier dont aucun fichier ne correspond ne change **rien**.

**Seul le casque est lié au mannequin**, et il a fallu balayer le parc pour le
voir. Les cinq mannequins Kunos réclament tous `2016_Suit_DIFF.dds` et
`2016_Gloves_DIFF.dds` : les **53** dossiers de combinaison et les **69** de
gants marchent donc sur n'importe lequel. Le casque, lui, est daté —
`driver`/`driver_no_HANS` veulent `HELMET_2012`, `driver_80` `HELMET_1985`,
`driver_70` `HELMET_1975`, `driver_60` `HELMET_1969` — et les **176** dossiers
de casque se répartissent en 100 / 11 / 44 / 21 selon l'époque.

Ce qu'un futur *sélecteur* de pilote doit donc offrir : **trois listes
indépendantes**, dont seule celle des casques est filtrée par le mannequin de
la voiture. La compatibilité se décide par nom de fichier, elle ne se déduit
pas de ce que d'autres voitures déclarent. Les vignettes existent déjà à côté
des `.dds` (173 des 176 casques en ont une), donc la sélection peut être
visuelle sans rien produire.

Le `skin.ini` nomme sa garde-robe **sous le nom du mannequin**, ce qui est la
façon dont AC évite d'habiller le mauvais corps :

```ini
[driver_80]                    ; lu seulement si driver3d.ini demande driver_80
SUIT=\plain\red                ; → content/texture/driver_suit/plain/red/
GLOVES=\classicpastel\blue_lite
HELMET=\helmet_1985\blue
[CREW]                         ; le personnel de stand, hors sujet ici
```

**Où le pilote s'assoit.** Le mannequin est le même corps assis pour toutes les
voitures ; ses coordonnées propres ne l'assoient nulle part. La voiture le
place en une ligne, `DRIVEREYES` — une paire d'yeux dans le repère du modèle —
et le mannequin y répond par son os de tête `DRIVER:RIG_Head`, qui vaut
**exactement (0, 1.1994, 0.0305) sur neuf des dix mannequins** de l'install de
référence, tiers compris (`rss_driver_80`, `gt-m24`, `woman_driver`). Le
dixième (`new_driver.kn5`) suffit à ce qu'on lise l'os dans le fichier plutôt
que de coder la constante.

Les yeux ne sont pas l'os : ils sont **10 cm au-dessus et 8 cm devant**, et
cette valeur est calibrée, pas estimée. Sur 69 voitures tirées au hasard, la
garde entre le haut du casque et le point le plus haut de la voiture passe de
« 15 voitures traversées, jusqu'à −7 cm » à « aucune, au pire +3 cm, médiane
+15 cm ». Deux mesures indépendantes la recoupent : le maillage du visage est
6,5 cm au-dessus de l'os et la visière 10,7 cm ; et `driver3d.ini` **cache** le
casque, la visière et le visage en vue cockpit — ce qu'AC n'aurait aucune
raison de faire si `DRIVEREYES` n'était pas *dans* le casque. Le `POSITION` de
`driver3d.ini`, lui, **n'est pas appliqué** : il ressemble à un réglage fin et
n'en est pas un — treize voitures de l'install y écrivent de quoi déplacer le
pilote de 25 cm à cinq mètres, dont `1,1,1` sur quatre d'entre elles. Mesures
dans `kn5-format.md`.

**Ce qui est implémenté** (`crates/kn5-gltf/src/driver.rs` pour la greffe,
`src/driver/` pour la résolution) : le mannequin est lu, habillé, puis greffé
dans le modèle de la voiture **après** la passe CSP (PREVIEW§4.5ter), par la même
mécanique de fusion d'assets — donc avec le même arbitrage sur les collisions
de nom de texture. La voiture reçoit une racine neuve : les coordonnées du
mannequin sont dans l'espace *objet* de la voiture, pas sous la transformation
de sa racine.

**Ce qui ne l'est pas.** L'animation `steer.ksanim` qui pose les mains sur le
volant : le mannequin est donc figé dans sa pose de repos, bras tendus devant
lui. Les `HIDE_OBJECT_*` de `driver3d.ini` non plus — ils ne servent qu'à la
vue cockpit, où la caméra est dans la tête. Ni la substitution de mannequin par
`ext_config.ini`, qui existe mais n'a pas été rencontrée.

**Réglage et cache.** L'affichage du pilote est une option de l'écran Réglages
à **trois valeurs** : toujours, jamais, ou **au démarrage du moteur** — le
défaut. Dans ce dernier mode le pilote s'installe quand une clé de contact
tourne sur une fiche (§8.3, écoute d'un son moteur) et repart quand elle se
coupe, en fondu de 0,45 s.

Deux entrées de cache et non trois : « toujours » et « au démarrage » greffent
toutes deux le mannequin, seule la vue les distingue. C'est délibéré — laisser
la conversion suivre la clé demanderait quatorze mégaoctets de mannequin à
convertir au moment où on tourne la clé, ce qu'aucun fondu ne rattraperait. La
vue retrouve le pilote dans le `.glb` au **préfixe posé sur le nom de ses
maillages** (`PITBOX_DRIVER:`), seul repère qui traverse l'aplatissement de
l'arbre et le regroupement par matériau.

« Jamais » n'ajoute rien à la clé — les entrées écrites avant que le pilote
n'existe restent valides. Basculer entre les deux familles convertit une fois ;
les deux versions de la voiture coexistent ensuite et se rendent l'une l'autre
instantanément.

**`data.acd` est déchiffré** pour lire `driver3d.ini` et `car.ini` quand la
voiture est packagée, ce qui est le cas général — `acd::read_text`, le lecteur
qui servait déjà au régime moteur. Le PREVIEW§4.5ter dit encore que ce conteneur n'est
pas déchiffré : ce n'est plus vrai côté application, et la limite qu'il
documente sur la taille des jantes (`data/tyres.ini`) pourrait tomber pour la
même raison.

---

### Les mains sur le volant

Le mannequin est modelé bras tendus devant lui : c'est sa pose de liaison, et
elle ne conduit rien. Ce qui lui met les mains sur le volant est un fichier de
la voiture, `animations/steer.ksanim`, que **298 des 312 voitures** de
l'install de référence embarquent.

**Rien n'est calculé.** Le moddeur pose les bras pour son propre volant et
livre le résultat : pas de cinématique inverse, pas de rayon de jante mesuré,
donc rien à résoudre de notre côté — et, en contrepartie, des mains dans le
vide sur un mod dont le volant aurait été redimensionné après coup.

Le format est décrit dans `kn5::ksanim` : deux versions, l'une rangeant chaque
image en quaternion + translation + échelle (271 voitures), l'autre en matrice
4×4 (27). Une image est la transformation **locale** d'un nœud, exactement le
créneau qu'occupe la matrice d'un dummy dans le modèle — poser revient donc à
les échanger, et la hiérarchie fait le reste.

**Ce n'est pas l'animation qui assoit le pilote**, contrairement à ce qu'elle
laisse croire — 212 des 271 qui nomment son nœud racine lui laissent
l'identité. C'est
un quatrième fichier, `<voiture>/driver_base_pos.knh`, que **les 312 voitures**
de l'install livrent : le rig entier, placé dans la voiture. Format récursif et
minuscule, décrit dans `kn5::knh` et dans `docs/kn5-format.md`.

Le piège valait d'être documenté : appliquer la seule animation fait tomber la
tête à moins de 6 cm de `DRIVEREYES` sur 213 des 251 voitures dont elle nomme
un rig complet — assez pour croire qu'elle suffit. Les 38 autres s'écartent de
35 cm ou plus, sans rien entre les deux, et c'est ce qu'un utilisateur a vu :
un pilote mal assis sur une conduite à droite. En lisant le `.knh` comme socle
et l'animation par-dessus, **les 269 voitures mesurables tombent à moins de
6 cm**, sans seconde population.

**Le skinning devient obligatoire.** La combinaison et les gants sont des
maillages skinnés ; jusqu'ici ils s'affichaient juste par chance, la pose de
liaison rendant le skinning équivalent à l'identité. Dès qu'on pose le rig,
cette équivalence tombe : sans skinning, le casque et le visage — simples
enfants de l'os de tête — suivraient le rig pendant que le corps resterait en
arrière. Le skinning linéaire est donc implémenté pour de bon dans
`geometry.rs`, normales et tangentes comprises. **Vérifié sur les 311 voitures
de l'install** : la combinaison suit le rig partout, et les 297 animations se
posent sans exception (`every_installed_car_seats_its_driver`, test de corpus).

**Une seule règle, sans seuil : le fichier qui a placé le pilote fait foi.**
La hiérarchie d'abord ; à défaut l'animation, qui place bel et bien sur les
trois voitures livrant une `.knh` vide — leur tête y tombe à l'écart œil près
de `DRIVEREYES` ; et `DRIVEREYES` en dernier recours, quand rien n'a placé
personne. C'est là, et là seulement, que l'écart œil / os de tête du PREVIEW§4.6
s'applique encore.

**Le braquage est un réglage, et il tourne trois choses.** Il est exprimé en
degrés **aux roues avant**, de −35° à +35°, et non au volant : c'est ce qu'on
veut régler en regardant une voiture à l'arrêt, et c'est ce qui se voit. Gradué
au volant d'abord, il s'arrêtait à moins de la moitié de la butée — un
demi-tour de volant ne braque les roues que de 13° sur une démultiplication de
14 (signalé à l'écran).

**Et rien ne borne les roues par voiture**, ce qui est un second correctif. La
butée déclarée (`STEER_LOCK / STEER_RATIO`) est pourtant juste : mesurée sur les
voitures de route de l'installation, elle donne 33,6° à la MX-5, 32,1° à
l'AE86, 29,6° à l'Abarth 500 — la réalité. Mais AC met dans `STEER_LOCK` le
débattement **utile** d'un volant de simulation, pas la butée mécanique, et les
voitures de course y déclarent très peu : 22,9° à l'Evora GTC, 20,2° à la
488 GT3, **12,0° à la Huracán GT3**. S'en servir pour borner les roues laissait
une GT3 posée pour la photo quasi droite (signalé à l'écran). L'aperçu n'est pas
une simulation : le réglage donne l'angle des roues, point, et 35° est le haut
de ce que braque une vraie voiture. Le **volant**, lui, garde cette butée — la
course du volant est la seule chose qu'AC en dise sans ambiguïté.

L'animation de bras, elle, est indexée sur l'angle du **volant** : la
démultiplication fait le pont. Elle couvre toute la course, donc choisir une
image revient à choisir un angle, rapporté au `LOCK` de `driver3d.ini` — 360 sur
271 voitures, 180 sur quatorze. L'image du milieu est le volant droit.

Les bras seuls n'ont pas de sens : un pilote qui braque devant des roues droites
et un volant immobile ne braque rien. Le même angle tourne donc aussi :

- **le volant du poste de pilotage** (`STEER_HR` / `STEER_LR`), du même angle.
  L'axe de la colonne est **mesuré**, et par deux critères géométriques : le
  plan d'un volant contient forcément la direction 9 h – 3 h, donc l'axe local
  le plus latéral y est et n'est jamais la colonne ; des deux qui restent —
  perpendiculaires entre eux et tous deux dans le plan vertical longitudinal —
  la colonne est le **moins vertical**, un volant faisant face au pilote.
  Deux critères de forme ont été essayés avant et échouent tous deux : « le plus
  plat des trois » perd neuf voitures, parce qu'un `STEER_HR` porte aussi la
  colonne, les palettes et parfois un écran, qui s'étendent le long de l'axe
  qu'on cherche à trouver court ; « le plus court des deux restants » en met
  sept debout, parce qu'un volant de monoplace est large et bas. Résultat :
  103 volants décrits sur les 108 nommés, axe longitudinal à chaque fois
  (composante latérale 0,000 à la médiane, 0,070 au pire) et **incliné de 20°
  à la médiane**, ce qui est la nappe d'une vraie colonne. Les cinq indécis sont
  des monoplaces dont le nœud est plus profond que large ;
- **les coins avant**, autour de la verticale, de l'angle demandé tel quel —
  c'est le volant qui le multiplie par la démultiplication que la voiture
  déclare (`car.ini`, `[CONTROLS] STEER_RATIO` — de 10 à 24 sur la
  bibliothèque, 14 sur la moitié d'entre elles). Seul le volant s'arrête, à
  `STEER_LOCK / STEER_RATIO` ramené aux roues. Le pivot est le **milieu de la
  géométrie**, pas l'origine du nœud : une rotation ne dépend pas du point de
  l'axe qu'on choisit, mais certains mods accrochent le volant à un nœud posé à
  l'origine de la voiture, et un demi-tour autour d'un point à deux mètres
  l'envoie à travers l'habitacle.

  **Un coin avant est en TROIS nœuds frères, pas un.** C'était le défaut : ne
  tourner que `WHEEL_?F` laissait l'étrier de frein immobile, et il ressortait
  à travers le flanc du pneu dès quelques degrés (signalé à l'écran sur
  `bmw_m3_e30_dtm`). Mesuré sur six voitures — MX-5 Cup, 911 GT3 RS, Abarth
  500, R8 LMS, M3 E30, M3 E30 DTM — AC découpe toujours de la même façon :

  | Nœud | Contenu | Matériaux vus |
  | --- | --- | --- |
  | `WHEEL_?F` | pneu + jante | `EXT_Tyre`, `Rim_BASE` |
  | `DISC_?F` | disque de frein | `EXT_Disc`, `CAR_DiscoFreni` |
  | `SUSP_?F` | fusée + étrier | `EXT_Calipers`, `CAR_PinzaFreni`, `EXT_Caliper_base` |

  Les noms de matériaux changent d'un studio à l'autre, le découpage non. Les
  trois tournent donc ensemble — et **autour du centre de la roue**, jamais du
  leur : l'étrier n'est pas sur l'axe de braquage, il est à côté du disque, et
  le faire pivoter sur son propre milieu le fait tourner sur lui-même en
  restant où il est, c'est-à-dire traverser le pneu. Le pivot du coin se mesure
  donc **avant le parcours de l'arbre**, dans une passe à part : rien ne
  garantit l'ordre des frères — sur la M3 E30 DTM la roue précède l'étrier,
  sur la MX-5 Cup c'est l'inverse. Les trois partagent aussi leur **groupe**,
  ce qui laisse leurs maillages fusionner par matériau ; les deux coins, eux,
  restent deux groupes, parce que deux pivots distincts ne se fusionnent pas.

Rien de tout cela n'est dans le modèle : le `steer.ksanim` d'une voiture ne
contient que le rig du pilote — mesuré, pas un seul nœud en dehors — et AC
tourne les roues depuis la physique.

**L'angle n'est pas cuit dans le `.glb`, et c'est un correctif.** Il l'a été, et
la mesure a dit pourquoi il ne fallait pas : l'angle entrant dans la clé de
cache, chaque valeur essayée laissait une entrée complète — **onze entrées de
42 Mo pour une seule voiture**, relevées sur le poste, sur un plafond de 2 Gio.
Un curseur se balaye ; un réglage qui coûte une conversion par valeur ne peut
pas être un curseur.

La conversion **décrit** donc ce qui tourne au lieu de le tourner : chaque nœud
braqué sort en nœud glTF à part, ses sommets exprimés relativement à son pivot,
le pivot en translation et l'axe et le facteur en `extras` (`pitboxSteer`). La
vue n'écrit qu'une rotation dessus. Le braquage est alors **gratuit et
instantané**, et n'apparaît plus dans la clé.

Deux conséquences à connaître. La fusion par matériau doit désormais **séparer
les nœuds braqués** : deux roues tournent du même angle autour du même axe mais
pas autour du même pivot, donc elles ne peuvent pas partager un lot même à
matériau égal (sur beaucoup de voitures les quatre jantes partagent le leur).
Et ce que la voiture déclare de sa direction — `STEER_LOCK`, `STEER_RATIO` —
est écrit dans le `.glb`, donc *cela* fait partie de la clé.

**Et les bras du pilote suivent le même chemin.** Ils étaient écrits en sommets
figés, donc eux seuls obligeaient encore à reconvertir quand l'angle changeait.
Le mannequin est désormais exporté **vivant** (`kn5-gltf/src/rig.rs`) : ses os en
nœuds glTF, sa peau en `skin` — les `inverse_bind_matrix` et les quatre
poids/indices par sommet que le KN5 porte déjà sont exactement ce que glTF
réclame —, ses pièces rigides en enfants de l'os qui les porte, et l'animation de
braquage de la voiture en animation glTF. Poser les bras revient alors à choisir
un instant dans ce clip, ce que `AnimationMixer` fait pour rien.

Trois choses à ne pas perdre de vue :

- **les matrices ne sont pas transposées**, et ce n'est pas un oubli : le KN5
  travaille en ligne-vecteur rangé par lignes, glTF en colonne-vecteur rangé par
  colonnes, et le rangement par colonnes de `Mᵀ` est la même suite d'octets que
  celui par lignes de `M`. Les deux conventions se croisent et s'annulent ;
- **une pièce rigide accrochée à un os doit devenir son enfant.** Elle suivait
  jusqu'ici par la transformation accumulée, cuite dans ses sommets ; avec un
  squelette vivant elle resterait en arrière. Cas réel :
  `rh_schuberth_helmet_driver_19`, dont le casque est cinq maillages statiques
  sous `DRIVER:RIG_Head` ;
- **les bouts d'une animation de braquage ne sont pas des butées** — voir l'écart
  n°20 de `kn5-format.md`. La conversion mesure les trois images qui comptent et
  les écrit dans le `.glb`.

La pose de repos reste celle du volant droit : la greffe pose l'assise et l'image
centrale exactement comme avant, si bien que tout ce qui a été mesuré sur
l'assise tient sans changer. Un mannequin sans peau ni animation exploitable
retombe sur le chemin cuit.
## Transparence : découpe ou fondu

**Mais tout ce qu'AC déclare en fondu n'est pas une vitre, et le renoncement à la profondeur ne vaut que pour la vitre.** Une décalcomanie porte le même `blend_mode = 1` : posée dans la passe transparente sans écriture de profondeur, elle perd la géométrie comme arbitre, et c'est l'ordre des matériaux dans le fichier qui décide de qui passe devant qui. Ça ne se voit que quand deux calques se superposent — précisément la façon dont un mod pose une décalcomanie sur une plaque.

La mesure sépare les deux sans ambiguïté : **l'alpha d'une découpe ne prend que ses deux extrêmes** (0 ou 255, aux bords adoucis près), celui d'une vitre s'étale entre les deux. Mesuré sur `rss_gtm_lanzo_v10` : le numéro de portière 0,78 % de valeurs intermédiaires, l'atlas de décalcomanies 1,23 %, la vitre **100 %**. Un alpha qui découpe passe donc en `MASK` avec son seuil — même image, l'alpha ne valant que 0 ou 255, mais rendue dans la passe opaque, où la profondeur arbitre. Le crénelage du seuil est rattrapé par `alphaToCoverage`, déjà posé sur tout `alphaTest > 0`. Le verre en est exempté par son shader, comme pour l'approximation d'opacité.

Bug réel qui a fait écrire la règle : le numéro `20` de la portière de `rss_gtm_lanzo_v10`, posé 2,3 mm devant sa plaque, était recouvert par le calque de décalcomanies de toute la voiture — dessiné après lui. Il disparaissait **d'un bloc** d'un côté de la voiture et pas de l'autre, les deux portières n'échantillonnant pas la même région de l'atlas : seule l'une des deux y trouve des texels opaques. Mesuré sur banc, le numéro passait de 4 558 à 119 pixels selon l'angle. Portée sur la bibliothèque : **129 matériaux sur 55 voitures de 133** quittent la passe transparente — décalcomanies, autocollants, numéros, surpiqûres, grilles, rivets, chiffres de cadrans.
## L'état d'avancement au fil des lots

> Section ajoutée **après** la rédaction initiale. Le corps de ce document
> reste la spécification de départ, y compris là où l'implémentation l'a
> contredite : les écarts sont listés plus bas (« Écarts au plan d'origine »), avec leur raison. Quelqu'un qui
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
| 6 — finitions | ⏳ **en cours** | voir PREVIEW§15 |

**Validé à l'écran par l'utilisateur** : l'aperçu s'affiche dans la fiche
voiture, tourne sur son socle, se manipule à la souris, et les textures se
posent au bon endroit.

**Corpus** : 198 voitures sur 201 de la bibliothèque de référence se parsent
et se convertissent sans échec (les 3 restantes n'ont pas de modèle).

**Vitrage** : il était rendu à moins de 1 % d'opacité sur la quasi-totalité
des voitures — l'alpha de la texture se **multipliant** au plancher d'opacité
au lieu de lui céder la place (écart n°11 de `kn5-format.md`). Un alpha dont
aucun matériau ne se sert comme d'une découpe est maintenant retiré avant
encodage ; les découpes réelles (grilles, jantes ajourées, décalcomanies) sont
reconnues en mesurant l'alpha **dans l'empreinte UV du matériau**, point par
point. Au passage, `ksAlphaRef = 0` est traité comme « non réglé » (écart
n°12). Validé à l'écran par l'utilisateur : vitrage visible sur les voitures
Kunos, panneau orange disparu sur `j8_mitsubishi_gto_twin_turbo_91`.
**Reste ouvert** : `vrc_erc_1999_renoir_csp`, dont le pare-brise partage
l'atlas de carrosserie et dont l'empreinte alpha varie réellement (0–255) —
il garde donc sa découpe, et reste très peu marqué. CSP y remplace de toute
façon `MAIN_GLASS` par ses propres shaders, donc les valeurs brutes du KN5 n'y
sont pas ce que le jeu applique.

**Repère tangent** : le `TANGENT` de glTF est écrit sur tout maillage dont le
matériau porte une carte de normales. Il ne l'était pas, alors que le KN5 en
donne un par sommet — d'où des stries blanches sur les intérieurs, là où la
reconstruction par dérivées écran s'effondre sur des UV dégénérés (écart n°14
de `kn5-format.md`). `kn5-tool inspect --tangents` mesure les deux grandeurs
qui le décident.

**Verre physique** : un matériau qu'un mod déclare en `[Material_Glass]` est
converti en `KHR_materials_transmission` + `KHR_materials_ior` plutôt qu'en
fondu — c'est ce que fait CSP, dont le template est livré dans
`<AC>/extension/config/cars/common/materials_glass.ini` (écart n°13 de
`kn5-format.md`). 71 voitures sur 298 de la bibliothèque de référence sont
concernées. Les voitures Kunos ne le sont pas : leur config vit dans
`<AC>/extension/config/cars/loaded/`, que l'on ne lit pas encore.

**Modèles étendus par CSP** (PREVIEW§4.5ter) : les mods de préparation qui laissent
CSP greffer leurs pièces skin par skin s'affichaient troués — jantes absentes,
boucliers et optiques manquants. Les `[MODEL_REPLACEMENT_*]` littéraux et le
template `[ReplaceRims]` sont maintenant appliqués avant conversion. Validé à
l'écran par l'utilisateur sur `ks_toyota_ae86_tuned` + son layer de
préparation ; passé sans un seul échec de greffe sur les 19 voitures concernées
de la bibliothèque de référence.
## Écarts au plan d'origine

Chacun a été pris en connaissance de cause, avec sa raison. Ne pas les
« corriger » vers la spec sans relire la raison.

### Ajouts demandés par l'utilisateur en cours de route

- **Plateau tournant.** C'était le *but* du chantier, non écrit dans la
  demande initiale : la voiture tourne lentement sur elle-même, comme sur un
  socle de salon (un tour en ~28 s). C'est la voiture qui tourne, pas la
  caméra : le cadrage reste stable et l'état d'`OrbitControls` n'est jamais
  contrarié quand l'utilisateur prend la main.
  **Contredit le PREVIEW§8.4** (« rendre à la demande, pas en continu »), et les deux
  sont inconciliables. Contrepartie payée là où elle se voit : rotation
  suspendue hors écran, en arrière-plan, et pendant la manipulation (reprise
  après 4 s) ; désactivée si le système demande de réduire les animations ;
  avance calculée sur le temps écoulé, pour qu'un écran 144 Hz ne double pas
  la vitesse.
- **Manipulation à la souris** conservée par-dessus le plateau : orbite et
  zoom, sans panoramique, angle polaire borné pour ne pas passer sous le sol.
- **Coexistence avec le showroom natif** (`acShowroom.exe`, SESSION§4 du SPEC
  principal) au lieu du remplacement : les deux ne rendent pas le même
  service. Une barre d'outils vit dans la zone héros : bascule photo/3D
  mémorisée, remise en place de la voiture, et ouverture des réglages de
  cadrage. **Révélée au survol** (et au focus clavier) — l'aperçu est là pour
  être regardé, pas pour montrer ses commandes ; le panneau de réglages ouvert
  la maintient visible, sinon régler un curseur la ferait disparaître sous les
  doigts.
- **Photo de repli non floutée** pendant la préparation : c'est déjà l'aperçu
  habituel de la fiche, la flouter la rendait illisible au moment où elle sert.

### Écarts techniques

| Spec | Fait | Raison |
| --- | --- | --- |
| PREVIEW§4.4 négation d'un axe + inversion du winding | **Aucune conversion de repère** | Prémisse fausse. Voir `kn5-format.md` PREVIEW§12 q4 : deux mesures numériques le disaient dès le lot 3, une validation sur une voiture à l'atlas symétrique les a fait écarter à tort. |
| PREVIEW§4.4 `v = 1.0 - v` | **UV reprises telles quelles** | DirectX et glTF placent tous deux l'origine des textures en haut à gauche. L'inversion vaut pour OpenGL. |
| PREVIEW§5.2 `binrw` | Lecteur binaire écrit à la main | Chaque compteur doit être validé *avant* d'allouer ; l'exprimer en attributs coûtait plus que 130 lignes, pour une dépendance de plus. |
| PREVIEW§5.2 `gltf-json` | Document glTF écrit avec `serde_json` | Sous-ensemble petit et figé ; le test d'acceptation est empirique de toute façon. |
| PREVIEW§5.4 WebP q85 | **JPEG q85**, PNG si alpha utile | Le glTF de base n'accepte que PNG et JPEG ; WebP exige `EXT_texture_webp`, ce qui casse le critère « s'ouvre dans Blender ». `image` 0.25 n'encode plus le WebP. |
| PREVIEW§7.1 `PreviewHandle` | `CarPreview` | Nom déjà pris par `music::PreviewHandle`. |
| PREVIEW§5.1 `crates/` à la racine | `src-tauri/crates/` | Garde `target/`, `Cargo.lock` et les chemins CI où ils étaient. |
| PREVIEW§5.4 décodage BC1–BC7 | + **décodeur DDS par masques** | 12 % des textures AC sont des DDS non compressés qu'`image_dds` refuse. |
| — | **L'alpha des textures diffuses est retiré** quand aucun matériau ne l'exploite | Chez AC il ne code pas la transparence (82,5 % des pixels à alpha nul sur une carrosserie blanche). Le conserver efface la carrosserie. Il code en fait le **masque de peinture** — voir la ligne suivante. |
| PREVIEW§6.2 teinte au `baseColorFactor` | **Peinture cuite dans une variante de la texture diffuse** | Le masque est par pixel (l'alpha de la diffuse), donc un facteur global peindrait aussi les décalcomanies ; et glTF borne le facteur à 1, alors qu'un aplat blanc demande ×1,87. Voir `kn5-format.md`, écart n°5. |
| PREVIEW§6.2 `txMaps` laissé de côté | **Rugosité par pixel tirée de son canal vert, métallicité tirée de `fresnelC`** | La sémantique du vert est mesurée (brillance) ; `ksSpecularEXP` seul ne distinguait pas le chrome du cuir. R et B, eux, ne portent rien d'exploitable — mesuré, pas supposé. La métallicité que le PREVIEW§6.2 attendait d'une texture vient d'un scalaire du matériau. Voir `kn5-format.md`, écarts n°7 et n°10. |
## Le lot 6, point par point

Points remontés par l'utilisateur après validation à l'écran, par ordre de
gêne constatée :

1. ~~**La carrosserie paraît cabossée.**~~ ✅ **Corrigé.** C'était bien une
   carte de dégâts appliquée à tort : sur un shader `*_damage*`, `txNormal`
   est la déformation des tôles, qu'AC ne mélange qu'à proportion des dégâts.
   Vérifié sur quatre voitures — voir `kn5-format.md`, écart n°4.
   *Reste possible plus tard* : exporter le `TANGENT` du KN5 (three.js
   reconstruit aujourd'hui les tangentes par dérivées d'écran), et vérifier le
   canal vert des normal maps DirectX. Non nécessaire pour ce défaut-ci.
2. ~~**Couleur de peinture.**~~ ✅ **Corrigée.** La peinture vient de la carte
   de détail du skin (multipliée ×2, convention `MODULATE2X`) et elle est
   **masquée par l'alpha de la diffuse**, qui distingue la carrosserie des
   décalcomanies. Elle est donc cuite dans une variante de la texture, pas
   posée en `baseColorFactor` — voir `kn5-format.md`, écart n°5, réécrit : la
   mesure qui avait justifié le facteur calibré à l'œil (supprimé) était
   fausse. Vérifié sur six couples voiture/skin, dont
   `ks_abarth500_assetto_corse` / `dark_blue`, qui ressortait blanche.
3. ~~**Pare-brise.**~~ ✅ **Corrigé, en quatre passes** — c'est le point qui a
   le plus résisté, et la leçon vaut le détour : *le même défaut apparent*
   (« la vitre est sale ») avait **quatre causes différentes**, dévoilées une
   à une, chacune masquant la suivante.
   1. sa `txDiffuse` est une carte de rayures, pas une couleur (écart n°6) ;
   2. son `ksDiffuse` n'est pas une opacité mais une constante de famille de
      shaders — un voile blanc à 45 % sur tout l'habitacle ;
   3. son `ksSpecularEXP` ne donne pas une rugosité utilisable : 0,8, soit du
      verre dépoli ;
   4. et surtout, un **maillage entier** de vitre brisée (`ksBrokenGlass`) est
      posé par-dessus en permanence (écart n°8).
   Une fois ce dernier retiré il n'y avait plus de vitrage du tout, ce qui a
   révélé une cinquième chose : un alpha **constant** dans une texture n'est
   pas une découpe mais une opacité (écart n°9).
   **Ce qu'il faut en retenir** : quand un correctif ne change rien au défaut,
   se demander si on regarde le bon objet — et pas seulement le bon champ.
   Détail de la première passe : `ksWindscreen` réserve sa `txDiffuse` aux
   rayures et à la poussière. Voir `kn5-format.md`, écarts n°6, 8 et 9.
4. ~~**Réglages à exposer**~~ ✅ **Fait.** Écran Réglages, onglet **Aperçu**
   (`components/settings/PreviewTab.svelte`) : aperçu photo ou 3D, zoom,
   orientation, hauteur de vue, vitesse du plateau, plus un retour au cadrage
   d'origine. Persistance dans `ui_prefs.json` via
   `src/lib/preview3dPrefs.svelte.ts` (`$state` de module, partagé avec la
   bascule de la zone héros pour que les deux restent d'accord ; les curseurs
   vivent dans `Preview3dControls.svelte`, utilisé par les deux écrans — on les
   règle sur la fiche, où le résultat est sous les yeux, on les retrouve dans
   les Réglages avec leur mode d'emploi). Un changement
   se voit sur une fiche déjà ouverte : la caméra est reposée, le modèle n'est
   pas rechargé.
   La hauteur de vue est le réglage qui répond au cadrage : plus la caméra est
   haute, plus elle plonge, et plus l'avant sort du cadre quand le plateau
   tourne. **Décidé avec l'utilisateur** : on expose le réglage, on ne calcule
   pas une hauteur idéale par angle.
6. ~~**Rugosité par pixel depuis `txMaps`**~~ ✅ **Fait**, ajouté en cours de
   lot : son canal vert est la brillance, et `ksSpecularEXP` seul ne
   distinguait pas le chrome du cuir. Voir `kn5-format.md`, écart n°7.
7. **Éclairage et cadrage calés sur les `preview.jpg` Kunos** ✅ **pour
   l'essentiel** — reste un écart de luminosité décrit plus bas.
   **Fait après la rugosité** (décidé avec l'utilisateur) : elle change la
   réponse à la lumière, donc calibrer avant l'aurait fait recommencer.

   **Méthode** — un banc de comparaison plutôt que du tâtonnement : une page
   qui rend le `.glb` dans la même scène three.js à côté du `preview.jpg` du
   même skin, et qui mesure les deux (couverture, boîte de la voiture,
   luminance médiane, couleur médiane de la carrosserie). Jetable, hors dépôt.

   **Corrigé, mesuré sur deux voitures** :
   - **Focale** 35° → **20°**, avec la distance qui suit (4,9 rayons à
     zoom 100 %). À 35°, l'avant d'une voiture enfle et l'arrière fuit ; les
     photos du jeu n'ont pas cette déformation.
   - **Angle par défaut** : trois-quarts avant **gauche** (azimut 318°,
     hauteur 13°). Toutes les photos Kunos sont prises de ce côté ; on
     présentait le côté opposé, ce qui suffisait à faire sauter la bascule
     photo/3D.
   - **Cadrage** : la voiture couvre désormais ~17,6 % de l'image contre
     17,5 % chez Kunos, et n'est plus coupée en bas.
   - **Environnement** : la `RoomEnvironment` de three.js est une pièce
     **blanche**, et une peinture peu rugueuse y reflète des murs clairs sur
     toute sa surface. Remplacée par un studio sombre à rampes zénithales
     (`src/lib/preview3d/showroomEnvironment.ts`), procédural comme elle et
     sans asset (PREVIEW§8.1).
   - **Sol** : l'ombre de contact seule laissait la voiture posée sur rien.
     Le sol porte maintenant la **flaque de lumière** que renvoie un showroom
     (intensité partie de la photo — son fond passe de rgb(2,3,5) dans les
     coins à rgb(12,13,15) sous la voiture — puis remontée d'un cran, l'aperçu
     n'ayant pas le décor autour pour donner la profondeur), **et l'ombre
     portée de la voiture**, projetée pour de vrai.
     L'ombre vient d'une lumière directionnelle **d'intensité nulle** : elle
     n'éclaire rien, tout ce que la voiture reçoit continue de venir de la
     carte d'environnement calée plus haut. Elle n'existe que pour donner une
     direction de projection, `ShadowMaterial` lisant le masque d'ombre et non
     la contribution de la lumière. Carte d'ombre en **VSM** et non
     `PCFSoftShadowMap`, dont le filtre est de taille fixe : `shadow.radius`
     n'y fait rien, et une ombre nette sous une rampe large ne ressemble à
     rien (retour utilisateur). Le dégradé du sol ne garde qu'un
     assombrissement de contact, là où une carte d'ombre manque toujours de
     résolution.

     ⚠️ **Sol brillant essayé, mesuré, abandonné.** Retour utilisateur : « la
     flaque ne renvoie aucune lumière ». C'était exact — le sol est un
     `MeshBasicMaterial`, donc de la peinture plate : un matériau « basic » ne
     reçoit aucune lumière et n'échantillonne aucune carte d'environnement, par
     construction. Le remède évident était un `MeshStandardMaterial`
     diélectrique, sans passe supplémentaire. **Il ne produit rien**, et le
     banc le montre au lieu de le supposer (`kn5`/three hors application,
     lecture de pixels par `gl.readPixels`, pas de capture d'écran) :

     | sol, à 13° de plongée | valeur des pixels (sur 255) |
     | --- | --- |
     | miroir parfait, studio nu | **1 à 11** |
     | miroir parfait, un objet clair posé dessus | **109** |

     Autrement dit : **le miroir marche, c'est le studio qui n'a rien à
     refléter.** Ses deux seules sources vives sont les rampes zénithales, à la
     verticale de la voiture ; à 13° de plongée, un sol renvoie l'horizon, et
     l'horizon est ici une boîte noire (murs à 0,01, panneaux latéraux à 0,05).
     Un sol brillant dans une pièce noire est un sol noir.

     Conséquence : **seul le reflet de la voiture elle-même peut se voir**, ce
     qui est d'ailleurs ce que l'utilisateur demandait au départ. La matière
     seule est une fausse piste — pire, elle rend visible une large étendue
     grise là où il n'y avait que du noir (l'éclairage diffus de
     l'environnement, 5 à 33), ce que l'utilisateur a vu et signalé.

     **Le vrai miroir a donc été fait**, et il est en place : voir le point 12
     du PREVIEW§15 ci-dessous.

     **Découverte annexe, à ne pas réapprendre** : quand l'environnement vient
     de `scene.environment`, c'est **`scene.environmentIntensity`** qui le
     dose. `material.envMapIntensity` n'a **aucun effet** — vérifié à 10, image
     inchangée au pixel près, là où `scene.environmentIntensity = 8` change
     tout.
     ⚠️ **VSM essayé puis écarté** : c'est le seul type d'ombre dont
     `shadow.radius` règle le flou, mais il zébrait le sol de barres grises
     (retour utilisateur). Retour à `PCFSoftShadowMap`, dont le noyau est fixe
     en **texels** — la douceur s'y règle donc par la résolution de la carte
     d'ombre, à contre-intuition : 512 pour une ombre molle, monter la valeur
     la redurcit.

   ⚠️ **Piège de la méthode, vérifié par l'utilisateur.** La carrosserie sort
   plus claire que le `preview.jpg` (Supra verte : (53, 182, 72) contre
   (14, 81, 36)), et j'en avais conclu qu'il restait un écart à corriger, du
   côté de `ksAmbient + ksDiffuse`. **Comparaison faite avec le jeu lancé : il
   n'y a pas d'écart** — ce sont les `preview.jpg` qui sont plus sombres que
   le rendu d'AC. Le `preview.jpg` reste la bonne référence pour le *cadrage*
   et la *géométrie de l'éclairage*, il ne l'est pas pour le niveau absolu.
   Ne pas « corriger » la luminosité sur cette base.

8. **Anti-crénelage** ✅ **complété.** Le MSAA du contexte
   (`antialias: true`) ne lisse que les **bords de géométrie**. Trois sources
   de fourmillement lui échappaient, chacune avec son remède :
   - une texture vue en biais (décalcomanies d'une portière, rainures d'un
     pneu, le sol) → **filtrage anisotrope** au maximum de la carte, posé sur
     chaque texture après chargement ;
   - une **découpe en alpha** (calandre, jante ajourée, grillage), dont le bord
     vient d'un seuil que le MSAA ne voit pas → `alphaToCoverage` sur les
     matériaux à `alphaTest`, qui reporte ce bord sur la couverture des
     échantillons ;
   - le scintillement des **reflets** sur une carrosserie lisse, qui n'est pas
     un problème de bord → rendu à 1,5× au minimum puis réduit
     (`setPixelRatio`).

   **Complété ensuite par un niveau de qualité** (Réglages → Aperçu → Rendu),
   après que l'utilisateur a signalé des marches d'escalier persistantes sur
   deux voitures — le flanc d'une NSX et la calandre d'une Mustang. Les deux
   défauts ne sont pas de la même famille, d'où deux leviers :

   - un **reflet plus fin qu'un pixel** sur une surface quasi-miroir. Le MSAA
     ne peut rien pour lui : il échantillonne la *couverture* des triangles,
     mais n'ombre qu'une fois par pixel — un scintillement à l'intérieur d'une
     face lui est invisible par construction. Seul le suréchantillonnage
     l'attaque, parce que lui seul augmente le taux d'ombrage. D'où un facteur
     porté de 1,5 à 2,5 (Élevée) ou 4 (Ultra).

     **Le niveau est une cible, pas un plafond**, et la distinction est tout le
     réglage. Écrit d'abord en plafond — `min(max(dpr, 1.5), niveau)` — il ne
     servait à rien sur un écran à 1 dpi : le plancher de 1,5 l'emportait, les
     trois niveaux rendaient à l'identique, et l'utilisateur ne voyait aucune
     différence entre Standard et Ultra. La densité de l'écran reste un
     plancher (rendre sous elle serait flou), le niveau est le minimum visé, et
     une borne dure à 4 protège la mémoire.
   - une **géométrie sous-pixel** — des lames de calandre plus fines qu'un
     pixel — que le MSAA voit, mais où quatre échantillons saturent. Traitée
     par une passe **SMAA**, qui travaille sur l'image finie.

   Le montage SMAA porte un piège à connaître : dès qu'on passe par un
   `EffectComposer`, le rendu ne va plus dans le tampon d'écran et
   l'`antialias: true` du contexte ne s'applique plus à rien. Sa cible est donc
   créée à la main avec `samples: 4`, sans quoi activer SMAA *retirerait* le
   MSAA au lieu de s'y ajouter. Ordre des passes : rendu → SMAA →
   `OutputPass` — SMAA **avant** la sortie, et non après comme on l'écrirait
   spontanément pour un filtre morphologique : three.js documente cette
   implémentation comme travaillant en `linear-srgb` (en-tête de
   `SMAAPass.js`, r185).

   ⚠️ **Ce paragraphe est périmé sur les chiffres — voir le point 17.** Les
   trois niveaux étaient 1,5× / 2,5× / 4× ; il n'en reste que deux, et ils ne
   se comptent plus en valeur absolue mais en multiple de la densité de
   l'écran. Le raisonnement qui suit reste juste, c'est son application qui
   était fausse.

   Trois niveaux, **rendu seulement** : rien de ce qu'ils changent n'entre
   dans la conversion, donc en changer n'invalide aucune entrée de cache et
   s'applique à l'image suivante, sur une fiche déjà ouverte, sans recharger
   le modèle. Standard reproduit exactement ce que faisait l'app avant ce
   réglage ; Élevée est le défaut.

   **Suite, et deux corrections.**

   *Première fausse piste, la mienne.* Après le correctif cible/plafond,
   l'utilisateur a d'abord rapporté ne voir **aucune** différence entre
   Standard et Ultra, ce qui m'a fait soupçonner la plomberie — sept fois plus
   de pixels ne pouvant pas donner la même image. La prémisse était fausse :
   deux captures de l'arrière de la NSX montrent une différence franche (ovales
   des sorties d'échappement ronds au lieu de dentelés, cadre de plaque propre,
   jonc chromé continu). J'avais bâti quatre hypothèses sur une impression.
   **Une absence de différence rapportée à l'œil n'est pas une mesure.**

   *Seconde, sur SMAA.* La passe a été ajoutée, puis déplacée après
   `OutputPass` — argument chiffré à l'appui : son seuil de détection est 0,1,
   en dur dans le shader, et le linéaire écrase les sombres, un bord typique du
   bas de caisse (40 → 90 sur 255) pesant 0,196 en sRGB contre 0,078 en
   linéaire, donc sous le seuil. Le raisonnement était juste et **le résultat
   n'a rien donné** : comparée à l'écran sur le cas le plus défavorable qui
   soit — un jonc chromé quasi horizontal d'un pixel de haut sur fond noir — la
   passe n'a produit aucune différence visible, ni avant ni après le
   déplacement. Un essai à 5× n'en a pas produit non plus par rapport à 4×.

   **Conclusion, et état final** : la chaîne de post-traitement est **retirée**.
   Elle imposait un `EffectComposer`, donc **deux** cibles RGBA16F
   multi-échantillonnées (il clone la sienne) plus les deux tampons internes de
   SMAA — près d'un gigaoctet de mémoire graphique sur une fiche large — pour
   un gain que personne n'a pu voir. Le réglage se réduit donc à ce qui se
   voit, le **suréchantillonnage seul** : 1,5× / 2,5× / 4×.

   Trois effets de bord, tous favorables :

   - le MSAA du contexte (`antialias: true`) revient à **tous** les niveaux, et
     avec lui `alphaToCoverage` sur les découpes en alpha — le montage
     post-traitement les avait contournés ;
   - la mémoire du tampon tombe à 88 Mo (panneau 780 px) à 207 Mo (panneau
     1200 px) à Ultra, contre près d'un gigaoctet ;
   - le composant perd la gestion de cycle de vie de la chaîne, qui était la
     partie la plus fragile du montage.

   Un **budget en pixels** (16 Mpx) est conservé : il porte sur la surface et
   non sur le facteur, parce que c'est la fenêtre qui décide de la taille du
   panneau. Une allocation qui échoue ne dégrade pas l'image, elle fait perdre
   le contexte WebGL et laisse le panneau noir.

   **Ce qui reste ouvert** : un crénelage résiduel sur les lignes claires quasi
   horizontales, y compris à 4×. Le suréchantillonnage l'atténue sans le
   supprimer, et aucune passe en aval n'y a changé quoi que ce soit — ce qui
   oriente le prochain essai **en amont** : lissage des normales ou plancher de
   rugosité sur ces pièces fines, plutôt qu'un filtre de plus sur l'image finie.
   Si l'idée d'une passe de post-traitement revient, la leçon est qu'il ne
   suffit pas de l'ajouter : il faut prouver qu'elle se voit sur ce panneau-là.

   ⚠️ **Et surtout : ce point-ci ne parle que de crénelage *spatial*.** Le
   défaut que l'utilisateur a signalé ensuite n'en était pas un — voir le
   point 16, qui pose la distinction et la question à se poser en premier.

9. ~~**Certains mods s'affichent en carré/nuage de triangles bleus.**~~
   ✅ **Corrigé.** Signalé par l'utilisateur sur deux mods
   (`ms_citroen_berlingo_2003_vts`, `gmp_w204_c63_c13`) — pas un problème de
   protection CSP au sens où on l'avait prévu (PREVIEW§4.5) : les deux fichiers ont
   un magic KN5 parfaitement valide. La moitié de leurs triangles n'a plus de
   rapport cohérent avec sa normale stockée (PREVIEW§4.5bis, mesure détaillée dans
   `kn5-format.md`) — un magic intact n'était donc pas une garantie de
   géométrie exploitable. `preview::prepare` rejette maintenant ces modèles
   avant `convert()`, même repli que PREVIEW§4.5 (photo + badge + infobulle).
   `CONVERTER_VERSION` incrémenté pour que les entrées déjà mises en cache
   (converties avec succès mais montrant la géométrie cassée) soient
   reconverties — et cette fois refusées — plutôt que servies telles quelles.

10. **Effet d'entrée du plateau** (Réglages → Aperçu → Rendu). Trois choix :
    *Aucun* (la voiture tourne tout de suite à sa vitesse), *Progressif* — le
    défaut, une montée en douceur sur 1,2 s — et *Lancé*, qui part à cinq fois
    la vitesse réglée et décroît vers elle en ~2,6 s. Ce n'est qu'un facteur
    appliqué à la vitesse déjà calculée par image : aucune image de plus,
    aucun coût GPU, et le facteur se désarme tout seul une fois l'effet fini.
    Il est armé **au moment où le modèle arrive à l'écran**, pas à la fin de
    `build()` : commencé pendant la conversion, il serait à moitié joué avant
    d'être visible. Jamais sur un changement de skin à chaud — la voiture en
    place tourne déjà, la relancer serait un défaut et non un effet. Le bouton
    « replacer la voiture », lui, le rejoue. Un plateau à l'arrêt (vitesse 0)
    ou une préférence système « moins d'animations » n'en reçoivent aucun.

11. **Métallicité** ✅ **Faite**, et la question qui la bloquait est close.
    `metallicFactor` était tenu à zéro depuis le début du chantier faute de
    savoir lire R et B de `txMaps` : aucun matériau n'était métallique, donc le
    chrome, les optiques et le métal nu rendaient comme de la peinture
    brillante. Deux campagnes de mesure sur 6 597 textures et 16 791 matériaux
    (`kn5-tool maps`, écrit pour ça) : **R et B ne portent rien d'exploitable**
    — réponse négative, pas prudence — et la métallicité est dans `fresnelC`,
    la réflectance à incidence normale, vetée par `fresnelMaxLevel`. Détail,
    chiffres et pièges dans `kn5-format.md`, écarts n°7 et n°10.

12. **Fusion des maillages par matériau** ✅ **Faite.** Un appel de dessin par
    matériau au lieu d'un par nœud. Mesuré sur cinq voitures : 133 à 208
    primitives tombent à 36 à 56, soit **3,2× à 4,7× moins** — et sur Windows,
    où WebGL passe par la traduction D3D11, un appel de dessin n'est pas
    gratuit. Le panneau rendant en continu (plateau tournant), ce coût est payé
    soixante fois par seconde.

    Simple concaténation : les sommets sont déjà en espace monde à ce stade et
    les nœuds glTF écrits ensuite ne portent aucune matrice, donc il n'y a que
    des index à décaler. **La clé de fusion inclut la transparence**, parce que
    le drapeau est porté par le *maillage* et non par le matériau : les
    fusionner mélangerait un objet qui doit passer après l'opaque, sans
    écriture de profondeur (PREVIEW§8.2), avec un objet ordinaire.

    Les index passent en 32 bits **seulement quand il le faut** — un maillage
    fusionné peut dépasser 65 535 sommets, mais aucun ne le fait sur les cinq
    voitures mesurées, et les écrire tous en 32 bits gonflait le `.glb` de
    10 % pour rien. Le cache a une taille réglée par l'utilisateur ; la
    gaspiller en zéros de poids fort serait un mauvais échange.

    Vérifié : nombre de triangles **identique au triangle près** sur les cinq
    voitures, et `.glb` marginalement plus petit qu'avant (moins d'accesseurs).

13. **Reflet de la voiture au sol** ✅ **Fait**, et c'est la demande d'origine
    de l'utilisateur (« un effet vraiment sous la voiture, comme si elle était
    posée sur du matériel un peu brillant comme dans un salon »).

    `Reflector` de three.js, avec un shader dérivé du sien
    (`src/lib/preview3d/floorMirror.ts`). Le miroir brut de three est net et
    infini, ce qui donne un sol mouillé de jeu vidéo ; trois ajouts en font un
    sol de salon, et les trois sont exposés à l'utilisateur : un **flou** en 25
    prises pondérées, une **extinction radiale** pour que le reflet meure près
    de la voiture, et une **intensité** portée par l'alpha — le matériau est
    transparent, donc le reflet se compose par-dessus la flaque et l'ombre au
    lieu de les remplacer.

    Trois précautions, chacune née d'un vrai défaut :

    - **Le reflet ne montre que la voiture.** La flaque et l'ombre sont
      masquées le temps de la passe miroir, sans quoi elles se retrouvent dans
      leur propre reflet et le sol se dédouble.
    - **La cible du miroir suit le tampon de rendu**, elle n'a pas de taille à
      elle. Voir le point 16 : elle a été fixe à 512×512 pendant deux lots, et
      c'était la cause principale du scintillement.
    - **À 0 %, le miroir n'existe pas** plutôt que d'exister à l'opacité zéro.
      Et le remonter depuis 0 le construit **à chaud** : un curseur n'a pas à
      faire clignoter l'aperçu.

    ⚠️ **Le piège qui a failli tout faire abandonner** : la portée par défaut
    (30 %) éteignait le reflet **avant** qu'il n'atteigne la voiture. Le miroir
    fonctionnait, le réglage le masquait, et le constat « je ne vois aucun
    reflet » a bien failli conclure à un échec. La borne basse du curseur est
    donc haute exprès (20 %).

    **Méthode** : un banc hors application (three.js chargé à la main, la
    vraie NSX convertie par Pit Box, le même studio et le même cadrage) avec
    des curseurs, servi à l'utilisateur pour qu'il choisisse lui-même ses
    valeurs. Ce sont les siennes qui sont devenues les défauts — reflet 85 %,
    flou 0,5, portée 75 %, flaque 85 %, ombre 50 %.

14. **Réglages de décor** ✅ **Faits**, dans la foulée : exposition
    (`toneMappingExposure`), intensité de l'éclairage du studio et focale.
    La **focale recalcule la distance** pour que la voiture garde sa taille
    dans le cadre — sans quoi elle ferait doublon avec le zoom, alors qu'elle
    doit ne changer que la perspective.

    Les treize curseurs sont rangés en **groupes** (`PREVIEW3D_GROUPS`), chacun
    avec son bouton de remise à zéro posé à côté de ce qu'il remet à zéro.

    **Les défauts de cadrage ne sont plus ceux des `preview.jpg` Kunos** : ce
    sont ceux que l'utilisateur a choisis sur l'aperçu de l'écran Réglages —
    zoom 140 %, plongée 6°, hauteur −13 %, plateau à 50 %. Une vue nettement
    plus basse et plus serrée, tournant moitié moins vite. Ils ne s'appliquent qu'aux
    installations neuves : une préférence déjà écrite dans `ui_prefs.json` a la
    priorité, et c'est le bouton « rétablir » du groupe qui les fait apparaître
    chez quelqu'un qui y a déjà touché.

    L'aperçu de l'écran Réglages porte le même bouton « replacer » que la fiche
    voiture, et **changer l'effet d'entrée le rejoue** (`setPreview3dIntro`
    appelle `resetPreview3dView`) : un effet d'entrée ne se voit qu'à l'entrée,
    donc le choisir sans le déclencher revient à le régler à l'aveugle.

    ⚠️ **Rien ne s'enregistre tout seul**, et c'est un correctif après coup. La
    première version écrivait sur minuterie et au démontage des curseurs : le
    bouton Enregistrer ne décidait de rien, quitter l'écran validait en
    silence, et surtout **il n'y avait aucun retour en arrière possible** — le
    réglage d'avant était perdu dès le premier mouvement de souris (retour
    utilisateur). Le module tient donc deux états, `values` (ce qu'on voit) et
    `stored` (ce qui est sur disque), exactement comme l'onglet Général compare
    `config` à `savedConfig`. Un bouton **Annuler** revient sur l'enregistré, et
    la **garde de navigation** de `Settings.svelte` interroge les deux jeux de
    réglages — `setSectionGuard` n'ayant qu'un emplacement, c'est elle qui
    couvre aussi l'onglet Aperçu.

    Une seule exception, assumée : la **bascule photo/3D de la fiche voiture**
    s'enregistre sur-le-champ. C'est un interrupteur d'un clic, pas un
    formulaire, et personne ne s'attend à devoir aller valider ailleurs pour
    qu'il tienne.

15. **L'aperçu est passé dans l'écran Réglages** (idée de l'utilisateur), et
    c'est ce qui rend les treize curseurs tenables : on règle en voyant le
    résultat, sur le même composant `CarPreview3D` que la fiche. La voiture
    montrée est **celle de la session en cours** — à défaut la première de la
    bibliothèque : n'importe laquelle ferait l'affaire pour juger d'un sol ou
    d'une exposition, autant que ce soit celle que l'utilisateur a en tête.

    Le panneau compact posé sur la fiche voiture **disparaît** : il ne tenait
    que cinq curseurs sur treize, et le reste s'y serait entassé. Il ne reste
    qu'un raccourci vers l'onglet, qui passe par `nav.settingsTab` — l'onglet
    actif est un état interne de `Settings.svelte`, et le lui *demander* avant
    de naviguer évite de sortir cet état de son composant pour un seul
    appelant (même schéma que `nav.autoLaunch`).

    Côté présentation, les cinq blocs reprennent les classes globales du
    projet plutôt que d'inventer : `.blk`/`.blk-h`/`.blk-t` pour la carte et
    son titre rouge, `.blk-sub` pour les sous-rubriques. Les groupes de boutons
    radio passent **côte à côte** : empilés, trois options mangeaient une
    hauteur d'écran pour trois mots.

16. **Scintillement du reflet au sol** ✅ **Corrigé**, et le diagnostic vaut
    plus que le correctif.

    Signalé par l'utilisateur comme « de l'aliasing », particulièrement visible
    sur le reflet du sol. La phrase qui a tout débloqué est venue avec sa
    capture d'écran : **« ça choque beaucoup moins quand je fais une capture,
    quand ça bouge ça scintille »**.

    **Un défaut qu'on ne voit pas sur une image fixe n'est pas du crénelage
    spatial.** C'est la première question à poser, avant toute hypothèse : le
    défaut est-il sur l'image, ou entre deux images ? Elle sépare deux familles
    de causes qui n'ont aucun remède commun, et elle coûte une capture d'écran.
    Ici elle a éliminé d'un coup deux pistes que je tenais pour sérieuses — la
    taille fractionnaire du canevas (`clientWidth` est arrondi, la boîte de
    mise en page ne l'est pas) et un défaut de mipmapping — parce que l'une
    comme l'autre se seraient vues sur la capture. Le convertisseur écrit bien
    un échantillonneur trilinéaire (`minFilter: 9987`), vérifié au passage.

    Restaient deux causes, toutes deux **temporelles**, toutes deux dans
    `attachMirror`, et la seconde multipliant la première :

    - **La cible du miroir était fixe à 512×512.** C'est une texture *projetée
      à l'écran* : elle couvre le panneau, un texel pour un pixel quand les deux
      coïncident. Sur un panneau de 1268 px suréchantillonné 2,5× — donc un
      tampon de 3170 px — un texel couvrait **six pixels en largeur et quatre
      en hauteur**, la cible carrée sur un panneau rectangulaire ajoutant son
      anisotropie au manque de résolution. Une arête ne glissait donc pas dans
      le reflet, elle **sautait de texel en texel, cinq pixels d'écran à la
      fois**. En fixe, le grossissement bilinéaire adoucit tout ça — d'où une
      capture qui ne choque pas.

      Corollaire qu'il faut lire pour ce qu'il est : **le niveau de qualité
      n'atteignait pas le miroir**. Le facteur de suréchantillonnage se pose sur
      le tampon de rendu ; une cible dont la taille est un littéral y échappe
      par construction. Tout le raisonnement du point 8 — seul le
      suréchantillonnage augmente le taux d'ombrage, donc seul lui attaque un
      reflet spéculaire sous-pixel — s'appliquait au miroir sans jamais
      l'atteindre.

    - **Le reflet ne se rafraîchissait qu'une image sur deux.** L'économie
      était réelle et l'argument d'origine juste sur la *position* du reflet
      (deux dixièmes de degré, invisible sur une surface floutée) — mais le prix
      se payait sur sa **cadence**. Sauter une image ne fait pas retarder le
      reflet d'un dixième de degré : elle le fait avancer **par pas doubles à
      30 Hz sous une voiture qui tourne à 60**. Invisible en fixe, là encore.

    **Ce qui est en place** : `sizeMirror` aligne la cible sur le tampon de
    rendu à la construction, à chaque redimensionnement et à chaque changement
    de qualité (`applyQuality` rejoue `resize`), avec son propre budget de
    8 Mpx — plus bas que celui du tampon principal, parce qu'un reflet flouté
    puis éteint radialement ne rend rien d'une résolution qui dépasse l'écran.
    Le reflet se rafraîchit à chaque image.

    **Le MSAA de la passe miroir est retiré** (`multisample: 0`) et la mémoire
    part dans la résolution — même arbitrage qu'au point 8 : le MSAA
    échantillonne la couverture des triangles mais n'ombre qu'une fois par
    texel, alors que la cible suit désormais un tampon déjà suréchantillonné.
    Quatre échantillons auraient coûté quatre fois la mémoire pour le seul bord
    de géométrie.

    ⚠️ **Le piège de la montée en résolution, qui n'est pas celui qu'on croit.**
    Le pas du flou est exprimé en **UV**, donc en fraction du panneau : son
    étendue à l'écran ne dépend pas de la résolution, et c'est bien ce qu'on
    veut puisque c'est ce que l'utilisateur a calibré au banc. Mais 25 prises ne
    couvrent que ±2 pas. À 512 le pas valait un texel et le noyau était plein ;
    à 3170 il en vaut trois, et le noyau cesse de moyenner la cible pour en
    **échantillonner un peigne** — un peigne qui, sur une image en mouvement,
    scintille au lieu de masquer. Autrement dit, augmenter la résolution seule
    aurait remplacé un défaut temporel par un autre.

    Le remède est un uniforme `lod` passé en troisième argument de
    `texture2D` : on lit le niveau de mip dont le texel est aussi large que le
    pas, soit `log2(pas × largeur de la cible)`, borné à zéro. Les prises
    redeviennent jointives à toute résolution. Deux effets de bord : la cible
    demande ses mipmaps (three les régénère seul à chaque fois qu'il la délie,
    `updateRenderTargetMipmap`), et **le réglage de flou fort était déjà
    peigné avant ce correctif** — à 4,0 le pas valait quatre texels sur une
    cible de 512.

    Mesuré plutôt que supposé, parce que la spec WebGL2 ne le garantit pas :
    `generateMipmap` sur une texture RGBA16F rend `NO_ERROR` sur le GPU de
    l'utilisateur (AMD Radeon via ANGLE/D3D11, la pile de WebView2), avant comme
    après attachement au framebuffer, `EXT_color_buffer_float` et
    `OES_texture_float_linear` présents.

17. **Le suréchantillonnage n'a que deux valeurs possibles** ✅ **Corrigé**,
    et c'est le résultat le plus contre-intuitif du chantier : **les deux
    niveaux « qualité » dégradaient l'image**, et le plus élevé le plus
    fortement.

    Signalé par l'utilisateur juste après le point 16 : « en ultra sur le
    modèle (pas le reflet) je vois encore plus d'aliasing », capture à l'appui —
    des lignes claires quasi horizontales en marches d'escalier.

    **La cause n'est pas dans le rendu, elle est dans la réduction.** Le canevas
    est dessiné plus grand que le panneau, puis c'est le **compositeur du
    navigateur** qui le ramène à la résolution de l'écran, avec une seule prise
    bilinéaire et sans mipmap. Ce qui décide du résultat n'est donc pas le
    facteur de suréchantillonnage mais le **rapport de réduction**, et une
    prise bilinéaire ne tombe juste qu'à un seul rapport :

    | rapport | où tombe la prise | ce qu'elle lit |
    | --- | --- | --- |
    | 2 | sur le coin entre quatre texels | les 4, à poids égaux — **filtre boîte 2×2 exact, gratuit** |
    | 3 | sur le **centre** d'un texel | 1 texel sur 9 — la bilinéaire dégénère en plus proche voisin |
    | 4 | sur un coin | 4 texels sur 16 |
    | non entier | quelque part entre les deux | poids déséquilibrés, texels sautés |

    **Mesuré**, hors application : la même scène — des lignes claires fines et
    quasi horizontales, le défaut signalé — rendue à chaque facteur avec une
    couverture analytique (donc parfaitement lissée *à sa propre résolution*,
    pour que tout écart vienne de la réduction et non du rendu), réduite par
    minification bilinéaire sur GPU, et comparée à une référence obtenue par
    vrai filtre boîte depuis un rendu 16×. Écart quadratique moyen, sur 255 :

    | réduction | 1,00 | 1,33 | 1,50 | 1,67 | **2,00** | 2,50 | 2,67 | 3,00 | 4,00 |
    | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
    | RMS | 7,69 | 9,98 | 8,87 | 11,08 | **4,97** | 12,17 | 13,47 | **21,34** | 15,43 |

    Sur l'écran de l'utilisateur (`devicePixelRatio` = 1,5), les anciens niveaux
    donnaient des rapports de **1,00 / 1,67 / 2,67**. Autrement dit : Standard
    ne suréchantillonnait pas du tout (7,69, la ligne de base), et les deux
    niveaux censés améliorer l'image la **dégradaient** — Élevée à 11,08,
    Ultra à 13,47. L'utilisateur avait raison de bout en bout.

    Ça explique après coup deux observations du point 8 restées sans réponse :
    le « aucune différence entre Standard et Ultra » du début, et le « 5× ne se
    distinguait pas de 4× ». Et le correctif cible/plafond, juste dans son
    intention, avait poussé le rapport **plus loin dans la zone morte**.

    **Ce qui est en place** : le facteur vaut `devicePixelRatio × k` avec
    `k ∈ {1, 2}`, jamais autre chose. Deux niveaux, donc, et non trois — Ultra
    disparaît, y compris ses clés de locale ; une préférence `"ultra"` déjà
    écrite retombe sur Élevée toute seule (`oneOf`, `preview3dPrefs`). Le
    budget de pixels fait redescendre le **niveau** au lieu de borner le
    facteur : borner rendrait un rapport fractionnaire, exactement le défaut
    qu'on cherche à éviter. Il se mesure donc en pixels physiques, avant
    suréchantillonnage.

    **Pourquoi pas un troisième niveau** : il faudrait faire la réduction
    soi-même — cible hors écran à 4×, passe de filtre boîte. Or three désactive
    le tone mapping dès qu'on rend dans une cible (`WebGLPrograms`), donc il
    faudrait le refaire à la main *par échantillon* sous peine de faire fleurir
    les lucioles au lieu de les moyenner ; `alphaToCoverage` serait perdu avec
    le MSAA du contexte ; et la cible pèserait 133 Mio. C'est le montage
    construit puis retiré au point 8. Le gain irait de 4 à 16 échantillons par
    pixel écran — à reprendre le jour où quelqu'un prouve qu'il se voit.

    ⚠️ **Réserve de méthode, à ne pas oublier si on y revient** : le banc
    **modèle** le compositeur (une prise bilinéaire, sans mipmap), il ne le
    mesure pas — le code de Chromium n'a pas été lu. Ce qui le valide, c'est
    que sa prédiction est exactement ce que l'utilisateur voyait à l'écran.
    Si un jour Chromium mipmappe les calques de canevas, tout ce point est à
    refaire.

18. **Les leviers de rendu sont regroupés** dans un objet `TUNING` en tête de
    `CarPreview3D.svelte` (demande de l'utilisateur : « peux-tu me dire où sont
    les paramètres à changer ? c'est regroupé au même endroit ? »). Budget du
    tampon, budget et MSAA du miroir, taille et biais de la carte d'ombre, flou
    de l'environnement, plancher de rugosité — chacun avec ce qu'il fait et ce
    qu'il coûte. Rien de tout ça n'entre dans la conversion, donc une valeur
    changée s'applique à l'image suivante sans invalider le cache.

    Deux choses restent volontairement dehors : le **facteur de
    suréchantillonnage** (point 17 — c'est la seule valeur qu'on ne choisit pas
    librement) et le **filtrage anisotrope**, laissé au maximum de la carte.

    **Et deux leviers mesurés contre le scintillement en mouvement**, qui
    s'ajoutent l'un à l'autre. Banc hors application : un nœud torique
    quasi-miroir tournant d'un demi-degré entre deux images, huit paires, en
    comptant les pixels qui sautent de plus de 40 sur 255 — *quelques* pixels
    qui basculent violemment, c'est ça qui se lit comme du scintillement, pas
    beaucoup de pixels qui dérivent un peu. La moyenne des écarts avait d'abord
    été prise comme mesure et elle disait exactement l'inverse : à retenir, un
    défaut ne se mesure pas avec la première statistique venue.

    | `environmentBlur` | `roughnessFloor` | pixels sautant de +40 | luminance |
    | --- | --- | --- | --- |
    | 0,04 | 0 | 1,50 % (référence) | 36,1 |
    | 0,08 | 0 | 1,34 % (−11 %) | 38,0 |
    | 0,04 | 0,15 | 1,21 % (−19 %) | 39,1 |
    | **0,08** | **0,15** | **0,97 % (−35 %)** | 40,5 |
    | 0,15 | 0,15 | 0,92 % (−38 %) | 41,0 |
    | 0,30 | 0,15 | 0,92 % (−38 %) | 41,1 |

    L'effet **sature vers 0,08–0,15** : au-delà, plus de flou coûte du contraste
    et ne gagne rien. La dernière colonne est le prix — les surfaces ressortent
    ~12 % plus claires et plus plates, le chrome le moins miroir.

    **Le couple livré est 0,08 / 0,15**, la ligne en gras : la dernière qui gagne
    encore quelque chose. Le prix relevant du goût et non de la justesse, les
    valeurs ont été soumises à l'utilisateur et sont les siennes — même règle que
    les défauts de cadrage du point 14. `environmentBlur: 0.04` avec
    `roughnessFloor: 0` rend exactement l'aspect d'avant.

    ⚠️ **Et le niveau de qualité n'a rien à voir avec le scintillement.**
    Question posée au même banc après un retour de l'utilisateur (« ça scintille
    plus en Élevée »), en modélisant cette fois la réduction du compositeur avant
    de comparer les images :

    | niveau | réduction | p99 | p99,9 | pixels sautant de +40 |
    | --- | --- | --- | --- | --- |
    | Standard (dpr ×1) | 1,00 | 54 | 116 | 1,49 % |
    | Élevée (dpr ×2) | 2,00 | 54 | 114 | 1,47 % |
    | ancien « Ultra » 4× | 2,67 | 54 | 116 | 1,48 % |
    | Standard + les deux remèdes | 1,00 | 41 | 77 | 1,03 % |
    | Élevée + les deux remèdes | 2,00 | 42 | 78 | 1,07 % |

    Plat à un dixième de pour cent près. **Le suréchantillonnage joue sur la
    qualité *statique* des bords — c'est ce que mesure le point 17 — et sur rien
    d'autre.** Ce que l'utilisateur voit est réel mais n'est pas ce qu'il croit :
    en Élevée l'image est plus nette, donc le même scintillement devient plus
    lisible. Il n'augmente pas, il se voit mieux. Baisser le niveau pour le
    masquer reviendrait à flouter toute l'image pour cacher un défaut qui a son
    propre remède.

    **Les deux axes sont donc indépendants**, et c'est ce qui a fait tourner en
    rond pendant deux lots : chaque fois qu'un défaut de netteté était corrigé,
    le scintillement restait, et inversement. Devant un défaut de l'aperçu, la
    première question reste celle du point 16 — sur l'image, ou entre deux
    images ? — et la réponse décide de quel axe on parle.

    Le plancher de rugosité est **injecté dans le shader** et non posé sur
    `material.roughness` : three *multiplie* ce dernier par le canal vert de la
    carte de rugosité, donc il mettrait la carte à l'échelle au lieu d'en
    relever le plancher — et ici la rugosité vient de `txMaps` sur la plupart
    des matériaux (écart n°7). Point d'insertion juste après
    `<roughnessmap_fragment>`, vérifié sur three r185, avec un
    `customProgramCacheKey` sans lequel three mettrait en commun le programme
    compilé de deux matériaux qu'il croit identiques.

    ⚠️ **L'anticrénelage spéculaire géométrique a été essayé, mesuré, retiré.**
    C'était le remède que j'avais annoncé, celui que le point 8 appelait de ses
    vœux en écrivant « le prochain essai est en amont », et c'est la recette
    standard du domaine (Kaplanyan, puis Frostbite et Filament : replier la
    dérivée d'écran de la normale dans la rugosité). Il ne fait **exactement
    rien** ici : 1,50 % de pixels violents contre 1,51 %, inchangé jusqu'à
    quatre fois la force standard. La raison est structurelle — il se déclenche
    sur une normale qui varie vite d'un pixel à l'autre, c'est-à-dire sur une
    géométrie sous-échantillonnée, alors que ces voitures sont densément
    maillées et remplissent le cadre. **Le scintillement est dans la netteté du
    reflet, pas dans la géométrie.** Ne pas le réintroduire sans mesurer.

Restent aussi, hérités du plan initial : le choix du LOD en config, et l'aperçu
dans `ModDetail.svelte` (panneau latéral), qui n'a jamais été branché.

### Taille du `.glb` — attributs quantifiés

Question posée à l'envers pendant longtemps. « Réduire le cache » faisait
penser aux textures ; la mesure dit l'inverse. Sur onze entrées réelles :
**133 Mo de géométrie contre 92 Mo d'images**, soit 59 / 41. Le poste le plus
lourd n'était pas ce qu'on regarde mais quatre attributs écrits en `f32` alors
qu'aucun n'a besoin de 32 bits.

D'où `KHR_mesh_quantization` (extension ratifiée Khronos, gérée nativement par
`GLTFLoader` et par l'importeur Blender) :

| Attribut | Avant | Après | Pourquoi c'est sans risque |
| --- | --- | --- | --- |
| `NORMAL` | 12 o | 8 o (`short` normalisé + complément) | 0,002° d'erreur |
| `TANGENT` | 16 o | 4 o (`byte` normalisé) | oriente une carte de normales, dont le bruit de texel domine |
| `TEXCOORD_0` | 8 o | 4 o (`short` normalisé) | 1/32767 d'atlas, six centièmes de texel sur 2048 |
| `WEIGHTS_0` | 16 o | 4 o (`ubyte` normalisé) | 1/255 par poids, somme corrigée à 255 |

**Mesuré sur quatre voitures** (MX-5 Cup, GT-R, 911 RSR 2017, Abarth 500) :
géométrie **26,5 → 18,2 Mo (−31 %)**, fichiers **41,5 → 33,2 Mo (−20 %)**.

Trois choix méritent d'être retenus, parce qu'ils ne se devinent pas :

- **`short` pour les normales, `byte` pour les tangentes**, et pas `byte`
  partout (quatre octets de plus par sommet à gagner). La normale est ce que
  tout calcul d'éclairage consomme directement, et l'erreur d'un byte — un demi
  degré — se voit d'abord en bandes dans un reflet net qui balaie une grande
  surface courbe : le capot d'une voiture, exactement. La tangente ne fait
  qu'orienter une carte de normales. Le passage des normales en byte reste une
  ligne à changer si la place manque plus que la finesse.
- **Les UV se quantifient sur `[-1,1]` et pas sur le carré unité.** Le premier
  jet testait `[0,1]`, la plage d'un normalisé non signé, et n'a rien rapporté :
  **2 primitives sur 301** y tenaient. Les îlots d'UV d'AC débordent presque
  toujours un peu sans pour autant répéter — et la fusion par matériau
  (`geometry::merge_by_material`) aggrave tout, puisqu'un seul détail qui répète
  entraîne la carrosserie entière. En signé, **242 primitives passent, 3,6 Mo
  sur 4,1**. Il ne reste que 46 primitives qui répètent vraiment, et elles
  restent en flottants : un normalisé les écrêterait, et une texture posée de
  travers ne se signalerait nulle part.
- **`extensionsRequired`, et c'est la seule du projet.** Les extensions de
  matériau ne vont qu'en `extensionsUsed` : un lecteur qui les ignore rend un
  modèle un peu terne. Celle-ci change le *type* des octets d'un attribut — un
  lecteur qui la passerait sous silence relirait des entiers courts comme des
  flottants, donc un tas de triangles. Le refus franc vaut mieux.

`POSITION` reste en `f32` (6,2 Mo sur 18,2, le premier poste restant). Le
quantifier demande de rendre l'échelle par la transformation du nœud, or celle
d'un maillage skinné est **ignorée** par la spec : il faudrait la cuire dans les
matrices de liaison du mannequin. C'est le lot suivant, avec la déduplication
entre skins — mesurée à 34 Mo de géométrie strictement identique sur onze
entrées, une voiture à trois livrées écrivant trois fois les mêmes sommets.

### Taille des textures — ce qui marche et ce qui ne marche pas

L'autre moitié du poids (41 %). Quatre pistes essayées, **une seule retenue**,
et les trois abandons valent d'être écrits : ils reviendront sinon.

**Retenu — l'alpha constant ne s'écrit pas.** `texture::encode` calculait déjà
`has_alpha`, mais ne s'en servait que pour choisir le JPEG d'une carte de
couleur ; tout le reste partait en RGBA8, quatrième canal muet compris. Sur les
70 cartes de données d'une F40, **55 portent un alpha à 255 partout**, et le
laisser leur coûte 10 % de leur poids pour rien — un glTF sans canal alpha se
lit avec une opacité de 1, exactement ce que valait le canal. L'encodage y gagne
aussi du temps. Effet réel sur quatre voitures : **−1,7 %** des fichiers, le PNG
n'en faisant qu'un quart. Le test porte sur l'information et non sur le rôle :
une carte de données qui découpe vraiment garde son canal.

**Retenu — les cartes de données opaques peuvent perdre un peu.** Le module
l'excluait sur le principe : « les artefacts JPEG d'une carte de normales se
voient en ondulations sur un panneau ». C'est vrai d'un JPEG *ordinaire*, dont
le sous-échantillonnage de chrominance écrase le R et le V — là où vivent
justement le X et le Y de la normale. Or l'encodeur du crate `image` **n'en
fait pas** : h=1, v=1 sur les trois composantes, donc du 4:4:4. Le dégât se
mesure alors, et il est petit : sur les 35 cartes de normales d'une F40, à
q95, **0,75° d'écart angulaire moyen, 0,57 % des pixels au-delà de 5°, 0,0035 %
au-delà de 15°** — le pire cas (50°) étant une poignée de pixels isolés sur des
coutures. Les cartes de normales pèsent **65 % des octets de données** (5151 Ko
sur 7923) : les épargner ne laisserait qu'un tiers du gain.

**Mais le JPEG n'est gardé que s'il est réellement plus petit**, et ce
garde-fou n'est pas théorique : sur `ks_nissan_gtr`, dont les cartes de données
sont synthétiques (larges aplats, dégradés francs — le cas idéal du PNG et le
cas médiocre du JPEG), le q95 rendait un fichier **plus gros**, 2,50 Mo
d'images contre 2,48. Sans la comparaison, on aurait payé de la place *et* de
la qualité. Avec, aucune voiture ne régresse : Abarth 500 −9 %, 911 RSR −7 %,
MX-5 −4 %, GT-R −2 %, soit **−6 %** sur les quatre. `data_jpeg_quality: None`
ramène tout au sans-perte en une valeur — c'est le seul réglage de cette passe
dont la bonne valeur soit une affaire de goût plutôt que de mesure.

**Abandonné — WebP sans perte.** C'était la piste la plus prometteuse sur le
papier, et elle l'est toujours *avec libwebp* : mesuré par ffmpeg sur les mêmes
70 textures, **−43 %**. Mais l'encodeur disponible sans compilateur C — celui
d'`image` 0.25, en Rust pur via `image-webp`, dont la propre documentation
prévient qu'il « n'atteint pas le plein potentiel du WebP sans perte » — donne
**+10 %**, et **38 textures sur 70 en ressortent individuellement plus
grosses**. Il est trois fois plus rapide, ce qui ne sert à rien quand le
résultat est plus lourd. Reprendre cette piste veut donc dire : libwebp (donc
une chaîne C en CI) *et* `EXT_texture_webp`, l'extension même que le critère
d'acceptation du lot 3 (« s'ouvre dans Blender et dans un visualiseur en
ligne ») cherchait à éviter. Le gain qu'on y cherchait a été pris autrement,
par la perte sur les cartes de données ci-dessus.

**Abandonné — les cartes en niveaux de gris.** L'idée était de ranger en L8 les
cartes dont les trois canaux sont égaux (rugosité, occlusion, masques), ce qui
les diviserait par trois. Mesuré : **5 sur 70**, soit 215 Ko sur 7,9 Mo. La
plupart des `txMaps` d'AC empaquettent réellement trois choses différentes dans
leurs trois canaux.

**Abandonné — `CompressionType::Best`.** −2 % pour 2,8 fois le temps
d'encodage.

### Déduplication entre skins — le cache éclaté

Le plus gros des trois gains, et le seul qui **grandit avec l'usage**.

La clé de cache contient le skin, ce qui est correct — deux livrées ne donnent
pas le même rendu — mais chaque entrée écrivait un `.glb` autonome. Or, mesuré
sur trois skins de deux voitures : **une seule variante de géométrie pour les
trois** (au bit près), et les images partagées aux deux tiers. Seule la livrée
diffère vraiment. Une voiture à trois livrées écrivait donc trois fois les
mêmes sommets et deux fois sur trois les mêmes textures.

**Le rangement change, pas le format.** Une entrée est désormais un `.gltf`
(le document JSON) qui référence des blobs **adressés par leur contenu**,
rangés dans `previews/blobs/<empreinte SHA-256>.<bin|png|jpg>` :

- `buffers[0].uri` → la géométrie, un seul blob ;
- `images[i].uri` → une texture, un blob chacune.

`GLTFLoader` résout une URI relative depuis l'adresse du document, donc
`blobs/…` se demande au même protocole `carpreview` — c'est pourquoi le nom du
sous-dossier et le préfixe des URI ne font qu'une constante (`BLOBS`). Le
`.glb` autonome, lui, reste ce que produit `kn5-tool` : sa sortie doit s'ouvrir
telle quelle dans Blender, et un fichier éclaté ne le ferait pas. D'où
`ConvertOptions::layout`, autonome par défaut.

**Mesuré sur le cache réel de l'application**, quatre entrées d'une même
voiture : **123,5 Mo en monolithique → 43,2 Mo sur disque, soit −65 %.** Un
seul blob de géométrie de 11,4 Mo pour les quatre. Bénéfice qui n'est pas que
la place : le deuxième skin d'une voiture ne réécrit plus que ses textures
propres.

**Ce que l'éviction devient, et pourquoi elle compte à l'envers.** Un blob
n'appartient à personne : sa place ne se libère qu'une fois partie la dernière
entrée qui le cite. L'éviction parcourt donc les entrées de la plus récente à
la plus ancienne pour décider **qui reste**, en cumulant la taille de chaque
entrée plus celle des blobs qu'elle est la première à réclamer ; tout ce qui
suit le dépassement du plafond est supprimé, puis un balayage efface les blobs
que plus aucune entrée survivante ne cite. Compter en supprimant aurait
demandé de refaire la somme après chaque suppression.

Le balayage relit les entrées survivantes plutôt que de faire confiance à
l'ensemble accumulé : une suppression peut échouer, et effacer le blob d'une
entrée encore là donnerait un modèle sans texture — un dégât **silencieux**,
bien pire qu'un octet de trop. Il tourne aussi après un changement de version
du convertisseur : de nouveaux octets produisent de nouvelles empreintes, donc
laissent l'intégralité des anciens blobs derrière eux, de loin le plus gros tas
de fichiers morts que ce cache puisse accumuler.

**Deux pièges rencontrés à l'écriture, tous deux silencieux :**

- **Lire une référence de blob s'arrête au point, pas au premier caractère non
  hexadécimal.** Le `b` de `.bin` est un chiffre hexadécimal : la première
  version relisait `<empreinte>.b`, ne le retrouvait jamais parmi les fichiers,
  et **aurait effacé tous les blobs à la première éviction**. Un test le
  verrouille.
- **Un blob s'écrit sous un nom temporaire puis se renomme.** Un blob tronqué
  par une fermeture brutale serait ensuite réputé bon par toutes les entrées
  qui le citent, puisqu'on ne vérifie que son existence. Le renommage est
  atomique, donc un blob présent est un blob complet.

Et l'ordre d'écriture est fixe : **les blobs, puis le document**. Une entrée qui
cite un blob absent est un modèle cassé ; un blob que rien ne cite encore sera
simplement repris par le balayage suivant.

### Cache — ce qui est en place

Rappel, parce que la question revient : le cache est **sur disque**
(`%LOCALAPPDATA%\com.pitbox.app\previews`), donc il survit à un redémarrage —
une voiture déjà vue s'ouvre sans reconversion. Chaque entrée est un
`v<version>-<hachage>.glb` plus un `.txt` de compteurs à côté.

Le **numéro de version du convertisseur** (`preview::CONVERTER_VERSION`) est
dans le nom du fichier. Deux effets, et il faut les deux :

- une entrée d'une autre version n'est jamais servie, puisque son nom ne peut
  plus être demandé — c'est l'invalidation ;
- elle est **reconnaissable**, donc effaçable. Au premier aperçu de chaque
  exécution, les entrées d'une autre version sont supprimées. Sans ça elles
  restaient à occuper le disque jusqu'à ce que le plafond les évince : trois
  incréments en une session de travail avaient laissé plusieurs centaines de
  Mo derrière eux.

**Le plafond est un réglage** (Réglages → Aperçu → Cache), de 0,5 à 20 Go, à
2 Go par défaut. Il vit dans `ui_prefs.json` comme les autres réglages
d'aperçu, et c'est le **frontend qui le pousse au backend** — au chargement et
à chaque changement — plutôt que le backend qui irait le lire : le schéma de ce
fichier appartient au frontend (voir l'en-tête de `ui_prefs.rs`). Le backend
borne la valeur lui-même, et le plancher n'est pas cosmétique : un plafond
inférieur à une entrée évincerait un modèle à l'instant où il est écrit, donc
le reconvertirait à chaque visite — un réglage qui désactiverait le cache sans
le dire.

Baisser le plafond **évince tout de suite**, sans attendre la prochaine
conversion : quelqu'un qui réduit le plafond pour libérer de la place attend
que la place soit libre quand le chiffre affiché à côté du curseur change, pas
après avoir rouvert une voiture.

La purge manuelle depuis les Réglages garde son intérêt malgré ce ménage — le
cache d'une grande bibliothèque atteint vite le gigaoctet **en entrées
valides**. L'écran affiche la taille réellement occupée à côté du plafond :
sans elle, le réglage se règle à l'aveugle.