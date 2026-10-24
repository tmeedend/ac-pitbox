<script lang="ts">
  // Story Mode's door in the hub (SPEC-story-mode, Story Mode: player side):
  // the stories a player can play - the sample Pit Box carries, the packages
  // installed, and the author's own - each with the game under way in it. A
  // new game replaces that game, after a confirmation; a story the checks
  // find errors in does not start. A package (`.pitstory`) is installed from
  // here; an update that the game under way would not resume on asks first.
  import { onMount } from "svelte";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { t } from "$lib/i18n/index.svelte";
  import {
    installPackage,
    listPlayable,
    uninstallStory,
    type InstallOutcome,
    type Playable,
    type Source,
  } from "$lib/story/play";
  import { openGame } from "$lib/story/playState.svelte";

  let stories = $state<Playable[] | null>(null);
  let problem = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);

  async function refresh() {
    try {
      stories = await listPlayable();
    } catch (e) {
      problem = errorText(e);
      stories = [];
    }
  }

  onMount(refresh);

  function key(source: Source): string {
    if (source.kind === "sample") return "sample";
    return source.kind === "installed" ? `installed:${source.id}` : source.path;
  }

  function origin(p: Playable): string {
    if (p.source.kind === "sample") return t("story.play.sample");
    if (p.source.kind === "installed") return t("story.play.installedVersion", { version: p.version ?? "?" });
    return p.source.path;
  }

  async function start(p: Playable, fresh: boolean) {
    if (fresh && p.game && !(await confirm(t("story.play.replaceConfirm"), { kind: "warning" }))) return;
    problem = await openGame(p.source, fresh);
  }

  async function install() {
    const picked = await open({
      title: t("story.play.install"),
      filters: [{ name: t("story.play.packageFile"), extensions: ["pitstory"] }],
    });
    if (typeof picked !== "string") return;
    problem = null;
    notice = null;
    busy = true;
    try {
      let outcome: InstallOutcome = await installPackage(picked, false);
      if (outcome.kind === "loses_game") {
        const go = await confirm(t("story.play.losesGameConfirm", outcome), { kind: "warning" });
        if (!go) return;
        outcome = await installPackage(picked, true);
      }
      if (outcome.kind === "installed") {
        const { title, version, previous } = outcome;
        notice =
          previous === null
            ? t("story.play.installDone", { title, version })
            : t("story.play.updateDone", { title, version, previous });
      }
    } catch (e) {
      problem = errorText(e);
    } finally {
      busy = false;
      await refresh();
    }
  }

  async function uninstall(p: Playable) {
    if (p.source.kind !== "installed") return;
    const title = p.title ?? p.source.id;
    if (!(await confirm(t("story.play.uninstallConfirm", { title }), { kind: "warning" }))) return;
    problem = null;
    notice = null;
    try {
      await uninstallStory(p.source.id);
    } catch (e) {
      problem = errorText(e);
    }
    await refresh();
  }
</script>

<section class="blk">
  <div class="blk-h">
    <span class="blk-t">{t("story.modeTitle")}</span>
    <span class="actions">
      <button class="btn" disabled={busy} onclick={install}>{t("story.play.install")}</button>
    </span>
  </div>
  <div class="blk-b">
    {#if problem}<p class="errbox">{problem}</p>{/if}
    {#if notice}<p class="notice">{notice}</p>{/if}
    {#if stories}
      <ul class="stories">
        {#each stories as p (key(p.source))}
          <li>
            <div class="what">
              <span class="name">{p.title ?? t("story.studio.untitled")}</span>
              <span class="sub">{origin(p)}</span>
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
                <button class="btn" disabled={p.errors > 0 || p.game.problem !== null} onclick={() => start(p, false)}
                  >{t("story.play.continue")}</button
                >
              {/if}
              <button class="btn" disabled={p.errors > 0} onclick={() => start(p, true)}>{t("story.play.newGame")}</button>
              {#if p.source.kind === "installed"}
                <button class="btn btn-ghost" disabled={busy} onclick={() => uninstall(p)}
                  >{t("story.play.uninstall")}</button
                >
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .errbox {
    margin: 0 0 12px;
  }
  .notice {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 12px;
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
  .actions,
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
