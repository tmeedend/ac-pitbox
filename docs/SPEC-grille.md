# Pit Box — grille de la bibliothèque

## Spécification d'implémentation

*Lisibilité des cartes, affichage du nom, et régénération des vignettes. Ni les filtres (§7.1 de `SPEC.md`), ni la fiche d'un mod.*

> **État.** La partie A (§2 à §4 : la carte, le nom, la densité) est **livrée et visible**. La partie B (§5 à §8 : les vignettes régénérées) est **construite mais éteinte** — `FEATURE_GRID_THUMBS`, dans `src/lib/features.ts`, qui porte le mode d'emploi : ni l'onglet de réglages, ni le profil « Soigné » de l'assistant, ni la génération ne sont visibles, et les cartes gardent la `preview.png` du mod. Il ne manque que le réglage des valeurs par défaut des trois presets (`CHANTIERS.md`).

---

# 1. Diagnostic

« Les voitures sombres sont indiscernables » recouvre **deux problèmes distincts**, et les confondre conduit à régler le mauvais :

**A — Les cartes ne se détachent pas de la page.** Un problème de contenant, réglé par le §2.

**B — Deux voitures sombres se ressemblent.** Les `preview.png` portent leur propre fond, cuit dans l'image, et viennent de sources hétérogènes : aucun réglage du contenant n'agit derrière la voiture. Réglé en partie par le §3 (identifier plutôt que voir), en entier par le §5 (régénérer).

**Contrainte permanente** : certaines voitures sont chiffrées et ne seront **jamais** régénérables (§7). La grille reste donc mixte pour toujours, et le traitement du contenant est requis dans tous les cas.

---

# 2. Traitement de la carte

## 2.1 Ce qui change

La carte a un fond au-dessus de celui de la grille, une bordure visible, une élévation (liseré clair en haut, ombre portée), et l'image posée **en retrait sur un mat neutre**. Les valeurs sont celles du design system (`global.css`).

## 2.2 Le mat

L'image est posée sur un rectangle **au rapport 16:9** — celui des previews d'AC —, un **dégradé radial** clair au centre et sombre aux bords, avec sa bordure. Il fait trois métiers :

- **il unifie deux sources** : les `preview.png` opaques le recouvrent, les vignettes transparentes le laissent voir ; c'est ce qui rend les deux comparables dans une grille mixte ;
- **il porte le fond des vignettes transparentes**, en CSS, donc il suit le thème, la densité et les états de carte sans jamais régénérer une image ;
- **il absorbe les formats inattendus** : l'image est en `contain`.

## 2.3 Ce qu'on ne touche pas

**Les images elles-mêmes** : aucun filtre, aucune correction de luminosité, aucun masque. Le gain vient du contenant, ou de la source (§5).

## 2.4 Marqueurs d'état : suppression

**On joue dans la grille, on gère dans le tableau.** Les marqueurs d'état d'installation ont quitté les cartes : ils vivent dans la vue tableau, la fiche et les filtres (`État`, `Déjà essayé`). **Il ne reste qu'un marqueur dans la grille, le badge `SESSION`** — la sélection courante, pas un état de mod (niveau 2 du barème d'accent, §7.2ter de `SPEC.md`).

## 2.5 L'exception : ce qui empêche de jouer

Un mod cassé, incomplet, privé d'une dépendance ou désactivé change la décision. **La carte entière est éteinte** (opacité réduite, nom en gris) : ça se comprend au premier regard, sans légende ni traduction. La **raison** est dans l'infobulle et la fiche, pas dans la grille. **Ces cartes ne sont jamais masquées** : elles restent sélectionnables et consultables, seul le lancement est bloqué, avec l'explication à ce moment-là.

---

# 3. Affichage du nom

## 3.1 Le problème réel

Sur une ligne, trois Skyline s'affichaient à l'identique — `Nissan Skyline GT-R R3…` —, la troncature tombant juste avant ce qui les distingue.

## 3.2 Deux lignes : obligatoire

Le nom tient sur **deux lignes au plus**, tronqué en fin de seconde, et **la hauteur des deux lignes est réservée** même pour un nom court : sans quoi les cartes n'ont pas la même hauteur et la grille se déchire.

## 3.3 Hiérarchie typographique

Trois niveaux de gris : le **nom**, puis la **marque**, puis le séparateur et l'**année** — l'identité se sépare de la métadonnée, et la grille se balaie sans lire.

## 3.4 Retrait de la marque — option

