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

### 2.3 Pas de `.strings.bank`, mais un `GUIDs.txt`

AC ne livre pas la banque de chaînes qui permettrait de retrouver un événement
par son chemin. Il livre `sfx/GUIDs.txt`, une ligne par événement :

```
{e5f55589-e9ff-47f8-ba0c-664827bb8bef} event:/cars/ks_ford_gt40/engine_ext
{dc0becd4-7ef3-4b80-a37d-7f8efc53ecb4} event:/cars/ks_ford_gt40/engine_int
```

On y lit le GUID et on appelle `GetEventByID`. **C'est précisément la raison
d'être de ce fichier** — ne pas chercher à charger une banque de chaînes qui
n'existe pas.

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

**Lot 0 — la preuve, hors application.** Un petit binaire Rust (ou un test
`#[ignore]`) qui charge les deux DLL, ouvre le bank de la GT40, lit le GUID de
`engine_ext` dans `GUIDs.txt`, crée l'instance, règle le régime à 900 et joue
trois secondes. **Rien d'autre ne commence avant que ce son sorte** : c'est là
que se découvriront les surprises d'ABI.

**Lot 1 — les liaisons.** Les douze fonctions et les deux structures
(`FMOD_GUID`, `FMOD_STUDIO_PARAMETER_DESCRIPTION`) dans un module dédié, avec
les tests qui peuvent l'être sans DLL (parsing de `GUIDs.txt`, reconnaissance
des paramètres par nom et plage).

**Lot 2 — le thread et les commandes.** `Play`/`SetRpm`/`Stop`, l'état partagé,
le basculement vers le repli quand FMOD n'est pas disponible.

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
| Chemins avec accents / espaces | `LoadBankFile` prend un chemin UTF-8. À vérifier au lot 0 sur un chemin accentué. |
| Un mod sans `engine_ext` | Essayer `engine_int`, puis n'importe quel événement dont le chemin contient `engine`, puis repli. |

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
