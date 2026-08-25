# SPEC — Écouter le moteur avec le FMOD d'Assetto Corsa

> Chantier suivant de l'écoute des sons moteur. Remplace une heuristique juste à
> 44 % par le moteur audio du jeu lui-même.
>
> À lire avec `docs/fsb5-format.md`, qui décrit le format des banks et **pourquoi
> l'approche actuelle plafonne**.

---

## 1. Le problème que ça résout

L'écoute d'un mod de son existe (§8 du SPEC, clé de contact dans la fiche
voiture et dans la fiche du mod). Elle repose sur un décodeur maison qui ouvre
le `.bank`, y cherche « le ralenti » et le joue en boucle.

**Chercher le ralenti est un problème qu'on ne sait pas résoudre.** Mesuré sur
91 banks Kunos, dont les noms d'échantillons donnent la bonne réponse : 40 choix
acceptables sur 91 après trois corrections successives de l'estimateur de
hauteur. Les erreurs ne sont pas absurdes — ce sont d'autres couches moteur à
bas régime — mais rien dans le signal ne distingue un ralenti extérieur d'un bas
régime en lâcher de gaz. Le détail, la méthode et les impasses sont dans
`fsb5-format.md`.

Or **cette information existe** : elle est dans le graphe d'événements FMOD du
bank. Un événement `engine_ext` sait quels échantillons mélanger à 900 tr/min
sans accélérateur, parce que c'est exactement ce que fait le jeu.

Et AC livre le moteur qui sait le lire.

---

## 2. Ce qui est vérifié

Tout ce qui suit a été constaté sur l'installation de référence, pas supposé.

### 2.1 Les DLL

`<AC>/fmod64.dll` et `<AC>/fmodstudio64.dll` — **FMOD Studio 1.08.12**
(build 80229, Firelight Technologies), 64 bits, donc la même architecture que
Pit Box. Les variantes 32 bits (`fmod.dll`, `fmodstudio.dll`) existent aussi et
ne nous concernent pas.

### 2.2 Les douze fonctions nécessaires sont exportées

Vérifié par recherche des noms de symboles dans `fmodstudio64.dll` :

| fonction | rôle |
| --- | --- |
| `FMOD_Studio_System_Create` / `_Initialize` / `_Release` | cycle de vie |
| `FMOD_Studio_System_Update` | **à appeler régulièrement**, voir §4.3 |
| `FMOD_Studio_System_LoadBankFile` | charger le `.bank` |
| `FMOD_Studio_System_GetEventByID` | retrouver l'événement par GUID |
| `FMOD_Studio_EventDescription_CreateInstance` | instancier |
| `FMOD_Studio_EventDescription_GetParameterCount` / `_GetParameterByIndex` | **énumérer** les paramètres |
| `FMOD_Studio_EventInstance_SetParameterValue` | régler régime et accélérateur |
| `FMOD_Studio_EventInstance_Start` / `_Stop` | jouer, couper |

### 2.3 Pas de banque de chaînes **pour les voitures**, mais un `GUIDs.txt`

AC livre bien un `content/sfx/common.strings.bank` — corrigé au lot 0, ce
document disait le contraire. Mais il accompagne `common.bank` et ne contient
que les chaînes de celui-ci : **aucune voiture n'a la sienne**, donc retrouver
`event:/cars/…` par son chemin reste impossible.

Le fichier est `content/sfx/GUIDs.txt` (et non `sfx/GUIDs.txt`), une ligne par
événement, GUID puis chemin :

```
{d33f0a36-b38e-410f-b895-4797f5f77e18} event:/cars/ks_ford_gt40/engine_ext
{6855af70-8f4e-4851-a5b0-237bc434d2c1} event:/cars/ks_ford_gt40/engine_int
```

On y lit le GUID et on appelle `GetEventByID`. **C'est précisément la raison
d'être de ce fichier** — ne pas chercher à charger une banque de chaînes de
voiture qui n'existe pas.

