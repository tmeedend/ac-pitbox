// Feature switches — source constants, not settings.
//
// **The distinction is the point of this file.** A setting belongs to the
// user: it lives in `ui_prefs.json`, it changes from a screen, and the app
// must stay consistent in both positions. A switch of this file belongs to
// the project: it decides whether a feature **exists** in a shipped build, it
// changes by editing its line and rebuilding, and nobody but us sees it.
//
// Hence the rule: a switch at `false` leaves **no trace on screen** — no
// empty tab, no greyed-out box, no question asked in the setup wizard. A
// disabled feature that leaves its button behind is not disabled, it is
// broken.
//
// Only for what is **finished but not ready to be seen**. This file is
// neither a drafts folder nor a substitute for a branch: dead code behind a
// switch nobody turns back on rots exactly as anywhere else, only less
// visibly.
//
// **Work still being built** is the other category, and it does not live
// here: a long-running work in progress (Story Mode, `$lib/story/access.ts`)
// grows on `main` behind its own switch, and pays for it with an isolated
// domain that nothing else depends on (`CLAUDE.md`, « Chantiers longs
// derrière un drapeau », held by the `story-isolation` convention).

/**
 * **Vignettes régénérées de la grille** (GRILLE§5 à §8, §11).
 *
 * Éteint le temps que les valeurs par défaut des trois presets embarqués soient
 * arrêtées. Le mécanisme est complet et fusionné dans `main` ; ce qui manque
 * est un travail de réglage à l'œil, long et fastidieux, qui ne peut pas se
 * faire à moitié : trois presets livrés avec des valeurs provisoires seraient
 * jugés sur elles.
 *
 * **Ce que `false` éteint**, et ce sont les trois seuls endroits qui exposaient
 * la fonctionnalité :
 *
 *  - l'onglet `Réglages › Vignettes` (`Settings.svelte`) ;
 *  - la deuxième page de l'assistant de première configuration, celle des trois
 *    profils (`SetupWizard.svelte`) — une nouvelle installation repart donc
 *    directement sur les défauts de l'ancien profil « Normal », qui était de
 *    toute façon le présélectionné ;
 *  - la tâche de fond de génération (`ShellToasts.svelte`).
 *
 * **Et surtout : `gridThumbsOn()` rend `false`**, ce qui suffit à ce qu'aucune
 * vignette ne soit demandée, produite ni affichée. Les cartes reprennent la
 * `preview.png` du mod, exactement comme avant le chantier — y compris chez
 * quelqu'un qui avait coché l'option, dont le réglage reste écrit sur disque et
 * redeviendra effectif au rallumage.
 *
 * **Rien n'est effacé en s'éteignant.** Les images déjà produites restent dans
 * `app_cache_dir/gridthumbs/`, les presets de l'utilisateur dans
 * `ui_prefs.json` : rallumer retrouve tout en l'état. C'est voulu — un
 * interrupteur qui détruit en passant à `false` n'est plus un interrupteur.
 */
export const FEATURE_GRID_THUMBS = false;

/**
 * **Export and import of the library** (EXPORT§).
 *
 * On in development (`npm run tauri dev`), off in a built installer — the
 * one switch here that is not a constant: the feature is complete, and
 * must be tried on real libraries before anyone else sees it. Once it is,
 * this becomes `true`.
 *
 * **What `false` turns off**, the only two places that exposed it:
 *
 *  - the « Transférer la bibliothèque » section of Fichiers › Maintenance
 *    (`Maintenance.svelte`);
 *  - the import step of the setup wizard (`SetupWizard.svelte`).
 *
 * The backend commands stay registered: nothing calls them without a screen.
 */
export const FEATURE_LIBRARY_TRANSFER: boolean = import.meta.env.DEV;
