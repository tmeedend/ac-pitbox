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

  onMount(() => void loadLibrary());

  function add(name: string): string | null {
    const id = nameFrom(name, ids, "event");
    if (story.events.some((e) => e.file === eventFile(id))) return t("story.world.nameTaken");
    void addEvent(newEvent(id, name));
    return null;
  }
</script>

{#snippet item(file: string, label: string, sub: string)}
  <li>
    <button class="item" class:on={studio.event === file} onclick={() => (studio.event = file)}>
      <span class="name">{label}</span>
      <span class="sub mono">{sub}</span>
      {#if errorFiles.has(file)}<span class="pill pill-err">!</span>{/if}
    </button>
  </li>
{/snippet}

{#snippet group(title: string, list: string[])}
  <div class="lbl group">{title}</div>
  <ul>
    {#each list as id (id)}
      {@const entry = story.events.find((e) => e.value?.id === id)}
      {#if entry}
        {@render item(entry.file, labels[id], t(`story.events.type_${entry.value!.type}`))}
      {/if}
    {/each}
  </ul>
{/snippet}

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.events.title")}</span>
    <span class="count">{ids.length}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <div class="body">
    <nav class="list">
      {#each places.chapters as c (c.index)}
        {@render group(c.title, c.ids)}
      {/each}
      {#if places.side.length}{@render group(t("story.events.sideEvents"), places.side)}{/if}
      {#if places.unused.length}{@render group(t("story.events.unused"), places.unused)}{/if}
      {#if unreadable.length}
        <div class="lbl group">{t("story.events.unreadable")}</div>
        <ul>
          {#each unreadable as e (e.file)}{@render item(e.file, e.file, "")}{/each}
        </ul>
      {/if}
      <AddName placeholder={t("story.events.newEvent")} onadd={add} />
    </nav>

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
  .list {
    width: 250px;
    flex-shrink: 0;
  }
  .list ul {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
  }
  .group {
    margin: 12px 0 4px;
  }
  .group:first-child {
    margin-top: 0;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    background: transparent;
    color: var(--txt2);
    font-size: 13px;
  }
  .item:hover {
    background: var(--raised);
  }
  .item.on {
    background: var(--raised);
    color: var(--txt);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--muted);
    font-size: 10.5px;
    flex-shrink: 0;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
  }
</style>
