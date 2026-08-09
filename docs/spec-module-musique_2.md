# Spécification — Module Musique « Big Picture »

**Projet :** gestionnaire de mods Assetto Corsa
**Version :** 2.0 — périmètre réduit aux fichiers locaux
**Cible technique :** C# / .NET 8 Windows (`net8.0-windows10.0.19041.0`)

---

## 1. Objectifs et périmètre

Fournir une ambiance sonore configurable dans le mode Big Picture, avec deux ambiances distinctes (navigation / préparation de course) et une extinction propre au lancement d'une session.

**La musique provient exclusivement de fichiers audio présents sur le disque de l'utilisateur** : un pack CC0 embarqué avec l'application, les bandes-son que l'utilisateur possède via Steam, ou n'importe quel dossier qu'il désigne.

### Hors périmètre (décision assumée)

Un mode « playlist en ligne » pilotant un navigateur via SMTC a été étudié puis écarté. Motifs : impossibilité de gérer deux ambiances distinctes, absence de contrôle sur les fondus, dépendance à des comportements navigateur instables (vol de focus, autoplay bloqué, identifiants de session variables), et contraintes des CGU YouTube sur la lecture en arrière-plan.

Conséquence directe : **aucune dépendance réseau, aucune API tierce, aucun contrat de service à respecter.** Le module est entièrement local et testable hors ligne.

---

## 2. Configuration

Fichier : `%APPDATA%\<AppName>\music.json`

```json
{
  "version": 2,
  "enabled": true,

  "menuFolder": "%APPDATA%\\<AppName>\\Music\\menu",
  "gridFolder": "%APPDATA%\\<AppName>\\Music\\grid",

  "shuffle": true,
  "volume": 0.45,
  "normalize": true,

  "crossfadeMs": 2500,
  "fadeOutMs": 1500,
  "fadeInMs": 2000,

  "sessionBehavior": "stop",
  "sessionDuckVolume": 0.12
}
```

**`sessionBehavior`** — comportement au lancement d'Assetto Corsa :
- `"stop"` *(défaut)* — fade-out complet puis arrêt. Silence pendant la course.
- `"duck"` — fade vers `sessionDuckVolume`, la musique reste en fond derrière le bruit moteur.

Le second mode a du sens en essais libres ou en hotlap solo. Le proposer, mais ne pas en faire le défaut : en course, la majorité des pilotes veulent le silence pour entendre le moteur et les concurrents.

**Règles de persistance :**
- Les chemins sont stockés avec variables d'environnement non résolues, pour rester portables entre machines.
- Le fichier n'est jamais écrasé lors d'une mise à jour de l'application : migration par numéro de `version`.

---

## 3. Dossiers musicaux

### 3.1 Dossiers par défaut

À la première exécution, créer et pré-remplir :

```
%APPDATA%\<AppName>\Music\menu\    ← pack CC0 ambiance calme
%APPDATA%\<AppName>\Music\grid\    ← pack CC0 ambiance montante
```

Justification : l'expérience est bonne immédiatement, sans aucune configuration.

Le dossier Musique de Windows (`Environment.SpecialFolder.MyMusic`) sert uniquement de **répertoire initial du sélecteur de dossier**, jamais de destination d'écriture — c'est la bibliothèque personnelle de l'utilisateur, on n'y dépose rien.

> ⚠️ Ne jamais coder en dur `C:\Users\<user>\Music`. Le dossier existe bien (`FOLDERID_Music`) mais peut être redirigé vers OneDrive. Toujours passer par `Environment.GetFolderPath`.

### 3.2 Détection des bandes-son Steam

Alimente une liste déroulante « Bandes-son détectées » à côté du bouton Parcourir.

**Étape 1 — localiser Steam**

```csharp
var steamPath =
    Registry.GetValue(@"HKEY_CURRENT_USER\Software\Valve\Steam", "SteamPath", null) as string
    ?? Registry.GetValue(@"HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath", null) as string;
```

**Étape 2 — énumérer les bibliothèques**

Parser `<steamPath>\steamapps\libraryfolders.vdf` (format VDF, texte). Récupérer chaque valeur `"path"`. L'utilisateur a fréquemment ses jeux sur un second SSD.

**Étape 3 — scanner les emplacements musicaux**

Pour chaque bibliothèque, dans l'ordre de priorité :

