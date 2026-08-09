<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    getMusicConfig,
    saveMusicConfig,
    getDefaultMusicFolders,
    scanMusicFolder,
    musicPreviewStart,
    musicPreviewStop,
    emptyMusicConfig,
    type MusicConfig,
    type DefaultMusicFolders,
  } from "$lib/music";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";

  // Fichier séparé de AppConfig (music.json, §2 de la spec musique) : état et
  // sauvegarde propres à cet onglet, indépendants de la garde de navigation
  // de Settings.svelte (qui ne porte que sur AppConfig).
  let config = $state<MusicConfig>(emptyMusicConfig());
  let savedConfig = emptyMusicConfig();
  let defaults = $state<DefaultMusicFolders>({ menu: "", grid: "" });
  let menuTrackCount = $state<number | null>(null);
  let gridTrackCount = $state<number | null>(null);
  let previewing = $state<"menu" | "grid" | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let error = $state("");
  let loaded = $state(false);

  const menuPath = $derived(config.menu_folder ?? defaults.menu);
  const gridPath = $derived(config.grid_folder ?? defaults.grid);

  onMount(async () => {
    const [cfg, def] = await Promise.all([getMusicConfig(), getDefaultMusicFolders()]);
    config = cfg;
    savedConfig = structuredClone($state.snapshot(cfg));
    defaults = def;
    loaded = true;
  });

  onDestroy(() => {
    if (previewing) musicPreviewStop();
  });

  // Recompte les pistes à chaque changement de chemin (saisie ou Parcourir),
  // menu et grid indépendamment.
  $effect(() => {
    if (!loaded) return;
    const path = menuPath;
    scanMusicFolder(path).then((info) => (menuTrackCount = info.track_count));
  });
  $effect(() => {
    if (!loaded) return;
    const path = gridPath;
    scanMusicFolder(path).then((info) => (gridTrackCount = info.track_count));
  });

  async function browse(which: "menu" | "grid") {
    const selected = await open({ directory: true, multiple: false, defaultPath: which === "menu" ? menuPath : gridPath });
    if (typeof selected !== "string") return;
    if (which === "menu") config.menu_folder = selected;
    else config.grid_folder = selected;
  }

  async function togglePreview(which: "menu" | "grid") {
    error = "";
    if (previewing === which) {
      await musicPreviewStop();
      previewing = null;
      return;
    }
    try {
      await musicPreviewStart(which === "menu" ? menuPath : gridPath, config.volume);
      previewing = which;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function save() {
    saving = true;
    error = "";
    try {
      await saveMusicConfig(config);
      saved = true;
      savedConfig = structuredClone($state.snapshot(config));
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }

  function fmtSeconds(ms: number): string {
    return (ms / 1000).toFixed(1).replace(".", ",");
  }
  function pct(v: number): number {
    return Math.round(v * 100);
  }
</script>

<p class="sub">{t("settings.tabMusicHint")}</p>

<section class="lang-section">
  <label class="check">
    <input type="checkbox" bind:checked={config.enabled} />
    <span>{t("music.enable")}</span>
  </label>
</section>

<section class="folder-block">
  <div class="label">{t("music.menuAmbience")}</div>
  <div class="row2">
    <input
      class="input mono"
      type="text"
      placeholder={defaults.menu}
      value={config.menu_folder ?? ""}
      spellcheck="false"
      oninput={(e) => (config.menu_folder = e.currentTarget.value || null)}
    />
    <button class="btn" type="button" onclick={() => browse("menu")}>{t("pathfield.browse")}</button>
    <button
      class="btn preview"
      type="button"
      title={t("music.preview")}
      disabled={!menuTrackCount}
      onclick={() => togglePreview("menu")}
    >
      {previewing === "menu" ? "■" : "▶"}
    </button>
  </div>
  <p class="hint">{menuTrackCount === null ? t("common.loading") : t("music.trackCount", { count: menuTrackCount })}</p>
</section>

<section class="folder-block">
  <div class="label">{t("music.gridAmbience")}</div>
  <div class="row2">
    <input
      class="input mono"
      type="text"
      placeholder={defaults.grid}
      value={config.grid_folder ?? ""}
      spellcheck="false"
      oninput={(e) => (config.grid_folder = e.currentTarget.value || null)}
    />
    <button class="btn" type="button" onclick={() => browse("grid")}>{t("pathfield.browse")}</button>
    <button
      class="btn preview"
      type="button"
      title={t("music.preview")}
      disabled={!gridTrackCount}
      onclick={() => togglePreview("grid")}
    >
      {previewing === "grid" ? "■" : "▶"}
    </button>
  </div>
  <p class="hint">{gridTrackCount === null ? t("common.loading") : t("music.trackCount", { count: gridTrackCount })}</p>
</section>

<section class="lang-section">
  <label class="check">
    <input type="checkbox" bind:checked={config.shuffle} />
    <span>{t("music.shuffle")}</span>
  </label>
</section>

<section class="lang-section">
  <label>
    <span>{t("music.volume")} — {pct(config.volume)}%</span>
    <input
      type="range"
      min="0"
      max="100"
      value={pct(config.volume)}
      oninput={(e) => (config.volume = Number(e.currentTarget.value) / 100)}
    />
  </label>
</section>

<section class="lang-section">
  <label>
    <span>{t("music.crossfade")} — {fmtSeconds(config.crossfade_ms)} s</span>
    <input type="range" min="500" max="6000" step="250" bind:value={config.crossfade_ms} />
  </label>
</section>

<div class="deploy-block">
  <div class="row1"><span class="label">{t("music.sessionBehavior")}</span></div>

  <label class="radio-opt">
    <input type="radio" name="session_behavior" value="stop" bind:group={config.session_behavior} />
    <span>
      <span class="radio-title">{t("music.sessionStop")}</span>
      <span class="radio-hint">{t("music.sessionStopHint")}</span>
    </span>
  </label>

  <label class="radio-opt">
    <input type="radio" name="session_behavior" value="duck" bind:group={config.session_behavior} />
    <span>
      <span class="radio-title">{t("music.sessionDuck")}</span>
      <span class="radio-hint">{t("music.sessionDuckHint")}</span>
    </span>
  </label>

  {#if config.session_behavior === "duck"}
    <label class="duck-volume">
      <span>{t("music.duckVolume")} — {pct(config.session_duck_volume)}%</span>
      <input
        type="range"
        min="0"
        max="100"
        value={pct(config.session_duck_volume)}
        oninput={(e) => (config.session_duck_volume = Number(e.currentTarget.value) / 100)}
      />
    </label>
  {/if}
</div>

{#if error}<div class="error">{error}</div>{/if}

<footer>
  {#if saved}<span class="pill pill-ok">{t("settings.saved")}</span>{/if}
  <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
    {saving ? t("settings.saving") : t("settings.save")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
  .sub {
    color: var(--muted);
    margin-bottom: 22px;
    line-height: 1.5;
  }
  .lang-section {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  .lang-section label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 340px;
  }
  .lang-section label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    max-width: none;
    cursor: pointer;
  }
  .folder-block {
    margin-bottom: 22px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--line);
  }
  .folder-block .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: var(--txt2);
    text-transform: uppercase;
    margin-bottom: 8px;
  }
  .row2 {
    display: flex;
    gap: 8px;
  }
  .row2 .input {
    flex: 1;
  }
  .btn.preview {
    flex: none;
    width: 38px;
    justify-content: center;
    padding: 8px 0;
  }
  .hint {
    margin-top: 8px;
    font-size: 11px;
    color: var(--faint);
  }

  .deploy-block {
    margin-bottom: 16px;
  }
  .row1 {
    margin-bottom: 8px;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.5px;
    color: var(--txt2);
    text-transform: uppercase;
  }
  .radio-opt {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 8px 0;
    cursor: pointer;
  }
  .radio-opt input {
    margin-top: 2px;
    accent-color: var(--rosso);
    flex: none;
  }
  .radio-title {
    display: block;
    font-size: 12.5px;
    color: var(--txt);
  }
  .radio-hint {
    display: block;
    font-size: 11px;
    color: var(--muted);
    line-height: 1.5;
    margin-top: 2px;
  }
  .duck-volume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    max-width: 340px;
    margin: 4px 0 4px 24px;
  }

  .error {
    margin: 12px 0;
    padding: 10px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 12px;
  }
  footer {
    margin-top: 24px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
  }
</style>
