<script lang="ts">
  // Create Your Own, one story open (SPEC-story-mode, Create Your Own: the
  // editor). The side column names the story, its sections and its chapters;
  // the rest is the section shown. A section appears with the lot that
  // builds it, rather than as an entry that opens nothing: the Story flow,
  // the Events, the Contents, the Drivers, the World, the Scenes, the
  // Frames and the Settings so far, and the story's copies.
  // Ctrl+Z and Ctrl+Y undo and redo the last edits, whatever section shows.
  import { t } from "$lib/i18n/index.svelte";
  import { allNodeIds, freshChapterId, freshNodeId, nextChapterFile } from "$lib/story/flow";
  import { historyKey, isTypingTarget } from "$lib/story/history";
  import {
    addChapter,
    closeStudio,
    sectionOf,
    stepHistory,
    studio,
    type StudioSection,
  } from "$lib/story/studioState.svelte";
  import ContentEditor from "./ContentEditor.svelte";
  import DriversEditor from "./DriversEditor.svelte";
  import EventsEditor from "./EventsEditor.svelte";
  import FlowEditor from "./FlowEditor.svelte";
  import ScenesEditor from "./ScenesEditor.svelte";
  import FramesEditor from "./FramesEditor.svelte";
  import SettingsEditor from "./SettingsEditor.svelte";
  import StoryBackups from "./StoryBackups.svelte";
  import WorldEditor from "./WorldEditor.svelte";

  interface Props {
    onleave: () => void;
  }
  const { onleave }: Props = $props();

  const story = $derived(studio.story!);

  const SECTIONS: { id: StudioSection; label: string }[] = [
    { id: "flow", label: "story.studio.flow" },
    { id: "events", label: "story.events.title" },
    { id: "content", label: "story.content.title" },
    { id: "drivers", label: "story.drivers.title" },
    { id: "world", label: "story.world.title" },
    { id: "scenes", label: "story.scenes.title" },
    { id: "frames", label: "story.frames.title" },
    { id: "settings", label: "story.settings.title" },
    { id: "backups", label: "story.backups.title" },
  ];

  /** Errors by section, for the counts beside their entries. */
  const errors = $derived.by(() => {
    const out: Record<StudioSection, number> = {
      flow: 0,
      events: 0,
      content: 0,
      drivers: 0,
      world: 0,
      scenes: 0,
      frames: 0,
      settings: 0,
      backups: 0,
    };
    for (const d of studio.diagnostics) if (d.severity === "error") out[sectionOf(d.file)]++;
    return out;
  });

  function onkeydown(e: KeyboardEvent) {
    const step = historyKey(e, isTypingTarget(e.target));
    if (!step) return;
    e.preventDefault();
    void stepHistory(step);
  }

  function leave() {
    closeStudio();
    onleave();
  }

  function newChapter() {
    const { file, number } = nextChapterFile(story.chapters.map((c) => c.file));
    const chapters = story.chapters.map((c) => c.chapter);
    studio.section = "flow";
    addChapter(file, {
      id: freshChapterId(number, chapters),
      title: t("story.studio.chapterTitle", { n: number }),
      nodes: [{ id: freshNodeId("beat", allNodeIds(chapters)), beat: {} }],
    });
  }
</script>

<svelte:window {onkeydown} />

<div class="studio">
  <aside class="side">
    <button class="back btn-ghost" onclick={leave}>‹ {t("story.createTitle")}</button>
    <div class="title">{story.manifest.value?.title ?? t("story.studio.untitled")}</div>
    <div class="path mono" title={story.path}>{story.path}</div>
    <div class="history">
      <button
        class="btn btn-ghost"
        disabled={!story.history.canUndo}
        title={t("story.studio.undo")}
        aria-label={t("story.studio.undo")}
        onclick={() => stepHistory("undo")}>↶</button
      >
      <button
        class="btn btn-ghost"
        disabled={!story.history.canRedo}
        title={t("story.studio.redo")}
        aria-label={t("story.studio.redo")}
        onclick={() => stepHistory("redo")}>↷</button
      >
    </div>

    <nav class="sections">
      {#each SECTIONS as s (s.id)}
        <button class="entry" class:on={studio.section === s.id} onclick={() => (studio.section = s.id)}>
          {t(s.label)}
          {#if errors[s.id] > 0}<span class="pill pill-err">{errors[s.id]}</span>{/if}
        </button>
      {/each}
    </nav>

    <div class="lbl section">{t("story.studio.chapters")}</div>
    <ul class="chapters">
      {#each story.chapters as c, i (c.file)}
        <li>
          <button
            class="chapter"
            class:on={studio.section === "flow" && i === studio.chapter}
            onclick={() => {
              studio.section = "flow";
              studio.chapter = i;
              studio.node = null;
            }}
          >
            <span class="n mono">{i + 1}</span>
            <span class="name">{c.chapter?.title ?? c.chapter?.id ?? c.file}</span>
            {#if c.error}<span class="pill pill-err">!</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
    <button class="btn btn-ghost add" onclick={newChapter}>+ {t("story.studio.addChapter")}</button>
  </aside>

  {#if studio.section === "world"}
    <WorldEditor />
  {:else if studio.section === "events"}
    <EventsEditor />
  {:else if studio.section === "content"}
    <ContentEditor />
  {:else if studio.section === "drivers"}
    <DriversEditor />
  {:else if studio.section === "scenes"}
    <ScenesEditor />
  {:else if studio.section === "frames"}
    <FramesEditor />
  {:else if studio.section === "backups"}
    <StoryBackups />
  {:else if studio.section === "settings"}
    <SettingsEditor />
  {:else}
    <FlowEditor />
  {/if}
</div>

<style>
  .studio {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .side {
    width: 230px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
    background: var(--panel);
    padding: 18px 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .back {
    align-self: flex-start;
    padding: 4px 6px;
    margin: 0 0 14px -6px;
    font-size: 12px;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
  .path {
    color: var(--muted);
    font-size: 10.5px;
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .history {
    display: flex;
    gap: 4px;
    margin: 8px 0 0 -6px;
  }
  .history .btn {
    padding: 2px 8px;
    font-size: 15px;
  }
  .sections {
    margin-top: 18px;
    display: flex;
    flex-direction: column;
  }
  .entry {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    font-size: 13px;
    background: transparent;
    color: var(--txt2);
  }
  .entry:hover {
    background: var(--raised);
  }
  .entry.on {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .chapters {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .chapter {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    background: transparent;
    color: var(--muted);
    font-size: 12.5px;
  }
  .chapter:hover {
    color: var(--txt2);
    background: var(--raised);
  }
  .chapter.on {
    color: var(--txt);
    background: var(--raised);
  }
  .chapter .n {
    color: var(--faint);
    font-size: 11px;
  }
  .chapter .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .add {
    align-self: flex-start;
    margin-top: 6px;
    font-size: 12px;
  }
</style>
