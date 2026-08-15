<script lang="ts">
  // Retour visuel de l'import, global (§4.6bis : le glisser-déposer marche sur
  // toutes les vues, donc ce retour doit être visible peu importe l'écran ouvert).
  import { importState, dismissReport, resolvePendingConflict, resolveAmbiguous } from "$lib/importState.svelte";
  import { t } from "$lib/i18n/index.svelte";

  // Libellé + classe CSS de la pastille d'issue d'un mod importé.
  function outcomeChip(o: string): { cls: string; label: string } {
    if (o === "UPDATE_REPLACE") return { cls: "upd", label: t("importOverlay.outcomeUpdate") };
    if (o === "DUPLICATE") return { cls: "dup", label: t("importOverlay.outcomeDuplicate") };
    if (o === "EXTENSION") return { cls: "ext", label: t("importOverlay.outcomeExtension") };
    return { cls: "new", label: t("importOverlay.outcomeNew") };
  }

  // Total tous types confondus (§4.6bis) : un import peut ne produire aucun
  // mod de premier niveau (ex. un pack de skins/sons rattaché à une voiture
  // déjà connue) sans pour autant n'avoir « rien » importé — le titre doit
  // refléter ce qui a été réellement ajouté, pas seulement les mods.
  function importSummary(): string {
    const r = importState.report ?? [];
    const n = r.reduce(
      (acc, a) => acc + a.mods.length + (a.subs?.length ?? 0) + (a.apps?.length ?? 0) + (a.others?.length ?? 0),
      0,
    );
    const errs = r.filter((a) => a.error).length;
    return t("importOverlay.summaryBase", { n }) + (errs ? t("importOverlay.summaryErrs", { errs }) : "");
  }
</script>

{#if importState.importing && importState.progress}
  {@const p = importState.progress}
  <div class="toast progress-toast">
    <div class="p-label">
      {#if p.phase === "queued"}
        {t("importOverlay.queued", { n: p.total })}
      {:else}
        <span class="mono p-phase">{p.phase}</span>
        {p.archive} — {p.label}
        {#if p.total > 0 && p.phase === "filing"}
          <span class="mono">({p.current}/{p.total})</span>
        {/if}
      {/if}
    </div>
    <div class="p-bar">
      <div
        class="p-fill"
        style:width={p.total > 0 && p.phase !== "queued" ? `${(p.current / p.total) * 100}%` : "30%"}
        class:indeterminate={p.phase === "queued" || p.total === 0}
      ></div>
    </div>
  </div>
{/if}

{#if importState.report && !importState.pendingConflicts.length && !importState.pendingAmbiguous.length}
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
          {@const chip = outcomeChip(m.outcome)}
          <div class="r-line">
            <span class="r-out {chip.cls}">{chip.label}</span>
            {m.display_name ?? m.id_interne}
            {#if m.outcome === "DUPLICATE"}
              <span class="r-conflict">{t("importOverlay.duplicateNote")}</span>
            {:else if m.outcome === "EXTENSION"}
              <span class="r-conflict">{t("importOverlay.extensionNote", { added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
            {/if}
          </div>
        {/each}
        {@const replaced = (a.shared ?? []).filter((s) => s.disposition === "replaced")}
        {@const added = (a.shared ?? []).filter((s) => s.disposition === "installed")}
        {#if added.length}
          <div class="r-line shared">{t("importOverlay.sharedInstalled", { count: added.length })}</div>
        {/if}
        {#each replaced as s}
          <div class="r-line shared warn">{t("importOverlay.sharedReplaced", { kind: s.kind === "fonts" ? t("importOverlay.fontLabel") : t("importOverlay.driverLabel"), name: s.name })}</div>
        {/each}
        {#if (a.subs ?? []).length}
          {@const skins = a.subs.filter((s) => s.sub_type === "SKIN").length}
          {@const sounds = a.subs.filter((s) => s.sub_type === "SOUND").length}
          <div class="r-line shared">
            {t("importOverlay.subsAttached", {
              parts: `${skins ? t("importOverlay.skinCount", { count: skins }) : ""}${skins && sounds ? " · " : ""}${sounds ? t("importOverlay.soundCount", { count: sounds }) : ""}`,
            })}
          </div>
        {/if}
        {#if (a.apps ?? []).length}
          <div class="r-line shared">{t("importOverlay.appsImported", { count: a.apps.length })}</div>
        {/if}
        {#if (a.others ?? []).length}
          <div class="r-line shared">{t("importOverlay.othersImported", { count: a.others.length })}</div>
        {/if}
        {@const resExtracted =
          a.mods.reduce((acc, m) => acc + (m.resources_extracted ?? 0), 0) +
          (a.subs ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
          (a.apps ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
          (a.others ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0)}
        {#if resExtracted > 0}
          <div class="r-line shared">{t("importOverlay.resourcesExtracted", { count: resExtracted })}</div>
        {/if}
        {@const satellites = a.satellites ?? 0}
        {#if satellites > 0}
          <div class="r-line shared">{t("importOverlay.satellitesAttached", { count: satellites })}</div>
        {/if}
      {/each}
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
  .r-out.ext {
    color: var(--green);
    border-color: var(--green-border);
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