Quand le nom commence par sa marque, ce préfixe peut être retiré **à l'affichage de la grille seulement**. **Désactivé par défaut** : la troncature est déjà réglée par les deux lignes, et les voitures se désignent par leur nom complet. Le réglage, *Masquer la marque dans le nom*, vit dans le **menu d'affichage** accroché à la bascule de vue (§4) : il faut en voir l'effet pour le juger.

**Règle** : si le nom commence, sans la casse ni les tirets, par la marque ou l'un de ses **alias** (`Alfa`, `Mercedes`, `Chevy`, `VW`…) suivi d'une espace, et qu'il reste quelque chose, on affiche le reste ; sinon le nom entier. La table d'alias reste courte : un alias trop gourmand amputerait une identité (`displayName.ts`).

## 3.5 Portée du retrait — trois interdits

- **Le tri porte toujours sur le nom complet** : c'est ce qui garde les Nissan sous N. Le tri par nom complet *est* le tri par marque.
- **L'index de recherche contient le nom complet** : « nissan skyline » trouve une carte qui affiche « Skyline GT-R R34 ».
- **Le nom stocké n'est jamais modifié** : retrait au rendu, réversible instantanément.

Ils valent aussi pour la vue tableau, qui affiche le nom entier.

---

# 4. Densité

La bascule de vue porte **l'index** (INDEX§3), **grille dense**, **grille confortable** et **liste**. Les deux grilles ne diffèrent que par la largeur minimale d'une carte. **Le choix est persisté par type** (voitures et circuits ne se regardent pas pareil). Un chevron accolé à la bascule ouvre le **menu d'affichage** : la densité (nommée) et le retrait de la marque.

---

# 5. Vignettes régénérées

## 5.1 Principe

Le moteur qui rend la voiture en 3D sur la fiche produit aussi les vignettes de la grille : **même cadrage, même éclairage, même fond pour toute la bibliothèque**. Une fois les fonds homogènes, l'œil compare les *formes*, ce qu'il ne peut pas faire quand il se réadapte à chaque carte.

## 5.2 Coût disque — deux caches distincts

| | Contenu | Ordre de grandeur | Éviction |
|---|---|---|---|
| **Cache 3D** | modèles convertis des voitures consultées | ~20 Mo par voiture | plafonné (PREVIEW§5.3) |
| **Vignettes** | images finies (`app_cache_dir()/gridthumbs/`) | ~150 Ko par voiture et par jeu d'images | jamais — le balayage efface ce qu'aucun preset vivant ne réclame |

## 5.3 Le pipeline de vignettes ne passe pas par le cache LRU

**Point critique** : le réutiliser le remplirait de voitures que personne n'ouvrira, en évinçant celles qu'on consulte. La conversion écrit dans un **brouillon** (`previews/scratch/`, `preview::prepare_scratch`), vidé avant chaque conversion et jeté aussitôt l'image rendue — pic disque d'une voiture à la fois. Ni l'éviction ni la mesure d'occupation ne le regardent.

## 5.4 Génération au fil de l'eau

**Une carte demande sa vignette quand elle entre dans le champ de vision**, jamais au chargement de la liste : la bibliothèque se normalise pendant qu'on l'utilise. Ce qui devient visible passe devant la file, et changer de filtre la réordonne. **La génération rend la machine pendant une session** : elle se suspend **dès le clic** sur « Démarrer » et reprend à la fermeture du jeu ; hors session, elle tourne sur un pool borné à la moitié des cœurs, pour que l'interface ne saccade pas.

**L'identité d'une vignette** est le nom de l'entrée de cache de la voiture (`.kn5` et sa date, livrée, `ext_config.ini`, version du convertisseur) **plus l'empreinte du gabarit** et `RENDERER_VERSION` (`gridthumbs.rs`) : un mod mis à jour, un gabarit changé ou une correction du code de rendu se régénèrent d'eux-mêmes, sans invalidation à écrire. `RENDERER_VERSION` s'incrémente dès qu'une correction change les pixels produits.

## 5.5 Profils à l'installation

**Le choix se pose à l'installation, chiffré sur sa propre bibliothèque**, en deuxième page de l'assistant — après les chemins, quand le contenu de base est indexé et que « 312 voitures détectées » veut dire quelque chose :

- **Léger** — pas d'aperçu 3D, pas de vignettes régénérées ;
- **Normal** (présélectionné) — aperçu 3D sur la fiche, vignettes d'origine, cache de 2 Go ;
- **Soigné** — aperçu 3D et vignettes régénérées, cache de 4 Go, plus le poids des vignettes.

Nommés **par leur résultat**, chiffrés sur la bibliothèque scannée, avec la phrase « Modifiable à tout moment dans les réglages ».

