<script lang="ts">
  // The replies a phone template offers at the bottom of an item, answered
  // by their label (SPEC-story-mode, Dialogue text). Nothing when there are
  // none.
  import type { PlayerChoice } from "$lib/story/player/types";

  interface Props {
    choices: PlayerChoice[];
    onchoose: (label: string) => void;
  }
  const { choices, onchoose }: Props = $props();
</script>

{#if choices.length}
  <div class="replies">
    {#each choices as c (c.label)}
      <button class="reply" onclick={() => onchoose(c.label)}>{c.text}</button>
    {/each}
  </div>
{/if}

<style>
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
