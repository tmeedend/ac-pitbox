# L4 — Pilotage de Content Manager : résolution du point ouvert §8.3

> Recherche menée sur la source primaire AcTools (`gro-ove/actools`, branche master).
> Fichiers clés : `AcManager/Tools/ArgumentsHandler.cs`, `ArgumentsHandler.Commands.cs`,
> `ArgumentsHandler.Race.cs`, `GameWrapper.cs`.

## Verdict (mis à jour 2026-07-xx)

**`race/config`/`PreparedConfig` (race.ini) a été abandonné : il ne déclenche pas le
téléchargement CSP automatique.** Bug confirmé empiriquement (lancement direct depuis
l'UI CM = CSP charge VAO/config manquants ; lancement Pit Box via `race/config` = rien).
Root cause trouvée dans le pipeline `GameWrapper.StartAsync` :

```
StartAsync_AdjustProperties → StartAsync_Prepare → StartAsync_PrepareRace (mode course
uniquement) → StartAsync_Ui / StartAsync_NoUi
```

Le check CSP (`PatchUpdater.Instance` / `LoadPatchDataAutomatically`) dans
`StartAsync_Ui` lit `StartProperties.BasicProperties` (`CarId`/`TrackId` typés). Or
`race/config` ne peuple **que** `PreparedConfig` (le `race.ini` brut, tel quel) —
`BasicProperties` reste `null`, donc le check CSP n'a rien à quoi s'accrocher et ne se
déclenche jamais. C'est un chemin délibérément « bas niveau » côté CM : il lance
la session sans repasser par le pipeline normal de préparation.

**On pilote désormais CM via `acmanager://race/quick`**, qui déclenche
`QuickDrive.ViewModel.Go()` côté CM — le même chemin que le bouton « DRIVE » de son
propre UI Quick Drive. Ce chemin peuple correctement `BasicProperties`, donc le
téléchargement CSP automatique se déclenche normalement.

Preuve minimale (2026-07-xx) : preset `.cmpreset` sauvegardé par CM lui-même
(`AppData\Local\AcTools Content Manager\Presets\Quick Drive\pitbox.cmpreset`), rejoué
via `Content Manager.exe "acmanager://race/quick?presetFile=…\pitbox.cmpreset"` →
téléchargement CSP déclenché, confirmé par l'utilisateur.

## Mécanisme retenu : `acmanager://race/quick`

```csharp
var preset = GetSettings(custom.Params, "preset");   // requis (preset Quick Drive sérialisé)
if (custom.Params.GetFlag("loadPreset")) {           // n'ouvre l'UI que pour charger
    QuickDrive.Show(serializedPreset: preset, …);
} else {
    await QuickDrive.RunAsync(serializedPreset: preset, …);  // lance directement
}
```
Forme retenue : `acmanager://race/quick?presetFile=C:\…\preset.json` (fichier
temporaire, comme pour l'ancien `race/config?configFile=`, zéro encodage base64).

### Lecture des paramètres — helper `GetSettings(params, key)`
Pour une clé `X`, CM lit dans l'ordre :
- `XData` → chaîne **base64 « cut »** (URL-safe, padding retiré) décodée en UTF-8 ;
- `XFile` → **chemin d'un fichier** dont on lit le contenu brut (`File.ReadAllText`) ;
- `X` → la valeur brute en clair.

### Invocation depuis l'app
- `ProcessArguments()` traite les `args` de la ligne de commande ; `ProcessArgument`
  → si `IsCustomUriScheme` → `ProcessUriRequest`. Donc :
  **lancer `Content Manager.exe "acmanager://race/quick?presetFile=…"`** suffit.
- CM est **mono-instance** : si CM tourne déjà, l'URI est transmise à l'instance
  active (IPC). Sinon le process démarre et traite l'argument.
- `GameWrapper.StartAsync` gère le démarrage du jeu (incl. Steam) côté CM — on n'a
  pas à réécrire cette orchestration.

## Format du preset Quick Drive (`SaveableData`, sérialisé JSON)

Le format n'est pas documenté publiquement dans le dépôt (juste des classes C#
partielles) — **reconstruit empiriquement** à partir de fichiers `.cmpreset` réels
sauvegardés depuis l'UI Quick Drive de CM (un par type de session : Practice, Hotlap,
Weekend). Champs de premier niveau confirmés sur ces captures :

