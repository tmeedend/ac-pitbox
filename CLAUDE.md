# Pit Box — consignes de travail

Gestionnaire de mods **Assetto Corsa** : remplace Mod Organizer 2 et pilote
Content Manager (CM) comme moteur de lancement. Application desktop Windows.

## Langues — à ne pas confondre

L'application a vocation à être **publique et open source**. D'où trois régimes
distincts :

| Quoi | Langue |
| --- | --- |
| **Code : identifiants, commentaires, doc-comments, noms de tests, messages d'erreur techniques** | **anglais** |
| Échanges avec l'utilisateur, `docs/`, messages de commit | français |
| Chaînes visibles par l'utilisateur | ni l'un ni l'autre en dur → **i18n** (`fr.json` + `en.json`) |

**Tout code nouveau ou modifié s'écrit en anglais.** L'existant est encore
largement en français : le traduire au fil de l'eau, sur ce qu'on touche
réellement. Ne pas partir en traduction spontanée de fichiers qu'on n'a pas
besoin de modifier — ça noie la revue du vrai changement.

Aucune chaîne visible en dur dans un composant : elle passe par `t("clé")` et la
clé est ajoutée **dans les deux locales**, jamais une seule.

**Un libellé d'écran n'explique jamais son propre fonctionnement.** Pas de
texte du genre « 1 clic = layout de session » ou « version installée en tête »
sur l'interface : ça décrit l'implémentation, pas ce dont l'utilisateur a
besoin pour agir. Si l'interaction n'est pas assez claire par elle-même
(bouton, état visuel, tooltip au survol), c'est ça qu'il faut corriger — pas
ajouter une légende qui explique la mécanique. Un décompte (`{n}`) ou un badge
d'état restent bienvenus dans le complément d'un bandeau (`.blk-n`) ; un mode
d'emploi n'y a pas sa place.

**Les erreurs backend destinées à l'utilisateur sont des clés, pas des
phrases.** Une `String` française renvoyée par une commande Tauri atterrit telle
quelle dans l'UI et n'est traduisible nulle part. Donc :
`ok_or(crate::errors::AC_NOT_CONFIGURED)?` (constante = `"errors.acNotConfigured"`),
résolue côté front par `errorText(e)` de `$lib/errors`. Les détails techniques
(E/S, SQLite, 7-Zip) gardent leur message brut : ce sont des diagnostics, pas
des conseils. Toute nouvelle erreur user-facing ajoute sa constante dans
`errors.rs` **et** sa clé dans les deux locales.