## 5.6 Paramètres de rendu — le gabarit

**Il n'y a pas un gabarit mais trois presets embarqués**, parce que la preview d'AC est plus *jolie* que notre rendu et le nôtre plus *lisible* — deux objectifs, pas deux qualités d'exécution :

| Preset | Ce qu'il cherche | Ce que porte l'image |
|---|---|---|
| **Catalogue** | identifier vite | la voiture seule, détourée, sur fond transparent |
| **Vitrine** | avoir envie de regarder | + une flaque de lumière et un reflet coupé court |
| **Officiel** | être **indiscernable** d'une `preview.png` du jeu | + son fond entier, cuit dans l'image |

**La propriété non négociable** : dans un preset, toute valeur est **identique pour toute la bibliothèque**. Elle porte sur un jeu d'images, pas sur le nombre de jeux possibles.

- **Les embarqués sont en lecture seule, et se dupliquent** : une copie de Vitrine est à un réglage d'être la sienne. Les embarqués évoluent avec l'app ; une copie ne bouge jamais.
- **Un preset par densité de grille** : dense → Catalogue, confortable → Vitrine par défaut. Les deux jeux d'images **coexistent** sur disque, donc rebasculer est instantané une fois les deux produits.
- **Le fond de carte (le mat) fait partie du preset.** Il n'entre dans l'empreinte d'une vignette **que lorsqu'il est cuit** dans l'image : sinon un changement de thème régénérerait toute la grille, ou un fond peint ne correspondrait plus à sa carte.
- **Le sol appartient au preset, et il décide de la transparence** : Catalogue n'a pas de sol ; Vitrine allume flaque et reflet, ceux de l'aperçu de la fiche (`studioFloor.ts`, `floorMirror.ts`), et cuit donc une part de son fond ; Officiel cuit le sien entier.
- **Officiel** a pour critère « est-ce indiscernable », que mesure la bascule *comparer à l'image d'origine* de l'atelier — la `preview.png` réelle à côté du rendu, sur le même mat. Son cadrage est **mesuré sur les 178 voitures officielles** (§11.4). Il porte **« laisser le contenu de base tel quel »** : les voitures Kunos ont déjà ce rendu.
- **Format** : 16:9, PNG à canal alpha prémultiplié (sans quoi un liseré clair borde les carrosseries sombres), une **ombre de contact** cuite (sans elle la voiture flotte).
- **Caméra** : trois-quarts avant, **l'angle relevé sur les previews Kunos**, jamais inventé (une grille mixte où la moitié des voitures pointe à l'opposé fatigue l'œil) ; cible au centre de la boîte englobante, cadrage **ajusté** à sa largeur (chaque voiture remplit le cadre), roues droites.
- **Éclairage** : clé, complément, et **contre-jour** — le paramètre le plus utile au problème de départ, celui qui détache la silhouette d'une carrosserie noire —, sans dominante colorée. **Exposition fixe, aucune auto-exposition par voiture**, qui ramènerait voitures noires et blanches au même gris.
- **Ce que l'utilisateur règle** (`Réglages › Vignettes`) : angles, champ, marge, intensité des lumières, ombre, sol et mat. Le format, l'exposition fixe et l'absence de post-traitement ne se règlent pas : ce sont eux qui garantissent la comparabilité.

## 5.7 Versionnage du gabarit d'origine

Aucun : les presets embarqués évoluent avec l'app et les copies ne bougent jamais, donc il n'y a rien à deviner. C'est `RENDERER_VERSION` (§5.4) qui périme les images quand le code de rendu change.

---

# 6. Réglage du gabarit des vignettes

## 6.1 Séparé de l'aperçu 3D

| | Portée | Conséquence d'un changement |
|---|---|---|
| **Aperçu 3D de la fiche** (`Réglages › Aperçu`) | une vue | aucune, jamais |
| **Presets des vignettes** (`Réglages › Vignettes`) | toutes les images de la grille | régénération |

C'est cette séparation qui rend acceptable qu'un écran affiche une facture et que l'autre n'avertisse jamais.

## 6.2 L'aperçu de réglage est une grille, pas une voiture

Régler l'angle sur une seule voiture l'optimise pour elle et massacre les autres : **l'aperçu est une grille de six voitures**. La **voiture de la session** occupe la première case (l'utilisateur sait à quoi elle doit ressembler) ; les cinq autres sont prises une par catégorie distincte. Elles sont chargées une fois et gardées en mémoire : bouger un curseur redessine, ne reconvertit jamais.

## 6.3 Rien ne s'applique avant « Appliquer »