`FMOD_Studio_ParseID` est exporté et transforme le texte `{…}` en `FMOD_GUID`.
Le lot 1 a malgré tout écrit l'analyse à la main (`fmod/guids.rs`) : elle rend
un événement résoluble **avant** tout chargement de DLL — donc sur une machine
sans jeu installé — et testable unitairement. L'ordre des octets, seul vrai
risque de ce choix, est **figé par un test** sur ce que `ParseID` a réellement
rendu au lot 0 (`guid_matches_what_fmod_parse_id_returned`). Ne pas défaire ce
test : c'est lui qui remplace la fonction.

### 2.4 Les paramètres

Le pipeline audio d'AC pilote chaque échantillon moteur par un paramètre de
**régime** (avec autopitch par rapport au régime naturel de l'échantillon), et
le mélange charge/décélération par une courbe de volume liée à
l'**accélérateur**.

⚠️ **Ne pas coder les noms en dur.** Ils varient selon l'auteur du mod
(`rpms`, `rpm`, `throttle`, `load`…). Les fonctions d'énumération existent
(§2.2) : on liste les paramètres de l'événement, on reconnaît celui du régime et
celui de l'accélérateur par leur nom **et** leur plage, et on ignore le reste.
Un mod dont on ne reconnaît aucun paramètre se joue quand même — l'événement
démarre à ses valeurs par défaut.

⚠️ **Le nom et la plage ne suffisent pas : filtrer d'abord sur le type.** Voir
§2bis — un événement expose des paramètres *automatiques*, calculés par FMOD à
partir des attributs 3D, à côté de ceux que le jeu pilote.

---

## 2bis. Ce que le lot 0 a établi

Mesuré sur la GT40, `event:/cars/ks_ford_gt40/engine_ext`, avec les DLL de
l'installation de référence. Tout ce qui suit a **surpris** : c'est la raison
d'être du lot 0.

### `ALLOW_MISSING_PLUGINS` n'est pas une commodité, c'est obligatoire

Sans `FMOD_STUDIO_INIT_ALLOW_MISSING_PLUGINS` (0x02) passé à `Initialize`,
`LoadBankFile` **refuse tout bank de voiture** avec `FMOD_ERR_PLUGIN_MISSING`
(54). Le bank référence l'effet « FMOD Distance Filter », et ce plugin n'existe
sous **aucune** forme dans l'installation AC — vérifié : ni DLL à part, ni
symbole dans `fmod64.dll` ou `fmodstudio64.dll`.

Conséquence à ne pas manquer : **le jeu lui-même tourne donc avec ce drapeau**,
et l'effet n'est pas davantage appliqué dans AC. On ne dégrade rien.

`common.bank` (le bank maître, reconnaissable à son `.strings.bank` voisin) se
charge **avant** le bank de la voiture : c'est lui qui porte les bus dans
lesquels les événements de voiture se routent.

### `FMOD_STUDIO_PARAMETER_DESCRIPTION` n'a pas la disposition attendue

La disposition « évidente » de la 1.x — `name / index / minimum / maximum /
type`, 24 octets — est **fausse**. Elle place `type` à l'offset 20, or l'offset
20 vaut toujours zéro et l'énumération de type est à l'**offset 24** :

| offset | champ |
| --- | --- |
| 0 | `const char *name` |
| 8 | `int index` |
| 12 | `float minimum` |
| 16 | `float maximum` |
| 20 | `float` — 0.0 partout jusqu'ici, vraisemblablement `defaultvalue` |
| 24 | `FMOD_STUDIO_PARAMETER_TYPE type` |

soit 28 octets utiles, 32 avec l'alignement du pointeur.

**Méthode**, la même que pour le KN5 : passer à FMOD un tampon zéroté bien plus
grand que la structure attendue et vider ses octets bruts. Une erreur de
disposition se lit alors dans le tampon au lieu d'écraser la pile.

**Et la confirmation n'est pas venue du tampon, mais du sens.** Lire à l'offset
24 donne `Distance` = 1 et `Event Cone Angle` = 2, soit exactement
`AUTOMATIC_DISTANCE` et `AUTOMATIC_EVENT_CONE_ANGLE` dans
`FMOD_STUDIO_PARAMETER_TYPE`. Lire à l'offset 20 donne 0 pour tout le monde —
ce qui **ressemble à un succès** : quatre paramètres tous `GAME_CONTROLLED`,
rien d'aberrant à l'écran. C'est le genre d'erreur qui ne se voit qu'en
cherchant à confirmer une valeur autrement que par elle-même.

### Seuls les paramètres `GAME_CONTROLLED` nous appartiennent

Les quatre paramètres de `engine_ext` :

| nom | plage | type |
| --- | --- | --- |
| `throttle` | 0 – 1 | 0 `GAME_CONTROLLED` |
| `rpms` | 0 – 20000 | 0 `GAME_CONTROLLED` |
| `Event Cone Angle` | 0 – 180 | 2 `AUTOMATIC_EVENT_CONE_ANGLE` |
| `Distance` | 0 – 500 | 1 `AUTOMATIC_DISTANCE` |

Les automatiques sont calculés par FMOD à partir des attributs 3D ; y écrire
n'a pas de sens. **Le filtre par type passe donc avant le filtre par nom** :
sans lui, un repli « la plage la plus large gagne » finirait un jour par
désigner `Distance`.

### Le paramètre de régime pilote réellement la hauteur

Vérifié en redirigeant la sortie de FMOD vers son écrivain WAV
(`FMOD_System_SetOutput(WAVWRITER)` sur le système bas niveau **avant**
`Initialize`, nom de fichier passé en `extradriverdata`) — c'est ce qui permet
de *mesurer* au lieu de constater qu'un état vaut `PLAYING`, lequel ne prouve
pas qu'un seul échantillon non nul soit sorti.

| `rpms` | f0 mesurée | autocorrélation |
| --- | --- | --- |
| 900 | 57,5 Hz | 0,655 |
| 1800 | 114,6 Hz | 0,577 |
| 3600 | — | 0,163 |
| 6000 | — | 0,221 |

**Exactement le double entre 900 et 1800** : la relation est linéaire, ce qui
est tout ce dont le curseur de §4.4 a besoin. L'autocorrélation reste dans la
bande 0,53–0,84 que `fsb5-format.md` a établie pour un vrai échantillon moteur.

Deux réserves honnêtes :

- il subsiste un écart **constant** de 4 % (862 et 1718 tr/min implicites pour
  900 et 1800, en supposant un V8 quatre temps). Constant aux deux points, donc
  un facteur d'échelle — calibrage d'autopitch de l'auteur, hypothèse sur le
  nombre de cylindres, ou l'écrivain WAV lui-même. **Sans conséquence** tant
  qu'on affiche le régime demandé et non un régime mesuré ;
- à 3600 et 6000 l'estimateur d'autocorrélation s'effondre (0,16 et 0,22, le
  niveau du bruit de vent). Ce n'est **pas** l'audio qui faiblit — RMS et crête
  montent au contraire (1446 et 1456, crêtes 8840 et 12405). C'est la limite de
  l'analyse du signal documentée dans `fsb5-format.md`, et précisément ce que ce
  chantier rend sans objet.

