<script lang="ts">
  // The story's copies (SPEC-story-mode, Create Your Own): one taken the
  // first time the story is opened on a day, one before every restore. A
  // restore puts the whole story back as it was then; Ctrl+Z undoes it.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { backupDate } from "$lib/story/history";
  import { listBackups, type Backup } from "$lib/story/studio";
  import { restoreCopy, studio } from "$lib/story/studioState.svelte";

  const path = $derived(studio.story!.path);
  let copies = $state<Backup[] | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  async function load(folder: string) {
    try {
      copies = await listBackups(folder);
    } catch (e) {
      problem = errorText(e);
      copies = [];
    }
  }

  $effect(() => {
    void load(path);
  });

  function when(id: string): string {
    const date = backupDate(id);
    return date ? date.toLocaleString(i18n.locale, { dateStyle: "full", timeStyle: "short" }) : id;
  }

  async function restore(copy: Backup) {
    const ok = await confirm(t("story.backups.restoreConfirm", { when: when(copy.id) }), {
      title: t("story.backups.restore"),
      kind: "warning",
    });
    if (!ok) return;
    busy = true;
    problem = await restoreCopy(copy.id);
    busy = false;
    await load(path);
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.backups.title")}</span>
  </header>
  {#if problem}<p class="errbox">{problem}</p>{/if}

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.backups.copies")}</span>
      <span class="blk-n">{copies?.length ?? ""}</span>
    </div>
    <div class="blk-b">
      {#if copies && copies.length === 0}
        <p class="empty">{t("story.backups.none")}</p>
      {:else if copies}
        <ul>
          {#each copies as copy (copy.id)}
            <li>
              <span class="when">{when(copy.id)}</span>
              <span class="files mono">{t("story.backups.files", { count: copy.files })}</span>
              <button class="btn" disabled={busy} onclick={() => restore(copy)}>{t("story.backups.restore")}</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </section>
</div>

<style>
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    margin-bottom: 14px;
  }
  .blk {
    max-width: 720px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  li:first-child {
    border-top: none;
  }
  .when {
    flex: 1;
    color: var(--txt2);
    font-size: 13px;
  }
  .files {
    color: var(--muted);
    font-size: 11.5px;
  }
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  .errbox {
    margin: 0 0 14px;
  }
</style>
