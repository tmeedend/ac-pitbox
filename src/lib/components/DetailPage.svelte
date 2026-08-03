<script lang="ts">
  // Page de détail pleine page (§6.3, maquette pitbox-fiche-B-revisee.html).
  // Riche pour les voitures (héros + specs natives + fiche technique + courbe +
  // description + skins + tags/versions/historique). Panneaux Son et Distance =
  // placeholders « à venir » (lots §12bis et §6.5). Réduite pour les circuits.
  import {
    activateMod,
    deactivateMod,
    getModDetail,
    listLibrary,
    listLayers,
    deleteLayer,
    setLayerActive,
    reorderLayer,
    openModFolder,
    listModResources,
    openModResource,
    previewSrc,
    setFavorite,
    setManualTags,
    type ModCard,
    type ModDetail,
    type ModKind,
    type NativeSpecs,
    type LayerRow,
    type LayoutItem,
    type ResourceFile,
  } from "$lib/library";
  import { listModSkins, openNativeShowroom, type SkinItem } from "$lib/launch";
  import { exportMod, deletePack, deleteBrokenMod, reinstallFromArchive, type ExportReport } from "$lib/maintenance";
  import {
    listSubMods,
    activateSound,
    restoreSound,
    syncTrackSkins,
    listActiveTrackSkins,
    setTrackSkinActive,
    type SubModRow,
  } from "$lib/submods";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import PowerCurve from "./PowerCurve.svelte";
  import { nav, pickSession, requestSection } from "$lib/nav.svelte";
  import { importState } from "$lib/importState.svelte";
  import { historyEventLabel, historyDetails } from "$lib/history";
  import { getPreferredSkin, setPreferredSkin, getPreferredLayout, setPreferredLayout } from "$lib/preferred";
  import { getConfig } from "$lib/config";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  interface Props {
    id: string;
    kind: ModKind;
    onclose: () => void;
    onchange?: () => void;
  }
  let { id, kind, onclose, onchange }: Props = $props();
  const isCar = kind === "Car";

  let detail = $state<ModDetail | null>(null);
  let skins = $state<SkinItem[]>([]);
  let previewSkin = $state(0);
  let previewLayout = $state(0);
  let sounds = $state<SubModRow[]>([]);
  let soundBusy = $state(false);
  const activeSound = $derived(sounds.find((s) => s.is_active) ?? null);
  let trackSkins = $state<SubModRow[]>([]);
  let activeTrackSkins = $state<string[]>([]);
  let trackSkinsLoading = $state(true);
  let trackSkinBusy = $state(false);
  let busy = $state(false);
  let actionError = $state("");
  let manualInput = $state("");
  let exporting = $state(false);
  let exportResult = $state<ExportReport | null>(null);
  // Provenance / pack d'origine (§4.7).
  let siblings = $state<ModCard[]>([]);
  let packBusy = $state(false);
  // Couches / extensions rattachées (§4.4).
  let layerList = $state<LayerRow[]>([]);
  // Fichiers annexes du mod (§4.6, Bloc Ressources) — lus en direct sur disque.
  let resourceList = $state<ResourceFile[]>([]);

  // Image héros : voiture → skin sélectionné ; circuit → preview du layout
  // sélectionné ; sinon preview par défaut du mod.
  const heroImg = $derived.by(() => {
    if (isCar && skins[previewSkin]?.preview) return previewSrc(skins[previewSkin].preview);
    const lay = detail?.track?.layouts[previewLayout];
    if (!isCar && lay?.preview) return previewSrc(lay.preview);
    return previewSrc(detail?.preview ?? null);
  });

  async function filterByPack() {
    if (!detail?.source_pack) return;
    if (await requestSection(detail.kind === "Track" ? "tracks" : "cars")) {
      nav.search = detail.source_pack;
    }
  }

  async function openSibling(c: ModCard) {
    if (await requestSection(c.kind === "Track" ? "tracks" : "cars")) {
      nav.openMod = c.id_interne;
    }
  }

  function activeArchive(d: ModDetail): string | null {
    return d.versions.find((v) => v.id === d.active_version_id)?.source_archive ?? null;
  }

  // Archive/dossier source conservé pour la version active (§10/§11), s'il y
  // en a un — conditionne l'affichage du bouton « Réinstaller ».
  function keptArchive(d: ModDetail): string | null {
    return d.versions.find((v) => v.id === d.active_version_id)?.kept_archive_path ?? null;
  }

  let deleteBusy = $state(false);
  let reinstallBusy = $state(false);
  let reinstallOk = $state(false);

  // Supprimer de la bibliothèque : action distincte de Désactiver (§10) —
  // efface les fichiers de toutes les versions, jamais réversible sans
  // réimport (sauf réinstallation depuis une archive source conservée).
  async function doDelete() {
    if (!detail || deleteBusy) return;
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
      onclose();
    } catch (e) {
      actionError = errorText(e);
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

  let layerBusy = $state(false);

  /** Recharge la fiche + les couches + les ressources (après compositing/
   * import) en préservant le layout sélectionné : activer une couche ajoute
   * souvent des layouts (§4.4). */
  async function refreshEntity() {
    const current = id;
    const [d, ls, rs] = await Promise.all([getModDetail(current), listLayers(current), listModResources(current)]);
    if (current !== id) return;
    layerList = ls;
    resourceList = rs;
    if (d) {
      const prevLayoutId = detail?.track?.layouts[previewLayout]?.id;
      detail = d;
      if (!isCar && d.track) {
        const li = d.track.layouts.findIndex((l) => l.id === prevLayoutId);
        previewLayout = li >= 0 ? li : Math.min(previewLayout, Math.max(0, d.track.layouts.length - 1));
        resyncStaleSessionLayout(current, d.track.layouts);
      }
    }
    if (isCar) {
      const s = await listModSkins(current);
      if (current === id) skins = s;
    } else {
      await loadTrackSkins(current);
    }
  }

  async function removeLayer(layer: LayerRow) {
    const ok = await confirm(t("detail.layerDeleteConfirm", { name: layer.source_archive ?? layer.name }), {
      title: t("detail.layerDeleteTitle"),
      kind: "warning",
    });
    if (!ok) return;
    layerBusy = true;
    actionError = "";
    try {
      await deleteLayer(layer.id);
      await refreshEntity();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      layerBusy = false;
    }
  }

  async function toggleLayer(layer: LayerRow) {
    layerBusy = true;
    actionError = "";
    try {
      await setLayerActive(layer.id, !layer.is_active);
      await refreshEntity();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      layerBusy = false;
    }
  }

  async function moveLayer(layer: LayerRow, direction: "up" | "down") {
    layerBusy = true;
    actionError = "";
    try {
      await reorderLayer(layer.id, direction);
      await refreshEntity();
    } catch (e) {
      actionError = errorText(e);
    } finally {
      layerBusy = false;
    }
  }

  /** Taille lisible (Ko/Mo/Go, base 1024) pour le bloc Ressources (§4.6). */
  function fmtFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} o`;
    const units = ["Ko", "Mo", "Go"];
    let v = bytes;
    let i = -1;
    do {
      v /= 1024;
      i++;
    } while (v >= 1024 && i < units.length - 1);
    return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
  }

  async function openResource(f: ResourceFile) {
    try {
      await openModResource(id, f.rel_path);
    } catch (e) {
      actionError = errorText(e);
    }
  }

  async function uninstallPack() {
    if (!detail?.source_pack || packBusy) return;
    const ok = await confirm(
      t("detail.uninstallConfirm", { pack: detail.source_pack, count: siblings.length + 1 }),
      { title: t("detail.uninstallTitle"), kind: "warning" },
    );
    if (!ok) return;
    packBusy = true;
    actionError = "";
    try {
      await deletePack(detail.source_pack);
      onchange?.();
      onclose();
    } catch (e) {
      actionError = errorText(e);
      packBusy = false;
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

  // Aperçu 3D natif (acShowroom.exe) : lancé en **process indépendant**, par
  // -dessus l'app, avec les réglages vidéo du jeu. C'est l'utilisateur qui
  // ferme le showroom pour revenir à Pit Box. L'intégration de la fenêtre
  // native dans la page a été tentée puis abandonnée (voir showroom.rs).
  let showroomBusy = $state(false);
  // Résolu une fois les skins de la fiche courante chargés (§skin sélectionné) —
  // `openShowroom` l'attend pour ne jamais ouvrir avant de connaître le skin
  // sélectionné (sinon SKIN= part vide → voiture toute blanche au 1er affichage,
  // course entre le chargement de `detail` et celui de `skins`).
  let skinsLoadResolve: (() => void) | null = null;
  let skinsLoadPromise: Promise<void> = Promise.resolve();

  async function openShowroom() {
    if (!detail || showroomBusy) return;
    showroomBusy = true;
    actionError = "";
    try {
      // Attend que le skin sélectionné soit connu (sinon course possible avec
      // le chargement de la fiche → showroom ouvert sans skin, voiture blanche).
      await skinsLoadPromise;
      await openNativeShowroom(detail.id_interne, skins[previewSkin]?.id ?? null);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      showroomBusy = false;
    }
  }

  $effect(() => {
    const current = id;
    actionError = "";
    siblings = [];
    layerList = [];
    resourceList = [];
    previewLayout = 0;
    trackSkinsLoading = true;
    // Couches/extensions rattachées (§4.4) : rangées à part, la base est intacte.
    listLayers(current).then((ls) => {
      if (current === id) layerList = ls;
    });
    // Fichiers annexes (§4.6) : lus en direct sur disque, jamais mémorisés.
    listModResources(current).then((rs) => {
      if (current === id) resourceList = rs;
    });
    getModDetail(current).then((d) => {
      if (current !== id) return;
      detail = d;
      // Autres entités du même pack (§4.7).
      if (d?.source_pack) {
        listLibrary().then((all) => {
          if (current !== id) return;
          siblings = all.filter((c) => c.source_pack === d.source_pack && c.id_interne !== d.id_interne);
        });
      }
      // Circuit : restaure le layout mémorisé pour cette entité.
      if (d && !isCar && d.track) {
        const savedLayout = getPreferredLayout(current);
        const li = d.track.layouts.findIndex((l) => l.id === savedLayout?.id);
        previewLayout = li >= 0 ? li : 0;
        resyncStaleSessionLayout(current, d.track.layouts);
      }
    });
    if (isCar) {
      skinsLoadPromise = new Promise((resolve) => {
        skinsLoadResolve = resolve;
      });
      const savedSkin = getPreferredSkin(current);
      listModSkins(current)
        .then((s) => {
          if (current !== id) return;
          skins = s;
          const pi = s.findIndex((x) => x.id === savedSkin?.id);
          previewSkin = pi >= 0 ? pi : 0;
        })
        .finally(() => skinsLoadResolve?.());
      loadSounds(current);
    } else {
      loadTrackSkins(current);
    }
  });

  async function loadSounds(parent: string) {
    const all = await listSubMods(parent);
    if (parent !== id) return;
    sounds = all.filter((s) => s.sub_type === "SOUND");
  }

  async function loadTrackSkins(parent: string) {
    try {
      // Reconnaît d'abord les skins fournis avec le mod (§4.6bis) — sinon ils
      // n'apparaîtraient pas encore dans le listSubMods qui suit.
      await syncTrackSkins(parent);
      if (parent !== id) return;
      const [all, active] = await Promise.all([listSubMods(parent), listActiveTrackSkins(parent)]);
      if (parent !== id) return;
      trackSkins = all.filter((s) => s.sub_type === "TRACK_SKIN");
      activeTrackSkins = active;
    } finally {
      if (parent === id) trackSkinsLoading = false;
    }
  }

  async function toggleTrackSkin(name: string) {
    if (trackSkinBusy) return;
    trackSkinBusy = true;
    const wasActive = activeTrackSkins.includes(name);
    try {
      await setTrackSkinActive(id, name, !wasActive);
      activeTrackSkins = wasActive ? activeTrackSkins.filter((n) => n !== name) : [...activeTrackSkins, name];
    } catch (e) {
      actionError = errorText(e);
    } finally {
      trackSkinBusy = false;
    }
  }

  // Un import peut survenir depuis n'importe quel écran (§4.6bis) et cibler le
  // mod ouvert (ex. une extension). Dès qu'un import se termine, recharger la
  // fiche pour voir tout de suite la nouvelle couche + ses layouts.
  let lastImportVersion = importState.version;
  $effect(() => {
    const v = importState.version;
    if (v === lastImportVersion) return;
    lastImportVersion = v;
    // Différé hors du suivi réactif : ne dépend que de importState.version,
    // pas de `id` (évite un double rechargement à la navigation).
    queueMicrotask(() => void refreshEntity());
  });

  // Son = bascule exclusive (§12bis.2) : un seul actif, original restaurable.
  async function pickSound(subId: string | null) {
    if (!detail || soundBusy) return;
    soundBusy = true;
    actionError = "";
    try {
      if (subId) await activateSound(subId);
      else await restoreSound(detail.id_interne);
      await loadSounds(detail.id_interne);
    } catch (e) {
      actionError = errorText(e);
    } finally {
      soundBusy = false;
    }
  }

  async function reload() {
    detail = await getModDetail(id);
  }

  // Sélectionner un skin (§8.6/§12bis.2) : mémorisé par voiture ET poussé dans
  // le duo de session (visible dans le menu). Remplace l'ancienne « étoile ».
  function selectSkin(i: number) {
    previewSkin = i;
    if (!detail) return;
    const sk = skins[i];
    if (sk) setPreferredSkin(detail.id_interne, sk);
    const meta = [detail.brand, sk ? `skin: ${sk.name}` : null].filter(Boolean).join(" · ");
    pickSession("Car", {
      id: detail.id_interne,
      name: detail.display_name ?? detail.id_interne,
      meta,
      preview: sk?.preview ?? detail.preview,
      layout: null,
      skin: sk?.id ?? null,
      outline: null,
    });
    // Un showroom déjà ouvert garde le skin avec lequel il a été lancé : il
    // n'a pas de mécanisme connu pour en changer à chaud (pas d'IPC vers
    // acShowroom.exe, voir docs/showroom-3d-preview-research.md). Le prochain
    // clic sur « Aperçu 3D » prendra le nouveau skin.
  }

  // Sélectionner un layout de circuit : mémorisé + poussé dans le duo de session
  // (photo + tracé en surimpression dans le menu).
  /** Si le layout mémorisé comme choix de session (§8.6) pour cette entité a
   * disparu (couche retirée/réordonnée, §4.4) alors qu'il s'agit bien de
   * l'entité de la fiche courante, le resynchronise — sinon un layout fantôme
   * reste affiché dans la barre latérale et proposé au lancement, alors qu'il
   * n'existe plus sur le disque. Suppose `detail` déjà à jour à l'appel. */
  function resyncStaleSessionLayout(trackId: string, layouts: LayoutItem[]): void {
    if (nav.sessionTrack?.id !== trackId || !nav.sessionTrack.layout) return;
    if (layouts.some((l) => l.id === nav.sessionTrack?.layout)) return;
    selectLayout(previewLayout);
  }

  function selectLayout(i: number) {
    previewLayout = i;
    if (!detail?.track) return;
    const l = detail.track.layouts[i];
    if (l) setPreferredLayout(detail.id_interne, l);
    const meta = [l?.name, detail.author].filter(Boolean).join(" · ");
    pickSession("Track", {
      id: detail.id_interne,
      name: detail.display_name ?? detail.id_interne,
      meta,
      preview: l?.preview ?? detail.preview,
      layout: l?.id ?? null,
      skin: null,
      outline: l?.outline ?? detail.outline,
    });
  }

  // Ouvre le dossier réel du mod dans l'explorateur Windows (voir aussi
  // ce qu'il y a dedans, en dehors de la fiche). Fonctionne aussi pour le
  // contenu de base Kunos (lecture seule).
  async function openFolder() {
    if (!detail) return;
    try {
      await openModFolder(detail.id_interne);
    } catch (e) {
      actionError = errorText(e);
    }
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

  function initials(brand: string | null, id: string): string {
    const src = (brand ?? id).replace(/[^a-zA-Z]/g, "");
    return (src.slice(0, 2) || "??").toUpperCase();
  }

  // Bandeau de specs natives en surimpression du héros (§6.3).
  function heroSpecs(s: NativeSpecs | null): string {
    if (!s) return "";
    return [s.bhp, s.torque, s.weight, s.topspeed].filter((x): x is string => !!x).join(" · ");
  }

  const DASH = "—";

  function posLabel(pos: string): string {
    if (pos === "FRONT") return t("detail.posFront");
    if (pos === "MID") return t("detail.posMid");
    if (pos === "REAR") return t("detail.posRear");
    return pos;
  }

  // Fiche technique (champs structurés) — abréviations façon maquette.
  function ficheRows(d: ModDetail): [string, string][] {
    const engine = [d.engine_config, d.engine_pos ? posLabel(d.engine_pos) : null]
      .filter(Boolean)
      .join(" · ");
    return [
      [t("detail.specEngine"), engine || DASH],
      [t("detail.specAspiration"), d.aspiration ?? DASH],
      [t("detail.specDrivetrain"), d.drivetrain ?? DASH],
      [t("detail.specGearbox"), d.gearbox ?? DASH],
      [t("detail.specCountry"), d.country ?? DASH],
      [t("detail.specPowerWeight"), d.specs?.pwratio ?? DASH],
    ];
  }

  function fmtDate(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso.slice(0, 16).replace("T", " ") : d.toLocaleString();
  }
</script>

<div class="page">
  {#if !detail}
    <div class="empty">{t("common.loading")}</div>
  {:else}
    {@const d = detail}
    <header class="head">
      <button class="back" type="button" onclick={onclose} title={t("detail.backTooltip")}>←</button>
      {#if isCar && d.badge}
        <img class="escu badge-img" src={previewSrc(d.badge)} alt={d.brand ?? ""} />
      {:else}
        <span class="escu">{initials(d.brand, d.id_interne)}</span>
      {/if}
      <div class="title">
        <div class="t-name">{d.display_name ?? d.id_interne}</div>
        <div class="t-meta mono">
          {d.brand ?? ""}{d.year ? ` · ${d.year}` : ""}
          {#if d.category}· <span class="cat">{d.category}</span>{/if}
          {#if d.car_class}· {d.car_class.toUpperCase()}{/if}
        </div>
      </div>
      <div class="actions">
        <button class="fav" class:on={d.is_favorite} type="button" onclick={toggleFav} title={t("common.favorite")}>
          {d.is_favorite ? "♥" : "♡"}
        </button>
        {#if d.is_stock}
          <span class="base-tag" title={t("detail.stockTooltip")}>{t("detail.stockLabel")}</span>
        {:else if d.active}
          <button class="btn" type="button" onclick={deactivate} disabled={busy}>{t("common.deactivate")}</button>
        {:else}
          <button class="btn" type="button" onclick={() => activate()} disabled={busy}>{t("common.activate")}</button>
        {/if}
        {#if !d.is_stock}
          <button class="btn" type="button" onclick={doExport} disabled={exporting} title={t("detail.exportTooltip")}>
            {exporting ? t("detail.exporting") : t("detail.export")}
          </button>
          {#if keptArchive(d)}
            <button class="btn" type="button" onclick={doReinstall} disabled={reinstallBusy} title={t("detail.reinstallTooltip")}>
              {reinstallBusy ? t("detail.reinstalling") : t("detail.reinstallFromArchive")}
            </button>
          {/if}
          <button class="btn danger" type="button" onclick={doDelete} disabled={deleteBusy} title={t("detail.deleteFromLibraryTooltip")}>
            {deleteBusy ? t("common.working") : t("detail.deleteFromLibrary")}
          </button>
        {/if}
        {#if isCar}
          <button class="btn" type="button" onclick={openShowroom} disabled={showroomBusy} title={t("detail.showroomTooltip")}>
            {showroomBusy ? t("detail.showroomLaunching") : t("detail.showroom")}
          </button>
        {/if}
        <button class="btn" type="button" onclick={openFolder} title={t("detail.openFolderTooltip")}>{t("detail.openFolder")}</button>
      </div>
    </header>

    {#if actionError}<div class="action-err">{actionError}</div>{/if}
    {#if reinstallOk}<div class="export-ok">{t("detail.reinstallSuccess")}</div>{/if}
    {#if exportResult}
      <div class="export-ok">
        {t("detail.exportSuccess", { count: exportResult.included.length })}
        {#if exportResult.warnings.length}
          <ul class="export-warn">{#each exportResult.warnings as w}<li>⚠ {w}</li>{/each}</ul>
        {/if}
      </div>
    {/if}

    <!-- RANGÉE HAUTE : héros + panneau données -->
    <div class="row top" class:track={!isCar}>
      <div class="hero">
        {#if heroImg}
          <img src={heroImg} alt={d.display_name ?? d.id_interne} />
        {:else}
          <div class="hero-icon">{isCar ? "🚗" : "🏁"}</div>
        {/if}
        {#if showroomBusy}
          <!-- Lancement d'acShowroom : pastille discrète le temps que le
               process démarre, il s'affichera ensuite par-dessus l'app. -->
          <div class="hero-loading" title={t("detail.showroomLoading")}>
            <span class="spinner"></span>
          </div>
        {/if}
        {#if !isCar}
          {@const ol = previewSrc(d.track?.layouts[previewLayout]?.outline ?? null)}
          {#if ol}<img class="hero-outline" src={ol} alt="" />{/if}
        {/if}
        {#if isCar}
          {@const hs = heroSpecs(d.specs)}
          {#if hs}
            <div class="hero-specs">
              <div class="mono hs-line">{hs}</div>
              <div class="mono hs-label">{t("detail.specNative")}</div>
            </div>
          {/if}
        {/if}
      </div>

      <div class="data">
        {#if isCar}
          {@const hasCurve = !!d.specs && d.specs.power_curve.length > 1}
          <div class="tech-curve" class:with-curve={hasCurve}>
            <div class="box fiche">
              <div class="box-h">{t("detail.techSheet")}</div>
              <div class="specgrid">
                {#each ficheRows(d) as [k, v]}
                  <div><div class="k">{k}</div><div class="v">{v}</div></div>
                {/each}
              </div>
            </div>
            {#if hasCurve && d.specs}
              <div class="curve-col">
                <div class="lbl">
                  {t("detail.curve")}
                  <span class="legend"><span class="lg-pow">— bhp</span><span class="lg-tor">— Nm</span></span>
                </div>
                <div class="curve-box">
                  <PowerCurve power={d.specs.power_curve} torque={d.specs.torque_curve} />
                </div>
              </div>
            {/if}
          </div>

          {#if d.specs?.description}
            <div class="box-h">{t("common.description")}</div>
            <div class="desc-body">{decodeDescription(d.specs.description)}</div>
          {/if}
        {:else}
          {@const lay = d.track?.layouts[previewLayout]}
          <div class="box">
            <div class="box-h">{t("detail.trackInfo")}</div>
            <div class="specgrid" style="grid-template-columns:1fr 1fr;">
              <div><div class="k">{t("detail.layoutLabel")}</div><div class="v">{lay?.name ?? t("detail.defaultLayout")}</div></div>
              <div><div class="k">{t("detail.lengthLabel")}</div><div class="v">{lay?.length ?? "—"}</div></div>
            </div>
          </div>
          {#if d.csp_features.length}
            <div class="lbl">{t("columns.csp")}</div>
            <div class="csp-row">{#each d.csp_features as f}<span class="csp">{f}</span>{/each}</div>
          {/if}
          {#if d.track?.description}
            <div class="box-h" style="margin-top:11px;">{t("common.description")}</div>
            <div class="desc-body">{decodeDescription(d.track.description)}</div>
          {/if}
        {/if}
      </div>
    </div>

    <!-- RANGÉE BASSE -->
    <div class="row bottom" class:track={!isCar}>
      {#if isCar}
        <!-- Skins : le skin sélectionné devient le skin de session (§8.6), mémorisé -->
        <div class="col">
          <div class="lbl">
            {t("detail.skinsLabel")} <span class="lbl-sub">{t("detail.skinsHint", { count: skins.length })}</span>
          </div>
          {#if skins.length}
            <div class="skins">
              {#each skins as sk, i (sk.id)}
                {@const sp = previewSrc(sk.preview)}
                <button
                  class="skin"
                  class:preview={i === previewSkin}
                  onclick={() => selectSkin(i)}
                  title={t("detail.chooseSkinTooltip")}
                >
                  <div class="skin-img">
                    {#if sp}<img src={sp} alt={sk.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if i === previewSkin}<span class="skin-apercu mono">{t("library.sessionBadge")}</span>{/if}
                  </div>
                  <div class="skin-b">
                    <span class="skin-name">{sk.name}</span>
                  </div>
                </button>
              {/each}
            </div>
          {:else}
            <div class="muted small">{t("detail.noSkins")}</div>
          {/if}
        </div>

        <!-- Distance + Son : placeholders « à venir » désactivés -->
        <div class="col">
          <div class="lbl">{t("detail.distanceLabel")}</div>
          <div class="box">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? t("detail.triedYes") : t("detail.triedNo")}</span>
            </div>
          </div>
          <div class="lbl" style="margin-top:14px;">{t("detail.engineSound")} <span class="lbl-sub">{t("detail.soundHint")}</span></div>
          <div class="sounds">
            <button class="sound" class:sel={!activeSound} type="button" onclick={() => pickSound(null)} disabled={soundBusy}>
              <span class="radio"></span>
              <span class="s-name">{t("detail.soundOrigin")}</span>
              <span class="s-tag mono">{t("library.baseBadge")}</span>
            </button>
            {#each sounds as snd (snd.id)}
              <button class="sound" class:sel={snd.is_active} type="button" onclick={() => pickSound(snd.id)} disabled={soundBusy}>
                <span class="radio"></span>
                <span class="s-name">{snd.name}</span>
                <span class="s-tag mono">{t("detail.modTag")}</span>
              </button>
            {/each}
          </div>
          {#if sounds.length === 0}
            <div class="muted small" style="margin-top:6px;">{t("detail.noSounds")}</div>
          {:else}
            <div class="restore-note">↺ {t("detail.soundRestorable")}</div>
          {/if}

          {@render tagsBlock(d)}
        </div>

        <!-- Versions + Historique + Provenance -->
        <div class="col">
          {@render versionsBlock(d)}
          {@render historyBlock(d)}
          {@render publishedBlock(d)}
          {@render provenanceBlock(d)}
          {@render layersBlock()}
          {@render resourcesBlock()}
        </div>
      {:else}
        <!-- Layouts (galerie illustrée par le tracé, comme les skins voiture) -->
        <div class="col">
          <div class="lbl">
            {t("columns.layouts")} <span class="lbl-sub">{t("detail.layoutsHint", { count: d.track?.layouts.length ?? 0 })}</span>
          </div>
          {#if d.track && d.track.layouts.length}
            <div class="skins">
              {#each d.track.layouts as l, i (l.id || i)}
                {@const o = previewSrc(l.outline)}
                <button class="skin" class:preview={i === previewLayout} onclick={() => selectLayout(i)} title={t("detail.chooseLayoutTooltip")}>
                  <div class="skin-img layout-img">
                    {#if o}<img src={o} alt={l.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if i === previewLayout}<span class="skin-apercu mono">{t("library.sessionBadge")}</span>{/if}
                  </div>
                  <div class="skin-b"><span class="skin-name">{l.name}</span></div>
                </button>
              {/each}
            </div>
          {:else}
            <div class="muted small">{t("detail.singleLayout")}</div>
          {/if}

          <!-- Skins de circuit (TRACK_SKIN) — activables individuellement, plusieurs
               à la fois (§4.6bis, pas de notion d'exclusivité côté CSP). -->
          <div class="lbl section">
            {trackSkinsLoading ? t("detail.trackSkinsLabelPlain") : t("detail.trackSkinsLabel", { count: trackSkins.length })}
          </div>
          {#if trackSkinsLoading}
            <div class="muted small loading-inline"><span class="spinner-sm"></span>{t("common.loading")}</div>
          {:else if trackSkins.length}
            <ul class="tsk-list">
              {#each trackSkins as s (s.id)}
                {@const active = activeTrackSkins.includes(s.name)}
                <li class:inactive={!active}>
                  <label class="tog" title={active ? t("detail.trackSkinActiveOn") : t("detail.trackSkinActiveOff")}>
                    <input
                      type="checkbox"
                      checked={active}
                      disabled={trackSkinBusy}
                      onchange={() => toggleTrackSkin(s.name)}
                    />
                  </label>
                  <span class="tsk-name">{s.name}</span>
                  {#if s.source_archive}<span class="tsk-src mono">{s.source_archive}</span>{/if}
                </li>
              {/each}
            </ul>
            <div class="muted small">{t("detail.trackSkinsNote")}</div>
          {:else}
            <div class="muted small">{t("detail.noTrackSkins")}</div>
          {/if}
        </div>

        <!-- Distance + Auteur + Tags -->
        <div class="col">
          <div class="lbl">{t("detail.distanceLabel")}</div>
          <div class="box">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? t("detail.triedYes") : t("detail.triedNo")}</span>
            </div>
          </div>
          <div class="lbl">{t("detail.authorLabel")}</div>
          <div class="box">{d.author ?? "—"}</div>
          {@render tagsBlock(d)}
        </div>

        <!-- Versions + Historique + Provenance -->
        <div class="col">
          {@render versionsBlock(d)}
          {@render historyBlock(d)}
          {@render publishedBlock(d)}
          {@render provenanceBlock(d)}
          {@render layersBlock()}
          {@render resourcesBlock()}
        </div>
      {/if}
    </div>
  {/if}
</div>

{#snippet tagsBlock(d: ModDetail)}
  <div class="lbl">{t("detail.tagsLabel")}</div>
  <div class="tags">
    {#each d.tags_from_rule.filter((tag) => tag.startsWith("#")) as tag}<span class="tag cat">{tag}</span>{/each}
    {#each d.tags_from_rule.filter((tag) => !tag.startsWith("#")) as tag}<span class="tag rule">{tag}</span>{/each}
    {#each d.tags_manual as tag}
      <span class="tag manual">{tag}<button class="x" type="button" onclick={() => removeManual(tag)} title={t("common.remove")}>×</button></span>
    {/each}
    {#each d.tags_from_mod as tag}<span class="tag mod">{tag}</span>{/each}
  </div>
  <input
    class="input manual-input"
    placeholder={t("detail.addTagPlaceholder")}
    bind:value={manualInput}
    onkeydown={(e) => e.key === "Enter" && addManual()}
  />
{/snippet}

{#snippet versionsBlock(d: ModDetail)}
  <div class="lbl section">{t("detail.versionsLabel", { count: d.versions.length })}</div>
  {#each d.versions as v}
    <div class="ver" class:active={v.id === d.active_version_id}>
      <span class="v-label mono">{v.version_label ?? t("detail.noVersionNumber")}</span>
      {#if v.id === d.active_version_id}
        <span class="tag cat tiny">{t("common.active").toUpperCase()}</span>
      {:else}
        <button class="v-activate" type="button" onclick={() => activate(v.id)} disabled={busy}>{t("common.activate")}</button>
      {/if}
      <span class="v-meta mono">{fmtDate(v.imported_at)}</span>
    </div>
  {/each}
{/snippet}

{#snippet historyBlock(d: ModDetail)}
  <div class="lbl section">{t("detail.historyLabel")}</div>
  <ul class="history">
    {#each d.history.filter((h) => h.event !== "ACTIVATE" && h.event !== "DEACTIVATE") as h}
      <li>
        <span class="ev">{historyEventLabel(h.event)}</span>
        <span class="det">{historyDetails(h.details)}</span>
        <span class="ts mono">{fmtDate(h.timestamp)}</span>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet publishedBlock(d: ModDetail)}
  <div class="lbl section">{t("detail.publishedLabel")}</div>
  <div class="srcbox">
    <div class="srcrow">
      <span class="src-k">{t("detail.estimated")}</span>
      <span class="src-v">{d.published_at ? fmtDate(d.published_at) : "—"}</span>
    </div>
  </div>
{/snippet}

{#snippet provenanceBlock(d: ModDetail)}
  {@const archive = activeArchive(d)}
  {#if d.source_pack || archive || d.source_url}
    <div class="lbl section">{t("detail.sourceLabel")}</div>
    <div class="srcbox">
      <div class="src-h">{t("detail.provenanceTitle")}</div>
      {#if d.source_pack}
        <div class="srcrow">
          <span class="src-k">{t("detail.packLabel")}</span>
          <button class="chip" type="button" onclick={filterByPack} title={t("detail.viewPackTooltip")}>
            ⬢ {d.source_pack} <span class="chip-n">· {t("detail.modCount", { count: siblings.length + 1 })}</span>
          </button>
        </div>
      {/if}
      <div class="srcrow">
        <span class="src-k">{t("detail.archiveLabel")}</span>
        <span class="src-v">{archive ?? "—"}</span>
      </div>
      <div class="srcrow">
        <span class="src-k">{t("detail.originUrlLabel")}</span>
        {#if d.source_url}
          <span class="src-v url">{d.source_url}</span>
        {:else}
          <span class="src-empty">{t("detail.noUrl")}</span>
        {/if}
      </div>
    </div>

    {#if d.source_pack}
      <div class="lbl section">{t("detail.siblingsLabel", { count: siblings.length })}</div>
      {#if siblings.length}
        <div class="siblings">
          {#each siblings as c (c.id_interne)}
            <button class="sib" type="button" onclick={() => openSibling(c)} title={t("detail.openSheetTooltip")}>
              <span class="sib-dot">{c.kind === "Track" ? "🏁" : "🚗"}</span>
              <span class="sib-nm">{c.display_name ?? c.id_interne}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="muted small">{t("detail.onlyEntity")}</div>
      {/if}
      <div class="prov-note">{t("detail.packNote")}</div>
      <div class="prov-actions">
        <button class="btn" type="button" onclick={filterByPack}>⌕ {t("detail.filterByPack")}</button>
        <button class="btn danger" type="button" onclick={uninstallPack} disabled={packBusy}>
          {packBusy ? t("common.working") : `🗑 ${t("detail.uninstallPack")}`}
        </button>
      </div>
    {/if}
  {/if}
{/snippet}

{#snippet layersBlock()}
  {#if layerList.length}
    {@const ordered = [...layerList].reverse()}
    <div class="lbl section">{t("detail.layersLabel", { count: layerList.length })}</div>
    <div class="prov-note">{t("detail.layersNote")}</div>
    <ul class="layer-list">
      {#each ordered as l, i (l.id)}
        <li class="layer-row" class:inactive={!l.is_active}>
          <label class="layer-tog" title={l.is_active ? t("detail.layerActiveOn") : t("detail.layerActiveOff")}>
            <input type="checkbox" checked={l.is_active} disabled={layerBusy} onchange={() => toggleLayer(l)} />
          </label>
          <div class="layer-main">
            <span class="layer-nm">{l.source_archive ?? l.name}</span>
            <span class="layer-counts mono">{t("detail.layerCounts", { added: l.added_count, overwritten: l.overwritten_count })}</span>
          </div>
          <div class="layer-ord">
            <button class="layer-arrow" type="button" title={t("detail.layerUp")} disabled={layerBusy || i === 0} onclick={() => moveLayer(l, "up")}>▲</button>
            <button class="layer-arrow" type="button" title={t("detail.layerDown")} disabled={layerBusy || i === ordered.length - 1} onclick={() => moveLayer(l, "down")}>▼</button>
          </div>
          <button class="layer-x" type="button" title={t("detail.layerDeleteTitle")} disabled={layerBusy} onclick={() => removeLayer(l)}>✕</button>
        </li>
      {/each}
    </ul>
    <div class="prov-note">{t("detail.layersRecomposeNote")}</div>
  {/if}
{/snippet}

{#snippet resourcesBlock()}
  <div class="lbl section">{t("detail.resourcesLabel", { count: resourceList.length })}</div>
  {#if resourceList.length}
    <div class="prov-note">{t("detail.resourcesNote")}</div>
    <ul class="res-list">
      {#each resourceList as f (f.rel_path)}
        <li>
          <button class="res-row" type="button" onclick={() => openResource(f)} title={t("detail.resourceOpenTooltip")}>
            <span class="res-nm">{f.rel_path}</span>
            <span class="res-size mono">{fmtFileSize(f.size_bytes)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="muted small">{t("detail.noResources")}</div>
  {/if}
{/snippet}

<style>
  .page {
    margin: -28px -32px;
    min-height: 100%;
    background: var(--panel);
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 80px 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
  }
  .back {
    background: transparent;
    color: var(--muted);
    font-size: 18px;
    line-height: 1;
    padding: 2px 8px;
  }
  .back:hover {
    color: var(--txt);
  }
  .escu {
    width: 30px;
    height: 30px;
    background: var(--rosso);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-family: var(--mono);
    font-weight: 600;
    font-size: 11px;
    flex: none;
  }
  .escu.badge-img {
    background: var(--panel2);
    border: 1px solid var(--line);
    object-fit: contain;
    padding: 3px;
  }
  .title {
    min-width: 0;
  }
  .t-name {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.1;
  }
  .t-meta {
    color: var(--muted);
    font-size: 10px;
    margin-top: 2px;
  }
  .t-meta .cat {
    color: var(--rosso-bright);
  }
  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fav {
    background: transparent;
    color: var(--muted2);
    font-size: 18px;
    line-height: 1;
  }
  .fav.on {
    color: var(--rosso-bright);
  }
  .base-tag {
    color: var(--blue);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .action-err {
    margin: 10px 18px 0;
    padding: 8px 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 11.5px;
  }
  .export-ok {
    margin: 10px 18px 0;
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

  .row {
    display: grid;
    gap: 1px;
    background: var(--line);
  }
  .row.top {
    grid-template-columns: 1.4fr 1fr;
    border-bottom: 1px solid var(--line);
  }
  .row.bottom {
    grid-template-columns: 1.3fr 1fr 1fr;
  }
  .row.track {
    grid-template-columns: 1fr 1fr;
  }
  .row.bottom.track {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .hero {
    background: linear-gradient(135deg, #2a0a0a, var(--panel) 72%);
    min-height: 300px;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
  }
  /* Voiture : cadre à ratio fixe 16:9 (celui des previews AC), aligné en haut
     — pas étiré par la hauteur du panneau de données voisin. */
  .row.top:not(.track) .hero {
    aspect-ratio: 16 / 9;
    min-height: 0;
    align-self: start;
  }
  .hero img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Pastille de lancement de l'aperçu 3D : petite, en haut à droite, sans
     assombrir l'image — le showroom s'ouvrira par-dessus l'app. */
  .hero-loading {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    background: rgba(8, 8, 12, 0.6);
    border: 1px solid var(--line);
    z-index: 3;
  }
  .hero-loading .spinner {
    width: 15px;
    height: 15px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: hero-spin 0.8s linear infinite;
  }
  @keyframes hero-spin {
    to {
      transform: rotate(360deg);
    }
  }
  /* Tracé du layout superposé à la photo du circuit (§6.1). */
  .hero img.hero-outline {
    position: absolute;
    inset: 0;
    object-fit: contain;
    padding: 24px;
  }
  .hero-icon {
    font-size: 90px;
    opacity: 0.5;
  }
  .hero-specs {
    position: absolute;
    left: 16px;
    bottom: 14px;
  }
  .hs-line {
    color: #e8e8ea;
    font-size: 13px;
  }
  .hs-label {
    color: var(--muted);
    font-size: 8px;
    margin-top: 3px;
  }
  .data {
    background: var(--panel);
    padding: 14px;
  }
  .box {
    border: 1px solid var(--line);
    margin-bottom: 12px;
  }
  /* Fiche technique + courbe carrée côte à côte (§5bis.1). */
  .tech-curve {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: flex-start;
    margin-bottom: 12px;
  }
  .tech-curve .fiche {
    flex: 1 1 200px;
    min-width: 0;
    margin-bottom: 0;
  }
  .tech-curve.with-curve .specgrid {
    grid-template-columns: 1fr 1fr;
  }
  .curve-col {
    flex: 1 1 200px;
    max-width: 260px;
    min-width: 0;
  }
  .curve-col .lbl {
    margin-bottom: 6px;
  }
  .box-h {
    background: var(--raised);
    padding: 5px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 9px;
    letter-spacing: 1.5px;
    display: flex;
    align-items: center;
    width: 100%;
    text-align: left;
  }
  .specgrid {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    background: var(--line);
    gap: 1px;
  }
  .specgrid > div {
    background: var(--panel2);
    padding: 7px 10px;
  }
  .specgrid .k {
    color: var(--faint);
    font-size: 8px;
    letter-spacing: 1px;
    margin-bottom: 3px;
  }
  .specgrid .v {
    color: var(--txt2);
    font-size: 11px;
    font-family: var(--mono);
  }
  .lbl {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1.5px;
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    text-transform: uppercase;
  }
  .lbl.section {
    margin-top: 14px;
  }
  .lbl-sub {
    color: var(--muted);
    text-transform: none;
    letter-spacing: 0;
    margin-left: 6px;
    font-size: 9px;
  }
  .legend {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
  .lg-pow {
    color: var(--rosso-bright);
  }
  .lg-tor {
    color: var(--yellow);
  }
  .curve-box {
    border: 1px solid var(--line);
    padding: 8px;
    margin-bottom: 0;
  }
  .desc-body {
    border: 1px solid var(--line);
    border-top: none;
    background: var(--panel2);
    padding: 9px;
    color: var(--txt2);
    font-size: 11px;
    line-height: 1.55;
    white-space: pre-line;
  }
  .csp-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .csp {
    font-size: 10px;
    color: var(--green);
    border: 1px solid var(--green-border);
    padding: 2px 8px;
  }

  .col {
    background: var(--panel);
    padding: 14px;
  }

  .skins {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .skin {
    background: var(--card);
    padding: 0;
    text-align: left;
    cursor: pointer;
    position: relative;
  }
  /* Cadre du choix de session en calque par-dessus la vignette : un `outline`
     inset était peint avant les descendants positionnés (.skin-img), donc
     masqué par le tracé/la preview qui remplit la cellule. */
  .skin.preview::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 2px solid var(--rosso);
    pointer-events: none;
    z-index: 2;
  }
  .skin-img {
    /* Ratio des previews AC (~16:9) : la hauteur suit la largeur de la cellule,
       au lieu d'une hauteur fixe qui rognait la voiture. */
    aspect-ratio: 16 / 9;
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid var(--line);
    position: relative;
    overflow: hidden;
    background: var(--bg);
  }
  .skin-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* Tracé de layout : afficher la forme complète (pas de recadrage). */
  .layout-img img {
    object-fit: contain;
    padding: 4px;
  }
  .skin-noimg {
    color: var(--faint);
    font-size: 16px;
  }
  .skin-apercu {
    position: absolute;
    bottom: 3px;
    left: 3px;
    background: var(--rosso);
    color: #fff;
    font-size: 7px;
    padding: 0 3px;
  }
  .skin-b {
    padding: 5px 7px;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .skin-name {
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  .dist {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
  }
  .dist-ic {
    font-size: 14px;
    opacity: 0.8;
  }
  .dist-km {
    font-size: 13px;
    font-weight: 600;
    color: var(--txt);
  }
  .dist-state {
    margin-left: auto;
    font-size: 8px;
    color: var(--muted);
  }
  .dist-state.on {
    color: var(--green);
  }

  .sounds {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .sound {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--panel2);
    border: 1px solid var(--line);
    padding: 7px 10px;
    text-align: left;
  }
  .sound.sel {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
  .sound:disabled {
    opacity: 0.6;
  }
  .radio {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 1px solid var(--muted2);
    flex: none;
  }
  .sound.sel .radio {
    border-color: var(--rosso-bright);
    background: radial-gradient(var(--rosso-bright) 40%, transparent 45%);
  }
  .s-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .s-tag {
    font-size: 7px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .restore-note {
    margin-top: 6px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 9px;
    font-family: var(--mono);
    padding: 5px 9px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 8px;
  }
  .tag {
    font-size: 10px;
    padding: 2px 8px;
    font-family: var(--mono);
    border: 1px solid var(--line);
  }
  .tag.tiny {
    font-size: 7px;
    padding: 0 5px;
  }
  .tag.cat {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
  }
  .tag.rule {
    background: var(--green-dim);
    color: var(--green);
    border-color: var(--green-border);
  }
  .tag.manual {
    background: var(--raised);
    color: var(--txt2);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .tag.mod {
    background: var(--blue-dim);
    color: var(--blue);
    border-color: var(--blue-border);
  }
  .tag .x {
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    line-height: 1;
    padding: 0;
  }
  .tag .x:hover {
    color: var(--rosso-bright);
  }
  .manual-input {
    width: 100%;
    padding: 5px 8px;
    font-size: 11px;
  }

  .ver {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 6px 10px;
    margin-bottom: 5px;
  }
  .ver.active {
    border-left: 3px solid var(--rosso);
  }
  .v-label {
    font-size: 10px;
    font-weight: 600;
  }
  .v-activate {
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 9px;
    padding: 2px 7px;
  }
  .v-activate:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .v-meta {
    margin-left: auto;
    color: var(--faint);
    font-size: 9px;
  }

  .history {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .history li {
    display: flex;
    flex-direction: column;
    font-size: 11px;
    border-left: 2px solid var(--line);
    padding-left: 8px;
  }
  .history .ev {
    color: var(--rosso-bright);
    font-weight: 600;
    font-size: 9px;
    letter-spacing: 0.5px;
  }
  .history .det {
    color: var(--txt2);
  }
  .history .ts {
    color: var(--muted2);
    font-size: 9px;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 11px;
  }

  .tsk-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 6px;
  }
  .tsk-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
  }
  .tsk-list li.inactive {
    opacity: 0.6;
  }
  .tsk-list .tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  .tsk-name {
    flex: 1;
    font-size: 11px;
    color: var(--txt2);
  }
  .tsk-src {
    font-size: 9px;
    color: var(--muted2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }
  .loading-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spinner-sm {
    flex: none;
    width: 12px;
    height: 12px;
    border: 2px solid var(--line);
    border-top-color: var(--rosso);
    border-radius: 50%;
    animation: tsk-spin 0.8s linear infinite;
  }
  @keyframes tsk-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Couches / extensions (§4.4) */
  .layer-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 6px 0 0;
    padding: 0;
  }
  .layer-row {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
  }
  .layer-row.inactive {
    opacity: 0.5;
  }
  .layer-tog {
    flex: none;
    display: flex;
    align-items: center;
    cursor: pointer;
  }
  .layer-ord {
    flex: none;
    display: flex;
    flex-direction: column;
    line-height: 0.7;
  }
  .layer-arrow {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 8px;
    padding: 1px 2px;
  }
  .layer-arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .layer-arrow:not(:disabled):hover {
    color: var(--txt2);
  }
  .layer-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .layer-nm {
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .layer-counts {
    font-size: 9px;
    color: var(--muted2);
  }
  .layer-x {
    flex: none;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 12px;
    padding: 2px 4px;
  }
  .layer-x:hover {
    color: var(--rosso-bright);
  }

  /* Bloc Ressources (§4.6) */
  .res-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 6px 0 0;
    padding: 0;
  }
  .res-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 5px 9px;
    text-align: left;
    cursor: pointer;
  }
  .res-row:hover {
    border-color: var(--rosso-border);
  }
  .res-nm {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .res-row:hover .res-nm {
    color: var(--rosso-bright);
  }
  .res-size {
    flex: none;
    font-size: 9px;
    color: var(--muted2);
  }

  /* Provenance / pack d'origine (§4.7) */
  .srcbox {
    border: 1px solid var(--line);
  }
  .src-h {
    background: var(--raised);
    padding: 5px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 8px;
    letter-spacing: 1.5px;
  }
  .srcrow {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px 11px;
    border-bottom: 1px solid var(--line);
  }
  .srcrow:last-child {
    border-bottom: none;
  }
  .src-k {
    color: var(--faint);
    font-size: 8px;
    letter-spacing: 1px;
    width: 84px;
    flex-shrink: 0;
  }
  .src-v {
    font-size: 10.5px;
    font-family: var(--mono);
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .src-v.url {
    color: var(--blue);
  }
  .src-empty {
    color: var(--muted2);
    font-size: 9.5px;
    font-family: var(--mono);
    font-style: italic;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 10px;
    font-family: var(--mono);
    padding: 3px 9px;
  }
  .chip .chip-n {
    color: var(--muted);
  }
  .siblings {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
  }
  .sib {
    background: var(--card);
    padding: 7px 9px;
    display: flex;
    align-items: center;
    gap: 7px;
    text-align: left;
  }
  .sib:hover {
    background: var(--raised);
  }
  .sib-dot {
    font-size: 13px;
    flex: none;
  }
  .sib-nm {
    font-size: 9.5px;
    color: var(--txt2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prov-note {
    margin-top: 8px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    color: var(--blue);
    font-size: 9px;
    font-family: var(--mono);
    padding: 6px 9px;
  }
  .prov-actions {
    display: flex;
    gap: 7px;
    margin-top: 10px;
  }
  .btn.danger {
    color: var(--muted);
  }
  .btn.danger:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
</style>