### Le régime se pilote sur une instance qui tourne déjà

Distinction qui a failli passer à la trappe : régler `rpms` **avant** `Start` ne
prouve rien sur le fait de le régler **après**. Or c'est la seconde forme, et
elle seule, dont dépend le curseur du §4.4.

Vérifié par un balayage 900 → 7000 → 900 tr/min joué en une seule instance, en
appelant `SetParameterValue` toutes les 20 ms depuis la boucle d'`Update`. Le
taux de passages par zéro de la capture suit la consigne dans les deux sens
(216 → 2668 → 554 par seconde), donc la hauteur suit.

**Et `throttle` fait son travail au passage** : le RMS chute de 5366 à 1422
exactement à l'instant où il passe de 1,0 à 0,0 à mi-balayage. Les couches en
charge cèdent la place aux couches en lâcher de gaz — le mélange décrit en §2.4
n'est pas une supposition.

Le taux de passages par zéro est ici un meilleur juge que l'autocorrélation :
il reste monotone sur toute la plage, là où l'autocorrélation décroche au-delà
de ~2000 tr/min.

### Divers, vérifié au passage

- **Chemins accentués : réglé.** `LoadBankFile` prend bien de l'UTF-8 en 1.08 —
  un bank chargé depuis `…\écurie Ferrari — essai\sfx\` (accent, tiret cadratin,
  espaces) s'ouvre sans rien de particulier. Le risque de §6 est clos.
- `FMOD_VERSION` = `0x00010812`, lu dans la ressource de version des DLL
  (« 1.8.12 (build 80229) »). `System_Create` l'accepte ; une valeur fausse
  rendrait `FMOD_ERR_HEADER_MISMATCH` (20).
- Les échantillons ne sont **pas** chargés par `LoadBankFile`. Passer par
  `EventDescription_LoadSampleData` puis attendre
  `GetSampleLoadingState == LOADED` en appelant `Update` — sinon les premières
  centaines de millisecondes sont muettes.
- Aucune configuration d'auditeur 3D n'a été nécessaire : auditeur et instance
  sont à l'origine par défaut, donc à distance nulle.
- Les douze fonctions de §2.2 sont exportées, plus `ParseID`, `LoadSampleData`,
  `GetSampleLoadingState`, `GetPlaybackState` et `IsVirtual` — tous utiles.

---

## 3. Licence FMOD — position prise

FMOD est propriétaire. Le contrat autorise l'usage **non commercial**
(clause 1.2 : loisir, éducation, sans monétisation, sponsoring ni promotion) et
n'exige **aucune inscription** dans ce cadre. Pit Box est gratuit et open source,
donc dedans.

**Obligation qui s'applique et qu'il faut tenir** : la clause 3 impose une
mention visible contenant les mots « FMOD Studio » et « Firelight
Technologies Pty Ltd ». Elle va dans l'écran **À propos**, à côté des mentions
existantes. À faire dans le même lot que le code, pas « plus tard ».

**Deux réserves, écrites ici pour qu'elles soient une décision et pas un
oubli :**

- le contrat décrit le cas « moteur intégré **et redistribué** dans un
  produit ». Nous ne redistribuons **rien** : aucune DLL FMOD n'entre dans le
  dépôt ni dans l'installateur, on charge la copie que l'utilisateur possède
  déjà avec son jeu. Le texte est **silencieux** sur ce cas — ni permis ni
  interdit explicitement ;
- si Pit Box devenait un jour monétisé, sponsorisé ou promotionnel, une licence
  FMOD deviendrait nécessaire.

**Corollaire technique** : ne jamais copier une DLL FMOD ailleurs sur le disque,
ne jamais la charger depuis un autre endroit que l'installation AC configurée.

---

## 4. Architecture

### 4.1 Deux chemins, et le repli n'est pas une consolation

| chemin | quand | ce qu'il donne |
| --- | --- | --- |
| **FMOD** | AC configuré et DLL chargeables | l'événement moteur réel, régime réglable |
| **décodeur maison** | pas d'AC, DLL absentes, bank refusé | un échantillon en boucle, ralenti deviné |

Le décodeur FSB5/FADPCM existant **reste** et n'a rien perdu de son utilité :
c'est lui qui alimente la fiche du mod (encodage, nombre d'échantillons,
fréquence, durée, présence des noms), et c'est le seul chemin qui marche sans
installation de jeu. Son heuristique de ralenti ne sert plus qu'ici — **ne pas
chercher à l'améliorer**, `fsb5-format.md` explique pourquoi c'est une impasse.

### 4.2 Chargement des DLL

- `libloading` (ou `LoadLibraryW` direct) sur `<AC>/fmod64.dll` **puis**
  `<AC>/fmodstudio64.dll` : le second dépend du premier, et charger le premier
  explicitement évite de dépendre du chemin de recherche du système.
- Ne pas modifier le `PATH` du processus. Si une aide est nécessaire,
  `AddDllDirectory` limité au dossier du jeu.
- L'échec de chargement n'est **pas** une erreur à remonter à l'écran : c'est le
  basculement silencieux vers le repli, avec un `log::warn!` (règle du
  CLAUDE.md sur les `let _ =`).

### 4.3 Un thread propriétaire, comme le moteur musical

`FMOD_Studio_System_Update` doit être appelé régulièrement (le mélange et la
libération des instances s'y font). Le projet a déjà ce patron :
`music/engine.rs` possède son `OutputStream` rodio sur un thread dédié et reçoit
des commandes.

**Faire pareil** : un thread possède le système FMOD, appelle `Update` à
intervalle fixe, et reçoit `Play { bank, guid, rpm } | SetRpm(f32) | Stop`. Le
système FMOD n'est **jamais** touché depuis un autre thread.

Conséquence sur l'existant : l'audio quitte la webview. `enginePlayer.svelte.ts`
garde son rôle d'état d'interface (quelle ligne joue, laquelle charge) mais
appelle des commandes Tauri au lieu de Web Audio, **sur le chemin FMOD
seulement** — le repli continue de passer par Web Audio, puisqu'il produit un
WAV.

⚠️ **Deux sorties audio coexisteront** (rodio pour la musique Big Picture, FMOD
ici). Les deux ouvrent WASAPI en mode partagé, donc ça se mélange sans
conflit — mais il faut le vérifier une fois, à l'oreille, en Big Picture.

### 4.4 Ce que l'interface gagne

La clé de contact ne change pas. S'y ajoute, **là où le chemin FMOD est actif**,
un **curseur de régime** du ralenti à la zone rouge : c'est trivial une fois le
paramètre piloté (`SetParameterValue` sur l'instance qui tourne), et c'est plus
utile que le seul ralenti — entendre un mod monter en régime sans lancer le jeu
est exactement ce qu'on veut comparer entre deux mods.

Le régime de ralenti exact vit dans `data/engine.ini`, donc dans `data.acd`
**chiffré** la plupart du temps. **Ne pas s'y attaquer** : 900 tr/min au départ,
et le curseur rend la question sans objet.

---

## 5. Plan par lots

Chaque lot se termine par quelque chose qu'on peut entendre ou mesurer.

**Lot 0 — la preuve, hors application. ✅ fait.** Un petit binaire Rust hors
dépôt (zéro dépendance, `LoadLibraryW`/`GetProcAddress` à la main) charge les
deux DLL, ouvre le bank de la GT40, lit le GUID de `engine_ext` dans
`GUIDs.txt`, énumère les paramètres, crée l'instance, règle le régime et joue.
Les cinq surprises d'ABI qu'il a fait sortir sont en **§2bis** — ce sont elles
le livrable du lot, pas le binaire.

**Lot 1 — les liaisons. ✅ fait.** `src-tauri/src/fmod/`, découpé pour que la
partie qui décide *ce qu'on joue* se teste sans DLL :

| fichier | rôle |
| --- | --- |
| `guids.rs` | `FMOD_GUID`, lecture de `GUIDs.txt`, chaîne de repli sur l'événement |
| `params.rs` | reconnaissance des paramètres par type, nom et plage |
| `sys.rs` | la FFI Windows — les seize entrées, les deux structures, le cycle de vie |

Seize entrées et non douze : le lot 0 a montré que le chargement des
échantillons n'est pas implicite et que l'état de lecture est le seul moyen de
savoir qu'une instance est vivante.

**21 tests, aucun n'ayant besoin d'une DLL.** Deux méritent d'être signalés
parce qu'ils gèlent une erreur déjà commise plutôt qu'une intention :
`parameter_description_keeps_its_measured_layout` échoue si quelqu'un
« range » la structure vers la version plausible à 24 octets, et
`guid_matches_what_fmod_parse_id_returned` fixe l'ordre des octets sur ce que
`ParseID` a réellement rendu au lot 0 — c'est ce qui permet d'analyser un GUID
**sans** charger la moindre DLL, donc de résoudre un événement sur une machine
sans jeu installé.

Découverte du lot, mesurée sur l'installation de référence : **122 voitures sur
299 livrent leur propre `sfx/GUIDs.txt`** — ce sont les mods. Les événements
d'un mod de son sont **absents** de la table globale, donc l'ordre de recherche
(fichier de la voiture, puis table globale) n'est pas un raffinement : c'est ce
qui fait qu'un mod se joue du tout.

**Lot 2 — le thread et les commandes. ✅ fait.** `fmod/engine.rs` : un thread
possède le système, reçoit `Play`/`SetRev`/`SetThrottle`/`Stop` et appelle
`Update` toutes les 20 ms. Rien n'est chargé au démarrage de l'app — les DLL du
jeu ne sont touchées qu'à la première écoute, donc une machine sans AC ne paie
rien.

`Play` **répond** (canal de retour, appel bloquant) : l'appelant a besoin de
savoir si le chemin natif a marché pour retomber sur le décodeur maison, et la
réponse coûte un chargement de bank, pas une attente visible.

Quatre décisions qui ne se devinent pas :

- **Un seul bank de voiture chargé à la fois.** Deux mods de son pour la même
  voiture déclarent les **mêmes GUID d'événements** : laisser le précédent
  chargé fait rendre à `GetEventByID` l'événement de l'autre bank — et ça se
  présente comme « ce mod sonne pareil », pas comme un bug. D'où
  `FMOD_Studio_Bank_Unload`, une entrée de plus que prévu.
- **Le `GUIDs.txt` se cherche à côté du bank**, pas sous la voiture : un mod
  s'auditionne depuis la bibliothèque, où la disposition du jeu ne s'applique
  pas. Repli sur la table globale pour le contenu Kunos.
- **`System` possède son `Fmod`** au lieu de l'emprunter, sinon le thread se
  retrouve avec une structure auto-référentielle. Et `System` n'est pas `Send` :
  la règle « un seul thread y touche » est tenue par le compilateur, pas par la
  discipline.
- **L'état de lecture est relu après `Start`.** « Démarré sans erreur » et
  « réellement audible » sont deux affirmations différentes — une instance peut
  revenir virtuelle ou déjà arrêtée, et c'est exactement le cas où l'utilisateur
  dit « j'ai cliqué et je n'ai rien entendu ». Journalisé, pas fatal.

Commandes : `audition_engine_native`, `set_audition_rev`,
`set_audition_throttle`, `stop_audition_native`. Toute erreur de la première est
un **signal de repli**, jamais un message à afficher — d'où des diagnostics
bruts plutôt que des clés i18n.

**Le seul test qui exerce une DLL est `#[ignore]`**, première occurrence dans le
projet et pour une raison précise : aucun agent de CI n'a d'installation
d'Assetto Corsa ni de carte son. Il se lance à la main et sans la variable il
passe au lieu d'échouer faussement :

