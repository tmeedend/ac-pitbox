<script lang="ts">
  // A modal dialog's frame: the dimmed backdrop, the red-bordered panel, its
  // title, a scrolling body and a footer of buttons. It knows nothing of what
  // it holds. Escape closes it, unless `onclose` is left out — a dialog that
  // must not be dismissed (an import running) simply does not pass one.
  //
  // The dialogs written before it (`DeleteDialog`, `NamedListDialog`…) still
  // carry their own copy of this frame.
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    onclose?: () => void;
    /** CSS width of the panel. */
    width?: string;
    children: Snippet;
    footer?: Snippet;
  }
  let { title, onclose, width = "520px", children, footer }: Props = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && onclose) onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop">
  <div class="modal" role="dialog" aria-modal="true" style:width>
    <header><h2>{title}</h2></header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 60%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    max-width: 92vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .body {
    overflow-y: auto;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px;
    border-top: 1px solid var(--line);
  }
</style>