1. `<lib>\steamapps\music\<Nom>\` — emplacement des **DLC Soundtrack Steam**. Source principale : contenu acheté séparément, livré en MP3 ou FLAC en clair.
2. `<lib>\steamapps\common\<Jeu>\` avec sous-dossier nommé `Soundtrack`, `OST`, `Music` ou `Audio\Music` — OST livrée avec certains jeux.

Un dossier n'est retenu que s'il contient **au moins un fichier audio lisible** (`.mp3`, `.flac`, `.ogg`, `.wav`, `.m4a`). Afficher le nom du dossier et le nombre de pistes.

> **Note.** Le second emplacement est moins net juridiquement que le premier — c'est du contenu livré avec le jeu plutôt qu'acheté séparément. Il reste de la lecture de fichiers en clair que l'utilisateur possède légitimement, mais si tu veux le périmètre le plus strict possible, ne garde que `steamapps/music/`.

**Étape 4 — présentation**

```
Dossier Menu :  [ Pack par défaut (CC0)              ▾ ]  [ Parcourir… ]
                ├─ Pack par défaut (CC0) — 6 pistes
                ├─ ── Bandes-son Steam ──
                ├─ DiRT Rally 2.0 Soundtrack — 24 pistes
                ├─ Euro Truck Simulator 2 OST — 18 pistes
                └─ ── Personnalisé ──
```

### 3.3 Limite absolue

> 🚫 **Aucune lecture des conteneurs audio des jeux** (`.bank` FMOD utilisé par Assetto Corsa, `.bnk` Wwise, formats propriétaires). Aucun outil d'extraction embarqué ni appelé (`bnkextr`, `ww2ogg`, `vgmstream`, `QuickBMS`).
>
> Le module ne lit que des fichiers audio standards déjà en clair sur le disque. Motifs : clauses anti-extraction des CLUF, article L331-5 du CPI sur les mesures techniques de protection, et exposition à une mise en demeure pour un outil public et nominatif.
>
> Cette ligne ne doit pas être franchie, y compris sur demande insistante d'utilisateurs. Si un utilisateur a extrait sa propre musique de son côté, il pointe le dossier — ce n'est pas le problème de l'application, tant qu'elle ne fournit pas l'outil.

### 3.4 Indexation et normalisation

Au premier scan d'un dossier, écrire un cache `.<AppName>-index.json` dans ce dossier :

```json
{
  "scannedAt": "2026-08-09T14:00:00Z",
  "tracks": [
    { "file": "track01.mp3", "durationMs": 214000, "gainDb": -3.2 }
  ]
}
```

**Calcul du gain :** RMS sur l'ensemble du fichier, cible −18 dBFS, correction bornée à ±12 dB pour éviter les aberrations. Si un tag ReplayGain est présent, le préférer au calcul.

Sans cette normalisation, un MP3 ripé à −18 LUFS suivi d'un FLAC masterisé à −8 produit un saut de volume brutal. C'est le principal écart de qualité perçue entre un lecteur amateur et un module agréable au quotidien.

Le scan tourne en tâche de fond avec barre de progression — quelques secondes pour 30 pistes.

**Invalidation :** comparer le nombre de fichiers et la date de modification du dossier au chargement. Si divergence, rescanner.

---

## 4. Machine à états

```
        ┌──────────────────────────────────────────┐
        │                                          │
        ▼                                          │
     ┌──────┐   entrée configurateur   ┌──────┐    │
     │ MENU │ ───────────────────────► │ GRID │    │
     │      │ ◄─────────────────────── │      │    │
     └──────┘        retour            └──────┘    │
        │                                  │       │
        │      lancement AC (les deux)     │       │
        └──────────────┬───────────────────┘       │
                       ▼                           │
                 ┌───────────┐   fermeture AC      │
                 │  SESSION  │ ────────────────────┘
                 └───────────┘
