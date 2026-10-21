<script lang="ts">
  // An event's summary before its session (SPEC-story-mode, Story Mode:
  // player side): what, where, in what car, for what, at what cost, and its
  // launch. A session launched shows as under way until the game closes;
  // then, without a result - closed early, never started, a launch that
  // failed - it can be launched again (no fee: the attempt is paid) or given
  // up, which counts as not finishing.
  import { onMount } from "svelte";
  import { gameSession } from "$lib/launch/gameSession.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { driverLabel } from "$lib/story/drivers";
  import { cardOf, loadLibrary } from "$lib/story/library.svelte";
  import { goals } from "$lib/story/player/game";
  import type { Waiting } from "$lib/story/play";
  import type { Character, Driver } from "$lib/story/studio";

  interface Props {
    waiting: Extract<Waiting, { kind: "event" }>;
    characters: Character[];
    drivers: Driver[];
    busy: boolean;
    /** The money the player has, when the story has an economy. */
    money: number | null;
    /** The event a session was launched for and has not come back from. */
    launched: string | null;
    /** The session's result is being read. */
    collecting: boolean;
    onlaunch: () => void;
    ongiveup: () => void;
  }
  const { waiting, characters, drivers, busy, money, launched, collecting, onlaunch, ongiveup }: Props = $props();

  onMount(() => void loadLibrary());

  const track = $derived(cardOf("Track", waiting.session.track));
  const car = $derived(cardOf("Car", waiting.session.car));

  function driverName(id: string): string {
    const d = drivers.find((x) => x.id === id);
    return d ? driverLabel(d, characters) : id;
  }
</script>

<div class="event">
  <div class="lbl">{t(`story.events.type_${waiting.event_type}`)}</div>
  <div class="title">{waiting.name ?? waiting.event}</div>
  <dl>
    <dt>{t("story.events.track")}</dt>
    <dd>{track?.display_name ?? waiting.session.track ?? "—"}</dd>
    <dt>{t("story.events.car")}</dt>
    <dd>{car?.display_name ?? waiting.session.car ?? "—"}</dd>
    {#if waiting.session.laps}
      <dt>{t("story.events.laps")}</dt>
      <dd>{waiting.session.laps}</dd>
    {/if}
    {#if waiting.fee !== null}
      <dt>{t("story.events.entryFee")}</dt>
      <dd>{waiting.fee}</dd>
    {/if}
  </dl>
  <div class="lbl-key">{t("story.play.goal")}</div>
  <ul class="goals">
    {#each goals(waiting.success, driverName) as g, i (i)}
      <li>{t(g.key, g.values)}</li>
    {/each}
  </ul>
  {#if waiting.attempt > 1}
    <p class="attempt">{t("story.play.attempt", { n: waiting.attempt })}</p>
  {/if}
  {#if launched === waiting.event && gameSession.phase !== "idle"}
    <p class="note">{t("story.play.racing")}</p>
  {:else if launched === waiting.event && !collecting}
    <p class="note">{t("story.play.noResult")}</p>
    <span class="buttons">
      <button class="btn btn-primary" disabled={busy} onclick={onlaunch}>{t("story.play.launchAgain")}</button>
      <button class="btn" disabled={busy} onclick={ongiveup}>{t("story.play.giveUp")}</button>
    </span>
  {:else if launched !== waiting.event}
    {@const short = waiting.fee !== null && money !== null && money < waiting.fee}
    {#if short}<p class="note">{t("story.play.cannotAfford", { fee: waiting.fee ?? 0, money: money ?? 0 })}</p>{/if}
    <button class="btn btn-primary" disabled={busy || short || gameSession.phase !== "idle"} onclick={onlaunch}
      >{t("story.play.launch")}</button
    >
  {/if}
</div>

<style>
  .event {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .title {
    font-size: 16px;
    font-weight: 600;
    color: var(--txt);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
    font-size: 12.5px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    color: var(--txt);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .goals {
    margin: 0;
    padding-left: 18px;
    font-size: 12.5px;
    color: var(--txt);
  }
  .attempt,
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .btn {
    align-self: flex-start;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
</style>
