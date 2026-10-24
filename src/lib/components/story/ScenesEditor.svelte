<script lang="ts">
  // The Scenes screen (SPEC-story-mode, Create Your Own: Scenes): the
  // story's places - `scenes/scenes.yaml` - listed in the file's order, the
  // one selected with its variants, and beside it the place as the player
  // sees it at the moment the author sets. A new place gets its id from the
  // name typed, like a character. Each edit saves the file at once.
  import { t } from "$lib/i18n/index.svelte";
  import { newPlace } from "$lib/story/scenes";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import { nameFrom } from "$lib/story/world";
  import AddName from "./AddName.svelte";
  import FileStatus from "./FileStatus.svelte";
  import PickList, { type PickGroup } from "./PickList.svelte";
  import ScenePlaceSheet from "./ScenePlaceSheet.svelte";
  import ScenePreview from "./ScenePreview.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const scenes = $derived(story.scenes.value);
  const ids = $derived(scenes?.scenes.map((p) => p.id) ?? []);
  const groups = $derived<PickGroup[]>([
    {
      title: t("story.scenes.places"),
      entries: (scenes?.scenes ?? []).map((p) => ({
        key: p.id,
        label: p.name ?? p.id,
        sub: t("story.scenes.variantCount", { count: p.variants.length }),
      })),
    },
  ]);
  const shown = $derived(scenes?.scenes.findIndex((p) => p.id === studio.scene) ?? -1);

  function add(name: string): string | null {
    if (!scenes) return null;
    const id = nameFrom(name, ids, "place");
    scenes.scenes.push(newPlace(id, name));
    studio.scene = id;
    void saveStoryFile("scenes");
    return null;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.scenes.title")}</span>
    <span class="count">{ids.length}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}
  <FileStatus entry={story.scenes} />

  {#if scenes}
    <div class="body">
      <PickList {groups} selected={studio.scene} onselect={(key) => (studio.scene = key)}>
        <AddName placeholder={t("story.scenes.newPlace")} onadd={add} />
      </PickList>

      {#if shown >= 0}
        <!-- Made again for another place: the preview's moment and the
             sheet's open fields belong to the place they were set on. -->
        {#key studio.scene}
          <ScenePlaceSheet index={shown} />
          <ScenePreview place={scenes.scenes[shown]} />
        {/key}
      {:else}
        <p class="empty">{t(ids.length ? "story.scenes.pickOne" : "story.scenes.none")}</p>
      {/if}
    </div>
  {/if}

  <StoryChecks section="scenes" />
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
  /* The preview goes below the sheet when the row runs out of room, as on
     the Contents screen. */
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
