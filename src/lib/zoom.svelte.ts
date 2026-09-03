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

/**
 * Facteur du zoom d'interface, et **le seul endroit qui explique pourquoi il
 * faut y penser**.
 *
 * Le zoom est un `zoom` CSS posé sur `<html>`. Tout ce qu'on **mesure** —
 * `getBoundingClientRect`, `clientX/clientY`, `innerWidth/innerHeight` — est
 * en pixels réels de la fenêtre, donc déjà multiplié par ce facteur. Tout ce
 * qu'on **écrit** dans un `style` d'un descendant de `<html>` est en pixels
 * CSS, que le zoom multipliera à son tour. Reporter une mesure dans un
 * `left`/`top` sans repasser par ici applique donc le zoom deux fois, et le
 * décalage grandit avec la distance au coin haut-gauche : à 110 %, une liste
 * ouverte en bas de fenêtre sort de l'écran (bug réel, signalé deux fois — le
 * menu contextuel d'abord, la liste déroulante des skins et des tenues
 * ensuite).
 *
 * Règle : `mesure / zoomFactor()` avant d'écrire, jamais l'inverse.
 */
export function zoomFactor(): number {
  return zoomState.level / 100;
}
