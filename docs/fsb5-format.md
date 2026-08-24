# Le format FSB5 — ce qu'il fait réellement

> Pendant du `kn5-format.md` pour le son. Même discipline : on n'écrit ici que
> ce qui a été **mesuré**, avec la méthode qui l'a établi, et on note aussi les
> hypothèses **écartées** — ce sont elles qui font perdre le plus de temps quand
> quelqu'un les retente en croyant avoir une idée neuve.
>
> Implémentation : `src-tauri/src/fsb5.rs`.

## Pourquoi lire ces fichiers

Pour **auditionner un mod de son sans lancer le jeu**. Le cadre « Son du moteur »
de la fiche voiture liste le son d'origine et les mods installés ; les écouter
demandait jusqu'ici de démarrer une session.

Les fichiers lus sont ceux de l'installation locale de l'utilisateur. Rien n'est
redistribué, et aucune bibliothèque FMOD n'est embarquée : le lecteur est écrit
à partir de la description du format, ce que le §2 du `SPEC-preview-3d-kn5.md`
tranche déjà pour le KN5 — les offsets et constantes d'un format sont des faits
techniques, c'est le *code* d'un tiers qui est sous licence.

## Le conteneur

Un `.bank` est un fichier RIFF (`RIFF … FEV FMT …`) dont la charge utile contient
une section **FSB5**. On la trouve en cherchant son magic plutôt qu'en
parcourant l'arbre RIFF : cet arbre porte les métadonnées de FMOD Studio, dont
on n'a aucun usage, et l'ignorer retire tout un format de la surface à
maintenir.

En-tête FSB5, à partir du magic :

| offset | champ |
| --- | --- |
| 0x00 | `FSB5` |
| 0x04 | version (1 en pratique ; **0 ajoute un mot**, donc en-tête de 64 au lieu de 60) |
| 0x08 | nombre d'échantillons |
| 0x0c | taille de la table d'en-têtes |
| 0x10 | taille de la table de noms |
| 0x14 | taille du bloc de données |
| 0x18 | **codec, un seul pour tout le bank** |

Puis : table d'en-têtes, table de noms, bloc de données.

### L'en-tête d'échantillon — le point qui ne se devine pas

Un `u64` empaqueté, suivi d'une chaîne optionnelle de chunks :

| bits | champ |
| --- | --- |
| 0 | des chunks suivent |
| 1–4 | index de fréquence |
| 5–6 | canaux − 1 |
| 7–29 (23 bits) | offset des données **× 32** |
| 34–63 | nombre d'échantillons |

Index de fréquence : 1 = 8000, 2 = 11000, 3 = 11025, 4 = 16000, 5 = 22050,
6 = 24000, 7 = 32000, 8 = 44100, 9 = 48000, 10 = 96000.

Chunks connus : 1 = canaux, 2 = fréquence (écrase le champ empaqueté),
3 = points de boucle (deux `u32`).

**Méthode.** Ces bornes ont été tranchées par une contrainte que le PCM16
fournit gratuitement : un échantillon PCM16 occupe exactement
`nombre × 2 × canaux` octets. Sur les 52 échantillons du bank d'origine de la
GT40 :

| lecture | résultat |
| --- | --- |
| offset 23 bits **× 32**, nombre à partir du bit 34 | écarts de **+32 à +92 octets**, jamais négatifs — de l'alignement |
| offset 27 bits × 16, nombre à partir du bit 34 | **51 longueurs négatives sur 52** — impossible |

Deux erreurs commises en chemin, toutes deux silencieuses :

- le nombre d'échantillons lu à partir du **bit 30** donnait un rapport
  `octets / (nombre × canaux)` de **0,125** au lieu de 2 — soit exactement 16
  fois trop, ce qui a désigné le décalage de 4 bits ;
- la longueur d'un échantillon **ne figure nulle part** : elle se déduit du
  début du suivant, le dernier courant jusqu'à la fin du bloc.

### La table de noms est souvent absente

`taille = 0` sur les deux mods de son de la GT40, et sur 8 des 297 banks du
corpus. Quand elle existe : `n` offsets `u32` depuis son propre début, puis des
chaînes terminées par zéro.

**Conséquence de conception** : choisir un échantillon ne peut pas reposer sur
son nom. Les banks Kunos en ont (`idle_1383`, `mk1_idle_1655a`, `5167b_off` —
le régime y est écrit en clair), les mods n'en ont pas.

