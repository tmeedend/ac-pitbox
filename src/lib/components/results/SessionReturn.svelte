<script lang="ts">
  // The session just driven, shown when the game closes (RESULTAT§4). In
  // the place of the library, the session column beside it: not a modal —
  // going anywhere else closes it. What it says depends on the session
  // (`returnView.ts`); the replay and the screenshots are computed from its
  // times, never stored (RESULTAT§6).
  import { onMount, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { nav, requestSection } from "$lib/shell/nav.svelte";
  import { getModDetail, previewSrc } from "$lib/library/library";
  import { loadThumbnails } from "$lib/detail/thumbnails";
  import { CM_SETTLE_MS, keepReplay, launchReplay } from "$lib/detail/media";
  import { canStartSession, markStarting } from "$lib/launch/gameSession.svelte";
  import { LaunchSequence } from "$lib/launch/launchSequence.svelte";
  import { PAUSE_SESSION, pauseGridThumbs } from "$lib/gridthumbs/gridThumbs.svelte";
  import SteamPrompt from "$lib/components/launch/SteamPrompt.svelte";
  import Lightbox, { type LightboxItem } from "$lib/components/ui/Lightbox.svelte";
  import {
    launchAgain,
    sessionMedia,
    sessionReturn,
    type ReturnView,
    type SessionMedia,
  } from "$lib/results/results";
  import { fmtGap, fmtLap, returnContent, type LapLine } from "$lib/results/returnView";

  let view = $state<ReturnView | null>(null);
  let media = $state<SessionMedia>({ replay: null, screenshots: [] });
  let names = $state<{ car: string; track: string }>({ car: "", track: "" });
  let thumbs = $state<Record<string, string>>({});
  let lightboxIndex = $state<number | null>(null);
  let error = $state("");
  let keeping = $state(false);
  let kept = $state(false);
  let againButton = $state<HTMLButtonElement | null>(null);

  const content = $derived(returnContent(view?.result ?? null));
  const modeLabel = $derived(view ? t(`results.mode.${view.mode}`) : "");

  async function load(launchId: number) {
    error = "";
    try {
      const v = await sessionReturn(launchId);
      if (nav.resultLaunch !== launchId) return;
      view = v;
      if (!v) return;
      const [car, track] = await Promise.all([
        v.car_id ? getModDetail(v.car_id).catch(() => null) : null,
        v.track_id ? getModDetail(v.track_id).catch(() => null) : null,
      ]);
      names = { car: car?.display_name ?? v.car_id ?? "", track: track?.display_name ?? v.track_id ?? "" };
    } catch (e) {
      error = errorText(e);
    }
  }

  async function loadMedia(launchId: number) {
    try {
      const m = await sessionMedia(launchId);
      if (nav.resultLaunch === launchId) media = m;
    } catch (e) {
      console.warn("session_media", e);
    }
  }

  $effect(() => {
    const id = nav.resultLaunch;
    if (id == null) return;
    untrack(() => {
      view = null;
      media = { replay: null, screenshots: [] };
      kept = false;
      void load(id);
      void loadMedia(id);
    });
    // The replay lands as the game closes, and CM may rename it a few seconds
    // later: looked for again then, as the media tab does (§6.1). Not there
    // by then, no button and nothing said.
    const again = setTimeout(() => void loadMedia(id), CM_SETTLE_MS);
    return () => clearTimeout(again);
  });

  $effect(() => {
    const id = nav.resultLaunch;
    const paths = media.screenshots.map((s) => s.path);
    loadThumbnails(
      paths,
      () => nav.resultLaunch !== id,
      (path, src) => (thumbs = { ...thumbs, [path]: src }),
    );
  });

  // The main action has the focus: A on the controller is "Again".
  $effect(() => {
    if (view?.can_again && againButton) untrack(() => againButton?.focus());
  });

  const sequence = new LaunchSequence({
    canLaunch: () => !!view?.can_again,
    run: async () => {
      const id = view?.launch_id;
      if (id == null) return;
      error = "";
      // As every launch does (`launchSession`): the machine goes to the game.
      pauseGridThumbs(PAUSE_SESSION);
      try {
        await launchAgain(id);
        markStarting();
      } catch (e) {
        error = errorText(e);
      }
    },
  });

  /** Keeps the track and opens the car library. */
  function changeCar() {
    void requestSection("cars");
  }

  async function watchReplay() {
    if (!media.replay) return;
    error = "";
    try {
      await launchReplay(media.replay.path);
      markStarting();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function keep() {
    const replay = media.replay;
    if (!replay || keeping) return;
    keeping = true;
    error = "";
    try {
      const path = await keepReplay(replay.path);
      media = { ...media, replay: { ...replay, path, autosave: false, file_name: path.split(/[\\/]/).pop() ?? path } };
      kept = true;
    } catch (e) {
      error = errorText(e);
    } finally {
      keeping = false;
    }
  }

  const lightboxItems = $derived<LightboxItem[]>(
    media.screenshots.map((s) => ({ src: previewSrc(s.path) ?? "", caption: s.file_name })),
  );

  onMount(() => () => {
    // Leaving the screen closes it: the next visit is the next session's.
    if (nav.section !== "result") nav.resultLaunch = null;
  });
</script>

<div class="return">
  <header class="head">
    <div>
      <h1 class="lbl-screen">{modeLabel}</h1>
      {#if view}
        <p class="combo">{[names.car, names.track].filter(Boolean).join(" · ")}</p>
      {/if}
    </div>
  </header>

  {#if error}
    <p class="errbox">{error}</p>
  {/if}

  {#if view}
    {#if content.kind === "race"}
      <table class="board">
        <tbody>
          {#each content.lines as line (line.standing.car)}
            <tr class:me={line.isPlayer} class:out={line.standing.dnf}>
              <td class="pos mono">{line.standing.position}</td>
              <td class="who">{line.standing.driver_name}</td>
              <td class="mono">{fmtLap(line.standing.best_lap_ms)}</td>
              <td class="mono gap">
                {#if line.standing.dnf}
                  {t("results.dnf")}
                {:else if line.gap && "laps" in line.gap}
                  {t("results.lapsDown", { count: line.gap.laps })}
                {:else if line.gap}
                  {fmtGap(line.gap.ms)}
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if content.kind === "laps"}
      <div class="best">
        <span class="lbl">{t("results.bestLap")}</span>
        <span class="time mono">{fmtLap(content.bestMs)}</span>
        {#if view.new_best}
          <span class="pill pb">{t("results.newBest")}</span>
        {:else if view.previous_best_ms != null}
          <span class="mono vs" title={t("results.personalBest", { time: fmtLap(view.previous_best_ms) })}>
            {fmtGap(content.bestMs - view.previous_best_ms)}
          </span>
        {/if}
      </div>
      {@render lapList(content.laps)}
    {:else}
      <p class="quiet">
        {content.count > 0
          ? t("results.lineNoValidLap", { mode: modeLabel, count: content.count })
          : t("results.lineNoLap", { mode: modeLabel })}
      </p>
      {@render lapList(content.laps)}
    {/if}

    {#if media.screenshots.length > 0}
      <div class="shots">
        {#each media.screenshots as shot, i (shot.path)}
          <button class="shot" type="button" onclick={() => (lightboxIndex = i)} title={shot.file_name}>
            {#if thumbs[shot.path]}<img src={thumbs[shot.path]} alt="" />{/if}
          </button>
        {/each}
      </div>
    {/if}

    <div class="actions">
      {#if view.can_again}
        <button
          class="btn btn-primary"
          type="button"
          bind:this={againButton}
          disabled={!canStartSession() || sequence.launching}
          onclick={() => void sequence.launch()}>{t("results.again")}</button
        >
      {/if}
      {#if view.kind === "session"}
        <button class="btn" type="button" onclick={changeCar}>{t("results.changeCar")}</button>
      {/if}
      {#if media.replay}
        <button class="btn" type="button" disabled={!canStartSession()} onclick={() => void watchReplay()}
          >{t("results.watchReplay")}</button
        >
        {#if media.replay.autosave}
          <button class="btn" type="button" disabled={keeping} onclick={() => void keep()}>{t("results.keep")}</button>
        {:else if kept}
          <span class="pill">{t("results.kept")}</span>
        {/if}
      {/if}
    </div>
  {/if}
</div>

{#snippet lapList(laps: LapLine[])}
  {#if laps.length > 0}
    <ol class="laps">
      {#each laps as lap (lap.number)}
        <li class:cut={lap.cut} class:top={lap.best}>
          <span class="n mono">{lap.number}</span>
          <span class="mono">{fmtLap(lap.time_ms)}</span>
          {#if lap.cut}<span class="cutmark">{t("results.cut")}</span>{/if}
        </li>
      {/each}
    </ol>
  {/if}
{/snippet}

{#if lightboxIndex !== null}
  <Lightbox items={lightboxItems} startIndex={lightboxIndex} onclose={() => (lightboxIndex = null)} />
{/if}
<SteamPrompt {sequence} />

<style>
  .return {
    display: flex;
    flex-direction: column;
    gap: 18px;
    max-width: 760px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }
  .combo {
    color: var(--muted);
    margin: 4px 0 0;
  }
  .board {
    border-collapse: collapse;
    width: 100%;
  }
  .board td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .board tr.me td {
    color: var(--rosso);
    font-weight: 600;
  }
  .board tr.out td {
    color: var(--faint);
  }
  .board .pos {
    width: 2.5em;
    text-align: right;
  }
  .board .gap {
    text-align: right;
    color: var(--muted);
  }
  .best {
    display: flex;
    align-items: baseline;
    gap: 14px;
  }
  .best .time {
    font-size: 28px;
    font-weight: 600;
  }
  .best .vs {
    color: var(--muted);
  }
  .pill.pb {
    border-color: var(--rosso);
    color: var(--rosso);
  }
  .laps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 4px 18px;
  }
  .laps li {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .laps .n {
    color: var(--faint);
    width: 2em;
    text-align: right;
  }
  .laps li.cut {
    color: var(--faint);
  }
  .laps li.top {
    color: var(--rosso);
    font-weight: 600;
  }
  .cutmark {
    font-size: 11px;
    color: var(--yellow);
  }
  .quiet {
    color: var(--muted);
  }
  .shots {
    display: flex;
    gap: 8px;
    overflow-x: auto;
  }
  .shot {
    flex: none;
    width: 160px;
    aspect-ratio: 16 / 9;
    padding: 0;
    border: 1px solid var(--line);
    background: var(--panel);
    cursor: pointer;
  }
  .shot img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
  }
</style>
