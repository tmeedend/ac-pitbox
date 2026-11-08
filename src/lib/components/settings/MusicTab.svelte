<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    getMusicConfig,
    saveMusicConfig,
    getDefaultMusicFolders,
    emptyMusicConfig,
    type MusicConfig,
    type DefaultMusicFolders,
    type PlayMode,
  } from "$lib/music/music";
  import { musicConfigSaved } from "$lib/music/context.svelte";
  import { stopPreview } from "$lib/music/preview.svelte";
  import { SCREEN_KEYS, withAssignment, withRecent, type ScreenKey, type Taxonomy } from "$lib/music/resolve";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import Slider from "$lib/components/ui/Slider.svelte";
  import MusicSourcePicker from "$lib/components/music/MusicSourcePicker.svelte";
  import MusicAssignments from "$lib/components/music/MusicAssignments.svelte";

  // Fichier séparé de AppConfig (music.json, §16.1) : état et sauvegarde
  // propres à cet onglet, indépendants de la garde de navigation de
  // Settings.svelte (qui ne porte que sur AppConfig). Tout ce qui se règle ici
  // part au bouton Enregistrer, musiques par écran et suppression d'une
  // orpheline comprises ; les lignes du Classement, elles, écrivent aussitôt.
  let config = $state<MusicConfig>(emptyMusicConfig());
  let defaults = $state<DefaultMusicFolders>({ menu: "", grid: "" });
  let saving = $state(false);
  let saved = $state(false);
  let error = $state("");

  onMount(async () => {
    try {
      const [cfg, def] = await Promise.all([getMusicConfig(), getDefaultMusicFolders()]);
      config = cfg;
      defaults = def;
    } catch (e) {
      error = errorText(e);
    }
  });

  onDestroy(() => void stopPreview());

  const MODES: { value: PlayMode; key: string }[] = [
    { value: "off", key: "music.modeOff" },
    { value: "big_picture", key: "music.modeBigPicture" },
    { value: "always", key: "music.modeAlways" },
  ];

  /** The label of each screen: the rail's where it has one. */
  const SCREEN_LABEL: Record<ScreenKey, string> = {
    cars: "rules.tabCars",
    tracks: "rules.tabTracks",
    driver: "driver.title",
    online: "nav.online",
    apps: "nav.apps",
    others: "nav.others",
    sorting: "nav.sorting",
    files: "nav.files",
    settings: "nav.settings",
    about: "nav.about",
  };

  function pickBase(which: "menu_folder" | "grid_folder", source: string | null) {
    config[which] = source;
    if (source) config.recent_sources = withRecent(config.recent_sources, source);
  }

  function pickScreen(key: ScreenKey, source: string | null) {
    config.assignments = withAssignment(config.assignments, "screen", key, source);
    if (source) config.recent_sources = withRecent(config.recent_sources, source);
  }

  function removeAssignment(taxonomy: Taxonomy, id: string) {
    config.assignments = withAssignment(config.assignments, taxonomy, id, null);
  }

  async function save() {
    saving = true;
    error = "";
    try {
      await saveMusicConfig(config);
      musicConfigSaved(config);
      saved = true;
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

<section class="lang-section">
  <label>
    <span>{t("music.playMode")}</span>
    <select class="input" bind:value={config.play_mode}>
      {#each MODES as m (m.value)}<option value={m.value}>{t(m.key)}</option>{/each}
    </select>
  </label>
</section>

<section class="lang-section">
  <label class="check">
    <input type="checkbox" bind:checked={config.use_custom_folders} />
    <span>{t("music.useCustomFolders")}</span>
  </label>
  <p class="hint">{t("music.useCustomFoldersHint")}</p>
</section>

{#if config.use_custom_folders}
  <section class="folder-block">
    <div class="label">{t("music.menuAmbience")}</div>
    <MusicSourcePicker
      value={config.menu_folder}
      defaultLabel={t("music.defaultFolder")}
      defaultSource={defaults.menu}
      volume={config.music_volume}
      label={t("music.menuAmbience")}
      onchange={(s) => pickBase("menu_folder", s)}
    />
  </section>

  <section class="folder-block">
    <div class="label">{t("music.gridAmbience")}</div>
    <MusicSourcePicker
      value={config.grid_folder}
      defaultLabel={t("music.defaultFolder")}
      defaultSource={defaults.grid}
      volume={config.music_volume}
      label={t("music.gridAmbience")}
      onchange={(s) => pickBase("grid_folder", s)}
    />
  </section>
{/if}

<section class="lang-section">
  <label class="check">
    <input type="checkbox" bind:checked={config.shuffle} />
    <span>{t("music.shuffle")}</span>
  </label>
</section>

<section class="lang-section">
  <label class="check">
    <input type="checkbox" bind:checked={config.show_now_playing} />
    <span>{t("music.showNowPlaying")}</span>
  </label>
</section>

<section class="lang-section">
  <Slider
    label={t("music.volume")}
    value={pct(config.music_volume)}
    min={0}
    max={100}
    display="{pct(config.music_volume)}%"
    oninput={(v) => (config.music_volume = v / 100)}
  />
</section>

<section class="lang-section">
  <Slider
    label={t("music.crossfade")}
    value={config.crossfade_ms}
    min={500}
    max={6000}
    step={250}
    display="{fmtSeconds(config.crossfade_ms)} s"
    oninput={(v) => (config.crossfade_ms = v)}
  />
</section>

{#if config.use_custom_folders}
  <section class="blk">
    <div class="blk-h"><span class="blk-t">{t("music.byScreen")}</span></div>
    <div class="blk-b screens">
      {#each SCREEN_KEYS as key (key)}
        <span class="screen">{t(SCREEN_LABEL[key])}</span>
        <MusicSourcePicker
          value={config.assignments.screen[key] ?? null}
          defaultLabel={t("music.menuAmbience")}
          volume={config.music_volume}
          label={t(SCREEN_LABEL[key])}
          onchange={(s) => pickScreen(key, s)}
        />
      {/each}
    </div>
  </section>

  <section class="blk">
    <div class="blk-h"><span class="blk-t">{t("music.assignments")}</span></div>
    <div class="blk-b">
      <MusicAssignments assignments={config.assignments} onremove={removeAssignment} />
    </div>
  </section>
{/if}

{#if error}<div class="errbox">{error}</div>{/if}

<footer>
  {#if saved}<span class="pill pill-ok">{t("settings.saved")}</span>{/if}
  <button class="btn btn-primary" type="button" onclick={save} disabled={saving}>
    {saving ? t("settings.saving") : t("settings.save")}
  </button>
</footer>

<style>
  /* Repris de Settings.svelte : le CSS Svelte est scopé par composant, ces
     classes ne traversent pas depuis l'écran parent (§ conventions projet). */
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
  .screens {
    display: grid;
    grid-template-columns: minmax(90px, max-content) minmax(0, 1fr);
    align-items: center;
    gap: 8px 16px;
  }
  .screen {
    font-size: 12px;
    color: var(--txt2);
  }
  .hint {
    margin-top: 8px;
    font-size: 11px;
    color: var(--faint);
  }

  .errbox {
    margin: 12px 0;
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
