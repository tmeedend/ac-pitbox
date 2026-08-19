<script lang="ts">
  // Import feedback in the notification stack (§4.2): progress while a batch
  // runs, then one report per batch. Global, because drag-and-drop works on
  // every screen — this must show up whatever is open.
  //
  // The arbitration modals (fuzzy conflict, ambiguous import) stay in
  // `ImportOverlay`: they are full-screen and block, they are not stack items.
  import {
    importState,
    dismissReport,
    toggleReport,
    collapseReportOnNavigate,
    importSummary,
    requestCancelImport,
  } from "$lib/importState.svelte";
  import ImportReport from "./ImportReport.svelte";
  import Toast from "./Toast.svelte";
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

  // A modal is waiting for an answer: reports would be dead weight behind it.
  const arbitrating = $derived(
    importState.pendingConflicts.length > 0 || importState.pendingAmbiguous.length > 0,
  );
</script>

<!-- Oldest first: the stack is a column, so the newest report — the unfolded
     one — ends up nearest the corner, and progress below everything. -->
{#if !arbitrating}
  {#each importState.reports as entry (entry.id)}
    <Toast
      title={importSummary(entry.report)}
      collapsed={entry.collapsed}
      ontoggle={() => toggleReport(entry.id)}
      onclose={() => dismissReport(entry.id)}
    >
      <ImportReport report={entry.report} onnavigate={() => collapseReportOnNavigate(entry.id)} />
    </Toast>
  {/each}
{/if}

{#if importState.importing && importState.progress}
  {@const p = importState.progress}
  {@const settled = p.phase !== "queued" && p.phase !== "sizing"}
  <!-- Le titre retombe sur la phase : au tout début d'un lot, le backend n'a
       encore ni archive ni libellé, et un bandeau vide n'annonce rien. -->
  <Toast title={p.archive || p.label || t(PHASE_KEYS[p.phase] ?? p.phase)} truncate>
    {#snippet actions()}
      <button
        class="btn-ghost p-cancel"
        type="button"
        onclick={requestCancelImport}
        disabled={importState.cancelling}
      >
        {importState.cancelling ? t("importOverlay.cancelling") : t("importOverlay.cancel")}
      </button>
    {/snippet}
    <div class="p-phase-row">
      <span class="mono p-phase">{t(PHASE_KEYS[p.phase] ?? p.phase)}</span>
      {#if settled && p.sub_total > 1}
        <span class="p-sub">{p.label} <span class="mono">({p.sub_current}/{p.sub_total})</span></span>
      {/if}
    </div>
    <div class="p-bar">
      <div class="p-fill" style:width="{p.item_ratio * 100}%" class:indeterminate={!settled}></div>
    </div>
    <!-- Compte du lot et temps restant sur la même ligne, mais deux
         conditions distinctes : le compte n'a de sens que pour un vrai lot
         (pour un seul mod, ce serait toujours « 1/1 ») alors que l'ETA reste
         utile même seul — c'est justement l'import le plus long (un gros mod)
         qui en a le plus besoin. La barre globale, elle, répéterait
         exactement celle du dessus pour un seul mod : elle reste réservée au lot. -->
    {#if settled && (p.item_count > 1 || p.eta_secs !== null)}
      <div class="p-overall">
        {#if p.item_count > 1}<span class="mono">{p.item_index || 1} / {p.item_count}</span>{/if}
        {#if p.eta_secs !== null}<span class="p-eta">{etaText(p.eta_secs)}</span>{/if}
      </div>
    {/if}
    {#if p.item_count > 1}
      <div class="p-bar global">
        <div class="p-fill" style:width="{p.overall_ratio * 100}%"></div>
      </div>
    {/if}
  </Toast>
{/if}

<style>
  .p-cancel {
    font-size: 11px;
  }
  .p-cancel:disabled {
    color: var(--muted);
    cursor: default;
  }
  .p-phase-row {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-bottom: 6px;
    min-width: 0;
  }
  .p-sub {
    color: var(--muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .p-phase {
    flex: none;
    color: var(--rosso-bright);
    font-size: 10px;
    text-transform: uppercase;
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
    /* Poussé à droite même seul (pas de compte de lot à côté) : une marge
       auto l'emporte sur `justify-content` qu'il ait ou non un voisin. */
    margin-left: auto;
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
  /* Les styles des lignes du rapport vivent dans `ImportReport.svelte` avec
     leur markup : le CSS des composants est scopé, déplacer l'un sans l'autre
     laisserait ici des règles qui ne s'appliquent plus à rien. */
</style>