- `Mode` : chemin de la page XAML du mode (`/Pages/Drive/QuickDrive_Practice.xaml`,
  `..._Hotlap.xaml`, `..._Weekend.xaml`) — détermine quel `ModeData` est attendu.
- `ModeData` : JSON **imbriqué en chaîne** (échappé), spécifique au mode — voir §
  ci-dessous.
- `CarId` / `TrackId` (avec `/layout` suffixé si applicable) — typés, peuplent
  `BasicProperties` (c'est le point qui débloque le CSP auto-load).
- `WeatherId`, `RealConditions`, `Temperature`, `Time` (secondes depuis minuit),
  `TimeMultipler`.
- `udt`/`dtv` : date de simulation (`udt` = flag « utiliser une date », `dtv` =
  date ISO `YYYY-MM-DDT00:00:00`) — équivalent de l'ancien `__CM_DATE` du race.ini.
- `TrackPropertiesData` : JSON imbriqué en chaîne (état de piste — grip/évolution).
  Pas de mapping trouvé côté Pit Box pour l'instant (toujours « Optimum »/sec,
  voir limites connues plus bas).
- `AssistsData` : JSON imbriqué en chaîne (aides + simulation dégâts/carburant/usure).
- Vent (`wsf`/`wst`/`wd`), divers flags de comportement CM (`rws`, `rwd`,
  `rcTimezones`, …) — repris tels quels avec des valeurs par défaut raisonnables,
  non exposés dans notre UI.

### `ModeData` par type de session
- **Practice** (`QuickDrive_Practice.xaml`) : `{StartType, Penalties, PlayerBallast,
  PlayerRestrictor}`. **Pas de champ durée** — session à durée libre par design Quick
  Drive (pas un manque du format).
- **Hotlap** (`QuickDrive_Hotlap.xaml`) : `{GhostCar, DoNotRecordGhostCar,
  GhostCarAdvantage, Penalties, PlayerBallast, PlayerRestrictor}`.
- **Weekend** (`QuickDrive_Weekend.xaml`, couvre Course + Qualif/Practice optionnels) :
  `{PracticeLength, QualificationLength, Penalties, JumpStartPenalty, LapsNumber,
  RaceGridSerialized, Version}`. `RaceGridSerialized` est lui-même du JSON en chaîne :
  `{ModeId:"manual", CarIds[], SkinIds[], AiLevels[], OpponentsNumber,
  StartingPosition, …}` pour un plateau explicite par adversaire (voir
  `RaceGridViewModel`/`RaceGridEntry` dans `AcManager.Controls`).

Pit Box génère ce preset en implémentant uniquement le mode **Weekend** pour la
course (pas de mode Race/Trackday/Drift séparé côté CM) : sans practice
configurée, Weekend se comporte comme une course avec qualif minimale — pas
d'équivalent Weekend sans qualif, voir point 4 ci-dessous.

## Limites connues du nouveau mécanisme
1. **Skin du joueur non forçable** : pas de champ dans le schéma Quick Drive pour le
   skin du joueur (contrairement à `race/csp` qui a `CarSkinId`, mais ce chemin ne
   supporte pas un plateau complet). CM retombe sur le dernier skin utilisé pour la
   voiture. `RaceSetup.car_skin` est conservé côté Rust (compat aller-retour front)
   mais non lu par `quickdrive::build_preset`.
2. **Grip/évolution de piste non mappés** : `TrackPropertiesData` toujours codé en
   dur sur « Optimum »/sec — pas de champ identifié correspondant au réglage
   `grip` de notre UI dans les captures réelles.
3. **Durée de session Practice non appliquée** : le schéma `QuickDrive_Practice`
   n'a pas de champ durée — comportement Quick Drive natif, pas un bug.
4. **Qualification jamais désactivable en mode Weekend** : `QuickDrive_Weekend.xaml.cs`
   borne `QualificationDuration` à `[5, 90]` minutes (`value.Clamp(5, 90)`), et son
   `Save()` écrit toujours une durée concrète — aucun état « off » n'existe côté CM
   pour cette phase (confirmé aussi bien sur les `.cmpreset` réels de l'utilisateur,
   qui ont toujours `QualificationLength` non nul, que dans le code source). Envoyer
   `QualificationLength: null` ne désactive donc rien — `Load()` retombe juste sur
   `r.QualificationLength ?? 30`, un défaut fixe. Pit Box envoie toujours une durée
   concrète (mini 5 min côté UI) plutôt qu'une case « pas de qualif » qui ne pourrait
   jamais être honorée par CM.

