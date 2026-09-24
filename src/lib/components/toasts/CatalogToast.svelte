<script lang="ts">
  // "The rules catalogue was updated" in the notification stack (REGLES§6.2).
  //
  // Not a timed toast: it stays until closed, like the import report - a user
  // who comes back to the screen later must still find out what changed. It
  // gives the effect (mods reclassified) and the way out (go back); the detail
  // lives in the Workshop, where the rules are.
  import { onMount } from "svelte";
  import Toast from "./Toast.svelte";
  import {
    catalogReport,
    dismissCatalogReport,
    loadCatalogReport,
    sameVersion,
    setCatalogReverted,
  } from "$lib/workshop/catalogReport.svelte";
  import { requestSection } from "$lib/shell/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";

  onMount(() => {
    void loadCatalogReport();
  });

  const view = $derived(catalogReport.view);
  const report = $derived(view?.report ?? null);
</script>

{#if report && view}
  {@const c = report.changes}
  <Toast
    tone="info"
    title={sameVersion(report)
      ? t("catalogReport.titleSameVersion")
      : t("catalogReport.title", { from: report.from_version, to: report.to_version })}
    onclose={() => void dismissCatalogReport()}
  >
    <div class="line">
      {t("catalogReport.counts", { added: c.added.length, corrected: c.corrected.length, retired: c.retired.length })}
    </div>
    <div class="line strong">{t("catalogReport.reclassified", { count: report.reclassified.length })}</div>
    {#if catalogReport.error}<div class="err">{catalogReport.error}</div>{/if}
    <div class="acts">
      <button type="button" class="btn" onclick={() => void requestSection("rules")}>{t("catalogReport.details")}</button>
      {#if view.can_revert && !view.reverted}
        <button type="button" class="btn" disabled={catalogReport.busy} onclick={() => void setCatalogReverted(true)}>
          {catalogReport.busy ? t("catalogReport.applying") : t("catalogReport.revert", { version: report.from_version })}
        </button>
      {/if}
    </div>
  </Toast>
{/if}

<style>
  .line {
    font-size: 12px;
    color: var(--txt2);
  }
  .strong {
    color: var(--txt);
    margin-top: 2px;
  }
  .err {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--rosso-bright);
  }
  .acts {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
</style>
