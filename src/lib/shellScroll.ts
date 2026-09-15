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
