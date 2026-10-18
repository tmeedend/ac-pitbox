<script lang="ts">
  // Create Your Own, one story open (SPEC-story-mode, Create Your Own: the
  // editor). The side column names the story, its sections and its chapters;
  // the rest is the section shown. Only the Story flow exists so far: the
  // other sections appear with the lot that builds them, rather than as
  // entries that open nothing.
  import { t } from "$lib/i18n/index.svelte";
  import { allNodeIds, freshChapterId, freshNodeId, nextChapterFile } from "$lib/story/flow";
  import { addChapter, closeStudio, studio } from "$lib/story/studioState.svelte";
  import FlowEditor from "./FlowEditor.svelte";

  interface Props {
    onleave: () => void;
  }
  const { onleave }: Props = $props();

  const story = $derived(studio.story!);
  const errors = $derived(studio.diagnostics.filter((d) => d.severity === "error").length);

  function leave() {
    closeStudio();
    onleave();
  }

  function newChapter() {
    const { file, number } = nextChapterFile(story.chapters.map((c) => c.file));
    const chapters = story.chapters.map((c) => c.chapter);
    addChapter(file, {
      id: freshChapterId(number, chapters),
      title: t("story.studio.chapterTitle", { n: number }),
      nodes: [{ id: freshNodeId("beat", allNodeIds(chapters)), beat: {} }],
    });
  }
</script>

<div class="studio">
  <aside class="side">
    <button class="back btn-ghost" onclick={leave}>‹ {t("story.createTitle")}</button>
    <div class="title">{story.title ?? t("story.studio.untitled")}</div>
    <div class="path mono" title={story.path}>{story.path}</div>

    <nav class="sections">
      <span class="entry on">
        {t("story.studio.flow")}
        {#if errors > 0}<span class="pill pill-err">{errors}</span>{/if}
      </span>
    </nav>

    <div class="lbl section">{t("story.studio.chapters")}</div>
    <ul class="chapters">
      {#each story.chapters as c, i (c.file)}
        <li>
          <button
            class="chapter"
            class:on={i === studio.chapter}
            onclick={() => {
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

  <FlowEditor />
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
  .sections {
    margin-top: 18px;
    display: flex;
    flex-direction: column;
  }
  .entry {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px;
    font-size: 13px;
    color: var(--txt2);
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
