<script lang="ts">
  // One row of the Mechanics block (FICHE§7.1): its key on the left, its
  // sentence — or, in edit mode, its fields — on the right. Shared by the sheet
  // and its edit mode, so both keep the same key column.
  import type { Snippet } from "svelte";

  let { label, children }: { label: string; children: Snippet } = $props();
</script>

<div class="row">
  <div class="lbl-key key">{label}</div>
  <div class="body">{@render children()}</div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 92px minmax(0, 1fr);
    gap: 10px;
    align-items: baseline;
    padding: 5px 0;
  }
  .key {
    text-transform: uppercase;
  }
  .body {
    min-width: 0;
  }
</style>
