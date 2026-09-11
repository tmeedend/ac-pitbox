<script lang="ts">
  // Écran d'import dédié (§4.2) : remplace l'ancienne barre de boutons en
  // haut de la bibliothèque — trop discrète pour expliquer les choix, et
  // limitée à Voitures/Circuits. Le glisser-déposer reste le geste rapide,
  // disponible partout dans l'app (voir initGlobalDragDrop dans AppShell).
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import BulkImport from "./BulkImport.svelte";
  import ImportReport from "./ImportReport.svelte";
  import Field from "./Field.svelte";
  import { getConfig, saveConfig, type AppConfig } from "$lib/config";
  import { errorText } from "$lib/errors";
  import {
    importState,
    importSummary,
    setCopyMode,
    pickAndImportArchive,
    pickAndImportFolder,
    reportBulkDone,
  } from "$lib/importState.svelte";
  import { t } from "$lib/i18n/index.svelte";

  // --- Préférences d'import (SPEC §7.2quater) -------------------------------
  //
  // Elles vivaient dans un onglet « Import » des Réglages, à deux noms quasi
  // identiques de cet écran-ci : l'un l'action, l'autre ses préférences. Les
  // préférences d'une opération se consultent à côté de l'opération. Section
  // repliable et non onglet : cet écran est DÉJÀ un onglet de l'Atelier, et
  // des onglets dedans recréeraient le double niveau interdit.
  //
  // Écriture immédiate, sans bouton « Enregistrer » : deux réglages, aucun
  // aperçu live à valider ou à annuler — la garde de navigation des Réglages
  // n'aurait rien à garder ici. Un échec se dit (`prefsError`), il ne se
  // perd pas en silence.
  let config = $state<AppConfig | null>(null);
  let prefsOpen = $state(false);
  let prefsError = $state("");
  onMount(async () => {
    try {
      config = await getConfig();
    } catch (e) {
      prefsError = errorText(e);
    }
  });
  async function persistPrefs() {
    if (!config) return;
    try {
      await saveConfig($state.snapshot(config));
      prefsError = "";
    } catch (e) {
      prefsError = errorText(e);
    }
  }

  let bulkParent = $state<string | null>(null);
  async function pickBulkImport() {
    const sel = await open({ directory: true, multiple: false });
    if (sel && typeof sel === "string") bulkParent = sel;
  }
</script>

<div class="import-screen">
  <!-- Onglet de l'Atelier : le titre d'écran est porté par l'Atelier. -->
  <header class="head">
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

  {#if config}
    <section class="prefs">
      <button class="prefs-h" type="button" aria-expanded={prefsOpen} onclick={() => (prefsOpen = !prefsOpen)}>
        <span class="chev" aria-hidden="true">{prefsOpen ? "▾" : "▸"}</span>{t("import.prefsTitle")}
      </button>
      {#if prefsOpen}
        <div class="prefs-b">
          <Field label={t("settings.resourceExtraction")} hint={t("settings.resourceExtractionHint")}>
            <select class="input" bind:value={config.prefs.resource_extraction_mode} onchange={persistPrefs}>
              <option value="none">{t("settings.resourceExtractionNone")}</option>
              <option value="info_only">{t("settings.resourceExtractionInfo")}</option>
              <option value="all">{t("settings.resourceExtractionAll")}</option>
            </select>
          </Field>
          <Field hint={t("settings.keepSourceArchiveHint")}>
            <label class="check">
              <input type="checkbox" bind:checked={config.prefs.keep_source_archive} onchange={persistPrefs} />
              <span>{t("settings.keepSourceArchive")}</span>
            </label>
          </Field>
          {#if prefsError}<div class="errbox">{prefsError}</div>{/if}
        </div>
      {/if}
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
  /* Repliée par défaut : on vient ici pour importer, pas pour régler. */
  .prefs {
    margin-top: 26px;
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }
  .prefs-h {
    display: flex;
    align-items: center;
    gap: 8px;
    background: none;
    color: var(--muted);
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 2px;
    text-transform: uppercase;
  }
  .prefs-h:hover {
    color: var(--txt2);
  }
  .prefs-h .chev {
    font-size: 9px;
  }
  .prefs-b {
    margin-top: 16px;
    max-width: 420px;
  }
  .prefs-b .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    cursor: pointer;
  }
  .prefs-b .input {
    max-width: 260px;
  }
  .errbox {
    margin-top: 12px;
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
