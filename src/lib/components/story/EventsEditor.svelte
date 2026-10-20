<script lang="ts">
  // The Events screen (SPEC-story-mode, Create Your Own: Events): the
  // story's events in the order it plays them - by chapter, then the side
  // events, then those nothing uses yet - and the one selected. A new event
  // gets its id from its display name, like a new character.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { eventFile, eventIds, eventLabels, eventPlaces, newEvent } from "$lib/story/events";
  import { loadLibrary } from "$lib/story/library.svelte";
  import { addEvent, studio } from "$lib/story/studioState.svelte";
  import { nameFrom } from "$lib/story/world";
  import AddName from "./AddName.svelte";
  import EventSheet from "./EventSheet.svelte";
  import PickList, { type PickEntry, type PickGroup } from "./PickList.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const ids = $derived(eventIds(story.events));
  const labels = $derived(eventLabels(story.events));
  const places = $derived(
    eventPlaces(
      story.chapters.map((c) => c.chapter),
      ids,
    ),
  );
  /** Event files that do not read: listed by their file, to be fixed. */
  const unreadable = $derived(story.events.filter((e) => !e.value));
  const shown = $derived(story.events.find((e) => e.file === studio.event) ?? null);
  const errorFiles = $derived(
    new Set(studio.diagnostics.filter((d) => d.severity === "error").map((d) => d.file)),
  );

  const groups = $derived.by((): PickGroup[] => {
    const entry = (id: string): PickEntry[] => {
      const e = story.events.find((x) => x.value?.id === id);
      if (!e) return [];
      const sub = t(`story.events.type_${e.value!.type}`);
      return [{ key: e.file, label: labels[id], sub, error: errorFiles.has(e.file) }];
    };
    return [
      ...places.chapters.map((c) => ({ title: c.title, entries: c.ids.flatMap(entry) })),
      { title: t("story.events.sideEvents"), entries: places.side.flatMap(entry) },
      { title: t("story.events.unused"), entries: places.unused.flatMap(entry) },
      {
        title: t("story.events.unreadable"),
        entries: unreadable.map((e) => ({ key: e.file, label: e.file, sub: "", error: errorFiles.has(e.file) })),
      },
    ];
  });

  onMount(() => void loadLibrary());

  function add(name: string): string | null {
    const id = nameFrom(name, ids, "event");
    if (story.events.some((e) => e.file === eventFile(id))) return t("story.world.nameTaken");
    void addEvent(newEvent(id, name));
    return null;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.events.title")}</span>
    <span class="count">{ids.length}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <div class="body">
    <PickList {groups} selected={studio.event} onselect={(file) => (studio.event = file)}>
      <AddName placeholder={t("story.events.newEvent")} onadd={add} />
    </PickList>

    {#if shown}
      {#key shown.file}
        <EventSheet entry={shown} />
      {/key}
    {:else}
      <p class="empty">{t(ids.length ? "story.events.pickOne" : "story.events.none")}</p>
    {/if}
  </div>

  <StoryChecks section="events" />
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
  .body {
    display: flex;
    gap: 28px;
    align-items: flex-start;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
  }
</style>
