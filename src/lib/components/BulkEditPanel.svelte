<script lang="ts">
  // Édition groupée (§6.3bis/§6.3ter) : panneau bas en surimpression quand
  // plusieurs mods sont sélectionnés (Ctrl/Alt-clic dans la bibliothèque) —
  // ne remplace plus le panneau de détail (ModDetail reste sur la voiture/le
  // circuit dernier cliqué) ni ne réduit la largeur de la grille. N'expose que
  // les champs communs à tout mod — jamais les champs propres à un type
  // (specs voiture, skin piloté, version active), qui restent réservés à la
  // fiche détail d'un seul mod. Exception : la section Adversaires
  // (voitures uniquement), qui agit sur le réglage de session course.
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import type { ModCard } from "$lib/library";
  import {
    bulkSetFavorite,
    bulkSetCategory,
    bulkAddTag,
    bulkRemoveTag,
    bulkActivate,
    bulkDeactivate,
    bulkDelete,
    bulkExport,
    type BulkReport,
  } from "$lib/bulkEdit";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    ids: string[];
    cards: ModCard[];
    /** Voitures uniquement : conditionne l'affichage de la section Adversaires (§6.3ter). */
    isCar: boolean;
    onclose: () => void;
    onchange: () => void;
    onSetOpponents: () => void;
    onAddOpponents: () => void;
  }
  let { ids, cards, isCar, onclose, onchange, onSetOpponents, onAddOpponents }: Props = $props();

  let busy = $state(false);
  let error = $state("");
  let report = $state<BulkReport | null>(null);
  let categoryInput = $state("");
  let tagInput = $state("");
  let exporting = $state(false);
  let exportMsg = $state("");

  const categories = $derived(
    [...new Set(cards.map((c) => c.category).filter((c): c is string => !!c))].sort(),
  );

  async function run(action: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await action();
      onchange();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function runReport(action: () => Promise<BulkReport>) {
    busy = true;
    error = "";
    report = null;
    try {
      report = await action();
      onchange();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const markFavorite = () => run(() => bulkSetFavorite(ids, true));
  const unmarkFavorite = () => run(() => bulkSetFavorite(ids, false));
  const activateAll = () => runReport(() => bulkActivate(ids));
  const deactivateAll = () => runReport(() => bulkDeactivate(ids));

  function applyCategory() {
    const cat = categoryInput.trim();
    if (!cat) return;
    run(() => bulkSetCategory(ids, cat));
  }

  function addTag() {
    const tag = tagInput.trim();
    if (!tag) return;
    tagInput = "";
    run(() => bulkAddTag(ids, tag));
  }

  function removeTag() {
    const tag = tagInput.trim();
    if (!tag) return;
    tagInput = "";
    run(() => bulkRemoveTag(ids, tag));
  }

  async function deleteAll() {
    if (busy) return;
    const ok = await confirm(t("bulkEdit.confirmDelete", { count: ids.length }), {
      title: t("common.delete"),
      kind: "warning",
    });
    if (!ok) return;
    await runReport(() => bulkDelete(ids));
    if (report && report.failed.length === 0) onclose();
  }

  async function doExport() {
    if (exporting) return;
    const dir = await open({ directory: true, multiple: false });
    if (!dir || typeof dir !== "string") return;
    exporting = true;
    error = "";
    exportMsg = "";
    try {
      const items = await bulkExport(ids, dir);
      const done = items.filter((i) => i.report).length;
      exportMsg = t("bulkEdit.exportDone", { count: done });
    } catch (e) {
      error = String(e);
    } finally {
      exporting = false;
    }
  }
</script>

<aside class="panel">
  <header>
    <h2>{t("bulkEdit.title", { count: ids.length })}</h2>
    <ul class="chips">
      {#each cards as c (c.id_interne)}
        <li>{c.display_name ?? c.id_interne}</li>
      {/each}
    </ul>
    <button class="btn-ghost close" type="button" onclick={onclose} title={t("bulkEdit.clearTooltip")}>✕</button>
  </header>

  {#if error}<div class="err">{error}</div>{/if}
  {#if report}
    <div class="report" class:warn={report.failed.length > 0}>
      {t("bulkEdit.reportOk", { count: report.ok.length })}
      {#if report.failed.length}
        · {t("bulkEdit.reportFailed", { count: report.failed.length })}
        <ul class="fail-list">
          {#each report.failed as f}<li>{f.id} — {f.error}</li>{/each}
        </ul>
      {/if}
    </div>
  {/if}

  <div class="sections">
    <section>
      <h3>{t("bulkEdit.favoriteSection")}</h3>
      <div class="row">
        <button class="btn" type="button" onclick={markFavorite} disabled={busy}>{t("bulkEdit.markFavorite")}</button>
        <button class="btn" type="button" onclick={unmarkFavorite} disabled={busy}>{t("bulkEdit.unmarkFavorite")}</button>
      </div>
    </section>

    <section>
      <h3>{t("bulkEdit.stateSection")}</h3>
      <div class="row">
        <button class="btn" type="button" onclick={activateAll} disabled={busy}>{t("bulkEdit.activateAll")}</button>
        <button class="btn" type="button" onclick={deactivateAll} disabled={busy}>{t("bulkEdit.deactivateAll")}</button>
      </div>
    </section>

    <section>
      <h3>{t("bulkEdit.categorySection")}</h3>
      <div class="row">
        <input
          class="input"
          list="bulk-categories"
          placeholder={t("bulkEdit.categoryPlaceholder")}
          bind:value={categoryInput}
          onkeydown={(e) => e.key === "Enter" && applyCategory()}
        />
        <datalist id="bulk-categories">
          {#each categories as cat}<option value={cat}></option>{/each}
        </datalist>
        <button class="btn" type="button" onclick={applyCategory} disabled={busy || !categoryInput.trim()}>{t("bulkEdit.apply")}</button>
      </div>
    </section>

    <section>
      <h3>{t("bulkEdit.tagsSection")}</h3>
      <div class="row">
        <input
          class="input"
          placeholder={t("bulkEdit.tagPlaceholder")}
          bind:value={tagInput}
          onkeydown={(e) => e.key === "Enter" && addTag()}
        />
        <button class="btn" type="button" onclick={addTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.addTagToAll")}</button>
        <button class="btn" type="button" onclick={removeTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.removeTagFromAll")}</button>
      </div>
    </section>

    {#if isCar}
      <section>
        <h3>{t("bulkEdit.opponentsSection")}</h3>
        <p class="opp-hint">{t("bulkEdit.opponentsHint")}</p>
        <div class="row">
          <button class="btn" type="button" onclick={onSetOpponents}>{t("bulkEdit.setOpponents")}</button>
          <button class="btn" type="button" onclick={onAddOpponents}>{t("bulkEdit.addOpponents")}</button>
        </div>
      </section>
    {/if}

    <section>
      <h3>{t("bulkEdit.exportSection")}</h3>
      <div class="row">
        <button class="btn-ghost export" type="button" onclick={doExport} disabled={exporting}>
          {exporting ? t("detail.exporting") : t("bulkEdit.exportAll")}
        </button>
      </div>
      {#if exportMsg}<div class="export-ok">{exportMsg}</div>{/if}
    </section>

    <section class="danger">
      <h3>{t("bulkEdit.deleteSection")}</h3>
      <div class="row">
        <button class="btn danger" type="button" onclick={deleteAll} disabled={busy}>{t("bulkEdit.deleteAll")}</button>
      </div>
    </section>
  </div>
</aside>

<style>
  /* Bandeau bas en surimpression (§6.3ter) : ancré au bas de `.main-wrap`
     (non-scrollant, voir Library.svelte) — flotte par-dessus la grille sans
     jamais réduire sa largeur, contrairement à l'ancien panneau latéral droit. */
  .panel {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 46%;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--rosso-border);
    background: var(--panel2);
    box-shadow: 0 -10px 28px rgba(0, 0, 0, 0.55);
    z-index: 9;
  }
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
    flex: none;
  }
  .close {
    font-size: 14px;
    padding: 4px 8px;
    margin-left: auto;
  }
  .chips {
    list-style: none;
    display: flex;
    gap: 6px;
    overflow-x: auto;
    font-size: 11px;
    color: var(--txt2);
    flex: 1;
    min-width: 0;
  }
  .chips li {
    padding: 3px 8px;
    background: var(--raised);
    border: 1px solid var(--line);
    white-space: nowrap;
    flex: none;
  }
  .err {
    margin: 10px 16px 0;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
    flex: none;
  }
  .report {
    margin: 10px 16px 0;
    padding: 8px 10px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 11.5px;
    line-height: 1.5;
    flex: none;
  }
  .report.warn {
    background: var(--rosso-dim);
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .fail-list {
    margin-top: 4px;
    padding-left: 16px;
  }
  .sections {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 20px;
    padding: 14px 16px;
  }
  section {
    flex: none;
  }
  h3 {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .opp-hint {
    font-size: 10px;
    color: var(--faint);
    max-width: 220px;
    margin-bottom: 8px;
    line-height: 1.4;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    flex-wrap: wrap;
  }
  .row .input {
    width: 160px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 10px;
    flex: none;
  }
  .btn:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .btn.danger {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .export {
    font-size: 11px;
    padding: 4px 6px;
    color: var(--muted);
  }
  .export:hover {
    color: var(--rosso-bright);
  }
  .export-ok {
    margin-top: 6px;
    padding: 6px 8px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 11px;
  }
  .danger {
    padding-left: 14px;
    border-left: 1px solid var(--line);
  }
</style>