```
PITBOX_AC_ROOT="D:\...ssettocorsa" cargo test --lib fmod::engine -- --ignored --nocapture
```

Vérifié ainsi sur trois voitures — la GT40 Kunos et deux mods (`art_skyline_r32_gtr`,
`bati_fd3s_rx7`) qui passent par leur propre table : `rpms` 0–20000 et
`throttle` reconnus sur les trois, régime piloté en cours de lecture.

**Lot 3 — l'interface.** La clé branchée sur le chemin natif, le curseur de
régime, et la mention FMOD dans À propos (§3).

**Lot 4 — le corpus.** Passer sur les 297 voitures : combien chargent, combien
exposent un paramètre de régime reconnu, combien tombent au repli. Le chiffre
va dans ce document.

---

## 6. Risques, et ce qu'on en fait

| risque | traitement |
| --- | --- |
| **Un plantage de FMOD emporte l'app** (DLL dans notre processus) | Accepté au départ : c'est la DLL du jeu sur les banks du jeu, le chemin le plus éprouvé qui soit. Un processus compagnon isolerait totalement, au prix de la livraison — à ne faire que si un plantage réel est constaté. |
| Les crates Rust FMOD (`libfmod`, `fmod-sys`) visent **2.x** | Inutilisables : l'API des paramètres a changé entre 1.x et 2.x. FFI écrite à la main — et donc **aucune dépendance ajoutée**, ce qui va dans le sens du projet. |
| Version de bank incompatible | **Impossible par construction** : on utilise la DLL du jeu de l'utilisateur, donc tout ce que son jeu sait jouer, on sait le jouer. |
| ~~Chemins avec accents / espaces~~ | **Clos au lot 0** : `LoadBankFile` prend bien de l'UTF-8 (§2bis). |
| Un mod sans `engine_ext` | Essayer `engine_int`, puis n'importe quel événement dont le chemin contient `engine`, puis repli. |

