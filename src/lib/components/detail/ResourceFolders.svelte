<script lang="ts">
  // The folders of a mod's resources, each removable (§4.6ter).
  //
  // A proposed folder answered "keep" is given the same answer at the next
  // import of the mod, without a question. This is where the user changes
  // their mind: the result of the answer is here, and removing it becomes the
  // answer — the folder offered again by a newer version is not imported.
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/errors";
  import { fmtSize } from "$lib/format";
  import {
    listResourceFolders,
    removeResourceFolder,
    type ResourceFolder,
    type ResourceFolderSource,
  } from "$lib/detail/resourceFolders";
  import { t } from "$lib/i18n/index.svelte";

  let {
    modId,
    source,
    onremoved,
    onerror,
  }: {
    modId: string;
    source: ResourceFolderSource;
    /** A folder left: the files listed around this block went with it. */
    onremoved: () => void;
    onerror: (message: string) => void;
  } = $props();

  let folders = $state<ResourceFolder[]>([]);
  let busy = $state(false);

  // Same guard as the rest of the Resources card: a late answer for the
  // previous mod must not overwrite the current one's list.
  $effect(() => {
    const current = modId;
    const from = source;
    folders = [];
    listResourceFolders(current, from)
      .then((list) => {
        if (current === modId) folders = list;
      })
      .catch((e) => onerror(errorText(e)));
  });

  async function remove(f: ResourceFolder): Promise<void> {
    const ok = await confirm(t("detail.resourceFolderRemoveConfirm", { name: f.name }), {
      title: t("detail.resourceFolderRemoveTitle"),
      kind: "warning",
    });
    if (!ok) return;
    busy = true;
    try {
      await removeResourceFolder(modId, source, f.name);
      folders = folders.filter((x) => x.name !== f.name);
      onremoved();
    } catch (e) {
      onerror(errorText(e));
    } finally {
      busy = false;
    }
  }
</script>

{#if folders.length}
  <ul class="fold-list">
    {#each folders as f (f.name)}
      <li class="fold-row">
        <span class="fold-nm mono">{f.name}/</span>
        <span class="fold-size mono">
          {t("detail.resourceFolderFiles", { count: f.file_count, size: fmtSize(f.size_bytes) })}
        </span>
        <button
          class="fold-x"
          type="button"
          title={t("detail.resourceFolderRemoveTitle")}
          aria-label={t("detail.resourceFolderRemoveTitle")}
          disabled={busy}
          onclick={() => remove(f)}>✕</button
        >
      </li>
    {/each}
  </ul>
{/if}

<style>
  /* Same row as the documents listed below it, so the card reads as one list:
     bordered, raised, the name first and the size in the margin. */
  .fold-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 12px;
  }
  .fold-row {
    display: flex;
    align-items: stretch;
    border: 1px solid var(--line);
    background: var(--raised);
  }
  .fold-nm {
    flex: 1;
    min-width: 0;
    padding: 8px 11px;
    font-size: 12px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fold-size {
    flex: none;
    align-self: center;
    padding-right: 10px;
    font-size: 10.5px;
    color: var(--muted2);
  }
  .fold-x {
    flex: none;
    padding: 0 10px;
    background: none;
    border: none;
    border-left: 1px solid var(--line);
    color: var(--muted2);
    font-size: 12px;
    cursor: pointer;
  }
  .fold-x:hover:not(:disabled),
  .fold-x:focus-visible {
    color: var(--rosso-bright);
  }
  .fold-x:disabled {
    cursor: default;
    opacity: 0.5;
  }
</style>
