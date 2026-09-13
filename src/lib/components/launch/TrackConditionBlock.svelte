<script lang="ts">
  // "Track condition" block of the right rail (§2.1/§2.2).
  //
  // **Why it lives next to the weather, and must stay there.** The first entry
  // of the list is `Auto (set by weather)` — it is not a track state but the
  // `WeatherDefined` flag, which hands the decision to the weather. An entry
  // that names its neighbour only reads right while that neighbour is in sight:
  // this block and the weather are adjacent by necessity, not by layout
  // convenience. Nothing goes between them.
  //
  // **A select, not a seven-row list.** Seven named rows down the rail cost the
  // height of the weather block for a setting one picks once; and the four real
  // numbers — the ones that actually decide how the track evolves — had nowhere
  // to be read except a hover tooltip, which is to say nowhere.
  //
  // Read-only: editing the four values and reading Content Manager's own user
  // presets is a separate matter.
  import { nearestGrip, type RaceSetup, type TrackStateOption } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";

  let { setup, states }: { setup: RaceSetup; states: TrackStateOption[] } = $props();

  // Realigned on the offered list: a preset saved before the list matched the
  // game's own table can carry a value that is no longer in it (92 % existed,
  // invented), and the select would then mark none of them as current.
  const current = $derived(states.find((s) => s.start === nearestGrip(setup.grip)) ?? states[0]);
</script>

<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.trackConditionLabel")}</span></header>
  <div class="blk-b">
    <select class="input" value={String(setup.grip)} onchange={(e) => (setup.grip = Number(e.currentTarget.value))}>
      {#each states as s (s.start)}
        <option value={String(s.start)}>{s.name}</option>
      {/each}
    </select>

    {#if current}
      <!-- The four values of `cfg/templates/tracks.ini`, in Content Manager's
           own vocabulary: someone who set a track state there must recognise
           what they are reading here. -->
      <p class="vals mono">
        {t("launch.initialGrip")}&nbsp;{current.start}%&nbsp; · &nbsp;{t("launch.gripTransfer")}&nbsp;{current.transfer}%&nbsp;
        · &nbsp;{t("launch.randomization")}&nbsp;{current.randomness}%&nbsp; · &nbsp;{t("launch.lapGain")}&nbsp;{current.lap_gain}
        {t("launch.lapsUnit")}
      </p>
      {#if current.weather_defined}
        <!-- Not a warning: nothing is wrong and nothing is to be done. It says
             what the four numbers above mean for this entry — they are Green's,
             and Green is where the game lands when the weather says nothing. -->
        <p class="fallback">{t("launch.gripWeatherFallback")}</p>
      {/if}
    {/if}
  </div>
</section>

<style>
  .blk-b select {
    width: 100%;
  }
  /* Mono and the readable grey: these are data, and they are meant to be read,
     not glanced past. Wrapping is expected in a rail this narrow — the
     separators carry the grouping so a wrapped line stays legible. */
  .vals {
    margin-top: 8px;
    font-size: 9px;
    line-height: 1.6;
    color: var(--muted);
  }
  .fallback {
    margin-top: 6px;
    font-size: 10.5px;
    line-height: 1.45;
    color: var(--faint);
  }
</style>