---

## 6bis. La cible : le coup d'accélérateur à l'arrêt

**Décidée avec l'utilisateur après avoir entendu le balayage du lot 0**, et
placée ici pour ne pas se perdre. Elle ne remplace aucun lot du §5 : elle vient
**après**, et le plan existant se termine d'abord.

L'idée : depuis l'interface, reproduire ce que fait quelqu'un qui donne
quelques coups d'accélérateur, voiture à l'arrêt, pour faire écouter son moteur
aux autres — avec quelques arrivées au **rupteur**.

Ce n'est pas un nouveau chantier technique, et c'est ce qui la rend
intéressante : le lot 0 a déjà prouvé les deux mécaniques dont elle dépend.

- Le régime **et** l'accélérateur se pilotent sur une instance qui tourne, à la
  cadence de la boucle d'`Update` (§2bis). Un coup d'accélérateur n'est donc
  qu'une **enveloppe** appliquée à ces deux paramètres — montée franche,
  maintien court, retombée plus lente que la montée — là où le curseur du §4.4
  suit la main de l'utilisateur.
- La chute de RMS mesurée au passage `throttle` 1,0 → 0,0 montre que les
  couches en charge et en lâcher de gaz se relaient bien. C'est ce contraste
  qui fait qu'un coup d'accélérateur *sonne* comme tel plutôt que comme une
  montée en régime.

