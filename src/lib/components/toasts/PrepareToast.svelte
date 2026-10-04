<script lang="ts">
  // « Prepare & join » in the notification stack (SPEC-play-online.md,
  // "Contenu manquant"): one notification for the whole run, the way a batch
  // import shows its overall bar above the current archive. The title says
  // which server is being prepared; the first bar, which mod of how many; the
  // second, the current one's download, then its import. While it runs, the
  // single-mod toasts it is made of (`UpdateToast`'s download, `ImportToasts`'
  // progress and reports) step aside: a dozen of them came and went, one per
  // mod, with no idea of how far the whole was (reported).
  import Toast from "./Toast.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { cancelUpdateDownload, modUpdates } from "$lib/library/modUpdates.svelte";
  import { importState, requestCancelImport } from "$lib/workshop/importState.svelte";
  import { prepareRun, prepareStopping, stopPrepare } from "$lib/online/prepare.svelte";

  function mb(bytes: number): string {
    return (bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0);
  }

  /** « Stop » ends the run, and cancels what is under way for the current mod. */
  function stop() {
    stopPrepare();
    if (modUpdates.busy?.phase === "download") cancelUpdateDownload();
    else if (importState.importing) requestCancelImport();
  }
</script>

{#if prepareRun()}
  {@const run = prepareRun()!}
  {@const download = modUpdates.busy?.phase === "download" ? modUpdates.busy : null}
  {@const importing = importState.importing ? importState.progress : null}
  <Toast title={t("online.prepareTitle", { server: run.server })} truncate>
    {#snippet actions()}
      <button class="btn-ghost p-cancel" type="button" disabled={prepareStopping()} onclick={stop}>
        {prepareStopping() ? t("importOverlay.cancelling") : t("importOverlay.cancel")}
      </button>
    {/snippet}
    <!-- The layout of a batch import (`ImportToasts`), asked for as such:
         the current mod in the main bar, the count and the run's bar under
         it, thinner. -->
    {#if download}
      <ProgressBar ratio={download.total ? download.received / download.total : null} phase={t("modUpdates.downloadPhase")}>
        {#snippet detail()}
          {run.name}
          <span class="mono">
            {download.total
              ? t("modUpdates.sizeOf", { done: mb(download.received), total: mb(download.total) })
              : t("modUpdates.size", { done: mb(download.received) })}
          </span>
        {/snippet}
      </ProgressBar>
    {:else}
      {@const settled = importing && importing.phase !== "queued" && importing.phase !== "sizing"}
      <ProgressBar ratio={settled ? importing.item_ratio : null} phase={t("online.prepareImporting")}>
        {#snippet detail()}{run.name}{/snippet}
      </ProgressBar>
    {/if}
    <div class="p-overall"><span class="mono">{run.index + 1} / {run.total}</span></div>
    <ProgressBar ratio={run.index / run.total} thin />
  </Toast>
{/if}

<style>
  .p-overall {
    display: flex;
    justify-content: flex-end;
    margin-top: 8px;
    font-size: 11px;
    color: var(--muted);
  }
</style>
