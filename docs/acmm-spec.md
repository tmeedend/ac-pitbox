# AC Mod Manager — Spécification fonctionnelle et technique

> Application desktop unifiée de gestion de mods pour Assetto Corsa.
> Remplace Mod Organizer 2, s'appuie sur Content Manager comme moteur de lancement.
> Version de spec : 0.2 (brouillon de travail)

> **Nouveautés v0.2** (issues des sessions de conception d'interface) :
> surcouche de métadonnées non destructive (le fichier du mod n'est jamais modifié) ;
> tags tracés par origine (fichier mod / règle / manuel) ;
> classe + année comme dimension de composition de plateau ;
> météo simplifiée à dégradé gracieux selon la stack (Pure/SOL/CSP/vanilla) avec température implicite ;
> presets de session par type (réglages persistants, disponibilité selon le type) ;
> écran de course à deux niveaux ; skins, layouts et miniatures ;
> confirmation que CM est maître de `race.ini` (pilotage par presets, pas par écriture de fichiers).

---

## 1. Contexte et objectif

### 1.1 Problème actuel

La gestion de mods Assetto Corsa repose aujourd'hui sur un empilement d'outils mal articulés :

- Les mods se téléchargent sous forme d'archives (`.zip`, `.rar`, `.7z`) de structure hétérogène et souvent incorrecte.
- **Mod Organizer 2** sert à activer/désactiver et à catégoriser — mais il a été conçu pour Bethesda (Skyrim/Fallout) et son *Virtual File System* n'apporte presque rien sur AC, dont les mods sont des dossiers autonomes.
- **Content Manager (CM)** sert de launcher et de moteur de configuration — mais oblige à re-catégoriser une seconde fois, en doublon de MO2.
- Résultat : double gestion des catégories, tags non harmonisés, et surtout une **maintenance manuelle pénible** : retrouver si un mod est déjà installé, gérer les mises à jour quand l'auteur renomme le dossier, traquer les doublons.

### 1.2 Objectif

Une application desktop **unique** qui prend en charge tout le cycle de vie d'un mod :

1. **Importer** une archive (analyse, détection de type, rangement propre).
2. **Identifier** le mod de façon fiable (nouveau / mise à jour / doublon), indépendamment du nom de dossier.
3. **Organiser** : harmonisation automatique des tags, recherche, filtres, bibliothèque visuelle.
4. **Activer/désactiver** sans duplication de données, même avec ~300 Go de mods.
5. **Lancer** une session directement, sans subir l'UI de CM.
6. **Maintenir** : mises à jour, historique, export d'archives autonomes, nettoyage.

### 1.3 Principe directeur

Le cœur de l'application n'est pas « activer des mods » — c'est **résoudre l'identité d'un mod**. Tout le reste en découle.

---

## 2. Architecture

### 2.1 Vue en couches

```
┌─────────────────────────────────────────────────────────────┐
│  COUCHE 1 — BIBLIOTHÈQUE (source de vérité)                   │
│  Disque dédié, ~300 Go. Tous les mods rangés proprement,     │
│  potentiellement plusieurs versions d'un même mod.           │
└─────────────────────────────────────────────────────────────┘
                          │  junctions (mklink /J)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  COUCHE 2 — APPLICATION (ce document)                        │
│  Import · moteur d'identité · tags · activation · lancement  │
└─────────────────────────────────────────────────────────────┘
                          │  écrit/supprime des junctions
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  COUCHE 3 — content/ d'Assetto Corsa                         │
│  Peuplé dynamiquement de junctions vers la bibliothèque.     │
│  content/cars/<id>  ·  content/tracks/<id>                   │
└─────────────────────────────────────────────────────────────┘
                          │  protocole acmanager:// + CLI
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  COUCHE 4 — CONTENT MANAGER (moteur conservé)                │
│  Presets graphiques/FFB/contrôleur configurés une fois.     │
│  Invoqué par l'app pour lancer une session.                  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Décisions structurantes (verrouillées)

| Sujet | Décision | Justification |
|---|---|---|
| Remplacement de MO2 | **Oui, total** | Son VFS n'apporte rien sur AC ; les mods sont des dossiers autonomes. |
| Mécanique d'activation | **Directory junctions Windows** (`mklink /J`) | Zéro duplication (critique à 300 Go), instantané, pas de droits admin requis (contrairement aux symlinks). |
| Source de vérité | **Bibliothèque hors du jeu** | Le dossier `content/` d'AC devient une projection, jamais l'original. |
| Content Manager | **Conservé comme moteur + launcher** | Reproduire son moteur de config (CSP/Sol/Pure, FFB) serait un chantier énorme et fragile. On contourne son UI, pas son moteur. |
| Backend | **Réécrit dans le langage de l'app** (voir §2.3) | Le code Python existant est abandonné ; sa logique sert de référence. |
| Stack desktop | **Tauri (frontend web + backend Rust)** | Voir §2.3. |

### 2.3 Choix de la stack : Tauri

**Recommandation : Tauri plutôt qu'Electron.**

- **Empreinte et démarrage** : binaire léger, lancement rapide — adapté à une app qu'on ouvre souvent pour un geste rapide (importer un zip, activer un set).
- **Manipulation système** : Rust est à l'aise avec les opérations de système de fichiers, la création de junctions (`std::os::windows::fs` / appels `mklink`), le lancement de process et l'enregistrement de protocole personnalisé.
- **Frontend web** : toute la richesse visuelle (galerie, tableau, vignettes de preview) en HTML/CSS/TS, là où la bibliothèque doit être belle et fluide.
- **Sécurité du modèle** : le pont commande/événement de Tauri impose une frontière nette entre l'UI et les opérations privilégiées (suppression de junctions, écriture disque), ce qui est sain pour une app qui modifie l'install d'un jeu.

> Note : le code Python actuel (`actools/`) n'est pas réutilisé comme runtime, mais il documente les cas réels à reproduire (voir §4.2 et §11). C'est une spécification vivante, pas du code mort.

---

## 3. Modèle de données

### 3.0 Principe fondateur : surcouche non destructive (overlay)

**Le fichier `ui_car.json` / `ui_track.json` d'un mod n'est JAMAIS modifié.** C'est une règle absolue, pas une option.

L'ancien code Python réécrivait ces fichiers (`json.dump` dans `fixCarTags`) — cohérent pour un outil personnel de *génération* de mods, mais inacceptable pour une app destinée à la communauté : réécrire le travail d'un moddeur casse la signature d'intégrité que certains serveurs vérifient et rend les modifications indissociables du mod.

Modèle retenu : **deux sources de vérité séparées.**

- La **bibliothèque** est la source de vérité des *fichiers* (le contenu des mods, en lecture seule).
- La **base d'overlay** (base de l'app, à côté) est la source de vérité des *métadonnées produites par l'app* : tags ajoutés/déduits, classe corrigée, année, catégorie, historique, presets. Indexée sur l'empreinte du mod (§4.1).

Le fichier du mod est une **entrée** du pipeline (on le lit), jamais une **sortie** (on ne l'écrit pas). Conséquences : si l'app est désinstallée, les mods sont intacts ; les métadonnées peuvent être partagées ou réinitialisées sans toucher au contenu ; un badge « fichier du mod jamais modifié » peut être affiché pour rassurer l'utilisateur.

### 3.1 Entités

**Mod** — unité logique (une voiture, un circuit). Possède une identité stable (voir §4.1) indépendante du nom de dossier.

**ModVersion** — une version concrète d'un Mod, telle qu'importée. Plusieurs ModVersions peuvent coexister pour un même Mod (rollback).

**Tag** — étiquette harmonisée. Issue de l'ontologie (voir §5).

**Profile** — un ensemble nommé de ModVersions activées (« GT3 endurance », « Drift touge », etc.).

**HistoryEntry** — entrée horodatée d'événement sur un Mod (import, mise à jour, remplacement, activation…).

### 3.2 Schéma indicatif

```
Mod
  id_interne        : string   # ex. "ferrari_488_gt3" (dossier content/<type>s/<id>)
  type              : enum     { CAR, TRACK }
  brand             : string   # lu dans ui_*.json (lecture seule)
  display_name      : string   # lu dans ui_*.json "name" (lecture seule)
  identity_hash     : string   # empreinte composite (voir §4.1)
  # --- dimension de composition de plateau (overlay, éditable) ---
  car_class         : enum?    { RACE, STREET }   # voitures uniquement
  year              : int?     # lu de ui_*.json, corrigeable dans l'overlay
  category          : string?  # ex. "GT3" (overlay) — sert à monter les plateaux (§5bis)
  # --- tags tracés par origine ---
  tags_from_mod     : [string] # lus dans ui_*.json, LECTURE SEULE
  tags_from_rule    : [string] # déduits par l'ontologie, stockés dans l'overlay
  tags_manual       : [string] # ajoutés à la main, stockés dans l'overlay
  versions          : [ModVersion]
  active_version_id : string?  # null si aucune version active
  history           : [HistoryEntry]

ModVersion
  id                : uuid
  version_label     : string?  # depuis ui_*.json "version", si présent
  author            : string?
  imported_at       : datetime
  library_path      : string   # emplacement dans la bibliothèque
  source_archive    : string   # nom de l'archive d'origine
  content_signature : string   # signature des fichiers clés
  csp_features      : [string] # rainfx, grassfx, weatherfx, has-skins...
  skins             : [string] # skins disponibles (voitures)
  layouts           : [string] # layouts disponibles (circuits)

Profile
  id                : uuid
  name              : string
  entries           : [{ mod_id, version_id }]

HistoryEntry
  timestamp         : datetime
  event             : enum { IMPORT, UPDATE_REPLACE, UPDATE_KEPT_BOTH, ACTIVATE, DEACTIVATE, EXPORT, DELETE }
  details           : string   # ex. "v1.2 → v1.3"
```

---

## 4. Moteur d'identité (cœur de l'app)

### 4.1 Empreinte composite

Le nom de dossier seul **n'est pas une identité fiable** : un auteur peut mettre à jour un mod en renommant le dossier. L'identité se construit donc sur plusieurs signaux :

1. **Identifiant interne** — le nom du dossier `content/cars/<id>` ou `content/tracks/<id>`. Souvent stable même quand le nom d'archive change. Signal le plus fort.
2. **Couple `brand` + `name`** — lu dans `ui_car.json` / `ui_track.json`. Survit aux renommages de dossier.
3. **Signature de contenu** — empreinte des fichiers clés présents (ex. modèle `.kn5`, présence de `data.acd`, structure de skins). Permet de détecter qu'un contenu est « le même à peu de chose près ».

### 4.2 Résolution à l'import

À chaque import, l'app calcule l'empreinte du mod entrant et la compare à la bibliothèque :

```
┌─ Même id de dossier ?
│    OUI → MISE À JOUR DIRECTE
│           Remplace l'ancienne version.
│           Message d'information : "Ferrari 488 GT3 v1.2 → v1.3,
│           l'ancienne version sera remplacée."
│           → HistoryEntry(UPDATE_REPLACE)
│
├─ Match flou (brand+name proches, id différent) ?
│    OUI → DEMANDE EXPLICITE À L'UTILISATEUR (validation 1 clic)
│           "Ceci ressemble à une nouvelle version de <X>
│            (dossier différent : <ancien_id> → <nouveau_id>).
│            Que faire ?"
│             [ Garder les deux versions ]  [ Écraser l'ancienne ]
│           → HistoryEntry(UPDATE_KEPT_BOTH | UPDATE_REPLACE)
│
└─ Aucun match ?
     → NOUVEL IMPORT
       → HistoryEntry(IMPORT)
```

### 4.3 Règles (telles que définies)

- **Id identique** → remplacement automatique + message d'info, pas de blocage.
- **Id différent mais match flou** → l'app *propose* le rapprochement et demande : garder les deux, ou écraser. Jamais de décision silencieuse sur un cas ambigu.
- **Tout import** → ajoute une entrée horodatée à l'historique du mod.

### 4.4 Conservation multi-versions

L'archi à junctions rend la coexistence de plusieurs versions quasi gratuite : les versions vivent côte à côte dans la bibliothèque, une seule est « active » (projetée par junction vers `content/`). Bénéfice : rollback immédiat si une mise à jour casse quelque chose.

### 4.5 Sources d'import : archive OU dossier existant

L'import accepte **deux sources**, traitées par le même pipeline d'analyse/identité/tagging (seule l'entrée diffère) :

1. **Archive** (`.zip`/`.rar`/`.7z`) — décompression puis analyse.
2. **Dossier déjà décompressé** — analyse directe, sans décompression. Cas d'usage majeur : **migrer un catalogue Mod Organizer 2 existant** sans tout re-zipper. La descente récursive trouve le `content/cars` / `content/tracks` à l'intérieur du dossier de mod, exactement comme pour une archive.
   - **Périmètre v1 : un dossier de mod à la fois** (pas l'import en masse de tous les dossiers MO2 d'un coup — prévu plus tard).

**Option copier / déplacer** (case à cocher à l'import de dossier) :
- **Copier** : l'original reste en place (filet de sécurité ; recommandé au début d'une migration).
- **Déplacer** : l'original est retiré de la source — plus rapide, but de la migration une fois la confiance acquise.

**Déplacement adaptatif selon le disque** (l'app détecte, l'utilisateur ne s'en occupe pas) :
- **Même disque** source/destination → *rename* (déplacement quasi instantané, atomique, quel que soit le poids). C'est le gain de vitesse recherché.
- **Disques différents** → copie physique inévitable (les octets voyagent) puis suppression de la source **après** vérification de la copie (la vérification ne coûte rien de plus puisque la copie est déjà l'opération lente). Évite la perte de données pour cet utilisateur, sans pénaliser le cas même-disque.

### 4.6 Import en masse (dossier parent → plusieurs mods)

En plus de l'import unitaire (§4.5), l'app accepte un **dossier parent** dont **chaque sous-dossier direct est traité comme un mod**. Cas d'usage majeur : migrer un catalogue Mod Organizer 2 entier en une fois (la racine `mods/` contient un dossier par mod). Mode connu des utilisateurs (CM fonctionne de façon comparable).

- **Scan : un seul niveau** de sous-dossiers (sous-dossiers directs = mods). Pas de récursion en profondeur sur le parent (la descente récursive s'applique *à l'intérieur* de chaque mod, pas pour découvrir les mods).
- **Flux en deux temps — analyser puis exécuter** (pas de traitement bloquant au fil de l'eau, sinon l'app se fige à chaque cas ambigu sur un gros volume) :
  1. **Phase d'analyse** : l'app scanne et analyse tous les sous-dossiers **sans rien écrire**, puis présente un **récapitulatif** : nb de nouveaux, de mises à jour, de doublons, de **cas ambigus** (match flou), et de **dossiers ignorés** (pas de `content/` reconnu — ex. dossiers d'un autre jeu, notes, dossiers vides).
  2. **Arbitrage groupé** : l'utilisateur traite les exceptions (cas ambigus) de façon **ramassée**, pas une par une au fil de l'eau. Les cas clairs ne demandent rien.
  3. **Exécution** : import en masse selon les décisions + option copier/déplacer.
- **Robustesse à l'échelle** : sur des centaines de dossiers, l'exécution doit être **reprenable** — une interruption ne doit pas laisser d'état incohérent ni obliger à tout refaire.
- **Détection des non-mods** : un sous-dossier sans structure AC reconnaissable est listé comme « ignoré », jamais traité de travers.

> Périmètre : l'import unitaire (§4.5) reste le mode par défaut/initial ; l'import en masse est l'outil de migration de catalogue, utilisé une fois la confiance acquise.

> Note : l'import remplit la **bibliothèque** (source de vérité), pas `content/`. L'activation (junction) reste une étape séparée — l'import de dossier ne court-circuite pas le modèle.

### 4.6bis Interface d'import et activation par défaut

**Deux voies d'accès à l'import** (correction : l'implémentation initiale mettait une barre de boutons en haut, seulement sur Voitures/Circuits — jugée peu esthétique et incohérente) :
- **Glisser-déposer partout** : déposer une archive ou un dossier n'importe où dans l'app lance l'import. Geste rapide, disponible sur toutes les vues (pas seulement Voitures/Circuits).
- **Écran d'import dédié** : un écran à part (pas une barre en haut) qui présente et **explique chaque option** — import unitaire vs en masse, dossier vs archive, copier/déplacer, récapitulatif d'import en masse. C'est là qu'on contrôle finement, quand on ne se contente pas du drag-drop.

**Choix copier/déplacer — mieux présenté** (correction : deux boutons « Copier »/« Déplacer » surgissant à chaque import de dossier = pénible). À présenter clairement dans l'écran dédié, avec explication de chaque option (et de l'accélération même-disque du déplacement). Un **réglage par défaut** peut être mémorisé pour ne pas reposer la question à chaque fois (ex. « toujours copier »).

**Activation par défaut à l'import** : un mod **importé est activé par défaut** (junction créée immédiatement) — on veut pouvoir le conduire tout de suite, sans étape d'activation manuelle. Lors d'une **mise à jour**, c'est la **nouvelle version** qui devient active (l'ancienne reste disponible pour rollback, §4.4).

### 4.7 Packs multi-voitures et entités de premier niveau

Un mod peut contenir **plusieurs voitures** (pack) + des ressources communes (fonts, drivers). Décision :

- **Chaque voiture est une entité de premier niveau** dans la bibliothèque — pas un « gros mod pack » monolithique. Raison : l'unité d'usage est *la voiture* (on l'active, la tague, la met en favori, lui donne catégorie + fiche technique individuellement). Un pack monolithique écraserait cette granularité.
- **Lien de source** : les voitures d'un même pack partagent une référence à leur **source** (nom d'archive, et idéalement URL d'origine — lot futur §12ter). Permet de filtrer « toutes les voitures du pack X », de les désinstaller en lot, de regrouper. Le pack est une **métadonnée de regroupement**, pas une entité.

**Matérialisation visible (à implémenter, pas seulement stocker)** — maquette `pitbox-source-pack.html` :
- Sur la **fiche d'un mod**, un bloc « Source / origine » affiche : le **pack** (avec nb de voitures, cliquable), le **nom d'archive**, et l'**URL d'origine** (vide en v1 → mention « non renseignée, lot L7 »).
- Une section « **autres voitures du même pack** » liste les entités partageant le même `source_pack`, chacune cliquable (navigation entre voitures sœurs). Rappel visible : chaque voiture reste indépendante (activable/tagguable séparément).
- Actions : **filtrer par ce pack**, **désinstaller le pack** (en lot).
- Deux mods « viennent du même pack » ⟺ même valeur `source_pack`. Cette info est **connue dès l'import** (l'app voit l'archive/dossier source) — donc disponible et fiable en v1, contrairement à `source_url`.

```
Mod (overlay, ajout)
  source_pack   : string?   # archive/dossier d'origine commun à un pack
  source_url    : string?   # URL d'origine (rempli par l'extension, §12ter)
```

### 4.8 Ressources partagées (fonts, drivers) et collisions

Les **fonts** (`content/fonts`) et **drivers 3D** (`content/driver`) sont de petits fichiers, souvent partagés entre mods, dans des dossiers globaux.

**Stratégie légère (validée)** :
- Installés **globalement**, **non gérés en activation/désactivation** avec la voiture (les désactiver casserait les autres mods qui les partagent). Coût d'un orphelin = quelques Ko, négligeable.
- **Nettoyage optionnel en L5** : fonction détectant les fonts/drivers qu'aucune voiture installée ne référence, avec proposition de suppression. Ménage ponctuel.

**Détection de collision par contenu** (installation d'un fichier partagé déjà présent) :
- Existant **identique** (même empreinte/taille) → **silencieux**.
- Existant **différent** → **warning** « le mod veut remplacer X par une version différente », choix garder / écraser, **défaut = écraser**.
- Raison : le vrai risque en ligne n'est pas la font mais un **mélange incohérent de versions** (cause réelle de problèmes constatés). Pas de bruit quand les fichiers sont identiques.

**Note checksum en ligne (vérifiée)** : l'anti-triche d'AC checksumme surtout le `data.acd` de la voiture et les `surfaces.ini` du circuit — **pas** les fonts/drivers par défaut. La stratégie légère ne cause donc pas d'éjection. (Exception rare : un admin peut checksummer n'importe quel fichier via "required apps".) Le « réinstaller corrige le kick » vient typiquement de fichiers résiduels incohérents d'une version antérieure — ce que la détection de collision aide à éviter.

---

## 5. Tags et harmonisation

### 5.1 Le problème

Les tags des moddeurs sont incohérents : `hothatch` / `hot hatchback`, `jdm` / `wangan`, `historic` / `vintage`, des dizaines de variantes pour la même notion, plus du bruit (`#rss`, noms de marque en doublon du champ brand, années isolées…).

### 5.2 Objectif : vocabulaire fermé (liste blanche)

L'app ne court pas après le bruit avec une liste noire infinie. Elle définit un **vocabulaire fermé** : l'univers fini des tags qui ont droit d'exister (catégories de course `#gt3`/`#gte`/`#lmp1`…, familles `prototype`/`endurance`/`openwheeler`/`vintage`…, styles `#drift`/`#rally`/`#jdm`…, propriétés CSP `rainfx`…). Tout tag entrant est soit **mappé** vers ce vocabulaire, soit **rejeté**. C'est ça qui harmonise au maximum : un vocabulaire borné, pas une meilleure détection.

### 5.3 Trois origines de tags (tracées séparément)

Conséquence directe de la surcouche non destructive (§3.0) : on ne fusionne pas les tags dans le fichier, donc on peut garder leur **origine** distincte.

1. **Tags du mod** (`tags_from_mod`) — lus dans `ui_*.json`, **lecture seule**. Affichés par défaut, masquables via un réglage global (§10bis).
2. **Tags déduits par règle** (`tags_from_rule`) — calculés par l'ontologie, stockés dans l'overlay.
3. **Tags ajoutés à la main** (`tags_manual`) — saisis par l'utilisateur, stockés dans l'overlay. Les seuls directement supprimables.

L'UI distingue les trois par **code couleur seul** (pas d'en-têtes répétés), avec une **légende discrète** unique. Ordre d'affichage : **catégorie (`#`) → déduits par règle → manuels → fichier mod en dernier**. Les tags du fichier mod sont **masquables** (clic direct + réglage global) — relégués en fin pour qu'on puisse les cacher sans laisser de trou. Couleurs : rouge = catégorie, vert = règle, gris = manuel, bleu = fichier mod.

### 5.4 Ontologie de règles

L'ontologie est un dictionnaire de règles de normalisation. **Quatre types de règles**, gérés dans le même écran et le même moteur (pas de logique cachée dans le code) :

1. **FUSION** — synonymes vers un tag canonique (`hothatch`, `hot hatchback` → `hatchback`).
2. **SUPPRESSION** — retrait du bruit (`#rss`, tags d'auteur, numéros de version → rien).
3. **DÉDUCTION** — tags implicites (`lmp1` → `#lmp1` + `prototype` + `endurance`).
4. **EXTRACTION** (mapping tag → champ technique) — « quand tu vois tel tag, écris telle valeur dans tel champ de la fiche technique » (ex. `turbo` → `aspiration=TURBO` ; `rwd`/`propulsion` → `drivetrain=RWD`). Même nature que les autres règles, mais la **cible est un champ structuré** (§5bis.1) et non un tag. L'éditeur, pour ce type, demande le champ destination + la valeur.

C'est ce 4e type qui répond à « comment remplir automatiquement les specs depuis les tags » : **par des règles éditables, pas du code en dur.** Bénéfice : quand un moddeur écrit `propulsion` au lieu de `rwd`, on ajoute le synonyme soi-même sans recompiler.

> Cette ontologie est l'actif le plus précieux et le plus difficile à reconstruire : elle doit être **données, pas code** — fichier de règles éditable, versionnable. Une **page graphique de gestion des règles** est prévue (§ maquette validée).

**Règles par défaut livrées avec l'app.** Une ontologie vide est inutile : l'app embarque un **jeu de règles par défaut**, que l'utilisateur amende ensuite. Ce jeu est **extrait de `archives.py`** puis enrichi par l'analyse du catalogue réel, fourni dans `default-tag-rules-enriched.json`. Il couvre **cinq familles de règles** — dont une que la conception initiale n'avait pas anticipée :
- **brand_fix** : correction de marque depuis le nom (`bayro`→BMW, `darche`→Porsche…).

> **Catégories MO2 retirées.** La famille `mo2_category_map` (mapping des anciennes catégories Mod Organizer 2 → tags) a été **supprimée** : usage trop spécifique (un seul utilisateur), et les tags corrects ont déjà été réécrits dans les mods. Inutile d'alourdir l'ontologie et l'écran de règles.

Cette famille s'ajoute aux quatre types généraux (fusion, suppression, déduction, extraction). L'écran de règles gère donc cinq types (brand_fix pouvant être un cas particulier).

**Aperçu d'impact.** Modifier/ajouter une règle peut affecter des dizaines de mods d'un coup. Avant validation, l'éditeur affiche le **nombre de mods affectés** (« 23 voitures affectées ») — garde-fou absent quand les règles sont en dur.

### 5.5 Comportement

- **Harmonisation automatique** à l'import (applique l'ontologie) — l'utilisateur n'intervient qu'en exception.
- **Édition manuelle** des tags (origine « manuel ») à tout moment.
- **Détection CSP** : lecture des `ext_config.ini` / configs d'extension pour ajouter `rainfx`, `grassfx`, `weatherfx`, `lightingfx`, `has-skins`.
- **Recherche et filtres** par tag, marque, type, catégorie, année, auteur.

### 5.6 (Exploration, non spécifié) Enrichissement en ligne

Idée à creuser : récupérer des infos sur un mod depuis le web pour auto-compléter les tags, en n'acceptant que des tags du vocabulaire fermé. Non trivial : pas de source faisant autorité avec des tags structurés, matching mod local ↔ fiche en ligne incertain, gestion des erreurs de source. Gardé en exploration, non conçu en v1.

---

## 5bis. Catégorie et composition de plateau

La **catégorie** n'est pas un tag libre : c'est, par **convention Content Manager, le tag canonique préfixé `#`** (ex. `#gt3`, `#gte`, `#lmp1`, `#dtm`). CM lit le tag commençant par `#` comme catégorie de la voiture. C'est rétrospectivement la raison d'être des hash dans l'ancienne ontologie Python (`#gt3`, `#lmp1`…) : ils **marquent la catégorie**, ils ne sont pas décoratifs.

Implications pour l'app :
- L'ontologie (vocabulaire fermé, §5.2) doit garantir qu'une voiture possède un tag `#` **principal** identifiant sa catégorie.
- La catégorie combine, pour la composition de plateau, ce tag `#` **+ la fenêtre d'années** (ex. « `#gt3` · 2018-2023 »).
- Sa fonction n'est pas le rangement mais la **composition de plateau** — « quelles voitures roulent bien ensemble ». Utilisée surtout à la préparation de course (§8) : on choisit une catégorie, l'app propose le sous-ensemble cohérent (même tag `#` + années proches), et c'est dans ce vivier qu'on pioche voiture et grille d'IA.

La classe `race`/`street` reste un **champ de métadonnée/affichage** (filtrage, rangement) — et **pas** le levier physique de CSP (§5ter).

### 5bis.1 Fiche technique : champs dédiés (≠ tags)

Décision de conception : les **caractéristiques mécaniques** d'une voiture ne sont PAS des tags. Mettre `turbo`, `rwd`, `v8`, `mid-engine` dans les tags est un abus : un tag sert à *filtrer/grouper* (catégorie, style, époque), pas à *décrire une fiche technique*. Les y laisser pollue le vocabulaire de tags et empêche de s'en servir proprement.

**Champ `year` — stratégie de résolution.** Le champ `year` n'est PAS d'origine dans les `ui_car.json` de Kunos : il a été ajouté par l'écosystème AcTools/CM, qui le récupère depuis une base en ligne et le met en cache localement (`AppData\Local\AcTools Content Manager`). Pit Box **ne dépend pas** de cette base (non documentée, fragile, liée à CM). Stratégie à trois niveaux :
1. Lire `year` de l'`ui_car.json` **s'il est présent** (cas des mods bien renseignés).
2. Sinon, pour le **contenu de base** (`is_stock`), chercher dans une **table statique embarquée** (`kunos_content_dates.json`, ~150 voitures + ~36 circuits, clé = nom de dossier) — données stables, jamais à maintenir.
3. Sinon, afficher **« — »**.
La table Kunos est un point de départ à valider contre l'installation réelle (noms de dossiers pouvant varier légèrement selon versions) ; correction triviale ligne par ligne si besoin.
- Champ **`specs`** : c'est un **OBJET structuré**, pas une string. Clés présentes à ~100% : `bhp`, `torque`, `weight`, `topspeed`, `acceleration`, `pwratio`, parfois `range`. **À lire directement** — aucun parsing de chaîne nécessaire. (La ligne « 190 bhp, 180 Nm… » des captures était le *rendu* de CM, pas la donnée brute.)
- **Courbes `powerCurve` / `torqueCurve`** : présentes sur **688/689** fichiers. La courbe moteur est donc réalisable sur quasiment toutes les voitures, pas un cas rare. À tracer (graphe Nm/bhp/RPM).
- Champ **`description`** : présent à ~96%. Affiché à la demande (bouton « Description »).
- Champs natifs **`country`**, **`author`**, **`version`** : présents sur la grande majorité. À exploiter (country alimenté par extraction si vide, voir ci-dessous). Pour **`year`**, voir la stratégie de résolution ci-dessus (souvent absent du contenu de base → table Kunos).

**Champs structurés complémentaires** (overlay) — uniquement pour ce que `specs` ne couvre pas :

```
CarSpecs (overlay, voitures) — complète l'objet specs natif (qui a déjà bhp/torque/weight/topspeed/accel/pwratio)
  drivetrain   : enum?  { RWD, FWD, AWD }
  engine_pos   : enum?  { FRONT, MID, REAR }
  aspiration   : enum?  { NA, TURBO, SUPERCHARGED }
  engine_config: enum?  { V6,V8,V10,V12,I4,I6,FLAT,ROTARY,ELECTRIC,HYBRID,DIESEL }
  gearbox      : enum?  { MANUAL, SEQUENTIAL, SEMIAUTO, AUTO, DCT }
```

Ces champs sont remplis par la **famille de règles EXTRACTION** (§5.4), dérivée de l'analyse du catalogue réel : 25 règles sur 5 champs, couvrant les tags techniques massivement présents (rwd ×490, manual ×273, sequential ×209, turbo ×96…). Une fois extraits, ces tags techniques sont **retirés du vocabulaire** (ajoutés à la liste de suppression) — ils ne réapparaissent pas comme tags.

**Extraction du pays.** Même principe que la technique : un tag pays (`italy`, `germany`…) est du bruit dans les tags, mais l'information est utile. Règle : si un tag correspond à un pays **et** que le champ natif `country` est vide, remplir `country` ; puis retirer le tag. (27 pays mappés.)

**Remplissage en cascade** (sans traçage d'origine — seule la valeur compte, ou « à compléter » si vide) :
1. `spec`, courbes, `description` lus de `ui_car.json`.
2. **Déduction depuis les tags du mod** pour les champs structurés (`turbo`/`rwd`/`v8`… → champ correspondant). Couverture partielle.
3. **Édition manuelle** des champs restants, marqués « à compléter ».

> **Enrichissement web — prévu, non implémenté en v1.** L'architecture réserve la place d'une source web pour compléter (et idem tags, §5.6), sans la livrer en v1 (fiabilité non maîtrisée). Le modèle de données et l'UI doivent pouvoir l'accueillir sans refonte.

- **Conséquence sur l'ontologie** : notions techniques retirées du vocabulaire de tags (mais lues au passage pour alimenter les champs).
- **Affichage** : bloc « fiche technique » distinct du bloc tags. Grille de valeurs simples, pas de code couleur d'origine.

### 5bis.1bis Pistes futures — « plonger dans la voiture » (hors v1, à conserver)

Idées notées pour ne pas les perdre, à détailler ultérieurement. Panneau d'actions sur la fiche d'une voiture :
- **Wikipedia** : ouvrir la fiche du modèle réel.
- **Ouvrir dans le showroom** d'AC (studio 3D de CM) pour inspecter le modèle.
- **Poser des questions à une IA** sur ce modèle (histoire, palmarès, specs réelles, anecdotes).
- **Mode découverte cinématographique** (ambitieux) : lancer une session où **l'IA conduit**, caméra TV façon retransmission, avec **lecture vocale de la fiche Wikipedia** en fond pendant que la voiture tourne.

Ces fonctions transformeraient la fiche en point d'entrée encyclopédique/expérientiel, au-delà de la simple gestion. Non spécifiées en détail, conservées comme cap.

### 5bis.2 Favori : état personnel distinct

Le **favori** n'est ni un tag ni une caractéristique : c'est un **état personnel** (j'aime / j'aime pas), togglable d'un clic. Représenté par un cœur près du nom, **séparé du système de tags** (où il figurait à tort comme tag manuel). Trois natures bien distinctes : tags = description, fiche technique = mécanique, favori = rapport personnel au mod.

```
Mod (overlay, ajout)
  is_favorite  : bool
```

---

## 5ter. Note physique CSP (à ne pas confondre)

Pour mémoire, afin d'éviter une erreur de conception récurrente : le champ `class` de l'`ui_car.json` ne pilote pas la physique. Référence : la physique étendue se déclenche au niveau `car.ini` (`[HEADER] VERSION=extended-N`) et/ou par l'option globale de CSP. L'app n'a pas à manipuler ça en v1.

---

## 6. Bibliothèque (UI)

### 6.1 Bibliothèques Voitures et Circuits (distinctes)

**Voitures et circuits sont deux bibliothèques distinctes, jamais mélangées** : chacune a sa propre vue avec ses propres colonnes. Raison : un tableau mixte est condamné au plus petit dénominateur commun (nom, auteur, tags) et perd les attributs spécifiques de chaque type.

> ⚠️ Écart constaté à corriger : une implémentation regroupant tout sous une seule « Bibliothèque » avec un tableau mixte voitures+circuits est incorrecte.

**Accès** : on n'ouvre plus ces bibliothèques via des entrées de menu « Voitures »/« Circuits » séparées, mais via le **bloc Session** de la barre latérale (§6.1ter) — cliquer la preview voiture ouvre la bibliothèque voitures, cliquer la preview circuit ouvre la bibliothèque circuits.

### 6.1ter Barre latérale unifiée (écran principal)

Layout de référence : maquette `pitbox-biblio-session2.html`. Une **colonne latérale unique** regroupe la session et la navigation :
- **Bloc SESSION en haut** (prend la place des anciennes entrées Voitures/Circuits) : previews du **duo sélectionné** (voiture + circuit), chacune **cliquable pour ouvrir la bibliothèque** correspondante (le bloc Session EST le point d'accès aux bibliothèques). Bouton **« Démarrer une session »** qui ouvre l'écran de réglages dédié à droite (§8.6). Le menu montre *quoi* (sélection courante) ; l'écran dédié gère *comment* (réglages). Pas de paramétrage de session dans le menu.
- **ADD-ONS** (titre style « Session » : rouge, mono, séparateur) en **deux colonnes** : Skins | Sons, puis Apps, puis **Autres mods** (§6.1bis).
- **ATELIER** (même style) en deux colonnes : Règles | Importer, puis Réglages (extensible — d'autres outils viendront).

### 6.1bis Type « Autres mods »

Un mod importé qui n'est **ni** voiture, circuit, skin, son ou app reconnu (ex. shaders, configs CSP, mods d'UI, weather patterns, mods de physique globale) n'est plus perdu : il est listé dans une vue **« Autres mods »** (entrée dans ADD-ONS). Cohérent avec la philosophie « ne jamais perdre un mod ».
- **Activables/désactivables** comme les autres (par junction), même si le type précis est inconnu.
- **Surcharges & priorité** : ces mods sont souvent des **surcharges** de contenu existant. L'app **enregistre l'intention de priorité** (marquer un mod « autre » comme prioritaire) et **détecte/signale les conflits** de fichiers (« ce mod surcharge des fichiers de tel autre »). Le mod marqué prioritaire est appliqué en dernier → ses fichiers l'emportent.
  - ⚠️ *Limite assumée* : les junctions n'offrent pas un vrai moteur de superposition ordonné comme le VFS de MO2. On vise, en v1, **priorité notée + détection de conflits**, pas une résolution automatique complète par couches. (Un moteur de priorités type MO2 serait un gros chantier séparé.)

### 6.2 Deux vues commutables (par bibliothèque)

- **Galerie** — vignettes de preview (le `preview.png` des skins voitures, l'`outline`/preview des circuits). Visuelle, faite pour parcourir.
- **Tableau** — dense, **colonnes sélectionnables par l'utilisateur, propres à chaque type** (le choix de colonnes est mémorisé indépendamment pour voitures et circuits). Tri par colonne.
  - *Colonnes voitures* : nom, marque, catégorie (tag #), classe (race/road), année, auteur, version, tags, état (actif/inactif), distance, **date d'ajout**, **date de mise à jour**, date d'import.
  - *Colonnes circuits* : nom, layouts, longueur, nombre de virages, extensions CSP, auteur, version, tags, état, distance, **date d'ajout**, **date de mise à jour**, date d'import.
  - Un sélecteur de colonnes (bouton « Colonnes ») permet de cocher celles à afficher ; la sélection persiste par type.

**Colonnes de dates (sur tous les types de mods : voitures, circuits, et add-ons skins/sons/apps)** — trois dates, à fiabilité distincte :
- **Date d'ajout** : quand l'utilisateur a importé le mod la première fois. **Fiable** (posée à l'import). Toujours remplie.
- **Date de mise à jour (par l'utilisateur)** : dernière fois qu'une nouvelle version a été réimportée par-dessus dans l'app. **Fiable**. Égale la date d'ajout si jamais mis à jour.
- **Date de publication** : date estimée de sortie du mod. **Alimentée dès l'import** par la **date de modification** des fichiers — de l'archive (dates internes, lues avant décompression, plus fiables car non corrompues par l'extraction) ou du dossier (date de modification des fichiers) selon la source d'import. C'est une **estimation** (elle mesure quand les fichiers ont été écrits, pas la publication officielle), jugée suffisamment proche en pratique (vérifié sur cas réels). Le champ n'est donc **plus vide en v1**. **Pour le contenu de base** (`is_stock`), ne pas utiliser la date des fichiers Steam (= date d'installation, sans intérêt) mais la **date de sortie dans AC** issue de `kunos_content_dates.json` (champ `release`, daté par pack : jeu de base, Dream Packs, Porsche Packs, etc.). Si un jour l'**extension L7** fournit la vraie date de publication (lue sur la page d'origine), elle **remplace** l'estimation (source plus fiable). Le libellé pourra être affiné (« estimée ») ultérieurement — champ unique, source qui s'améliore.

Les deux premières dates sont des colonnes fonctionnelles dès maintenant, sur tous les types. La troisième s'enrichit avec L7.

Bascule d'un mode à l'autre par un toggle persistant.

### 6.3 Panneau / page de détail

Deux présentations selon le contexte :
- **Panneau latéral** (depuis la liste) : preview, métadonnées, versions (active mise en évidence), tags éditables, historique, actions.
- **Page de détail pleine page** (layout de référence : maquette `pitbox-fiche-B-revisee.html`) — pour exploiter les grands écrans. Structure validée :
  - *Rangée haute* : **image héros large à gauche** (preview de la voiture en grand, specs natives en surimpression, badge « fichier non modifié ») + **panneau données à droite** (fiche technique en grille, courbe moteur power/torque, **description dépliable** derrière un bouton — jamais affichée en permanence).
  - *Rangée basse, 3 colonnes* : **Skins** (sélection/prévisualisation, étoile = piloté — voir §12bis.2, pas d'activation) | **Distance puis Son moteur** (§6.5 et §12bis.2) | **Tags** (par origine, §5.3) + **Versions** + **Historique**.
  - Un peu de scroll est acceptable : l'objectif est d'exploiter la largeur, pas le zéro-scroll absolu.

### 6.3bis Interactions de sélection

**Clic simple / double-clic** :
- **1 clic** = **sélection** : affiche le mod dans le panneau de droite ET le définit comme choix de session (voiture ou circuit courant, §8.6).
- **Double-clic** = ouvre les **détails** (page/fiche complète), où l'on peut en plus choisir le **skin piloté**.

**Skin piloté persistant** : le skin choisi pour une voiture est **mémorisé** (préférence durable de l'utilisateur pour cette voiture), **affiché dans la liste** (pastille/miniature du skin sur la vignette) et rappelé dans le bandeau de session. Il devient une donnée de la voiture, pas un choix éphémère.

**Sélection multiple (Ctrl / Alt)** : plusieurs mods sélectionnés → le panneau de droite bascule en **mode édition groupée** : il n'affiche plus les détails d'un mod mais uniquement les **champs applicables à tous**. Champs d'édition en masse :
- **Tags** : ajouter / retirer des tags sur toute la sélection.
- **Activation** : activer / désactiver en masse.
- **Suppression** : désinstaller en masse.
- **Favori** : marquer / démarquer en masse.
- **Catégorie** : assigner le même tag # à tout le groupe (utile après un gros import).
- **Export** : générer des archives autonomes de la sélection.
Champs propres à une voiture (specs, skin piloté, version active) ne sont **pas** proposés en masse (aucun sens de leur donner une valeur commune).

### 6.4 États visuels

- Mod **actif** : marqueur clair (la junction existe dans `content/`).
- Mod ayant **plusieurs versions** : badge indiquant le nombre, version active distinguée.
- Mod **cassé / incomplet** : signalé (ex. pas d'`ui_*.json`, structure invalide).

### 6.5 Suivi d'usage : distance et « jamais essayé »

Filtre/donnée pour retrouver ce qu'on n'a pas encore exploré (utile à 500+ mods).

**Donnée affichée** : la **distance parcourue** par voiture et par circuit — pas un simple booléen. Affichée sur la fiche, **colonne triable** dans le tableau (trier par km croissants fait remonter les peu/pas explorés), et **filtre « jamais essayé »**.

**Deux sources combinées** (l'une enrichit, l'autre fiabilise) :
1. **CM / CSP** : tient le kilométrage par voiture et par circuit, stocké dans `%userprofile%\AppData\Local\AcTools Content Manager`. Donne l'historique d'avant l'app, mais **fragile** :
   - le kilométrage **se réinitialise à la mise à jour d'un mod** (donc « 0 km » ≠ forcément « jamais essayé ») ;
   - le comptage **échoue silencieusement** si le nom de dossier a majuscules/espaces/tirets (faux zéros fréquents) ;
   - fiabilité variable selon la version de CSP.
2. **Suivi propre de l'app** : l'app pose un marqueur **« déjà essayé » définitif** dès qu'elle lance une session avec une voiture/un circuit. Information contrôlée, qui ne ment pas et survit aux resets CM et aux dossiers mal nommés.

**Règle du filtre** : un mod est « jamais essayé » **seulement si** CM affiche 0 km **ET** l'app ne l'a jamais lancé. La combinaison corrige les faux zéros de CM.

**Limite assumée** : le suivi propre ne capture que les sessions lancées **via l'app**. Une session lancée directement dans CM (bouton « Ouvrir dans CM », §12bis.5) n'est vue que par CM. Les deux sources ensemble couvrent presque tout ; aucune seule n'est complète.

> **À vérifier (implémentation)** : format exact du stockage des stats dans `AppData\Local\AcTools Content Manager` — `Values.data` binaire (difficile) ou fichier séparé plus accessible ? Conditionne la difficulté de lecture du kilométrage CM.

---

---

## 7. Activation / désactivation

- **Activer** un mod = créer une junction `content/<type>s/<id>` → `bibliothèque/.../<version active>`.
- **Désactiver** = supprimer la junction (le contenu reste intact dans la bibliothèque).
- **Changer de version active** = supprimer la junction et la recréer vers une autre version.
- **Profils** : activer/désactiver en masse un ensemble nommé (« GT3 endurance »). Application d'un profil = réconciliation des junctions pour correspondre exactement au set.

> Garde-fou : l'app ne doit jamais supprimer un dossier réel de `content/` qui ne serait pas une junction qu'elle a créée (protection contre la perte de contenu installé hors de l'app). Détection junction vs dossier réel obligatoire avant toute suppression.

---

## 8. Lancement de session

### 8.1 Principe

On ne reproduit pas le moteur de configuration de CM. Les réglages lourds (graphismes CSP/Sol/Pure, FFB, contrôleur) restent dans des **presets CM** configurés une fois. L'app ne pilote que ce qui change d'une session à l'autre.

### 8.2 Ce que l'app pilote

Voiture · circuit (+ layout) · mode (practice / hotlap / race / weekend) · nombre d'IA · niveau d'IA · météo/heure (si exposé).

### 8.3 Mécanique — pilotage par presets (et non écriture de fichiers)

**Fait technique vérifié et structurant.** CM est **maître de `race.ini`** : à chaque session il réécrit `race.ini` puis le restaure, et stocke ses réglages dans un format binaire (`%userprofile%\AppData\Local\AcTools Content Manager\Values.data`). Tenter de dicter les paramètres en écrivant `race.ini` ne fonctionne pas — CM les écrase. Le circuit peut parfois être changé par fichier, la voiture non, et rien ne marche de façon fiable côté CM par cette voie.

**Conséquence sur le modèle.** L'app ne fixe pas les réglages par écriture de fichiers : elle **pilote des presets CM**. CM est conçu pour ça (presets de session sauvegardables couvrant voiture, circuit, météo, heure, grille, assists, carburant, usure, pénalités, format).

**Valeurs par défaut.** Il n'y a pas de « défaut nul » : un champ non spécifié prend la valeur du **preset actif / dernier état** (dans `Values.data`). Le preset fait foi.

**Mécanique de lancement.**
- CM démarré en service + Steam ouvert, puis émission de la commande — séquence nécessaire pour court-circuiter l'UI d'AC et tomber directement en session.
- L'app orchestre cet ordre avant d'émettre la commande.

> **POINT OUVERT CRITIQUE (bloque le lot L4)** : *comment* l'app sélectionne/active un preset CM par programmation, et la syntaxe exacte du protocole `acmanager://` / des arguments CLI. Les presets sont confirmés comme le bon levier, mais la commande exacte pour en activer un reste à vérifier sur la doc primaire AcTools. Trois pistes à investiguer : protocole `acmanager://`, manipulation de `Values.data`, argument CLI. Ne pas figer le module de lancement avant cette vérification.

### 8.4 Presets de session par type

Chaque **type de session** (Practice, Hotlap, Course, Weekend) possède un **preset mémorisé**. Règles :

- Les réglages du preset sont **toujours modifiables** ; toute modif est **persistée pour les prochaines sessions du même type** (activer les dégâts en course une fois → ils restent actifs pour les courses suivantes).
- La **liste des réglages affichés dépend du type** : certains ne sont pertinents que pour un type (ex. *ghost car* en hotlap uniquement), d'autres sont universels (ex. *dégâts*, toujours visibles).
- Mappés sur des presets CM (§8.3).

### 8.5 Météo simplifiée à dégradé gracieux

Principe : l'utilisateur choisit une **intention** (« Pluie »), l'app la traduit dans le **meilleur backend disponible**.

- Presets clairs façon jeu grand public : Beau, Couvert, Pluie, Orage, Coucher… (pas de réglage abstrait type « intensité »).
- **Résolution selon la stack détectée** : Pure présent → pluie via Pure ; sinon SOL → pluie SOL ; sinon weather FX de CSP → pluie CSP ; sinon vanilla (pas de pluie → preset indisponible ou dégradé en « couvert »). Une mention discrète indique le backend retenu (« via Pure »).
- Un preset que la stack ne sait pas rendre est **grisé** avec l'explication (« nécessite SOL ou Pure »).
- **Température implicite** : déduite de (condition + heure [+ saison]), affichée en lecture seule (« ~24°C air · 31°C piste »), jamais saisie — contrairement à CM qui demande air ET piste.
- **Heure du jour** : curseur simple.
- **Périmètre v1** : météo **statique** (fixe sur la session) + heure. Le **dynamique** (météo évoluant en course) est laissé à SOL pendant la session pour les power-users — l'app ne cherche pas à l'unifier (les leviers SOL/Pure diffèrent).

> **À vérifier** : détection fiable de la stack (Pure/SOL/CSP/vanilla) au démarrage, et correspondance preset → réglages backend. Dépendance du module météo.

### 8.6 Architecture : la bibliothèque EST le sélecteur de session

**Décision d'unification (revient sur le modèle "deux mondes" antérieur).** Il n'y a pas d'écran séparé pour choisir la voiture/le circuit d'une session : **la bibliothèque elle-même est le sélecteur**. La voiture ouverte/sélectionnée dans la bibliothèque voitures = la voiture de la session ; idem pour le circuit. Raison : dupliquer la sélection dans un écran de session distinct recréait deux fois les mêmes listes (doublon repéré à l'usage). La bibliothèque (grille de previews, ou tableau dense au choix) fait déjà très bien ce travail — inutile de la refaire ailleurs.

**Mécanique de sélection** :
- **Ouvrir/cliquer une voiture** dans la bibliothèque la définit comme voiture de session (pas d'action distincte type « sélectionner pour la session » — le simple usage suffit). Idem circuit dans la bibliothèque circuits.
- Le **skin piloté** se choisit dans la fiche de la voiture (§12bis.2).

**Bandeau persistant de session** (élément clé) : un bandeau **visible en permanence depuis la bibliothèque** affiche le **duo actuellement sélectionné** — voiture + circuit (avec preview miniature) — plus un accès direct aux réglages/lancement. C'est le fil rouge qui indique toujours « voici ce qui partira au lancement ». Sans lui, l'utilisateur ne saurait plus quel est le choix courant. (À NE PAS confondre avec un historique « dernière voiture de session » : c'est bien la *sélection courante* dans la bibliothèque.)

**Page « Démarrer une session »** : ne contient **plus aucune sélection de voiture/circuit**. Elle réunit :
- un **rappel du duo sélectionné** (voiture + circuit + previews) ;
- les **réglages de la session** (voir §8.4) ;
- le bouton **Lancer**.
Pour changer de voiture/circuit, l'utilisateur retourne dans la bibliothèque et en ouvre un autre.

**Réglages selon le type de session** (rappel §8.4 — écart constaté dans l'implémentation initiale à corriger) : les réglages affichés **dépendent du type** (ghost car en Hotlap uniquement ; dégâts/usure/carburant en Course ; etc.), et sont **persistants par type**. L'écran ne doit PAS afficher un bloc d'options fixe identique pour tous les types.

**Réglages — contenu** (selon le type) :
- *Toujours pertinents* : météo (température implicite), heure.
- *Course* : grille d'adversaires (plateau cohérent par défaut / libre ; nb IA, force), durée (tours OU temps), dégâts, usure, carburant, + options repliables (départ arrêté/lancé, position, qualif, grip, assists).
- *Hotlap* : ghost car, + réglages pertinents.
- **Hors périmètre** (rester simple) : pénalités détaillées, limites de piste/drapeaux, tyre blankets, ballast/restrictor.

**Valeurs par défaut** (si aucune sélection encore faite, ou cible disparue) : voiture = première du vivier ou dernière ouverte si encore installée ; circuit = dernier si présent, sinon premier alphabétique ; type = dernier utilisé sinon Practice ; réglages = derniers du type courant. Validées à chaque ouverture (une cible disparue déclenche le repli).

**Choix du layout et du skin de circuit** : sur la fiche/bibliothèque circuit — **image d'aperçu du circuit en fond** (`preview.png`) avec le **tracé du layout sélectionné par-dessus** (`outline.png`/`map.png`), infos (longueur, virages, CSP). Un circuit a souvent plusieurs layouts, chacun avec son tracé. Ne pas se limiter au nom du layout — l'illustration est essentielle.

---

## 9. Maintenance et export

### 9.1 Export d'archive autonome (fonctionnalité réhabilitée)

Repackager un mod en archive **autonome et complète** — utile pour sauvegarde, repartage, ou migration d'install. Pour une voiture, cela implique d'embarquer non seulement le dossier de la voiture mais aussi ses **dépendances éparpillées** : pilotes 3D (`content/driver/*.kn5`), polices custom (`content/fonts/*`), crews.

C'est la **seule** fonction qui justifie de lire le `data.acd` chiffré (voir §9.2).

### 9.2 Extraction acd.bms — cantonnée à l'export

- Le `data.acd` d'une voiture (chiffré, clé = nom du dossier) contient `driver3d.ini` et `digital_instruments.ini`, qui référencent les pilotes et polices custom à embarquer.
- Cette extraction (historiquement via QuickBMS + script `acd.bms`) est **lente et fragile** (le `.acd` doit rester dans son dossier d'origine, son nom servant de clé).
- **Décision** : elle ne fait PAS partie du chemin critique d'import/activation. Elle est isolée dans le module d'export, et n'est sollicitée que lorsqu'on demande explicitement « exporter un mod voiture autonome ».

### 9.3 Nettoyage

Détection et suppression assistée des mods cassés : voitures sans `ui/`, circuits sans contenu valide, junctions orphelines pointant vers une version supprimée.

---

## 10bis. Paramètres de l'application (préférences persistantes)

Réglages globaux, persistants entre sessions (et entre mods pour les bascules d'UI) :

- **Affichage des tags du fichier mod** : affichés par défaut ; option pour les masquer complètement (pour qui ne veut voir que ses tags harmonisés).
- **Panneau de suivi (versions/historique) sur la fiche mod** : repliable ; l'état (ouvert/fermé) est mémorisé **globalement**, pas par mod — si on le replie, il reste replié en ouvrant d'autres fiches.
- **Vue bibliothèque** : galerie vs tableau, et colonnes sélectionnées en mode tableau — persistants.
- **Presets de session** par type (voir §8.4) — persistants par type.
- **Preset CM graphique/FFB** par défaut à appliquer au lancement.

---

## 10. Découpage en lots (proposition)

| Lot | Contenu | Pourquoi |
|---|---|---|
| **L1 — Bibliothèque & identité** | Import multi-format, descente récursive, moteur d'identité, résolution de version, historique, vue galerie+tableau, **base d'overlay non destructive** | C'est le cœur ; tout le reste s'appuie dessus. |
| **L2 — Tags & organisation** | Vocabulaire fermé, ontologie (données) + **page graphique de règles**, harmonisation auto, **trois origines de tags**, détection CSP, recherche/filtres, **classe + année + catégorie** | Donne sa valeur quotidienne à la bibliothèque. |
| **L3 — Activation & profils** | Junctions, états actif/inactif, profils, garde-fous | Rend l'app opérationnelle pour jouer. |
| **L4 — Lancement** | **Pilotage par presets CM** (point ouvert §8.3), orchestration CM+Steam, presets de session par type, météo adaptative, écrans session + course | Ferme la boucle « de la biblio au circuit ». |
| **L5 — Maintenance & export** | Export autonome (+ acd.bms isolé), nettoyage, sauvegarde | Confort et pérennité, non bloquant. |

---

## 11. Matière première à porter (depuis le code existant)

Le code Python actuel n'est pas réutilisé comme runtime, mais ces éléments encodent des cas réels à reproduire dans le nouveau backend :

- **Détection de type** — règles `isCar` (présence `ui/ui_car.json`), `isTrack` (`ui_track.json` à la racine ou dans un sous-dossier de layout), `isMod` (présence de `content`/`extension`), `isCarSound` (`.bank` + `GUIDs.txt`).
- **Descente récursive** — gestion des archives à racine décalée, mods imbriqués, plusieurs mods dans une même archive.
- **Ontologie de tags** — l'ensemble des règles de normalisation (synonymes, suppression de bruit, déductions implicites). **À extraire en fichier de données.**
- **Détection CSP** — lecture des sections `GRASS_FX`, `RAIN_FX`, `LIGHT_SERIES_1`, `SEASON_WINTER` dans les configs d'extension.
- **Dépendances voiture** (pour l'export only) — résolution des pilotes 3D et polices via `driver3d.ini` / `digital_instruments.ini` extraits du `data.acd`.
- **Listes Kunos** — identifiants des voitures/circuits/polices/pilotes natifs, à exclure du packaging et de certains traitements.

---

## 12. Configuration de l'application

L'app a besoin, dès le premier lancement, de connaître son environnement. Inspiré de l'ancien `configuration.ini` (`params.py`), mais formalisé.

**Chemins requis :**
```
config
  ac_install_path     : path   # dossier d'install Assetto Corsa (contient content/)
  library_path        : path   # dossier de travail = bibliothèque (source de vérité, ~300 Go)
  content_manager_exe : path   # exécutable Content Manager (pour le lancement)
  sevenzip_exe        : path   # 7-Zip (extraction rar/zip/7z)
  quickbms_exe        : path?  # QuickBMS — optionnel, requis seulement pour l'export acd.bms
  acd_bms_script      : path?  # script acd.bms — idem
```

**Comportement :**
- **Assistant de première configuration** au premier démarrage : détection automatique si possible (chemins Steam habituels pour AC, install CM connue), sinon saisie manuelle avec validation (le dossier `content/` existe-t-il ? CM est-il là ?).
- **Écran Réglages** ensuite : modifier ces chemins, plus les préférences de §10bis (affichage tags mod, état panneau de suivi, vue biblio, presets de session, preset CM par défaut), plus les réglages météo (stack détectée).
- **Validation** : avant toute opération de junction, vérifier que `ac_install_path/content` est accessible en écriture. Avant export, vérifier la présence de QuickBMS.
- **Stockage** : fichier de config local (format au choix de l'implémentation : TOML/JSON), séparé de la base d'overlay et de la base de règles.

**Trois bases/fichiers distincts à ne pas confondre :**
1. **Bibliothèque** (fichiers des mods, disque dédié) — source de vérité des fichiers.
2. **Base d'overlay** (métadonnées produites par l'app : tags, specs, favori, historique, profils) — indexée sur l'empreinte.
3. **Fichier de règles** (`default-tag-rules.json` au départ, puis amendé) — l'ontologie.
Plus le **fichier de config** (chemins + préférences).

---

## 12bis. Lot L6 — Types de mods étendus (évolution post-v1)

Lot conçu après la v1 initiale. Introduit un changement de modèle : l'app cesse de raisonner uniquement en « mods autonomes » et gère des **entités** (voitures/circuits) auxquelles s'**attachent** des sous-éléments, plus des types de mods nouveaux.

### 12bis.1 Indexation du contenu de base (Kunos)

L'app **indexe le contenu de base** d'Assetto Corsa (voitures et circuits Kunos présents dans `content/`), avec un statut spécial :
- **Lecture seule, non désactivable, non supprimable** — il appartient au jeu, l'app n'y touche jamais (pas de junction, pas de déplacement).
- Marqué visuellement « contenu de base » / « Kunos ».
- **Raison d'être** : permettre à des sous-éléments (skins, sons) de s'attacher à une voiture/circuit de base, pas seulement à un mod. L'app raisonne en « voitures connues » (mod OU base), pas en « mods » uniquement.
- Identité : même mécanisme d'empreinte (§4.1), avec un flag `is_stock`.

### 12bis.2 Sous-éléments rattachés : skins et sons

Un skin ou un son n'est **pas autonome** : il s'attache à une voiture (ou circuit) existante. Modèle :

```
SubMod (overlay)
  id            : uuid
  type          : enum { SKIN, SOUND, TRACK_SKIN, TRACK_MOD }
  parent_id     : string   # empreinte de la voiture/circuit cible (mod OU stock)
  library_path  : string
  source_archive: string
  # SKIN : pas de champ d'activation (voir ci-dessous)
  # SOUND : is_active (un seul son actif par voiture)
```

**Asymétrie fondamentale (à respecter dans l'UI et la mécanique) — les skins et les sons ne se gèrent PAS pareil :**

- **Skins = AUCUNE activation filesystem.** Un skin est juste un sous-dossier dans `skins/`. Il ne gêne pas, ne crée pas de conflit, ne coûte rien à laisser en place — AC les charge tous. **Il n'y a donc rien à activer/désactiver.** Tous les skins présents sont disponibles. La seule action utile est la **sélection** : (a) *prévisualiser* un skin (le voir), et (b) désigner le **skin piloté** au lancement (celui utilisé quand on clique « Conduire »). Pas de case à cocher d'activation — une sélection de prévisualisation + une désignation « piloté » (une étoile). *(Correction d'un modèle initial erroné qui prévoyait une activation additive avec un champ `active` : abandonné.)*
- **Son = exclusif / unique, vraie bascule de fichiers.** Une voiture n'a **qu'un seul** son moteur à la fois. Activer un mod son = **remplacer** réellement le son courant (dossier `sfx` : `.bank` + `GUIDs.txt`, détecté par `isCarSound` dans `archives.py`). Choix « radio », pas accumulation. C'est la seule des deux qui touche vraiment aux fichiers.
- **Restauration du son d'origine** : activer un son sauvegarde le son précédent (base ou mod) pour pouvoir y revenir. L'app ne détruit jamais le son d'origine de façon irréversible.

En résumé : **skin = choix d'affichage** (sélection/prévisualisation/piloté), **son = bascule de fichiers** (exclusive, réversible).

**Import des skins** : pas de bouton d'import sur la fiche. Un pack de skins entre par l'**import général** (c'est un mod comme un autre) et se rattache automatiquement à la bonne voiture via le nom de dossier cible qu'il contient (`skins/<voiture>/`). La fiche sert à *voir et choisir* les skins, l'import général à *les faire entrer*.

**Circuits** : même logique avec `TRACK_SKIN` (skins de circuit) et `TRACK_MOD` (modifications), gérés depuis la fiche du circuit.

### 12bis.3 Double accès : fiche + vue transversale

Les sous-éléments ne polluent **jamais** la bibliothèque principale (qui ne liste que voitures et circuits de premier niveau — critique avec 500+ mods). Ils sont accessibles par deux chemins :
- **Depuis la fiche de l'entité** : section « Skins » (sélection/prévisualisation) / « Son » (sélecteur exclusif) sur la fiche d'une voiture (idem circuit). Chemin naturel quand on travaille sur une voiture précise.
- **Vues transversales dédiées** : trois entrées séparées dans la barre latérale — **Skins**, **Sons**, **Apps** — listant tous les éléments du type, filtrables, avec l'entité cible affichée à côté. Chemin pour retrouver « ce pack de skins F1 2023 » sans ouvrir les fiches une à une. Ces vues sont à l'écart du flux principal.

### 12bis.4 Mods de type Application

Les **apps** (apps Python d'AC, dans `content/../apps` ou `apps/`) sont un **type autonome** : ni rattachées à une voiture, ni de premier niveau dans la biblio voitures/circuits. Gérées dans leur propre vue (entrée « Apps » de la barre latérale), simplement **activables/désactivables** (par junction comme le reste). Pas de fiche technique ni de tags élaborés en v1 — juste nom, état, activation.

### 12bis.5 Lancer directement Content Manager

Bouton **« Ouvrir dans CM »** : lance `ContentManager.exe` **sans** argument de session, avec la sélection de mods déjà active (junctions en place). Sert d'échappatoire vers la complexité — l'utilisateur fait ses réglages fins (graphismes CSP/Sol/Pure, FFB, options avancées) dans l'UI native de CM. Complète la page de lancement (§8.6) : chemin simple (session directe via preset) d'un côté, « Ouvrir dans CM » pour les power-users de l'autre. Techniquement trivial (lancement de process sans argument).

---

## 12ter. Lot L7 — Source d'origine & extension navigateur (évolution future)

Idée pour le **moteur d'identité** : capturer l'**URL d'origine** d'un mod via une extension navigateur.

### Distinction cruciale : deux fonctions de difficulté très inégale

Le mot « mise à jour » recouvre deux choses à ne PAS confondre :

**(A) Identité — « est-ce le même mod que ce que j'ai déjà ? »** ✅ Faisable et fiable, toutes sources.
- L'URL de la page d'un mod est une **identité stable**, supérieure au nom de dossier (qui change quand l'auteur renomme). Un fichier téléchargé depuis une URL déjà connue EST une nouvelle version d'un mod existant → plus de match flou (§4.2). L'URL devient le **signal d'identité prioritaire** quand elle est disponible.
- Marche pour **toutes** les sources (OverTake, RSS, VRC…), car l'extension capture l'URL au moment du clic « télécharger », même sur une page payante/protégée.

**(B) Détection de mise à jour *disponible* en ligne — « une version plus récente existe-t-elle que je n'ai pas ? »** ❌ **Hors périmètre — abandonné.**
- Nécessiterait de lire la page du mod et d'y trouver un numéro de version comparable. Aucun standard : chaque site (OverTake, RSS, VRC) affiche la version différemment → il faudrait un **scraper spécifique par site**, fragile (casse à chaque refonte) et **impossible pour les sources fermées** (RSS/VRC : compte payant, page protégée — l'app ne peut pas s'y connecter).
- Le champ URL natif de l'`ui_car.json` est censé faire ça mais **échoue en pratique**, y compris (surtout) pour les grosses sources.
- Décision : **on ne promet pas** la détection auto. Ni scraper, ni suivi manuel bâtard (marquer des dates à la main n'apporte rien de plus que la mémoire de l'utilisateur). La vérification d'une nouvelle version reste manuelle, facilitée par le point ci-dessous.

### Périmètre retenu pour L7

1. **Champ `source_url`** sur le mod, rempli par l'extension (ou saisie manuelle).
2. **Extension navigateur** : capture l'URL de la page au moment du téléchargement, la transmet à l'app pour rattachement. Sous-projet (architecture extension ↔ app à concevoir).
3. **Identité par URL** (fonction A) : reconnaissance d'un téléchargement comme mise à jour d'un mod existant via URL identique. Signal d'identité prioritaire.
4. **Bouton « voir la page d'origine »** : accès direct en un clic à la page du mod (grâce à l'URL capturée), pour que l'utilisateur vérifie lui-même l'existence d'une nouvelle version. Modeste mais réaliste, et couvre toutes les sources.
5. **Filtrage par source** : par site, par auteur.

**Explicitement hors périmètre** : détection automatique de mise à jour disponible (fonction B), scraping de pages, suivi manuel de versions en ligne.

**Statut** : non v1. Le champ `source_url` peut être introduit avant (saisie manuelle) pour préparer le terrain.

---



**Ne pas donner toute la spec à Claude Code en demandant « construis l'app ».** Trop large. Procéder **lot par lot**, la spec servant de contexte permanent et chaque lot faisant l'objet d'une consigne ciblée.

### Ordre recommandé

**Préalable — Squelette + Config.** Avant L1 : initialiser le projet Tauri, l'écran de réglages et l'assistant de première config (§12). Sans chemins valides, rien ne tourne. Livrable testable : l'app démarre, demande les chemins, les valide, les persiste.

**L1 — Bibliothèque & identité** (le cœur, dépend de rien d'autre que la config) :
- Import multi-source : **archive (zip/rar/7z) ET dossier déjà décompressé** (§4.5 — clé pour migrer un catalogue MO2 sans re-zipper), avec descente récursive — porter la logique de `recursiveMoveModsToValidModDir` / `isCar` / `isTrack` / `isCarSound` de `archives.py`. Option copier/déplacer avec déplacement adaptatif selon le disque.
- Détection de type, empreinte composite, résolution de version (règles §4.2/§4.3), base d'overlay non destructive (§3.0), historique.
- Vues galerie + tableau.
- **Source / pack d'origine** (§4.7) : à l'import d'un pack multi-voitures, renseigner `source_pack` sur chaque voiture, et l'**afficher** sur la fiche (bloc Source + section « autres voitures du même pack » + filtrer/désinstaller par pack). Ne pas seulement stocker le champ — l'exposer dans l'UI (maquette `pitbox-source-pack.html`).
- Livrable testable : importer un zip, le voir rangé dans la biblio, le retrouver dans les deux vues, gérer une mise à jour, et pour un pack : voir les voitures sœurs liées par le pack.

**L2 — Tags & organisation** :
- Charger `default-tag-rules.json` (les 264 règles). Moteur d'application des 6 familles de règles.
- Trois origines de tags (§5.3), catégorie = tag `#` (§5bis), fiche technique (§5bis.1), favori (§5bis.2).
- Écran de règles + aperçu d'impact.
- Détection CSP (§5.5).

**L3 — Activation & profils** : junctions Windows (`mklink /J`), états actif/inactif, profils, garde-fous (ne jamais supprimer un dossier réel non-junction).

**L4 — Lancement** : ⚠️ **résoudre d'abord le point ouvert §8.3** (pilotage preset CM) avant d'implémenter. Puis presets de session par type, météo adaptative, écrans session + course.

**L5 — Maintenance & export** : export autonome (+ acd.bms isolé), nettoyage.

**L6 — Types de mods étendus** (post-v1, voir §12bis) : indexation du contenu Kunos (lecture seule), sous-éléments rattachés (skins additifs, sons exclusifs avec restauration), vues transversales Skins/Sons/Apps, mods Application, bouton « Ouvrir dans CM ». Dépend de L1 (modèle d'identité) et L3 (junctions). À faire après que le socle v1 (L1-L4) est stable.

### Artefacts fournis à Claude Code
- Cette spec (contexte).
- `default-tag-rules.json` (ontologie de départ — L2).
- `archives.py` (référence pour la logique d'import/détection — L1) : à **porter**, pas à exécuter.
- `pitbox-mockup.html` (référence visuelle/UX pour tous les écrans).

### Garde-fous
- Le fichier `ui_*.json` du mod est **lecture seule** — jamais réécrit (contrairement à l'ancien `archives.py` qui faisait `json.dump`). C'est la différence n°1 avec le code de référence.
- Vérifier junction vs dossier réel avant toute suppression dans `content/`.
- Ne pas implémenter le lancement (L4) tant que §8.3 n'est pas tranché.

---

## 14. Points ouverts / à vérifier

**Bloquants techniques :**
1. **Pilotage des presets CM + `acmanager://`** — *le* point critique du lot L4. Confirmé : CM est maître de `race.ini`, le bon levier est le preset CM. À vérifier : la commande exacte pour activer un preset par programmation (protocole `acmanager://` ? `Values.data` ? CLI ?). Source primaire AcTools (§8.3).
2. **Détection de la stack météo** — méthode fiable pour repérer Pure/SOL/CSP/vanilla, et table de correspondance preset → backend (§8.5).
3. **Granularité de la signature de contenu** — quels fichiers clés, quel seuil de « match flou » (§4.1).

**Décisions de périmètre :**
4. **Skins voitures** — sous-éléments d'une voiture ou entités à part dans la bibliothèque ? (penche vers sous-éléments, cf. sélection de skin sur l'écran de course).
5. **Mods hors car/track** — PP filters, apps Python, weather : gérés ou hors périmètre v1 ?
6. **Format de la bibliothèque sur disque** — convention de nommage des dossiers de versions (lisible hors de l'app).

**Résolus depuis v0.1 :**
- ✅ Backend réécrit (Python abandonné comme runtime, conservé comme référence).
- ✅ acd.bms cantonné à la fonction d'export autonome uniquement.
- ✅ Fichier du mod jamais modifié → surcouche d'overlay (§3.0).
- ✅ Trois origines de tags tracées (§5.3).
- ✅ Classe `race`/`street` = métadonnée, PAS levier physique CSP (§5bis, §5ter).
- ✅ Catégorie = dimension de composition de plateau (§5bis).
- ✅ Météo : presets + dégradé gracieux + température implicite + statique en v1 (§8.5).
- ✅ Presets de session par type, persistants, disponibilité selon le type (§8.4).
- ✅ Écran de course à deux niveaux + options repliables (§8.6).
- ✅ Page graphique de gestion des règles de tags (§5.4) — maquettée.
- ✅ Règles par défaut extraites de archives.py (264 règles, `default-tag-rules.json`).
- ✅ Fiche détail track maquettée (layouts, CSP, caractéristiques).
- ✅ Configuration formalisée (§12) + guide Claude Code (§13).