## Les codecs rencontrés

Relevé sur les 297 voitures de `content/cars` :

| codec | voitures |
| --- | --- |
| PCM16 (2) | 291 |
| VORBIS (15) | 4 |
| FADPCM (16) | 2 |

⚠️ **Ce relevé est trompeur et a failli faire sous-estimer le travail.** Il
porte sur du contenu très majoritairement **Kunos**. Les **mods**, eux,
compressent : les deux mods de son de la GT40 sont en FADPCM, et sans table de
noms. Autrement dit, la partie facile ne couvre que le son qu'on veut le moins
écouter. **Mesurer sur la bonne population.**

Vorbis reste non décodé : il demanderait les codebooks de FMOD, qui ne sont pas
un fait de format. Le codec est nommé dans l'erreur plutôt que de rendre du
silence.

## FADPCM

Trame de **0x8c = 140 octets par canal**, pour **256 échantillons**
(`(0x8c − 0x0c) × 2`). Les canaux sont entrelacés **par trame**, pas par
échantillon.

| offset | champ |
| --- | --- |
| 0x00 | `u32` : huit index de coefficient, 4 bits chacun |
| 0x04 | `u32` : huit facteurs de décalage, 4 bits chacun |
| 0x08 | `i16` : historique n−1 |
| 0x0a | `i16` : historique n−2 |
| 0x0c | 128 octets : huit sous-blocs de 16, soit 32 nibbles chacun |

Coefficients, huit entrées dont seules les cinq premières sont non nulles :
`(0,0) (60,0) (122,60) (115,52) (98,55)`.

Reconstruction, par échantillon :

```
décalage    = 22 − facteur
résidu      = (nibble << 28) >> décalage      // le << 28 porte le signe
échantillon = (résidu + c1·h₁ − c2·h₂) >> 6   // borné en 16 bits
h₂ ← h₁ ; h₁ ← échantillon
```

⚠️ **Le pas croît avec le facteur de décalage.** Écrit `22 − facteur` après
avoir cadré le nibble en haut d'un mot de 32 bits, il se lit comme un décalage à
droite et se comporte comme un décalage à gauche : le résidu vaut
`nibble × 2^(6 + facteur)`. Une recherche exhaustive menée en supposant
l'inverse n'a jamais approché la cible — **la réponse n'était pas dans l'espace
fouillé**, et aucun élargissement du même espace ne l'y aurait mise.

**Vérification indépendante du décodeur** : les index de coefficient réellement
rencontrés sont **tous dans 0–4**, les seules entrées non nulles de la table. Un
champ mal aligné les étalerait sur 0–15. C'est le contrôle le moins cher qui
soit sur l'alignement de l'en-tête.

### Hypothèses écartées

- ❌ **`hist1`/`hist2` ne sont pas les deux derniers échantillons de la trame
  précédente.** L'idée était séduisante — elle aurait donné un oracle *exact*
  pour valider n'importe quelle formule, au lieu d'un score statistique. Mesuré :
  0 à 2 correspondances sur 499 trames sur les échantillons propres. Ils
  amorcent la trame, ils ne la chaînent pas.
- ❌ **Une forte autocorrélation ne prouve rien à elle seule.** Une table de
  coefficients instable (second coefficient additionné au lieu d'être soustrait)
  donnait un pic de **0,70** — meilleur que le vrai décodage sur certains
  échantillons — parce qu'un prédicteur divergent oscille d'une butée à l'autre,
  ce qui est parfaitement périodique et parfaitement faux. **Toujours rejeter la
  saturation avant de regarder la périodicité.**

## Comment on valide un décodage audio

Trois niveaux, du moins au plus concluant.

1. **L'étalon.** Le même véhicule a un bank PCM16 (l'original Kunos) et des
   banks FADPCM (les mods). Le PCM16 se décode trivialement et donne l'échelle :
   un vrai échantillon moteur a un pic d'autocorrélation normalisée de **0,53 à
   0,84** (`mk1_idle_1655a` : 0,840 à 55 Hz), un bruit de vent est à **0,10**.
   Sans cette échelle, un score de 0,25 paraît encourageant alors qu'il est faux.
2. **La cohérence interne.** Index de coefficient dans 0–4 ; absence de
   saturation ; et pour les deux échantillons qui *saturent* quand même, leurs
   en-têtes **stockés** sont à 49–58 % en butée — écrit par l'encodeur, donc la
   source est réellement écrêtée et le décodeur ne diverge pas.
