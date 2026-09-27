# Pit Box — la fiche technique d'une voiture

## Spécification d'implémentation

*La fiche technique passe en base, avec une source par valeur ; la physique du mod remplace les devinettes quand elle sait répondre ; l'électronique embarquée apparaît ; l'utilisateur peut tout corriger ; et la fiche reste entière pour un mod en vitrine et dans un export.*

> **Étiquette de renvoi : `FICHE§`.** Une maquette accompagne cette spec : `maquettes/pitbox-fiche-technique.html` (trois voitures d'origine, bascule vitrine, mode édition). Elle juge la disposition et les gestes, pas l'UI : couleurs et composants viennent de l'app. En cas d'écart, la spec gagne.
>
> **Mesures du 2026-09-27** sur l'install de référence : `overlay.sqlite` (324 voitures dont 178 d'origine), et la physique déchiffrée de **104 voitures** (100 d'origine, 4 mods). Les autres mods n'ont pas pu être lus par l'outil de mesure (fichiers en hardlink), **pas** par Pit Box : la mesure est à rejouer sur eux à l'implémentation (§10).
>
> **Livrée le 2026-09-27** (`src-tauri/src/techsheet/`). La mesure de §10 a été refaite sur 397 voitures et ses résultats y sont consignés. Les tests de §9.4 qui supposent la vitrine et l'export sont portés par ces chantiers-là (`ESPACE§9.4`, `EXPORT§8.4`), qui ne sont pas encore construits.

---

# 1. Objet

La fiche technique d'aujourd'hui (`TechSheet.svelte`) mélange trois sources sans le dire, et n'en garde aucune :

| Cellules | Source | En base |
|---|---|---|
| Puissance, couple, poids, vitesse max, 0-100, rapport P/P, autonomie | le `ui_car.json`, **relu à chaque affichage**, tel quel | non |
| Transmission, aspiration, moteur, position, boîte (en vert) | **devinées à partir des tags** par les règles | oui, mais **réécrites** à chaque réharmonisation |
| Odomètre | journal de Content Manager | non |

D'où quatre défauts :

1. **Rien n'est corrigeable.** Une valeur fausse ne peut être corrigée qu'en écrivant une règle ; la corriger en base serait écrasé à la prochaine réharmonisation.
2. **On devine ce que le mod dit.** La transmission, le turbo, le nombre de rapports sont écrits dans la physique du mod ; on les tire de tags approximatifs.
3. **Les valeurs sont brutes.** `--s 0-100` est un tiret de remplissage de l'auteur, affiché tel quel ; « 182 » n'a pas d'unité.
4. **Le vert ne s'explique qu'au survol**, invisible à la manette ; et une couleur seule pour porter un sens se lit mal (et n'est pas jolie, de l'avis de l'utilisateur).

Et il manque ce que l'utilisateur veut voir en premier sur une voiture de course moderne : **son électronique** (ABS, antipatinage, DRS, ERS…).

---

# 2. Ce que la mesure a montré

## 2.1 Ce que les règles remplissent aujourd'hui

Sur les 324 voitures de la base :

| Champ | Rempli | |
|---|---|---|
| transmission | 290 | |
| boîte | 253 | |
| aspiration | 103 | |
| architecture moteur | **9** | à peu près jamais : aucun tag courant ne la porte |
| position moteur | **3** | idem |

## 2.2 Ce que la physique dit, sur 104 voitures

| Donnée | Où | Présente | Accord avec les règles |
|---|---|---|---|
| Roues motrices | `drivetrain.ini [TRACTION] TYPE` : `RWD`, `FWD`, `AWD`, `AWD2` | 104 / 104 | 97 d'accord, 2 que les règles laissaient vides, **5 en désaccord** : voir ci-dessous |
| Nombre de rapports | `drivetrain.ini [GEARS] COUNT` | 104 / 104 | les règles ne le donnent pas |
| Boîte en H | `drivetrain.ini [GEARBOX] SUPPORTS_SHIFTER` | 104 / 104 | `1` ↔ « manuelle » 47 fois sur 49 ; `0` ↔ « séquentielle » ou « semi-auto » 49 fois sur 51 |
| Suralimentation | `engine.ini`, sections `[TURBO_n]` | 104 / 104 (0, 1 ou 2 turbos) | 35 d'accord, **15 turbos que les règles ne voyaient pas**, aucun faux positif |
| Régime maximal | `engine.ini [ENGINE_DATA] LIMITER` | 104 / 104 | — |
| Réservoir | `car.ini [FUEL] MAX_FUEL` | 104 / 104 | — |
| ABS | `electronics.ini [ABS] PRESENT` | 94 / 104 | — |
| Antipatinage | `electronics.ini [TRACTION_CONTROL] PRESENT` | 102 / 104 (une valeur `2`) | — |
| Blocage électronique (EDL) | `electronics.ini [EDL] PRESENT` | 46 / 104 | — |
| DRS | `drs.ini` **avec une section `[WING_n]`** | 7 / 104 | — |
| KERS | `kers.ini`, section `[KERS]` | 3 / 104 | — |
| ERS | `ers.ini`, section `[KINETIC]` ; `[HEAT]` = MGU-H ; `[FRONT_MOTORS]` = moteurs électriques avant | 10 / 104 | — |
| Aides actives | `ctrl_4ws.ini` (roues arrière directrices), `ctrl_arb_*.ini` (anti-roulis actif), `ctrl_ebb.ini` (répartiteur de freinage électronique) | 5, 5 et 13 | — |

Trois pièges que seule la mesure montrait :

- **Le fichier `drs.ini` ne veut pas dire DRS.** Il existe sur 66 voitures, vide sur 59 d'entre elles (une Abarth 500, un Macan). Seule une section `[WING_n]` désigne un aileron piloté. Le mod RSS Formula 2013 et la McLaren P1 de route l'ont ; la Macan ne l'a pas.
- **La traction ne se lit pas en un seul champ.** Les 5 désaccords sont les hybrides à moteurs électriques avant (919 Hybrid, TS040, R18 e-tron, 918 Spyder) : `TYPE=RWD` pour le moteur thermique, et `ers.ini [FRONT_MOTORS]` pour l'essieu avant électrique. **Ce sont les règles qui avaient raison** : ce sont des voitures à quatre roues motrices. La physique doit lire les deux fichiers ensemble.
- **La physique ne distingue pas séquentielle et double embrayage.** `SUPPORTS_SHIFTER=0` dit seulement « pas de grille en H ». Ce sont les tags qui disent séquentielle ou semi-automatique ; la physique dit s'il y a un levier et combien de rapports.

## 2.3 Ce que dit le `ui_car.json`

Chez Kunos même, les specs sont des chaînes libres avec des tirets de remplissage : `"topspeed": "--km/h"`, `"acceleration": "--s 0-100"`, `"torque": "500+Nm"`. `range` est un entier **sans unité**. La F2004 n'a que sa puissance.

Le poids du `ui_car.json` vaut `TOTALMASS` de `car.ini` moins 75 kg sur les deux voitures vérifiées (458 GT2 : 1 320 → 1 245 ; P1 : 1 525 → 1 450), c'est-à-dire la masse sans pilote. **À vérifier sur l'échantillon entier** avant de s'en servir de repli (§4).

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

**Aucune couleur ne porte la provenance** : les valeurs ont toutes la même couleur. Ce qui vient des fichiers du mod n'a pas de signe. Ce qui est **déduit des tags** est précédé de **« ≈ »**, en gris : le signe dit « à peu près », ce qui est exactement ce qu'est une valeur devinée. Ce que **l'utilisateur a modifié** est suivi de **« ✎ »**, en gris. Une **légende** en pied de bloc, affichée seulement si l'un des signes est présent, dit ce qu'ils veulent dire (« ≈ déduit des tags · ✎ modifié par vous »). En mode édition, chaque champ dit sa source en toutes lettres.

La couleur verte des valeurs déduites et son infobulle (`modpanel.derivedTooltip`) disparaissent. Les colonnes de la bibliothèque qui affichent ces mêmes champs prennent le même signe, pour qu'une valeur déduite se reconnaisse partout de la même façon.

## R6 — Une saisie survit à tout

Une valeur modifiée par l'utilisateur vit dans une table à elle (§6), que rien ne recalcule : ni une mise à jour du mod, ni une réimportation, ni une réharmonisation, ni une mise en vitrine. Seul « revenir à la valeur du mod » l'efface.

---

# 4. Les champs

| Groupe | Champ | Sources, dans l'ordre (R2) | Affichage |
|---|---|---|---|
| Chiffres clés | Puissance | vous · `ui` `specs.bhp` | `470 bhp` |
| | Couple | vous · `ui` `specs.torque` | `520 Nm` |
| | Poids | vous · `ui` `specs.weight` · *physique `TOTALMASS` − 75, si §2.3 se confirme* | `1 245 kg` |
| | Rapport P/P | vous · `ui` `specs.pwratio` · calculé poids ÷ puissance si les deux sont connus | `2,65 kg/hp` |
| | Vitesse max | vous · `ui` `specs.topspeed` | `270+ km/h` (le `+` de l'auteur est gardé) |
| | 0-100 | vous · `ui` `specs.acceleration` | `2,8 s` |
| Moteur | Architecture | vous · tags (`engine_config`) | `V8`, `6 à plat`, `rotatif`, `électrique` |
| | Suralimentation | vous · physique (0 section `[TURBO_n]` : atmosphérique ; 1 : turbo ; 2 ou plus : biturbo, §10) · tags (`aspiration` ; « compresseur » passe devant une physique sans turbo) | `biturbo` |
| | Position | vous · tags (`engine_pos`) | `central` |
| | Régime maximal | vous · physique `LIMITER` | `8 300 tr/min` |
| Transmission | Roues motrices | vous · physique (`TYPE`, plus `ers.ini [FRONT_MOTORS]` qui rend intégrale une propulsion) · tags | `propulsion`, `traction`, `intégrale` |
| | Boîte | vous · tags pour séquentielle, double embrayage, automatique · physique pour manuelle (`SUPPORTS_SHIFTER=1`) | `séquentielle` |
| | Rapports | vous · physique `GEARS COUNT` | `6 rapports` |
| Carburant | Réservoir | vous · physique `MAX_FUEL` | `110 L` |
| | Autonomie | vous · `ui` `specs.range` | **unité à confirmer** (km, comme Content Manager l'affiche ?) avant de l'écrire ; d'ici là, le nombre seul |
| Origine | Pays | vous · harmonisation actuelle (`ui_car.json`, puis alias et extraction de tags, `SPEC.md`) | drapeau, nom |
| | Année | vous · `ui_car.json` · table du contenu d'origine | `2011` |
| Électronique | §5 | | puces |

**La boîte suit l'ordre de R2 avec une nuance** : la physique ne sait dire que « manuelle » ou « pas de grille » (§2.2). Quand elle dit « pas de grille » et que les tags ne disent rien, on écrit **« à palettes »**, ce qui est exact dans les deux cas qu'elle ne sait pas distinguer.

**Les colonnes de la bibliothèque et les filtres** (transmission, aspiration, boîte…) lisent aujourd'hui les colonnes de `mods` remplies par les règles. Ces colonnes deviennent **un cache de la valeur affichée** (§6.3) : la bibliothèque filtre sur la même valeur que celle que la fiche montre, saisie de l'utilisateur comprise.

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

`electronics.rs` lit déjà ABS et antipatinage pour l'écran de session (`SESSION§3`) : il lit désormais ce qui est en base, et la lecture du fichier rejoint le module de la fiche (§9.1).

---

# 6. Le stockage

## 6.1 Deux tables

```sql
-- Ce que les sources disent. Recalculé, jamais édité à la main.
CREATE TABLE tech_facts (
  mod_id     TEXT NOT NULL,
  version_id TEXT NOT NULL DEFAULT '',  -- '' pour le contenu d'origine, sans version
  field      TEXT NOT NULL,             -- 'power', 'drivetrain', 'aid.drs'…
  source     TEXT NOT NULL,             -- 'physics' | 'ui' | 'rules'
  value      TEXT NOT NULL,             -- JSON : nombre, texte, booléen
  PRIMARY KEY (mod_id, version_id, field, source)
);

-- Ce que l'utilisateur a décidé. Jamais recalculé (R6).
CREATE TABLE tech_user (
  mod_id    TEXT NOT NULL,
  field     TEXT NOT NULL,
  value     TEXT,                       -- JSON ; NULL = « forcé inconnu »
  edited_at TEXT NOT NULL,
  PRIMARY KEY (mod_id, field)
);
```

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

**La structure de la page ne change pas.** L'aperçu à gauche ; à sa droite, la fiche technique **et la courbe à côté d'elle**, comme aujourd'hui (`CarSpecsBlock.svelte`, qui les pose côte à côte et passe la courbe dessous quand la place manque) ; et sous le tout, la description puis Wikipédia, qui se lisent en article en faisant défiler la page. Seul le contenu de la fiche change :

1. **En-tête** : nom, marque, année, badges de classe et de catégorie. En dessous, une ligne **d'usage** : odomètre et « déjà essayée ». L'odomètre sort de la fiche technique : c'est l'usage de l'utilisateur, pas une caractéristique de la voiture.
2. **À droite de l'aperçu**, la fiche technique, de haut en bas :
   - **les chiffres clés** (puissance, couple, poids, rapport P/P, vitesse max, 0-100), grands chiffres et unités en petit, les absents disparaissant ;
   - **Mécanique** : moteur, transmission, carburant, origine, en phrases courtes (« atmosphérique · 8 300 tr/min », « propulsion · ≈ séquentielle · 6 rapports ») ;
   - **Électronique** : les puces de §5 ;
   - la légende de R5, si un signe est présent.
3. **À droite de la fiche**, la courbe, inchangée.
4. **Description, puis Wikipédia**, inchangées.

## 7.2 La frontière avec Wikipédia

**La fiche technique décrit le mod** : ce qu'on conduit dans le jeu. **Wikipédia décrit la voiture réelle** : dimensions, designer, production. Les deux divergent souvent, un mod pouvant être plus puissant ou plus léger que l'original. Aucun champ de Wikipédia n'entre dans la fiche. Une comparaison « dans le jeu / en réalité » sur deux ou trois chiffres est une idée pour plus tard (§11).

## 7.3 Mod en vitrine, export

Rien de particulier : la fiche est en base (R1). Un mod en vitrine montre la même fiche, courbe comprise **si elle est stockée** (§9.1). C'est ce que la maquette montre avec sa bascule.

---

# 8. L'édition

- **« ✎ Modifier »** dans l'en-tête du bloc, joignable à la manette comme tout bouton. Pas d'édition cellule par cellule au clic : une fiche où chaque valeur est un champ se lit mal et se manipule mal à la croix.
- **En mode édition**, chaque champ montre sa valeur effective et, dessous, **sa source en toutes lettres** (« lu dans la physique du mod », « déduit des tags », « modifié par vous », « absent du fichier »). Un champ modifié porte **« ↺ revenir »**, qui supprime la ligne de `tech_user`.
- **Types de champ** : nombres avec unité fixe pour les chiffres clés, le régime, le réservoir, les rapports, l'année ; **listes fermées** pour l'architecture, la suralimentation (atmosphérique, turbo, biturbo, compresseur), la position, les roues motrices, la boîte (mêmes valeurs que les filtres), et **le pays** (la liste des pays du jeu, avec leur drapeau, celle de l'onglet Pays de Classement) ; **puce à trois états** pour chaque aide (un clic : présente → absente → inconnue).
- **Le pays saisi passe devant toute l'harmonisation.** Aujourd'hui le pays se décide à l'écriture (`harmonize::store` : fichier, alias, extraction de tag). Une saisie de l'utilisateur, choisie dans la liste des pays du jeu, n'a pas besoin des alias : elle est déjà normalisée. `harmonize::store` doit donc la respecter, comme la valeur effective de §6.3 : la réharmonisation qui suit un changement de règles ne la touche pas. L'index par pays et le filtre pays suivent la saisie.
- **Enregistrer / Annuler** en pied de bloc. Enregistrer écrit `tech_user`, recalcule la valeur effective et le cache des colonnes (§6.3).
- **L'historique du mod** note « Fiche technique modifiée (boîte, rapports) ». Pas la valeur : l'historique raconte, il ne sauvegarde pas.

---

# 9. Implémentation

## 9.1 Backend

Un module **`techsheet.rs`** :

| Fonction | Rôle |
|---|---|
| `read_physics(car_dir, car_id) -> Vec<Fact>` | §2.2 et §5, par `acd.rs` pour un `data.acd`, ou le dossier `data/` s'il est à nu (même règle que `driver::data_file`) |
| `read_ui(ui_car) -> Vec<Fact>` | §4, avec l'analyse des chaînes de l'auteur (R4) |
| `rules_facts(mod) -> Vec<Fact>` | ce que l'harmonisation déduit, sans rien réécrire d'autre |
| `store(conn, mod_id, version_id, facts)` | §6.2 |
| `effective(conn, mod_id) -> TechSheet` | §6.3, seule lecture |
| `set_user(conn, mod_id, field, value)`, `clear_user(conn, mod_id, field)` | §8 |

**La courbe de puissance et de couple** entre aussi en base (`field = 'power_curve'`, `'torque_curve'`, source `ui`) : c'est ce qui la garde pour un mod en vitrine et dans un export, où le `ui_car.json` n'est plus lu (R1).

**Les couches** (`SPEC.md` §4.3) : les fichiers d'une voiture se lisent à travers la même pile que celle que `compose` pose dans le jeu — couches actives par priorité, puis la base —, fichier par fichier : un `data/<fichier>` du dossier le plus haut qui en a un, sinon l'entrée du `data.acd` le plus haut ; le `ui_car.json` entier du dossier le plus haut. `compose::recompose`, par où passent tout changement de couche et tout changement de version active, relit la fiche (`techsheet::refresh_active`) ; au changement de version, les tags de la nouvelle repassent aux règles (`harmonize::harmonize_mod`).

**Le marqueur de lecture** (`_recorded`) dit ce qui a lu une version et à travers quoi : `{"reader": N, "stack": ["<couche>@<date d'import>", …]}`. Une relecture n'a lieu que si l'un des deux a changé — une simple activation ne relit rien —, et une couche regarnie en place, qui garde son identifiant, change de date. Jamais de chemin : la table part dans un export (`EXPORT§4.1`).

**`READER_VERSION` s'incrémente à chaque correction d'un lecteur** (`physics.rs`, `ui.rs`) : le rattrapage du démarrage ne relit que ce qui n'a pas été lu par les lecteurs du moment. Sans cela, une correction n'atteint que les imports suivants — piège payé une fois : les deux RUF RT12R, `1,495kg`, restaient à 1,5 kg.

**`electronics.rs`** ne lit plus le fichier : il demande `effective`. **`steering.rs`** (braquage, pour l'aperçu 3D) peut garder sa lecture, l'aperçu 3D n'existant que pour un mod complet ; la spec vitrine (`ESPACE§3.4`) n'a donc plus à copier que la fiche, ce que R1 fait déjà.

## 9.2 Frontend

- `TechSheet.svelte` et `CarSpecsBlock.svelte` refaits selon §7.1 : bande de chiffres, bloc Mécanique et Électronique, courbe.
- L'odomètre rejoint l'en-tête avec « déjà essayée » (`odometer.ts` inchangé). Le nombre de lancements n'est pas affiché.
- Le mode édition (§8), avec les listes fermées des filtres et la liste des pays du jeu.
- La légende de R5, **une clé de langue par signe**.

## 9.3 Remplissage de l'existant

Au démarrage, en fond : pour chaque version et chaque voiture d'origine sans ligne dans `tech_facts`, lire la physique et le `ui_car.json`, écrire les faits, recalculer la valeur effective. Idempotent (plus rien à faire au démarrage suivant). Le déchiffrement d'un `data.acd` coûte quelques millisecondes : quelques secondes pour une bibliothèque de 300 voitures.

**Un changement à surveiller** : la valeur effective remplaçant les colonnes de `mods`, des voitures vont **changer de transmission ou d'aspiration** dans les filtres (les 15 turbos que les règles ne voyaient pas, par exemple). C'est l'objet même de la spec, et il se dit, en nombre : **une notification à part**, « Fiches techniques relues », donne par colonne de la bibliothèque combien de voitures ont gagné, changé ou perdu une valeur (`techsheet-report.json`, gardé jusqu'à ce qu'on le ferme). Pas dans le rapport de mise à jour des règles (`REGLES§`), comme d'abord prévu : aucune règle n'a changé, et une relecture après une correction de lecteur n'a rien à voir avec le catalogue. Mesuré au premier démarrage sur l'install de dev : 111 voitures gagnent « turbo », 176 « atmosphérique », 19 changent de transmission, 61 gagnent « manuelle » et 38 « à palettes ».

## 9.4 Tests

- **Physique** : sur des fichiers de fixture reprenant les cas de §2.2 : hybride `RWD` + `[FRONT_MOTORS]` → intégrale ; `drs.ini` vide → pas de DRS ; `drs.ini` avec `[WING_3]` → DRS ; `TRACTION_CONTROL PRESENT=2` → présent ; deux `[TURBO_n]` → biturbo.
- **`ui_car.json`** : `--s 0-100` → absent ; `500+Nm` → 500 avec « + » ; `range: 195` sans unité ; chaîne illisible gardée telle quelle.
- **Ordre des sources** : une saisie bat la physique, qui bat le `ui`, qui bat les tags ; `NULL` saisi force l'inconnu.
- **Survie** : une saisie survit à une mise à jour du mod, à une réharmonisation, à une mise en vitrine et à un export suivi d'un import (ces deux derniers : `ESPACE§9.4`, `EXPORT§8.4`).
- **Cache** : après une saisie « intégrale », le filtre transmission de la bibliothèque la trouve.
- **Pays** : un pays saisi survit à une réharmonisation et à un changement d'alias ; l'index par pays range la voiture sous lui.
- **Biturbo** : deux sections `[TURBO_n]` donnent « biturbo » sans autre signal, une seule « turbo » (§10).
- **Vitrine** : un mod en vitrine rend exactement la même fiche, courbe comprise, sans lire aucun fichier (`ESPACE§9.4`).
- **Couches** : une couche qui apporte un `engine.ini` est lue devant la base, puis plus du tout une fois éteinte ; une couche regarnie en place est relue ; la même pile n'est pas relue deux fois.

---

# 10. Mesuré à l'implémentation

**Qui mesure, et où.** La mesure de §2 a été faite de l'extérieur, et l'outil utilisé refuse les fichiers en hardlink : les mods gérés par Pit Box lui étaient illisibles. **Pit Box n'a pas cette limite.** La mesure se refait donc **au début de l'implémentation, sur la machine de développement, par le code** : une commande de développement (ou un test ignoré par défaut) qui parcourt toute la bibliothèque et le contenu d'origine, applique les lecteurs de `techsheet.rs`, et écrit les décomptes de §2.2 et §5 dans un fichier. Elle resservira à chaque règle de lecture ajoutée. Les règles de lecture ne se figent qu'après.

**Mesuré le 2026-09-27 sur 397 voitures** (217 mods, 180 d'origine) par `techsheet::measure` (tests ignorés par défaut, rejouables) :

- **Les mods.** Physique lisible sur 395 voitures (les deux autres : un dossier sans physique). 18 désaccords de transmission entre physique et tags, **tous du côté de la physique** à la vérification (Trans-Am VRC en propulsion taggées `awd`, Subaru 22B intégrale taggée `rwd`…) ; les hybrides à moteurs avant sont lus intégrales, comme §2.2 le demandait.
- **Le poids** tiré de `TOTALMASS` (§2.3) : `TOTALMASS − 75` ne vaut le poids du `ui_car.json` que sur 162 voitures sur 390 (0 sur 53, 80 sur 30, le reste épars). **Pas de repli** sur la physique pour le poids.
- **L'unité de l'autonomie** (§4) : 1,3 à 2,9 par litre de réservoir, moins sur la version plus puissante d'une même voiture (BMW 1M : 80, 1M Stage 3 : 69) — des km à l'allure de course. **Écrite en km.**
- **Un compresseur** n'apparaît **pas** comme un `[TURBO_n]` : les 16 voitures que les tags disent à compresseur n'en ont aucun. Les tags gagnent donc quand la physique dit « pas de turbo » ; un tag « turbo » que la physique dément (Cayman GT4, atmosphérique) perd.
- **Les « twin turbo ».** Deux sections `[TURBO_n]` ne veulent pas forcément dire deux turbos en parallèle : un auteur peut modéliser ainsi un turbo séquentiel ou deux étages. **Tranché par la mesure (2026-09-27, 397 voitures)** : 105 voitures ont deux sections ou plus, et presque toutes sont de vrais biturbos (F40, GT-R, M4, 488, McLaren V8, RX-7 FD à turbos séquentiels…) ; aucun tag ne dit « twin turbo » et seuls deux noms le disent, si bien qu'exiger un signal confirmant aurait écrit « turbo » sur toutes. **Deux sections ou plus donnent donc « biturbo »**, l'auteur qui modélise un turbo à deux étages étant le coût accepté.

---

# 11. Hors périmètre

- **La fiche d'un circuit** (`TrackInfoBlock.svelte`) : même principe possible (longueur, nombre de stands, largeur), à part.
- **Comparer à la voiture réelle** : afficher, à côté d'un chiffre du mod, celui de Wikipédia quand il existe.
- **Unités au choix** (ch, kW, lb-ft, mph) : une préférence, plus tard ; la base garde l'unité de la source.
- **Éditer la courbe de puissance.**
