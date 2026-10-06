// Everything the shell starts once and keeps running for the whole life of the
// window: global listeners, background checks, progress watchers. They used to
// be a row of `onMount` calls in `AppShell.svelte`, which grew by one each time
// a feature needed to outlive the screen that launched it — a batch started
// from the library, an import dropped on any screen, a mod update found in the
// background. None of them touches the shell's own state, so they live here and
// `AppShell` starts them with a single call.
import { initGlobalDragDrop } from "$lib/workshop/importState.svelte";
import { watchShellScroll } from "$lib/shell/shellScroll";
import { initBulkProgress } from "$lib/library/bulkState.svelte";
import { initRepairProgress } from "$lib/workshop/repairState.svelte";
import { startModUpdateChecks } from "$lib/library/modUpdates.svelte";
import { loadGridCars } from "$lib/launch/gridMods.svelte";
import { loadPlayerHandicap } from "$lib/launch/playerHandicap.svelte";
import { PAUSE_SESSION, pauseGridThumbs, resumeGridThumbs } from "$lib/gridthumbs/gridThumbs.svelte";
import { onAcRunning } from "$lib/launch/launch";
import { startGameSessionWatch } from "$lib/launch/gameSession.svelte";
import { startGamepadNav } from "$lib/shell/gamepadNav";
import { startControllerWatch } from "$lib/shell/gamepadDevices.svelte";
import { goBack, goForward } from "$lib/shell/navHistory";
import { bigPictureState, exitBigPicture } from "$lib/shell/bigpicture.svelte";
import { listen } from "@tauri-apps/api/event";
import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
import { loadTechSheetReport } from "$lib/detail/techSheetReport.svelte";
import { startOnlineWatch } from "$lib/online/watch.svelte";

/** Starts every shell-wide service; the returned function stops them all. */
export function startShellServices(): () => void {
  const stops: Array<() => void> = [];

  // Glisser-déposer disponible partout : un seul listener, monté ici à la
  // racine, plutôt que dans chaque écran susceptible de recevoir un drop.
  stops.push(initGlobalDragDrop());

  // La coquille ne défile jamais (§13) : le filet qui l'y ramène est monté
  // avec elle parce que c'est elle qu'il protège — voir `shellScroll.ts` pour
  // le pourquoi, et pour les deux chemins par lesquels le décalage est arrivé.
  stops.push(watchShellScroll());

  // Progression des actions groupées (§6.3bis) : un seul écouteur, monté ici
  // comme le glisser-déposer — un lot lancé depuis la bibliothèque doit rester
  // visible même si on change d'écran pendant.
  stops.push(initBulkProgress());
  stops.push(initRepairProgress());
  // Mises à jour de mods (§4.7) : vérification une minute après le démarrage
  // puis une fois par jour, et suivi du téléchargement en cours — monté ici
  // pour la même raison que les lots.
  stops.push(startModUpdateChecks());
  // Plateau du dernier réglage de session : la garde d'activation doit savoir
  // ce qu'elle protège même quand l'écran de réglages n'est pas monté.
  void loadGridCars();
  void loadPlayerHandicap();

  stops.push(pauseGridThumbsWhileRacing());
  // Whether a session runs (SESSION§2.4): the launch button of the session
  // column reads it on every screen, so it is followed from the start.
  stops.push(startGameSessionWatch());
  // Friends connecting, a slot freed (SPEC-play-online.md, v2): watched
  // whatever screen is open, so started with the shell.
  stops.push(startOnlineWatch());
  stops.push(reloadLibraryAfterTechBackfill());

  // Navigation manette dans toute l'app (croix/stick = déplace le focus,
  // A/Croix = valide, B/Rond = ferme la fiche pleine page). Un seul scrutin
  // global, monté une fois ici.
  stops.push(startGamepadNav());

  // Détection des périphériques et décision « lequel pilote l'interface »
  // (§7.4). Démarrage, branchement à chaud et première installation sont le
  // même événement — un périphérique visible sans décision enregistrée — donc
  // une seule surveillance, montée ici comme le scrutin ci-dessus.
  stops.push(startControllerWatch());

  stops.push(suppressNativeContextMenu());
  stops.push(mouseSideButtonsNavigate());
  stops.push(escapeLeavesBigPicture());

  // Language and zoom are not here any more: applied by `+page.svelte` before
  // the shell is mounted (`shell/boot.ts`), they no longer redraw it once.

  return () => {
    for (const stop of stops) stop();
  };
}

