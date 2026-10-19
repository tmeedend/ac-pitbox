<script lang="ts">
  // One of the story's own names - a character, a flag, a counter, a
  // relation step - with what every one of them shares: its name, which
  // renames every sentence citing it (SPEC-story-mode, Create Your Own), the
  // lines citing it, and its removal. What differs (a character's display
  // name, a counter's starting value) comes in as `children`.
  import type { Snippet } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { fileCount, nameProblem, usagesOf } from "$lib/story/world";
  import type { NameKind } from "$lib/story/studio";
  import { renameEverywhere, selectNode, studio } from "$lib/story/studioState.svelte";

  interface Props {
    kind: NameKind;
    id: string;
    /** Names this one cannot take. */
    taken: string[];
    onremove: () => void;
    children?: Snippet;
    /** Shown before the name: a step's move buttons. */
    lead?: Snippet;
  }
  const { kind, id, taken, onremove, children, lead }: Props = $props();

  const usages = $derived(usagesOf(studio.story?.usages ?? [], kind, id));
  let open = $state(false);
  let problem = $state<string | null>(null);

  async function rename(e: Event) {
    const field = e.currentTarget as HTMLInputElement;
    const to = field.value.trim();
    if (to === id) return (problem = null);
    const why = nameProblem(
      to,
      taken.filter((n) => n !== id),
    );
    if (why) return (problem = t(why));
    problem = null;
    if (usages.length > 0) {
      const ok = await confirm(
        t("story.world.renameConfirm", { from: id, to, count: usages.length, files: fileCount(usages) }),
        { title: t("story.world.rename"), kind: "warning" },
      );
      if (!ok) {
        field.value = id;
        return;
      }
    }
    problem = await renameEverywhere(kind, id, to);
    if (problem) field.value = id;
  }

  async function remove() {
    const message =
      usages.length > 0
        ? t("story.world.deleteCitedConfirm", { id, count: usages.length })
        : t("story.world.deleteConfirm", { id });
    if (await confirm(message, { title: t("story.world.delete"), kind: "warning" })) onremove();
  }
</script>

<li class="row">
  <div class="line">
    {@render lead?.()}
    <input class="input mono name" value={id} onchange={rename} aria-label={t("story.world.name")} />
    {@render children?.()}
    <button
      class="cited btn-ghost"
      class:none={usages.length === 0}
      disabled={usages.length === 0}
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      {usages.length === 0 ? t("story.world.notCited") : t("story.world.cited", { count: usages.length })}
      {#if usages.length > 0}<span class="caret">{open ? "▾" : "▸"}</span>{/if}
    </button>
    <button class="btn btn-ghost del" onclick={remove} aria-label={t("story.world.delete")}>✕</button>
  </div>
  {#if problem}<p class="errbox">{problem}</p>{/if}
  {#if open && usages.length > 0}
    <ul class="usages">
      {#each usages as u (u.file + ":" + u.line)}
        <li>
          {#if u.node}
            <button class="where mono" onclick={() => selectNode(u.node!)}>{u.node}</button>
          {:else}
            <span class="where mono file">{u.item ?? `${u.file}:${u.line}`}</span>
          {/if}
          <span class="text mono">{u.text}</span>
        </li>
      {/each}
    </ul>
  {/if}
</li>

<style>
  .row {
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .row:first-child {
    border-top: none;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name {
    width: 190px;
    flex-shrink: 0;
  }
  .cited {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 12px;
    padding: 4px 6px;
    color: var(--txt2);
  }
  .cited.none {
    color: var(--faint);
  }
  .caret {
    margin-left: 4px;
    color: var(--muted);
  }
  .del {
    flex-shrink: 0;
    padding: 4px 8px;
    color: var(--muted);
  }
  .errbox {
    margin: 6px 0 0;
  }
  .usages {
    list-style: none;
    margin: 6px 0 2px;
    padding: 6px 10px;
    background: var(--raised);
  }
  .usages li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 3px 0;
    font-size: 11.5px;
  }
  .where {
    flex-shrink: 0;
    background: transparent;
    color: var(--txt2);
    font-size: 11.5px;
    padding: 0;
  }
  button.where:hover {
    color: var(--txt);
    text-decoration: underline;
  }
  .file {
    color: var(--muted);
  }
  .text {
    color: var(--muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
