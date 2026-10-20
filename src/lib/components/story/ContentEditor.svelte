<script lang="ts">
  // The Contents screen (SPEC-story-mode, Create Your Own: Content): the
  // story's messages, mails, articles, dialogues… listed by file, as the
  // author filed them, and the one selected. A new item gets its id from
  // the name typed, like a new event, and a file of its own.
  import { t } from "$lib/i18n/index.svelte";
  import { contentFile, contentIds, fileTitle, findContent, newContent, placementOf } from "$lib/story/content";
  import { CONTENT_TYPE_LABEL } from "$lib/story/labels";
  import { addContent, studio } from "$lib/story/studioState.svelte";
  import { nameFrom } from "$lib/story/world";
  import AddName from "./AddName.svelte";
  import ContentPreview from "./ContentPreview.svelte";
  import ContentSheet from "./ContentSheet.svelte";
  import PickList, { type PickGroup } from "./PickList.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const ids = $derived(contentIds(story.contentFiles));
  /** A diagnostic names `file:line`: the files with an error. */
  const errorFiles = $derived(
    new Set(studio.diagnostics.filter((d) => d.severity === "error").map((d) => d.file.replace(/:\d+$/, ""))),
  );
  const PLACEMENT_LABEL = {
    flow: "story.content.placementFlow",
    reactive: "story.content.placementReactive",
    pool: "story.content.placementPool",
  } as const;

  const groups = $derived(
    story.contentFiles.map(
      (f): PickGroup => ({
        title: fileTitle(f.file),
        entries: f.value
          ? f.value.items.map((d) => ({
              key: d.header.id,
              label: d.header.id,
              sub: `${t(CONTENT_TYPE_LABEL[d.header.type])} · ${t(PLACEMENT_LABEL[placementOf(d.header)])}`,
              error: errorFiles.has(f.file),
            }))
          : [{ key: f.file, label: t("story.content.unreadable"), sub: "", error: true }],
      }),
    ),
  );
  const shown = $derived(studio.content ? findContent(story.contentFiles, studio.content) : null);
  const unreadable = $derived(story.contentFiles.find((f) => f.file === studio.content && !f.value) ?? null);

  function add(name: string): string | null {
    const id = nameFrom(name, ids, "content");
    if (story.contentFiles.some((f) => f.file === contentFile(id))) return t("story.world.nameTaken");
    void addContent(newContent(id));
    return null;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.content.title")}</span>
    <span class="count">{ids.length}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <div class="body">
    <PickList {groups} selected={studio.content} onselect={(key) => (studio.content = key)}>
      <AddName placeholder={t("story.content.newContent")} onadd={add} />
    </PickList>

    {#if shown}
      {#key studio.content}
        <ContentSheet file={shown.file} index={shown.index} />
        <ContentPreview id={studio.content!} />
      {/key}
    {:else if unreadable}
      <p class="errbox">{t("story.world.unreadable", { file: unreadable.file, reason: unreadable.error ?? "" })}</p>
    {:else}
      <p class="empty">{t(ids.length ? "story.content.pickOne" : "story.content.none")}</p>
    {/if}
  </div>

  <StoryChecks section="content" />
</div>

<style>
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 14px;
  }
  .count {
    color: var(--muted);
    font-size: 12px;
  }
  /* The preview goes below the sheet when the row runs out of room: the
     wrap follows the width the screen really has, with no breakpoint. */
  .body {
    display: flex;
    flex-wrap: wrap;
    gap: 28px;
    align-items: flex-start;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
  }
</style>
