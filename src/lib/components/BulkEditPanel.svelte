<script lang="ts">
  // Édition groupée (§6.3bis) : remplace le panneau de détail quand plusieurs
  // mods sont sélectionnés (Ctrl/Alt-clic dans la bibliothèque). N'expose que
  // les champs communs à tout mod — jamais les champs propres à un type
  // (specs voiture, skin piloté, version active), qui restent réservés à la
  // fiche détail d'un seul mod.
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
    onclose: () => void;
    onchange: () => void;
  }
  let { ids, cards, onclose, onchange }: Props = $props();

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
    <button class="btn-ghost close" type="button" onclick={onclose} title={t("bulkEdit.clearTooltip")}>✕</button>
  </header>

  <ul class="chips">
    {#each cards as c (c.id_interne)}
      <li>{c.display_name ?? c.id_interne}</li>
    {/each}
  </ul>

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
    </div>
    <div class="row">
      <button class="btn" type="button" onclick={addTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.addTagToAll")}</button>
      <button class="btn" type="button" onclick={removeTag} disabled={busy || !tagInput.trim()}>{t("bulkEdit.removeTagFromAll")}</button>
    </div>
  </section>

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
</aside>

<style>
  .panel {
    width: 320px;
    flex: none;
    border-left: 1px solid var(--line);
    background: var(--panel2);
    padding: 0 16px 20px;
    overflow-y: auto;
  }
  header {
    position: sticky;
    top: 0;
    background: var(--panel2);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
    padding: 10px 0 4px;
    z-index: 1;
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
  }
  .close {
    font-size: 14px;
    padding: 4px 8px;
  }
  .chips {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 8px;
    max-height: 120px;
    overflow-y: auto;
    font-size: 11.5px;
    color: var(--txt2);
  }
  .chips li {
    padding: 3px 8px;
    background: var(--raised);
    border: 1px solid var(--line);
  }
  .err {
    margin-top: 12px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
  .report {
    margin-top: 12px;
    padding: 8px 10px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 11.5px;
    line-height: 1.5;
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
  section {
    margin-top: 18px;
  }
  h3 {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .row .input {
    flex: 1;
    min-width: 0;
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
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
</style>
