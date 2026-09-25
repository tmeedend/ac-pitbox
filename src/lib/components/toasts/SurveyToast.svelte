<script lang="ts">
  // The survey in the notification stack: running (no percentage - the
  // backend reads mods without counting ahead), then its outcome with the
  // way to send it. Kept until closed, like the repair report.
  import Toast from "./Toast.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import { dismissSurvey, surveyState } from "$lib/workshop/surveyState.svelte";
  import { ISSUE_URL, openExternal } from "$lib/links";
  import { t } from "$lib/i18n/index.svelte";
</script>

{#if surveyState.running}
  <Toast title={t("maintenance.surveyTitle")}>
    <ProgressBar ratio={null} phase={t("maintenance.surveying")} />
  </Toast>
{:else if surveyState.result}
  {@const r = surveyState.result}
  <Toast title={t("maintenance.surveyDone", { cars: r.cars, tracks: r.tracks })} onclose={dismissSurvey}>
    <div class="acts">
      <button class="btn" type="button" onclick={() => openExternal(ISSUE_URL)}>{t("maintenance.surveySend")}</button>
    </div>
  </Toast>
{:else if surveyState.error}
  <Toast tone="warn" title={t("maintenance.surveyTitle")} onclose={dismissSurvey}>
    <div class="err">{surveyState.error}</div>
  </Toast>
{/if}

<style>
  .acts {
    display: flex;
    margin-top: 8px;
  }
  .err {
    font-size: 12px;
    color: var(--txt2);
  }
</style>
