# Les *texture updates* — la couche développée sur toutes les livrées

> **État : recherche faite, rien d'implémenté.** Tout ce qui suit est mesuré sur
> l'installation de référence (3585 livrées, 311 voitures). Les chiffres sont
> la partie qui ne se retrouve pas deux fois : si ce chantier est repris,
> commencer par les relire plutôt que par re-mesurer.

## 1. Le mod qui a soulevé la question

`ClimaxF1_ks_alfa_romeo_155_v6_texture_update` ([OverTake
72138](https://www.overtake.gg/downloads/alfa-romeo-155-v6-texture-update.72138/)).
Un dossier à plat de 28 fichiers `.dds`/`.png` — habitacle, alcantara, carbone,
pneus — et rien d'autre : ni `ui_skin.json`, ni `preview.jpg`, ni sous-dossier.

Sa notice tient en une ligne : *copier les textures dans **chaque** dossier de
skin*. C'est là tout le problème — ce n'est **ni une livrée ni une couche** au
sens de Pit Box :

- **Pas une livrée** : elle ne porte pas `Skin_00.dds`, la texture de livrée de
  cette voiture. Posée comme une livrée de plus, elle habillerait une seule
  livrée, ce que l'auteur ne veut pas.
- **Pas une couche** telle qu'on les pose : à la racine de la voiture, ces
  `.dds` ne sont lues par personne. AC ne cherche une texture qui surcharge
  celles du `.kn5` que dans le dossier de la **livrée courante**.

D'où la forme visée : une **couche développée**, dont l'arbre est
`skins/<chaque livrée>/<fichier>`. Le moteur de couches sait déjà déployer et
retirer exactement cela (§4.4 du SPEC) — c'est la *détection* et
l'*expansion* qui manquent, pas la pose.

Importé aujourd'hui, ce dossier finit en « autre mod » de nature
`UNRECOGNISED`, ce qui est littéralement exact : rien dans sa structure ne dit
ce qu'il est.

## 2. Ce que la mesure dit — et c'est net

### 2.1 Le signal décisif : les noms sont ceux du `.kn5`

Les 28 fichiers du mod comparés aux 74 textures embarquées dans
`Alfa_Romeo_155_V6.kn5` (`kn5-tool inspect --textures`) : **28 sur 28**
correspondent, à la casse près (`tyre_d.dds` ↔ `Tyre_D.dds`). Aucun intrus.

Une livrée, elle, apporte toujours des fichiers **qui ne sont pas dans le
modèle** — son aperçu, son `ac_crew.dds`, ses pneus numérotés. Mesuré sur
75 livrées réelles de 5 voitures :

| | part des fichiers présents dans le `.kn5` |
| --- | --- |
| `ks_alfa_romeo_155_v6` (10 livrées) | moyenne 37 %, max 54 % |
| `ks_porsche_911_gt3_r_2016` (5) | moyenne 50 %, max 58 % |
| `ks_mazda_mx5_cup` (17) | moyenne 43 %, max 50 % |
| `ks_audi_r8_lms_2016` (24) | moyenne 47 %, max 50 % |
| `ks_lamborghini_huracan_gt3` (19) | moyenne 53 %, max 54 % |
| **le texture update** | **100 %** |

**Jamais plus de 58 % pour une livrée, 100 % pour le mod.** La séparation ne se
joue pas à quelques points près, et c'est ce qui rend la règle défendable.

### 2.2 Le garde-fou gratuit : les quatre marqueurs de livrée

Sur les **3585 livrées installées** (contenu de base + mods déployés),
**zéro** est dépourvue des quatre fichiers `ui_skin.json` / `preview.jpg` /
`livery.png` / `skin.ini`. Aucune exception.

Un dossier à plat, fait d'images seules, qui n'en porte **aucun** n'est donc
pas une livrée — et cette porte-là ne coûte rien à franchir.

### 2.3 Le risque de recouvrement est théorique ici

Aucun des 28 noms du mod n'existe dans les 10 livrées de l'Alfa : la couche ne
recouvrirait aucun fichier qu'une livrée personnalise. Le cas reste possible
ailleurs — à signaler sur la fiche, jamais à trancher en silence.

## 3. La règle de détection proposée

Trois portes, de la plus gratuite à la plus coûteuse. Les trois doivent passer.

1. **Ce n'est pas une livrée.** Dossier à plat (aucun sous-dossier), rien que
   des images, et **aucun** des quatre marqueurs du §2.2. *Mesuré : 0 faux
   positif sur 3585 livrées.*
2. **Une seule voiture connue est nommée dans le nom du dossier.**
   `fragment::name_hosts` fait déjà exactement cette recherche de sous-chaîne
   (id d'au moins 5 caractères), et `fragment::only` refuse de trancher si deux
   voitures correspondent — ne pas deviner vaut mieux que se tromper d'hôte.
3. **Tous les fichiers sont des textures de son `.kn5`.** La confirmation.

### Ce que la porte 3 demande au crate `kn5`

Une lecture **des noms de textures seulement**. Les textures sont la *première*
section du format (en-tête → textures → matériaux → nœuds, cf.
`crates/kn5/src/parse.rs`), donc un lecteur dédié s'arrête avant les matériaux
et les nœuds, et saute les blobs au lieu de les copier. `kn5::parse` complet
copie les 24 Mo de textures de l'Alfa pour rien.

Attention au piège déjà documenté (`kn5-format.md`) : **une entrée de type 0
n'a ni nom ni taille** — la sauter entièrement, sinon toute la section se
désynchronise.

## 4. Les limites à assumer, et elles sont réelles

- **Un dossier qui ne nomme pas sa voiture ne sera pas détecté.**
  `MyTextureUpdate/` reste un « autre mod ». La porte 3 pourrait en principe
  *trouver* l'hôte en testant toutes les voitures, mais c'est 311 `.kn5` à
  ouvrir par import — pas envisageable sans un cache des noms de textures par
  version dans l'overlay. À garder pour plus tard, si le cas se présente
  vraiment.
- **La voiture doit être installée**, faute de quoi il n'y a pas de `.kn5` à
  interroger. Repli naturel : le chemin « en attente de son hôte » (§4.3bis),
  exactement comme une couche dont la base manque.
- **La règle ne couvre que les voitures.** Un circuit n'a pas de livrées au
  même sens ; rien de mesuré de ce côté.

## 5. Les deux points de conception à trancher avant de coder

### 5.1 Une livrée ajoutée après coup

Une couche développée à l'import ne couvre que les livrées **présentes ce
jour-là**. Ajouter une livrée ensuite la laisserait sans les textures, en
silence — le genre d'écart qu'on ne voit jamais.

**Parti proposé** : re-développer la couche à **chaque recomposition** plutôt
qu'une fois à l'import. La liste des livrées est de toute façon relue là, et
`compose::recompose` est déjà l'entonnoir unique par lequel tout passe.

### 5.2 Le coût disque

28 fichiers × 10 livrées, ce serait 10 copies. Les stocker **une fois** et
hardlinker dans chaque livrée à l'expansion ramène le coût au fichier unique :
`deploy::link_or_copy` fait déjà exactement cela, repli en copie compris.

## 6. Ce qui reste à décider et n'a pas été instruit

- Où vit la couche développée en bibliothèque : un `layers/<voiture>/<nom>/`
  déjà développé sur disque, ou un dossier plat plus une expansion au
  déploiement ? Le second est plus juste (§5.1) mais demande que
  `compose_tree` sache développer, ce qu'il ne sait pas.
- Ce que la fiche montre : une couche de plus dans `LayersBlock`, ou un type à
  part ? Elle ne se comporte pas comme les autres — elle touche N dossiers.
- Le recouvrement d'un fichier qu'une livrée personnalise (§2.3) : le signaler
  où, et sous quelle forme ?

## 7. Comment refaire les mesures

```bash
# Noms des textures d'un .kn5
cargo run -q -p kn5-tool -- inspect <voiture>/<modele>.kn5 --textures

# Livrées dépourvues des quatre marqueurs, sur toute l'installation
# (0 sur 3585 au moment de la mesure)
for sk in "$AC"/content/cars/*/skins/*/; do
  [ -f "$sk/ui_skin.json" ] || [ -f "$sk/preview.jpg" ] ||
  [ -f "$sk/livery.png" ] || [ -f "$sk/skin.ini" ] || echo "nue: $sk"
done
```
