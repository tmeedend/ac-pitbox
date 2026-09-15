// **La coquille ne défile jamais, et ça se garantit plutôt que ça s'espère.**
//
// `global.css` met `html` et `body` en `overflow: hidden` exprès : le document
// lui-même ne défile pas, chaque écran gère son propre défilement interne. Un
// défilement de page entraînerait toute la coquille — barre de titre comprise —
// hors champ.
//
// **Mais `overflow: hidden` n'interdit que la molette.** Le navigateur, lui,
// fait défiler ces conteneurs tout seul : pour amener dans la fenêtre un
// élément qui vient de prendre le focus, ou sur un `scrollIntoView` posé
// n'importe où dans l'app — qui fait défiler *tous* les ancêtres scrollables
// jusqu'à la fenêtre, le document inclus. Et comme la molette ne peut plus
// revenir dessus, le décalage est **définitif** : une bande noire sous la
// fenêtre, la barre de titre à moitié sortie, et aucun geste utilisateur pour
// rattraper — seul un redémarrage efface.
//
// Le symptôme a été vu à l'usage, et il est arrivé par au moins deux chemins
// différents (le sommaire de l'onglet Wikipédia, le défilement vers la carte
// sélectionnée de la bibliothèque). D'où un remède en deux temps, et non une
// rustine par appelant :
//
//  - `pinShell()` remet la coquille d'aplomb, à appeler juste après un geste
//    dont on sait qu'il peut la déplacer ;
//  - `watchShellScroll()` l'installe une fois pour toutes en écoutant le
//    défilement du document — un filet, pour les chemins qu'on n'a pas vus
//    venir. Un `scrollTop` non nul sur un document en `overflow: hidden` est
//    par définition un accident : il n'y a rien à préserver.

import { zoomFactor } from "./zoom.svelte";

/** Remet le document et ses ancêtres non défilables à zéro. Sans effet — et
 * sans coût — quand tout est déjà d'aplomb. `from` permet de remonter la
 * chaîne depuis un élément précis ; sans lui, seuls la racine et le corps sont
 * vérifiés. */
export function pinShell(from?: HTMLElement | null): void {
  const root = document.documentElement;
  if (root.scrollTop) root.scrollTop = 0;
  if (root.scrollLeft) root.scrollLeft = 0;
  if (document.body.scrollTop) document.body.scrollTop = 0;
  if (document.body.scrollLeft) document.body.scrollLeft = 0;
  let node: HTMLElement | null = from ?? null;
  while (node && node !== document.body) {
    // Seulement ceux qui ne DEVRAIENT pas défiler : un conteneur en `auto`
    // porte le défilement légitime de son écran, on n'y touche pas.
    if (getComputedStyle(node).overflowY === "hidden" && node.scrollTop) node.scrollTop = 0;
    node = node.parentElement;
  }
}

/**
 * Filet global, monté une fois par `AppShell`.
 *
 * Écoute le défilement du **document** — celui qui ne devrait jamais avoir
 * lieu. L'écouteur est en capture pour voir passer aussi les événements des
 * conteneurs internes (`scroll` ne remonte pas), et il sort tout de suite
 * quand la cible n'est pas le document : le défilement d'un écran est le cas
 * normal et ne doit rien coûter.
 *
 * Renvoie la fonction de retrait, pour l'`onMount` qui l'installe.
 */
export function watchShellScroll(): () => void {
  const onScroll = (e: Event) => {
    if (e.target !== document && e.target !== document.documentElement && e.target !== document.body) return;
    pinShell();
  };
  window.addEventListener("scroll", onScroll, true);
  return () => window.removeEventListener("scroll", onScroll, true);
}

/**
 * Le plus proche ancêtre qui défile réellement — et **jamais au-delà de
 * `<body>`**.
 *
 * C'est la deuxième façon de déplacer la coquille, et elle est sournoise :
 * l'`overflow-y` calculé de l'élément racine vaut « auto », pas « visible »,
 * donc un chercheur d'ancêtre naïf le retient comme conteneur défilable et
 * écrit dedans. La boucle s'arrête donc **avant** `document.body`, et exige en
 * plus que le conteneur ait vraiment de quoi défiler : un `auto` sur un
 * contenu qui tient n'est pas le bon candidat, on veut celui du dessus.
 */
function nearestScroller(el: HTMLElement, axis: "y" | "x"): HTMLElement | null {
  let node = el.parentElement;
  while (node && node !== document.body) {
    const style = getComputedStyle(node);
    const flow = axis === "y" ? style.overflowY : style.overflowX;
    const scrollable =
      axis === "y" ? node.scrollHeight > node.clientHeight : node.scrollWidth > node.clientWidth;
    if ((flow === "auto" || flow === "scroll") && scrollable) return node;
    node = node.parentElement;
  }
  return null;
}

/**
 * Amène `el` dans le champ **en ne faisant défiler que son conteneur**.
 *
 * Le remplaçant de `scrollIntoView`, qui est interdit ici : lui fait défiler
 * *tous* les ancêtres scrollables jusqu'à la fenêtre, document compris, et un
 * décalage posé sur un document en `overflow: hidden` est définitif — la
 * molette ne peut plus le rattraper, seul un redémarrage efface.
 *
 * `mode` suit la sémantique de l'API native : `"nearest"` ne bouge que si
 * l'élément dépasse (le bon choix pour un déplacement au clavier, qui ne doit
 * pas sauter), `"center"` le ramène au milieu (le bon choix à l'ouverture
 * d'une liste positionnée sur un élément déjà choisi).
 *
 * La division par `zoomFactor()` n'est pas décorative : le zoom d'interface
 * est un `zoom` CSS posé sur `<html>`, donc `getBoundingClientRect` rend des
 * pixels de fenêtre déjà multipliés, quand `scrollTop` et `clientHeight` sont
 * en pixels CSS. Reporter l'un dans l'autre sans diviser applique le facteur
 * deux fois — invisible à 100 %, donc invisible en développement.
 */
export function scrollIntoContainer(el: HTMLElement, mode: "nearest" | "center" = "nearest"): void {
  const f = zoomFactor();
  const rect = el.getBoundingClientRect();

  for (const axis of ["y", "x"] as const) {
    const scroller = nearestScroller(el, axis);
    if (!scroller) continue;
    const box = scroller.getBoundingClientRect();
    const start = axis === "y" ? (rect.top - box.top) / f : (rect.left - box.left) / f;
    const size = axis === "y" ? rect.height / f : rect.width / f;
    const view = axis === "y" ? scroller.clientHeight : scroller.clientWidth;

    let delta = 0;
    if (mode === "center") delta = start - (view / 2 - size / 2);
    else if (start < 0) delta = start;
    else if (start + size > view) delta = start + size - view;
    if (!delta) continue;

    if (axis === "y") scroller.scrollTop += delta;
    else scroller.scrollLeft += delta;
  }

  // Le défilement dû au focus, lui, a pu avoir lieu avant l'appel : on remet
  // la coquille d'aplomb derrière.
  pinShell(el);
}
