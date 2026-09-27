<script lang="ts">
  // One aid of the electronics (FICHE§5), in its three states: present (full),
  // absent (dashed, struck through), unknown (dotted — only ever drawn in the
  // edit mode, where it is a state one can pick; the read mode draws nothing).
  //
  // Shared by the sheet and its edit mode so the three states look the same in
  // both: a button there, a plain label here.
  import type { Snippet } from "svelte";

  interface Props {
    state: boolean | null;
    /** The aid's name — a snippet, the read mode prefixing a sign to it. */
    children: Snippet;
    /** A detail after the name: "MGU-K + MGU-H", or the state in words. */
    sub?: string;
    title?: string;
    /** Given, the chip is a button (edit mode). */
    onclick?: () => void;
  }
  let { state, children, sub, title, onclick }: Props = $props();
</script>

{#snippet content()}
  <span class="dot"></span>
  <span class="nm">{@render children()}</span>
  {#if sub}<span class="sub">{sub}</span>{/if}
{/snippet}

{#if onclick}
  <button type="button" class="chip" class:off={state === false} class:unk={state === null} {title} {onclick}>
    {@render content()}
  </button>
{:else}
  <span class="chip" class:off={state === false} class:unk={state === null} {title}>{@render content()}</span>
{/if}

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    padding: 3px 8px;
    border: 1px solid var(--line);
    color: var(--txt2);
    background: var(--panel);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--green);
  }
  .off,
  .unk {
    color: var(--muted2);
    background: transparent;
  }
  .off {
    border-style: dashed;
  }
  .off .dot {
    background: transparent;
    border: 1px solid var(--faint);
  }
  .off .nm {
    text-decoration: line-through;
    text-decoration-color: var(--faint);
  }
  .unk {
    border-style: dotted;
  }
  .unk .dot {
    background: transparent;
    border: 1px dotted var(--faint);
  }
  .sub {
    color: var(--muted);
    font-size: 10.5px;
  }
</style>
