<script lang="ts">
  // Retour visuel de l'import, global (§4.2 : le glisser-déposer marche sur
  // toutes les vues, donc ce retour doit être visible peu importe l'écran ouvert).
  import {
    importState,
    dismissReport,
    importSummary,
    resolvePendingConflict,
    resolveAmbiguous,
    requestCancelImport,
  } from "$lib/importState.svelte";
  import ImportReport from "./ImportReport.svelte";
  import { t } from "$lib/i18n/index.svelte";

  // Clés explicites plutôt qu'une clé construite à la volée : `t()` renvoyant la
  // clé quand elle manque, une phase non prévue s'afficherait telle quelle.
  const PHASE_KEYS: Record<string, string> = {
    queued: "importOverlay.phaseQueued",
    sizing: "importOverlay.phaseSizing",
    extract: "importOverlay.phaseExtract",
    scan: "importOverlay.phaseScan",
    filing: "importOverlay.phaseFiling",
    done: "importOverlay.phaseDone",
    cancelled: "importOverlay.phaseCancelled",
  };

  /** Temps restant en unités grossières : à la seconde près, il sauterait à
   * chaque événement pour une précision que l'estimation n'a pas. */
  function etaText(secs: number): string {
    if (secs < 60) return t("importOverlay.etaSeconds", { n: Math.max(5, Math.round(secs / 5) * 5) });
    return t("importOverlay.etaMinutes", { n: Math.max(1, Math.round(secs / 60)) });
  }

</script>

{#if importState.importing && importState.progress}
  {@const p = importState.progress}
  {@const settled = p.phase !== "queued" && p.phase !== "sizing"}
  <div class="toast progress-toast">
    <div class="p-head">
      <span class="p-title">
        <span class="mono p-phase">{t(PHASE_KEYS[p.phase] ?? p.phase)}</span>
        {p.archive || p.label}
      </span>
      <button
        class="btn-ghost p-cancel"
        type="button"
        onclick={requestCancelImport}
        disabled={importState.cancelling}
      >
        {importState.cancelling ? t("importOverlay.cancelling") : t("importOverlay.cancel")}
      </button>
    </div>
    {#if settled && p.sub_total > 1}
      <div class="p-sub">{p.label} <span class="mono">({p.sub_current}/{p.sub_total})</span></div>
    {/if}
    <div class="p-bar">
      <div class="p-fill" style:width="{p.item_ratio * 100}%" class:indeterminate={!settled}></div>
    </div>
    <!-- Barre globale seulement quand il y a bien un lot : pour un seul mod,
         elle répéterait la barre du dessus. -->
    {#if p.item_count > 1}
      <div class="p-overall">
        <span class="mono">{p.item_index || 1} / {p.item_count}</span>
        {#if p.eta_secs !== null}<span class="p-eta">{etaText(p.eta_secs)}</span>{/if}
      </div>
      <div class="p-bar global">
        <div class="p-fill" style:width="{p.overall_ratio * 100}%"></div>
      </div>
    {/if}
  </div>
{/if}

{#if importState.report && !importState.pendingConflicts.length && !importState.pendingAmbiguous.length}
  {@const report = importState.report}
  <div class="toast report-toast">
    <div class="report-head">
      <span>{importSummary(report)}</span>
      <button class="btn-ghost" onclick={dismissReport}>✕</button>
    </div>
    <div class="report-body">
      <ImportReport {report} onnavigate={dismissReport} />
    </div>
  </div>
{/if}

{#if importState.pendingConflicts.length}
  {@const c = importState.pendingConflicts[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>{t("importOverlay.newVersionTitle")}</h3>
      <p>
        {t("importOverlay.modalBodyOpen")}<b>{c.newName}</b>{t("importOverlay.modalBodyMid")}<span class="mono">{c.oldId}</span>{t("importOverlay.modalBodyArrow")}<span class="mono">{c.newId}</span>{t("importOverlay.modalBodyEnd")}
      </p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolvePendingConflict(c, "keep_both")}>
          {t("importOverlay.keepBoth")}
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolvePendingConflict(c, "replace")}>
          {t("importOverlay.replaceOld")}
        </button>
      </div>
      {#if importState.pendingConflicts.length > 1}
        <div class="modal-rest">{t("importOverlay.modalRest", { count: importState.pendingConflicts.length - 1 })}</div>
      {/if}
    </div>
  </div>
{/if}

{#if importState.pendingAmbiguous.length && !importState.pendingConflicts.length}
  {@const a = importState.pendingAmbiguous[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>{t("importOverlay.ambiguousTitle")}</h3>
      <p>{t("importOverlay.ambiguousBody", { name: a.name, added: a.added, overwritten: a.overwritten, total: a.total })}</p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolveAmbiguous(a, "extension")}>
          {t("importOverlay.chooseExtension")}
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolveAmbiguous(a, "update")}>
          {t("importOverlay.chooseUpdate")}
        </button>
      </div>
      {#if importState.pendingAmbiguous.length > 1}
        <div class="modal-rest">{t("importOverlay.ambiguousRest", { count: importState.pendingAmbiguous.length - 1 })}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 22px;
    bottom: 22px;
    width: 380px;
    max-width: calc(100vw - 44px);
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.45);
    z-index: 80;
    font-size: 12px;
  }
  .progress-toast {
    padding: 12px 14px;
  }
  .p-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    justify-content: space-between;
    color: var(--txt2);
    margin-bottom: 8px;
  }
  .p-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .p-cancel {
    flex: none;
    font-size: 11px;
  }
  .p-cancel:disabled {
    color: var(--muted);
    cursor: default;
  }
  .p-sub {
    color: var(--muted);
    font-size: 11px;
    margin-bottom: 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .p-phase {
    color: var(--rosso-bright);
    font-size: 10px;
    text-transform: uppercase;
    margin-right: 6px;
  }
  .p-overall {
    display: flex;
    justify-content: space-between;
    color: var(--muted);
    font-size: 11px;
    margin: 10px 0 4px;
  }
  .p-eta {
    color: var(--txt2);
  }
  .p-bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
  }
  /* Barre du lot : plus discrète que celle du mod en cours, qui est
     l'information immédiate. */
  .p-bar.global {
    height: 2px;
  }
  .p-fill {
    height: 100%;
    background: var(--rosso);
    transition: width 0.2s;
  }
  .p-fill.indeterminate {
    animation: slide 1s ease-in-out infinite;
  }
  @keyframes slide {
    0% { margin-left: 0; }
    50% { margin-left: 70%; }
    100% { margin-left: 0; }
  }

  .report-toast {
    padding: 0;
    max-height: 50vh;
    display: flex;
    flex-direction: column;
  }
  .report-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
    flex: none;
  }
  .report-body {
    padding: 8px 12px 10px;
    overflow-y: auto;
  }
  /* Les styles des lignes du rapport vivent dans `ImportReport.svelte` avec
     leur markup : le CSS des composants est scopé, déplacer l'un sans l'autre
     laisserait ici des règles qui ne s'appliquent plus à rien. */

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 90;
  }
  .modal {
    width: 440px;
    max-width: 90vw;
    background: var(--panel);
    border: 1px solid var(--rosso);
    padding: 22px 24px;
  }
  .modal h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 12px;
  }
  .modal p {
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--txt2);
    margin-bottom: 18px;
  }
  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
  }
  .modal-rest {
    margin-top: 12px;
    font-size: 11px;
    color: var(--muted);
    text-align: right;
  }
</style>
