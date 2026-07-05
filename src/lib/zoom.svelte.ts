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
    // Le CSS `zoom` agrandit tout le rendu, mais les unités `vh`/`vw` restent
    // relatives à la fenêtre réelle (pas au rendu zoomé) : une coquille en
    // `height: 100vh` devient donc physiquement plus haute que la fenêtre à
    // >100% (rien à scroller pour la voir en entier, ex. bouton Enregistrer
    // hors champ). Cette variable permet de diviser les tailles en vh par le
    // facteur de zoom (cf. AppShell.svelte .frame) pour qu'elles retrouvent
    // leur taille réelle une fois zoomées.
    document.documentElement.style.setProperty("--ui-zoom", String(lvl / 100));
  }
}
