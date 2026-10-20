<script lang="ts">
  // What the story waits for, on the scene beside the frame (SPEC-story-mode,
  // Story Mode: player side): an event's summary (what, where, in what car,
  // for what, at what cost), an action to do, a reply awaited, or the end.
  // The launch of an event comes with the races (L3.2).
  import { onMount } from "svelte";
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
    onaction: (action: string) => void;
    onleave: () => void;
  }
  const { waiting, characters, drivers, senderOf, busy, onaction, onleave }: Props = $props();

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
</style>
