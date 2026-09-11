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
| Chaînes visibles par l'utilisateur | ni l'un ni l'autre en dur → **i18n** (six locales, `fr` + `en` en référence) |

**Tout code nouveau ou modifié s'écrit en anglais.** L'existant est encore
largement en français : le traduire au fil de l'eau, sur ce qu'on touche
réellement. Ne pas partir en traduction spontanée de fichiers qu'on n'a pas
besoin de modifier — ça noie la revue du vrai changement.

Aucune chaîne visible en dur dans un composant : elle passe par `t("clé")` et la
clé est ajoutée **dans `fr.json` et `en.json`**, jamais une seule des deux.

**Six locales, mais deux seulement sont de ta responsabilité.** `fr` et `en`
forment le couple de référence : `en.json` définit l'ensemble des clés qui
existent, `fr.json` doit le suivre exactement. Les quatre autres (`it`, `de`,
`es`, `pt`) sont des traductions : `t()` retombe sur l'anglais pour une clé
qui y manque (`i18n/index.svelte.ts`), donc une clé ajoutée sans elles
s'affiche en anglais — jamais la clé brute à l'écran. Les traduire est un
bonus, pas un prérequis. `npm run check` lance `scripts/check-locales.mjs`,
qui **échoue** sur une clé inconnue, une clé manquante en `fr`, ou une
**variable d'interpolation perdue ou inventée** (`{count}`, `{name}`…) — ce
dernier cas est le seul défaut de traduction qu'on ne peut pas voir en
relisant une langue qu'on ne parle pas. Le nombre de clés traduites par langue
est simplement affiché.

**Le jargon Assetto Corsa ne se traduit pas** : *skin*, *layout*, *pack*,
*showroom*, *hardlink*, *junction*, *hotlap* restent en anglais dans toutes
les langues — c'est ainsi que les pilotes les emploient.

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
   **Corollaire découvert à l'usage : une écriture ne se rate jamais en
   silence.** Le fichier ne suffit pas si l'appel qui l'écrit a un repli
   muet. `invokeSafe` (délai de 5 s puis valeur par défaut) protège une
   *lecture* — un réglage manquant vaut mieux qu'un écran figé — mais appliqué
   à une *écriture* il rend « enregistré » une commande qui n'a rien écrit.
   Constaté : `ui_prefs.json` inchangé pendant six heures d'utilisation, corps
   et tenues de pilote adoptés puis disparus au redémarrage, sans un mot nulle
   part. Donc : écriture sans délai, échec réessayé puis journalisé, et la file
   d'écritures munie d'un `catch` — une promesse rejetée y gèle sinon toutes
   les suivantes jusqu'au prochain démarrage.
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
  fragment.rs           Mod ou couche déguisée en mod : géométrie, recherche de l'hôte
  activation.rs deploy.rs compose.rs layers.rs   Déploiement dans content/
  extras.rs gamebackup.rs              Ce qu'un mod pose hors de content/<type>/<id>
  library.rs submods.rs apps.rs others.rs        Bibliothèque et add-ons
  attach.rs inventory.rs usermeta.rs   Inventaire des compléments : sur quoi un
                        mod se greffe, ce qu'il fait, et ce que l'utilisateur a
                        saisi dessus (note, nom repris à la main)
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
  components/Inventory.svelte  L'écran Compléments (§7bis du SPEC)
  components/FicheHeader.svelte NoteBlock.svelte PickerBar.svelte Pencil.svelte
                        Briques de fiche partagées par les cinq types
  *.ts                  Bindings typés vers les commandes Tauri
  i18n/locales/         fr, en (référence) + it, de, es, pt (traductions)
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

