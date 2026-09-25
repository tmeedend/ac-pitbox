<script lang="ts">
  // Écran d'import dédié (§4.2) : remplace l'ancienne barre de boutons en
  // haut de la bibliothèque — trop discrète pour expliquer les choix, et
  // limitée à Voitures/Circuits. Le glisser-déposer reste le geste rapide,
  // disponible partout dans l'app (voir initGlobalDragDrop dans shellServices.ts).
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import BulkImport from "./BulkImport.svelte";
  import ImportReport from "./ImportReport.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import { convertCmGrids, scanCmPresets, type CmImportReport, type CmScan } from "$lib/launch/cmImport";
  import { getConfig, saveConfig, type AppConfig } from "$lib/config";
  import { listLibrary } from "$lib/library/library";
  import { addGrids } from "$lib/launch/savedGrids";
  import { errorText } from "$lib/errors";
  import {
    importState,
    importSummary,
    setCopyMode,
    pickAndImportArchive,
    pickAndImportFolder,
    reportBulkDone,
  } from "$lib/workshop/importState.svelte";
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

  // --- Presets Content Manager (§6) ----------------------------------------
  //
  // **Deux moments, un seul mécanisme.** La spec voulait une proposition au
  // premier lancement et un bouton permanent sur cette page. Les deux vivent
  // ici : la carte de proposition n'apparaît que si des presets existent et que
  // l'utilisateur n'a pas refusé — et son refus est définitif —, le bouton reste
  // en dessous quoi qu'il arrive. Une seule surface plutôt que deux à tenir
  // d'accord, et le « premier lancement » devient la première ouverture de la
  // page d'import, qui est de toute façon l'endroit où l'on vient chercher ça.
  let cmScan = $state<CmScan | null>(null);
  let cmReport = $state<CmImportReport | null>(null);
  let cmBusy = $state(false);
  let cmError = $state("");
  const cmOffer = $derived(!!cmScan?.grids.length && !!config && !config.prefs.cm_import_declined && !cmReport);

  /** L'import aboutit toujours (§6.3) : ce qui manque est nommé, jamais fatal. */
  async function importCmGrids() {
    if (!cmScan || cmBusy) return;
    cmBusy = true;
    cmError = "";
    try {
      const cars = (await listLibrary()).filter((c) => c.kind === "Car");
      const { grids, missing, disabled } = convertCmGrids(cmScan.grids, cars);
      const imported = grids.length ? await addGrids(grids) : [];
      cmReport = { imported, missing, disabled, skipped: cmScan.skipped };
    } catch (e) {
      cmError = errorText(e);
    } finally {
      cmBusy = false;
    }
  }

  /** Refus **définitif** : la proposition ne revient pas. Le bouton d'import,
   * lui, reste — ce qu'on refuse est qu'on le redemande, pas la
   * fonctionnalité. */
  async function declineCmOffer() {
    if (!config) return;
    config.prefs.cm_import_declined = true;
    await persistPrefs();
  }

  onMount(async () => {
    try {
      config = await getConfig();
    } catch (e) {
      prefsError = errorText(e);
    }
    // Jamais bloquant : Content Manager peut ne pas être installé, et c'est un
    // non-résultat, pas une panne.
    cmScan = await scanCmPresets().catch(() => null);
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
    <p class="lbl-sub">
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

  <!-- Grilles Content Manager (§6). La carte est la proposition, la section en
       dessous le point d'entrée permanent. -->
  {#if cmOffer}
    <section class="cm-offer">
      <h3>{t("import.cmFoundTitle")}</h3>
      <p class="hint">{t("import.cmFoundHint", { count: cmScan?.grids.length ?? 0 })}</p>
      <div class="cm-actions">
        <button class="btn btn-primary" type="button" disabled={cmBusy} onclick={importCmGrids}>
          {cmBusy ? t("import.importing") : t("import.cmImport")}
        </button>
        <button class="btn" type="button" onclick={() => void declineCmOffer()}>{t("import.cmNotNow")}</button>
      </div>
    </section>
  {/if}

  <section class="cm">
    <h3>{t("import.cmTitle")}</h3>
    <p class="hint">{t("import.cmHint")}</p>
    {#if cmScan?.root == null}
      <p class="hint small">{t("import.cmNotFound")}</p>
    {:else}
      <button class="btn" type="button" disabled={cmBusy || !cmScan.grids.length} onclick={importCmGrids}>
        {cmBusy ? t("import.importing") : t("import.cmImportCount", { count: cmScan.grids.length })}
      </button>
    {/if}
    {#if cmError}<div class="errbox">{cmError}</div>{/if}
    {#if cmReport}
      <!-- Un rapport, pas un échec (§6.3) : ce qui manque est nommé. -->
      <div class="cm-report">
        <p>{t("import.cmImported", { count: cmReport.imported.length })}</p>
        {#if cmReport.disabled.length}
          <p class="warnbox">{t("import.cmDisabled", { count: cmReport.disabled.length })}</p>
        {/if}
        {#if cmReport.missing.length}
          <p class="hint small">{t("import.cmMissing", { count: cmReport.missing.length })}</p>
          <p class="cm-ids mono">{cmReport.missing.join(", ")}</p>
        {/if}
        {#if cmReport.skipped.length}
          <p class="hint small">{t("import.cmSkipped", { count: cmReport.skipped.length })}</p>
          <p class="cm-ids mono">{cmReport.skipped.map((k) => k.name).join(", ")}</p>
        {/if}
      </div>
    {/if}
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
  /* Proposition : la couleur de l'alerte non bloquante, parce que c'est une
     offre qu'on peut refuser définitivement — ni une erreur, ni une action
     retenue pour la session (§7.2ter). */
  .cm-offer {
    border: 1px solid #4a4426;
    background: #1a1708;
    padding: 14px 16px;
    margin-bottom: 18px;
  }
  .cm-actions {
    display: flex;
    gap: 8px;
    margin-top: 11px;
  }
  .cm-report {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 7px;
    font-size: 12px;
    color: var(--txt2);
  }
  /* Les identifiants bruts : ce sont eux qu'on recopie dans une recherche de
     mod, donc ils doivent être sélectionnables et repliables. */
  .cm-ids {
    font-size: 10px;
    color: var(--faint);
    word-break: break-all;
    user-select: text;
  }
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
  .lbl-sub {
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
