<script lang="ts">
  // The agenda template (SPEC-story-mode, Content types, templates and
  // frames): the calendar entries in the order they came, each under its
  // date - a championship calendar, an appointment. An entry is the item's
  // text: the dates in it are the author's words. Shared by the preview and
  // Story Mode.
  import type { PlayerStep, StatementLine } from "$lib/story/player/types";

  interface Props {
    /** The entries, the earliest first; `when` is its date. */
    entries: StatementLine[];
  }
  const { entries }: Props = $props();

  function text(steps: PlayerStep[]): string {
    return steps.map((s) => s.text).join(" ");
  }
</script>

<div class="agenda">
  <ul>
    {#each entries as e (e.key)}
      <li>
        <span class="when">{e.when}</span>
        <span class="text">{text(e.steps)}</span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .agenda {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }
  /* The date first, in the phone's accent: it is what an agenda is read by. */
  .when {
    color: var(--blue);
    font-size: 11px;
    font-weight: 600;
  }
  .text {
    color: var(--txt);
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
</style>
