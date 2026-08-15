<script lang="ts">
  // Liste déroulante à choix multiple pour les skins de circuit (§8) —
  // plusieurs actifs à la fois, comme le fait Content Manager lui-même dans
  // son propre sélecteur. Reste ouverte tant qu'on coche/décoche.
  import { t } from "$lib/i18n/index.svelte";

  interface TrackSkinOption {
    name: string;
    image: string | null;
    active: boolean;
  }

  interface Props {
    options: TrackSkinOption[];
    busy?: boolean;
    ontoggle: (name: string, active: boolean) => void;
  }
  let { options, busy = false, ontoggle }: Props = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | undefined>(undefined);

  const activeCount = $derived(options.filter((o) => o.active).length);
  const label = $derived.by(() => {
    if (activeCount === 0) return t("session.trackSkinsNone");
    if (activeCount === 1) return options.find((o) => o.active)!.name;
    return t("session.trackSkinsCount", { count: activeCount });
  });

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (options.length) open = !open;
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
  <button class="isd-trigger" type="button" onclick={toggle} disabled={options.length === 0} title={t("session.trackSkinsTooltip")}>
    <span class="isd-name" class:muted={activeCount === 0}>{options.length ? label : t("session.trackSkinsEmpty")}</span>
    <span class="isd-caret">▾</span>
  </button>
  {#if open}
    <ul class="isd-list">
      {#each options as o (o.name)}
        <li>
          <label class:on={o.active}>
            <input
              type="checkbox"
              checked={o.active}
              disabled={busy}
              onchange={() => ontoggle(o.name, !o.active)}
            />
            <span class="isd-thumb">
              {#if o.image}<img src={o.image} alt="" />{:else}<span class="isd-noimg"></span>{/if}
            </span>
            <span class="isd-name">{o.name}</span>
          </label>
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
  .isd-list label {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    font-size: 11px;
    color: var(--txt2);
    cursor: pointer;
  }
  .isd-list label:hover {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .isd-list label.on {
    color: var(--rosso-bright);
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
  .isd-noimg {
    width: 100%;
    height: 100%;
    background: var(--raised);
  }
</style>
