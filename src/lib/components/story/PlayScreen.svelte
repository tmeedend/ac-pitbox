<script lang="ts">
  // A game of Story Mode (SPEC-story-mode, Story Mode: player side): the
  // three layers - the scene (a plain background until the story's scenes
  // exist), the frame over it (the phone), and the dialogue layer over both
  // while a dialogue plays - with what the story waits for beside the frame.
  // Every move is saved as it is played, so leaving loses nothing.
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { currentDialogue } from "$lib/story/player/game";
  import { dateLabel } from "$lib/story/player/phone";
  import { speakersOf } from "$lib/story/player/speakers";
  import { closeGame, move, play } from "$lib/story/playState.svelte";
  import DialogueStage from "./player/DialogueStage.svelte";
  import PhoneScreen from "./PhoneScreen.svelte";
  import WaitingCard from "./WaitingCard.svelte";

  const game = $derived(play.game!);
  const view = $derived(game.view);
  const dialogue = $derived(currentDialogue(view.items));
  const date = $derived(dateLabel(view.date, view.date_display, t, i18n.locale));

  const speakers = $derived(speakersOf(game.characters, game.folder));

  function senderOf(content: string): string | null {
    return [...view.items].reverse().find((i) => i.content === content)?.from ?? null;
  }

  /** Back to the hub, whose list is read again: nothing to save, every
   * move already is. */
  function leave() {
    closeGame();
  }
</script>

<div class="game">
  <header class="bar">
    <button class="btn btn-ghost" onclick={leave}>‹ {t("story.play.leave")}</button>
    <span class="title">{view.title}</span>
    <span class="stats">
      {#if date}<span>{date}</span>{/if}
      {#if view.money !== null}<span>{t("story.play.money", { n: view.money })}</span>{/if}
      <span>{t("story.play.reputation", { n: view.reputation })}</span>
    </span>
  </header>
  {#if play.problem}<p class="errbox">{play.problem}</p>{/if}

  <div class="scene">
    <WaitingCard
      waiting={view.waiting}
      characters={game.characters}
      drivers={game.drivers}
      {senderOf}
      busy={play.busy}
      onaction={(action) => void move({ kind: "action_done", action })}
      onleave={leave}
    />
    {#if view.frame !== "none"}
      <div class="device">
        <PhoneScreen
          items={view.items}
          clock={view.clock}
          {date}
          {speakers}
          onreply={(content, reply) => void move({ kind: "reply", content, reply })}
          onseen={(content) => void move({ kind: "seen", content })}
        />
      </div>
    {/if}

    {#if dialogue !== null}
      {@const d = view.items[dialogue]}
      <div class="layer">
        {#key dialogue}
          <DialogueStage
            steps={d.steps}
            choices={d.choices}
            present={d.characters}
            from={d.from}
            {speakers}
            ended={!d.open}
            onchoose={(reply) => void move({ kind: "reply", content: d.content, reply })}
            onend={() => void move({ kind: "seen", content: d.content })}
          />
        {/key}
      </div>
    {/if}
  </div>
</div>

<style>
  .game {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--line);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stats {
    display: flex;
    gap: 14px;
    font-size: 12px;
    color: var(--txt2);
  }
  .errbox {
    margin: 10px 18px 0;
  }
  /* The scene: a plain background until the story's scenes exist. */
  .scene {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 48px;
    padding: 24px;
    background: radial-gradient(ellipse at 50% 30%, var(--panel2), var(--bg));
    overflow: auto;
  }
  /* Over the scene and the frame, while a dialogue plays. */
  .layer {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
  }
  .layer > :global(*) {
    width: min(960px, 100%);
  }
</style>