/**
 * **La fin de la session lève la pause de la génération.**
 *
 * Premier essai : le retour du focus dans la fenêtre, au motif qu'on ne
 * saurait pas voir la fin d'une course. On sait très bien — l'app surveille
 * déjà le process d'Assetto Corsa depuis le début, pour couper et reprendre
 * la musique de Big Picture. Le focus était donc à la fois moins juste (une
 * fenêtre reprise en alt-tab pendant une course aurait relancé les trois
 * cents conversions) et redondant.
 *
 * Le process, et non le statut « en piste » : ce dernier retombe à chaque
 * retour aux stands. C'est la fermeture du jeu qui rend la machine.
 */
function pauseGridThumbsWhileRacing(): () => void {
  let stop: (() => void) | null = null;
  void onAcRunning((running) => {
    if (running) pauseGridThumbs(PAUSE_SESSION);
    else resumeGridThumbs(PAUSE_SESSION);
  }).then((off) => (stop = off));
  return () => stop?.();
}

/**
 * The first start of a version with the tech sheet reads every car's physics
 * in the background (FICHE§9.3), and rewrites the library's spec columns as it
 * goes: the library, loaded before it ends, reloads once it has, and the
 * notification says what moved.
 */
function reloadLibraryAfterTechBackfill(): () => void {
  const unlisten = listen<number>("techsheet://filled", () => {
    bumpLibraryVersion();
    void loadTechSheetReport();
  });
  return () => {
    void unlisten.then((f) => f());
  };
}

/**
 * Supprime le menu contextuel natif du navigateur (Actualiser/Enregistrer
 * sous/Imprimer…) partout dans l'app — une appli desktop n'en a pas besoin,
 * et il apparaîtrait sinon là où aucun menu contextuel maison n'est posé
 * (celui-ci, lui, s'affiche AVANT — donc gagne toujours). Un seul listener
 * global plutôt qu'un `preventDefault` à poser sur chaque écran.
 */
function suppressNativeContextMenu(): () => void {
  const suppress = (e: MouseEvent) => e.preventDefault();
  document.addEventListener("contextmenu", suppress);
  return () => document.removeEventListener("contextmenu", suppress);
}

/**
 * Boutons latéraux de la souris = précédent/suivant, comme dans un
 * navigateur. `preventDefault` sur le `mousedown` ET sur l'`auxclick` :
 * WebView2 mappe ces deux boutons sur SON historique de navigation, et une
 * app à route unique (adapter-static, SPA) n'a rien où reculer — au mieux il
 * ne se passe rien, au pire la webview quitte la page et l'app se retrouve
 * devant une fenêtre blanche.
 */
function mouseSideButtonsNavigate(): () => void {
  const onMouseDown = (e: MouseEvent) => {
    // 3 et 4 = les deux boutons latéraux (« précédent » et « suivant » chez
    // Chromium), pas les trois boutons principaux.
    if (e.button !== 3 && e.button !== 4) return;
    e.preventDefault();
    if (e.button === 3) void goBack();
    else void goForward();
  };
  const swallow = (e: MouseEvent) => {
    if (e.button === 3 || e.button === 4) e.preventDefault();
  };
  window.addEventListener("mousedown", onMouseDown);
  window.addEventListener("auxclick", swallow);
  return () => {
    window.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("auxclick", swallow);
  };
}

/** Sortie du mode Big Picture au clavier — pas d'autre chrome de fenêtre
 * visible une fois en plein écran pour cliquer un bouton "retour" évident. */
function escapeLeavesBigPicture(): () => void {
  const onKeydown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && bigPictureState.active) exitBigPicture();
  };
  document.addEventListener("keydown", onKeydown);
  return () => document.removeEventListener("keydown", onKeydown);
}
