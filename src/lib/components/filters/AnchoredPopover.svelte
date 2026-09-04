<script lang="ts">
  // Popover accroché à un élément de la barre de filtres (§6.3) : le menu
  // d'ajout sous « + Filtre », l'éditeur sous sa puce.
  //
  // C'est ce qui permet à la barre de garder une hauteur fixe quelle que soit
  // la complexité d'un filtre (R1 de la spec) — donc il y en aura d'autres, et
  // le placement vit ici une fois pour toutes plutôt que dans chaque contenu.
  import { zoomFactor } from "$lib/zoom.svelte";
  import type { Snippet } from "svelte";

  interface Props {
    /** Élément sous lequel s'ouvrir. */
    anchor: HTMLElement;
    onclose: () => void;
    minWidth?: number;
    children: Snippet;
  }
  let { anchor, onclose, minWidth = 264, children }: Props = $props();

  let root = $state<HTMLDivElement | undefined>(undefined);
  let pos = $state<{ left: number; top: number } | null>(null);

  const GAP = 6;
  const EDGE = 8;

  /** Mesures en pixels **réels** (`getBoundingClientRect`, `innerWidth`),
   * `left`/`top` écrits en pixels **CSS** : sans la division par le facteur de
   * zoom, celui-ci s'appliquerait une seconde fois et le popover s'ouvrirait
   * d'autant plus bas qu'il est loin du coin haut-gauche. Trois fois le même
   * bug dans cette app (menu contextuel, listes déroulantes, poignées de
   * colonnes) — l'explication complète est dans `zoom.svelte.ts`. */
  function place() {
    if (!root || !anchor) return;
    const factor = zoomFactor();
    const a = anchor.getBoundingClientRect();
    const r = root.getBoundingClientRect();
    let left = Math.min(a.left, window.innerWidth - r.width - EDGE);
    left = Math.max(EDGE, left);
    // Bascule au-dessus de l'ancre quand il n'y a plus la place dessous — un
    // éditeur ouvert depuis une puce en bas de fenêtre sortait sinon de
    // l'écran, et il n'y a pas de défilement de page pour aller le chercher.
    let top = a.bottom + GAP;
    if (top + r.height > window.innerHeight - EDGE) {
      const above = a.top - GAP - r.height;
      top = above >= EDGE ? above : Math.max(EDGE, window.innerHeight - r.height - EDGE);
    }
    pos = { left: left / factor, top: top / factor };
  }

  // Le contenu change de taille sous la frappe (la liste de suggestions se
  // réduit à chaque lettre) : un placement mesuré une seule fois au montage
  // laisserait le popover déborder dès qu'il grandit.
  $effect(() => {
    if (!root) return;
    place();
    const ro = new ResizeObserver(place);
    ro.observe(root);
    window.addEventListener("resize", place);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", place);
    };
  });

  // Le clic qui vient d'ouvrir ce popover ne l'atteint pas : il a fini de
  // buller vers `document` avant que Svelte n'ait monté le composant et posé
  // cet écouteur. Un clic sur l'ancre elle-même est ignoré ici — c'est son
  // propre gestionnaire qui bascule l'ouverture, sinon fermer puis rouvrir
  // ferait clignoter le popover sans jamais le refermer.
  function onDocPointer(e: MouseEvent) {
    const target = e.target as Node;
    if (root?.contains(target) || anchor?.contains(target)) return;
    onclose();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }

  $effect(() => {
    document.addEventListener("mousedown", onDocPointer);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDocPointer);
      document.removeEventListener("keydown", onKey, true);
    };
  });
</script>

<!-- `visibility` plutôt qu'un `{#if pos}` : le popover doit être dans le DOM et
     mesurable pour que `place()` sache où le mettre, mais ne doit pas être vu
     une frame en haut à gauche avant d'être placé. -->
<div
  class="pop"
  bind:this={root}
  data-gp-overlay
  style="left:{pos?.left ?? 0}px; top:{pos?.top ?? 0}px; min-width:{minWidth}px; visibility:{pos ? 'visible' : 'hidden'};"
>
  {@render children()}
</div>

<style>
  .pop {
    position: fixed;
    z-index: 220;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 12px 34px rgba(0, 0, 0, 0.62);
    padding: 9px;
    max-width: 92vw;
  }
</style>
