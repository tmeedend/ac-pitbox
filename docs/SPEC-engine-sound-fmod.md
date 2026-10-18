# SPEC — Écouter le moteur avec le FMOD d'Assetto Corsa

Ce que fait l'écoute d'un moteur sur la fiche voiture, et les règles qu'il
faut connaître pour la modifier sans la casser. **Les écarts d'ABI de la DLL,
les mesures au banc et sur le corpus, et le journal des lots sont dans
`fmod-research.md`** ; le format des banks, dans `fsb5-format.md`. Ce que
l'utilisateur voit (clé de contact, curseur, coups d'accélérateur) est décrit
au §8.3 de `SPEC.md`.

---

## 1. Le problème que ça résout

Pour faire entendre un mod de son, un décodeur maison ouvre le `.bank` et
cherche « le ralenti » à jouer en boucle — et **chercher le ralenti dans le
signal n'a pas de solution** : 40 bons choix sur 91 banks Kunos, et rien ne
distingue un ralenti d'un bas régime en lâcher de gaz. L'information existe
pourtant dans le graphe d'événements du bank, et AC livre le moteur audio qui
sait le lire. **Pit Box joue donc l'événement moteur du jeu par les DLL FMOD de
l'installation de l'utilisateur** : 299 voitures sur 299 de l'installation de
référence.

---

## 2. Ce qui est vérifié

### 2.1 Les DLL

`<AC>/fmod64.dll` et `<AC>/fmodstudio64.dll`, **FMOD Studio 1.08.12**, 64 bits.
Les crates Rust FMOD visent la 2.x, dont l'API de paramètres a changé : la FFI
est écrite à la main (`fmod/sys.rs`).

### 2.2 Les fonctions nécessaires sont exportées

Cycle de vie du système, `Update`, chargement et déchargement d'un bank,
recherche d'un événement par GUID, création d'instance, énumération des
paramètres, réglage d'un paramètre, `Start`/`Stop`, chargement explicite des
échantillons et état de lecture. La liste exacte est dans `fmod/sys.rs`.

### 2.3 Un `GUIDs.txt` plutôt qu'une banque de chaînes

**Aucune voiture n'a de banque de chaînes** : un événement se trouve par son
GUID, lu dans un `GUIDs.txt` (une ligne par événement, GUID puis chemin). On le
cherche **à côté du bank** (un mod s'auditionne depuis la bibliothèque), puis
dans la table globale du jeu, `content/sfx/GUIDs.txt` — les événements d'un mod
de son ne sont que dans sa propre table. La comparaison des chemins
d'événement **ignore la casse**. Le GUID est analysé en Rust, sans DLL, pour
qu'un événement se résolve sur une machine sans jeu ; l'ordre des octets est
figé par un test (`guid_matches_what_fmod_parse_id_returned`).

### 2.4 Les paramètres

Un événement moteur est piloté par un paramètre de **régime** et un
d'**accélérateur**. **Les noms ne sont pas codés en dur** : on énumère les
paramètres, on écarte d'abord ceux qui ne sont pas `GAME_CONTROLLED` (FMOD
calcule lui-même `Distance` et `Event Cone Angle`), puis on reconnaît régime et
accélérateur par leur nom **et** leur plage. Un événement dont rien n'est
reconnu se joue quand même, à ses valeurs par défaut.

---

## 2bis. Ce que le lot 0 a établi

Les règles qui en découlent ; la démonstration de chacune est dans
`fmod-research.md`.

- `FMOD_STUDIO_INIT_ALLOW_MISSING_PLUGINS` est **obligatoire** : sans lui, aucun
  bank de voiture ne se charge. L'effet manquant (un filtre de distance que le
  jeu enregistre depuis son propre exécutable) change peu à quelques mètres.
- `common.bank` se charge **avant** le bank de la voiture : il porte les bus.
- `FMOD_STUDIO_PARAMETER_DESCRIPTION` a une disposition mesurée, différente de
  la plausible ; un test la fige
  (`parameter_description_keeps_its_measured_layout`).
- Le régime se règle **sur une instance qui joue**, et il pilote réellement la
  hauteur ; l'accélérateur bascule entre couches en charge et en lâcher.
- Les échantillons se chargent explicitement et l'on attend `LOADED` (qui vaut
  **3**), sans quoi les premières centaines de millisecondes sont muettes.
- Un paramètre automatique se lit par index, jamais par nom.
- **L'événement est orienté vers −Z**, la queue de la voiture : le zéro du cône
  d'AC est à l'arrière. L'auditeur regarde toujours la voiture : c'est le
  **timbre** qui change en tournant autour, pas la direction. L'angle envoyé
  est celui **dans le repère de la voiture** — l'azimut de la caméra moins la
  rotation du plateau.