Manipuler les réglages ne régénère rien. Le pied de l'écran affiche la facture en continu (« Appliquer régénérera 298 vignettes · environ 6 min »), et Annuler ne coûte rien. L'original d'un preset embarqué est toujours là, à côté de sa copie.

## 6.4 Changement de gabarit en cours de génération

Appliquer un preset change l'empreinte de ses vignettes (§5.4) : celles de l'ancien ne correspondent plus, et chaque carte redemande la sienne au fil de l'eau, dans l'ordre habituel ; les anciennes sont remplacées au fur et à mesure.

## 6.5 Régénération sur mise à jour de l'application

Une version qui change le rendu (`RENDERER_VERSION`) ou un preset embarqué périme les images concernées, qui se régénèrent au fil de l'eau (§5.4) ; la grille reste utilisable entre-temps.

---

# 7. Voitures chiffrées et autres échecs

Une voiture qui ne peut pas être rendue **garde sa `preview.png`**.

- **Pas de badge sur la carte** : l'utilisateur n'y peut rien, et un marqueur d'erreur salirait une carte utilisable.
- **Le décompte se dit une fois, à la fin**, dans le rapport de génération (`298 générées · 14 impossibles (voitures protégées)`), sans tonalité d'échec.
- **L'échec est mémorisé avec l'empreinte du mod** : la tentative ne se refait que si le mod change.

C'est ce cas qui rend le §2 obligatoire pour toujours.

---

# 8. Suivi de la génération

## 8.1 Ce n'est pas un toast

Trois cents voitures à environ une seconde font cinq minutes : **une tâche de fond persistante** (`GridThumbToast`), dans la pile en bas à droite, qui ne se ferme pas seule, survit à la navigation, se réduit au lieu de disparaître et porte une annulation.

## 8.2 États

- **Déplié** : le titre, la barre et `47 / 312`, la voiture en cours, le temps restant et « Annuler ».
- **Réduit** : une barre d'une ligne — la pile n'a qu'une forme (§4.2bis de `SPEC.md`) —, **sans le nom de la voiture**, qui ferait clignoter le coin de l'écran.
- **Terminé** : le rapport reste jusqu'à fermeture explicite.

## 8.3 Trois règles de comportement

- **Le temps restant n'apparaît qu'après une dizaine de voitures** : avant, l'estimation est fantaisiste.
- **Annuler conserve ce qui est fait, et le dit** (« 148 vignettes générées · reprendre plus tard »).
- **Les cartes se remplissent une par une**, sans recharger la grille ni perdre le défilement.

## 8.4 Génération et navigation

Elle ne bloque rien : filtres, tri, navigation et lancement restent disponibles ; changer de filtre réordonne la file.

---

# 9. Repères visuels

Les valeurs (fonds, bordures, gris, largeurs minimales des deux grilles, gouttière, rayon) sont celles du design system et des composants de la grille. Seule règle propre à l'écran : **le rouge n'y paraît que sur la carte en session** (bordure et badge `SESSION`).

---

# 10. Hors périmètre

- **Aucune vignette du contenu de base n'est embarquée** : les previews Kunos sont des œuvres protégées, et un rendu des modèles Kunos une œuvre dérivée. Le contenu de base passe par la génération sur la machine de l'utilisateur, comme le reste.
- **Le traitement des images d'origine** (correction, détourage, harmonisation automatique) est écarté : on garde la source ou on en change.
- La vue tableau n'est concernée que par les trois interdits du §3.5.

---

# 11. Ce que l'implémentation a changé

Le plan d'origine décrivait **un** gabarit transparent et son versionnage ; l'implémentation en a fait trois presets (§5.6), a cuit le fond quand le preset le demande (§5.6), et a ajouté la version du code de rendu à l'identité d'une vignette (§5.4). Le texte ci-dessus est l'état actuel ; `CHANTIERS.md` garde le reste à faire.

## 11.4 Deux mesures qui ont corrigé la spec

Relevées sur les 178 voitures officielles :

- **le fond des previews Kunos est plat et quasi noir** : ce qu'on prend pour un halo derrière la voiture est la flaque au sol devant elle. Un dégradé radial derrière la voiture aurait été une erreur ;
- **la voiture n'occupe que 66 % de la largeur** du cadre, et elle y est basse (33 % de marge en haut, 17 % en bas).

**Les deux côtés se mesurent avec le même instrument** : à marge nulle, notre cadrage n'occupe déjà que ~75 % de la largeur, parce qu'il ajuste la boîte englobante projetée en 3D, plus grande que la silhouette.

---

*Pit Box · spécification grille · maquette de référence `pit-box-barre-et-grille.html`*
