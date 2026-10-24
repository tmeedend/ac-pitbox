<script lang="ts">
  // The Scenes screen's preview (SPEC-story-mode, Create Your Own: Scenes):
  // the place as the player sees it at the moment the author sets - season,
  // time of day, weather, mood - with the phone over it, or alone as behind
  // a dialogue. The variant is the engine's pick (`pickVariant`), on the
  // place as the screen holds it, so it follows each edit before it is
  // saved. Silent: the ambient sound is named, not played.
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import type { SceneShown } from "$lib/story/play";
  import {
    PERIODS,
    SEASONS,
    pickVariant,
    storyNames,
    type Period,
    type SceneMoment,
    type ScenePlace,
  } from "$lib/story/scenes";
  import { studio } from "$lib/story/studioState.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import PhoneFrame from "./player/PhoneFrame.svelte";
  import SceneView from "./player/SceneView.svelte";

  interface Props {
    place: ScenePlace;
  }
  const { place }: Props = $props();

  const story = $derived(studio.story!);
  const weathers = $derived(storyNames(story.scenes.value, story.chapters.map((c) => c.chapter), "weather"));
  const moods = $derived(storyNames(story.scenes.value, story.chapters.map((c) => c.chapter), "mood"));

  /** A clock the phone shows for each time of day: inside its span. */
  const CLOCK: Record<Period, string> = {
    morning: "09:00",
    afternoon: "15:00",
    evening: "19:30",
    night: "23:30",
  };

  const moment = $state<SceneMoment>({ season: "spring", period: "morning", weather: "", mood: "" });
  let phone = $state(true);
  let shown = $state<SceneShown | null>(null);
  let problem = $state<string | null>(null);

  $effect(() => {
    // Read whole first: any edit of the place, or of the moment, asks again.
    const [p, m] = [$state.snapshot(place), $state.snapshot(moment)];
    let current = true;
    pickVariant(p, m)
      .then((s) => {
        if (!current) return;
        shown = s;
        problem = null;
      })
      .catch((e) => {
        if (current) problem = errorText(e);
      });
    return () => {
      current = false;
    };
  });
</script>

<aside class="preview">
  <div class="lbl">{t("story.preview.title")}</div>
  <div class="moment">
    <Seg
      items={SEASONS.map((s) => ({ value: s, label: t(`story.scenes.seasons.${s}`) }))}
      tone="neutral"
      value={moment.season}
      onselect={(v) => (moment.season = v as SceneMoment["season"])}
    />
    <Seg
      items={PERIODS.map((p) => ({ value: p, label: t(`story.scenes.periods.${p}`) }))}
      tone="neutral"
      value={moment.period}
      onselect={(v) => (moment.period = v as Period)}
    />
    <div class="pair">
      <label class="pick">
        <span class="lbl-key">{t("story.scenes.weather")}</span>
        <select class="input mono" bind:value={moment.weather}>
          <option value="">{t("story.scenes.unset")}</option>
          {#each weathers as w (w)}<option value={w}>{w}</option>{/each}
        </select>
      </label>
      <label class="pick">
        <span class="lbl-key">{t("story.scenes.mood")}</span>
        <select class="input mono" bind:value={moment.mood}>
          <option value="">{t("story.scenes.unset")}</option>
          {#each moods as m (m)}<option value={m}>{m}</option>{/each}
        </select>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={phone} />
        {t("story.scenes.withPhone")}
      </label>
    </div>
  </div>

  {#if problem}
    <p class="errbox">{t("story.preview.unavailable", { reason: problem })}</p>
  {:else}
    <div class="view">
      <SceneView scene={shown} folder={story.path} staged={!phone} transition={null} />
      {#if phone}
        <div class="phone">
          <PhoneFrame clock={CLOCK[moment.period]} date="" title="—" avatar={null}>
            {#snippet children()}{/snippet}
          </PhoneFrame>
        </div>
      {/if}
    </div>
    <p class="what">
      {#if shown}
        <span class="mono">{shown.variant}</span> · <span class="mono">{shown.file || "—"}</span>
        {#if shown.ambient}· {t("story.scenes.ambientNamed", { file: shown.ambient })}{/if}
      {:else}
        {t("story.scenes.nothingShown")}
      {/if}
    </p>
  {/if}
</aside>

<style>
  .preview {
    width: 560px;
    flex-shrink: 0;
  }
  .moment {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 8px 0 12px;
  }
  .pair {
    display: flex;
    gap: 14px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  /* The player's 16:9, with the frame at its scale: the phone drawn at its
     size, scaled down to fit the view. */
  .view {
    position: relative;
    aspect-ratio: 16 / 9;
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .phone {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%) scale(0.45);
  }
  .what {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--txt2);
  }
  .errbox {
    margin: 0;
  }
</style>