3. **L'oreille.** Export en `.wav` et écoute par l'utilisateur. C'est le seul
   juge qui tranche pour du son, et c'est lui qui a validé le décodeur.

Le portage Rust a ensuite été comparé au prototype JavaScript **empreinte par
empreinte** sur six échantillons de 48 000 valeurs : identiques au bit près.
C'est ce qui permet de dire que le code livré est bien celui qui a été écouté.

## Trouver le ralenti sans nom d'échantillon

C'est le point **non résolu** de ce chantier, et il vaut d'être décrit : la
moitié du corpus n'a pas de table de noms, et il faut alors reconnaître un
ralenti à la seule analyse du signal.

### Le piège central : l'autocorrélation mesure la boucle, pas le moteur

Une couche moteur est une **boucle**. Son autocorrélation culmine donc sur la
période de la boucle, pas sur celle de l'allumage : un lâcher de gaz de deux
secondes y ressort à 20 Hz quel que soit son régime. Ranger les candidats par
« fondamentale la plus basse » revenait à les ranger par « boucle la plus
longue » — et un lâcher de gaz à 4000 tr/min passait sous un ralenti.

### Comment on sait que l'estimateur est juste

Les noms Kunos portent le **régime**. Pour un quatre temps, `f0 × 60 / régime`
vaut la moitié du nombre de cylindres : ce rapport doit donc être **constant**
sur tous les échantillons d'une même voiture. Sur la BMW 1M (six cylindres,
3,00 attendu) :

| règle | rapport | dispersion |
| --- | --- | --- |
| maximum global de l'autocorrélation | 0,56 | 25 % |
| plus petit retard ≥ 0,5 × pic | 2,77 | 21 % |
| **premier maximum local ≥ 0,3 × pic, après la première descente sous zéro** | **2,92** | **9 %** |

Trois règles, chacune née d'un défaut mesuré :

- **le plus petit retard**, pas le maximum — sinon on mesure la boucle ;
- **après la première descente sous zéro** — juste après le retard nul
  l'autocorrélation est encore haute, et un son pur de 60 Hz y franchissait
  n'importe quel seuil dès 600 Hz ;
- **le premier maximum local**, pas le premier franchissement — le seuil est
  franchi avant le sommet, ce qui donnait 75 Hz pour 60.

### Ce que ça donne, et ce que ça ne donne pas

Mesuré sur 91 banks Kunos, dont les noms donnent la bonne réponse :

| version | choix acceptables |
| --- | --- |
| avant (maximum global, sonde 0,4 s) | 15 / 91 |
| après (estimateur corrigé, sonde 1,2 s, filtre de stabilité) | 40 / 91 |

Deux fois et demie mieux, et **toujours faux dans plus de la moitié des cas**.
Les erreurs restantes ne sont pas absurdes — ce sont d'autres couches moteur à
bas régime, intérieures plutôt qu'extérieures, en lâcher de gaz plutôt qu'en
charge. Mais « le » ralenti n'est pas identifiable par l'analyse seule : rien
dans le signal ne distingue un ralenti extérieur d'un régime bas en lâcher de
gaz. **Ne pas repartir en réglage de seuils en espérant y arriver** — la suite
est de laisser l'utilisateur choisir, pas de deviner mieux.

Un filtre de **stabilité** s'ajoute à l'estimateur : l'écart-type du niveau sur
des fenêtres de 50 ms, rapporté à sa moyenne. Un ralenti tient son niveau et
reste sous 0,15 ; un son de mise en route enfle et meurt, et marquait 0,30 sur
la variante CSP d'une vraie voiture, où l'app le jouait en boucle.

## Ce qui reste ouvert

- **Choisir le ralenti sans nom d'échantillon.** Le `GUIDs.txt` ne donne que des
  noms d'*événements* (`engine_int`, `engine_ext`), pas d'échantillons ; remonter
  de l'un à l'autre demanderait de lire le graphe FMOD Studio. Piste retenue :
  l'autocorrélation, qui donne déjà le régime — sur le mod AmplifiedNL elle sort
  23 candidats moteur classés par fréquence, sans le moindre nom.
- **Vorbis** (4 voitures sur 297).
- **Le surplus de 32 à 92 octets** entre la fin des données d'un échantillon et
  le début du suivant. Sans conséquence pour le décodage — on lit `nombre`
  échantillons et on ignore la queue — mais mal expliqué par un simple
  alignement sur 32.
