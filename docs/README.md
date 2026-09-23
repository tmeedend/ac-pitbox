# docs/ — Index

Documentation de conception de Pit Box (gestionnaire de mods Assetto Corsa).
Ce fichier liste **tout** le contenu de `docs/` : si un fichier est là, il est
dans cette liste. Un fichier ajouté sans sa ligne d'index est un fichier que
personne ne retrouvera.

## Comment le code renvoie ici

Le code porte près de 2 800 renvois `§` vers ces documents. Un numéro seul ne
dit pas lequel — le numéro `5.3` existe dans quatre specs à la fois — donc **un `§` nu
désigne `SPEC.md`** et les autres documents portent une étiquette :

| Étiquette | Document |
| --- | --- |
| *(rien)* | `SPEC.md` |
| `SESSION§` | `SPEC-session.md` |
| `GRILLE§` | `SPEC-grille.md` |
| `PILOTE§` | `SPEC-ecran-pilote.md` |
| `WIKI§` | `SPEC-wikipedia-fiche-detail.md` |
| `PREVIEW§` | `SPEC-preview-3d-kn5.md` |
| `FMOD§` | `SPEC-engine-sound-fmod.md` |
| `MUSIQUE§` | `spec-module-musique_2.md` |
| `IMPORT§` | `SPEC-import.md` |
| `REFONTE§` | `SPEC-refonte-navigation-et-fiches.md` |
| `TEXTURE§` | `SPEC-texture-update.md` |
| `INDEX§` | `SPEC-index-bibliotheque.md` |
| `TAXO§` | `SPEC-taxonomies.md` |
| `SETUP§` | `LOT-session-setup.md` |
| `CIBLE§` | `CIBLE-reglages-session.md` |
| `L1§` | `LOT1-forme-ecran-session.md` |
| `L2§` | `LOT2-selection-adversaires.md` |
| `L4§` | `LOT4-etat-de-piste.md` |
| `L5§` | `LOT5-refonte-ecran-session.md` |

`npm run check` vérifie que chaque renvoi tombe sur une section qui existe
(`scripts/check-refs.mjs`). **Conséquence pour qui édite un document ici :
renuméroter une section casse le contrôle**, et c'est voulu — c'est la seule
chose qui rendait jusqu'ici une renumérotation invisible au code.

## Référence principale

- **`SPEC.md`** — spécification de référence, organisée par domaine
  (architecture, identité/import, tags, fiche technique, bibliothèque,
  skins/sons/apps, lancement de session, maintenance, config, conventions).
  **Point d'entrée** : commencer ici. Décrit l'app telle qu'elle fonctionne.

## Les deux gros domaines, sortis dans leur fichier

- **`SPEC-session.md`** — **lancement de session** : la bibliothèque comme
  sélecteur, le pilotage par preset Quick Drive, l'écran de réglages avec le
  plateau et les conditions, l'aperçu 3D, l'écran Pilote. Extrait du §9 de
  `SPEC.md`, qui en portait 38 % à lui seul. Le code y renvoie par `SESSION§`.

## Import (le domaine le plus dense)

- **`SPEC-import.md`** — l'arbre de décision de l'import et la table des
  mécanismes de pose, sur une page. **Il ne double pas `SPEC.md` §4, il le rend
  vérifiable** : le §4 énonce les règles une par une avec le *pourquoi* de
  chacune, celui-ci les montre **ensemble** — la seule façon de voir qu'une
  règle en contredit une autre. C'est ainsi que `mods/` a pu figurer des mois
  dans la liste des dossiers qu'AC lit : personne ne pouvait voir la liste et
  ses conséquences en même temps. À lire **avant** de toucher à une règle
  d'import, et à rejouer contre les cinq archives de référence qu'il liste.
  En cas d'écart, `SPEC.md` fait foi, et la relecture se fait en même temps que
  la modification de la règle — jamais après. Le code y renvoie par `IMPORT§`.

## Instructions par lot — livrées, gardées pour leurs arguments

Cinq instructions ont piloté la construction de l'écran de session, dans cet
ordre. Toutes **livrées**. On les garde parce que le code y renvoie et parce
qu'elles portent les *arguments* derrière les choix — la partie qu'on ne
retrouve pas deux fois. `SPEC-session.md` décrit ce que l'écran **est**
aujourd'hui ; celles-ci disent ce qui a été demandé et pourquoi. En cas
d'écart, `SPEC-session.md` fait foi.

- **`CIBLE-reglages-session.md`** (`CIBLE§`) — la cible de l'écran : le bloc
  Adversaires où **le filtre remplace les onglets**, le plateau, les grilles
  enregistrées, l'import Content Manager. C'est lui qui porte « le filtre
  définit le vivier, jamais le plateau ».
- **`LOT-session-setup.md`** (`SETUP§`) — corrections et réorganisation de
  départ, SETUP§1.1 à SETUP§1.9 puis SETUP§2.1 à SETUP§2.11. C'est à ces deux moitiés que le lot 5
  renvoie quand il écrit « la règle 1.2 du lot 1 » et « le 2.6 du lot 2 ».
