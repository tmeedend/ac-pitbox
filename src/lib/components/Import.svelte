<script lang="ts">
  // Écran d'import dédié (§4.2) : remplace l'ancienne barre de boutons en
  // haut de la bibliothèque — trop discrète pour expliquer les choix, et
  // limitée à Voitures/Circuits. Le glisser-déposer reste le geste rapide,
  // disponible partout dans l'app (voir initGlobalDragDrop dans AppShell).
  import { open } from "@tauri-apps/plugin-dialog";
  import BulkImport from "./BulkImport.svelte";
  import ImportReport from "./ImportReport.svelte";
  import {
    importState,
    importSummary,
    setCopyMode,
    pickAndImportArchive,
    pickAndImportFolder,
    reportBulkDone,
  } from "$lib/importState.svelte";
  import { t } from "$lib/i18n/index.svelte";

  let bulkParent = $state<string | null>(null);
  async function pickBulkImport() {
    const sel = await open({ directory: true, multiple: false });
    if (sel && typeof sel === "string") bulkParent = sel;
  }
</script>

<div class="import-screen">
  <header class="head">
    <h2 class="lbl-screen">{t("nav.import")}</h2>
    <p class="sub">
      {t("import.subtitlePrefix")}<b>{t("import.subtitleBold")}</b>{t("import.subtitleSuffix")}
    </p>
  </header>

  <section class="cards">
    <div class="card">
      <h3>{t("import.archiveTitle")}</h3>
      <p class="hint">{t("import.archiveHint")}</p>
      <button class="btn btn-primary" type="button" onclick={pickAndImportArchive} disabled={importState.importing}>
        {importState.importing ? t("import.importing") : t("import.chooseArchive")}
      </button>
    </div>

    <div class="card">
      <h3>{t("import.folderTitle")}</h3>
      <p class="hint">{t("import.folderHint")}</p>
      <div class="copy-choice">
        <span class="cc-label">{t("import.copyChoiceLabel")}</span>
        <div class="copy-toggle">
          <button class:on={importState.copyMode} onclick={() => setCopyMode(true)}>{t("import.copy")}</button>
          <button class:on={!importState.copyMode} onclick={() => setCopyMode(false)}>{t("import.move")}</button>
        </div>
      </div>
      <p class="hint small">
        <b>{t("import.copy")}</b>{t("import.copyHintSuffix")}
        <b>{t("import.move")}</b>{t("import.moveHintSuffix")}
      </p>
      <button class="btn" type="button" onclick={pickAndImportFolder} disabled={importState.importing} title={t("import.folderTooltip")}>
        {importState.importing ? t("import.importing") : t("import.chooseFolder")}
      </button>
    </div>
  </section>

  <section class="mass">
    <h3>{t("import.massTitle")}</h3>
    <p class="hint">
      {t("import.massHintPrefix")}<b>{t("import.massHintBold1")}</b>{t("import.massHintMid")}<b>{t("import.massHintBold2")}</b>{t("import.massHintSuffix")}
    </p>
    <button class="btn" type="button" onclick={pickBulkImport} disabled={importState.importing}>
      {t("import.chooseParentFolder")}
    </button>
  </section>

  <section class="dnd">
    <h3>{t("import.dndTitle")}</h3>
    <p class="hint">{t("import.dndHint")}</p>
  </section>

  <!-- Dernier rapport (§4.2bis) : le toast se ferme d'un clic, souvent par
       réflexe, et un import de quarante mods méritait mieux que de disparaître
       avec lui. -->
  {#if importState.lastReport?.length}
    {@const report = importState.lastReport}
    <section class="last-report">
      <h3>{t("import.lastReportTitle")}</h3>
      <p class="hint">{importSummary(report)}</p>
      <div class="lr-body">
        <ImportReport {report} />
      </div>
    </section>
  {/if}
</div>

{#if bulkParent}
  <BulkImport
    parent={bulkParent}
    copy={importState.copyMode}
    onclose={() => (bulkParent = null)}
    ondone={(r) => {
      reportBulkDone(r);
      bulkParent = null;
    }}
  />
{/if}

<style>
  .import-screen {
    max-width: 760px;
  }
  .head {
    margin-bottom: 22px;
  }
  /* Taille/graisse viennent de `.lbl-screen` (global, §chantier libellés). */
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 560px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 8px;
  }
  .cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 24px;
  }
  .card {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
    line-height: 1.55;
  }
  .hint.small {
    font-size: 11px;
  }
  .hint b {
    color: var(--txt2);
  }
  .copy-choice {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cc-label {
    font-size: 11px;
    color: var(--muted);
  }
  .copy-toggle {
    display: flex;
    border: 1px solid var(--line);
  }
  .copy-toggle button {
    background: var(--panel);
    color: var(--muted);
    font-size: 10.5px;
    padding: 6px 10px;
    border-right: 1px solid var(--line);
  }
  .copy-toggle button:last-child {
    border-right: none;
  }
  .copy-toggle button.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .mass,
  .dnd {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px 16px;
    margin-bottom: 16px;
  }
  .mass .hint,
  .dnd .hint {
    max-width: 620px;
    margin-bottom: 12px;
  }
  .last-report {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px 16px;
    margin-bottom: 16px;
  }
  .last-report .hint {
    margin-bottom: 10px;
  }
  /* Un lot de plusieurs dizaines de mods tient dans une hauteur bornée, sans
     repousser le reste de l'écran hors de vue. */
  .lr-body {
    max-height: 320px;
    overflow-y: auto;
    font-size: 12px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 7px 14px;
    align-self: flex-start;
  }
  .btn.btn-primary {
    background: var(--rosso);
    color: #fff;
    border-color: var(--rosso);
  }
  .btn:disabled {
    opacity: 0.5;
  }
</style>
