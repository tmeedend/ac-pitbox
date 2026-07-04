<script lang="ts">
  // Retour visuel de l'import, global (§4.6bis : le glisser-déposer marche sur
  // toutes les vues, donc ce retour doit être visible peu importe l'écran ouvert).
  import { importState, dismissReport, resolvePendingConflict } from "$lib/importState.svelte";

  function importSummary(): string {
    const r = importState.report ?? [];
    const n = r.reduce((acc, a) => acc + a.mods.length, 0);
    const errs = r.filter((a) => a.error).length;
    return `${n} mod(s) importé(s)${errs ? `, ${errs} archive(s) en erreur` : ""}`;
  }
</script>

{#if importState.importing && importState.progress}
  {@const p = importState.progress}
  <div class="toast progress-toast">
    <div class="p-label">
      <span class="mono p-phase">{p.phase}</span>
      {p.archive} — {p.label}
      {#if p.total > 0 && p.phase === "filing"}
        <span class="mono">({p.current}/{p.total})</span>
      {/if}
    </div>
    <div class="p-bar">
      <div
        class="p-fill"
        style:width={p.total > 0 ? `${(p.current / p.total) * 100}%` : "30%"}
        class:indeterminate={p.total === 0}
      ></div>
    </div>
  </div>
{/if}

{#if importState.report && !importState.pendingConflicts.length}
  {@const report = importState.report}
  <div class="toast report-toast">
    <div class="report-head">
      <span>{importSummary()}</span>
      <button class="btn-ghost" onclick={dismissReport}>✕</button>
    </div>
    <div class="report-body">
      {#each report as a}
        {#if a.error}
          <div class="r-line err">⚠ {a.archive} — {a.error}</div>
        {/if}
        {#each a.mods as m}
          <div class="r-line">
            <span class="r-out {m.outcome === 'UPDATE_REPLACE' ? 'upd' : m.outcome === 'DUPLICATE' ? 'dup' : 'new'}">
              {m.outcome === "UPDATE_REPLACE" ? "MAJ" : m.outcome === "DUPLICATE" ? "DÉJÀ PRÉSENT" : "NOUVEAU"}
            </span>
            {m.display_name ?? m.id_interne}
            {#if m.outcome === "DUPLICATE"}
              <span class="r-conflict">archive identique — non réimporté</span>
            {/if}
          </div>
        {/each}
        {@const replaced = (a.shared ?? []).filter((s) => s.disposition === "replaced")}
        {@const added = (a.shared ?? []).filter((s) => s.disposition === "installed")}
        {#if added.length}
          <div class="r-line shared">+ {added.length} ressource(s) partagée(s) installée(s) (fonts/drivers, §4.8)</div>
        {/if}
        {#each replaced as s}
          <div class="r-line shared warn">⚠ {s.kind === "fonts" ? "Font" : "Driver"} « {s.name} » remplacé par une version différente</div>
        {/each}
        {#if (a.subs ?? []).length}
          {@const skins = a.subs.filter((s) => s.sub_type === "SKIN").length}
          {@const sounds = a.subs.filter((s) => s.sub_type === "SOUND").length}
          <div class="r-line shared">
            + {skins ? `${skins} skin(s)` : ""}{skins && sounds ? " · " : ""}{sounds ? `${sounds} son(s)` : ""} rattaché(s) (§12bis)
          </div>
        {/if}
        {#if (a.apps ?? []).length}
          <div class="r-line shared">+ {a.apps.length} app(s) importée(s) (§12bis)</div>
        {/if}
        {#if (a.others ?? []).length}
          <div class="r-line shared">+ {a.others.length} mod(s) autre(s) importé(s) (§6.1bis)</div>
        {/if}
      {/each}
    </div>
  </div>
{/if}

{#if importState.pendingConflicts.length}
  {@const c = importState.pendingConflicts[0]}
  <div class="modal-backdrop">
    <div class="modal">
      <h3>Nouvelle version possible</h3>
      <p>
        « <b>{c.newName}</b> » ressemble à un mod déjà présent
        (dossier différent : <span class="mono">{c.oldId}</span> →
        <span class="mono">{c.newId}</span>). Que faire ?
      </p>
      <div class="modal-actions">
        <button class="btn" type="button" onclick={() => resolvePendingConflict(c, "keep_both")}>
          Garder les deux
        </button>
        <button class="btn btn-primary" type="button" onclick={() => resolvePendingConflict(c, "replace")}>
          Écraser l'ancienne
        </button>
      </div>
      {#if importState.pendingConflicts.length > 1}
        <div class="modal-rest">{importState.pendingConflicts.length - 1} autre(s) à traiter</div>
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
  .p-label {
    color: var(--txt2);
    margin-bottom: 8px;
  }
  .p-phase {
    color: var(--rosso-bright);
    font-size: 10px;
    text-transform: uppercase;
    margin-right: 6px;
  }
  .p-bar {
    height: 4px;
    background: var(--line);
    overflow: hidden;
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
  .r-line {
    padding: 2px 0;
    color: var(--txt2);
  }
  .r-line.err {
    color: var(--rosso-bright);
  }
  .r-line.shared {
    color: var(--muted);
    font-size: 11px;
    padding-top: 4px;
  }
  .r-line.shared.warn {
    color: var(--yellow);
  }
  .r-out {
    font-size: 9px;
    letter-spacing: 0.5px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    margin-right: 6px;
  }
  .r-out.new {
    color: var(--green);
    border-color: var(--green-border);
  }
  .r-out.upd {
    color: var(--yellow);
    border-color: #4a4426;
  }
  .r-out.dup {
    color: var(--muted);
    border-color: var(--line);
  }
  .r-conflict {
    color: var(--yellow);
    margin-left: 6px;
    font-size: 11px;
  }

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
