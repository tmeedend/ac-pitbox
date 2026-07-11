<script lang="ts">
  // Menu contextuel générique (clic droit) — positionné au curseur, se ferme
  // au clic ailleurs, à Echap, ou à un autre clic droit.
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
  let pos = $state({ left: x, top: y });

  $effect(() => {
    if (!root) return;
    const r = root.getBoundingClientRect();
    const left = Math.min(x, window.innerWidth - r.width - 8);
    const top = Math.min(y, window.innerHeight - r.height - 8);
    pos = { left: Math.max(4, left), top: Math.max(4, top) };
  });

  function pick(item: MenuItem) {
    if (item.disabled) return;
    onclose();
    item.onclick();
  }

  function onDocClick(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) onclose();
  }
  function onDocContextMenu(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) onclose();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }

  $effect(() => {
    document.addEventListener("click", onDocClick);
    document.addEventListener("contextmenu", onDocContextMenu);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      document.removeEventListener("contextmenu", onDocContextMenu);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<ul class="ctx" bind:this={root} style="left:{pos.left}px; top:{pos.top}px;">
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
