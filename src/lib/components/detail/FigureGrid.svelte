<script lang="ts">
  // The ruled grid of the key figures (FICHE§7.1), shared by the sheet and its
  // edit mode: three columns, two when the sheet is narrow.
  //
  // The rules are each cell's right and bottom borders, the outer ones pushed
  // out of the clipped frame. A 1px gap painted with `--line` — the former
  // sheet's way — shows as a grey block wherever the last row is not full.
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="wrap">
  <div class="grid">{@render children()}</div>
</div>

<style>
  /* Measures its own width: it shares its row with the power curve, and the
     page's width says nothing of what is left to it. */
  .wrap {
    container: figures / inline-size;
    border-bottom: 1px solid var(--line);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    overflow: hidden;
  }
  @container figures (max-width: 330px) {
    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  /* `:global`: the cells are the caller's markup. */
  .grid > :global(*) {
    margin: 0 -1px -1px 0;
    padding: 9px 12px;
    border-right: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
</style>
