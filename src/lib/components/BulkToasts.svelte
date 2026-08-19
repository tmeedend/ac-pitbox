<script lang="ts">
  // Progression et rapport d'un lot (§6.3bis), dans la pile de notifications.
  //
  // Le rapport vivait dans le panneau de sélection groupée, donc il partait
  // avec lui : fermer le panneau après un lot de quarante mods emportait la
  // liste des échecs, seul endroit où était écrit ce qui n'avait pas marché.
  import Toast from "./Toast.svelte";
  import { bulkState, dismissBulkResult, requestCancelBulk } from "$lib/bulkState.svelte";
  import { t } from "$lib/i18n/index.svelte";

  // Clés explicites plutôt que construites à la volée : `t()` renvoyant la clé
  // quand elle manque, une opération non prévue s'afficherait telle quelle.
  const RUNNING_KEYS: Record<string, string> = {
    activate: "bulk.runningActivate",
    deactivate: "bulk.runningDeactivate",
    delete: "bulk.runningDelete",
    export: "bulk.runningExport",
  };
  const DONE_KEYS: Record<string, string> = {
    activate: "bulk.doneActivate",
    deactivate: "bulk.doneDeactivate",
    delete: "bulk.doneDelete",
    export: "bulk.doneExport",
  };

  const resultTitle = $derived.by(() => {
    const r = bulkState.result;
    if (!r) return "";
    return (
      t(DONE_KEYS[r.op] ?? r.op, { count: r.report.ok.length }) +
      (r.report.failed.length ? t("bulk.failedCount", { count: r.report.failed.length }) : "") +
      (r.report.cancelled ? t("bulk.cancelledNote") : "")
    );
  });
</script>

{#if bulkState.result}
  {@const failed = bulkState.result.report.failed}
  <Toast title={resultTitle} onclose={dismissBulkResult}>
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
