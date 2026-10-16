<script lang="ts">
  // One track category pictogram (`trackCategoryIcons.ts`), drawn the same way
  // wherever it appears: the index tile, the filter editor and its chip, the
  // Families tab, the drawn thumbnail of a track the Online page cannot show.
  //
  // The set is drawn in `currentColor`: the parent gives the colour through
  // `color`, as for text, and the holes (`cut`, `cutln`) take `--tile-bg`,
  // the colour of whatever is behind - which is what lets one path read as a
  // road with its markings at 16 px.
  import { trackCategoryGlyph } from "$lib/library/trackCategoryTable.svelte";
  import { trackCategoryIcon } from "$lib/library/trackCategoryIcons";

  interface Props {
    /** The category (`#drift`): its icon as the user set it. */
    tag?: string;
    /** An icon id, drawn as is (the icon picker); `""` is the neutral glyph. */
    icon?: string;
    /** Height in px; the width follows the 120×52 grid. */
    size?: number;
  }
  let { tag, icon, size = 34 }: Props = $props();

  const markup = $derived(icon !== undefined ? trackCategoryIcon(icon) : trackCategoryGlyph(tag ?? ""));
</script>

<svg class="tci" viewBox="0 0 120 52" style:height="{size}px" aria-hidden="true">{@html markup}</svg>

<style>
  .tci {
    width: auto;
    flex: none;
    fill: currentColor;
    display: block;
  }
  .tci :global(.ln) {
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .tci :global(.cut) {
    fill: var(--tile-bg, var(--bg));
  }
  .tci :global(.cutln) {
    fill: none;
    stroke: var(--tile-bg, var(--bg));
  }
  .tci :global(.smoke) {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    opacity: 0.5;
  }
</style>
