<script lang="ts">
  // Story Mode's door in the hub (SPEC-story-mode, Story Mode: player side):
  // the stories a player can play - the sample Pit Box carries, the packages
  // installed, and the author's own - each with the game under way in it. A
  // new game replaces that game, after a confirmation; a story the checks
  // find errors in does not start, nor does one missing a car or a track it
  // races on. A package is installed from here, from a `.pitstory` or from
  // the link to its manifest; a story installed from a link shows its
  // updates. How an install runs is `installState`.
  import { onMount } from "svelte";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import { t } from "$lib/i18n/index.svelte";
  import { installing, runInstall } from "$lib/story/installState.svelte";
  import { missingLabels } from "$lib/story/package";
  import {
    cancelDownload,
    installFromUrl,
    installPackage,
    listPlayable,
    packageUpdates,
    uninstallStory,
    updatePackage,
    type PackageUpdate,
    type Playable,
    type Source,
  } from "$lib/story/play";
  import { openGame } from "$lib/story/playState.svelte";

  let stories = $state<Playable[] | null>(null);
  let updates = $state<PackageUpdate[]>([]);
  let problem = $state<string | null>(null);
  /** The link field, shown once asked for. */
  let link = $state<string | null>(null);

  async function refresh() {
    try {
      stories = await listPlayable();
    } catch (e) {
      problem = errorText(e);
      stories = [];
    }
  }

  async function checkUpdates() {
    try {
      updates = await packageUpdates();
    } catch (e) {
      console.warn("story: updates not read", e);
    }
  }

  onMount(() => {
    void refresh().then(checkUpdates);
  });

  function key(source: Source): string {
    if (source.kind === "sample") return "sample";
    return source.kind === "installed" ? `installed:${source.id}` : source.path;
  }

  function origin(p: Playable): string {
    if (p.source.kind === "sample") return t("story.play.sample");
    if (p.source.kind === "installed") return t("story.play.installedVersion", { version: p.version ?? "?" });
    return p.source.path;
  }

  function updateOf(p: Playable): PackageUpdate | undefined {
    const source = p.source;
    return source.kind === "installed" ? updates.find((u) => u.id === source.id) : undefined;
  }

  /** A story with errors, or missing what it races on, does not start. */
  function startable(p: Playable): boolean {
    return p.errors === 0 && p.content.missing.length === 0;
  }

  async function start(p: Playable, fresh: boolean) {
    if (fresh && p.game && !(await confirm(t("story.play.replaceConfirm"), { kind: "warning" }))) return;
    problem = await openGame(p.source, fresh);
  }

  async function install(call: Parameters<typeof runInstall>[0]) {
    problem = null;
    await runInstall(call);
    await refresh();
    await checkUpdates();
  }

  async function installFile() {
    const picked = await open({
      title: t("story.play.install"),
      filters: [{ name: t("story.play.packageFile"), extensions: ["pitstory"] }],
    });
    if (typeof picked === "string") await install((loseGame) => installPackage(picked, loseGame));
  }

  async function installLink() {
    const url = link?.trim();
    if (!url) return;
    link = null;
    await install((loseGame) => installFromUrl(url, loseGame));
  }

  async function uninstall(p: Playable) {
    if (p.source.kind !== "installed") return;
    const title = p.title ?? p.source.id;
    if (!(await confirm(t("story.play.uninstallConfirm", { title }), { kind: "warning" }))) return;
    problem = null;
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
      <button class="btn" disabled={installing.busy} onclick={() => (link = link === null ? "" : null)}
        >{t("story.play.installLink")}</button
      >
      <button class="btn" disabled={installing.busy} onclick={installFile}>{t("story.play.install")}</button>
    </span>
  </div>
  <div class="blk-b">
    {#if link !== null}
      <form class="link" onsubmit={(e) => (e.preventDefault(), void installLink())}>
        <input class="input mono" bind:value={link} placeholder={t("story.play.linkPlaceholder")} />
        <button class="btn" type="submit" disabled={!link.trim()}>{t("story.play.installNow")}</button>
      </form>
    {/if}
    {#if installing.progress}
      <div class="progress">
        <progress max={installing.progress.total || 1} value={installing.progress.received}></progress>
        <span class="count">
          {t("story.play.downloading", {
            received: fmtSize(installing.progress.received),
            total: fmtSize(installing.progress.total),
          })}
        </span>
        <button class="btn btn-ghost" onclick={() => void cancelDownload()}>{t("story.play.stop")}</button>
      </div>
    {/if}
    {#if problem ?? installing.problem}<p class="errbox">{problem ?? installing.problem}</p>{/if}
    {#if installing.notice}<p class="notice">{installing.notice}</p>{/if}
    {#if stories}
      <ul class="stories">
        {#each stories as p (key(p.source))}
          {@const update = updateOf(p)}
          <li>
            <div class="what">
              <span class="name">{p.title ?? t("story.studio.untitled")}</span>
              <span class="sub">{origin(p)}</span>
              {#if p.content.other_version.length}
                <span class="sub">
                  {t("story.play.otherVersion", { list: p.content.other_version.map(([, id]) => id).join(", ") })}
                </span>
              {/if}
            </div>
            <span class="state">
              {#if p.errors > 0}
                <span class="pill pill-err">{t("story.studio.errorCount", { count: p.errors })}</span>
              {:else if p.content.missing.length}
                <span class="pill pill-warn" title={missingLabels(p.content.missing).join("\n")}>
                  {t("story.play.missing", { list: missingLabels(p.content.missing).join(", ") })}
                </span>
              {:else if p.game?.problem}
                <span class="pill pill-warn">{t(p.game.problem)}</span>
              {:else if p.game?.ended}
                <span class="pill">{t("story.play.finished")}</span>
              {:else if p.game}
                <span class="count">{t("story.play.underWay")}</span>
              {/if}
            </span>
            <span class="row-actions">
              {#if update && p.source.kind === "installed"}
                {@const id = p.source.id}
                <button
                  class="btn"
                  disabled={installing.busy}
                  onclick={() => install((loseGame) => updatePackage(id, loseGame))}
                >
                  {t("story.play.update", { version: update.version, size: fmtSize(update.size) })}
                </button>
              {/if}
              {#if p.game && !p.game.ended}
                <button class="btn" disabled={!startable(p) || p.game.problem !== null} onclick={() => start(p, false)}
                  >{t("story.play.continue")}</button
                >
              {/if}
              <button class="btn" disabled={!startable(p)} onclick={() => start(p, true)}>{t("story.play.newGame")}</button>
              {#if p.source.kind === "installed"}
                <button class="btn btn-ghost" disabled={installing.busy} onclick={() => uninstall(p)}
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
  .link,
  .progress {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 12px;
  }
  .link .input {
    flex: 1;
  }
  .progress progress {
    flex: 1;
    accent-color: var(--rosso);
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
