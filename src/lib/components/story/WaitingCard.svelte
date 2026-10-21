<script lang="ts">
  // What the story waits for, on the scene beside the frame (SPEC-story-mode,
  // Story Mode: player side): an action to do, a reply awaited, the end, and
  // an event's summary and its launch (`EventCard`) when there is no frame to
  // hold it.
  import { t } from "$lib/i18n/index.svelte";
  import type { Waiting } from "$lib/story/play";
  import type { Character, Driver } from "$lib/story/studio";
  import EventCard from "./EventCard.svelte";

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

  function characterName(id: string | null): string {
    if (!id) return t("story.play.unknownSender");
    return characters.find((c) => c.id === id)?.name ?? id;
  }
</script>

<section class="card">
  {#if waiting.kind === "event"}
    <EventCard
      {waiting}
      {characters}
      {drivers}
      {busy}
      {money}
      {launched}
      {collecting}
      {onlaunch}
      {ongiveup}
    />
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
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
  }
  .btn {
    align-self: flex-start;
  }
</style>
