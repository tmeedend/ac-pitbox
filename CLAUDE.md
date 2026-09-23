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
   **Mais il n'est pas au même endroit partout, et c'est le piège.**
   `activation.rs`, `apps.rs` et `maintenance::remove_orphan` appellent
   `remove_junction`/`deploy::remove_deployment`, qui refusent d'eux-mêmes ce
   qu'ils ne reconnaissent pas. `maintenance::delete_broken` est le seul à
   supprimer directement (`std::fs::remove_dir`), protégé par le seul `if
   is_junction(…) … else if is_deployed(…)` qui l'entoure : l'invariant y vit
   dans la condition, pas dans la fonction appelée. Simplifier ce bloc en un
   `remove_dir_all` est à un geste de distance, et **aucun test
   d'`activation.rs` ne le verrait** — d'où
   `delete_broken_never_touches_an_unmanaged_folder_in_content`, qui garde ce
   chemin-là pour lui.
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

## Le dépôt est public

Ça change une chose et une seule, mais elle est sans retour : **un secret
poussé est un secret brûlé.** Les robots moissonnent les dépôts publics en
quelques secondes, et le retirer dans un commit suivant ne le dépublie pas — il
reste dans l'historique, et il est déjà copié ailleurs. Un jeton exposé se
**révoque**, il ne s'efface pas.

Donc : jamais de clé, de jeton, de mot de passe ni de certificat dans un
fichier du dépôt, **même le temps d'un essai**. Ce qui est secret vit dans une
variable d'environnement ou dans les secrets du dépôt GitHub — c'est déjà ce
que fait la signature de l'installateur (`SIGN_COMMAND`, voir
`docs/windows-code-signing.md`).

`npm run check` refuse un jeton reconnaissable (`ghp_`, `sk-ant-`, `AKIA`…),
une affectation dont le nom dit « secret » avec une valeur longue, et un
fichier dont le **nom** annonce un secret (`.pem`, `.pfx`, `.env`). **Il ne voit
pas tout** : un secret qui ne ressemble à rien de connu passe. Le contrôle
attrape l'accident courant, il ne remplace pas l'attention.

Rien de personnel non plus — chemins de la machine, adresses, captures d'écran
d'autre chose que l'app. Le dépôt a été vérifié de bout en bout, historique
compris : il en est indemne aujourd'hui.

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
  catalog/              Catalogue de règles en deux couches (REGLES§2) :
                        types et fusion, partagés par l'app et rules-tool
  rules-tool/           CLI développeur du catalogue de règles, jamais livrée
src/lib/                Modules : un dossier par domaine, comme le backend
  shell/                La coquille : navigation, historique, zoom, défilement,
                        manette, Big Picture
  library/              L'écran bibliothèque : cartes, colonnes, filtres, lots
  detail/               La fiche : caractéristiques, médias, ressources, son
  wiki/                 L'onglet Wikipédia (WIKI§7)
  preview3d/            L'aperçu 3D des voitures (PREVIEW§7)
  launch/               La session : plateau, sessions et grilles enregistrées
  gridthumbs/           Les vignettes de la grille (GRILLE§5), éteintes
  driver/               Le pilote : corps, tenues, surcharges
  inventory/            Les compléments : apps, autres mods, sous-éléments
  workshop/             L'Atelier : règles, import, profils, maintenance
  *.ts                  Ce qui ne relève d'aucun domaine : `config`, `errors`,
                        `features`, `format`, `invokeSafe`, `storage`,
                        `uiPrefs`, `preferred`
  components/           Composants Svelte, mêmes domaines (voir la carte des
                        écrans) + `ui/` (les briques partagées) et `toasts/`
  components/detail/    Blocs extraits de la fiche détail
  components/inventory/Inventory.svelte  L'écran Compléments (§7bis du SPEC)
  components/detail/FicheHeader.svelte NoteBlock.svelte PickerBar.svelte
  components/ui/Pencil.svelte   Briques de fiche partagées par les cinq types
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

