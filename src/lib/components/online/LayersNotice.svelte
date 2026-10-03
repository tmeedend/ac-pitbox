<script lang="ts">
  // The layers a join would carry (SPEC-play-online.md, "Couches et
  // versions"), said before the button rather than discovered as a kick.
  //
  // Certain — the layer replaces what the server checks: one line says it will
  // be deactivated for the session and given back after, with the choice to
  // keep it anyway. Possible — it touches something else: a yellow box names
  // it, with the choice to join without it.
  import { t } from "$lib/i18n/index.svelte";
  import type { LayerConflict } from "$lib/online/online";

  interface Props {
    conflicts: LayerConflict[];
    keepCertain: boolean;
    dropPossible: boolean;
  }
  let { conflicts, keepCertain = $bindable(), dropPossible = $bindable() }: Props = $props();

  const certain = $derived(conflicts.filter((c) => c.risk === "certain"));
  const possible = $derived(conflicts.filter((c) => c.risk === "possible"));
  const names = (list: LayerConflict[]) => list.map((c) => c.name).join(", ");
</script>

{#if certain.length}
  <div class="notice">
    {#if !keepCertain}
      <p>{t("online.layersSetAside", { names: names(certain) })}</p>
    {/if}
    <label class="tog">
      <input type="checkbox" bind:checked={keepCertain} />
      {t("online.layersKeep")}
    </label>
  </div>
{/if}
{#if possible.length}
  <div class="warnbox">
    <p>{t("online.layersPossible", { names: names(possible) })}</p>
    <label class="tog">
      <input type="checkbox" bind:checked={dropPossible} />
      {t("online.layersDrop")}
    </label>
  </div>
{/if}

<style>
  .notice p,
  .warnbox p {
    font-size: 12px;
    line-height: 1.45;
  }
  .notice p {
    color: var(--txt2);
  }
  .tog {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    font-size: 11.5px;
    color: var(--muted);
    cursor: pointer;
  }
  .tog input {
    accent-color: var(--rosso);
  }
</style>
