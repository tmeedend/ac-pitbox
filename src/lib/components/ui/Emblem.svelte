<script lang="ts">
  // A logo at a given size - on the light plate when its background is baked
  // in (TAXO§5): a white square around a crest bleeds on a dark interface,
  // and on a plate it reads as a choice instead of a defect.
  //
  // Knows nothing of brands or cars (a `ui/` brick): the caller says whether
  // this file goes on the plate - `$lib/library/brandLogos` knows, per file.
  interface Props {
    /** Resolved URL of the image. */
    src: string;
    plaque?: boolean;
    /** Side of the square it is drawn in, plate included, in CSS px. */
    size: number;
  }
  let { src, plaque = false, size }: Props = $props();
</script>

<img class="emblem" class:plaque {src} alt="" loading="lazy" style="width:{size}px;height:{size}px" />

<style>
  .emblem {
    flex: none;
    object-fit: contain;
    box-sizing: border-box;
    vertical-align: middle;
  }
  /* TAXO§11: light plate, radius 2, inner margin 2. The spec's `#ececed` is
     the app's lightest text tone here - no new grey for one surface. */
  .plaque {
    background: var(--txt);
    border-radius: 2px;
    padding: 2px;
  }
</style>
