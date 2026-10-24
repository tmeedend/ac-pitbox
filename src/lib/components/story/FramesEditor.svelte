<script lang="ts">
  // The Frames screen (SPEC-story-mode, Create Your Own: the Frame screen):
  // the story's phones - `frames/frames.yaml` - and its skins, each a folder
  // of `frames/skins/`. The one selected is edited in its sheet, and shown
  // beside it as the player sees it. A new phone or skin gets its id from
  // the name typed. The starter kit - a sheet per template - is saved from
  // here.
  import { t } from "$lib/i18n/index.svelte";
  import { newFrame, skinFile, skinIdOf } from "$lib/story/frameEdit";
  import { addSkin, saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import { nameFrom } from "$lib/story/world";
  import AddName from "./AddName.svelte";
  import FramePreview from "./FramePreview.svelte";
  import FileStatus from "./FileStatus.svelte";
  import FrameSheet from "./FrameSheet.svelte";
  import PickList, { type PickGroup } from "./PickList.svelte";
  import SkinSheet from "./SkinSheet.svelte";
  import StarterKit from "./StarterKit.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const frames = $derived(story.frames.value);
  const frameIds = $derived(frames?.frames.map((f) => f.id) ?? []);
  const skinIds = $derived(story.skins.map((s) => skinIdOf(s.file)).filter((id): id is string => id !== null));
  /** A diagnostic names `file:line`: the files with an error. */
  const errorFiles = $derived(
    new Set(studio.diagnostics.filter((d) => d.severity === "error").map((d) => d.file.replace(/:\d+$/, ""))),
  );
  const groups = $derived<PickGroup[]>([
    {
      title: t("story.frames.phones"),
      entries: (frames?.frames ?? []).map((f) => ({
        key: `frame:${f.id}`,
        label: f.name ?? f.id,
        sub: t("story.frames.appCount", { count: f.apps.length }),
        error: errorFiles.has("frames/frames.yaml"),
      })),
    },
    {
      title: t("story.frames.skins"),
      entries: story.skins.map((s) => ({
        key: s.file,
        label: skinIdOf(s.file) ?? s.file,
        sub: s.value?.template ?? "",
        error: errorFiles.has(s.file) || s.error !== null,
      })),
    },
  ]);
  const frameIndex = $derived(
    studio.frame?.startsWith("frame:") ? frameIds.indexOf(studio.frame.slice("frame:".length)) : -1,
  );
  const skin = $derived(story.skins.find((s) => s.file === studio.frame) ?? null);

  function addFrame(name: string): string | null {
    if (!frames) return null;
    const id = nameFrom(name, [...frameIds, "phone", "none"], "phone");
    frames.frames.push(newFrame(id, name));
    studio.frame = `frame:${id}`;
    void saveStoryFile("frames");
    return null;
  }

  function addNewSkin(name: string): string | null {
    const id = nameFrom(name, skinIds, "skin");
    void addSkin(skinFile(id), { template: "thread" });
    return null;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.frames.title")}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}
  <FileStatus entry={story.frames} />
  <StarterKit />

  <div class="body">
    <PickList {groups} selected={studio.frame} onselect={(key) => (studio.frame = key)}>
      <AddName placeholder={t("story.frames.newPhone")} onadd={addFrame} />
      <AddName placeholder={t("story.frames.newSkin")} onadd={addNewSkin} />
    </PickList>

    {#if frames && frameIndex >= 0}
      {#key studio.frame}
        <FrameSheet index={frameIndex} />
        <FramePreview frame={frames.frames[frameIndex]} skin={null} />
      {/key}
    {:else if skin}
      {#key studio.frame}
        <SkinSheet file={skin.file} />
        {#if skin.value}
          <FramePreview frame={null} skin={{ id: skinIdOf(skin.file) ?? "", value: skin.value }} />
        {/if}
      {/key}
    {:else}
      <p class="empty">{t("story.frames.pickOne")}</p>
    {/if}
  </div>

  <StoryChecks section="frames" />
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