`AppShell.svelte` est la coquille : **rail de navigation** (`NavRail.svelte`,
les lieux) + **colonne de session** (ce qu'on lance) + aiguillage sur
`nav.section` (`src/lib/nav.svelte.ts`). Les trois territoires et leur
frontière étanche sont au §7.2 du SPEC. Correspondance section → composant :

Le rail a **deux rangs**, et ils ne classent pas par type de contenu mais par
**durée de validité** de ce qu'on y règle : *La session* (ce qui se décide à
chaque fois) et *Le jeu* (ce qui reste vrai jusqu'à nouvel ordre).

| Section | Composant | Note |
| --- | --- | --- |
| `cars` / `tracks` | `Library.svelte` | **rendu deux fois**, prop `kind` — persistance suffixée par type |
| `driver` | `driver/DriverScreen.svelte` | galerie des mannequins + panneau d'essayage |
| `apps` | `Apps.svelte` | écran à part entière depuis la refonte (§3.2) |
| `others` | `Inventory.svelte` | **l'inventaire des compléments** — cinq sources en une liste |
| `race` | `Launch.svelte` | |
| `rules` / `import` / `profiles` / `maintenance` | `Workshop.svelte` | **un écran, quatre onglets** — l'onglet EST la section, pas un état local |
| `settings` / `about` | `Settings` / `About` | |

**Les trois écrans transversaux ont disparu** (Add-ons voiture, Add-ons
circuit, l'ancien fourre-tout) : ils classaient par mécanique d'installation,
et leur contenu est dans l'inventaire. `Transversal.svelte` est supprimé,
`OtherMods.svelte` n'est plus routé — la fiche d'un mod « autre »
(`OtherModDetail`) vit désormais par-dessus l'inventaire.

Deux pièges de ce regroupement : l'onglet de l'Atelier étant `nav.section`, un
`requestSection("import")` posé ailleurs (glisser-déposer global, rapport
d'import) continue d'atterrir au bon endroit — ne pas le remplacer par un état
local ; et `RulesEditor` reste le seul des quatre à gérer son propre
défilement (`noPad`), d'où le mode `full` de `Workshop`.

**Les fiches s'empilent**, et l'empilement est plat : `DetailPage` rend
`LayerDetail` (une couche) ou `OtherModDetail` (un mod greffé) **à sa place**
quand l'une d'elles est ouverte, `Library` rend `PackDetail` par-dessus
`DetailPage`, et `Inventory` rend `OtherModDetail`/`SoundDetail` par-dessus
sa liste. Le retour ferme la fiche du dessus, jamais l'écran entier.

**Une seule fiche** : `DetailPage.svelte`, la page pleine, ouverte par
`Library` au double-clic sur une carte ou une ligne (état `nav.openFull`). Le
panneau latéral compact qui la doublait à droite de la grille a été retiré —
il montrait moins, et toute évolution de fiche était à faire deux fois.

Hors aiguillage : `NavRail`, `TitleBar`, `ImportOverlay` (les modales d'arbitrage) et
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
- **Un `$effect` ne s'abonne qu'à ce qu'il a lu avant de sortir.** Une garde
  placée en tête (`if (!x) return;`) tronque donc la liste des dépendances au
  premier passage — et un premier passage a lieu au montage, quand rien n'est
  encore prêt. Ce qui est lu *sous* la garde n'est jamais enregistré, si bien
  que la valeur qu'on attendait peut arriver sans rien redéclencher. Bug réel :
  l'aperçu de l'écran des vignettes n'affichait que son fond à la première
  ouverture, et il fallait changer de preset — donc toucher à la seule
  dépendance qui, elle, avait été lue — pour que les voitures apparaissent. Le
  remède est mécanique : **lire toutes les dépendances en tête, avant la
  moindre sortie**. Corollaire : une variable dont l'affectation doit
  redéclencher un effet est un `$state`, jamais un `let` ordinaire — celui-ci
  ne s'observe pas.
- **Une mesure de pixels ne s'écrit jamais telle quelle dans un `style`.** Le
  zoom d'interface est un `zoom` CSS posé sur `<html>` : `getBoundingClientRect`,
  `clientX/clientY` et `innerWidth/Height` rendent des pixels **réels de la
  fenêtre**, déjà multipliés, alors qu'un `left`/`top`/`width` écrit sur un
  descendant est en pixels CSS que le zoom multipliera à son tour. Reporter
  l'un dans l'autre applique donc le facteur deux fois, et l'écart grandit avec
  la distance au coin haut-gauche — invisible à 100 %, donc invisible en
  développement. Trois fois le même bug : le menu contextuel décalé, puis les
  listes déroulantes (skin de la colonne de session, tenue par défaut) ouvertes
  très en dessous de leur bouton jusqu'à sortir de l'écran, puis les colonnes
  de bibliothèque élargies de 10 % à la première prise de poignée. Diviser par
  `zoomFactor()` (`zoom.svelte.ts`) avant d'écrire, toujours.
  **Corollaire côté CSS : un seuil de mise en page est une `@container`, pas une
  `@media`.** Une requête de média interroge la fenêtre — donc un seuil que le
  zoom déplace, et qui de toute façon ignore ce que le rail et la colonne de
  session ont déjà pris. Une requête de conteneur interroge la largeur
  réellement disponible, la seule dont dépende la mise en colonnes. `DetailPage`
  déclare `container: detail / inline-size` sur `.page`, et tous ses seuils s'y
  réfèrent.
- **`t("clé")` renvoie la clé elle-même si elle manque** en anglais aussi.
  Une clé oubliée n'explose donc pas : elle s'affiche telle quelle à l'écran
  (`detail.showroom`). C'est ce qui rend `errorText()` sûr, et c'est aussi
  pourquoi une relecture visuelle attrape ces oublis mieux que le typage.
- **Le CSS des composants est scopé** (voir l'en-tête de `global.css`) : seules
  `.btn`, `.input`, `.mono`, `.pill`, `.gp-focus`, `.warnbox`/`.errbox`, et les
  quatre niveaux de libellé `.lbl-screen`/`.lbl-sub`/`.lbl`/`.lbl-key`
  (§chantier libellés) sont globales.
  Déplacer du markup d'un composant à l'autre n'emporte pas son style.
- **Les clés `StorageKey.*` sont suffixées par type** (`storage.ts`) quand le
  composant est rendu plusieurs fois : `pitbox.view.cars` / `pitbox.view.tracks`,
  `pitbox.sort.<kind>.key`… Oublier le suffixe fait partager le réglage entre
  voitures et circuits. Ces clés ne servent plus qu'à nommer les entrées dans
  `ui_prefs.json`/les fichiers Rust dédiés (règle d'or n°5) — `localStorage`
  lui-même n'est plus écrit nulle part, seulement lu une fois en migration.
- **Une `Map` ou un `Set` clés par objet ne retrouvent rien si l'objet vient
  d'un `$state`.** `$state` enveloppe tableaux et objets dans un **proxy
  profond** : l'objet relu dans un `{#each}` (ou par un `$derived` qui filtre
  la liste) n'est pas celui qu'on avait rangé dans la table, et
  `map.get(f)` rend `undefined` — toujours, jamais par intermittence. Comme
  ce genre de table a presque toujours un repli raisonnable, l'échec est
  muet et le symptôme sort ailleurs : la carte Ressources fusionnée demandait
  la notice d'une livraison voisine au mod de la fiche, d'où « dossier
  ressources introuvable » sur une voiture qui n'a pas de dossier ressources.
  Le remède est de ne pas dépendre de l'identité : **l'information voyage avec
  l'élément** (`{ ...file, owner }`), ou la clé est une valeur (`id:origin:chemin`).
- **Une prop Svelte ne peut pas s'appeler `state`.** Svelte 5 y voit une
  ambiguïté avec la rune `$state` — un `$state` préfixant une variable locale
  crée un abonnement de store — et refuse de compiler. `FicheHeader` nomme donc
  sa prop `deployment`, du vocabulaire du §12, plutôt que `state`.
- **Une colonne lue par un `SELECT` mais oubliée dans `migrate()` échoue en
  silence.** Le SELECT rate, et comme la plupart des appelants de ces listes
  sont *best-effort* (`let _ = …`), le symptôme est à des kilomètres de la
  cause : un mod qui s'importe normalement mais ne s'active plus, sans un mot.
  Rien dans le typage ne relie la liste de colonnes d'un SELECT à celle des
  `ALTER` — c'est `overlay::tests::every_listing_runs_on_a_fresh_database` qui
  s'en charge, et il vient d'une erreur réelle.
- **Changer de section ET ouvrir une fiche demande `openInSection`.**
  `requestSection` remet `openFull` à zéro, et l'`await` qui la suit garantit
  que l'observateur d'historique (`navHistory.ts`) voit passer la liste
  d'arrivée comme un écran à part entière : « précédent » y ramenait au lieu de
  rendre l'écran d'où l'on venait. Les deux écritures d'affilée, sans `await`
  entre elles, n'exposent qu'un seul état.
- **Le « ← » d'une fiche passe par `goBackOr`**, jamais par une fermeture
  sèche : l'historique d'abord, la fermeture en repli quand il n'y a rien
  derrière. Même règle pour le bouton B de la manette et les boutons latéraux
  de la souris — c'est le même geste, il ne peut pas avoir deux comportements.
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
- **`docs/SPEC-import.md`** — l'import sur une page : une seule question
  (« où va ce fichier ? »), un arbre de décision, la table des mécanismes de
  pose, et les cinq archives réelles qui servent de tests. À lire **avant**
  de toucher à une règle d'import — le §4 du SPEC les décrit une par une,
  celui-ci les montre *ensemble*, ce qui est la seule façon de voir qu'une
  règle en contredit une autre. En cas d'écart, `SPEC.md` fait foi, et l'un
  des deux est à corriger tout de suite.
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
      d'apparence selon l'écran. Cible : quatre niveaux globaux —
      `.lbl-screen` (titre d'écran), `.lbl-sub` (le sous-titre qui l'explique),
      `.lbl` (rubrique), `.lbl-key` (clé de donnée) — et des
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
      quatre niveaux). **Couleurs sémantiques** : le **rouge** a désormais son
      barème, écrit au §7.2ter du SPEC — quatre niveaux, un quota par niveau,
      et la règle « le survol n'introduit jamais de rouge sur un élément qui
      n'y a pas droit au repos ». Appliqué au rail de navigation et à la
      colonne de session ; **les filtres et la grille de bibliothèque restent
      à faire** (specs séparées). Le reste n'est pas attaqué — le `--orange`
      ajouté pour « mod inactif » (`StateBadge`) est le premier pas dans cette
      direction (ni le jaune d'alerte, ni le rouge destructif).
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
      `InlineEdit` (nom et description repris à la main — SPEC §5bis.3),
      `Field` (un réglage dans un bloc : intitulé, commande, explication — et
      surtout **l'écart avec le précédent**, que chaque écran posait à la main
      et qu'un champ ajouté après coup oubliait ; c'est ce qui collait « pilote
      au volant » à la case du dessus), `.errbox` (la boîte d'erreur : 21
      copies locales retirées — sœur de `.warnbox`, même boîte, autre couleur,
      et c'est ce voisinage qui a tranché sa géométrie plutôt qu'un arbitrage
      entre les copies), `Seg.svelte` (sept groupes segmentés recopiés — trois
      axes de variation et trois seulement, chacun porté par une raison :
      `vertical`, `tone` au barème du rouge §7.2ter, et `size` nommée par son
      rôle, jamais par une taille), `FicheHeader` (l'en-tête des cinq fiches :
      tuile, nom éditable, sous-titre, état, favori, ⋮), `NoteBlock` (la note
      libre, avec sa variante `bare` pour vivre dans un sous-onglet),
      `PickerBar` (le sélecteur de livrée/tracé), `Pencil` (le crayon
      « ça se reprend à la main », partagé par le nom, la description, l'auteur
      et la note — le premier jet de la note en avait posé un **décoratif**,
      qui ressemblait à un bouton sans en être un), `.lbl-sub` (neuf copies du sous-titre
      d'écran ; `max-width` reste à l'appelant, la largeur de mesure d'un
      paragraphe dépendant de la colonne qui l'accueille et non du rôle du
      texte). **Trois homonymes ont été renommés au passage** — `.sub`
      désignait aussi un en-tête de dialogue, un message sous un champ et un
      surtitre rouge posé au-dessus de son titre : un nom qui veut dire trois
      choses est un piège au premier déplacement de markup.
      **Inventaire de ce qui reste** (mesuré le 2026-08-18, revu le
      2026-09-11) :
      - **Enregistrer / charger / supprimer une liste nommée : 2 copies**, et
        c'est la seule entrée de cet inventaire qui ne soit pas du style mais
        du **comportement**. Les sessions enregistrées (`SavedSessionsDialog`
        pour le nom, la liste de chargement restée inline dans `Launch.svelte`)
        et les tenues de pilote (`DriverOutfits.svelte`) font le même geste
        avec deux implémentations. Demandé par l'utilisateur, qui l'a reconnu
        d'un écran à l'autre. **Le mutualiser demande d'abord d'extraire la
        moitié « liste » de `Launch.svelte`** — c'est là qu'est le travail, pas
        dans le dialogue de nommage.
      Un lot de ce genre est du **reformatage pur sur une quinzaine de
      fichiers** : le faire dans son propre commit, jamais mélangé à un
      changement fonctionnel (sinon `git blame` devient inexploitable).
      **Une brique ne se crée pas avant son premier client** : `Toolbar`,
      `ListRow` et la coquille de fiche attendent donc les lots de la refonte
      qui les consomment (L2, L7), pour la même raison qu'une colonne SQL que
      rien n'écrit ni ne lit pourrit.
- [ ] **Vignettes régénérées de la grille** — **fusionné dans `main`, mais
      éteint** : `FEATURE_GRID_THUMBS` est à `false` dans `src/lib/features.ts`,
      qui porte le mode d'emploi de l'interrupteur. Il ne reste qu'un réglage,
      long et fastidieux, et trois presets livrés avec des valeurs provisoires
      seraient jugés sur elles. Spec dans `docs/SPEC-grille.md`, dont le §11 dit ce
      que l'implémentation a changé et pourquoi. La partie A (§2 à §4) était
      déjà livrée ; la **partie B** (§5 à §8) l'est — pipeline, tâche de fond,
      écran de réglage, profils à l'installation — et le modèle de **presets**
      est venu après, d'une remarque de l'utilisateur : la preview d'origine
      d'Assetto Corsa est plus *jolie* que notre rendu, le nôtre plus
      *lisible*. Deux objectifs, pas deux qualités d'exécution du même — d'où
      *Catalogue*, *Vitrine* et *Officiel*, embarqués et en lecture seule,
      qu'on duplique pour s'en faire un, et un preset **par densité de
      grille**.
      **Les pièges à ne pas réintroduire**, tous mesurés ou vécus :
      1. **Ne jamais faire passer la génération par `prepare_car_preview`.**
         C'est le §5.3, et il a une raison chiffrée : 312 conversions dans un
         cache déjà à son plafond évincent une entrée vivante chacune, donc les
         aperçus que l'utilisateur consulte vraiment. La voie parallèle est
         `preview::prepare_scratch` → brouillon vidé avant chaque conversion,
         jeté après le rendu.
      2. **Le brouillon a un verrou côté frontend** (`withScratch`), en plus du
         verrou de conversion côté Rust. Celui-ci ne protège que la conversion ;
         la fenêtre pendant laquelle three.js va chercher géométrie et textures
         vient après, et la conversion suivante vide le dossier en commençant.
         Symptôme d'un oubli : une voiture blanche, sans texture.
      3. **Un rendu abîmé ne se voit plus une fois le PNG écrit.** Contexte
         WebGL perdu ou textures non arrivées : le fichier existe et son nom est
         valide, donc il est resservi pour toujours. D'où `checkPlausible`
         (réduction à 64×36, refus du vide et du saturé) **avant** l'écriture.
         Le bouton « refaire la vignette » de la fiche a existé puis a été
         retiré, à la demande de l'utilisateur : on parie sur le contrôle, et
         `regenerateGridThumb` reste, un `{#if}` de le ramener si le cas revient.
      4. **Le mat n'entre pas dans l'empreinte d'une vignette** et ne doit
         jamais y entrer : il ne change aucun pixel du PNG. L'y mettre ferait
         régénérer trois cents images pour un changement de couleur de carte.
      5. **Le balayage garde les gabarits de *tous* les presets vivants**, pas
         seulement le dernier appliqué. Sinon chaque changement de densité
         relance cinq minutes de travail, et la coexistence des deux jeux — tout
         l'intérêt du preset par densité — tombe.
      6. **Le mat n'entre dans l'empreinte que quand un preset le cuit**
         (`background > 0`). Toujours le hacher ferait régénérer 312 images pour
         un changement de thème ; ne jamais le hacher laisserait un fond cuit
         qui ne correspond plus à sa carte. Le `renderTemplate` de
         `gridThumbPrefs` est le seul endroit où le gabarit et le mat se
         rejoignent : **tout ce qui rend ou cherche une image passe par lui**,
         jamais par `preset.template` directement.
      7. **`RENDERER_VERSION` s'incrémente dès qu'une correction change les
         pixels produits** (`gridThumbPrefs.svelte.ts`) — même règle et même
         raison que `preview::CONVERTER_VERSION`. Une vignette porte le `.kn5`,
         la livrée, les configs CSP, la version du convertisseur et le
         gabarit… mais rien du code qui dessine : sans cette version, une image
         fausse reste servie pour toujours, parfaitement valide au regard de
         tout ce que son nom sait vérifier. Vécu au premier bug de rendu.
      8. **La génération est un travail de fond, et ça se paie en cœurs.**
         `kn5_gltf::with_background_pool` la borne à la moitié des unités de
         calcul ; sans ça la machine est prise en entier et l'interface saccade
         — d'autant plus visible sur une grosse machine. La file rend aussi la
         main 16 ms entre deux voitures (rendu et écriture du PNG sont sur le
         fil principal), et elle **se suspend au lancement d'une session**,
         jusqu'à la fermeture du jeu. Les deux bouts ne suivent pas le même
         signal : pause **dès le clic** sur « Démarrer », reprise sur la
         disparition du process. C'est la différence avec la musique de Big
         Picture, qui continue pendant tout l'écran de chargement et ne se coupe
         qu'une fois la voiture pilotable — les conversions, elles,
         rallongeraient précisément ce chargement. Le process est déjà surveillé
         par `music/watch.rs` (sondage de `acs.exe`), qui rend maintenant compte
         à une **fermeture** en plus du canal du moteur audio : un seul sondage
         pour deux clients. Les pauses sont un **ensemble de raisons** et non un
         booléen — fermer l'atelier ne doit pas relancer la génération pendant
         que le jeu tourne.
      9. **Un fond cuit rend le contrôle du vide inopérant** : l'image est
         opaque partout, donc « rien n'a été dessiné » ressemble à « tout va
         bien ». D'où le troisième critère de `checkPlausible`, l'écart de
         luminance — un fond seul est un dégradé très doux, une voiture y ajoute
         forcément des clairs et des sombres.
      **Écarts assumés vis-à-vis de la spec** : l'ombre de contact est une vraie
      ombre projetée et non l'ellipse peinte du §5.6 ; l'azimut n'a pas été
      « relevé sur les previews Kunos » puisque c'est déjà fait (318°, l'aperçu
      3D) ; la tâche de fond réduite est une barre et non une pastille à anneau
      (la pile bas-droite n'a qu'une forme) ; les six voitures de l'atelier sont
      prises par **catégorie** (plus la voiture de session en tête) et non par
      silhouette, que rien dans les données ne dit ; et le §5.7 (versionnage du
      gabarit d'origine) est **supprimé**, rendu inutile par les presets
      embarqués.
      **Trois embarqués**, et ils ne poursuivent pas le même but : *Catalogue*
      (identifier vite, image détourée), *Vitrine* (avoir envie de regarder,
      flaque + reflet cuits dans l'image), *Officiel* (être indiscernable d'une
      `preview.png` du jeu, fond cuit, contenu de base laissé tel quel).
      **Reste — et c'est la seule chose qui reste : finaliser les valeurs par
      défaut des trois presets.** Celles livrées sont une passe de réglage, pas
      un point d'arrivée : l'utilisateur les a posées à l'écran et les reprendra
      plus tard, le travail étant fastidieux. Elles se figent dans l'atelier
      (`Réglages › Vignettes`), comme les défauts de l'aperçu 3D l'ont été, et
      il n'y a **rien à coder pour ça** — seulement à recopier les nombres
      arrêtés dans `BUILTIN_PRESETS`.
      **Pour reprendre :** passer `FEATURE_GRID_THUMBS` à `true`, et vérifier
      les trois points d'entrée qu'il rallume — l'onglet `Réglages › Vignettes`,
      la deuxième page de l'assistant (les trois profils), la tâche de fond.
      Rien n'a été effacé en s'éteignant : les images déjà produites sont dans
      `app_cache_dir/gridthumbs/` et les presets de l'utilisateur dans
      `ui_prefs.json`. Les images, en revanche, se refont dès que les valeurs
      d'un preset embarqué changent — c'est le fonctionnement normal de
      l'empreinte, pas un accident.
      Trois choses à savoir avant d'y toucher :
      - les trois partagent le cadrage arrêté à l'écran (angle 320°, focale
        26°, hauteur 8 %) ; les séparer est possible mais c'était un choix ;
      - la principale de Vitrine à 30 % **n'est pas une coquille** : complément
        et contre-jour s'expriment en pourcentage d'elle, donc la descendre
        éteint tout l'éclairage direct et laisse le showroom seul. La remonter
        remonte aussi les deux autres ;
      - le cadrage d'Officiel a une **mesure** derrière lui (§11.4 de la spec) ;
        s'en écarter est permis, l'ignorer serait dommage.
- [ ] **Écran Pilote** (branche `feature/ecran-pilote`). Spec et maquette dans
      `docs/SPEC-ecran-pilote.md` + `docs/pitbox-ecran-pilote.html`, résumé au
      §9.5 du SPEC. **À lire avant de reprendre** — l'asymétrie qui structure
      tout l'écran (le corps commande, la tenue en découle) y est expliquée une
      fois pour toutes.
      Fait : le backend liste les corps installés et écarte ceux sans squelette
      (`driver::bodies`, 45 sur 52 à la référence) ; l'époque d'un corps se lit
      sur sa texture de casque ; substituer le corps fait tomber la garde-robe
      de la livrée. Côté écran : section `driver`, ligne « Mon pilote » avec
      badge dans la colonne de session (elle remplace la bascule et les trois
      menus déroulants), panneau d'essayage + galerie, survol = essai / clic =
      adoption, favoris, récents, recherche, regroupement, bannière
      d'invalidation, états vides.
      Le plateau est en 3D : le mannequin seul (`kn5_gltf::standalone_driver`,
      pas `graft` avec un hôte vide — trois de ses quatre tâches parlent d'une
      voiture qui n'est pas là) et un cadrage par piste déduit du rig. La
      galerie des corps porte des vignettes rendues à la demande
      (`driverThumbs.svelte.ts`), une à la fois, au défilement, et **le PNG est
      conservé hors du plafond du cache d'aperçus**. Ce dernier point est un
      bug corrigé, pas une précaution : les 45 conversions écrivent ~180 Mo
      dans un pool déjà à ses 2 Gio, donc chacune évinçait une entrée plus
      ancienne — y compris les mannequins des vignettes précédentes. Le cache
      se mangeait lui-même et tout se recalculait à chaque visite.
      **Le volant générique du §D5 est abandonné**, décidé avec l'utilisateur
      après deux essais : sur les poignets (cerceau de 55 cm, personne ne le
      touche), puis sur les phalanges des majeurs — 13 cm plus en avant,
      mesuré, ce qui le met pourtant au bon endroit. Il ne convainc toujours
      pas à l'écran et n'a pas assez d'intérêt pour continuer. `DriverRig`
      garde `grip` : c'est le point que vise le cadrage « Gants ».
      **La mesure qui a débloqué le lot, et la correction qui a suivi** : les
      41 mannequins sur 44 qui partagent une pose de repos au millimètre
      (mains à ±0,277, 1,027, 0,530) *ressemblent* à une prise de volant et
      n'en sont pas — 55 cm d'écart, un volant de car, que les doigts ne
      touchent pas. Rendu à l'écran, c'était le principal défaut du premier
      essai. Appliquer la hiérarchie d'assise **et** l'animation de braquage
      de la voiture ramène l'écart à 35–43 cm selon la voiture, mesuré sur
      douze : le diamètre de son vrai volant. Donc `standalone` ne retire que
      l'ancrage (`DRIVEREYES`), jamais la pose — et la clé de cache porte la
      pose, puisque c'est elle qui décide du volant dessiné.
      **La deuxième mesure, celle qui rend le survol possible** : le `.jpg`
      qu'AC range à côté de chaque `.dds` de garde-robe est la **même image
      aux mêmes dimensions** (`HELMET_2012.dds` 2048×512 DXT5, son `.jpg`
      2048×512). Le survol échange donc la texture côté three.js — quelques
      millisecondes, comme la spec le suppose — au lieu de redemander une
      conversion. L'adoption, elle, reconvertit. Les noms d'image survivent
      dans le glTF (`2016_Suit_DIFF.dds#paint-babbba`), ce qui permet de
      retrouver le matériau à échanger ; les noms de *texture*, eux, sont
      vides — se fier aux images.
      **Le choix est passé par voiture** (`driverOverride.svelte.ts`), contre
      la spec qui le voulait global : une tenue choisie une fois s'imposait aux
      312 voitures, en silence. Cascade à trois niveaux — la tenue de cette
      voiture, puis la tenue par défaut si l'option est cochée, puis la livrée
      — le niveau 1 gagnant toujours. Rangé une clé par voiture dans
      `ui_prefs.json` sur le patron de `preferred.ts`, **parce que le filtre
      « Pilote choisi » de la bibliothèque lit ce drapeau par carte**, donc de
      façon synchrone (`peekUiPref`).
      **Reste, dans cet ordre :**
      1. **Écart spec/réalité à trancher avec l'utilisateur** : le §6.3 range
         les époques par ce que désigne la *famille*, mais mesuré sur
         l'installation, c'est la *variante* qui porte le sens en 1969
         (amon, clark…) et en 1985 (les couleurs). D'où le repli implémenté :
         un regroupement qui ne produirait qu'un groupe passe en grille plate.
      2. **Casque posé de travers sur un mannequin à pièces statiques.**
         `rh_schuberth_helmet_driver_19` traverse sa propre figure. Il est le
         cas que rien d'autre ne représente : cinq maillages **statiques**
         (casque, visière, HANS, prise d'air, visage) accrochés au nœud
         `DRIVER:RIG_Head`, là où les autres mannequins sont entièrement
         skinnés. Or `pose::apply_locals` ne remplace la transformation **que
         des nœuds `Dummy`** — un maillage nommé par la hiérarchie est ignoré.
         À vérifier avant de corriger : c'est le casque qui bouge, ou la tête ?
      3. **Poser le pilote en jeu — fait.** Le **corps** passe par une section
         `[DRIVER3D_MODEL] NAME=…` dans `<voiture>/extension/ext_config.ini`
         (CSP surcharge une section de `data.acd` en préfixant le nom du
         fichier ; `data.acd` n'est pas touché, donc le checksum tient), la
         **tenue** par le `skin.ini` de la livrée, sous le nom du mannequin —
         il n'existe aucune route CSP pour celle-là, cherchée et non trouvée.
         Tout est dans `docs/csp-driver-research.md`, et le code dans
         `driverapply.rs`, appelé par `launch()` : une seule entrée `sync()`
         qui pose quand la voiture a un choix et **retire** quand elle n'en a
         plus. Conséquence à ne pas rater : remplacer le corps rend la section
         de la livrée inopérante, la tenue doit être réécrite sous le
         **nouveau** nom de mannequin. Le déploiement étant en hardlink,
         l'écriture efface le fichier avant de le réécrire — sinon c'est la
         copie de bibliothèque qu'on modifie. **Ne pas écrire `POSITION`**
         dans `[DRIVER3D_MODEL]`, contrairement à ce que suggèrent les réponses
         trouvées en ligne : c'est le même `[MODEL] POSITION` que
         `seating_offset` a mesuré comme inapplicable.
      **Écarts assumés vis-à-vis de la spec, décidés avec l'utilisateur** :
      le favori se pose sur le cœur de la bibliothèque (`♥`/`♡`) placé sous
      l'image et non sur elle — on garde l'argument du §7.3 (l'échantillon est
      montré entier, un bouton posé dessus en cache un morceau) en prenant le
      glyphe du reste de l'app ; les **tenues enregistrées**
      (`driverOutfits.svelte.ts`, §13 complété) reposent les quatre pièces
      d'un clic — le corps d'abord, sinon `setDriverBody` efface les trois
      autres juste après les avoir posées ; le badge `MODIFIÉ` du §3.2 est
      retiré, la ligne de session disant désormais elle-même « Tenue
      d'origine », le nom de la tenue enregistrée, ou « Tenue personnalisée » ;
      et une pièce gardée qui ne s'applique pas au corps courant est **barrée**
      dans sa piste au lieu d'être affichée comme active — elle est conservée
      (§13) mais ne change rien, ce que rien ne disait.
      Le **verrou « voiture de course »** du §11.2 est retiré (décidé avec
      l'utilisateur) : son texte était devenu faux, le corps se pose là comme
      ailleurs, et un écran intermédiaire qu'un clic franchit ne protège rien.
      **Ce qui n'est pas tranché** : ce qu'AC met dans son checksum en ligne.
      La tenue ne demande qu'un `skin.ini`, fichier de skin — probablement sûr,
      non prouvé ; le corps ne touche qu'`ext_config.ini`, hors `data.acd`.
      Deux mesures à garder en tête : les trois voitures dont la `.knh` est
      vide retombent sur `DRIVEREYES`, et `[MODEL] POSITION` ne doit **pas**
      être appliqué (voir `seating_offset`).
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
      Reste surtout le choix du LOD — §15.
      Le réglage de qualité se réduit au suréchantillonnage : une passe SMAA
      a été essayée, déplacée, puis retirée faute de gain visible pour un
      gigaoctet de mémoire. Il subsiste du crénelage sur les lignes claires
      quasi horizontales ; le prochain essai est **en amont** (normales,
      rugosité), pas un filtre de plus. §15 point 8.
      (R et B de `txMaps` : question close, par la négative ; la métallicité
      vient de `fresnelC` — écarts n°7 et n°10 de `kn5-format.md`.)
      Trois règles à ne pas perdre de vue :
      **`preview::CONVERTER_VERSION` s'incrémente dès qu'on touche au rendu
      produit** (sinon les anciens `.glb` restent servis — la version est dans
      le *nom* des entrées de cache, ce qui permet aussi d'effacer les
      périmées) ; **une conversion ne se valide jamais sur une seule voiture**
      — l'atlas de la MX-5 est symétrique et a masqué une erreur de repère
      pendant deux lots ; et **le `preview.jpg` d'un skin est une référence de
      cadrage, pas de luminosité** — il est plus sombre que le rendu du jeu, ce
      qui m'a fait diagnostiquer un écart inexistant.
- [ ] **Enrichissement Wikipédia de la fiche** (branche
      `feature/wikipedia-fiche-detail`). Un extrait de l'article du véhicule ou
      du circuit **réel**, dans un onglet à côté de la description de l'auteur.
      Spec : `docs/SPEC-wikipedia-fiche-detail.md`, et son §1 commande tout —
      la fonctionnalité est **décorative**, donc l'ambiguïté n'affiche rien et
      l'absence n'est jamais une erreur. Son §2 est juridique et non
      négociable : le texte reste une **collection** (jamais fusionné à la
      description, jamais reformulé, résumé ni traduit — surtout pas par un
      modèle de langage), sinon le ShareAlike de CC BY-SA remonte sur l'app.
      **Fait : les lots 1 à 3, sans interface** — `src-tauri/src/wiki/` : les
      trois tables dans l'overlay (§3), la chaîne de repli et la remontée d'un
      cran (§5), le client Action API (§6), et l'appariement automatique des
      voitures et des circuits (§4) avec sa commande de calibration.
      Le module porte un `allow(dead_code)` **assumé et daté** : rien ne
      l'appelle tant que l'interface (§7) n'existe pas. À retirer au premier
      client.
      **Les identifiants Wikidata sont dans `wiki/ids.rs`**, un par un relevés
      sur l'API vivante (§4.4 l'exige) — le libellé en commentaire est celui
      que l'API a rendu, et chaque entrée dit sur quel item réel elle a été
      confirmée. Ne pas en ajouter de mémoire.
      **Les seuils sont dans `Prefs`** (`wiki_match_*`, `wiki_track_*`) et la
      liste de nettoyage des noms dans `rules/wiki-matching.json`, semée dans
      le dossier de config et éditable — §4.3 l'exige, et c'est ce qui permet
      de régler la reconnaissance sans release.
      **Pour calibrer** (rien n'est persisté, le rapport sort en Markdown) :
      ```
      PITBOX_WIKI_LIMIT=20 cargo test --lib wiki -- --ignored --nocapture calibrate_the_library
      ```
      Quatre mesures ont corrigé la spec, et elles ne se retrouvent pas deux
      fois :
      - **La recherche géographique des circuits tourne sur Wikidata, pas sur
        Wikipédia.** L'article anglais « Nürburgring » n'a *aucune* coordonnée
        GeoData (Suzuka non plus) : le `list=geosearch` de la §4.2 ne peut
        structurellement pas rendre le circuit qui lui sert d'exemple. L'item
        Wikidata porte bien P625, et y chercher rend des Q-ids directement.
      - **Le filtre de type porte toute la stratégie circuit.** À 5 m du
        Nordschleife, les vingt items les plus proches sont dix-neuf éditions
        de Grand Prix, un village, un château et un ruisseau — le circuit n'y
        est pas, une vingtaine d'items partageant la coordonnée exacte. D'où
        `gslimit=50` et l'allowlist de `ids::TRACK_TYPES`.
      - **Les coordonnées viennent de CSP, pas des `geotags`.**
        `sun::track_location` les résout déjà pour 445 circuits ; les `geotags`
        des circuits Kunos sont le littéral `["lat", "lon"]`.
      - **L'année n'existe presque jamais** : aucune voiture mesurée ne porte
        P571, seules les générations portent P580/P582. Elle est donc un bonus
        quand elle existe et jamais une pénalité quand elle manque — son poids
        quitte le dénominateur.
      Trois choses à savoir avant d'y toucher :
      - **Le client HTTP est WinHTTP**, via le crate `windows` déjà présent
        (`wiki/http.rs`) : le projet n'avait aucun client HTTP, et deux GET
        JSON ne justifiaient pas une trentaine de crates plus une pile TLS.
        L'OS fournit TLS, proxy, redirections et délais. Les deux solutions
        écartées (`reqwest` + `native-tls`, `ureq`) sont notées dans le fichier
        avec ce qui les ferait gagner — le jour où l'app cesse d'être
        Windows-only, c'est `reqwest`. Corollaire : tout est **bloquant**, donc
        les futures façades passent par `spawn_blocking`.
      - **Les deux formats de réponse ont été relevés sur l'API réelle**, pas
        déduits : `query.pages` est un *tableau* en `formatversion=2`, le
        parent se lit en `claims.P361[0].mainsnak.datavalue.value.id`, et les
        sitelinks mélangent `commonswiki` aux langues. Un test ignoré
        (`talks_to_wikipedia_for_real`) rejoue le tout contre le vrai service —
        c'est la seule preuve que le FFI WinHTTP fonctionne, la CI ne
        l'exécutant pas (§11 : aucun test ne dépend de Wikipédia).
      - **Un 404 et un réseau coupé ne sont pas le même non-résultat**
        (`api::Fetched`). Les confondre écrirait « pas d'article » dans le
        cache négatif pour 90 jours à cause d'un tunnel.
      **Reste** : **régler les seuils sur un vrai passage de calibration** —
      les valeurs livrées sont celles du §13, un point de départ, et un premier
      échantillon de huit mods n'en a retenu qu'un : le plancher de score et le
      signal « marque » méritent d'être revus sur les 312 (une Acura NSX ne
      correspond pas à la Honda NSX par la marque). Puis les lots 4 à 6 du §12 :
      interface (§7), correction manuelle (§7.6), réglages (§8, dont
      l'interrupteur « enrichissement en ligne » que `resolve_article` attend
      déjà sous la forme d'un `net: Option<&WikiClient>`).
      **Un trou de la spec à combler au lot 5** : la §7.6 accroche « Ce n'est
      pas le bon article ? » au menu de l'onglet, mais la §7.1 masque l'onglet
      quand il n'y a pas de contenu — donc un mod rejeté pour ambiguïté n'a
      aucun point d'entrée vers la correction manuelle, alors que c'est
      exactement le cas où choisir servirait. Décidé : déplacer l'entrée vers
      le menu ⋮ de la fiche, toujours présent. La plomberie existe déjà
      (`set_link` + précédence).
      TTL : 30 jours en positif, 90 en négatif.
- [ ] **Signature Authenticode** : le workflow est prêt, il attend un
      certificat. Définir la variable de dépôt `SIGN_COMMAND` suffit à
      l'activer — voir `docs/windows-code-signing.md` (lire **avant** d'acheter,
      deux pièges y sont documentés). Repo passé public, `v0.1.0` publiée,
      README doté de la « Code signing policy » exigée : candidature déposée
      auprès de la SignPath Foundation (certificat gratuit, projet open
      source), réponse attendue par email. Si refus ou trop long, plan B
      documenté : Certum Open Source Code Signing (~49€/an, cloud SimplySign,
      pas de jeton USB).
- [ ] **Refonte de la navigation et des fiches** (branche
      `feature/refonte-navigation`). Rail à deux rangs, inventaire unique des
      compléments, une seule anatomie de fiche, notes sur toutes les entités.
      **Spec, maquette et plan de livraison dans `docs/`** —
      `PLAN-refonte-navigation.md` porte l'ordre des lots et, surtout, les
      **mesures faites sur la bibliothèque réelle avant de commencer** : elles
      ont supprimé un lot entier (la détection CSP des voitures existe déjà,
      167 sur 311) et démenti le fourre-tout redouté (19 des 28 mods « autres »
      sont des mannequins). **Les neuf lots sont faits** (L1 à L9), le premier
      palier est fusionné dans `main`, le second reste à fusionner. Le détail
      lot par lot, avec ses écarts assumés, est dans le plan — ne pas le
      recopier ici.
      **Quatre points de la spec sont tombés à la mesure** plutôt qu'en
      implémentation, et c'est le genre d'information qu'on ne retrouve pas
      deux fois : la détection CSP des voitures existait déjà (167 sur 311),
      l'export des notes n'a nulle part où aller (il n'existe aucun export des
      métadonnées de l'overlay), la valeur d'origine des unités est déjà la
      seule affichée (rien n'est converti), et le retrait des mannequins de
      l'inventaire — pourtant demandé par le §5 — s'est révélé **nuisible** :
      la galerie de l'écran Pilote est un sélecteur, elle ne gère rien, et les
      en retirer supprimait le seul endroit d'où on pouvait les désactiver ou
      les supprimer. Les livrées tranchent par l'exemple : **choisir et gérer
      sont deux gestes, ils peuvent avoir deux écrans.**
      **Reste** : « aussi dans … » (§4.5, un mod rattaché à plusieurs entités),
      les points ouverts §14 à reposer avec l'inventaire réel sous les yeux, et
      le markdown dans les notes (demandé à l'usage, voir le §4bis du plan).
## Fin de tâche — dans cet ordre

1. **Mettre à jour `docs/SPEC.md`** dès qu'une évolution change le
   comportement de l'app. Le SPEC décrit ce que l'app *est* : il doit rester
   synchrone avec le code. Une simple correction de bug ne s'y écrit pas.
2. **Vérifier** — une seule commande, qui enchaîne exactement les portes de la
   CI dans l'ordre qui les satisfait (`tauri-build` exige que `build/` existe,
   donc le build front précède `cargo test`) :
   ```bash
   npm run verify
   ```
   Types + locales + version, build front, `cargo fmt --check`, clippy et les
   tests. `npm run check` doit rester à 0 erreur **et 0 warning**, et **clippy
   à 0 warning** — la CI les traite en erreurs.
   Elle ne couvre pas l'**empaquetage de l'installateur**, le seul job qui n'a
   pas d'équivalent local : c'est pourquoi le tag d'une release ne se pousse
   qu'après le vert de la CI (voir la section « Releasing » du README).
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

Côté Rust, tout en module, `#[cfg(test)] mod tests` en fin de fichier, pas de
dossier `tests/` : c'est là qu'est le risque réel, là où une erreur détruit des
fichiers de jeu.

Côté front, **Vitest sur la logique pure uniquement** (`vitest.config.ts`,
`npm run test`, enchaîné dans `npm run verify`) : un fichier `*.test.ts` à côté
de son module, aucun test de composant, pas de jsdom, pas de
`@testing-library`. La config est séparée de `vite.config.js` et n'a pas le
plugin SvelteKit — monter un composant demanderait trois dépendances de plus,
c'est une décision à reprendre explicitement le jour où elle se pose, pas à
franchir par inadvertance. Le typage reste le travail de `npm run check` ; le
test ne sert qu'aux fonctions dont les cas limites ne se vérifient qu'en les
exécutant (`displayName.ts` et ses règles de coupe contradictoires — dont le
premier test a d'ailleurs trouvé une table d'alias inatteignable).

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
