// Niveau de zoom de l'interface (en %). Utile sur écran haute résolution si
// la mise à l'échelle Windows n'est pas reprise par la webview : applique un
// `zoom` CSS sur le document, équivalent à un zoom navigateur.
export const ZOOM_LEVELS = [90, 100, 110, 125, 150, 175, 200];

export const zoomState = $state<{ level: number }>({ level: 100 });

/** Applique un niveau de zoom (ou repasse à 100% si `null`). */
export function setZoom(level: number | null): void {
  const lvl = level && ZOOM_LEVELS.includes(level) ? level : 100;
  zoomState.level = lvl;
  if (typeof document !== "undefined") {
    document.documentElement.style.zoom = `${lvl}%`;
  }
}
