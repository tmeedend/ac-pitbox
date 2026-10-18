# Pit Box — la fiche technique d'une voiture

## Spécification d'implémentation

*La fiche technique passe en base, avec une source par valeur ; la physique du mod remplace les devinettes quand elle sait répondre ; l'électronique embarquée apparaît ; l'utilisateur peut tout corriger ; et la fiche reste entière pour un mod en vitrine et dans un export.*

> **Étiquette de renvoi : `FICHE§`.** Livrée (`src-tauri/src/techsheet/`). Maquette : `maquettes/pitbox-fiche-technique.html`, qui juge la disposition et les gestes, pas l'UI.

---

# 1. Objet

La fiche technique d'une voiture garde **chaque source** de chaque valeur — la physique du mod, son `ui_car.json`, les tags, et ce que l'utilisateur a corrigé —, en affiche la meilleure en disant d'où elle vient, lit dans la physique ce que les tags ne faisaient que deviner (transmission, turbo, rapports, réservoir, électronique), et **se corrige** à la main. Elle est en base : elle reste entière pour un mod en vitrine et dans un export.

---

# 2. Ce que la mesure a montré

## 2.2 Ce que la physique dit, et où

| Donnée | Où |
|---|---|
| Roues motrices | `drivetrain.ini [TRACTION] TYPE` (`RWD`, `FWD`, `AWD`, `AWD2`) |
| Nombre de rapports | `drivetrain.ini [GEARS] COUNT` |
| Boîte en H | `drivetrain.ini [GEARBOX] SUPPORTS_SHIFTER` |
| Suralimentation | `engine.ini`, sections `[TURBO_n]` |
| Régime maximal | `engine.ini [ENGINE_DATA] LIMITER` |
| Réservoir | `car.ini [FUEL] MAX_FUEL` |
| Électronique | `electronics.ini`, `drs.ini`, `kers.ini`, `ers.ini`, `ctrl_*.ini` (§5) |

Trois pièges que seule la mesure montrait :

- **Le fichier `drs.ini` ne veut pas dire DRS.** Il existe sur 66 voitures, vide sur 59 d'entre elles (une Abarth 500, un Macan). Seule une section `[WING_n]` désigne un aileron piloté. Le mod RSS Formula 2013 et la McLaren P1 de route l'ont ; la Macan ne l'a pas.
- **La traction ne se lit pas en un seul champ.** Les 5 désaccords sont les hybrides à moteurs électriques avant (919 Hybrid, TS040, R18 e-tron, 918 Spyder) : `TYPE=RWD` pour le moteur thermique, et `ers.ini [FRONT_MOTORS]` pour l'essieu avant électrique. **Ce sont les règles qui avaient raison** : ce sont des voitures à quatre roues motrices. La physique doit lire les deux fichiers ensemble.
- **La physique ne distingue pas séquentielle et double embrayage.** `SUPPORTS_SHIFTER=0` dit seulement « pas de grille en H ». Ce sont les tags qui disent séquentielle ou semi-automatique ; la physique dit s'il y a un levier et combien de rapports.

## 2.3 Ce que dit le `ui_car.json`

Même chez Kunos, les specs sont des chaînes libres avec des tirets de remplissage (`"--km/h"`, `"--s 0-100"`, `"500+Nm"`), et `range` est un entier sans unité. **Le poids ne se tire pas de la physique** : `TOTALMASS` moins le pilote ne vaut le poids annoncé que sur une voiture sur trois.

---

# 3. Règles fondatrices

## R1 — La fiche est en base

Chaque valeur de la fiche est écrite en base **à l'import** (et à la réindexation, à la réharmonisation, et par un passage de rattrapage pour l'existant, §9.3). L'affichage ne lit plus aucun fichier. C'est ce qui rend la fiche entière pour un mod en vitrine (`ESPACE§`) et dans un export (`EXPORT§`), sans rien de particulier pour eux.

## R2 — Une valeur, une source, et un ordre fixe

Pour chaque champ, plusieurs sources peuvent répondre. On affiche la première qui répond, **toujours dans cet ordre** :

**modifié par vous › physique du mod › `ui_car.json` › déduit des tags**

Un champ n'a pas forcément toutes les sources (§4 dit lesquelles pour chacun). L'ordre ne change jamais d'un champ à l'autre, sauf pour ceux que la physique ne connaît pas.

