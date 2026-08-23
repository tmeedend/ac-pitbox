<script lang="ts">
  import { onMount } from "svelte";
  import {
    activateMod,
    deactivateMod,
    getModDetail,
    openModFolder,
    previewSrc,
    setFavorite,
    setManualTags,
    type ModDetail,
    type NativeSpecs,
  } from "$lib/library";
  import PowerCurve from "./PowerCurve.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import { exportMod, deleteBrokenMod, reinstallFromArchive, type ExportReport } from "$lib/maintenance";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  interface Props {
    id: string | null;
    onchange?: () => void;
    onexpand?: () => void;
  }
  let { id, onchange, onexpand }: Props = $props();

  // Ouvre le dossier réel du mod dans l'explorateur Windows.
  async function openFolder() {
    if (!detail) return;
    try {
      await openModFolder(detail.id_interne);
    } catch (e) {
      actionError = errorText(e);
    }
  }

  let detail = $state<ModDetail | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let actionError = $state("");
  let exporting = $state(false);
  let exportResult = $state<ExportReport | null>(null);
  let deleteBusy = $state(false);
  let reinstallBusy = $state(false);
  let reinstallOk = $state(false);

  // Archive/dossier source conservé pour la version active (§10/§11), s'il y
  // en a un — conditionne l'affichage du bouton « Réinstaller ».
  const keptArchive = $derived.by(() => {
    const d = detail;
    if (!d) return null;
    return d.versions.find((v) => v.id === d.active_version_id)?.kept_archive_path ?? null;
  });

  // Actions (activer/désactiver, ouvrir dossier, exporter, réinstaller,
  // supprimer) déplacées du panneau compact (déjà sur la fiche détail) vers
  // un clic droit — évite d'encombrer ce panneau tout en les gardant à
  // portée de main.
  let ctxMenu = $state<{ x: number; y: number } | null>(null);
  function openContextMenu(e: MouseEvent) {
    if (!detail) return;
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY };
  }
  const contextItems = $derived.by(() => {
    const d = detail;
    if (!d) return [];
    const items: { label: string; onclick: () => void; danger?: boolean }[] = [];
    if (onexpand) items.push({ label: t("modpanel.ctxOpenDetail"), onclick: onexpand });
    if (!d.is_stock) {
      items.push({
        label: d.active ? t("common.deactivate") : t("common.activate"),
        onclick: d.active ? deactivate : () => activate(),
      });
    }
    items.push({ label: t("detail.openFolder"), onclick: openFolder });
    if (d.is_stock) return items;
    items.push({ label: t("modpanel.exportFull"), onclick: doExport });
    if (keptArchive) {
      items.push({ label: t("detail.reinstallFromArchive"), onclick: doReinstall });
    }
    items.push({ label: t("detail.deleteFromLibrary"), onclick: doDelete, danger: true });
    return items;
  });

  // Supprimer de la bibliothèque : action distincte de Désactiver (§10) —
  // efface les fichiers de toutes les versions, jamais réversible sans
  // réimport (sauf réinstallation depuis une archive source conservée).
  async function doDelete() {
    if (!detail || busy || deleteBusy) return;
    const ok = await confirm(t("detail.deleteConfirm", { name: detail.display_name ?? detail.id_interne }), {
      title: t("detail.deleteTitle"),
      kind: "warning",
    });
    if (!ok) return;
    deleteBusy = true;
    actionError = "";
    try {
      await deleteBrokenMod(detail.id_interne);
      onchange?.();
      await reload();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      deleteBusy = false;
    }
  }

  async function doReinstall() {
    if (!detail || reinstallBusy) return;
    const ok = await confirm(t("detail.reinstallConfirm", { name: detail.display_name ?? detail.id_interne }), {
      title: t("detail.reinstallConfirmTitle"),
      kind: "warning",
    });
    if (!ok) return;
    reinstallBusy = true;
    actionError = "";
    reinstallOk = false;
    try {
      await reinstallFromArchive(detail.id_interne);
      await reload();
      onchange?.();
      reinstallOk = true;
    } catch (e) {
      actionError = errorText(e);
    } finally {
      reinstallBusy = false;
    }
  }

  async function doExport() {
    if (!detail || exporting) return;
    const dir = await open({ directory: true, multiple: false, title: t("detail.exportDirTitle") });
    if (!dir || typeof dir !== "string") return;
    exporting = true;
    actionError = "";
    exportResult = null;
    try {
      exportResult = await exportMod(detail.id_interne, dir);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      exporting = false;
    }
  }

  async function reload() {
    if (!id) return;
    detail = await getModDetail(id);
  }

  async function activate(versionId?: string) {
    if (!detail || busy) return;
    busy = true;
    actionError = "";
    try {
      await activateMod(detail.id_interne, versionId);
      await reload();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function deactivate() {
    if (!detail || busy) return;
    busy = true;
    actionError = "";
    try {
      await deactivateMod(detail.id_interne);
      await reload();
      onchange?.();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      busy = false;
    }
  }
  // Défaut synchrone (affiché tant que la valeur sauvegardée n'a pas répondu,
  // §6.2) : true, comme l'ancien repli `localStorage`.
  let showRawTags = $state(true);
  let manualInput = $state("");

  onMount(async () => {
    const saved = await getUiPref(StorageKey.showFileTags);
    if (saved != null) showRawTags = saved !== "false";
  });

  function toggleRawTags() {
    showRawTags = !showRawTags;
    setUiPref(StorageKey.showFileTags, String(showRawTags));
  }

  async function toggleFav() {
    if (!detail) return;
    detail.is_favorite = !detail.is_favorite;
    await setFavorite(detail.id_interne, detail.is_favorite);
    onchange?.();
  }

  async function addManual() {
    if (!detail) return;
    const tag = manualInput.trim().toLowerCase();
    manualInput = "";
    if (!tag || detail.tags_manual.includes(tag)) return;
    detail.tags_manual = [...detail.tags_manual, tag];
    await setManualTags(detail.id_interne, detail.tags_manual);
    onchange?.();
  }

  async function removeManual(tag: string) {
    if (!detail) return;
    detail.tags_manual = detail.tags_manual.filter((x) => x !== tag);
    await setManualTags(detail.id_interne, detail.tags_manual);
    onchange?.();
  }

  function mechRows(d: ModDetail): [string, string][] {
    const rows: [string, string][] = [];
    const add = (label: string, v: string | null) => {
      if (v) rows.push([label, v]);
    };
    add(t("columns.drivetrain"), d.drivetrain);
    add(t("columns.aspiration"), d.aspiration);
    add(t("columns.engineConfig"), d.engine_config);
    add(t("columns.enginePos"), d.engine_pos);
    add(t("columns.gearbox"), d.gearbox);
    return rows;
  }

  // Décode une description HTML (br + entités) en texte sûr (pas d'injection).
  function decodeDescription(html: string): string {
    return html
      .replace(/<\/?br\s*\/?>/gi, "\n")
      .replace(/<[^>]+>/g, "")
      .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(+n))
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#39;|&apos;/g, "'")
      .trim();
  }

  function specRows(s: NativeSpecs): [string, string][] {
    const rows: [string, string][] = [];
    const add = (label: string, v: string | null) => {
      if (v) rows.push([label, v]);
    };
    add(t("modpanel.specPower"), s.bhp);
    add(t("modpanel.specTorque"), s.torque);
    add(t("columns.weight"), s.weight);
    add(t("modpanel.specTopSpeed"), s.topspeed);
    add(t("modpanel.specAccel"), s.acceleration);
    add(t("modpanel.specPwRatio"), s.pwratio);
    add(t("modpanel.specRange"), s.range);
    add(t("columns.country"), s.country);
    return rows;
  }

  $effect(() => {
    const current = id;
    actionError = "";
    if (!current) {
      detail = null;
      return;
    }
    loading = true;
    getModDetail(current).then((d) => {
      // Évite d'écraser si la sélection a changé entre-temps.
      if (current === id) {
        detail = d;
        loading = false;
      }
    });
  });

</script>

<!-- Zone à part pour les gâchettes hautes de la manette (§7.4bis) : sans
     elle, rejoindre la fiche depuis la liste demandait de traverser toute la
     liste à la croix directionnelle. -->
<aside class="panel" data-gp-region="detail" oncontextmenu={openContextMenu}>
  {#if onexpand && (detail || (loading && id))}
    <header>
      <button class="btn-ghost expand" type="button" onclick={onexpand} title={t("modpanel.expandTooltip")}>⤢ {t("modpanel.expand")}</button>
    </header>
  {/if}

  {#if !id}
    <div class="empty">{t("modpanel.noSelection")}</div>
  {:else if loading && !detail}
    <div class="empty">{t("common.loading")}</div>
  {:else if detail}
      {@const preview = previewSrc(detail.preview)}
      {@const outline = previewSrc(detail.outline)}
      <div class="preview">
        {#if preview}
          <img src={preview} alt={detail.display_name ?? detail.id_interne} />
        {:else}
          <div class="noprev">{detail.kind === "Track" ? t("library.typeTrack") : t("library.typeCar")}</div>
        {/if}
        {#if detail.kind === "Track" && outline}<img class="outline" src={outline} alt="" />{/if}
      </div>

      <div class="name-row">
        <h2>{detail.display_name ?? detail.id_interne}</h2>
        <button
          class="fav"
          class:on={detail.is_favorite}
          type="button"
          onclick={toggleFav}
          title={detail.is_favorite ? t("modpanel.removeFavorite") : t("modpanel.addFavorite")}
        >
          {detail.is_favorite ? "♥" : "♡"}
        </button>
      </div>
      <div class="sub mono">{detail.id_interne}</div>

      <div class="meta">
        {#if detail.brand}<span><b>{t("columns.brand")}</b> {detail.brand}</span>{/if}
        {#if detail.year}<span><b>{t("columns.year")}</b> {detail.year}</span>{/if}
        {#if detail.author}<span><b>{t("columns.author")}</b> {detail.author}</span>{/if}
        {#if detail.source_pack}<span class="pack"><b>{t("detail.packLabel")}</b> {detail.source_pack}</span>{/if}
      </div>

      {#if actionError}
        <div class="action-err">{actionError}</div>
      {/if}
      {#if exportResult}
        <div class="export-ok">
          {t("detail.exportSuccess", { count: exportResult.included.length })}
          {#if exportResult.warnings.length}
            <ul class="export-warn">
              {#each exportResult.warnings as w}<li>⚠ {w}</li>{/each}
            </ul>
          {/if}
        </div>
      {/if}
      {#if reinstallOk}
        <div class="export-ok">{t("detail.reinstallSuccess")}</div>
      {/if}

      {#if detail.kind === "Car"}
        {@const rows = detail.specs ? specRows(detail.specs) : []}
        {@const mech = mechRows(detail)}
        {#if rows.length || mech.length}
          <section>
            <h3>{t("detail.techSheet")}</h3>
            <div class="specs">
              {#each rows as [label, value]}
                <div class="spec">
                  <span class="s-label">{label}</span>
                  <span class="s-value">{value}</span>
                </div>
              {/each}
              {#each mech as [label, value]}
                <div class="spec derived" title={t("modpanel.derivedTooltip")}>
                  <span class="s-label">{label}</span>
                  <span class="s-value">{value}</span>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      {/if}

      {#if detail.specs}
        {@const s = detail.specs}

        {#if s.power_curve.length > 1}
          <section>
            <h3>{t("modpanel.engineCurve")} <span class="count mono">RPM</span></h3>
            <PowerCurve power={s.power_curve} torque={s.torque_curve} />
          </section>
        {/if}

        {#if s.description}
          <section>
            <h3>{t("common.description")}</h3>
            <p class="description">{decodeDescription(s.description)}</p>
          </section>
        {/if}
      {/if}

      {#if detail.kind === "Track" && detail.track?.description}
        <section>
          <h3>{t("common.description")}</h3>
          <p class="description">{decodeDescription(detail.track.description)}</p>
        </section>
      {/if}

      <section>
        <h3>
          {t("detail.tagsLabel")}
          <span class="legend">
            <span class="lg cat">{t("detail.tagLegendCategory")}</span>
            <span class="lg rule">{t("detail.tagLegendRule")}</span>
            <span class="lg manual">{t("detail.tagLegendManual")}</span>
            <span class="lg file">{t("detail.tagLegendMod")}</span>
          </span>
        </h3>
        <div class="tags">
          {#each detail.tags_from_rule.filter((tag) => tag.startsWith("#")) as tag}
            <span class="tag cat">{tag}</span>
          {/each}
          {#each detail.tags_from_rule.filter((tag) => !tag.startsWith("#")) as tag}
            <span class="tag rule">{tag}</span>
          {/each}
          {#each detail.tags_manual as tag}
            <span class="tag manual">
              {tag}<button class="x" type="button" onclick={() => removeManual(tag)} title={t("common.remove")}>×</button>
            </span>
          {/each}
          {#if showRawTags}
            {#each detail.tags_from_mod as tag}
              <span class="tag file">{tag}</span>
            {/each}
          {/if}
        </div>
        <div class="tag-actions">
          <input
            class="input manual-input"
            placeholder={t("detail.addTagPlaceholder")}
            bind:value={manualInput}
            onkeydown={(e) => e.key === "Enter" && addManual()}
          />
          <label class="raw-toggle">
            <input type="checkbox" checked={showRawTags} onchange={toggleRawTags} />
            <span>{t("detail.rawModTags")}</span>
          </label>
        </div>
      </section>

  {:else}
    <div class="empty">{t("modpanel.notFound")}</div>
  {/if}
  {#if ctxMenu}
    <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={contextItems} onclose={() => (ctxMenu = null)} />
  {/if}
</aside>

<style>
  .panel {
    width: 320px;
    height: 100%;
    /* min-height:0 annule le plancher implicite « min-height:auto » des
       enfants flex (= hauteur de leur contenu) : sans lui, le panneau
       grandissait au-delà de la hauteur disponible au lieu de défiler en
       interne, et le débordement était rogné plus haut sans scrollbar. */
    min-height: 0;
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
    align-items: center;
    padding: 10px 0 4px;
    z-index: 1;
  }
  .expand {
    font-size: 11px;
    padding: 4px 8px;
    color: var(--muted);
  }
  .expand:hover {
    color: var(--rosso-bright);
  }
  .preview {
    position: relative;
    aspect-ratio: 16 / 9;
    background: var(--bg);
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .preview img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .preview img.outline {
    position: absolute;
    inset: 0;
    object-fit: contain;
    padding: 8px;
  }
  .noprev {
    color: var(--faint);
    font-size: 11px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
    margin-top: 12px;
  }
  .sub {
    color: var(--muted2);
    font-size: 11px;
    margin-top: 2px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin-top: 12px;
    font-size: 12px;
    color: var(--txt2);
  }
  .meta b {
    color: var(--muted);
    font-weight: 600;
    margin-right: 4px;
    font-size: 10px;
    text-transform: uppercase;
  }
  .meta .pack {
    flex-basis: 100%;
    color: var(--muted);
    font-family: var(--mono);
    font-size: 11px;
  }
  .action-err {
    margin-top: 10px;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
    line-height: 1.4;
  }
  .export-ok {
    margin-top: 10px;
    padding: 8px 10px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .export-warn {
    list-style: none;
    margin-top: 6px;
    color: var(--yellow);
    font-size: 11px;
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
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--faint);
    font-family: var(--mono);
  }
  .specs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .spec {
    background: var(--panel);
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .s-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .s-value {
    font-size: 12px;
    color: var(--txt);
    font-family: var(--mono);
  }
  .description {
    margin-top: 8px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--txt2);
    white-space: pre-line;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .tag {
    font-size: 11px;
    padding: 2px 7px;
    border: 1px solid var(--line);
  }
  .tag.cat {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .tag.rule {
    color: var(--green);
    border-color: var(--green-border);
  }
  .tag.manual {
    color: var(--txt2);
    border-color: var(--faint);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .tag.file {
    color: var(--blue);
    border-color: var(--blue-border);
  }
  .tag .x {
    background: transparent;
    color: var(--muted);
    font-size: 13px;
    line-height: 1;
    padding: 0;
  }
  .tag .x:hover {
    color: var(--rosso-bright);
  }
  .legend {
    display: inline-flex;
    gap: 8px;
    margin-left: auto;
  }
  .lg {
    font-size: 8.5px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .lg.cat { color: var(--rosso-bright); }
  .lg.rule { color: var(--green); }
  .lg.manual { color: var(--txt2); }
  .lg.file { color: var(--blue); }
  .tag-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    align-items: center;
  }
  .manual-input {
    flex: 1;
    padding: 5px 8px;
    font-size: 11.5px;
  }
  .raw-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    color: var(--txt2);
    white-space: nowrap;
    cursor: pointer;
  }
  .name-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }
  .name-row h2 {
    margin-top: 0;
    flex: 1;
    min-width: 0;
  }
  .fav {
    background: transparent;
    color: var(--muted2);
    font-size: 18px;
    line-height: 1;
    flex: none;
  }
  .fav.on {
    color: var(--rosso-bright);
  }
  .fav:hover {
    color: var(--rosso-bright);
  }
  .spec.derived .s-value {
    color: var(--green);
  }
  .empty {
    color: var(--muted);
    padding: 30px 0;
    text-align: center;
  }
</style>
