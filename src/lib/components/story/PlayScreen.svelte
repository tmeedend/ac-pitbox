<script lang="ts">
  // A game of Story Mode (SPEC-story-mode, Story Mode: player side): the
  // three layers - the scene (the current place, or the one the dialogue on
  // stage plays in; plain without one), with its ambient sound, the frame
  // over it (the phone), and the dialogue layer over both
  // while a dialogue or an illustration plays - with what the story waits for beside the frame;
  // the next event goes in the frame itself, heading its messages, and on
  // the scene only when there is no frame.
  // While a dialogue plays, or the journal reads past ones again, the frame
  // slides out of view and the scene's blur is reduced; it comes back after.
  // Every move is saved as it is played, so leaving loses nothing.
  //
  // An event goes through the session screen's own launch (the Steam gate,
  // the driver, `startSession`), and its result is read when the game the
  // launch started closes - or on opening, for a session left under way when
  // Pit Box was closed.
  import { onMount, untrack } from "svelte";
  import { fade } from "svelte/transition";
  import { errorText } from "$lib/errors";
  import { gameSession, markStarting } from "$lib/launch/gameSession.svelte";
  import { LaunchSequence, startSession } from "$lib/launch/launchSequence.svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { buildSetup, sessionResult } from "$lib/story/raceLaunch";
  import { dialogueLog, onStage, readBefore } from "$lib/story/player/game";
  import { ambientMuted, autoSpeed, setAmbientMuted } from "$lib/story/playPrefs";
  import { sceneShown } from "$lib/story/player/scene";
  import { homeApp } from "$lib/story/player/apps";
  import { dateLabel } from "$lib/story/player/phone";
  import { mediaUrl, speakersOf } from "$lib/story/player/speakers";
  import { beginStorySession } from "$lib/story/play";
  import { closeGame, keepReading, move, play } from "$lib/story/playState.svelte";
  import SteamPrompt from "$lib/components/launch/SteamPrompt.svelte";
  import DialogueStage from "./player/DialogueStage.svelte";
  import DialogueControls from "./DialogueControls.svelte";
  import DialogueLog from "./DialogueLog.svelte";
  import IllustrationView from "./player/IllustrationView.svelte";
  import DocumentView from "./player/DocumentView.svelte";
  import SceneSkin from "./player/SceneSkin.svelte";
  import TvView from "./player/TvView.svelte";
  import SceneAmbient from "./player/SceneAmbient.svelte";
  import SceneView from "./player/SceneView.svelte";
  import EventCard from "./EventCard.svelte";
  import Journal from "./Journal.svelte";
  import PhoneScreen from "./PhoneScreen.svelte";
  import WaitingCard from "./WaitingCard.svelte";

  const game = $derived(play.game!);
  const view = $derived(game.view);
  /** The dialogue or illustration the scene plays, as an index into the items. */
  const played = $derived(onStage(view.items));
  const date = $derived(dateLabel(view.date, view.date_display, t, i18n.locale));

  const speakers = $derived(speakersOf(game.characters, game.folder));

  /** The item on stage, a dialogue or an illustration. */
  const onStageItem = $derived(played !== null ? view.items[played] : null);
  const scene = $derived(sceneShown(view.scene, onStageItem));
  const ambient = $derived(mediaUrl(game.folder, scene?.ambient));
  const muted = $derived(ambientMuted());
  /** Something to mute: the scene's ambience, or the phone's own sounds. */
  const hasSound = $derived(ambient !== null || !!view.phone?.sounds.tap || !!view.phone?.sounds.send);

  let journal = $state(false);
  /** The scene holds the stage: the frame is out of view. */
  const staged = $derived(played !== null || journal);
  // A dialogue arriving (a session's result read on opening) takes the
  // scene, and the journal does not come back after it.
  $effect(() => {
    if (played !== null) journal = false;
  });

  // Reading the dialogue under way: fast-forward, auto-advance, the log and
  // the step shown, which the log stops at.
  let skip = $state(false);
  let auto = $state(false);
  let log = $state(false);
  let reading = $state(-1);
  // Another dialogue, or none: fast-forward and the log were for the last
  // one. Auto-advance is the player's way of reading: it stays.
  $effect(() => {
    void played;
    skip = false;
    log = false;
  });

  const eventWaiting = $derived(view.waiting.kind === "event" ? view.waiting : null);
  /** With a frame, the next event is in it, heading its messages; with
   * none - or a frame with no messaging app - on the scene. */
  const eventInFrame = $derived(view.frame !== "none" && homeApp(view.apps) !== null && eventWaiting !== null);
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
      // The story's own layers over the event's car and track, on until the
      // game closes; one that does not come on keeps the session from going.
      if (play.source) await beginStorySession(play.source, w.event);
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

  /** Who a document or a TV report is from, as the story names them. */
  function nameOf(who: string | null): string | null {
    return who ? (speakers[who]?.name ?? who) : null;
  }

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
    <button class="btn btn-ghost" disabled={played !== null} onclick={() => (journal = !journal)}
      >{t("story.journal.title")}</button
    >
    {#if hasSound}
      <button class="btn btn-ghost" aria-pressed={muted} onclick={() => setAmbientMuted(!muted)}
        >{t(muted ? "story.play.soundOff" : "story.play.sound")}</button
      >
    {/if}
    <span class="stats">
      {#if date}<span>{date}</span>{/if}
      {#if view.money !== null}<span>{t("story.play.money", { n: view.money })}</span>{/if}
      <span>{t("story.play.reputation", { n: view.reputation })}</span>
    </span>
  </header>
  {#if play.problem}<p class="errbox">{play.problem}</p>{/if}
  {#if launchProblem}<p class="errbox">{launchProblem}</p>{/if}

  <div class="scene">
    <SceneView {scene} folder={game.folder} {staged} transition={onStageItem?.transition ?? null} />
    <SceneAmbient src={ambient} {muted} />
    <div class="frame-layer" class:away={staged} inert={staged}>
      {#if !eventInFrame}
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
      {/if}
      {#if view.frame !== "none"}
        <div class="device">
          <PhoneScreen
            apps={view.apps}
            folder={game.folder}
            look={view.phone}
            {muted}
            items={view.items}
            clock={view.clock}
            {date}
            dateDisplay={view.date_display}
            {speakers}
            money={view.money}
            onreply={(content, reply) => void move({ kind: "reply", content, reply })}
            onseen={(content) => void move({ kind: "seen", content })}
            next={eventInFrame ? nextEvent : undefined}
          />
        </div>
      {/if}
    </div>

    {#if played !== null}
      {@const d = view.items[played]}
      {@const cut = d.transition === "cut"}
      <div
        class="layer"
        in:fade={{ duration: cut ? 0 : 250, delay: cut ? 0 : 200 }}
        out:fade={{ duration: cut ? 0 : 150 }}
      >
        {#if d.kind === "illustration"}
          <div class="fit">
            {#key played}
              <IllustrationView
                picture={mediaUrl(game.folder, d.picture)}
                steps={d.steps}
                bare
                autofocus
                onend={() => void move({ kind: "seen", content: d.content })}
              />
            {/key}
          </div>
        {:else if d.staged_as === "newspaper" || d.staged_as === "letter"}
          <div class="fit">
            <SceneSkin skin={d.skin} folder={game.folder}>
              {#key played}
                <DocumentView
                  layout={d.staged_as}
                  from={nameOf(d.from)}
                  picture={mediaUrl(game.folder, d.picture)}
                  steps={d.steps}
                  bare
                  autofocus
                  onend={() => void move({ kind: "seen", content: d.content })}
                />
              {/key}
            </SceneSkin>
          </div>
        {:else if d.staged_as === "tv"}
          <div class="fit">
            <SceneSkin skin={d.skin} folder={game.folder}>
              {#key played}
                <TvView
                  channel={nameOf(d.from)}
                  picture={mediaUrl(game.folder, d.picture)}
                  steps={d.steps}
                  bare
                  autofocus
                  onend={() => void move({ kind: "seen", content: d.content })}
                />
              {/key}
            </SceneSkin>
          </div>
        {:else}
          {#if d.staged_as === "landline"}
            <div class="landline">☎ {t("story.play.landline")}</div>
          {/if}
          <div class="reading"><DialogueControls bind:skip bind:auto onlog={() => (log = true)} /></div>
          <div class="fit" inert={log}>
            <SceneSkin skin={d.skin} folder={game.folder}>
            {#key played}
              <DialogueStage
                steps={d.steps}
                choices={d.choices}
                present={d.characters.length || !d.from ? d.characters : [d.from]}
                from={d.from}
                {speakers}
                ended={!d.open}
                transition={d.transition}
                bare
                autofocus
                auto={auto && !log ? autoSpeed() : null}
                bind:skip
                readKeys={readBefore(view.items, played)}
                onlog={() => (log = true)}
                startAt={game.reading?.item === played ? game.reading.at : undefined}
                onat={(at) => {
                  reading = at;
                  if (at >= 0) keepReading({ item: played, at });
                }}
                onchoose={(reply) => void move({ kind: "reply", content: d.content, reply })}
                onend={() => void move({ kind: "seen", content: d.content })}
              />
            {/key}
            </SceneSkin>
          </div>
          {#if log}
            <DialogueLog lines={dialogueLog(view.items, played, reading)} {speakers} onclose={() => (log = false)} />
          {/if}
        {/if}
      </div>
    {:else if journal}
      <div class="layer" in:fade={{ duration: 250, delay: 200 }} out:fade={{ duration: 150 }}>
        <Journal items={view.items} dateDisplay={view.date_display} {speakers} onclose={() => (journal = false)} />
      </div>
    {/if}
  </div>
</div>

{#snippet nextEvent()}
  {#if eventWaiting}
    <EventCard
      waiting={eventWaiting}
      characters={game.characters}
      drivers={game.drivers}
      busy={play.busy || sequence.launching}
      money={view.money}
      launched={view.launched}
      {collecting}
      onlaunch={() => void sequence.launch()}
      ongiveup={giveUp}
    />
  {/if}
{/snippet}

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
  /* The scene, its pictures drawn by `SceneView`, the layers over them. */
  .scene {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .frame-layer {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 48px;
    padding: 24px;
    overflow: auto;
    transition:
      transform 0.35s ease,
      opacity 0.35s ease;
  }
  /* Out of view, below the scene, while the stage holds it. */
  .frame-layer.away {
    transform: translateY(100%);
    opacity: 0;
  }
  /* Over the scene, while a dialogue plays or the journal is open. */
  .layer {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    container-type: size;
  }
  /* A call on the landline: said in the corner, over the stage. */
  .landline {
    position: absolute;
    top: 10px;
    left: 14px;
    z-index: 1;
    color: var(--txt2);
    font-size: 12px;
  }
  /* The reading controls, in the scene's corner above the stage. */
  .reading {
    position: absolute;
    top: 8px;
    right: 12px;
    z-index: 1;
  }
  /* The stage as large as the scene allows, in 16:9 like the preview's. */
  .fit {
    width: min(100cqw, calc(100cqh * 16 / 9));
  }
</style>