Deux points à traiter le moment venu, aucun tranché :

- **Le rupteur est un événement séparé**, `event:/cars/<id>/limiter` dans
  `GUIDs.txt` — pas une région de `engine_ext`. Il se déclenche donc comme
  seconde instance, et « arriver au rupteur » veut dire savoir à quel régime.
  Or ce régime vit dans `data/engine.ini`, donc dans `data.acd` chiffré : même
  impasse qu'au §4.4 pour le ralenti, et probablement la même réponse —
  approcher le haut de la plage du paramètre plutôt que chercher la vraie
  valeur.
- **Ne pas transformer ça en séquenceur.** Une poignée d'enveloppes crédibles
  vaut mieux qu'un éditeur de courbes ; c'est un bouton qui fait chanter le
  moteur, pas un outil d'automation.

---

## 7. Questions ouvertes

- **Quel événement par défaut**, `engine_ext` ou `engine_int` ? L'extérieur est
  le plus flatteur et le plus comparable d'un mod à l'autre ; l'intérieur est ce
  que le pilote entend. Peut-être les deux, en bascule — à trancher avec
  l'utilisateur une fois qu'on les aura entendus.
- **Les autres événements** (turbo, échappement, changements de rapport,
  klaxon) deviennent jouables une fois la plomberie en place. Intéressant, mais
  hors périmètre tant que le moteur ne marche pas.
- **Faut-il couper la musique Big Picture pendant l'écoute ?** À juger à
  l'oreille (§4.3).
