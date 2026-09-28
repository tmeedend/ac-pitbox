<script lang="ts">
  // Progression et rapport d'un lot (§6.3bis), dans la pile de notifications.
  //
  // Le rapport vivait dans le panneau de sélection groupée, donc il partait
  // avec lui : fermer le panneau après un lot de quarante mods emportait la
  // liste des échecs, seul endroit où était écrit ce qui n'avait pas marché.
  import Toast from "./Toast.svelte";
  import { bulkState, dismissBulkResult, requestCancelBulk } from "$lib/library/bulkState.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import { message } from "@tauri-apps/plugin-dialog";
  import { deleteShowcasedCompletely } from "$lib/library/showcase.svelte";

  /** "Delete completely" after a showcase lot: the answer to the likeliest
   * surprise, "I deleted it and it is still there" (ESPACE§5.2). */
  function deleteThemCompletely(ids: string[]) {
    dismissBulkResult();
    deleteShowcasedCompletely(ids).catch((e) => {
      console.error("delete completely", e);
      void message(errorText(e), { title: t("common.error"), kind: "error" });
    });
  }

  // Clés explicites plutôt que construites à la volée : `t()` renvoyant la clé
  // quand elle manque, une opération non prévue s'afficherait telle quelle.
  const RUNNING_KEYS: Record<string, string> = {
    activate: "bulk.runningActivate",
    deactivate: "bulk.runningDeactivate",
    delete: "bulk.runningDelete",
    export: "bulk.runningExport",
    showcase: "bulk.runningShowcase",
  };
  const DONE_KEYS: Record<string, string> = {
    activate: "bulk.doneActivate",
    deactivate: "bulk.doneDeactivate",
    delete: "bulk.doneDelete",
    export: "bulk.doneExport",
    showcase: "bulk.doneShowcase",
  };

  const resultTitle = $derived.by(() => {
    const r = bulkState.result;
    if (!r) return "";
    // A showcase lot says what it freed (ESPACE§5.2) — "RSS GTM Lanzo V10 is
    // in the showcase, 550 MB freed" rather than a count.
    const sc = r.report.showcase;
    const head = sc
      ? sc.name
        ? t("showcase.doneOne", { name: sc.name, size: fmtSize(sc.freedBytes) })
        : t("showcase.doneMany", { count: r.report.ok.length, size: fmtSize(sc.freedBytes) })
      : t(DONE_KEYS[r.op] ?? r.op, { count: r.report.ok.length });
    return (
      head +
      (r.report.skipped.length ? t("bulk.skippedCount", { count: r.report.skipped.length }) : "") +
      (r.report.failed.length ? t("bulk.failedCount", { count: r.report.failed.length }) : "") +
      (r.report.cancelled ? t("bulk.cancelledNote") : "")
    );
  });
</script>

{#if bulkState.result}
  {@const failed = bulkState.result.report.failed}
  {@const sc = bulkState.result.report.showcase}
  {@const done = bulkState.result.report.ok}
  <Toast title={resultTitle} onclose={dismissBulkResult}>
    {#snippet actions()}
      {#if sc && done.length}
        <button class="btn-ghost b-cancel" type="button" onclick={() => deleteThemCompletely(done)}>
          {t("showcase.complete")}
        </button>
      {/if}
    {/snippet}
    {#if sc && !sc.recycled && done.length}
      <div class="fail"><span class="fail-err">{t("showcase.deletedForGood")}</span></div>
    {/if}
    {#if failed.length}
      {#each failed as f (f.id)}
        <div class="fail">
          <span class="fail-id mono">{f.id}</span>
          <span class="fail-err">{f.error}</span>
        </div>
      {/each}
    {/if}
  </Toast>
{/if}

{#if bulkState.running && bulkState.progress}
  {@const p = bulkState.progress}
  <Toast title={t(RUNNING_KEYS[p.op] ?? p.op, { count: p.total })} truncate>
    {#snippet actions()}
      <button class="btn-ghost b-cancel" type="button" onclick={requestCancelBulk} disabled={bulkState.cancelling}>
        {bulkState.cancelling ? t("bulk.cancelling") : t("bulk.cancel")}
      </button>
    {/snippet}
    <div class="b-row">
      <span class="b-id mono">{p.id}</span>
      <span class="b-count mono">{Math.max(1, p.index)} / {p.total}</span>
    </div>
    <div class="b-bar">
      <!-- Rapport d'items, pas de temps : contrairement à l'import, un mod
           n'a pas de poids estimé ici — une activation est une junction, quel
           que soit le circuit derrière. -->
      <div class="b-fill" style:width="{p.total ? (p.index / p.total) * 100 : 0}%"></div>
    </div>
  </Toast>
{/if}

<style>
  .b-cancel {
    font-size: 11px;
  }
  .b-cancel:disabled {
    color: var(--muted);
    cursor: default;
  }
  .b-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 6px;
    min-width: 0;
  }
  .b-id {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .b-count {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .b-bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
  }
  .b-fill {
    height: 100%;
    background: var(--rosso);
    transition: width 0.2s;
  }
  .fail {
    display: flex;
    gap: 8px;
    padding: 2px 0;
    font-size: 11px;
  }
  .fail-id {
    flex: none;
    color: var(--txt2);
  }
  .fail-err {
    color: var(--rosso-bright);
    overflow-wrap: anywhere;
  }
</style>
