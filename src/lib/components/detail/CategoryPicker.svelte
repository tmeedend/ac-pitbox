<script lang="ts">
  // The list a category is chosen from (REGLES§3 level 4): what the rules can
  // produce, what the user decided on other mods, and `New category…` last.
  // One list for the car and track sheets, the tag field and the group panel,
  // so a category reads the same wherever it is picked.
  //
  // The caller draws the trigger (a chip, a ghost `+`, a button) and gets the
  // gesture to open it; the panel opens under it, or over it for the group
  // panel that sits at the bottom of the screen.
  //
  // The panel is `position: fixed`, like the context menu: the group panel
  // scrolls on its own (`overflow-y: auto`), and a panel hanging off the
  // trigger would be cut by it. The position is measured on the trigger in real
  // window pixels and divided by the interface zoom before it is written
  // (`zoomFactor`, which carries the explanation for the whole app).
  import type { Snippet } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { slugTag } from "$lib/library/trackCategoryNames";
  import { zoomFactor } from "$lib/shell/zoom.svelte";

  interface Props {
    /** The categories offered, read each time the list opens: what others
     * decided changes while one works. */
    choices: () => Promise<string[]> | string[];
    /** The category shown now, marked in the list. */
    current?: string | null;
    /** `New category…` is not offered where only existing ones make sense. */
    allowNew?: boolean;
    placement?: "down" | "up";
    onpick: (tag: string) => void;
    /** Draws what opens the list. */
    trigger: Snippet<[() => void, boolean]>;
  }
  const { choices, current = null, allowNew = true, placement = "down", onpick, trigger }: Props = $props();

  let open = $state(false);
  let items = $state<string[]>([]);
  let writing = $state(false);
  let draft = $state("");
  let root = $state<HTMLElement | null>(null);
  let panel = $state<HTMLElement | null>(null);
  let field = $state<HTMLInputElement | null>(null);
  let pos = $state({ left: 0, top: 0 });

  const typed = $derived(slugTag(draft));

  async function toggle() {
    if (open) return close();
    writing = false;
    draft = "";
    try {
      items = await choices();
    } catch (e) {
      // A list that cannot be read still offers `New category…`.
      console.warn("category choices", e);
      items = [];
    }
    open = true;
  }

  function close() {
    open = false;
    writing = false;
  }

  function pick(tag: string) {
    close();
    onpick(tag);
  }

  function commitNew() {
    if (typed) pick(typed);
  }

  function onfieldkey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitNew();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  $effect(() => {
    if (writing) field?.focus();
  });

  // Placed once the panel is in the page, since its height is what puts it
  // over the trigger; read again when the list grows (the field of `New
  // category…` replaces a line). Kept inside the window.
  $effect(() => {
    if (!open || !panel || !root) return;
    void [items, writing, typed];
    const factor = zoomFactor();
    const at = root.getBoundingClientRect();
    const box = panel.getBoundingClientRect();
    const top = placement === "up" ? at.top - box.height - 4 : at.bottom + 4;
    pos = {
      left: Math.max(4, Math.min(at.left, window.innerWidth - box.width - 8)) / factor,
      top: Math.max(4, Math.min(top, window.innerHeight - box.height - 8)) / factor,
    };
  });

  // Closes on a click elsewhere or Escape. Registered only while open.
  $effect(() => {
    if (!open) return;
    // `composedPath`, not `contains(target)`: the click on `New category…`
    // replaces its own button by the field, so by the time this runs the target
    // is no longer in the page and would read as "elsewhere".
    const away = (e: MouseEvent) => {
      if (root && !e.composedPath().includes(root)) close();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    // A fixed panel does not follow what scrolls under it: it goes away - but
    // not when it is its own list that scrolls.
    const leave = () => close();
    const scrolled = (e: Event) => {
      if (!(panel && e.target instanceof Node && panel.contains(e.target))) close();
    };
    document.addEventListener("click", away);
    document.addEventListener("keydown", key);
    window.addEventListener("resize", leave);
    document.addEventListener("scroll", scrolled, true);
    return () => {
      document.removeEventListener("click", away);
      document.removeEventListener("keydown", key);
      window.removeEventListener("resize", leave);
      document.removeEventListener("scroll", scrolled, true);
    };
  });
</script>

<span class="picker" bind:this={root}>
  {@render trigger(toggle, open)}
  {#if open}
    <ul
      class="panel"
      role="listbox"
      bind:this={panel}
      data-gp-overlay
      style="left:{pos.left}px; top:{pos.top}px;"
    >
      {#each items as tag (tag)}
        <li>
          <button class="item" class:cur={tag === current} type="button" role="option" aria-selected={tag === current} onclick={() => pick(tag)}>
            {tag}
          </button>
        </li>
      {/each}
      {#if allowNew}
        <li class="new">
          {#if writing}
            <input
              class="input"
              type="text"
              autocomplete="off"
              placeholder={t("categoryDecision.newPlaceholder")}
              bind:this={field}
              bind:value={draft}
              onkeydown={onfieldkey}
            />
            {#if typed}<span class="slug">{typed}</span>{/if}
          {:else}
            <button class="item add" type="button" onclick={() => (writing = true)}>{t("categoryDecision.new")}</button>
          {/if}
        </li>
      {/if}
    </ul>
  {/if}
</span>

<style>
  .picker {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .panel {
    position: fixed;
    z-index: 200;
    min-width: 160px;
    max-height: calc(40 * var(--vh));
    overflow-y: auto;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--line);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.45);
  }
  .item {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    padding: 5px 12px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--txt2);
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .item.cur {
    color: var(--rosso-bright);
  }
  .item.add {
    color: var(--muted);
    border-top: 1px solid var(--line);
  }
  .new {
    margin-top: 2px;
  }
  .new .input {
    width: calc(100% - 16px);
    margin: 4px 8px 0;
    font-size: 11px;
  }
  .slug {
    display: block;
    padding: 3px 12px 5px;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--muted);
  }
</style>
