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
