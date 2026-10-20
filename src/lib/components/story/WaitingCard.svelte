<script lang="ts">
  // What the story waits for, on the scene beside the frame (SPEC-story-mode,
  // Story Mode: player side): an event's summary (what, where, in what car,
  // for what, at what cost) and its launch, an action to do, a reply
  // awaited, or the end. A session launched shows as under way until the
  // game closes; then, without a result - closed early, never started, a
  // launch that failed - it can be launched again (no fee: the attempt is
  // paid) or given up, which counts as not finishing.
  import { onMount } from "svelte";
  import { gameSession } from "$lib/launch/gameSession.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { driverLabel } from "$lib/story/drivers";
  import { cardOf, loadLibrary } from "$lib/story/library.svelte";
  import { goals } from "$lib/story/player/game";
  import type { Waiting } from "$lib/story/play";
  import type { Character, Driver } from "$lib/story/studio";

  interface Props {
    waiting: Waiting;
    characters: Character[];
    drivers: Driver[];
    /** Who a reply is awaited from, by content item. */
    senderOf: (content: string) => string | null;
    busy: boolean;
    /** The money the player has, when the story has an economy. */
    money: number | null;
    /** The event a session was launched for and has not come back from. */
    launched: string | null;
    /** The session's result is being read. */
    collecting: boolean;
    onaction: (action: string) => void;
    onlaunch: () => void;
    ongiveup: () => void;
    onleave: () => void;
  }
  const {
    waiting,
    characters,
    drivers,
    senderOf,
    busy,
    money,
    launched,
    collecting,
    onaction,
    onlaunch,
    ongiveup,
    onleave,
  }: Props = $props();

  onMount(() => void loadLibrary());

  function driverName(id: string): string {
    const d = drivers.find((x) => x.id === id);
    return d ? driverLabel(d, characters) : id;
  }

  function characterName(id: string | null): string {
    if (!id) return t("story.play.unknownSender");
    return characters.find((c) => c.id === id)?.name ?? id;
  }
</script>

<section class="card">
  {#if waiting.kind === "event"}
    {@const track = cardOf("Track", waiting.session.track)}
    {@const car = cardOf("Car", waiting.session.car)}
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
  {:else if waiting.kind === "action"}
    <div class="title">{waiting.action}</div>
    <button class="btn" disabled={busy} onclick={() => onaction(waiting.action)}>{t("story.play.actionDone")}</button>
  {:else if waiting.kind === "reply"}
    <p class="note">{t("story.play.waitsReply", { name: characterName(senderOf(waiting.content)) })}</p>
  {:else}
    <div class="lbl">{t("story.play.theEnd")}</div>
    {#if waiting.ending}<div class="title">{waiting.ending}</div>{/if}
    <button class="btn" onclick={onleave}>{t("story.play.backToHub")}</button>
  {/if}
</section>

<style>
  .card {
    width: 300px;
    padding: 16px 18px;
    border: 1px solid var(--line);
    background: var(--card);
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