```

| Transition | Effet | Durée |
|---|---|---|
| MENU → GRID | Crossfade | 2500 ms |
| GRID → MENU | Crossfade | 2500 ms |
| * → SESSION | Fade-out (stop ou duck selon config) | 1500 ms |
| SESSION → MENU | Fade-in | 2000 ms |
| Fin de piste | Crossfade interne, même dossier | 2500 ms |

Le fade-in de retour est volontairement plus lent que le fade-out : après une course, une reprise brutale de la musique est désagréable.

**Détection du lancement d'AC :** surveillance des processus `acs.exe` et `AssettoCorsa.exe`. Un polling à 500 ms est suffisant et bien plus simple qu'un `ManagementEventWatcher` sur `Win32_ProcessStartTrace`, qui coûte un thread WMI.

**Reprise d'état :** si l'application démarre alors qu'AC tourne déjà, entrer directement en SESSION sans jouer le fade-in.

---

## 5. Moteur audio

**Bibliothèque : NAudio** (licence MIT, gratuite y compris en usage commercial).
Extensions nécessaires : `NAudio.Vorbis` (OGG) et `NAudio.Flac` (FLAC), non couverts nativement.

### 5.1 Chaîne de traitement

```
AudioFileReader (piste A) ─┐
    ↓ resample 44.1k/2ch   │
  GainProvider (fade)      ├─► MixingSampleProvider ─► VolumeProvider ─► WasapiOut
                           │        (44100 Hz, stéréo)      (volume global)
AudioFileReader (piste B) ─┘
    ↓ resample 44.1k/2ch
  GainProvider (fade)