**Où va un fichier neuf.** `src/lib/` et `src/lib/components/` portent les
**mêmes noms de domaine**, et ce sont ceux de la carte des écrans ci-dessous :
un module et le composant qui le consomme se trouvent au même endroit dans deux
arbres parallèles. Le critère est **ce qui change en même temps**, pas la nature
technique du fichier — le pont typé vers les commandes, l'état `$state` partagé
et la logique pure d'un domaine vivent ensemble, parce qu'une évolution de ce
domaine les touche ensemble. Un fichier ne reste à la racine de `src/lib/` que
s'il n'appartient à **aucun** domaine (`errors`, `format`, `storage`…) ; dans le
doute, il appartient à un domaine.

**Deux dossiers de `components/` ne sont pas des domaines, et pas pour la même
raison.** `ui/` regroupe les briques partagées (`PLAN-refonte-navigation.md` §0.b) :
elles ne savent **rien du contenu** qu'on leur passe, et c'est vérifiable — un
composant de `ui/` qui importe `$lib/library/…` ou `$lib/detail/…` s'est trompé
de dossier. La seule dépendance qu'elles ont le droit d'avoir est la coquille
(`$lib/shell/zoom.svelte` pour la règle du zoom, `screenActions` pour
l'inscription manette de `Tabs`), parce que le cadre s'applique à tout contrôle
quel qu'il soit. `toasts/`, lui, est regroupé par **l'endroit à l'écran** : la
pile bas-droite (§4.2bis) où six bandeaux se disputent la même place. Chacun
connaît bien son domaine (`bulkState`, `importState`, `gamepadDevices`…) — les
ranger chacun chez soi les laisserait se dessiner indépendamment, ce qui est
exactement ce que la pile existe pour empêcher.

**Un import qui traverse un domaine s'écrit en absolu** (`$lib/library/filters`,
`$lib/components/ui/Slider.svelte`) ; seul un voisin de dossier garde la forme
relative (`./ConditionsBlock.svelte`). Une chaîne de `../../` ne dit pas où elle
va et se casse au déplacement suivant — c'est exactement ce que ce rangement
coûtait avant d'exister.

### Carte des écrans

