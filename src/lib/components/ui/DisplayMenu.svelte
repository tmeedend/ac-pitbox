<script lang="ts">
  // The display menu (SPEC §7.4): a chevron that opens a screen's
  // presentation preferences — the library's density and brand, the Online
  // page's grouping. They live **here** and not in the global settings: one
  // has to see their effect to judge them, and a settings screen hides it.
  //
  // The screen writes the menu's content; its labels, `.dm-title` and
  // `.dm-sep` take their look from here (`:global` below, since a snippet
  // keeps the scope of the component that wrote it).
  import type { Snippet } from "svelte";

  interface Props {
    /** The tooltip of the chevron. */
    title: string;
    /** Joined to the control on its left (the library's view switch): the
     * two share their edge, or the chevron reads as one more control instead
     * of that one's complement. */
    attached?: boolean;
    children: Snippet;
  }
  let { title, attached = false, children }: Props = $props();

  let open = $state(false);
</script>

<div class="display-wrap">
  <button class="disp-toggle" class:attached type="button" aria-expanded={open} {title} onclick={() => (open = !open)}
    >▾</button
  >
  {#if open}
    <div class="display-menu">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .display-wrap {
    position: relative;
    display: flex;
  }
  .disp-toggle {
    height: 32px;
    padding: 0 7px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 10px;
    line-height: 1;
  }
  .disp-toggle.attached {
    border-left: none;
  }
  .disp-toggle:hover,
  .disp-toggle[aria-expanded="true"] {
    color: var(--txt);
    border-color: var(--faint2);
  }
  .display-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 20;
    background: var(--panel);
    border: 1px solid var(--line);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 200px;
    box-shadow: 0 6px 18px rgb(0 0 0 / 40%);
  }
  .display-menu :global(label) {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    padding: 3px 4px;
    cursor: pointer;
  }
  .display-menu :global(label:hover) {
    background: var(--raised);
  }
  .display-menu :global(.dm-title) {
    color: var(--muted);
    font-family: var(--mono);
    font-size: 9px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    padding: 2px 4px 4px;
  }
  .display-menu :global(.dm-sep) {
    height: 1px;
    background: var(--line);
    margin: 6px 0;
  }
</style>
