// Mode Big Picture : fenêtre plein écran + zoom dédié + démarrage/arrêt de
// l'ambiance musicale (docs/spec-module-musique_2.md). L'app ne propose pas
// (encore) d'interface "10-foot" dédiée : c'est l'UI existante, agrandie.
import { getCurrentWindow, currentMonitor, type PhysicalPosition, type PhysicalSize } from "@tauri-apps/api/window";
import { setZoom } from "./zoom.svelte";
import { getConfig } from "./config";
import { musicEnterBigPicture, musicExitBigPicture } from "./music";

export const bigPictureState = $state<{ active: boolean }>({ active: false });

/** Applies the zoom of the mode currently ON SCREEN.
 *
 * Two zoom settings, only one of them showing at any time: Big Picture has its
 * own, and falls back on the ordinary one when it has none. Anything applying a
 * zoom live - the Settings screen previews both of them as they are picked -
 * has to go through here, or changing the Big Picture zoom from inside Big
 * Picture would show nothing, and changing the ordinary zoom from inside it
 * would fight the mode's own. */
export function applyZoomFor(prefs: { ui_zoom: number | null; bigpicture_zoom: number | null }): void {
  setZoom(bigPictureState.active ? (prefs.bigpicture_zoom ?? prefs.ui_zoom) : prefs.ui_zoom);
}

// Restaurés à la sortie — taille/position/zoom en place avant l'entrée en
// Big Picture, pas forcément les valeurs par défaut si l'utilisateur avait
// déjà déplacé/redimensionné la fenêtre ou changé le zoom ailleurs.
let previousZoom: number | null = null;
let previousSize: PhysicalSize | null = null;
let previousPosition: PhysicalPosition | null = null;
// Une fenêtre maximisée ne se restaure PAS en lui rendant sa taille et sa
// position : elle redevient une fenêtre normale qui se trouve avoir la taille
// de l'écran, et le bouton d'agrandissement de Windows s'inverse (bug
// signalé). L'état est donc mémorisé à part, et c'est `maximize()` qui le
// rétablit à la sortie.
let previousMaximized = false;

export async function enterBigPicture(): Promise<void> {
  if (bigPictureState.active) return;
  const cfg = await getConfig();
  const win = getCurrentWindow();
  previousZoom = cfg.prefs.ui_zoom;
  previousMaximized = await win.isMaximized();
  previousSize = await win.outerSize();
  previousPosition = await win.outerPosition();

  await win.setFullscreen(true);
  // Filet de sécurité Windows : sur une fenêtre sans décorations,
  // `setFullscreen` couvre parfois seulement la zone de travail (écran moins
  // la barre des tâches) plutôt que l'écran entier — bug constaté (zone en
  // bas de l'écran, là où était la barre des tâches, qui restait hors de la
  // fenêtre). On force explicitement les bornes du moniteur pour ne jamais
  // laisser de bande non couverte.
  const monitor = await currentMonitor();
  if (monitor) {
    await win.setPosition(monitor.position);
    await win.setSize(monitor.size);
  }

  // Flag first, zoom after: `applyZoomFor` reads the flag to know which of the
  // two settings is the one showing.
  bigPictureState.active = true;
  applyZoomFor(cfg.prefs);
  // Musique après le passage en plein écran/zoom : si `enabled` est faux
  // côté réglages, le moteur ne joue rien (§2, coupe-circuit) — pas de
  // condition à dupliquer côté frontend.
  await musicEnterBigPicture();
}

export async function exitBigPicture(): Promise<void> {
  if (!bigPictureState.active) return;
  const win = getCurrentWindow();
  await win.setFullscreen(false);
  if (previousMaximized) {
    // Maximisée avant d'entrer : la remettre à sa taille d'alors la laisserait
    // « restaurée » à la taille de l'écran, ce qui n'est pas la même chose et
    // se voit tout de suite (bouton d'agrandissement inversé, glisser la barre
    // de titre ne la décolle plus). Pas de `setSize`/`setPosition` ici : ils
    // écraseraient au passage la géométrie que Windows garde pour le retour à
    // l'état restauré.
    await win.maximize();
  } else {
    // Restaure explicitement (voir le filet de sécurité ci-dessus : on a pu
    // redimensionner nous-mêmes la fenêtre au-delà de ce que `setFullscreen`
    // sait annuler tout seul).
    if (previousSize) await win.setSize(previousSize);
    if (previousPosition) await win.setPosition(previousPosition);
  }
  setZoom(previousZoom);
  bigPictureState.active = false;
  await musicExitBigPicture();
}
