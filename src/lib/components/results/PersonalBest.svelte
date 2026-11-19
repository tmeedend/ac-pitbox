<script lang="ts">
  // The record on the car and layout of the session, beside Start
  // (RESULTAT§9). No record: nothing at all. A click opens the history on that
  // combo. Read again when a session ends.
  import { onMount } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { nav } from "$lib/shell/nav.svelte";
  import { onSessionEnded, personalBest, type Combo } from "$lib/results/results";
  import { fmtAgo, fmtLap } from "$lib/results/returnView";
  import { openHistory } from "$lib/results/historyState.svelte";

  let best = $state<{ best_lap_ms: number; at_ms: number } | null>(null);
  /** Bumped when a session ends: the record may have moved. */
  let ended = $state(0);

  const combo = $derived<Combo | null>(
    nav.sessionCar?.id && nav.sessionTrack?.id
      ? { car_id: nav.sessionCar.id, track_id: nav.sessionTrack.id, track_layout: nav.sessionTrack.layout || null }
      : null,
  );

  $effect(() => {
    const c = combo;
    void ended;
    best = null;
    if (!c) return;
    personalBest(c)
      .then((b) => {
        if (combo === c) best = b;
      })
      .catch((e) => console.warn("personal_best", e));
  });

  onMount(() => {
    let stop: (() => void) | null = null;
    void onSessionEnded(() => (ended += 1)).then((off) => (stop = off));
    return () => stop?.();
  });
</script>

{#if best && combo}
  <button class="pb" type="button" title={new Date(best.at_ms).toLocaleString()} onclick={() => void openHistory(combo)}>
    {t("results.pb", { time: fmtLap(best.best_lap_ms), ago: fmtAgo(best.at_ms, Date.now(), i18n.locale) })}
  </button>
{/if}

<style>
  .pb {
    align-self: center;
    background: none;
    border: none;
    padding: 2px 6px;
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--muted);
    cursor: pointer;
  }
  .pb:hover {
    color: var(--txt);
    text-decoration: underline;
  }
</style>
