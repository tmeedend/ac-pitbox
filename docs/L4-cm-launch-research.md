# L4 — Pilotage de Content Manager : résolution du point ouvert §8.3

> Recherche menée sur la source primaire AcTools (`gro-ove/actools`, branche master).
> Fichiers clés : `AcManager/Tools/ArgumentsHandler.cs`, `ArgumentsHandler.Commands.cs`,
> `ArgumentsHandler.Race.cs`.

## Verdict

**RÉSOLU ET VALIDÉ EMPIRIQUEMENT (2026-06-27).** Test réel sur la machine :
`Content Manager.exe "acmanager://race/config?configFile=…\test-race.ini"` (GT86 @
Magione, practice). Log CM confirmé :
```
ProcessUriRequest(): URI Request: //race/config?configFile=…
GameWrapper: StartAsync_PrepareRace() → StartAsync_Ui(): Starting game...
Game: StartAsync(): Starting AC: AppIdStarter
→ acs.exe démarré, session chargée (aucune erreur, race.ini non écrasé)
```
Conclusion : passer un `race.ini` préparé via `race/config?configFile=` fonctionne
de bout en bout. La boucle « biblio → race.ini → CM → session » est confirmée.

**Le point ouvert est résolu.** On pilote CM via son **protocole `acmanager://`**, qui
est aussi accepté **en argument de ligne de commande** (même chemin de code). Deux
mécanismes officiels permettent de lancer une session par programmation :

1. **`acmanager://race/config`** — fournir un **`race.ini` préparé** que CM lance
   tel quel (sans le réécrire). **C'est le levier recommandé** : contrôle total
   (voiture, circuit, type de session, IA, météo, heure… tout est dans le race.ini).
2. **`acmanager://race/quick`** — piloter le **Quick Drive de CM** avec un **preset
   sérialisé**. Pratique pour les *presets de session par type* (§8.4).

> Nuance importante vs la spec : « CM écrase race.ini » est vrai pour l'approche
> naïve (écrire le race.ini d'AC sur disque puis lancer). Mais le chemin
> `race/config` passe explicitement notre config à `GameWrapper.StartAsync` via
> `PreparedConfig` — CM ne l'écrase pas, il l'utilise. C'est un chemin **supporté**.

## Détail technique (vérifié dans le code)

### Lecture des paramètres — helper `GetSettings(params, key)`
Pour une clé `X`, CM lit dans l'ordre :
- `XData` → chaîne **base64 « cut »** (URL-safe, padding retiré) décodée en UTF-8 ;
- `XFile` → **chemin d'un fichier** dont on lit le contenu brut (`File.ReadAllText`) ;
- `X` → la valeur brute en clair.

→ **Le plus simple : écrire le race.ini dans un fichier temporaire et passer `configFile=`** (zéro encodage).

### `acmanager://race/config` → `ProcessRaceConfig`
```csharp
var config = GetSettings(custom.Params, "config");   // requis
// (assists optionnels via "assists"/"assistsData"/"assistsFile")
await GameWrapper.StartAsync(new Game.StartProperties {
    PreparedConfig = IniFile.Parse(config)           // notre race.ini, tel quel
});
```
Forme : `acmanager://race/config?configFile=C:\…\race.ini`
ou `acmanager://race/config?configData=<cutbase64(race.ini)>`

### `acmanager://race/quick` → `ProcessRaceQuick`
```csharp
var preset = GetSettings(custom.Params, "preset");   // requis (preset Quick Drive sérialisé)
if (custom.Params.GetFlag("loadPreset")) {           // n'ouvre l'UI que pour charger
    QuickDrive.Show(serializedPreset: preset, …);
} else {
    await QuickDrive.RunAsync(serializedPreset: preset, …);  // lance directement
}
```
Forme : `acmanager://race/quick?presetData=<cutbase64(preset)>`
Flags : `loadPreset` (ouvrir l'UI au lieu de lancer), `loadAssists`.

### `acmanager://race/csp` → `ProcessRaceCsp` (alternative simple car/piste)
Accepte directement `car`, `skin`, `track` (ids), avec sélecteurs si absents.
Construit `StartProperties.BasicProperties { CarId, TrackId, TrackConfigurationId,
CarSkinId }`. Plus limité (orienté P2P/CSP) que `race/config`.

## Invocation depuis l'app
- `ProcessArguments()` traite les `args` de la ligne de commande ; `ProcessArgument`
  → si `IsCustomUriScheme` → `ProcessUriRequest`. Donc :
  **lancer `Content Manager.exe "acmanager://race/config?configFile=…"`** suffit.
- CM est **mono-instance** : si CM tourne déjà, l'URI est transmise à l'instance
  active (IPC). Sinon le process démarre et traite l'argument.
- `GameWrapper.StartAsync` gère le démarrage du jeu (incl. Steam) côté CM — on n'a
  pas à réécrire cette orchestration. La séquence « CM lancé + Steam ouvert » de la
  spec reste une bonne pratique de fiabilité.

## Conséquences pour le modèle L4
- **Voiture/circuit/skin/layout, mode, IA, météo statique, heure** → on construit un
  **`race.ini`** et on lance via `race/config?configFile=`. Pas de dépendance au
  format binaire `Values.data`.
- **Presets de session par type (§8.4)** → on peut soit garder nos propres `race.ini`
  par type, soit s'appuyer sur des presets Quick Drive CM via `race/quick`.
- **Réglages lourds (graphismes/FFB/contrôleur)** → restent gérés par CM (presets CM),
  non touchés par cette voie.

## À faire avant de coder L4
1. Confirmer la structure exacte d'un `race.ini` Quick-Race minimal qui fonctionne via
   `PreparedConfig` (sections `RACE`, `CAR_0`, `SESSION_*`, `WEATHER`, `LIGHTING`/heure,
   IA) — à dériver d'un race.ini réel généré par CM.
2. Vérifier l'encodage « cut base64 » si on préfère `configData` à `configFile`
   (URL-safe, padding retiré) — ou simplement utiliser `configFile` (recommandé).
3. Détection de la stack météo (Pure/SOL/CSP/vanilla) — autre point ouvert §8.5,
   indépendant de celui-ci.

## Sources
- `gro-ove/actools` — `AcManager/Tools/ArgumentsHandler.Race.cs` (ProcessRaceConfig /
  ProcessRaceQuick / GetSettings), `ArgumentsHandler.Commands.cs`, `ArgumentsHandler.cs`.
