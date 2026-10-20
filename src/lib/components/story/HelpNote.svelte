<script lang="ts">
  // A note beside a block of the editor, saying how to fill it in: the
  // theme's block (`.blk`), quieter than the form it explains. Its content is
  // the caller's - a few sentences, examples in `code`.
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    children: Snippet;
  }
  const { title, children }: Props = $props();
</script>

<aside class="blk help">
  <div class="blk-h"><span class="blk-n">{title}</span></div>
  <div class="blk-b">{@render children()}</div>
</aside>

<style>
  .help {
    background: transparent;
  }
  .help .blk-h {
    justify-content: flex-start;
  }
  .help .blk-b {
    padding: 12px 14px;
    font-size: 12px;
    line-height: 1.55;
    color: var(--txt2);
  }
  .help :global(p) {
    margin: 0 0 8px;
  }
  .help :global(p:last-child),
  .help :global(dl:last-child) {
    margin-bottom: 0;
  }
  .help :global(code) {
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--txt);
  }
</style>
