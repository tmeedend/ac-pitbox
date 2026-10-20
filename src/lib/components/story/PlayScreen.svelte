<script lang="ts">
  // A game of Story Mode (SPEC-story-mode, Story Mode: player side): the
  // three layers - the scene (a plain background until the story's scenes
  // exist), the frame over it (the phone), and the dialogue layer over both
  // while a dialogue plays - with what the story waits for beside the frame.
  // Every move is saved as it is played, so leaving loses nothing.
  //
  // An event goes through the session screen's own launch (the Steam gate,
  // the driver, `startSession`), and its result is read when the game the
  // launch started closes - or on opening, for a session left under way when
  // Pit Box was closed.
  import { onMount, untrack } from "svelte";
  import { errorText } from "$lib/errors";
  import { gameSession, markStarting } from "$lib/launch/gameSession.svelte";
  import { LaunchSequence, startSession } from "$lib/launch/launchSequence.svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { buildSetup, sessionResult } from "$lib/story/raceLaunch";
  import { currentDialogue } from "$lib/story/player/game";
  import { dateLabel } from "$lib/story/player/phone";
  import { speakersOf } from "$lib/story/player/speakers";
  import { closeGame, move, play } from "$lib/story/playState.svelte";
  import SteamPrompt from "$lib/components/launch/SteamPrompt.svelte";
  import DialogueStage from "./player/DialogueStage.svelte";
  import PhoneScreen from "./PhoneScreen.svelte";
  import WaitingCard from "./WaitingCard.svelte";

  const game = $derived(play.game!);
  const view = $derived(game.view);
  const dialogue = $derived(currentDialogue(view.items));
  const date = $derived(dateLabel(view.date, view.date_display, t, i18n.locale));

  const speakers = $derived(speakersOf(game.characters, game.folder));

  const eventWaiting = $derived(view.waiting.kind === "event" ? view.waiting : null);
  /** The session's result is being read. */
  let collecting = $state(false);
  /** Why the session could not be sent or read. */
  let launchProblem = $state<string | null>(null);

  const sequence = new LaunchSequence({ canLaunch: () => eventWaiting !== null, run: race });

  /** Pays the attempt (the engine refuses what the player cannot afford),
   * then sends the session to the game. */
  async function race() {
    const w = untrack(() => eventWaiting);
    if (!w) return;
    launchProblem = null;
    await move({ kind: "launch", event: w.event });
    const g = play.game;
    if (g?.view.launched !== w.event) return;
    try {
      await startSession(await buildSetup(w, g));
      markStarting();
    } catch (e) {
      launchProblem = errorText(e);
    }
  }

  /** Reads the session's result and plays it; none: the card offers to
   * launch again or give up. Only the flow's event: a side event has no
   * screen yet. */
  async function collect() {
    const g = play.game;
    const w = untrack(() => eventWaiting);
    if (!g || !w || g.view.launched !== w.event || g.launched_at_ms === null) return;
    collecting = true;
    try {
      const result = await sessionResult(w, g.launched_at_ms, g);
      if (result) {
        launchProblem = null;
        await move({ kind: "result", event: w.event, result });
      }
    } catch (e) {
      launchProblem = errorText(e);
    } finally {
      collecting = false;
    }
  }

  /** Given up: the session counts as not finished. */
  function giveUp() {
    const w = eventWaiting;
    if (!w) return;
    launchProblem = null;
    void move({
      kind: "result",
      event: w.event,
      result: { finished: false, position: null, best_lap_ms: null, ahead_of: [] },
    });
  }

  // The game the launch started closes: its result is due.
  let wasRunning = false;
  $effect(() => {
    const phase = gameSession.phase;
    if (phase === "running") {
      wasRunning = true;
    } else if (phase === "idle" && wasRunning) {
      wasRunning = false;
      void collect();
    }
  });

  onMount(() => {
    if (gameSession.phase === "idle") void collect();
  });

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
  {#if launchProblem}<p class="errbox">{launchProblem}</p>{/if}

  <div class="scene">
    <WaitingCard
      waiting={view.waiting}
      characters={game.characters}
      drivers={game.drivers}
      {senderOf}
      busy={play.busy || sequence.launching}
      money={view.money}
      launched={view.launched}
      {collecting}
      onaction={(action) => void move({ kind: "action_done", action })}
      onlaunch={() => void sequence.launch()}
      ongiveup={giveUp}
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

<SteamPrompt {sequence} />

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
