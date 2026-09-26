<script lang="ts">
  // The pills of a line (DOSSIER§4.2, DOSSIER§4.4): a file says its state, a folder
  // the counts of its two gravest states. Exactly the colours of the sheet's
  // "Game additions" tab - red replaces the game, yellow is an alert, blue an
  // information, green acquired; drift and waiting share the yellow and part by
  // their label and glyph. No new colour.
  import { t } from "$lib/i18n/index.svelte";
  import { folderPills } from "$lib/gamestate/display";
  import type { Row, StateValue } from "$lib/gamestate/gamestate";

  let { row }: { row: Row } = $props();

  const TONE: Record<StateValue, string> = {
    posed: "pill-ok",
    replacesGame: "pill-err",
    waiting: "pill-warn",
    drift: "pill-warn",
    cmZone: "pill-cm",
    nobody: "",
    shared: "",
  };

  const folder = $derived(row.kind === "dir");
  const pills = $derived(folder ? folderPills(row.counts) : []);
</script>

<span class="pills">
  {#if folder}
    {#each pills as p (p.value)}
      <span class="pill {TONE[p.value]}" title={t(`gamefolder.state.${p.value}`)}>
        {#if p.value === "drift"}<span aria-hidden="true">≠</span>{/if}
        {p.count === null ? t(`gamefolder.state.${p.value}`) : p.count.toLocaleString()}
      </span>
    {/each}
  {:else}
    {#if row.drift}
      <span class="pill pill-warn"><span aria-hidden="true">≠</span>{t(`gamefolder.drift.${row.drift}`)}</span>
    {:else if row.state !== "nobody"}
      <span class="pill {TONE[row.state]}">{t(`gamefolder.state.${row.state}`)}</span>
    {/if}
    {#if row.cmZone}
      <span class="pill pill-cm" title={t("gamefolder.state.cmZone")}>{t("gamefolder.cmShort")}</span>
    {/if}
  {/if}
</span>

<style>
  .pills {
    display: inline-flex;
    gap: 4px;
    justify-content: flex-end;
    white-space: nowrap;
  }
  .pill {
    font-size: 10.5px;
    padding: 1px 6px;
    font-variant-numeric: tabular-nums;
  }
  /* The blue of information - the same as the sheet's CM zone mark. */
  .pill-cm {
    color: var(--blue);
    border-color: var(--blue-border);
    background: var(--blue-dim);
  }
</style>
