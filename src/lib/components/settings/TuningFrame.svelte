<script lang="ts">
  // The frame both tuning screens share — Réglages › Aperçu 3D and ›
  // Vignettes: what a setting changes stays in sight while the settings
  // scroll.
  //
  // **The left column stays put** (`sticky`) and carries what one looks at
  // and chooses from: the presets when there are some, the preview, then what
  // belongs to that screen alone (rendering, the library's thumbnails). **The
  // right column carries the settings that change the image**, in the same
  // groups and the same order on both screens: framing, light, floor. Scrolling
  // moves only them. Reported on screen: with everything in one flow, a few
  // turns of the wheel down to a slider took the preview out of sight.
  //
  // Too narrow for two columns, they stack, and the preview alone sticks to
  // the top so that it still shows above the slider being moved.
  import type { Snippet } from "svelte";

  interface Props {
    /** Above the preview: the preset being edited, on the thumbnails screen. */
    head?: Snippet;
    preview: Snippet;
    /** Under the preview: what belongs to this screen alone. */
    side?: Snippet;
    /** The settings that change the image. */
    children: Snippet;
  }
  let { head, preview, side, children }: Props = $props();
</script>

<div class="frame">
  <div class="cols">
    <div class="left">
      {#if head}{@render head()}{/if}
      <div class="preview">{@render preview()}</div>
      {#if side}{@render side()}{/if}
    </div>
    <div class="right">{@render children()}</div>
  </div>
</div>

<style>
  .frame {
    container: tuning / inline-size;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    gap: 14px;
    align-items: start;
  }
  .left,
  .right {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  /* Its own scroll when it is taller than the window, so that sticking never
     hides its bottom. The 96 px are the title bar and the screen's padding;
     `--vh` and not `vh`, which the interface zoom would apply twice. */
  .left {
    position: sticky;
    top: 0;
    max-height: calc(100 * var(--vh) - 96px);
    overflow-y: auto;
  }
  /* The blocks keep no margin of their own: the gap spaces them. */
  .left :global(.blk),
  .right :global(.blk) {
    margin: 0;
  }
  @container tuning (max-width: 780px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
    /* Its children become the grid's own: a sticky element sticks only within
       its parent, and the preview has to stay up while the settings, below
       the whole left column, scroll by. */
    .left {
      display: contents;
    }
    .preview {
      position: sticky;
      top: 0;
      z-index: 3;
      background: var(--bg);
    }
  }
</style>
