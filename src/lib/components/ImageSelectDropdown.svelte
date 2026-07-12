<script lang="ts">
  // Liste déroulante compacte à choix unique, chaque entrée avec une
  // miniature (skin voiture, layout circuit…) — un <select> natif ne peut
  // pas afficher d'image par option, d'où ce composant maison.
  interface ImageOption {
    id: string;
    name: string;
    image: string | null;
  }

  interface Props {
    options: ImageOption[];
    selectedId: string | null;
    placeholder: string;
    emptyText: string;
    onselect: (id: string) => void;
    /** "contain" pour un tracé (forme complète, pas de recadrage) — défaut "cover" pour une photo/skin. */
    fit?: "cover" | "contain";
  }
  let { options, selectedId, placeholder, emptyText, onselect, fit = "cover" }: Props = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | undefined>(undefined);

  const selected = $derived(options.find((o) => o.id === selectedId) ?? null);

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (options.length) open = !open;
  }

  function pick(o: ImageOption) {
    open = false;
    onselect(o.id);
  }

  function onDocClick(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) open = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }

  $effect(() => {
    if (!open) return;
    document.addEventListener("click", onDocClick);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      document.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="isd" bind:this={root}>
  <button class="isd-trigger" type="button" onclick={toggle} disabled={options.length === 0} title={placeholder}>
    <span class="isd-thumb" class:contain={fit === "contain"}>
      {#if selected?.image}<img src={selected.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
    </span>
    <span class="isd-name" class:muted={!selected}>{selected?.name ?? (options.length ? placeholder : emptyText)}</span>
    <span class="isd-caret">▾</span>
  </button>
  {#if open}
    <ul class="isd-list">
      {#each options as o (o.id)}
        <li>
          <button type="button" class:on={o.id === selectedId} onclick={() => pick(o)}>
            <span class="isd-thumb" class:contain={fit === "contain"}>
              {#if o.image}<img src={o.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
            </span>
            <span class="isd-name">{o.name}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .isd {
    position: relative;
  }
  .isd-trigger {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    padding: 5px 8px;
    font-size: 11px;
    text-align: left;
  }
  .isd-trigger:hover:not(:disabled) {
    border-color: var(--rosso-border);
  }
  .isd-trigger:disabled {
    opacity: 0.5;
  }
  .isd-thumb {
    flex: none;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--raised);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .isd-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .isd-thumb.contain img {
    object-fit: contain;
    padding: 2px;
  }
  .isd-noimg {
    width: 100%;
    height: 100%;
    background: var(--raised);
  }
  .isd-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt);
  }
  .isd-name.muted {
    color: var(--muted);
  }
  .isd-caret {
    flex: none;
    color: var(--faint);
    font-size: 9px;
  }
  .isd-list {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    z-index: 50;
    list-style: none;
    max-height: 260px;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.4);
  }
  .isd-list li + li {
    border-top: 1px solid var(--line);
  }
  .isd-list button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    background: transparent;
    border: none;
    color: var(--txt2);
    padding: 6px 8px;
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }
  .isd-list button:hover {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .isd-list button.on {
    color: var(--rosso-bright);
    background: var(--rosso-dim);
  }
</style>
