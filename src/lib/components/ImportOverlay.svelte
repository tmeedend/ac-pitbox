<script lang="ts">
  // Retour visuel de l'import, global (§4.2 : le glisser-déposer marche sur
  // toutes les vues, donc ce retour doit être visible peu importe l'écran ouvert).
  import {
    importState,
    dismissReport,
    resolvePendingConflict,
    resolveAmbiguous,
    requestCancelImport,
  } from "$lib/importState.svelte";
  import { nav, requestSection } from "$lib/nav.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { SubImported } from "$lib/library";

  // Libellé + classe CSS de la pastille d'issue d'un mod importé.
  function outcomeChip(o: string): { cls: string; label: string } {
    if (o === "UPDATE_REPLACE") return { cls: "upd", label: t("importOverlay.outcomeUpdate") };
    if (o === "DUPLICATE") return { cls: "dup", label: t("importOverlay.outcomeDuplicate") };
    if (o === "EXTENSION") return { cls: "ext", label: t("importOverlay.outcomeExtension") };
    return { cls: "new", label: t("importOverlay.outcomeNew") };
  }

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

  /** Ouvre la fiche d'un contenu depuis le rapport. Pour une couche, `id` est
   * déjà celui du contenu de base (§4.4) : c'est donc lui qui s'ouvre. */
  async function openContent(id: string, kind: string): Promise<void> {
    dismissReport();
    if (await requestSection(kind === "Track" ? "tracks" : "cars")) nav.openMod = id;
  }

  async function openSection(section: string): Promise<void> {
    dismissReport();
    await requestSection(section);
  }

  /** Skins et sons regroupés par contenu parent : un pack de quarante livrées
   * ferait déborder le rapport à raison d'une ligne chacune, alors qu'il n'y a
   * qu'une seule fiche à ouvrir au bout. */
  function subsByParent(subs: SubImported[]): { id: string; kind: string; skins: number; sounds: number }[] {
    const groups = new Map<string, { id: string; kind: string; skins: number; sounds: number }>();
    for (const s of subs) {
      const g = groups.get(s.parent_id) ?? {
        id: s.parent_id,
        kind: s.sub_type === "TRACK_SKIN" ? "Track" : "Car",
        skins: 0,
        sounds: 0,
      };
      if (s.sub_type === "SOUND") g.sounds++;
      else g.skins++;
      groups.set(s.parent_id, g);
    }
    return [...groups.values()];
  }

  // Total tous types confondus (§4.2) : un import peut ne produire aucun
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
          <!-- Un mod resté AMBIGU n'a rien écrit (§4.4) : il n'y a pas de fiche
               à ouvrir tant que l'utilisateur n'a pas tranché. -->
          {@const openable = m.outcome !== "AMBIGUOUS"}
          <div class="r-line">
            <span class="r-out {chip.cls}">{chip.label}</span>
            {#if openable}
              <button class="r-open" type="button" onclick={() => openContent(m.id_interne, m.kind)}>
                {m.display_name ?? m.id_interne}
              </button>
            {:else}
              {m.display_name ?? m.id_interne}
            {/if}
            {#if m.outcome === "DUPLICATE"}
              <span class="r-conflict">{t("importOverlay.duplicateNote")}</span>
            {:else if m.outcome === "EXTENSION"}
              <span class="r-conflict">{t("importOverlay.extensionNote", { added: m.added_count ?? 0, overwritten: m.overwritten_count ?? 0 })}</span>
            {/if}
          </div>
        {/each}
        {#each subsByParent(a.subs ?? []) as g}
          <div class="r-line shared">
            {t("importOverlay.subsAttached", {
              parts: `${g.skins ? t("importOverlay.skinCount", { count: g.skins }) : ""}${g.skins && g.sounds ? " · " : ""}${g.sounds ? t("importOverlay.soundCount", { count: g.sounds }) : ""}`,
            })}
            <button class="r-open" type="button" onclick={() => openContent(g.id, g.kind)}>{g.id}</button>
          </div>
        {/each}
        {#if (a.apps ?? []).length}
          <div class="r-line shared">
            <button class="r-open" type="button" onclick={() => openSection("apps")}>
              {t("importOverlay.appsImported", { count: a.apps.length })}
            </button>
          </div>
        {/if}
        {#if (a.others ?? []).length}
          <div class="r-line shared">
            <button class="r-open" type="button" onclick={() => openSection("others")}>
              {t("importOverlay.othersImported", { count: a.others.length })}
            </button>
          </div>
        {/if}
        {@const resExtracted =
          a.mods.reduce((acc, m) => acc + (m.resources_extracted ?? 0), 0) +
          (a.subs ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
          (a.apps ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0) +
          (a.others ?? []).reduce((acc, s) => acc + (s.resources_extracted ?? 0), 0)}
        {#if resExtracted > 0}
          <div class="r-line shared">{t("importOverlay.resourcesExtracted", { count: resExtracted })}</div>
        {/if}
        {@const extras = a.extras ?? 0}
        {#if extras > 0}
          <div class="r-line shared">{t("importOverlay.extrasAttached", { count: extras })}</div>
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
  /* Nom cliquable du rapport : ouvre la fiche du contenu. Bouton et non
     `<span onclick>` — le rapport doit rester atteignable au clavier et à la
     manette comme le reste de l'app. */
  .r-open {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--txt);
    text-align: left;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--line);
    text-underline-offset: 2px;
  }
  .r-open:hover,
  .r-open:focus-visible {
    color: var(--rosso-bright);
    text-decoration-color: currentColor;
  }
  .r-line.shared .r-open {
    color: var(--muted);
    font-size: 11px;
  }
  .r-line.shared .r-open:hover,
  .r-line.shared .r-open:focus-visible {
    color: var(--rosso-bright);
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
