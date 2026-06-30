<script lang="ts">
  // Page de détail pleine page (§6.3, maquette pitbox-fiche-B-revisee.html).
  // Riche pour les voitures (héros + specs natives + fiche technique + courbe +
  // description + skins + tags/versions/historique). Panneaux Son et Distance =
  // placeholders « à venir » (lots §12bis et §6.5). Réduite pour les circuits.
  import {
    activateMod,
    deactivateMod,
    getModDetail,
    previewSrc,
    setFavorite,
    setManualTags,
    type ModDetail,
    type ModKind,
    type NativeSpecs,
  } from "$lib/library";
  import { listModSkins, type SkinItem } from "$lib/launch";
  import { exportMod, type ExportReport } from "$lib/maintenance";
  import {
    listSubMods,
    activateSound,
    restoreSound,
    type SubModRow,
  } from "$lib/submods";
  import { open } from "@tauri-apps/plugin-dialog";
  import PowerCurve from "./PowerCurve.svelte";
  import { nav } from "$lib/nav.svelte";

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
  let pilotedSkin = $state<string | null>(null);
  let sounds = $state<SubModRow[]>([]);
  let soundBusy = $state(false);
  const activeSound = $derived(sounds.find((s) => s.is_active) ?? null);
  let showDescription = $state(false);
  let busy = $state(false);
  let actionError = $state("");
  let manualInput = $state("");
  let exporting = $state(false);
  let exportResult = $state<ExportReport | null>(null);

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
    showDescription = false;
    actionError = "";
    getModDetail(current).then((d) => {
      if (current !== id) return;
      detail = d;
    });
    if (isCar) {
      pilotedSkin = localStorage.getItem(`pitbox.pilotedSkin.${current}`);
      listModSkins(current).then((s) => {
        if (current !== id) return;
        skins = s;
        const pi = s.findIndex((x) => x.id === pilotedSkin);
        previewSkin = pi >= 0 ? pi : 0;
      });
      loadSounds(current);
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

  function setPiloted(skinId: string, e: Event) {
    e.stopPropagation();
    pilotedSkin = skinId;
    localStorage.setItem(`pitbox.pilotedSkin.${id}`, skinId);
  }

  function drive() {
    if (!detail) return;
    nav.prefill = {
      kind: detail.kind,
      id: detail.id_interne,
      name: detail.display_name ?? detail.id_interne,
    };
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
    {@const hero = previewSrc(d.preview)}
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
        {#if hero}
          <img src={hero} alt={d.display_name ?? d.id_interne} />
        {:else}
          <div class="hero-icon">{isCar ? "🚗" : "🏁"}</div>
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
          <div class="box">
            <div class="box-h">FICHE TECHNIQUE</div>
            <div class="specgrid">
              {#each ficheRows(d) as [k, v]}
                <div><div class="k">{k}</div><div class="v">{v}</div></div>
              {/each}
            </div>
          </div>

          {#if d.specs && d.specs.power_curve.length > 1}
            <div class="lbl">
              COURBE MOTEUR
              <span class="legend"><span class="lg-pow">— POWER</span><span class="lg-tor">— TORQUE</span></span>
            </div>
            <div class="curve-box">
              <PowerCurve power={d.specs.power_curve} torque={d.specs.torque_curve} />
            </div>
          {/if}

          {#if d.specs?.description}
            <button class="box-h desc-toggle" type="button" onclick={() => (showDescription = !showDescription)}>
              DESCRIPTION <span class="chev">{showDescription ? "▲" : "▼"}</span>
            </button>
            {#if showDescription}
              <div class="desc-body">{decodeDescription(d.specs.description)}</div>
            {/if}
          {/if}
        {:else}
          <div class="box">
            <div class="box-h">LAYOUTS · {d.layouts.length || 1}</div>
            <div class="layouts">
              {#if d.layouts.length}
                {#each d.layouts as l}<span class="layout">{l}</span>{/each}
              {:else}
                <span class="muted">Tracé unique</span>
              {/if}
            </div>
          </div>
          {#if d.csp_features.length}
            <div class="lbl">EXTENSIONS CSP</div>
            <div class="csp-row">{#each d.csp_features as f}<span class="csp">{f}</span>{/each}</div>
          {/if}
        {/if}
      </div>
    </div>

    <!-- RANGÉE BASSE -->
    <div class="row bottom" class:track={!isCar}>
      {#if isCar}
        <!-- Skins (sélection/prévisualisation + étoile piloté ; pas d'activation, §12bis.2) -->
        <div class="col">
          <div class="lbl">
            SKINS <span class="lbl-sub">{skins.length} disponible(s) · cliquer pour prévisualiser · ★ = piloté</span>
          </div>
          {#if skins.length}
            <div class="skins">
              {#each skins as sk, i (sk.id)}
                {@const sp = previewSrc(sk.preview)}
                <button
                  class="skin"
                  class:preview={i === previewSkin}
                  class:piloted={sk.id === pilotedSkin}
                  onclick={() => (previewSkin = i)}
                  title="Cliquer pour prévisualiser"
                >
                  <div class="skin-img">
                    {#if sp}<img src={sp} alt={sk.name} loading="lazy" />{:else}<span class="skin-noimg">▦</span>{/if}
                    {#if sk.id === pilotedSkin}<span class="skin-star on" title="Piloté au lancement">★</span>{/if}
                    {#if i === previewSkin}<span class="skin-apercu mono">APERÇU</span>{/if}
                  </div>
                  <div class="skin-b">
                    <span class="skin-name">{sk.name}</span>
                    {#if sk.id !== pilotedSkin}
                      <span
                        class="skin-star"
                        role="button"
                        tabindex="-1"
                        title="Définir comme piloté"
                        onclick={(e) => setPiloted(sk.id, e)}
                        onkeydown={(e) => e.key === "Enter" && setPiloted(sk.id, e)}
                      >☆</span>
                    {/if}
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

        <!-- Tags + Versions + Historique -->
        <div class="col">
          {@render tagsBlock(d)}
          {@render versionsBlock(d)}
          {@render historyBlock(d)}
        </div>
      {:else}
        <!-- Circuit : tags + versions + historique -->
        <div class="col">{@render tagsBlock(d)}</div>
        <div class="col">{@render versionsBlock(d)}</div>
        <div class="col">{@render historyBlock(d)}</div>
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
    margin-bottom: 12px;
  }
  .desc-toggle {
    cursor: pointer;
    border: 1px solid var(--line);
  }
  .desc-toggle .chev {
    margin-left: auto;
    font-size: 10px;
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
  .layouts {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 10px;
    background: var(--panel2);
  }
  .layout {
    font-size: 11px;
    font-family: var(--mono);
    padding: 2px 8px;
    border: 1px solid var(--line);
    color: var(--txt2);
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
    height: 56px;
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
  .skin-noimg {
    color: var(--faint);
    font-size: 16px;
  }
  .skin-star {
    position: absolute;
    top: 3px;
    right: 4px;
    font-size: 12px;
    color: var(--muted2);
    cursor: pointer;
  }
  .skin-star.on {
    color: var(--rosso);
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
  .skin-b .skin-star {
    position: static;
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
</style>
