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

**Les erreurs backend destinées à l'utilisateur sont des clés, pas des
phrases.** Une `String` française renvoyée par une commande Tauri atterrit telle
quelle dans l'UI et n'est traduisible nulle part. Donc :
`ok_or(crate::errors::AC_NOT_CONFIGURED)?` (constante = `"errors.acNotConfigured"`),
résolue côté front par `errorText(e)` de `$lib/errors`. Les détails techniques
(E/S, SQLite, 7-Zip) gardent leur message brut : ce sont des diagnostics, pas
des conseils. Toute nouvelle erreur user-facing ajoute sa constante dans
`errors.rs` **et** sa clé dans les deux locales.

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
3. **CM est maître de `race.ini`.** On pilote des presets via le protocole
   `acmanager://`, on n'écrit pas les fichiers du jeu à la main.
4. **Aucun fichier du jeu altéré durablement.** Si un fichier d'AC doit être
   touché (cas exceptionnel), il est sauvegardé et restauré — et il faut un
   filet de sécurité au démarrage pour les fermetures anormales.

## Structure du projet

```
src-tauri/src/          Backend Rust — un module par domaine
  lib.rs                Point d'entrée : mod, état partagé, setup, invoke_handler
  commands/             Façades #[tauri::command], un fichier par domaine
  errors.rs             Clés i18n des erreurs destinées à l'utilisateur
  overlay.rs            Base SQLite : schéma, migrations ALTER idempotentes, CRUD
  importer.rs modscan.rs archive.rs    Import : détection, extraction, classement
  activation.rs deploy.rs compose.rs layers.rs   Déploiement dans content/
  library.rs submods.rs apps.rs others.rs        Bibliothèque et add-ons
  launch.rs quickdrive.rs weather.rs   Lancement de session via CM
  rules.rs harmonize.rs                Moteur de tags
  maintenance.rs export.rs             Outils
  uijson.rs inspect.rs identity.rs     Lecture des fichiers AC
src/lib/
  components/           Composants Svelte (un écran = un composant)
  *.ts                  Bindings typés vers les commandes Tauri
  i18n/locales/         fr.json + en.json
  styles/global.css     Design system Rosso Corsa
docs/                   Documentation (voir ci-dessous)
```

Ajouter une fonctionnalité backend = 3 endroits : la fonction dans son module
métier, la façade `pub fn` dans `commands/<domaine>.rs` **et** son inscription
dans `invoke_handler![…]` de `lib.rs`, puis le binding typé dans le
`src/lib/*.ts` correspondant. Oublier `invoke_handler` ne casse rien à la
compilation — l'erreur n'apparaît qu'à l'exécution.

Une façade ne fait que charger la config, prendre le verrou SQLite et déléguer.
Toute logique qui grossit dans `commands/` doit descendre dans son module
métier. Les commandes sont `pub` (obligatoire hors du crate racine) et
partagent `commands::prelude`.

`Prefs` (`config.rs`) est en `#[serde(default)]` : un champ retiré est
simplement ignoré dans les `config.json` existants, pas de migration à écrire.

## Documentation

- **`docs/README.md`** — index de tout `docs/`. Point d'entrée.
- **`docs/SPEC.md`** — spécification de référence, organisée par domaine.
  Décrit l'app telle qu'elle fonctionne. **La source de vérité.**
- **`docs/*.html`** — maquettes visuelles (référence de layout et de thème).
- **`docs/*-research.md`** — comptes rendus de recherche sur les points durs
  (lancement CM, aperçu 3D). Contiennent le *pourquoi* de choix non évidents et
  la trace des pistes abandonnées. À lire avant de retenter quelque chose.
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

`src-tauri/rustfmt.toml` fixe le style (`max_width = 120`) et `cargo fmt --check`
est dans la CI. Un reformatage massif se fait dans un commit isolé, jamais
mélangé à un changement fonctionnel : sinon `git blame` devient inexploitable.

## Chantiers en cours

Liste vivante : **retirer chaque entrée dès qu'elle est faite**, ne pas la
laisser pourrir ici.

- [ ] **Découpage des monolithes Svelte.** `DetailPage.svelte` (1725 l.) et
      `Launch.svelte` (1522 l.). Les blocs Couches et Ressources sont déjà
      sortis dans `components/detail/` ; restent Tags, Versions, Historique,
      Provenance côté fiche, et les étapes côté Lancer. **Exige l'app lancée** :
      le CSS Svelte est scopé par composant, chaque extraction déplace des
      styles et `npm run check` ne prouve que la compilation, pas le rendu.
- [ ] **Index manquants** (une demi-heure, rentable dès la session suivante) :
      carte des écrans Svelte (quel composant = quel écran, `Transversal` sert
      trois entrées de menu, `Library` est rendu deux fois avec une prop) ;
      une ligne en tête de `global.css` disant quelles classes sont globales
      (`.btn`, `.mono`, `.input`) par opposition aux classes locales aux
      composants (`.lbl`, `.tag`, `.srcbox`) ; le fait que `t()` renvoie la clé
      elle-même quand elle manque.
- [ ] **Clés localStorage en littéraux dispersés** (`pitbox.session.car`,
      `pitbox.skin.<id>`, `pitbox.transversal.groupBy`…) → un module de
      constantes. Une clé mal orthographiée ne casse rien : elle perd
      silencieusement un réglage, et ça ne se retrouve jamais.
- [ ] **Signature Authenticode** : voir `docs/windows-code-signing.md`.
      L'étape Azure de `release.yml` est écrite mais commentée, et placée avant
      le build — à déplacer après, ou à passer par `bundle.windows.signCommand`
      pour que l'exécutable *dans* l'installateur soit signé lui aussi.
- [ ] **La CI n'a encore jamais tourné** : les workflows n'ont pas été poussés.
      Le premier push est leur vrai test.
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
   cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
   ```
   `npm run check` doit rester à 0 erreur (des warnings
   `state_referenced_locally` préexistants sont attendus) et **clippy à 0
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
concernée (`§4.6bis`) quand elle existe.

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
