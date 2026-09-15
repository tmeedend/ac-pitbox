<script lang="ts">
  // Bottom-right notification stack. Everything transient the app has to say
  // goes here, one column, most recent nearest the corner.
  //
  // It exists because two `position: fixed` cards pinned to the same corner do
  // not stack, they overlap: a second import used to hide the first import's
  // report completely, with no way of getting it back. Position lives here and
  // nowhere else, so a new kind of notification can never re-create that.
  import type { Snippet } from "svelte";
  const { children }: { children: Snippet } = $props();
</script>

<div class="stack">{@render children()}</div>

<style>
  .stack {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 80;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
    /* The column is only as wide and tall as its content, but the gaps
       between two toasts still belong to it — and they sit over the screen
       below. Each toast takes its own events back. */
    pointer-events: none;
  }
  .stack > :global(*) {
    pointer-events: auto;
  }
</style>