**Un `let _ = ...` sur une opération qui peut échouer s'accompagne d'un
`log::warn!`.** Beaucoup d'opérations (activation à l'import, arbitrage de
priorité entre « autres mods », projections skin/circuit) sont *best-effort*
par design : un échec ne doit pas bloquer l'UI ni le reste d'un lot. Mais
« ne bloque pas l'UI » ne veut pas dire « ne laisse aucune trace » — sur une
install packagée (`.exe`), il n'y a pas de console, donc pas de log du tout
si l'échec n'est écrit nulle part. Journal fichier via `tauri-plugin-log`
(niveau Warn, `%APPDATA%\com.pitbox.app\logs\`, configuré dans `lib.rs`) :
un `log::warn!` au moment de l'échec (pas seulement un `Result` remonté et
jamais lu) est ce qui rend un bug rapporté par un utilisateur diagnosticable
après coup.

## Stack

| Couche | Techno |
| --- | --- |
| Backend | Rust 2021, Tauri v2 (`src-tauri/`), SQLite via `rusqlite` (bundled) |
| Frontend | SvelteKit SPA (adapter-static) + **Svelte 5 runes** + TypeScript + Vite 6 |
| Cible | Windows uniquement (junctions, hardlinks, chemins `D:\…`) |

**Svelte 5 runes, pas Svelte 4** : `$state` / `$derived` / `$props` / `$effect`.
Pas de `export let`, pas de stores `writable` pour l'état local, pas de `$:`.

Shell : PowerShell (l'outil Bash reste disponible pour les scripts POSIX).
Jamais d'élévation admin — l'app doit fonctionner en utilisateur standard.

## Règles d'or (non négociables)

1. **Le `ui_*.json` d'un mod est en lecture seule.** Jamais réécrit, jamais
   « corrigé ». Toutes les métadonnées vivent dans la base d'overlay SQLite
   (`app_config_dir/overlay.sqlite`). C'est ce qui distingue Pit Box de
   l'ancien `archives.py`.
2. **Avant toute suppression dans `content/`, vérifier junction/hardlink vs vrai
   dossier.** Le garde-fou existe dans `activation.rs` — ne jamais le
   contourner. Effacer un vrai dossier du jeu est irréversible.
3. **Jamais un fichier retiré de l'intérieur du dossier du mod.** Le dossier du
   mod, c'est le dossier que l'auteur a conçu pour être posé dans `content/`
   (`rss_gtm_lanzo_v8/`, `ks_nordschleife/`) — pas l'archive qui l'entoure.
   Tout ce qui est **dedans**, à quelque profondeur que ce soit, est du contenu
   du mod : ça se copie en bibliothèque intégralement, ça ne se trie pas.
   L'extraction des annexes (§4.5.1) ne s'applique **qu'à ce qui est à côté du
   dossier du mod**, jamais dedans. Bug réel : `body_shadow.png`,
   `tyre_*_shadow.png` et `logo.png` — de vrais assets AC vivant à la racine du
   dossier voiture — ont été déplacés en `resources/` sur 23 mods, parce que le
   classement se fondait sur l'extension et la profondeur au lieu de
   l'appartenance au mod. Une annexe **détectée dedans** (un PDF de notice au
   milieu de la voiture) peut être **signalée**, jamais déplacée : dans le
   doute, le fichier reste où l'auteur l'a mis. Le script
   `scripts/audit-resources.ps1` audite et répare l'existant.
4. **CM est maître de `race.ini`.** On pilote des presets via le protocole
   `acmanager://`, on n'écrit pas les fichiers du jeu à la main.
5. **Aucun fichier du jeu altéré durablement.** Un fichier d'AC *peut* être
   remplacé par un mod — beaucoup de mods ne font que ça — mais jamais sans
   filet : l'original est sauvegardé avant écriture, restauré dès que plus
   aucun mod ne le réclame, et un balayage au démarrage rattrape les fermetures
   anormales. Tout est dans `gamebackup.rs` (§4.5.4) : **passer par lui**, ne
   jamais écrire directement dans le dossier du jeu. Corollaire souvent
   oublié : un fichier qu'on n'a pas posé ne se supprime pas, et un exemplaire
   plus ancien ne déloge pas ce qui tourne déjà (même arbitrage par date que
   les fichiers partagés, §4.5.4). Ne pas confondre avec la règle n°2, qui
   porte sur les **dossiers** de `content/`.
6. **Jamais `localStorage` pour un réglage qui doit survivre à un
   redémarrage.** `localStorage` n'est pas garanti synchrone sur disque côté
   WebView2 — l'écriture part dans le buffer du navigateur, pas sur disque, et
   une fermeture de l'app juste après peut la perdre. Bug réel constaté
   plusieurs fois avant que la règle ne soit écrite ici (duo voiture/circuit,
   colonnes de bibliothèque, vue galerie/tableau…) : le réglage survivait tant
   que l'app restait ouverte, mais jamais à un vrai redémarrage — le genre de
   bug qui se reproduit à l'identique tant que le remède n'est pas
   systématique. Le remède : un petit fichier JSON dans `app_config_dir`,
   écrit côté Rust en `std::fs::write` **synchrone** (donc la commande Tauri
   ne rend la main que quand c'est réellement sur disque), jamais dans la base
   SQLite — voir `session_state.rs`/`saved_sessions.rs`/`library_columns.rs`
   pour le patron à suivre (charger/modifier/réécrire l'objet entier à chaque
   sauvegarde, structure opaque côté Rust — `serde_json::Value`, le schéma
   appartient au frontend). Pour un réglage qui ne mérite pas son propre
   fichier (une case à cocher, un tri, une préférence par mod…),
   `ui_prefs.json` via `src/lib/uiPrefs.svelte.ts` (`getUiPref`/`setUiPref`)
   est le point d'entrée générique — ne pas créer un fichier Rust dédié pour
   un seul booléen. Toute lecture qui doit rester synchrone (ex. dans une
   expression de template appelée pour chaque carte d'une liste) passe par
   `peekUiPref` (cache réactif, `$state`) plutôt que l'API asynchrone —
   `preferred.ts` en est l'exemple.
   `localStorage` reste acceptable pour un état **purement transitoire**,
   jamais relu après redémarrage (aucun cas de ce genre dans le projet
   aujourd'hui) — dans le doute, c'est un fichier Rust.

## Structure du projet

```
src-tauri/src/          Backend Rust — un module par domaine
  lib.rs                Point d'entrée : mod, état partagé, setup, invoke_handler
  commands/             Façades #[tauri::command], un fichier par domaine
  errors.rs             Clés i18n des erreurs destinées à l'utilisateur
  overlay.rs            Base SQLite : schéma, migrations ALTER idempotentes, CRUD
  importer.rs modscan.rs archive.rs    Import : détection, extraction, classement
  activation.rs deploy.rs compose.rs layers.rs   Déploiement dans content/
  extras.rs gamebackup.rs              Ce qu'un mod pose hors de content/<type>/<id>
  library.rs submods.rs apps.rs others.rs        Bibliothèque et add-ons
  launch.rs quickdrive.rs weather.rs   Lancement de session via CM
  rules.rs harmonize.rs                Moteur de tags
  maintenance.rs export.rs             Outils
  uijson.rs inspect.rs identity.rs     Lecture des fichiers AC
src-tauri/crates/       Crates du workspace (aperçu 3D, docs/SPEC-preview-3d-kn5.md)
  kn5/                  Parsing du format KN5 — pur, sans I/O ni Tauri
  kn5-gltf/             Textures et export glTF (touche au disque : skins)
  kn5-tool/             CLI de validation, jamais livrée à l'utilisateur
src/lib/
  components/           Composants Svelte (voir la carte des écrans)
  components/detail/    Blocs extraits de la fiche détail
  *.ts                  Bindings typés vers les commandes Tauri
  i18n/locales/         fr.json + en.json
  styles/global.css     Design system Rosso Corsa
docs/                   Documentation (voir ci-dessous)
scripts/                Outillage ponctuel, hors application (PowerShell)
```

Les deux scripts de `scripts/` sont des **outils de dépannage**, pas des
fonctionnalités : sortie sèche par défaut, action seulement sur option
explicite. `audit-resources.ps1` liste — et répare sur `-Restore` — les
fichiers que l'extracteur d'annexes a sortis d'un dossier de mod (règle d'or
n°3). `clean-ac-footprint.ps1` liste — et retire sur `-Apply` — tout ce que
l'app a déployé dans une install AC : indispensable **avant** de supprimer une
bibliothèque, sans quoi les déploiements par hardlink deviennent de vrais
dossiers pleins de contenu que rien ne nettoie et sur lesquels le garde-fou
refusera ensuite de reposer quoi que ce soit.

### Carte des écrans

`AppShell.svelte` est la coquille : barre latérale + aiguillage sur
`nav.section` (`src/lib/nav.svelte.ts`). Correspondance section → composant :

| Section | Composant | Note |
| --- | --- | --- |
| `cars` / `tracks` | `Library.svelte` | **rendu deux fois**, prop `kind` — persistance suffixée par type |
| `carskins` / `trackskins` / `sounds` | `Transversal.svelte` | **un seul composant pour trois entrées**, prop `variant` |
| `race` | `Launch.svelte` | |
| `import` | `Import.svelte` | contient `BulkImport` |
| `rules` / `profiles` / `maintenance` | `RulesEditor` / `Profiles` / `Maintenance` | |
| `apps` / `others` | `Apps` / `OtherMods` | |
| `settings` / `about` | `Settings` / `About` | |

Deux présentations coexistent pour une entité et **ne sont pas
interchangeables** (§6) : `ModDetail.svelte` = panneau latéral,
`DetailPage.svelte` = page pleine (ouverte par `Library` via son état
`fullId`). Une évolution de fiche est souvent à faire **dans les deux**.

Hors aiguillage : `TitleBar`, `ImportOverlay` (les modales d'arbitrage) et
`ToastStack` (`ImportToasts` + `ControllerToast`) — tous dans `AppShell` —,
`SetupWizard` (dans `routes/+page.svelte`, première configuration),
`BulkEditPanel` / `ContextMenu` (dans `Library`), `OpponentPicker` /
`SavedSessionsDialog` (dans `Launch`).

Ajouter une fonctionnalité backend = 3 endroits : la fonction dans son module
métier, la façade `pub fn` dans `commands/<domaine>.rs` **et** son inscription
dans `invoke_handler![…]` de `lib.rs`, puis le binding typé dans le
`src/lib/*.ts` correspondant. Oublier `invoke_handler` ne casse rien à la
compilation — l'erreur n'apparaît qu'à l'exécution.

Une façade ne fait que charger la config, prendre le verrou SQLite et déléguer.
Toute logique qui grossit dans `commands/` doit descendre dans son module
métier. Les commandes sont `pub` (obligatoire hors du crate racine) et
partagent `commands::prelude`.

## Conventions qui ne se devinent pas

Elles ne cassent rien quand on les ignore — elles produisent un bug silencieux.

- **Lire une préférence dans le corps d'un `$effect` abonne cet effet à
  *toutes* les préférences.** `peekUiPref` (donc `getPreferredSkin`,
  `getPreferredLayout`, `preferred.ts` en général) lit un cache `$state` global :
  toute écriture ailleurs dans l'app le remplace, et l'effet lecteur se
  redéclenche. Bug réel : bouger un curseur de l'aperçu 3D relançait le
  chargement complet de la fiche détail — skins rechargés, skin sélectionné
  réinitialisé, aperçu remonté et retour à la photo. Une restauration à
  l'ouverture n'est pas une dépendance : l'entourer d'`untrack`. Le symétrique
  côté écriture est déjà documenté dans `uiPrefs.svelte.ts` (`setUiPref` est
  `untrack`é pour la même raison, après une boucle infinie de 285 000 appels).
- **`t("clé")` renvoie la clé elle-même si elle manque** dans les deux locales.
  Une clé oubliée n'explose donc pas : elle s'affiche telle quelle à l'écran
  (`detail.showroom`). C'est ce qui rend `errorText()` sûr, et c'est aussi
  pourquoi une relecture visuelle attrape ces oublis mieux que le typage.
- **Le CSS des composants est scopé** (voir l'en-tête de `global.css`) : seules
  `.btn`, `.input`, `.mono`, `.pill`, `.gp-focus`, et les trois niveaux de
  libellé `.lbl-screen`/`.lbl`/`.lbl-key` (§chantier libellés) sont globales.
  Déplacer du markup d'un composant à l'autre n'emporte pas son style.
- **Les clés `StorageKey.*` sont suffixées par type** (`storage.ts`) quand le
  composant est rendu plusieurs fois : `pitbox.view.cars` / `pitbox.view.tracks`,
  `pitbox.sort.<kind>.key`… Oublier le suffixe fait partager le réglage entre
  voitures et circuits. Ces clés ne servent plus qu'à nommer les entrées dans
  `ui_prefs.json`/les fichiers Rust dédiés (règle d'or n°5) — `localStorage`
  lui-même n'est plus écrit nulle part, seulement lu une fois en migration.
- **`Prefs` (`config.rs`) est en `#[serde(default)]`** : un champ retiré est
  simplement ignoré dans les `config.json` existants, pas de migration à
  écrire. Un champ ajouté prend sa valeur par défaut chez les utilisateurs
  existants.
- **Un module métier Rust n'importe pas `tauri::{AppHandle, Emitter}`.** Pas
  seulement par propreté d'architecture : mesuré, l'import suffit à rendre le
  binaire de test de la lib **inexécutable** — il ne démarre plus du tout
  (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, avant le premier test), alors
  que le même import dans `commands/` ou dans `import_progress.rs` ne pose
  rien. Trouvé par bissection sur `bulk.rs` : 253 tests passent sans l'import,
  zéro avec. Un module qui doit rendre compte prend donc une **fermeture**
  (`ProgressSink` dans `bulk.rs`), et c'est la façade qui émet.
- **Les tests backend tournent sur un vrai système de fichiers** : ils créent
  des junctions et des hardlinks réels, donc uniquement sous Windows.
- **`Metadata::is_dir()` ne distingue pas une junction (dossier) d'un lien
  fichier sur un point de reparse Windows** : les deux renvoient `is_dir() ==
  false` via `symlink_metadata` (vérifié empiriquement, pas documenté côté
  Rust std). `activation::remove_junction` ne peut donc pas brancher sur
  `meta.is_dir()` pour choisir entre `remove_dir`/`remove_file` — il tente
  `remove_dir` puis se replie sur `remove_file` en cas d'échec, jamais
  l'inverse.

## Documentation

- **`docs/README.md`** — index de tout `docs/`. Point d'entrée.
- **`docs/SPEC.md`** — spécification de référence, organisée par domaine.
  Décrit l'app telle qu'elle fonctionne. **La source de vérité.**
- **`docs/*.html`** — maquettes visuelles (référence de layout et de thème).
- **`docs/*-research.md`** — comptes rendus de recherche sur les points durs
  (lancement CM, aperçu 3D). Contiennent le *pourquoi* de choix non évidents et
  la trace des pistes abandonnées. À lire avant de retenter quelque chose.
- **`docs/SPEC-preview-3d-kn5.md`** — spécification du chantier « aperçu 3D
  natif » (parser KN5 → glTF → three.js), avec son plan par lots. Accompagnée
  de **`docs/kn5-format.md`**, qui consigne ce que le format fait *réellement*
  quand il s'écarte de la spec, avec la méthode de vérification. Toute
  découverte sur le format s'écrit là, pas seulement dans un commentaire.
- **`docs/windows-code-signing.md`** — signature Authenticode de l'installateur
  (SmartScreen). À lire **avant** d'acheter un certificat.
- `docs/README-livrables.md` — doc d'amorçage historique, partiellement
  périmée : `SPEC.md` fait foi en cas d'écart.

## CI

`.github/workflows/ci.yml` sur chaque push et PR : types + build frontend
(Ubuntu), puis clippy `-D warnings` + tests + empaquetage de l'installateur
(Windows — les tests créent de vraies junctions, ils ne tournent nulle part
ailleurs). Piège à connaître : `tauri-build` exige que `../build` existe, donc
`npm run build` doit précéder `cargo test`, même pour de simples tests.

`.github/workflows/release.yml` sur tag `v*` : construit les installateurs et
crée une release **brouillon**. L'étape de signature Azure y est écrite mais
commentée — les binaires actuels ne sont pas signés.

`src-tauri/` est la **racine d'un workspace Cargo** : le paquet principal plus
les crates de l'aperçu 3D sous `src-tauri/crates/`. D'où le `--workspace` de
clippy/test ci-dessus — sans lui, cargo ne regarde que le paquet racine et les
crates passent en CI sans être vérifiés.

**Le profil `dev` optimise les dépendances, pas le code de l'app**
(`[profile.dev.package."*"]` et les deux crates de l'aperçu, dans
`src-tauri/Cargo.toml`). Mesuré : la conversion d'un aperçu 3D prenait **5,5 s
sous `tauri dev` contre 0,4 s en release**, tout le temps passant dans le
décodage et le ré-encodage des textures. Ramenée à 0,5 s. Le code de Pit Box
reste en `opt-level = 0`, donc débogable ; la contrepartie est une première
compilation plus longue après un `cargo clean`.

`src-tauri/rustfmt.toml` fixe le style (`max_width = 120`) et `cargo fmt --all
--check` est dans la CI. Un reformatage massif se fait dans un commit isolé, jamais
mélangé à un changement fonctionnel : sinon `git blame` devient inexploitable.

## Chantiers en cours

Liste vivante : **retirer chaque entrée dès qu'elle est faite**, ne pas la
laisser pourrir ici.

- [ ] **Harmonisation des libellés**. 68 règles de libellé
      produisent 53 signatures visuelles distinctes : 15 tailles de police,
      7 interlettrages, 9 couleurs. La même fonction visuelle change donc
      d'apparence selon l'écran. Cible : trois niveaux globaux — `.lbl-screen`
      (titre d'écran), `.lbl` (rubrique), `.lbl-key` (clé de donnée) — et des
      couleurs redevenues sémantiques (rouge = catégorie/session/destructif,
      bleu = info et fichier mod, vert = règle, jaune = alerte). Fait : fiche
      détail ; titres d'écran passés à `.lbl-screen` sur les quatre zones
      restantes (bibliothèque/lancement/réglages/add-ons) — au passage,
      `.lbl-screen` lui-même corrigé à 18px/600 dans `global.css` (4 écrans
      sur 5 avaient déjà convergé là spontanément, sans classe partagée ;
      Réglages à 15px était l'écart, pas la référence) ; quelques `.lbl`/
      `.lbl-key` ponctuels (`BulkEditPanel`, `OpponentsBlock`, `WeatherBlock`,
      `Transversal`). **Explicitement laissé de côté** (décidé avec
      l'utilisateur, à traiter séparément si besoin) : les libellés de champ
      de formulaire (Réglages, Chemins, filtres bibliothèque — rôle différent
      d'une clé de fiche technique en lecture seule, même si visuellement
      proche) et les titres de popup (`OpponentPicker`/`SavedSessionsDialog`,
      13px/majuscules, identiques entre eux mais ne correspondant à aucun des
      trois niveaux). **Couleurs sémantiques** : pas encore attaquées — le
      `--orange` ajouté pour « mod inactif » (`StateBadge`) est le premier pas
      dans cette direction (ni le jaune d'alerte, ni le rouge destructif).
- [ ] **Composants partagés plutôt que styles recopiés.** Même cause que le
      chantier ci-dessus, un cran au-dessus : le CSS Svelte étant **scopé par
      composant**, une même brique recopiée dans dix écrans y dérive sans que
      rien ne le signale — ni `npm run check`, ni la relecture d'un seul
      fichier. Fait : `Tabs.svelte` (remplace trois `.tabs` locaux — fiche
      détail, Réglages, Règles — et sert désormais aussi les deux écrans
      Add-ons), `StateBadge.svelte` (colonne « État » du tableau + fiche
      détail), `NumberStepper`, `LoadingState`, `Tooltip`, `ContextMenu`,
      `Toast`/`ToastStack` (pile bas-droite : progression et rapports
      d'import, actions groupées, nouveau périphérique — voir SPEC §4.2bis),
      `Slider` (tous les curseurs de l'app),
      `InlineEdit` (nom et description repris à la main — SPEC §5bis.3).
      **Inventaire de ce qui reste**, mesuré le 2026-08-18 :
      - **Boîte d'erreur : 15 définitions locales** (`.err` / `.error` /
        `.action-err` dans Apps, BulkEditPanel, BulkImport, DetailPage,
        Launch, LayersSection, Maintenance, ModDetail, OtherMods, Profiles,
        Settings, SetupWizard, Transversal, MusicTab). Mêmes trois couleurs
        partout (`--rosso-dim` / `--rosso-border` / `--rosso-bright`), seuls
        le padding (8/10 vs 10/12), la taille (11,5 vs 12px) et les marges
        diffèrent. Le cas le plus net : une classe globale `.errbox` suffit,
        les marges restant à l'appelant.
      - **Sous-titre d'écran : 8 copies** de `.sub`, identiques à `max-width`
        près (520/540/560/620/aucune). Trois n'avaient pas de `font-size` et
        étaient donc plus gros que les autres — corrigé, mais les 8 copies
        restent. En faire un 4ᵉ niveau global (`.lbl-sub` ?) est une décision
        de design à prendre avec l'utilisateur, pas à trancher seul.
      - **Groupe de boutons segmenté : 6 copies** (`.seg` / `.seg-v`) dans
        Library, Transversal, BulkImport, OpponentsBlock, SessionOptionsBlock,
        SessionTypeBlock. Deux orientations (horizontale/verticale) et deux
        traitements de l'état actif (fond rouge plein vs fond surélevé) : un
        vrai composant avec une prop d'orientation, pas juste une classe.
      Un lot de ce genre est du **reformatage pur sur une quinzaine de
      fichiers** : le faire dans son propre commit, jamais mélangé à un
      changement fonctionnel (sinon `git blame` devient inexploitable).
- [ ] **Aperçu 3D natif des voitures** (branche `feature/3dpreview`).
      **L'avancement détaillé, les écarts assumés vis-à-vis de la spec et le
      reste à faire sont dans `docs/SPEC-preview-3d-kn5.md` §13 à §15** — c'est
      là qu'il faut lire en reprenant, pas ici.
      En bref : **lots 0 à 6 faits et validés à l'écran** — la voiture
      s'affiche dans la fiche, tourne sur son socle, se manipule à la souris,
      porte la couleur de son skin, se règle depuis la fiche et projette son
      ombre sur le sol d'un studio.
      **Ce qu'il faut retenir des neuf écarts de format documentés** (tous dans
      `docs/kn5-format.md`, avec leur méthode de mesure) : **AC renseigne ses
      champs et ses slots standard avec des valeurs que ses shaders n'utilisent
      pas comme on le croirait.** Trois formes rencontrées, dans cet ordre de
      difficulté :
      *un état* (carte de dégâts, saleté de pare-brise) qu'il ne mélange qu'à
      proportion de quelque chose ; *un masque* (la peinture d'un skin, sous
      l'alpha de la diffuse) ; et *un objet entier* (`ksBrokenGlass`, la vitre
      brisée, toujours présente dans le modèle). Un même matériau peut mentir
      sur plusieurs de ses champs à la fois — `ksWindscreen` s'est trompé trois
      fois de suite (texture, opacité, exposant spéculaire). Donc : devant un
      défaut visuel, **ne pas s'arrêter au premier champ coupable**, et
      regarder aussi ce qui est dessiné par-dessus.
      Reste surtout **R et B de `txMaps`** (le vert est documenté et exploité),
      le choix du LOD, la purge du cache et l'aperçu dans `ModDetail` — §15.
      Trois règles à ne pas perdre de vue :
      **`preview::CONVERTER_VERSION` s'incrémente dès qu'on touche au rendu
      produit** (sinon les anciens `.glb` restent servis — la version est dans
      le *nom* des entrées de cache, ce qui permet aussi d'effacer les
      périmées) ; **une conversion ne se valide jamais sur une seule voiture**
      — l'atlas de la MX-5 est symétrique et a masqué une erreur de repère
      pendant deux lots ; et **le `preview.jpg` d'un skin est une référence de
      cadrage, pas de luminosité** — il est plus sombre que le rendu du jeu, ce
      qui m'a fait diagnostiquer un écart inexistant.
- [ ] **Signature Authenticode** : le workflow est prêt, il attend un
      certificat. Définir la variable de dépôt `SIGN_COMMAND` suffit à
      l'activer — voir `docs/windows-code-signing.md` (lire **avant** d'acheter,
      deux pièges y sont documentés). Repo passé public, `v0.1.0` publiée,
      README doté de la « Code signing policy » exigée : candidature déposée
      auprès de la SignPath Foundation (certificat gratuit, projet open
      source), réponse attendue par email. Si refus ou trop long, plan B
      documenté : Certum Open Source Code Signing (~49€/an, cloud SimplySign,
      pas de jeton USB).
- [ ] **Runner de tests frontend** : délibérément absent. À reconsidérer
      seulement le jour où de la logique pure sera extraite des composants
      (le tri/regroupement/cumul de `Transversal.svelte` en est proche) — pour
      tester *cette* logique, pas l'affichage.
## Fin de tâche — dans cet ordre

1. **Mettre à jour `docs/SPEC.md`** dès qu'une évolution change le
   comportement de l'app. Le SPEC décrit ce que l'app *est* : il doit rester
   synchrone avec le code. Une simple correction de bug ne s'y écrit pas.
2. **Vérifier** — c'est exactement ce que rejoue la CI, autant le savoir avant
   de pousser :
   ```bash
   npm run check && npm run build
   ```
   ```bash
   cd src-tauri && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
   ```
   `npm run check` doit rester à 0 erreur **et 0 warning**, et **clippy à 0
   warning** — la CI les traite en erreurs.
3. **Commiter** — message en français, court et descriptif, sans préfixe
   imposé. Le `push` n'est pas automatique : le demander.
4. **Démarrer l'application** pour que les développements soient disponibles,
   via la configuration `tauri (app desktop)` de `.claude/launch.json`
   (`npm run tauri dev`).

## Vérification : l'aperçu navigateur ne prouve rien

Le serveur Vite seul rend bien les pages, mais **`invoke` n'existe pas hors de
Tauri** : tous les appels backend échouent et les écrans restent vides. Une
capture d'écran du navigateur ne vaut donc pas vérification. Pour valider un
changement observable, lancer la vraie app Tauri. Sinon, le dire franchement
plutôt que de laisser croire à une vérification.

## Style

**Commentaires** : denses, ils expliquent le *pourquoi* — surtout les
contraintes découvertes empiriquement (« sans cette clé, écran noir en test
réel », « WebView2 compose son rendu par-dessus toute fenêtre native sœur »).
Un commentaire qui paraphrase le code est inutile ; un commentaire qui évite de
refaire une erreur déjà faite vaut de l'or. Référencer la section du SPEC
concernée (`§4.5.3`) quand elle existe.

**Dépendances** : ne pas en ajouter à la légère, et retirer celles qui ne
servent plus (une fonctionnalité abandonnée emporte sa dépendance).

## Tests

Tout en module, `#[cfg(test)] mod tests` en fin de fichier. Pas de dossier
`tests/`, pas de runner frontend — `npm run check` couvre le typage, et le
risque réel est côté Rust, là où une erreur détruit des fichiers de jeu.

- **Un test = une règle**, nommée en phrase :
  `junction_create_remove_and_guard`, `activate_deactivate_leave_no_history`.
  Un commentaire en tête rappelle la règle protégée et sa section de spec.
- **Un bug corrigé devient un test.** C'est la meilleure habitude de la suite —
  la moitié des tests existants sont là pour ça.
- **Les assertions portent un message** expliquant l'attente :
  `assert!(target.join("file.txt").is_file(), "target preserved")`. Quand un
  test casse, on lit l'intention au lieu de la deviner.
- **Vrai système de fichiers, pas de mock.** Les tests construisent une arbo AC
  synthétique : junctions, hardlinks et suppressions ne se prouvent pas
  autrement. Des fabriques locales au module (`make_fake_car`) évitent la
  répétition.
- **Un dossier temporaire par test**, via `crate::testutil::temp_dir("tag")`.
  Le garde renvoyé **nettoie sur `Drop`**, donc même quand le test échoue.
  Ne jamais réintroduire de `std::env::temp_dir()` ni de `remove_dir_all` en
  fin de test : c'est précisément ce qui laissait des milliers de dossiers dans
  `%TEMP%`. Le guard déréférence vers `Path` (`base.join(…)` marche tel quel) ;
  pour un `PathBuf` possédé, `base.to_path_buf()`.

Les tests actuels passent tous — ne pas en casser un sans le dire.
