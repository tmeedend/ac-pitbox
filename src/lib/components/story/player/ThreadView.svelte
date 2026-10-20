<script lang="ts">
  // The thread template (SPEC-story-mode, Content types, templates and
  // frames): a conversation as bubbles, and the replies the player can
  // choose at the bottom. Shared by the preview and Story Mode; the choice
  // goes back through `onchoose`, which decides what playing it means.
  import { tick } from "svelte";
  import { bubbles } from "$lib/story/player/thread";
  import type { PlayerChoice, PlayerStep } from "$lib/story/player/types";

  interface Props {
    steps: PlayerStep[];
    choices: PlayerChoice[];
    /** The item's sender: who says a line without a speaker. */
    from: string | null;
    /** Character id → name. */
    names: Record<string, string>;
    onchoose: (label: string) => void;
  }
  const { steps, choices, from, names, onchoose }: Props = $props();

  const shown = $derived(bubbles(steps, from, names));
  let list = $state<HTMLDivElement | undefined>(undefined);

  // The newest bubble in view: the list scrolls itself, never the page.
  $effect(() => {
    void shown.length;
    void tick().then(() => {
      if (list) list.scrollTop = list.scrollHeight;
    });
  });
</script>

<div class="thread" bind:this={list}>
  {#each shown as b, i (i)}
    {#if b.name}<div class="name">{b.name}</div>{/if}
    <div class="bubble {b.side}">{b.text}</div>
  {/each}
</div>
{#if choices.length}
  <div class="replies">
    {#each choices as c (c.label)}
      <button class="reply" onclick={() => onchoose(c.label)}>{c.text}</button>
    {/each}
  </div>
{/if}

<style>
  .thread {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 10px;
  }
  .name {
    margin: 6px 0 0 4px;
    color: var(--muted);
    font-size: 10.5px;
  }
  .bubble {
    max-width: 78%;
    padding: 7px 11px;
    border-radius: 14px;
    font-size: 12.5px;
    line-height: 1.35;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .them {
    align-self: flex-start;
    background: var(--mat);
    color: var(--txt);
    border-bottom-left-radius: 4px;
  }
  .me {
    align-self: flex-end;
    background: var(--blue-dim);
    color: var(--txt);
    border: 1px solid var(--blue-border);
    border-bottom-right-radius: 4px;
  }
  .replies {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border-top: 1px solid var(--line);
  }
  .reply {
    text-align: left;
    padding: 8px 11px;
    border-radius: 12px;
    background: transparent;
    color: var(--txt2);
    border: 1px solid var(--mat-line);
    font-size: 12.5px;
  }
  .reply:hover {
    border-color: var(--blue-border);
    color: var(--txt);
  }
</style>
