<script lang="ts">
  // An item's text as the inbox, the feed and the statement show it
  // (SPEC-story-mode, Content types, templates and frames): one paragraph per
  // line, a line said by someone else than the sender under their name, and
  // the replies the player chose in their blue.
  import type { PlayerStep } from "$lib/story/player/types";

  interface Props {
    steps: PlayerStep[];
    /** The item's sender: a line without a speaker is theirs. */
    from: string | null;
    /** Character id → name. */
    names: Record<string, string>;
  }
  const { steps, from, names }: Props = $props();
</script>

<div class="body">
  {#each steps as s, i (i)}
    {#if s.kind === "chose"}
      <p class="mine">{s.text}</p>
    {:else}
      <p>
        {#if s.speaker && s.speaker !== from}<span class="who">{names[s.speaker] ?? s.speaker}</span>{/if}
        {s.text}
      </p>
    {/if}
  {/each}
</div>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  p {
    margin: 0;
    color: var(--txt);
    font-size: 12.5px;
    line-height: 1.4;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .who {
    font-weight: 600;
    margin-right: 4px;
  }
  /* The player's reply, in the blue of their replies everywhere else. */
  .mine {
    padding-left: 8px;
    border-left: 2px solid var(--blue);
    color: var(--txt2);
  }
</style>
