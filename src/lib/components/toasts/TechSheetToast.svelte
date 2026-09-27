<script lang="ts">
  // "The cars' tech sheets were read" in the notification stack (FICHE§9.3).
  //
  // The first start of a version that reads the physics — or that reads it
  // better — rewrites the library's spec columns under the user: "turbo" on a
  // hundred cars, a drivetrain changed on a few. Said here, in numbers, per
  // column; not a timed toast, it stays until closed, like the catalogue's.
  import { onMount } from "svelte";
  import Toast from "./Toast.svelte";
  import { dismissTechSheetReport, loadTechSheetReport, techSheetReport } from "$lib/detail/techSheetReport.svelte";
  import { COLUMN_LABEL, reportLines } from "$lib/detail/techSheet";
  import { t } from "$lib/i18n/index.svelte";

  onMount(() => {
    void loadTechSheetReport();
  });

  const report = $derived(techSheetReport.report);
</script>

{#if report}
  <Toast tone="info" title={t("techsheet.report.title")} onclose={() => void dismissTechSheetReport()}>
    <div class="line strong">{t("techsheet.report.cars", { count: report.cars })}</div>
    {#each reportLines(report) as l (l.field)}
      <div class="line">
        <span class="col">{t(COLUMN_LABEL[l.field])}</span>
        {l.counts.map(([k, n]) => (n === 1 ? t(`techsheet.report.${k}One`) : t(`techsheet.report.${k}`, { count: n }))).join(" · ")}
      </div>
    {/each}
  </Toast>
{/if}

<style>
  .line {
    font-size: 12px;
    color: var(--txt2);
  }
  .strong {
    color: var(--txt);
    margin-bottom: 4px;
  }
  .col {
    color: var(--muted);
    margin-right: 6px;
  }
</style>
