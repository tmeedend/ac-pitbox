<script lang="ts">
  // The Story hub: the rail entry's two doors, Story Mode and Create Your Own
  // (`docs/story/SPEC-story-mode.md`, « Story Mode: player side »). Story
  // Mode lists the stories to play (`StoryShelf`); a game started replaces
  // the hub with the game. Create Your Own lists the author's stories;
  // opening one replaces the hub with the studio.
  //
  // Loaded lazily by `AppShell` and shown only while `storyUnlocked()` holds.
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { copySample, forgetStory, listStories, registerStory, type StorySummary } from "$lib/story/studio";
  import { play } from "$lib/story/playState.svelte";
  import { openStudio, studio } from "$lib/story/studioState.svelte";
  import NewStoryDialog from "./NewStoryDialog.svelte";
  import PlayScreen from "./PlayScreen.svelte";
  import StoryShelf from "./StoryShelf.svelte";
  import Studio from "./Studio.svelte";

  let stories = $state<StorySummary[] | null>(null);
  let error = $state<string | null>(null);
  let creating = $state(false);

  async function refresh() {
    try {
      stories = await listStories();
    } catch (e) {
      error = errorText(e);
      stories = [];
    }
  }

  onMount(refresh);

  async function openFolder() {
    const picked = await open({ directory: true, title: t("story.studio.openFolder") });
    if (typeof picked !== "string") return;
    error = null;
    try {
      await registerStory(picked);
      await refresh();
      await enter(picked);
    } catch (e) {
      error = errorText(e);
    }
  }

  /** The sample, copied into an empty folder the author picks: their own
   * story to read and change. */
  async function copyTheSample() {
    const picked = await open({ directory: true, title: t("story.studio.pickEmptyFolder") });
    if (typeof picked !== "string") return;
    error = null;
    try {
      await copySample(picked);
      await refresh();
      await enter(picked);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function enter(path: string) {
    error = null;
    try {
      await openStudio(path);
    } catch (e) {
      error = errorText(e);
    }
  }

  async function forget(path: string) {
    error = null;
    try {
      await forgetStory(path);
    } catch (e) {
      error = errorText(e);
    }
    await refresh();
  }

  async function created(path: string) {
    creating = false;
    await refresh();
    await enter(path);
  }

  function leaveStudio() {
    void refresh();
  }
</script>

{#if play.game}
  <PlayScreen />
{:else if studio.story}
  <Studio onleave={leaveStudio} />
{:else}
  <div class="hub">
    <header>
      <div class="lbl-screen">{t("story.title")}</div>
    </header>

    <section class="blk">
      <div class="blk-h"><span class="blk-t">{t("story.modeTitle")}</span></div>
      <div class="blk-b"><StoryShelf /></div>
    </section>

    <section class="blk">
      <div class="blk-h">
        <span class="blk-t">{t("story.createTitle")}</span>
        <span class="actions">
          <button class="btn" title={t("story.studio.copySampleHint")} onclick={copyTheSample}
            >{t("story.studio.copySample")}</button
          >
          <button class="btn" onclick={openFolder}>{t("story.studio.openFolder")}</button>
          <button class="btn" onclick={() => (creating = true)}>{t("story.studio.newStory")}</button>
        </span>
      </div>
      <div class="blk-b">
        {#if error}<p class="errbox">{error}</p>{/if}
        {#if stories && stories.length === 0}
          <p class="empty">{t("story.noOwnStory")}</p>
        {:else if stories}
          <ul class="stories">
            {#each stories as s (s.path)}
              <li>
                <div class="what">
                  <span class="name">{s.title ?? t("story.studio.untitled")}</span>
                  <span class="path mono">{s.path}</span>
                </div>
                <span class="state">
                  {#if s.missing}
                    <span class="pill pill-warn">{t("story.studio.missing")}</span>
                  {:else}
                    <span class="count">{t("story.studio.chapterCount", { count: s.chapters })}</span>
                    {#if s.errors > 0}
                      <span class="pill pill-err">{t("story.studio.errorCount", { count: s.errors })}</span>
                    {:else if s.warnings > 0}
                      <span class="pill pill-warn">{t("story.studio.warningCount", { count: s.warnings })}</span>
                    {/if}
                  {/if}
                </span>
                <span class="row-actions">
                  <button class="btn" disabled={s.missing} onclick={() => enter(s.path)}>
                    {t("story.studio.open")}
                  </button>
                  <button class="btn btn-ghost" title={t("story.studio.forgetHint")} onclick={() => forget(s.path)}>
                    {t("story.studio.forget")}
                  </button>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </section>
  </div>
{/if}

{#if creating}
  <NewStoryDialog oncreated={created} oncancel={() => (creating = false)} />
{/if}

<style>
  .hub {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 880px;
    padding: 28px 32px;
    overflow: auto;
    height: 100%;
    box-sizing: border-box;
  }
  header {
    margin-bottom: 14px;
  }
  .actions,
  .row-actions {
    display: flex;
    gap: 8px;
  }
  .empty {
    color: var(--muted);
    font-size: 13px;
    margin: 0;
  }
  .errbox {
    margin: 0 0 12px;
  }
  .stories {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .stories li {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    background: var(--card);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .name {
    font-weight: 600;
    font-size: 13px;
  }
  .path {
    color: var(--muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--muted);
    font-size: 12px;
  }
</style>