- **`LOT1-forme-ecran-session.md`** (`L1§`) — la forme de l'écran de réglages.
- **`LOT2-selection-adversaires.md`** (`L2§`) — la sélection d'adversaires.
- **`LOT4-etat-de-piste.md`** (`L4§`) — les presets d'état de piste de Content
  Manager.
- **`LOT5-refonte-ecran-session.md`** (`L5§`) — la refonte : le type devient la
  navigation, le repli `Performance`, le bloc `CONDITIONS` unique, la page
  adversaires. Il porte le principe directeur de l'ensemble — « un réglage se
  range selon sa portée, jamais selon sa fréquence d'usage ».

## Chantiers en cours

- **`CHANTIERS.md`** — le journal de bord : où en est chaque chantier, ce qui
  reste, et surtout **les pièges déjà payés une fois**. Il vivait dans
  `CLAUDE.md`, où il pesait 55 % du fichier relu à chaque session pour un
  contenu qui n'est pas une consigne. `CLAUDE.md` n'en garde qu'un tableau.
  **Point d'entrée pour reprendre un chantier à froid.**

Chacun porte en plus sa propre spec, et c'est elle qui décrit ce que l'app
fait — en cas d'écart, la spec fait foi.

- **`SPEC-refonte-navigation-et-fiches.md`** — rail à deux rangs, inventaire
  unique des compléments, anatomie de fiche commune. Énonce ce qui change, ce
  qui reste et ce qui devient caduc, section par section. **Point d'entrée.**
- **`PLAN-refonte-navigation.md`** — le plan de livraison en lots, et surtout
  les **mesures faites sur la bibliothèque réelle avant de commencer** : elles
  ont supprimé un lot entier (la détection CSP des voitures existait déjà) et
  démenti le fourre-tout redouté au REFONTE§14.4.
- **`SPEC-index-bibliotheque.md`** (`INDEX§`) — l'état par défaut des écrans
  Circuits et Voitures devient un **index** de tuiles (pays ; familles puis
  marques). Porte la règle qui tient tout le reste : la tuile **pose une
  puce**, elle n'ouvre pas un second mécanisme. Livrée.
- **`SPEC-taxonomies.md`** (`TAXO§`) — marques, pays et familles de catégorie
  deviennent des **termes** (nom canonique, alias, emblème), curés dans trois
  onglets de l'Atelier. **Seule la table des familles est livrée** (pour
  l'index) ; le reste est un chantier ouvert (`CHANTIERS.md`).
- **`SPEC-grille.md`** — lisibilité des cartes, affichage du nom, et
  **régénération des vignettes**. Porte la distinction qui structure le reste :
  « les voitures sombres sont indiscernables » recouvre deux problèmes — la
  carte qui ne se détache pas de la page (§2) et deux voitures sombres qui se
  ressemblent (§5). Les confondre conduit à régler le mauvais.
- **`SPEC-ecran-pilote.md`** — le corps (mannequin 3D) et la tenue en trois
  pièces. Porte l'asymétrie fondatrice : le corps est imposé par la physique de
  la voiture, la tenue tient à un fichier de skin.
- **`SPEC-wikipedia-fiche-detail.md`** — un extrait de l'article Wikipédia du
  véhicule ou du circuit réel, dans un onglet distinct de la description de
  l'auteur. Deux contraintes commandent tout le reste : la fonctionnalité est
  **décorative** (l'ambiguïté n'affiche rien, l'absence n'est pas une erreur),
  et l'affichage doit rester une **collection** au sens du droit d'auteur —
  jamais fusionné avec la description, jamais reformulé ni traduit.
- **`SPEC-preview-3d-kn5.md`** — le rendu 3D natif : parsing KN5 en Rust →
  glTF → three.js dans la webview. Décision d'architecture, layout binaire du
  format, plan par lots. L'avancement et le reste à faire sont à ses §13 à §15.
- **`SPEC-engine-sound-fmod.md`** — écouter le vrai moteur d'une voiture en
  passant par les DLL FMOD livrées avec Assetto Corsa, au lieu de deviner le
  ralenti par analyse du signal. Contient la position prise sur la licence
  FMOD, et les **écarts d'ABI mesurés** au FMOD§2bis — dont une structure dont la
  disposition documentée est fausse d'une manière qui ressemble à un succès.

## Pistes instruites, pas encore ouvertes