```

Deux lecteurs coexistent en permanence pour permettre le recouvrement. Utiliser `WdlResamplingSampleProvider` pour harmoniser les fréquences d'échantillonnage hétérogènes.

**Sortie :** `WasapiOut` en mode partagé (`AudioClientShareMode.Shared`), latence 200 ms.

> ⚠️ **Ne jamais utiliser le mode exclusif.** Il monopoliserait la carte son et couperait entièrement l'audio d'Assetto Corsa. C'est le bug le plus grave possible pour ce module.

### 5.2 Crossfade à puissance constante

Point critique. Un fondu linéaire produit un creux de volume audible au milieu de la transition, parce que deux signaux décorrélés à 50 % d'amplitude ne restituent pas 100 % de puissance perçue.

```csharp
// t va de 0.0 à 1.0 sur la durée du crossfade
float gainOut = (float)Math.Cos(t * Math.PI / 2.0);  // 1 → 0
float gainIn  = (float)Math.Sin(t * Math.PI / 2.0);  // 0 → 1
// Invariant : gainOut² + gainIn² = 1
```

Pour un fondu simple (entrée ou sortie seule), une courbe quadratique est préférable au linéaire, l'oreille percevant le volume de façon logarithmique :

```csharp
float gain = (float)Math.Pow(t, 2.0);  // fade-in perçu comme régulier
```

### 5.3 Préchargement

Ouvrir le `AudioFileReader` de la piste suivante **crossfadeMs + 500 ms** avant la fin de la piste courante. Sur un disque lent ou un FLAC volumineux, l'ouverture peut prendre plusieurs centaines de millisecondes et produire un trou audible.

### 5.4 Sélection des pistes

Mode aléatoire : permutation Fisher-Yates de l'index complet, rejouée quand la liste est épuisée.

Ne **pas** tirer au sort à chaque piste — cela produit des répétitions immédiates que l'utilisateur perçoit comme un bug.

Contrainte supplémentaire : lors d'une nouvelle permutation, si la première piste est identique à la dernière jouée, l'échanger avec la seconde.

**Transition MENU ↔ GRID :** chaque ambiance conserve sa propre position de lecture. Au retour vers MENU, reprendre la piste là où elle en était plutôt que d'en relancer une au hasard — la continuité est nettement plus agréable si l'utilisateur fait des allers-retours rapides.

---

## 6. Interface de configuration

```
┌─ Musique ────────────────────────────────────────────────┐
│                                                          │
│  [x] Activer la musique dans le mode Big Picture         │
│                                                          │
│  Ambiance menu                                           │
│   [ Pack par défaut (CC0)           ▾ ]  [ Parcourir… ]  │
│   6 pistes détectées                                     │
│                                                          │
│  Ambiance préparation de course                          │
│   [ DiRT Rally 2.0 Soundtrack       ▾ ]  [ Parcourir… ]  │
│   24 pistes détectées                                    │
│                                                          │
│  [x] Lecture aléatoire                                   │
│  [x] Normaliser le volume entre les pistes               │
│                                                          │
│  Volume       ├────●──────────┤  45 %                    │
│  Fondu        ├──────●────────┤  2,5 s                   │
│                                                          │
│  Pendant une session :                                   │
│   (•) Couper la musique                                  │
│   ( ) Baisser le volume                                  │
│                                                          │
│                              [ Crédits musicaux ]        │
└──────────────────────────────────────────────────────────┘
```

Prévoir un bouton d'écoute (▶) à côté de chaque sélecteur de dossier pour tester l'ambiance sans lancer le Big Picture.

---

## 7. Conformité

### Ce que fait l'application

| Action | Statut |
|---|---|
| Lire des fichiers audio standards du disque de l'utilisateur | ✅ |
| Lister les bandes-son Steam que l'utilisateur possède | ✅ |
| Embarquer et redistribuer un pack musical CC0 | ✅ |

### Ce que l'application ne fait jamais

| Action | Motif |
|---|---|
| Extraire l'audio de conteneurs de jeu (FMOD, Wwise, propriétaires) | Clauses anti-extraction des CLUF ; art. L331-5 CPI |
| Embarquer ou appeler `yt-dlp`, `ww2ogg`, `vgmstream`, `QuickBMS` | Contournement caractérisé |
| Redistribuer de la musique sous licence « créateurs de contenu » | NCS, StreamBeats et équivalents : licence limitée aux vidéos et streams, **jeux et logiciels explicitement exclus** |

> **Le piège principal de ce domaine :** « libre de droits pour ta vidéo YouTube » ≠ « libre de droits pour être embarqué dans un logiciel redistribué ». Les deux notions n'ont presque rien en commun. Pour du bundling, le seul critère fiable est **CC0** ou **CC-BY**.

### Pack CC0 embarqué

Sources acceptables pour du contenu redistribué avec l'application :

| Source | Licence | Attribution |
|---|---|---|
| FreePD.com | CC0 | Non requise |
| Kenney.nl | CC0 | Non requise |
| OpenGameArt (filtre CC0) | CC0 | Non requise |
| itch.io (filtre CC0) | CC0 | Non requise |
| Musopen | Domaine public | Non requise |
| Pixabay Music | Licence Pixabay | Non requise (revente du fichier seul interdite) |
| Incompetech / Kevin MacLeod | CC-BY | **Requise** |

**Ambiance recherchée**
- *Menu* : chillhop, downtempo, ambient synthwave, tempo bas.
- *Grille* : synthwave driving, hybride orchestral, percussions montantes, 120–140 BPM. Le domaine public classique fonctionne remarquablement bien ici (Holst *Mars*, Moussorgski, Wagner).

Volume cible du pack embarqué : 5 à 6 pistes par ambiance, environ 30 Mo au total en MP3 192 kbps.

Maintenir un fichier `CREDITS.md` listant pour chaque piste : titre, auteur, URL source, licence, date de téléchargement. À conserver **même pour du CC0** — c'est la seule protection en cas de contestation ultérieure.

---

## 8. Découpage en lots

| Lot | Contenu | Dépendances |
|---|---|---|
| 1 | Moteur audio, une seule ambiance, sans fondu | NAudio |
| 2 | Deux ambiances, machine à états, crossfade puissance constante | Lot 1 |
| 3 | Scan de dossiers, indexation, normalisation RMS | Lot 1 |
| 4 | Détection Steam, liste déroulante | Lot 3 |
| 5 | Détection processus AC, fade-out / duck de session | Lot 2 |
| 6 | Panneau de configuration, pack CC0, crédits | Tous |

Le lot 1 est livrable et testable seul. Chaque lot suivant ajoute une couche sans casser la précédente.

---

## 9. Points de vigilance

- **Mode exclusif WASAPI** : à proscrire absolument, il couperait le son d'Assetto Corsa.
- **Changement de périphérique audio à chaud** (casque branché en cours de route) : `WasapiOut` lève une exception. S'abonner à `MMNotificationClient.OnDefaultDeviceChanged` et reconstruire la chaîne.
- **Fichiers corrompus ou illisibles** : encapsuler chaque ouverture dans un `try`, passer à la piste suivante, journaliser sans interrompre la lecture. Un seul MP3 tronqué dans un dossier de 40 ne doit pas tuer le module.
- **Dossier vide, supprimé ou déplacé** entre deux lancements : repli silencieux sur le pack par défaut, avec notification discrète.
- **Threading** : NAudio effectue ses callbacks sur un thread dédié. Toute mise à jour d'interface (piste en cours, progression) doit passer par le dispatcher.
- **Libération des ressources** : `AudioFileReader` maintient un verrou sur le fichier. Sans `Dispose` systématique, l'utilisateur ne pourra plus renommer ni supprimer ses morceaux tant que l'application tourne.
- **Chemins longs** : les dossiers Steam imbriqués dépassent parfois 260 caractères. Activer `<LongPathAware>` dans le manifeste applicatif.