- `LoadBankFile` prend de l'UTF-8 : les chemins accentués passent.

---

## 3. Licence FMOD — position prise

FMOD est propriétaire. Son contrat autorise l'usage **non commercial** sans
inscription ; Pit Box est gratuit et open source, donc dedans.

- **La mention exigée par la clause 3** — « Made with FMOD Studio by Firelight
  Technologies Pty Ltd. » — est dans l'écran À propos, **en dur et non
  traduite** : c'est un texte légal.
- **Rien n'est redistribué** : aucune DLL FMOD dans le dépôt ni dans
  l'installateur. **Une DLL FMOD n'est jamais copiée ailleurs, ni chargée
  depuis un autre endroit que l'installation AC configurée.** Le contrat est
  silencieux sur ce cas.
- Si Pit Box devenait monétisé, sponsorisé ou promotionnel, une licence FMOD
  deviendrait nécessaire.

---

## 4. Architecture

### 4.1 Deux chemins, et le repli n'est pas une consolation

| chemin | quand | ce qu'il donne |
| --- | --- | --- |
| **FMOD** | AC configuré, DLL chargeables, bank accepté | l'événement moteur réel, régime réglable |
| **décodeur maison** | sinon | un échantillon en boucle, ralenti deviné |

Le passage de l'un à l'autre est **silencieux** pour l'utilisateur (un
`log::warn!`, jamais un message) : toute erreur du chemin FMOD est un signal
de repli. Le décodeur maison reste ce qui alimente la fiche d'un mod de son et
le seul chemin sans jeu installé ; **son heuristique de ralenti ne doit pas
être retouchée** (`fsb5-format.md`).

### 4.2 Chargement des DLL

`fmod64.dll` **puis** `fmodstudio64.dll`, depuis le dossier du jeu, sans
toucher au `PATH` du processus. Rien n'est chargé au démarrage : une machine
sans AC ne paie rien, les DLL ne sont touchées qu'à la première écoute.

### 4.3 Un thread propriétaire