- **`SPEC-texture-update.md`** — les *texture updates* (un dossier de textures à
  copier dans **chaque** livrée d'une voiture), qui ne sont ni une livrée ni une
  couche telle qu'on les pose. Contient la **règle de détection mesurée** —
  100 % des fichiers du mod sont des textures de son `.kn5`, contre 58 % au
  maximum pour 75 livrées réelles ; 0 livrée sur 3585 dépourvue des quatre
  marqueurs de livrée — et les deux points de conception à trancher avant de
  coder. Rien n'est implémenté.
- **`windows-code-signing.md`** — signature Authenticode de l'installateur
  (écran SmartScreen « Éditeur inconnu »). À lire **avant** d'acheter un
  certificat : deux pièges y sont documentés, dont l'impossibilité depuis
  juin 2023 de mettre un `.pfx` dans les secrets de la CI.

## Formats de fichiers, mesurés sur des fichiers réels

Ce ne sont pas des specs mais des relevés : ce que le format fait *vraiment*,
avec la méthode de vérification. Toute découverte s'écrit là, pas seulement
dans un commentaire de code.

- **`kn5-format.md`** — le format des modèles 3D d'Assetto Corsa : écarts
  constatés avec la spec, réponses à ses questions ouvertes.
- **`fsb5-format.md`** — le format des banks de son FMOD (`.bank`) : conteneur
  FSB5, codec FADPCM, hypothèses écartées. **Son heuristique de ralenti est une
  impasse assumée** (40 sur 91) : elle ne sert plus que de repli et ne doit pas
  être retouchée — le paragraphe « Trouver le ralenti » explique pourquoi.

## Recherches préalables — pourquoi une piste a été abandonnée

À lire **avant** de retenter quelque chose dans ces directions.

- **`csp-driver-research.md`** — appliquer le pilote en jeu : ce qui marche
  (`[DRIVER3D_MODEL]` d'un `ext_config.ini` pour le corps, le `skin.ini` de la
  livrée pour la tenue), ce qui ne marche pas, et les quatre pistes écartées.
- **`showroom-3d-preview-research.md`** — les trois pistes explorées pour
  l'aperçu 3D (fenêtre Content Manager, parser maison, `acShowroom.exe`), et
  pourquoi l'intégration de la fenêtre native a été abandonnée.
- **`L4-cm-launch-research.md`** — pilotage de Content Manager, mené sur la
  source primaire AcTools. Dit pourquoi `race/config`/`PreparedConfig` a été
  abandonné : il ne déclenche pas le téléchargement CSP automatique, bug
  confirmé empiriquement et cause trouvée dans `GameWrapper.StartAsync`.
- **`controller-onboarding-design.md`** — choix du périphérique de contrôle.
  Part d'un bug réel (des éléments d'interface se déplaçaient seuls, volant
  branché) : `mapping === "standard"` est *déclaré* par le périphérique, pas
  vérifié, et un volant en mode Xbox s'annonce standard.

## Données embarquées

- **`kunos_content_dates.json`** — table statique du contenu officiel Kunos
  (178 voitures + 21 circuits, tirés des dossiers réels). Pour chaque entrée :
  `year` (année du modèle) et `release` (date de sortie dans AC via son pack).
  Section `packs` = dates des DLC. **Le seul fichier de `docs/` que l'app lit
  vraiment** : `kunos_dates.rs` l'embarque par `include_str!`.
- **`default-tag-rules-enriched.json`** — ontologie de tags (vocabulaire fermé
  + règles fusion/suppression/déduction/extraction/brand_fix). ⚠️ **Ce n'est
  pas le fichier que l'app charge** : elle sème
  `src-tauri/rules/default-tag-rules.json` dans le dossier de config au premier
  démarrage. Les deux ont divergé — celui-ci porte un groupe de règles de plus.
  L'éditer ne change rien au comportement de l'app.

## Maquettes et captures

- **`maquettes/`** — les maquettes HTML, **datées**, avec leur propre index
  (`maquettes/README.md`) qui dit pour chacune ce qu'elle a servi à décider et
  si elle fait encore autorité. Les périmées vivent dans `maquettes/archive/` :
  gardées pour le *pourquoi* qu'elles portent, jamais pour savoir à quoi
  ressemble l'app. Une maquette apporte l'**UX**, jamais l'UI — voir
  `CLAUDE.md`, « Le design system fait foi, pas la maquette ».
- **`screenshots/`** — trois captures de l'app réelle (grille, tableau, fiche).

## Historique

- **`README-livrables.md`** (2026-06-29) — doc d'amorçage du projet, **périmée**
  (elle renvoie à `acmm-spec.md`, devenu `SPEC.md`). Gardée pour la trace ;
  `SPEC.md` fait foi en cas d'écart.
- **`spec-module-musique_2.md`** (2026-08-09) — spec de référence du module
  musique (§16 de `SPEC.md`), écrite pour une **autre stack** (C#/NAudio). Les
  écarts de transposition vers Rust/`rodio` sont documentés en tête de
  `src-tauri/src/music/engine.rs`, pas ici.

## Méthode de travail

- Le SPEC décrit *ce que l'app est*. Les **bugs** se corrigent directement avec
  Claude Code (pas dans le SPEC). Les **consignes d'implémentation** ciblées se
  donnent par sujet, en pointant la section concernée — jamais tout le SPEC en
  demandant « construis l'app ».
- Langue de travail : français. Thème Rosso Corsa (§12 de `SPEC.md`).
