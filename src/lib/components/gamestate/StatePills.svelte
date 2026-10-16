<script lang="ts">
  // The pills of a line (DOSSIER§4.2, DOSSIER§4.4): a file says its state, a
  // folder the counts of its two gravest states. Red replaces the game, yellow
  // is an alert (drift), blue an information (CM zone), green acquired, and
  // orange - the app's neutral-negative, an inactive mod's - a copy that waits:
  // nothing is wrong, it is just not laid. Waiting used to share the yellow of
  // drift, and the two read as one at a glance (reported 2026-10).
  // No new colour: every one is a token of `global.css`.
  import { t } from "$lib/i18n/index.svelte";
  import { folderPills } from "$lib/gamestate/display";
  import type { Row, StateValue } from "$lib/gamestate/gamestate";

  let { row }: { row: Row } = $props();

  const TONE: Record<StateValue, string> = {
    posed: "pill-ok",
    replacesGame: "pill-err",
    waiting: "pill-idle",
    drift: "pill-warn",
    cmZone: "pill-info",
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
      <span class="pill pill-info" title={t("gamefolder.state.cmZone")}>{t("gamefolder.cmShort")}</span>
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
</style>