`AppShell.svelte` est la coquille : **rail de navigation** (`NavRail.svelte`,
les lieux) + **colonne de session** (ce qu'on lance) + aiguillage sur
`nav.section` (`src/lib/shell/nav.svelte.ts`). Les trois territoires et leur
frontière étanche sont au §7.2 du SPEC. Correspondance section → composant :

Le rail a **deux rangs**, et ils ne classent pas par type de contenu mais par
**durée de validité** de ce qu'on y règle : *La session* (ce qui se décide à
chaque fois) et *Le jeu* (ce qui reste vrai jusqu'à nouvel ordre).

| Section | Composant | Note |
| --- | --- | --- |
| `cars` / `tracks` | `library/Library.svelte` | **rendu deux fois**, prop `kind` — persistance suffixée par type |
| `driver` | `driver/DriverScreen.svelte` | galerie des mannequins + panneau d'essayage |
| `apps` | `inventory/Apps.svelte` | écran à part entière depuis la refonte (§3.2) |
| `others` | `inventory/Inventory.svelte` | **l'inventaire des compléments** — cinq sources en une liste |
| `race` | `launch/Launch.svelte` | |
| `rules` / `categories` / `countries` / `import` / `profiles` / `maintenance` | `workshop/Workshop.svelte` | **un écran, six onglets** — l'onglet EST la section, pas un état local |
| `settings` / `about` | `settings/Settings` / `settings/About` | |

**Les trois écrans transversaux ont disparu** (Add-ons voiture, Add-ons
circuit, l'ancien fourre-tout) : ils classaient par mécanique d'installation,
et leur contenu est dans l'inventaire. `Transversal.svelte`, `OtherMods.svelte`
et `LayersSection.svelte` sont supprimés — la fiche d'un mod « autre »
(`OtherModDetail`) vit désormais par-dessus l'inventaire.

Deux pièges de ce regroupement : l'onglet de l'Atelier étant `nav.section`, un
`requestSection("import")` posé ailleurs (glisser-déposer global, rapport
d'import) continue d'atterrir au bon endroit — ne pas le remplacer par un état
local ; et `RulesEditor` reste le seul des six à gérer son propre
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

**Neuf d'entre elles sont désormais vérifiées** par `scripts/check-conventions.mjs`,
dans `npm run check` : `scrollIntoView`, écriture dans `localStorage`, mesure de
fenêtre écrite dans un style sans `zoomFactor()`, composant `.svelte` importé
nulle part, `#[tauri::command]` absente d'`invoke_handler`, clé i18n devenue
inatteignable, **renvoi de spec dans une chaîne visible** — « non activable
(§12bis.1) » s'affichait tel quel à l'utilisateur, en pointant vers une section
disparue —, **secret sur le point d'être versionné**, et **fichier de `docs/`
absent de l'index**. Une exception légitime se déclare sur la ligne ou juste
au-dessus : `// conventions: allow <règle>` — rare, et visible en revue.

Deux choses à savoir avant d'y toucher. **Une règle ajoutée se prouve** :
`node scripts/check-conventions-selftest.mjs` injecte une violation par règle et
vérifie qu'elle sort — une porte verte dont les règles sont cassées est pire
qu'aucune porte, et le banc en a démasqué une en naissant. Et **une règle qui
crie sur du code correct n'entre pas** : « un `let _ =` s'accompagne d'un
`log::warn!` » reste vraie mais donne 145 occurrences dont beaucoup de
légitimes, « aucune chaîne visible en dur » en donne 1 880 sans un vrai parseur
Svelte. Les deux sont documentées comme écartées, en tête du script.

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
  `zoomFactor()` (`shell/zoom.svelte.ts`) avant d'écrire, toujours.
  **Corollaire côté CSS : un seuil de mise en page est une `@container`, pas une
  `@media`.** Une requête de média interroge la fenêtre — donc un seuil que le
  zoom déplace, et qui de toute façon ignore ce que le rail et la colonne de
  session ont déjà pris. Une requête de conteneur interroge la largeur
  réellement disponible, la seule dont dépende la mise en colonnes. `DetailPage`
  déclare `container: detail / inline-size` sur `.page`, et tous ses seuils s'y
  réfèrent.
- **Un défilement programmatique ne doit jamais atteindre `<html>` ni
  `<body>`.** `global.css` les met en `overflow: hidden` exprès — « le document
  lui-même ne défile jamais, un scroll de page entraînait toute la coquille,
  barre de titre comprise, hors champ ». Le piège : `scrollTo()` et
  `scrollTop = …` **fonctionnent quand même** sur un élément en
  `overflow: hidden`, alors que la molette ne peut plus le ramener. Un
  décalage posé là est donc **définitif** — bande noire sous la fenêtre,
  coquille coincée, et aucun geste utilisateur pour revenir ; seul un
  redémarrage efface. Deux façons d'y tomber, toutes deux vécues sur le
  sommaire de l'onglet Wikipédia : `scrollIntoView()`, qui fait défiler *tous*
  les ancêtres scrollables jusqu'à la fenêtre, et un chercheur d'ancêtre
  scrollable qui remonte trop haut — l'`overflow-y` calculé de l'élément racine
  vaut « auto », pas « visible ». Faire défiler le conteneur d'écran, et
  s'arrêter avant `document.body`. **Les deux pièges sont pris en charge par
  `scrollIntoContainer` de `$lib/shell/shellScroll`** : c'est lui qu'on appelle, pas
  `scrollIntoView`, et `check-conventions.mjs` refuse le second. La règle était
  écrite ici et commentée dans deux fichiers ; elle était quand même violée
  dans deux autres, ce qui est précisément la raison d'être de la porte.
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
  que l'observateur d'historique (`shell/navHistory.ts`) voit passer la liste
  d'arrivée comme un écran à part entière : « précédent » y ramenait au lieu de
  rendre l'écran d'où l'on venait. Les deux écritures d'affilée, sans `await`
  entre elles, n'exposent qu'un seul état.
- **Le « ← » d'une fiche passe par `goBackOr`**, jamais par une fermeture
  sèche : l'historique d'abord, la fermeture en repli quand il n'y a rien
  derrière. Même règle pour le bouton B de la manette et les boutons latéraux
  de la souris — c'est le même geste, il ne peut pas avoir deux comportements.
- **Une table livrée ne se recopie jamais chez l'utilisateur.** Recopiée,
  elle devient sa vérité et plus aucune mise à jour ne l'atteint — c'est
  arrivé à tout `tag-rules.json`, semé une fois puis figé (il est désormais
  migré et mis de côté). Le modèle est `taxonomy.rs` et `rule_overlay.rs`
  (REGLES§2) : le catalogue relu depuis l'embarqué à chaque
  chargement, et à côté, dans un fichier à part, les seules **décisions** de
  l'utilisateur, indexées par la clé naturelle de l'entrée, une suppression
  étant une pierre tombale. La fusion des deux ne se calcule qu'à un endroit,
  la crate `pitbox-catalog`. **Pour faire évoluer ce catalogue, on ne
  l'édite pas à la main** : on cure dans l'app (Atelier › Catégories, Pays),
  puis `npm run rules:diff` / `npm run rules:promote`.
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

**Un renvoi `§` dit de quel document il parle.** Le code en porte près de
2 800, et le numéro seul ne suffit pas : `§5.3` désigne `SPEC-grille.md` dans
`gridthumbs.rs` et `SPEC.md` dans `importer.rs`. Donc : **`§4.5` nu = `SPEC.md`**
(le défaut, inchangé), et une **étiquette** pour les autres — `SESSION§3.2`,
`GRILLE§5.3`, `WIKI§4.2`, `PILOTE§6.3`, `PREVIEW§8.1`, `FMOD§2bis`,
`MUSIQUE§3.4`, `IMPORT§`, `REFONTE§`, `TEXTURE§`, `INDEX§`, `TAXO§`, `REGLES§`. La liste fait foi dans
`scripts/check-refs.mjs`.

**`npm run check` affiche aussi le poids de la documentation** — nombre de
documents, de lignes, et les trois sections les plus lourdes. **C'est un
rapport, pas une porte** : aucun seuil, aucun échec. `SPEC.md` a atteint
2 448 lignes dont 924 pour son seul §9 sans que personne le voie venir, parce
qu'un document grossit d'une ligne à la fois. Le nombre passe sous les yeux à
chaque vérification, et c'est l'œil humain qui décide quand découper — une
porte qui refuse un commit parce qu'un document a grandi de dix lignes finirait
désactivée.

`npm run check` vérifie que chaque renvoi **du code** tombe sur un titre qui
existe, et **les 2 785 y tombent** : le socle (`scripts/refs-baseline.json`) est
vide. Le contrôle est donc strict — le moindre renvoi sans cible fait échouer
`npm run check`. Il a démarré avec 348 renvois cassés hérités de
renumérotations successives ; ils ont tous été repris.

**Les renvois de `docs/` sont un rapport, pas une porte — et leur règle n'est pas
celle du code.** Le contrôle n'a longtemps regardé que `src/` et `src-tauri/` :
les documents qui *définissent* les sections n'avaient jamais eu leurs propres
renvois vérifiés. Dans un document, **un `§` nu vaut d'abord le document
lui-même** — une spec qui écrit « voir §7.2 » parle de son §7.2, et c'est le cas
le plus courant de loin (401 des 679). Appliquer la règle du code les
condamnerait tous. L'ordre est : le document, puis l'étiquette, et un défaut
seulement quand ni l'un ni l'autre ne répond.

**Les 76 renvois sans cible ont été repris, et cette moitié est donc devenue une
porte** — un renvoi mort dans `docs/` fait désormais échouer `npm run check`, au
même titre qu'un renvoi mort dans le code. Trois familles y étaient : des
sections déplacées (le §9 de `SPEC.md` est parti dans `SPEC-session.md` et a été
renuméroté, d'où onze renvois morts dans le seul `LOT1`), des renvois vers une
autre spec écrits sans étiquette, et sept sous-sections de `REFONTE§14` écrites
**en gras au lieu de titres** — elles existaient, aucun outil ne les voyait.

**Les 137 `§` nus qui sortaient de leur document ont été relus un par un**
(2026-09-16) : **75 visaient une autre spec** et portent désormais leur
étiquette — un document compagnon (`kn5-format.md`, le plan de refonte) renvoie
à la spec qu'il accompagne, et une entrée de `CHANTIERS.md` renvoie à la spec de
son chantier. Les **62 restants sont corrects** : un `§` nu vaut `SPEC.md`,
c'est la convention.

Ce second compteur **n'a donc pas vocation à tomber à zéro** — il est un
**fil-piège** : s'il bondit, quelqu'un a écrit des renvois sans se demander vers
quel document ils pointaient. Aucune règle mécanique ne sépare les deux cas, il
faut lire la phrase. `node scripts/check-refs.mjs --docs` liste le détail.

**Deux angles morts découverts en le vidant.** Une section peut exister sans
qu'aucun outil la voie : `REFONTE§14.1` à `14.7` étaient en **gras**, pas en
titres. Et un `§` nu peut être **avalé comme auto-renvoi** — le `§4` du plan de
refonte se résolvait sur son propre « 4. Les lots » alors qu'il visait
`REFONTE§4`, donc il paraissait juste tout en étant faux.

Le mécanisme du socle reste, pour le jour où une refonte en casse trente d'un
coup : `node scripts/check-refs.mjs --update` les gèle et on les reprend par
lots. **Mais on ne grossit pas le socle pour faire taire une erreur** — il est
là pour se vider, et il est vide.

- **`docs/README.md`** — index de tout `docs/`. Point d'entrée.
- **`docs/SPEC.md`** — spécification de référence, organisée par domaine.
  Décrit l'app telle qu'elle fonctionne. **La source de vérité.**
- **`docs/CHANTIERS.md`** — **le travail non terminé**, chantier par chantier :
  ce qui est fait, **ce qui reste**, les écarts assumés et les pièges déjà
  payés. C'est là qu'on regarde pour savoir quoi faire ensuite. Le tableau
  ci-dessous n'en est que l'index — une ligne par chantier, à tenir à jour
  en même temps que lui.
- **`docs/SPEC-import.md`** — l'import sur une page : une seule question
  (« où va ce fichier ? »), un arbre de décision, la table des mécanismes de
  pose, et les cinq archives réelles qui servent de tests. À lire **avant**
  de toucher à une règle d'import — le §4 du SPEC les décrit une par une,
  celui-ci les montre *ensemble*, ce qui est la seule façon de voir qu'une
  règle en contredit une autre. **Les deux ne se fusionnent pas** : un arbre de
  décision et la prose dont il est tiré sont deux objets, pas deux copies. En
  cas d'écart, `SPEC.md` fait foi, et la relecture se fait en même temps que la
  modification de la règle — jamais après.
- **`docs/maquettes/`** — les maquettes visuelles, **datées**, avec leur index
  (`maquettes/README.md`) : ce que chacune a servi à décider, et si elle fait
  encore autorité. Les périmées sont dans `maquettes/archive/`, gardées pour le
  *pourquoi* qu'elles portent. **Référence d'UX, pas d'UI** : voir la règle
  « Le design system fait foi » ci-dessous.
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

**Le détail vit dans `docs/CHANTIERS.md`** : avancement, écarts assumés, et les
pièges déjà payés une fois. Ici, juste de quoi savoir qu'un chantier existe et
où aller lire. Une entrée se retire **des deux endroits** dès qu'elle est faite.

| Chantier | Où il en est | Où lire |
| --- | --- | --- |
| **Sessions au format `.cmpreset`** | livré ; reste à vérifier dans CM qu'il ignore bien notre clé `PitBox` | `docs/SPEC-session.md` SESSION§3.6 |
| **Texture updates** | rien d'implémenté ; la règle de détection est mesurée et nette | `docs/SPEC-texture-update.md` |
| **Harmonisation des libellés** | libellés et barème du rouge faits ; restent le bleu, le vert et le jaune, sans barème | `docs/CHANTIERS.md` |
| **Vignettes de la grille** | fusionné mais **éteint** (`FEATURE_GRID_THUMBS`) ; reste à figer les valeurs des trois presets | `docs/SPEC-grille.md` |
| **Aperçu 3D natif** | lots 0 à 6 validés à l'écran ; reste le choix du LOD | `docs/SPEC-preview-3d-kn5.md` PREVIEW§13 à §15 |
| **Enrichissement Wikipédia** | livré et nettoyé ; reste la seule mesure des seuils, sur tes corrections manuelles | `docs/SPEC-wikipedia-fiche-detail.md` |
| **Signature Authenticode** | le workflow est prêt, il attend un certificat | `docs/windows-code-signing.md` |
| **Refonte navigation et fiches** | livrée et fusionnée ; restent trois questions ouvertes, dont le markdown dans les notes | `docs/PLAN-refonte-navigation.md` |
| **Taxonomies** | familles, onglets Catégories et Pays livrés ; reste l'onglet Marques (fusions, logos canoniques) | `docs/SPEC-taxonomies.md`, `docs/CHANTIERS.md` |
| **Catalogue de règles** | tout en deux couches (catalogue + décisions), `tag-rules.json` retiré ; restent le rapport de mise à jour et l'écran Règles refait |  `docs/SPEC-regles.md`, `docs/CHANTIERS.md` |
| **Règles de tags divergentes** | deux fichiers décrivent la même ontologie, un seul est chargé | `docs/CHANTIERS.md` |

## Fin de tâche — dans cet ordre

1. **Mettre à jour `docs/SPEC.md`** dès qu'une évolution change le
   comportement de l'app. Le SPEC décrit ce que l'app *est* : il doit rester
   synchrone avec le code. Une simple correction de bug ne s'y écrit pas.
   Si le travail avance un chantier, **`docs/CHANTIERS.md` dans la foulée** —
   et son entrée se retire quand il est fini, ici comme dans le tableau des
   chantiers. Les deux fichiers ne disent pas la même chose : le SPEC dit ce
   que l'app fait, le journal dit ce qu'on a appris en le construisant.
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

**Le design system fait foi, pas la maquette.** Sauf indication contraire
explicite, une maquette (`docs/maquettes/`, un fichier reçu, une capture) apporte
l'**UX** — ce qu'on montre, dans quel ordre, quel geste fait quoi, ce qui est
groupé avec quoi. L'**UI** vient de l'application : les jetons de
`styles/global.css` (`--rosso`, `--muted`, `--faint`, `--line`, `--yellow`…),
les quatre niveaux de libellé, le barème du rouge (§7.2ter), les composants
partagés, les tailles et les graisses déjà en place.

Concrètement : reprendre d'une maquette sa disposition et son intention, jamais
ses couleurs ni ses polices. Une maquette écrit couramment des jetons qui
n'existent pas ici (`--text-secondary`, `--border`, `--warn`) ou des valeurs en
dur — les traduire vers les nôtres au lieu de les recopier, et le dire dans le
compte rendu. Une maquette qui introduirait un cinquième gris ou un rouge de
plus se trompe sur ce point précis : c'est elle qui s'aligne sur l'app, pas
l'inverse.

Cette règle vaut aussi pour la géométrie quand elle porte du sens : la hauteur
d'un contrôle, l'écart entre deux réglages et l'alignement d'une rangée sont
déjà arbitrés par les composants partagés. Si une maquette demande autre chose,
c'est une demande d'UX à traduire, pas un gabarit à reproduire au pixel.

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
