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
    previewSrc,
    setFavorite,
    setManualTags,
    type ModCard,
    type ModDetail,
    type ModKind,
    type NativeSpecs,
  } from "$lib/library";
  import { listModSkins, type SkinItem } from "$lib/launch";
  import { exportMod, deletePack, type ExportReport } from "$lib/maintenance";
  import {
    listSubMods,
    activateSound,
    restoreSound,
    type SubModRow,
  } from "$lib/submods";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import PowerCurve from "./PowerCurve.svelte";
  import { nav, pickSession } from "$lib/nav.svelte";

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
  let busy = $state(false);
  let actionError = $state("");
  let manualInput = $state("");
  let exporting = $state(false);
  let exportResult = $state<ExportReport | null>(null);
  // Provenance / pack d'origine (§4.7).
  let siblings = $state<ModCard[]>([]);
  let packBusy = $state(false);

  // Image héros : voiture → skin sélectionné ; circuit → preview du layout
  // sélectionné ; sinon preview par défaut du mod.
  const heroImg = $derived.by(() => {
    if (isCar && skins[previewSkin]?.preview) return previewSrc(skins[previewSkin].preview);
    const lay = detail?.track?.layouts[previewLayout];
    if (!isCar && lay?.preview) return previewSrc(lay.preview);
    return previewSrc(detail?.preview ?? null);
  });

  function filterByPack() {
    if (!detail?.source_pack) return;
    nav.section = detail.kind === "Track" ? "tracks" : "cars";
    nav.search = detail.source_pack;
  }

  function openSibling(c: ModCard) {
    nav.section = c.kind === "Track" ? "tracks" : "cars";
    nav.openMod = c.id_interne;
  }

  function activeArchive(d: ModDetail): string | null {
    return d.versions.find((v) => v.id === d.active_version_id)?.source_archive ?? null;
  }

  async function uninstallPack() {
    if (!detail?.source_pack || packBusy) return;
    const ok = await confirm(
      `Désinstaller le pack « ${detail.source_pack} » ? Les ${siblings.length + 1} mods du pack seront supprimés (fichiers + bibliothèque). Irréversible.`,
      { title: "Désinstaller le pack", kind: "warning" },
    );
    if (!ok) return;
    packBusy = true;
    actionError = "";
    try {
      await deletePack(detail.source_pack);
      onchange?.();
      onclose();
    } catch (e) {
      actionError = String(e);
      packBusy = false;
    }
  }

  async function doExport() {
    if (!detail || exporting) return;
    const dir = await open({ directory: true, multiple: false, title: "Dossier d'export" });
    if (!dir || typeof dir !== "string") return;
    exporting = true;
    actionError = "";
    exportResult = null;
    try {
      exportResult = await exportMod(detail.id_interne, dir);
    } catch (e) {
      actionError = String(e);
    } finally {
      exporting = false;
    }
  }

  $effect(() => {
    const current = id;
    actionError = "";
    siblings = [];
    previewLayout = 0;
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
        const savedLayout = localStorage.getItem(`pitbox.layout.${current}`);
        const li = d.track.layouts.findIndex((l) => l.id === savedLayout);
        previewLayout = li >= 0 ? li : 0;
      }
    });
    if (isCar) {
      const savedSkin = localStorage.getItem(`pitbox.skin.${current}`);
      listModSkins(current).then((s) => {
        if (current !== id) return;
        skins = s;
        const pi = s.findIndex((x) => x.id === savedSkin);
        previewSkin = pi >= 0 ? pi : 0;
      });
      loadSounds(current);
    } else {
      listSubMods(current).then((all) => {
        if (current !== id) return;
        trackSkins = all.filter((s) => s.sub_type === "TRACK_SKIN");
      });
    }
  });

  async function loadSounds(parent: string) {
    const all = await listSubMods(parent);
    if (parent !== id) return;
    sounds = all.filter((s) => s.sub_type === "SOUND");
  }

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
      actionError = String(e);
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
    if (sk) localStorage.setItem(`pitbox.skin.${detail.id_interne}`, sk.id);
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
  }

  // Sélectionner un layout de circuit : mémorisé + poussé dans le duo de session
  // (photo + tracé en surimpression dans le menu).
  function selectLayout(i: number) {
    previewLayout = i;
    if (!detail?.track) return;
    const l = detail.track.layouts[i];
    if (l) localStorage.setItem(`pitbox.layout.${detail.id_interne}`, l.id);
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

  function drive() {
    if (!detail) return;
    // Fige le duo de session avec la sélection courante (skin ou layout), puis
    // ouvre la page session.
    if (isCar) selectSkin(previewSkin);
    else selectLayout(previewLayout);
    nav.section = "race";
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
      actionError = String(e);
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
      actionError = String(e);
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
    const t = manualInput.trim().toLowerCase();
    manualInput = "";
    if (!t || detail.tags_manual.includes(t)) return;
    detail.tags_manual = [...detail.tags_manual, t];
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
      .replace(/<br\s*\/?>/gi, "\n")
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

  const POS: Record<string, string> = { FRONT: "AV", MID: "CEN", REAR: "ARR" };
  const DASH = "—";

  // Fiche technique (champs structurés §5bis.1) — abréviations façon maquette.
  function ficheRows(d: ModDetail): [string, string][] {
    const engine = [d.engine_config, d.engine_pos ? POS[d.engine_pos] ?? d.engine_pos : null]
      .filter(Boolean)
      .join(" · ");
    return [
      ["MOTEUR", engine || DASH],
      ["ADMISSION", d.aspiration ?? DASH],
      ["TRANSM.", d.drivetrain ?? DASH],
      ["BOÎTE", d.gearbox ?? DASH],
      ["PAYS", d.country ?? DASH],
      ["P/POIDS", d.specs?.pwratio ?? DASH],
    ];
  }

  function fmtDate(iso: string): string {
    return iso.slice(0, 16).replace("T", " ");
  }
</script>

<div class="page">
  {#if !detail}
    <div class="empty">Chargement…</div>
  {:else}
    {@const d = detail}
    <header class="head">
      <button class="back" type="button" onclick={onclose} title="Retour à la liste">←</button>
      <span class="escu">{initials(d.brand, d.id_interne)}</span>
      <div class="title">
        <div class="t-name">{d.display_name ?? d.id_interne}</div>
        <div class="t-meta mono">
          {d.brand ?? ""}{d.year ? ` · ${d.year}` : ""}
          {#if d.category}· <span class="cat">{d.category}</span>{/if}
          {#if d.car_class}· {d.car_class.toUpperCase()}{/if}
        </div>
      </div>
      <div class="actions">
        <button class="fav" class:on={d.is_favorite} type="button" onclick={toggleFav} title="Favori">
          {d.is_favorite ? "♥" : "♡"}
        </button>
        {#if d.is_stock}
          <span class="base-tag" title="Contenu de base Kunos — lecture seule (§12bis.1)">Contenu de base</span>
        {:else if d.active}
          <button class="btn" type="button" onclick={deactivate} disabled={busy}>Désactiver</button>
        {:else}
          <button class="btn" type="button" onclick={() => activate()} disabled={busy}>Activer</button>
        {/if}
        {#if !d.is_stock}
          <button class="btn" type="button" onclick={doExport} disabled={exporting} title="Exporter en archive autonome (§9.1)">
            {exporting ? "Export…" : "⤓ Exporter"}
          </button>
        {/if}
        <button class="btn primary" type="button" onclick={drive}>Conduire</button>
      </div>
    </header>

    {#if actionError}<div class="action-err">{actionError}</div>{/if}
    {#if exportResult}
      <div class="export-ok">
        ✓ Archive créée : {exportResult.included.length} élément(s) embarqué(s).
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
        {#if !isCar}
          {@const ol = previewSrc(d.track?.layouts[previewLayout]?.outline ?? null)}
          {#if ol}<img class="hero-outline" src={ol} alt="" />{/if}
        {/if}
        {#if isCar}
          {@const hs = heroSpecs(d.specs)}
          {#if hs}
            <div class="hero-specs">
              <div class="mono hs-line">{hs}</div>
              <div class="mono hs-label">SPEC NATIF</div>
            </div>
          {/if}
        {/if}
        <!-- Le fichier du mod n'est jamais réécrit (règle d'or §3.0). -->
        <div class="badge-lock"><span class="lock">🔒</span> FICHIER NON MODIFIÉ</div>
      </div>

      <div class="data">
        {#if isCar}
          {@const hasCurve = !!d.specs && d.specs.power_curve.length > 1}
          <div class="tech-curve" class:with-curve={hasCurve}>
            <div class="box fiche">
              <div class="box-h">FICHE TECHNIQUE</div>
              <div class="specgrid">
                {#each ficheRows(d) as [k, v]}
                  <div><div class="k">{k}</div><div class="v">{v}</div></div>
                {/each}
              </div>
            </div>
            {#if hasCurve && d.specs}
              <div class="curve-col">
                <div class="lbl">
                  COURBE
                  <span class="legend"><span class="lg-pow">— bhp</span><span class="lg-tor">— Nm</span></span>
                </div>
                <div class="curve-box">
                  <PowerCurve power={d.specs.power_curve} torque={d.specs.torque_curve} />
                </div>
              </div>
            {/if}
          </div>

          {#if d.specs?.description}
            <div class="box-h">DESCRIPTION</div>
            <div class="desc-body">{decodeDescription(d.specs.description)}</div>
          {/if}
        {:else}
          {@const lay = d.track?.layouts[previewLayout]}
          <div class="box">
            <div class="box-h">INFOS CIRCUIT</div>
            <div class="specgrid" style="grid-template-columns:1fr 1fr;">
              <div><div class="k">LAYOUT</div><div class="v">{lay?.name ?? "(défaut)"}</div></div>
              <div><div class="k">LONGUEUR</div><div class="v">{lay?.length ?? "—"}</div></div>
            </div>
          </div>
          {#if d.csp_features.length}
            <div class="lbl">EXTENSIONS CSP</div>
            <div class="csp-row">{#each d.csp_features as f}<span class="csp">{f}</span>{/each}</div>
          {/if}
          {#if d.track?.description}
            <div class="box-h" style="margin-top:11px;">DESCRIPTION</div>
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
            SKINS <span class="lbl-sub">{skins.length} disponible(s) · cliquer = skin de session</span>
          </div>
          {#if skins.length}
            <div class="skins">
              {#each skins as sk, i (sk.id)}
                {@const sp = previewSrc(sk.preview)}
                <button
                  class="skin"
                  class:preview={i === previewSkin}
                  onclick={() => selectSkin(i)}
                  title="Choisir ce skin pour la session"
                >
                  <div class="skin-img">
                    {#if sp}<img src={sp} alt={sk.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if i === previewSkin}<span class="skin-apercu mono">SESSION</span>{/if}
                  </div>
                  <div class="skin-b">
                    <span class="skin-name">{sk.name}</span>
                  </div>
                </button>
              {/each}
            </div>
          {:else}
            <div class="muted small">Aucun skin pour cette voiture.</div>
          {/if}
        </div>

        <!-- Distance (§6.5) + Son (§12bis) : placeholders « à venir » désactivés -->
        <div class="col">
          <div class="lbl">DISTANCE</div>
          <div class="box">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? "essayée ✓" : "jamais essayée"}</span>
            </div>
          </div>
          <div class="lbl" style="margin-top:14px;">SON MOTEUR <span class="lbl-sub">exclusif — un seul</span></div>
          <div class="sounds">
            <button class="sound" class:sel={!activeSound} type="button" onclick={() => pickSound(null)} disabled={soundBusy}>
              <span class="radio"></span>
              <span class="s-name">Origine</span>
              <span class="s-tag mono">BASE</span>
            </button>
            {#each sounds as snd (snd.id)}
              <button class="sound" class:sel={snd.is_active} type="button" onclick={() => pickSound(snd.id)} disabled={soundBusy}>
                <span class="radio"></span>
                <span class="s-name">{snd.name}</span>
                <span class="s-tag mono">MOD</span>
              </button>
            {/each}
          </div>
          {#if sounds.length === 0}
            <div class="muted small" style="margin-top:6px;">Aucun mod de son importé pour cette voiture.</div>
          {:else}
            <div class="restore-note">↺ son d'origine restaurable</div>
          {/if}
        </div>

        <!-- Tags + Versions + Historique + Provenance -->
        <div class="col">
          {@render tagsBlock(d)}
          {@render versionsBlock(d)}
          {@render historyBlock(d)}
          {@render provenanceBlock(d)}
        </div>
      {:else}
        <!-- Layouts (galerie illustrée par le tracé, comme les skins voiture) -->
        <div class="col">
          <div class="lbl">
            LAYOUTS <span class="lbl-sub">{d.track?.layouts.length ?? 0} · cliquer = layout de session</span>
          </div>
          {#if d.track && d.track.layouts.length}
            <div class="skins">
              {#each d.track.layouts as l, i (l.id || i)}
                {@const o = previewSrc(l.outline)}
                <button class="skin" class:preview={i === previewLayout} onclick={() => selectLayout(i)} title="Choisir ce layout pour la session">
                  <div class="skin-img layout-img">
                    {#if o}<img src={o} alt={l.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if i === previewLayout}<span class="skin-apercu mono">SESSION</span>{/if}
                  </div>
                  <div class="skin-b"><span class="skin-name">{l.name}</span></div>
                </button>
              {/each}
            </div>
          {:else}
            <div class="muted small">Tracé unique.</div>
          {/if}

          <!-- Skins de circuit (TRACK_SKIN, §12bis.2) — pas d'activation, tous chargés. -->
          <div class="lbl section">SKINS DE CIRCUIT · {trackSkins.length}</div>
          {#if trackSkins.length}
            <ul class="tsk-list">
              {#each trackSkins as s (s.id)}
                <li><span class="tsk-name">{s.name}</span>{#if s.source_archive}<span class="tsk-src mono">{s.source_archive}</span>{/if}</li>
              {/each}
            </ul>
            <div class="muted small">Tous présents → chargés par AC. Gestion détaillée dans la vue Skins.</div>
          {:else}
            <div class="muted small">Aucun skin de circuit importé.</div>
          {/if}
        </div>

        <!-- Distance (§6.5) -->
        <div class="col">
          <div class="lbl">DISTANCE</div>
          <div class="box">
            <div class="dist">
              <span class="dist-ic">🛣</span>
              <span class="dist-km mono">{d.distance_km != null ? `${d.distance_km.toFixed(1)} km` : "—"}</span>
              <span class="dist-state mono" class:on={d.tried}>{d.tried ? "essayé ✓" : "jamais essayé"}</span>
            </div>
          </div>
        </div>

        <!-- Tags + Versions + Historique + Provenance -->
        <div class="col">
          {@render tagsBlock(d)}
          {@render versionsBlock(d)}
          {@render historyBlock(d)}
          {@render provenanceBlock(d)}
        </div>
      {/if}
    </div>
  {/if}
</div>

{#snippet tagsBlock(d: ModDetail)}
  <div class="lbl">TAGS</div>
  <div class="tags">
    {#each d.tags_from_rule.filter((t) => t.startsWith("#")) as t}<span class="tag cat">{t}</span>{/each}
    {#each d.tags_from_rule.filter((t) => !t.startsWith("#")) as t}<span class="tag rule">{t}</span>{/each}
    {#each d.tags_manual as t}
      <span class="tag manual">{t}<button class="x" type="button" onclick={() => removeManual(t)} title="Retirer">×</button></span>
    {/each}
    {#each d.tags_from_mod as t}<span class="tag mod">{t}</span>{/each}
  </div>
  <input
    class="input manual-input"
    placeholder="ajouter un tag manuel…"
    bind:value={manualInput}
    onkeydown={(e) => e.key === "Enter" && addManual()}
  />
{/snippet}

{#snippet versionsBlock(d: ModDetail)}
  <div class="lbl section">VERSIONS · {d.versions.length}</div>
  {#each d.versions as v}
    <div class="ver" class:active={v.id === d.active_version_id}>
      <span class="v-label mono">{v.version_label ?? "(sans n°)"}</span>
      {#if v.id === d.active_version_id}
        <span class="tag cat tiny">ACTIVE</span>
      {:else}
        <button class="v-activate" type="button" onclick={() => activate(v.id)} disabled={busy}>Activer</button>
      {/if}
      <span class="v-meta mono">{fmtDate(v.imported_at)}</span>
    </div>
  {/each}
{/snippet}

{#snippet historyBlock(d: ModDetail)}
  <div class="lbl section">HISTORIQUE</div>
  <ul class="history">
    {#each d.history as h}
      <li>
        <span class="ev">{h.event}</span>
        <span class="det">{h.details}</span>
        <span class="ts mono">{fmtDate(h.timestamp)}</span>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet provenanceBlock(d: ModDetail)}
  {@const archive = activeArchive(d)}
  {#if d.source_pack || archive || d.source_url}
    <div class="lbl section">SOURCE / ORIGINE</div>
    <div class="srcbox">
      <div class="src-h">PROVENANCE DU MOD</div>
      {#if d.source_pack}
        <div class="srcrow">
          <span class="src-k">PACK</span>
          <button class="chip" type="button" onclick={filterByPack} title="Voir toutes les entités de ce pack">
            ⬢ {d.source_pack} <span class="chip-n">· {siblings.length + 1} mod(s)</span>
          </button>
        </div>
      {/if}
      <div class="srcrow">
        <span class="src-k">ARCHIVE</span>
        <span class="src-v">{archive ?? "—"}</span>
      </div>
      <div class="srcrow">
        <span class="src-k">URL D'ORIGINE</span>
        {#if d.source_url}
          <span class="src-v url">{d.source_url}</span>
        {:else}
          <span class="src-empty">— non renseignée (extension navigateur, lot L7)</span>
        {/if}
      </div>
    </div>

    {#if d.source_pack}
      <div class="lbl section">AUTRES ENTITÉS DU MÊME PACK · {siblings.length}</div>
      {#if siblings.length}
        <div class="siblings">
          {#each siblings as c (c.id_interne)}
            <button class="sib" type="button" onclick={() => openSibling(c)} title="Ouvrir la fiche">
              <span class="sib-dot">{c.kind === "Track" ? "🏁" : "🚗"}</span>
              <span class="sib-nm">{c.display_name ?? c.id_interne}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="muted small">Seule entité de ce pack pour l'instant.</div>
      {/if}
      <div class="prov-note">Chaque entité reste indépendante (activable, tagguable séparément).</div>
      <div class="prov-actions">
        <button class="btn" type="button" onclick={filterByPack}>⌕ Filtrer par ce pack</button>
        <button class="btn danger" type="button" onclick={uninstallPack} disabled={packBusy}>
          {packBusy ? "…" : "🗑 Désinstaller le pack"}
        </button>
      </div>
    {/if}
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
  .btn.primary {
    background: var(--rosso);
    color: #fff;
    border-color: var(--rosso);
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
  .hero img {
    width: 100%;
    height: 100%;
    object-fit: cover;
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
  .badge-lock {
    position: absolute;
    left: 16px;
    top: 14px;
    display: flex;
    align-items: center;
    gap: 5px;
    background: rgba(8, 8, 12, 0.6);
    border: 1px solid var(--green-border);
    padding: 3px 8px;
    color: var(--green);
    font-family: var(--mono);
    font-size: 8px;
    letter-spacing: 0.5px;
  }
  .badge-lock .lock {
    font-size: 9px;
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
  }
  .skin.preview {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
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