## R3 — La physique avant les tags

Quand le mod le dit dans sa physique, on le lit ; on ne le devine plus. Les tags restent la source des champs que la physique ignore (architecture moteur, position, séquentielle ou double embrayage) et le repli quand la physique est illisible.

## R4 — Rien ne s'invente

`--`, une chaîne vide, un zéro de remplissage : **absent**. Un champ absent disparaît de l'écran, un groupe vide aussi. On n'écrit jamais « — » dans une case. Une unité qu'on ne connaît pas ne s'affiche pas.

## R5 — La provenance se voit sans survol

**Aucune couleur ne porte la provenance** : les valeurs ont toutes la même couleur. Ce qui vient des fichiers du mod n'a pas de signe. Ce qui est **déduit des tags** est précédé de **« ≈ »**, en gris : le signe dit « à peu près », ce qui est exactement ce qu'est une valeur devinée. Ce que **l'utilisateur a modifié** est suivi de **« ✎ »**, en gris. Chaque signe dit ce qu'il veut dire **au survol** (« déduit des tags », « modifié par vous ») ; il n'y a pas de légende en pied de bloc, qui coûtait une ligne sur chaque fiche. Le signe, lui, reste visible sans survol. En mode édition, chaque champ dit sa source en toutes lettres.

Les colonnes de la bibliothèque qui affichent ces mêmes champs portent le même signe, avec le même sens au survol : une valeur déduite se reconnaît partout de la même façon.

## R6 — Une saisie survit à tout

Une valeur modifiée par l'utilisateur vit dans une table à elle (§6), que rien ne recalcule : ni une mise à jour du mod, ni une réimportation, ni une réharmonisation, ni une mise en vitrine. Seul « revenir à la valeur du mod » l'efface.

---

# 4. Les champs

