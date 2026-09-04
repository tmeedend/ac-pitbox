<script lang="ts">
  // Menu contextuel générique (clic droit) — positionné au curseur, se ferme
  // au clic ailleurs, à Echap, ou à un autre clic droit.
  import { untrack } from "svelte";
  import { zoomFactor } from "$lib/zoom.svelte";

  interface MenuItem {
    label: string;
    onclick: () => void;
    disabled?: boolean;
    danger?: boolean;
  }

  interface Props {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  }
  let { x, y, items, onclose }: Props = $props();

  let root = $state<HTMLUListElement | undefined>(undefined);
  // Recale dans la fenêtre si le menu déborderait (mesuré après montage).
  // Le composant est recréé à chaque ouverture (voir les appelants, tous sous
  // `{#if ...}`) : ne capturer que la position initiale de x/y est voulu, le
  // `$effect` ci-dessous la corrige ensuite — `untrack` documente cette
  // intention pour le compilateur.
  let pos = $state(untrack(() => ({ left: x, top: y })));

  $effect(() => {
    if (!root) return;
    // Mesures en pixels réels, `left`/`top` en pixels CSS : voir `zoomFactor`,
    // qui porte l'explication pour toute l'app.
    const factor = zoomFactor();
    const r = root.getBoundingClientRect();
    const visualLeft = Math.max(4, Math.min(x, window.innerWidth - r.width - 8));
    const visualTop = Math.max(4, Math.min(y, window.innerHeight - r.height - 8));
    pos = { left: visualLeft / factor, top: visualTop / factor };
  });

  function pick(item: MenuItem) {
    if (item.disabled) return;
    onclose();
    item.onclick();
  }

  // Se ferme au clic gauche ailleurs ou à Echap — PAS à un autre clic droit :
  // celui qui vient d'ouvrir ce menu est le même événement qui bulle jusqu'à
  // `document` juste après le montage, un listener contextmenu ici le
  // fermerait instantanément (bug réel observé). Un nouveau clic droit sur
  // une autre cible rouvre de toute façon le menu ailleurs via son propre
  // gestionnaire (écrase directement l'état côté appelant).
  function onDocClick(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) onclose();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }

  $effect(() => {
    document.addEventListener("click", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<ul class="ctx" bind:this={root} data-gp-overlay style="left:{pos.left}px; top:{pos.top}px;">
  {#each items as it}
    <li>
      <button type="button" class:danger={it.danger} disabled={it.disabled} onclick={() => pick(it)}>
        {it.label}
      </button>
    </li>
  {/each}
</ul>

<style>
  .ctx {
    position: fixed;
    z-index: 200;
    list-style: none;
    min-width: 190px;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.5);
    padding: 4px 0;
  }
  .ctx li + li {
    border-top: 1px solid var(--line);
  }
  .ctx button {
    width: 100%;
    display: block;
    background: transparent;
    border: none;
    color: var(--txt2);
    padding: 8px 12px;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .ctx button:hover:not(:disabled) {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .ctx button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .ctx button.danger:hover:not(:disabled) {
    color: var(--rosso-bright);
  }
</style>
