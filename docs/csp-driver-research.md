# Appliquer le pilote en jeu — ce qui marche, ce qui ne marche pas

Compte rendu de recherche. Question de départ : **peut-on imposer un corps et
une tenue de pilote à une voiture, en jeu, sans casser le checksum en ligne ?**

Réponse : **oui pour le corps**, par une section de config CSP ; **oui pour la
tenue**, par le `skin.ini` de la livrée ; et les deux passent par des fichiers
différents, pour des raisons qui tiennent à AC et pas à nous.

Tout ce qui suit est mesuré sur l'installation de référence (312 voitures,
CSP présent) ou vérifié dans `dwrite.dll`, le cœur natif de CSP.

---

## 1. Le corps : `[DRIVER3D_MODEL]` dans `ext_config.ini`

**Ça marche.** Vérifié à l'écran : `abarth500` avec

```ini
; content/cars/abarth500/extension/ext_config.ini
[DRIVER3D_MODEL]
NAME=driver_501
```

affiche bien `driver_501` au volant, en session.

**Pourquoi ça marche.** CSP surcharge une section d'un ini de `data.acd` depuis
`ext_config.ini` en **préfixant le nom du fichier** à celui de la section :
`driver3d.ini` + `[MODEL]` → `[DRIVER3D_MODEL]`. Les deux chaînes existent
littéralement dans `dwrite.dll`, entourées de `DriverModel::DriverModel`,
`loadDriverBasePos` et d'un message de log `Driver model \`` — c'est bien le
code de chargement du mannequin qui les lit :

```
DRIVER3D_MODEL
DRIVER3D_HIDE_OBJECT
```

**Pourquoi le checksum tient.** `data.acd` n'est pas touché. C'est le conteneur
que les serveurs vérifient ; un config CSP n'en fait pas partie.

**`POSITION` : ne pas l'écrire.** La documentation officieuse et les réponses
d'IA suggèrent d'ajouter `POSITION=0,0,0` « au cas où le pilote traverse le
siège ». C'est un mauvais conseil ici : la mesure faite pour l'aperçu 3D (voir
`kn5_gltf`'s `seating_offset`) montre que `[MODEL] POSITION` **n'est pas** un
décalage d'assise — l'appliquer déplace treize voitures de l'installation
jusqu'à cinq mètres. On l'omet, et la voiture garde le sien.

**Bonus, non testé** : `[DRIVER3D_HIDE_OBJECT_n]` existe aussi, pendant du
`[HIDE_OBJECT_n]` de `driver3d.ini`. De quoi masquer une pièce précise —
typiquement le casque — sans changer de corps. À garder en tête, mais un corps
conçu pour porter un casque n'a pas de tête modélisée dessous.

## 2. La tenue : le `skin.ini` de la livrée, et rien d'autre

**Pas de route CSP.** Cherché, et non trouvé :

| Piste | Ce que c'est réellement |
| --- | --- |
| `HIDE_DRIVER_SUIT` | réglage de `small_tweaks.ini` : masquer la combinaison en vue interne |
| `EXT_SKIN_ID` | protocole réseau d'`ACClient` |
| `CUSTOM_SKIN_OCCLUSION` | cuisson d'occlusion de RainFX |
| configs par voiture | 195 sur l'installation, **aucune** ne définit `SUIT`/`GLOVES`/`HELMET` |

La tenue reste donc ce qu'elle est dans AC : une section du `skin.ini` d'une
livrée, nommée d'après le mannequin.

```ini
[driver_no_HANS]
SUIT=\sparco\blue
GLOVES=\sparco_roadcars_rg3.1\blue
HELMET=\helmet_base_blue\7
```

Le format est universel : sur 2542 `skin.ini` présents, 399 des 400 premiers
examinés portent déjà cette section. L'écrire, c'est faire ce que fait l'auteur
de chaque livrée.

## 3. La conséquence qui lie les deux

Le nom de la section **est celui du mannequin**. Donc remplacer le corps par
`[DRIVER3D_MODEL] NAME=driver_501` rend la section `[driver]` de la livrée
inopérante : il faut écrire la tenue sous `[driver_501]`.

C'est exactement la règle que l'écran Pilote applique déjà côté aperçu
(substituer le corps fait tomber la garde-robe de la livrée, §10.1) — elle
n'était pas une commodité d'interface, c'est le comportement du jeu.

## 4. Pistes explorées et écartées

**`[MODEL_REPLACEMENT_N]`** — mauvais outil. Ses seules clés sont `HIDE`,
`INSERT` et `INSERT_AFTER` : elle masque des meshes et greffe un autre `.kn5`
après un nœud, c'est-à-dire de la géométrie **statique**. Le pilote est un
*skinned mesh* déformé par le squelette de la voiture et son animation de
braquage ; un modèle greffé ainsi resterait figé pendant que les bras de
l'autre bougeraient. Confirmé par l'usage : sur tout ce que CSP livre, les
`FILE=` de ces sections visent des voitures et des circuits, jamais un
mannequin.

**`[CUSTOM_DRIVER_MODELS] DRIVER_MODEL_KEY`** (`small_tweaks.ini`) — vraie
fonctionnalité, mauvais périmètre. C'est « *ton* pilote », un seul, **global**,
tiré d'un catalogue distant au format `<List ID>/<Model ID>` — pas un chemin
vers `content/driver/`. Le drapeau `ALLOW_ANY_CUSTOM_DRIVER_MODELS` montre au
passage que les serveurs connaissent cette fonctionnalité et peuvent la
refuser.

**`ac.replaceDriverModel`** (API Lua de CSP) — marcherait, par voiture, mais
supposerait que Pit Box livre un script Lua dans `extension/lua/` du jeu :
une surface entièrement nouvelle, à suivre à chaque version de CSP. Sans objet
maintenant que la section de config suffit.

**Repacker `data.acd`** — écarté d'emblée. Sur les 312 voitures, **aucune**
n'expose son `driver3d.ini` en clair et **toutes** ont un `data.acd`. C'est le
fichier que les serveurs vérifient.

## 5. Ce qu'il reste à régler pour en faire une fonctionnalité

Rien de tout cela n'est un inconnu de recherche — ce sont des problèmes
d'ingénierie, listés pour ne pas être redécouverts.

1. **Les deux fichiers sont dans le mod.** `<voiture>/extension/ext_config.ini`
   et `<livrée>/skin.ini` vivent dans le dossier de la voiture, qui est un
   hardlink par fichier ou une junction vers la bibliothèque. Écrire là modifie
   donc la copie de bibliothèque. En hardlink on peut casser le lien avant
   d'écrire ; en junction, non — il faut sauvegarder l'original.
2. **Le retour en arrière exige un registre.** Ce qu'on a écrit, et ce qu'il y
   avait avant. C'est la discipline de `gamebackup.rs`, à étendre à des
   fichiers de mod.
3. **Fusionner, jamais écraser.** Un `ext_config.ini` existant fait couramment
   plusieurs centaines de lignes (735 sur la NSX de l'installation), et un
   `skin.ini` porte d'autres sections (`[CREW]`). On ajoute ou on remplace une
   section, on ne réécrit pas le fichier.
4. **Une écriture par livrée réellement pilotée**, au lancement, et seulement
   si le contenu diffère — c'est la stratégie qui écrit le moins.
5. **Le checksum de la tenue** reste argumenté et non prouvé : un `skin.ini`
   est hors de `data.acd`, et tout le monde roule en ligne avec des skins
   personnalisés. Le corps, lui, est réglé : `ext_config.ini` non plus n'est
   pas dans `data.acd`.