| Groupe | Champ | Sources, dans l'ordre (R2) | Affichage |
|---|---|---|---|
| Chiffres clés | Puissance | vous · `ui` `specs.bhp` | `470 bhp` |
| | Couple | vous · `ui` `specs.torque` | `520 Nm` |
| | Poids | vous · `ui` `specs.weight` (pas de repli sur la physique, §2.3) | `1 245 kg` |
| | Rapport P/P | vous · `ui` `specs.pwratio` · calculé poids ÷ puissance si les deux sont connus | `2,65 kg/hp` |
| | Vitesse max | vous · `ui` `specs.topspeed` | `270+ km/h` (le `+` de l'auteur est gardé) |
| | 0-100 | vous · `ui` `specs.acceleration` | `2,8 s` |
| Moteur | Architecture | vous · tags (`engine_config`) | `V8`, `6 à plat`, `rotatif`, `électrique` |
| | Suralimentation | vous · physique (0 section `[TURBO_n]` : atmosphérique ; 1 : turbo ; 2 ou plus : biturbo, §10) · tags (« compresseur » passe devant une physique sans turbo — un compresseur n'est pas un `[TURBO_n]` ; un tag « turbo » que la physique dément perd) | `biturbo` |
| | Position | vous · tags (`engine_pos`) | `central` |
| | Régime maximal | vous · physique `LIMITER` | `8 300 tr/min` |
| Transmission | Roues motrices | vous · physique (`TYPE`, plus `ers.ini [FRONT_MOTORS]` qui rend intégrale une propulsion) · tags | `propulsion`, `traction`, `intégrale` |
| | Boîte | vous · tags pour séquentielle, double embrayage, automatique · physique pour manuelle (`SUPPORTS_SHIFTER=1`) | `séquentielle` |
| | Rapports | vous · physique `GEARS COUNT` | `6 rapports` |
| Carburant | Réservoir | vous · physique `MAX_FUEL` | `110 L` |
| | Autonomie | vous · `ui` `specs.range` | **en km** (à l'allure de course, §10) |
| Origine | Pays | vous · harmonisation actuelle (`ui_car.json`, puis alias et extraction de tags, `SPEC.md`) | drapeau, nom |
| | Année | vous · `ui_car.json` · table du contenu d'origine | `2011` |
| Électronique | §5 | | puces |

**La boîte suit l'ordre de R2 avec une nuance** : la physique ne sait dire que « manuelle » ou « pas de grille » (§2.2). Quand elle dit « pas de grille » et que les tags ne disent rien, on écrit **« à palettes »**, ce qui est exact dans les deux cas qu'elle ne sait pas distinguer.

**Les colonnes de la bibliothèque et les filtres** (transmission, aspiration, boîte…) lisent des colonnes de `mods` qui sont **un cache de la valeur affichée** (§6.3) : la bibliothèque filtre sur la valeur que la fiche montre, saisie de l'utilisateur comprise.

**Formatage.** Nombres extraits des chaînes de l'auteur (`470bhp`, `500+Nm`, `2.65kg/hp`), séparateurs de la langue (`1 245`, `2,65`), unités en petit derrière. Une chaîne qu'on ne sait pas lire comme un nombre est affichée telle quelle plutôt que perdue.

---

# 5. L'électronique

Des **puces**, chacune dans un de trois états :

| État | Affichage |
|---|---|
| présente | puce pleine |
| absente | puce en pointillés, nom barré |
| inconnue | **pas de puce** |

| Aide | Présente si | Affichée absente ? |
|---|---|---|
| ABS | `electronics.ini [ABS] PRESENT` ≥ 1 | **oui** : « pas d'ABS » sur une voiture de 1966 est une information |
| Antipatinage | `[TRACTION_CONTROL] PRESENT` ≥ 1 (la valeur `2` existe) | **oui** |
| Blocage électronique (EDL) | `[EDL] PRESENT` = 1 | non |
| DRS | `drs.ini` contient une section `[WING_n]` | non : « pas de DRS » sur une citadine est du bruit |
| KERS | `kers.ini` contient `[KERS]` | non |
| ERS | `ers.ini` contient `[KINETIC]` ; précision « MGU-K + MGU-H » si `[HEAT]` | non |
| Roues arrière directrices | `ctrl_4ws.ini` | non |
| Anti-roulis actif | `ctrl_arb_front.ini` ou `ctrl_arb_rear.ini` | non |
| Répartiteur de freinage électronique | `ctrl_ebb.ini` | non |

C'est le seul endroit où la fiche dépend du type de voiture, et elle en dépend par une règle **par aide**, pas par catégorie : une fiche par type (route, GT, formule) vieillirait à chaque mod qui ne rentre dans aucune case.

L'écran de session (`SESSION§3`) lit ABS et antipatinage dans la même valeur effective : une correction y vaut aussi.

---

# 6. Le stockage

## 6.1 Deux tables

`tech_facts` garde ce que les sources disent (par version, par champ, par source : `physics`, `ui`, `rules`), recalculé et jamais édité ; `tech_user` garde ce que l'utilisateur a décidé (par mod et par champ), jamais recalculé (R6).

- **Les faits sont par version**, parce qu'une mise à jour du mod peut changer sa physique. **Les saisies sont par mod** : ce que l'utilisateur a corrigé vaut pour la voiture, pas pour une archive.
- **Toutes les sources sont gardées**, pas seulement la gagnante. C'est ce qui permet au mode édition de dire « la physique dit 6, les tags ne disent rien » et à « revenir à la valeur du mod » de savoir où revenir.
- **`NULL` dans `tech_user`** force l'état inconnu : utile pour une aide que la physique déclare à tort.
- `tech_user` n'a pas de clé étrangère vers `mods`, **délibérément**, comme `wiki_link` : une saisie doit survivre à une suppression complète suivie d'un réimport. La suppression complète l'efface quand même (ESPACE§5.3), mais c'est un geste explicite, pas une cascade.

## 6.2 Qui écrit

| Moment | Écrit |
|---|---|
| Import d'une version, réindexation du contenu d'origine | `physics` et `ui` pour cette version |
| Réharmonisation (règles changées) | `rules`, pour toutes les versions |
| Passage de rattrapage (§9.3) | les trois, pour ce qui n'en a pas |
| Édition (§8) | `tech_user` |

## 6.3 La valeur effective

Une seule fonction, `techsheet::effective(mod_id)`, applique R2 et rend la fiche entière, avec la source de chaque valeur. **Tout le monde la lit** : la fiche, l'écran de session (ABS et antipatinage), et le cache des colonnes de `mods` (`drivetrain`, `aspiration`, `gearbox`, `engine_config`, `engine_pos`), réécrit après chaque écriture de §6.2. Les filtres et colonnes de la bibliothèque n'ont rien à changer.

---

# 7. L'affichage

## 7.1 La page de la voiture

L'aperçu à gauche ; à sa droite, la fiche technique **et la courbe à côté d'elle** (dessous quand la place manque) ; sous le tout, la description puis Wikipédia :

1. **En-tête** : nom, marque, année, badges de classe et de catégorie. En dessous, une ligne **d'usage** : odomètre et « déjà essayée ». L'odomètre sort de la fiche technique : c'est l'usage de l'utilisateur, pas une caractéristique de la voiture.
2. **À droite de l'aperçu**, la fiche technique, de haut en bas :
   - **les chiffres clés** (puissance, couple, poids, rapport P/P, vitesse max, 0-100), grands chiffres et unités en petit, les absents disparaissant ;
   - **Mécanique** : moteur, transmission, carburant, origine, en phrases courtes (« atmosphérique · 8 300 tr/min », « propulsion · ≈ séquentielle · 6 rapports ») ;
   - **Électronique** : les puces de §5.
3. **À droite de la fiche**, la courbe.
4. **Description, puis Wikipédia.** Le nombre de lancements n'est pas affiché.

## 7.2 La frontière avec Wikipédia

**La fiche technique décrit le mod** : ce qu'on conduit dans le jeu. **Wikipédia décrit la voiture réelle** : dimensions, designer, production. Les deux divergent souvent, un mod pouvant être plus puissant ou plus léger que l'original. Aucun champ de Wikipédia n'entre dans la fiche. Une comparaison « dans le jeu / en réalité » sur deux ou trois chiffres est une idée pour plus tard (§11).

## 7.3 Mod en vitrine, export

Rien de particulier : la fiche est en base (R1). Un mod en vitrine montre la même fiche, courbe comprise **si elle est stockée** (§9.1). C'est ce que la maquette montre avec sa bascule.

---

# 8. L'édition

- **« ✎ Modifier »** dans l'en-tête du bloc, joignable à la manette comme tout bouton. Pas d'édition cellule par cellule au clic : une fiche où chaque valeur est un champ se lit mal et se manipule mal à la croix.
- **En mode édition**, chaque champ montre sa valeur effective et, dessous, **sa source en toutes lettres** (« lu dans la physique du mod », « déduit des tags », « modifié par vous », « absent du fichier »). Un champ modifié porte **« ↺ revenir »**, qui supprime la ligne de `tech_user`.
- **Types de champ** : nombres avec unité fixe pour les chiffres clés, le régime, le réservoir, les rapports, l'année ; **listes fermées** pour l'architecture, la suralimentation (atmosphérique, turbo, biturbo, compresseur), la position, les roues motrices, la boîte (mêmes valeurs que les filtres), et **le pays** (la liste des pays du jeu, avec leur drapeau, celle de l'onglet Pays de Classement) ; **puce à trois états** pour chaque aide (un clic : présente → absente → inconnue).
- **Le pays saisi passe devant toute l'harmonisation** (`harmonize::store`) : choisi dans la liste des pays du jeu, il est déjà normalisé, et la réharmonisation qui suit un changement de règles ne le touche pas. L'index par pays et le filtre pays suivent la saisie.
- **Enregistrer / Annuler** en pied de bloc. Enregistrer écrit `tech_user`, recalcule la valeur effective et le cache des colonnes (§6.3).
- **L'historique du mod** note « Fiche technique modifiée (boîte, rapports) ». Pas la valeur : l'historique raconte, il ne sauvegarde pas.

---

# 9. Implémentation

## 9.1 Backend

**La courbe de puissance et de couple** entre aussi en base (`field = 'power_curve'`, `'torque_curve'`, source `ui`) : c'est ce qui la garde pour un mod en vitrine et dans un export, où le `ui_car.json` n'est plus lu (R1).

**Les couches** (`SPEC.md` §4.3) : les fichiers d'une voiture se lisent à travers la même pile que celle que `compose` pose dans le jeu — couches actives par priorité, puis la base —, fichier par fichier : un `data/<fichier>` du dossier le plus haut qui en a un, sinon l'entrée du `data.acd` le plus haut ; le `ui_car.json` entier du dossier le plus haut. `compose::recompose`, par où passent tout changement de couche et tout changement de version active, relit la fiche (`techsheet::refresh_active`) ; au changement de version, les tags de la nouvelle repassent aux règles (`harmonize::harmonize_mod`).

**Le marqueur de lecture** (`_recorded`) dit ce qui a lu une version et à travers quoi : `{"reader": N, "stack": ["<couche>@<date d'import>", …]}`. Une relecture n'a lieu que si l'un des deux a changé — une simple activation ne relit rien —, et une couche regarnie en place, qui garde son identifiant, change de date. Jamais de chemin : la table part dans un export (`EXPORT§4.1`).

**`READER_VERSION` s'incrémente à chaque correction d'un lecteur** (`physics.rs`, `ui.rs`) : le rattrapage du démarrage ne relit que ce qui n'a pas été lu par les lecteurs du moment. Sans cela, une correction n'atteint que les imports suivants — piège payé une fois : les deux RUF RT12R, `1,495kg`, restaient à 1,5 kg.

**`electronics.rs`** demande `effective`, il ne lit pas de fichier. **`steering.rs`** (braquage de l'aperçu 3D) garde sa lecture : l'aperçu 3D n'existe que pour un mod complet.

## 9.2 Frontend

`TechSheet.svelte` et `CarSpecsBlock.svelte` (§7.1), le mode édition (§8), et le sens des signes de R5 repris des clés de provenance du mode édition (`techsheet.source.*`).

## 9.3 Remplissage de l'existant

Au démarrage, en fond : pour chaque version et chaque voiture d'origine sans ligne dans `tech_facts`, lire la physique et le `ui_car.json`, écrire les faits, recalculer la valeur effective. Idempotent (plus rien à faire au démarrage suivant). Le déchiffrement d'un `data.acd` coûte quelques millisecondes : quelques secondes pour une bibliothèque de 300 voitures.

**Ce qui bouge se dit** : une relecture peut faire **changer de transmission ou d'aspiration** des voitures dans les filtres. Il se dit en nombre : **une notification à part**, « Fiches techniques relues », donne par colonne de la bibliothèque combien de voitures ont gagné, changé ou perdu une valeur (`techsheet-report.json`, gardé jusqu'à ce qu'on le ferme). Pas dans le rapport de mise à jour des règles (`REGLES§`) : aucune règle n'a changé.

## 9.4 Tests

Les tests de `techsheet` portent les cas : les pièges de la physique (§2.2 : hybride avant électrique → intégrale, `drs.ini` vide, `TRACTION_CONTROL=2`, deux `[TURBO_n]` → biturbo), les chaînes du `ui_car.json` (`--s 0-100` absent, `500+Nm`, chaîne illisible gardée), l'ordre des sources, la survie d'une saisie (mise à jour, réharmonisation, vitrine, export — ces deux derniers dans `ESPACE§9.4` et `EXPORT§8.4`), le cache des filtres, le pays saisi, la fiche identique d'un mod en vitrine, et les couches (lue devant la base, relue regarnie, jamais deux fois pour la même pile).

---

# 10. Mesuré à l'implémentation

**Comment mesurer** : `techsheet::measure` (tests ignorés par défaut, rejouables) parcourt toute la bibliothèque et le contenu d'origine avec les lecteurs de l'app et écrit les décomptes ; il ressert à chaque règle de lecture ajoutée.

**Ce qui a été tranché par la mesure** (397 voitures, 2026-09-27) :

- **la physique a raison contre les tags** sur la transmission (18 désaccords, tous de son côté), hybrides à moteurs avant compris ;
- **pas de repli du poids sur la physique** (§2.3) ;
- **l'autonomie est en km**, à l'allure de course (1,3 à 2,9 km par litre de réservoir) ;
- **un compresseur n'est pas un `[TURBO_n]`**, donc les tags gagnent quand la physique ne voit pas de turbo ;
- **deux sections `[TURBO_n]` ou plus donnent « biturbo »** : presque toutes les voitures concernées sont de vrais biturbos, et l'auteur qui modélise un turbo à deux étages est le coût accepté.

---

# 11. Hors périmètre

- **La fiche d'un circuit** (`TrackInfoBlock.svelte`) : même principe possible (longueur, nombre de stands, largeur), à part.
- **Comparer à la voiture réelle** : afficher, à côté d'un chiffre du mod, celui de Wikipédia quand il existe.
- **Unités au choix** (ch, kW, lb-ft, mph) : une préférence, plus tard ; la base garde l'unité de la source.
- **Éditer la courbe de puissance.**
