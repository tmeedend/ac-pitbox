<script lang="ts">
  // Story Mode's door in the hub (SPEC-story-mode, Story Mode: player side):
  // the stories a player can play - the sample Pit Box carries and the
  // author's own, until stories come as packages - each with the game under
  // way in it. A new game replaces that game, after a confirmation; a story
  // the checks find errors in does not start.
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import { listPlayable, type Playable, type Source } from "$lib/story/play";
  import { openGame } from "$lib/story/playState.svelte";

  let stories = $state<Playable[] | null>(null);
  let problem = $state<string | null>(null);

  onMount(async () => {
    try {
      stories = await listPlayable();
    } catch (e) {
      problem = errorText(e);
      stories = [];
    }
  });

  function key(source: Source): string {
    return source.kind === "sample" ? "sample" : source.path;
  }

  async function start(p: Playable, fresh: boolean) {
    if (fresh && p.game && !(await confirm(t("story.play.replaceConfirm"), { kind: "warning" }))) return;
    problem = await openGame(p.source, fresh);
  }
</script>

{#if problem}<p class="errbox">{problem}</p>{/if}
{#if stories}
  <ul class="stories">
    {#each stories as p (key(p.source))}
      <li>
        <div class="what">
          <span class="name">{p.title ?? t("story.studio.untitled")}</span>
          <span class="sub">
            {p.source.kind === "sample" ? t("story.play.sample") : p.source.path}
          </span>
        </div>
        <span class="state">
          {#if p.errors > 0}
            <span class="pill pill-err">{t("story.studio.errorCount", { count: p.errors })}</span>
          {:else if p.game?.problem}
            <span class="pill pill-warn">{t(p.game.problem)}</span>
          {:else if p.game?.ended}
            <span class="pill">{t("story.play.finished")}</span>
          {:else if p.game}
            <span class="count">{t("story.play.underWay")}</span>
          {/if}
        </span>
        <span class="row-actions">
          {#if p.game && !p.game.ended}
            <button class="btn" disabled={p.errors > 0 || p.game.problem !== null} onclick={() => start(p, false)}>{t("story.play.continue")}</button>
          {/if}
          <button class="btn" disabled={p.errors > 0} onclick={() => start(p, true)}>{t("story.play.newGame")}</button>
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
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
  .sub {
    color: var(--muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state,
  .row-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--muted);
    font-size: 12px;
  }
</style>
