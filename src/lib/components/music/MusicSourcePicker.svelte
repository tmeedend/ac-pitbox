<script lang="ts">
  // A music source picker (§16.8): one list — the default, the recent sources,
  // the Steam soundtracks —, Browse… for a folder or a file, and ▶ to listen.
  // The same control on every line that takes a music: MENU and GRID, each
  // screen, each term of Classement.
  //
  // It only reports the choice (`onchange`); writing it is the caller's job —
  // the Music settings save on their button, Classement writes at once.
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import ContextMenu from "$lib/components/ui/ContextMenu.svelte";
  import { musicConfig } from "$lib/music/context.svelte";
  import { listSteamSoundtracks, scanMusicSource, sourceName, type SourceInfo, type Soundtrack } from "$lib/music/music";
  import { previewing, togglePreview } from "$lib/music/preview.svelte";

  interface Props {
    /** The source chosen, `null` for the default. */
    value: string | null;
    /** What `null` means here: "Follows screen", "Menu ambience"… */
    defaultLabel: string;
    /** The source `null` plays, for ▶ — absent when the default has nothing
     * of its own to listen to (it follows another line). */
    defaultSource?: string | null;
    onchange: (source: string | null) => void;
    /** Volume of the preview, 0–1: the one being set, on the settings. */
    volume?: number;
    label?: string;
  }
  let { value, defaultLabel, defaultSource = null, onchange, volume, label }: Props = $props();

  // The Steam soundtracks are read at the first opening of a list, once per
  // run (§16.7) — never at mount: a screen of twenty pickers would scan every
  // Steam library on arrival.
  let steam = $state<Soundtrack[] | null>(null);
  let steamAsked = false;
  function askSteam() {
    if (steamAsked) return;
    steamAsked = true;
    listSteamSoundtracks()
      .then((list) => (steam = list))
      .catch((e) => {
        steamAsked = false;
        console.warn("music: Steam soundtracks not listed", e);
      });
  }

  const recent = $derived((musicConfig()?.recent_sources ?? []).filter((r) => r !== value));
  const inSteam = $derived(steam?.some((s) => s.path === value) ?? false);

  let info = $state<SourceInfo | null>(null);
  $effect(() => {
    const source = value ?? defaultSource;
    info = null;
    if (!source) return;
    scanMusicSource(source)
      .then((i) => {
        if ((value ?? defaultSource) === source) info = i;
      })
      .catch((e) => console.warn("music: source not scanned", e));
  });

  let menu = $state<{ x: number; y: number } | null>(null);
  let error = $state("");

  async function browse(directory: boolean) {
    const picked = await open({
      directory,
      multiple: false,
      defaultPath: value ?? undefined,
      filters: directory ? undefined : [{ name: t("music.audioFiles"), extensions: ["mp3", "flac", "ogg", "wav"] }],
    });
    if (typeof picked === "string") onchange(picked);
  }

  const playable = $derived(value ?? defaultSource);
  const playingHere = $derived(playable != null && previewing() === playable);

  async function listen() {
    error = "";
    if (!playable) return;
    try {
      await togglePreview(playable, volume ?? musicConfig()?.music_volume ?? 0.45);
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<div class="wrap">
<div class="picker">
  <select
    class="input"
    aria-label={label ?? defaultLabel}
    value={value ?? ""}
    onfocus={askSteam}
    onpointerdown={askSteam}
    onchange={(e) => onchange(e.currentTarget.value || null)}
  >
    <option value="">{defaultLabel}</option>
    {#if value && !inSteam}
      <!-- The source chosen, shown by its name whether or not it is still in
           the recent list. -->
      <option {value}>{sourceName(value)}</option>
    {/if}
    {#if recent.length}
      <optgroup label={t("music.recent")}>
        {#each recent as r (r)}<option value={r}>{sourceName(r)}</option>{/each}
      </optgroup>
    {/if}
    {#if steam?.length}
      <optgroup label={t("music.steamSoundtracks")}>
        {#each steam as s (s.path)}<option value={s.path}>{s.name}</option>{/each}
      </optgroup>
    {/if}
  </select>
  <button
    class="btn"
    type="button"
    onclick={(e) => {
      const r = e.currentTarget.getBoundingClientRect();
      // After the click has finished bubbling: the menu closes on any click
      // outside it, and opened right away it caught the very click that
      // opened it — Browse… then seemed to do nothing (reported).
      setTimeout(() => (menu = { x: r.left, y: r.bottom + 2 }));
    }}>{t("pathfield.browse")}</button
  >
  <button
    class="btn play"
    type="button"
    title={t("music.preview")}
    aria-pressed={playingHere}
    disabled={!info?.track_count}
    onclick={listen}>{playingHere ? "■" : "▶"}</button
  >
  <span class="meta mono" class:missing={info?.missing}>
    {#if info?.missing}{t("music.missing")}{:else if info}{t("music.trackCount", { count: info.track_count })}{/if}
  </span>
</div>
{#if error}<div class="errbox">{error}</div>{/if}
</div>

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    onclose={() => (menu = null)}
    items={[
      { label: t("music.browseFolder"), onclick: () => void browse(true) },
      { label: t("music.browseFile"), onclick: () => void browse(false) },
    ]}
  />
{/if}

<style>
  .wrap {
    min-width: 0;
  }
  .picker {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .picker select {
    flex: 0 1 280px;
    min-width: 0;
  }
  .play {
    flex: none;
    width: 38px;
    justify-content: center;
    padding: 8px 0;
  }
  .meta {
    font-size: 11px;
    color: var(--muted2);
    white-space: nowrap;
  }
  .meta.missing {
    color: var(--yellow);
  }
  .errbox {
    margin-top: 6px;
  }
</style>