Ces quatre points sont documentés en commentaire sur `RaceSetup` (`src-tauri/src/launch.rs`)
et sur `quickdrive::build_preset` (`src-tauri/src/quickdrive.rs`).

## Chargement de l'`AssistsData` : gardé par un réglage global de CM (2026-08-11)

Root cause d'un bug réel signalé par l'utilisateur : dégâts/carburant/usure/ABS/
antipatinage/ligne idéale du preset Quick Drive n'avaient **aucun effet**, quelle
que soit la valeur choisie dans Pit Box — et pour cause, le mécanisme touchait même
les `.cmpreset` sauvegardés manuellement par l'utilisateur depuis l'UI de CM.

`QuickDrive.xaml.cs` :
```csharp
private bool IsToLoadAssists() {
    return SettingsHolder.Drive.LoadAssistsWithQuickDrivePreset ^ (Keyboard.Modifiers == ModifierKeys.Control);
}
...
if (forceAssistsLoading || IsToLoadAssists()) {
    LoadPreset(AssistsViewModel, o.AssistsPresetFilename, o.AssistsData, o.AssistsChanged);
}
```
`LoadAssistsWithQuickDrivePreset` est un réglage global de CM (case à cocher sur sa
page Quick Drive, libellé « Charger assistances avec préréglage de course rapide »),
**`false` par défaut** (`SettingsHolder.Drive.cs` : `ValuesStorage.Get(…, false)`).
Tant qu'il n'est pas activé, CM ignore silencieusement (pas d'exception — vérifié
dans les logs CM, `%LOCALAPPDATA%\AcTools Content Manager\Logs\`) l'`AssistsData` de
**tout** preset Quick Drive lancé via `race/quick`, et garde les assistances
actuellement actives dans son UI. Le raccourci Ctrl qui inverse ce comportement ne
s'applique pas non plus : Pit Box lance CM en tâche de fond, sans interaction clavier.

`ArgumentsHandler.Race.cs::ProcessRaceQuick` expose un échappatoire dans l'URI elle-même :
```csharp
if (!await QuickDrive.RunAsync(serializedPreset: preset, forceAssistsLoading: custom.Params.GetFlag("loadAssists"))) { … }
```
D'où le flag `&loadAssists=true` ajouté à l'URI par `launch::launch()` — force
`forceAssistsLoading`, indépendamment du réglage global de CM. `TrackPropertiesData`
(grip) n'a pas de garde équivalente : toujours chargé (`if (!LoadPreset(TrackState, …))`
sans condition), donc pas de flag additionnel nécessaire pour ce champ.

Conséquence annexe : l'ancienne écriture directe de `assists.ini` avant le lancement
de CM (`launch::apply_assists`, best-effort avant ce correctif) était sans effet réel
— CM régénère son propre état d'assistances au démarrage du jeu, par-dessus n'importe
quelle écriture externe du fichier. Fonction retirée avec l'ajout de `loadAssists=true`.

## Génération du preset côté Pit Box
`quickdrive::build_preset(&RaceSetup) -> Result<String, String>` (Rust,
`src-tauri/src/quickdrive.rs`) construit le JSON directement avec `serde_json::json!`
(pas de template texte). `launch::launch()` écrit le résultat dans un fichier
temporaire jetable (`%TEMP%\pitbox-quickdrive-<uuid>.json`, un par lancement — jamais
les `.cmpreset` sauvegardés par l'utilisateur lui-même) et invoque
`Content Manager.exe "acmanager://race/quick?presetFile=…"`.

## Sources
- `gro-ove/actools` — `AcManager/Tools/ArgumentsHandler.Race.cs` (ProcessRaceConfig /
  ProcessRaceQuick / GetSettings), `ArgumentsHandler.Commands.cs`, `ArgumentsHandler.cs`,
  `GameWrapper.cs` (pipeline `StartAsync`).
- Fichiers `.cmpreset` réels sauvegardés par CM (Practice/Hotlap/Weekend), lus et
  comparés champ par champ pour reconstruire le schéma `SaveableData`.