Un seul thread possède le système FMOD (`fmod/engine.rs`), appelle `Update`
toutes les 20 ms et reçoit des commandes ; **aucun autre thread n'y touche**, et
le compilateur le tient (le système n'est pas `Send`). Les commandes Tauri
d'écoute sont asynchrones : une commande synchrone gèlerait la fenêtre pendant
le chargement. Le chemin FMOD joue en natif ; le repli passe par Web Audio.

### 4.4 Ce que l'interface gagne

Sur le chemin FMOD, un **curseur de régime**, du ralenti au rupteur de la
voiture. Les deux bornes viennent de la physique de la voiture (`data.acd`
déchiffré, `acd.rs`) :

- **le rupteur** : `LIMITER` d'`engine.ini` ; à défaut le dernier point de la
  courbe de puissance de `ui_car.json` ; à défaut 8000 tr/min ;
- **le ralenti** : `MINIMUM` d'`engine.ini` — un `MINIMUM` négatif n'est cru en
  valeur absolue que s'il tombe entre 5 % et 35 % du rupteur ; à défaut le
  régime écrit dans le nom de l'échantillon de ralenti (un nombre déjà présent
  dans l'identifiant de la voiture est écarté) ; à défaut 0,13 × le rupteur,
  borné à [700, 4000].

Le bas de course est le ralenti exact, jamais en dessous.

---

## 5. Plan par lots

Les cinq lots (preuve hors application, liaisons, thread et commandes,
interface, relevé du corpus) sont faits ; leur journal est dans
`fmod-research.md`. Ce qu'ils ont fixé :

- **Un seul bank de voiture chargé à la fois** : deux mods de son d'une même
  voiture déclarent les mêmes GUID, et l'autre bank répondrait à leur place.
- Un mod qui livre son propre `common.bank`, ou un `.strings.bank`, voit ce
  fichier refusé comme bank de voiture.
- L'état de lecture est relu après `Start` : « démarré sans erreur » n'est pas
  « audible », et l'écart est journalisé.
- Le seul test qui charge une DLL est `#[ignore]` (aucune CI n'a AC) et se lance
  à la main avec `PITBOX_AC_ROOT` ; le relevé du corpus (`fmod::sys::survey`)
  emprunte le sélecteur de bank de la production.

---

## 6. Risques, et ce qu'on en fait

| risque | traitement |
| --- | --- |
| Un plantage de FMOD emporte l'app (DLL dans notre processus) | accepté : c'est la DLL du jeu sur les banks du jeu. Un processus compagnon seulement si un plantage réel est constaté |
| Version de bank incompatible | impossible par construction : la DLL est celle du jeu de l'utilisateur |
| Un mod sans `engine_ext` | `engine_int`, puis tout événement dont le chemin contient `engine`, puis le repli |

---

## 6bis. Le coup d'accélérateur à l'arrêt

Un bouton fait jouer le moteur tout seul, comme quelqu'un qui fait écouter sa
voiture : **3 à 7 s de ralenti, puis une rafale de 2 à 5 coups brefs**, en
boucle et sans régularité. Chaque coup monte plus vite qu'il ne redescend. **Le
sommet se tire par rapport au rupteur de cette voiture**, jamais en valeur
absolue : 50 à 75 % la plupart du temps, et 28 % des fois entre 88 et 97 %. Les
coups qui vont au rupteur visent le `LIMITER` lui-même, et **déclenchent
l'événement du rupteur** (`event:/cars/<id>/limiter`), joué quand le régime
atteint la butée à 60 tr/min près et relâché quand il en repart.

La machine à états vit **dans le thread audio**, pas dans l'interface. Le
curseur et la démonstration pilotent le même paramètre : **toucher le curseur
arrête la démonstration**, et pendant qu'elle tourne le curseur s'efface.

---

## 6ter. Le ralenti, et la salle

Le ralenti suit la voiture (§4.4). **La salle** : une réverbération FMOD de
petite salle à surfaces dures, sur le bus maître — 700 ms de déclin, coupure
à 8 kHz, **niveau humide à −14 dB**, le seul chiffre qui relève du goût. AC
n'en fournit pas (ses banks n'ont aucun snapshot), et une salle posée
généreusement donne une salle de bains : les échantillons portent déjà leur
acoustique.

---

## 6quater. Le gaz vient du geste, pas d'un second réglage

**Il n'y a pas de réglage d'accélérateur** : le gaz se déduit du mouvement du
curseur, dans le thread FMOD, et une commande extérieure serait réécrite au
tick suivant.

| Le curseur | Le gaz |
| --- | --- |
| monte | vers 1 — en charge |
| descend | vers 0 — lâcher de gaz |
| ne bouge plus depuis 180 ms | 0,3, le maintien |

Le passage d'une valeur à l'autre est lissé (70 ms) : une commutation nette
entre deux couches s'entend comme un clic. Le bouton de la souris affine la
dernière ligne (§6septies).

---

## 6quinquies. `MINIMUM` négatif

Voir §4.4 : la magnitude d'un `MINIMUM` négatif est un candidat, cru seulement
dans la bande de plausibilité ; aucune théorie sur ce que le signe veut dire.

---

## 6sexies. Le démarreur : la clé démarre le moteur

**Cliquer sur la clé de contact démarre le moteur** quand le bank porte un
événement d'allumage — `ign_ext`, à défaut `ign_int` (le seul qu'on rencontre,
joué même en vue extérieure). Une voiture sans événement d'allumage, dont
toutes les Kunos, se trouve moteur tournant.

| phase | durée | régime | gaz |
| --- | --- | --- | --- |
| lancement | 1300 ms | monte vers 30 % du ralenti, en ondulant | 0 |
| allumage | 400 ms | bondit à 1,35 × le ralenti | s'ouvre à 0,55 puis retombe |
| stabilisation | 900 ms | redescend au ralenti | 0 |

La voiture démarre **à l'arrêt** (premier bloc à 0 tr/min). Toucher le curseur
ou lancer les coups d'accélérateur pendant la séquence l'annule. Le paramètre
`state` d'`ign_int` reste à sa valeur par défaut (0) ; ce qu'il sélectionne
n'est pas connu.

---

## 6septies. La pédale, c'est le bouton de la souris

Le bouton de la souris dit ce que le mouvement ne peut pas dire : ce que veut
dire un curseur **immobile**.

| Le curseur | Le gaz |
| --- | --- |
| monte | 1 — en charge |
| descend | 0 — lâcher de gaz, bouton tenu ou non |
| immobile, bouton tenu | 1 — le moteur est tenu là |
| bouton relâché | 0, et ça y reste |
| immobile, jamais touché à la souris (clavier, manette) | 0,3, le maintien |

Le relâchement s'écoute sur la fenêtre et non sur le curseur : un glissement
finit souvent hors de la piste, et un relâchement perdu laisserait la pédale
au plancher.

---

## 7. Questions ouvertes

- **Extérieur ou intérieur** : le backend accepte les deux, l'interface n'expose
  que l'extérieur, le plus comparable d'un mod à l'autre. Question de goût, non
  tranchée.
- **Les autres événements** (turbo, échappement, rapports, klaxon) sont
  jouables par la même plomberie, hors périmètre.
- **Couper la musique Big Picture pendant l'écoute ?** À juger à l'oreille.
