<script lang="ts">
  // A text field that suggests existing values while one types - and still takes
  // a new one. The native `<datalist>` it replaces is drawn by the system, out of
  // the theme and out of reach of the gamepad; this list is drawn like the other
  // dropdowns of the app (`CategoryPicker`, `ImageSelectDropdown`).
  //
  // Keys: arrows move in the list, Enter takes the highlighted suggestion (and
  // is otherwise left to the caller: it saves the form), Escape closes the list
  // first and leaves the form to the next press.
  //
  // The panel is `position: fixed`, measured on the field in real window pixels
  // and divided by the interface zoom (`zoomFactor`, which carries the
  // explanation for the whole app).
  import { fold } from "$lib/text";
  import { zoomFactor } from "$lib/shell/zoom.svelte";

  interface Props {
    value: string;
    suggestions: string[];
    ariaLabel: string;
    placeholder?: string;
    /** Called for the keys the list did not use. */
    onkeydown?: (e: KeyboardEvent) => void;
  }
  let { value = $bindable(), suggestions, ariaLabel, placeholder = "", onkeydown }: Props = $props();

  const MAX_ITEMS = 50;
  const uid = $props.id();
  const listId = `${uid}-list`;

  let open = $state(false);
  let active = $state(-1);
  let field = $state<HTMLInputElement | null>(null);
  let panel = $state<HTMLElement | null>(null);
  let pos = $state({ left: 0, top: 0, width: 0 });

  /** What the typed text leaves of the list. Whoever typed exactly a suggestion
   * has nothing left to be offered. */
  const shown = $derived.by(() => {
    const typed = fold(value.trim());
    if (suggestions.some((s) => fold(s) === typed) && typed !== "") return [];
    return suggestions.filter((s) => fold(s).includes(typed)).slice(0, MAX_ITEMS);
  });
  const visible = $derived(open && shown.length > 0);

  function pick(s: string) {
    value = s;
    open = false;
    active = -1;
    field?.focus();
  }

  function keydown(e: KeyboardEvent) {
    if (visible && e.key === "ArrowDown") {
      e.preventDefault();
      active = (active + 1) % shown.length;
    } else if (visible && e.key === "ArrowUp") {
      e.preventDefault();
      active = active <= 0 ? shown.length - 1 : active - 1;
    } else if (visible && e.key === "Enter" && active >= 0) {
      e.preventDefault();
      pick(shown[active]);
    } else if (visible && e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      open = false;
    } else {
      onkeydown?.(e);
    }
  }

  function oninput() {
    open = true;
    active = -1;
  }

  /** Not on a click inside the panel: its buttons take the focus from the field
   * for a moment (gamepad), and the choice must still land. */
  function onblur(e: FocusEvent) {
    if (e.relatedTarget instanceof Node && panel?.contains(e.relatedTarget)) return;
    open = false;
  }

  // Placed under the field once the panel is in the page; read again when the
  // list shrinks as one types.
  $effect(() => {
    if (!visible || !field) return;
    void shown;
    const factor = zoomFactor();
    const at = field.getBoundingClientRect();
    pos = { left: at.left / factor, top: (at.bottom + 2) / factor, width: at.width / factor };
  });

  // A fixed panel does not follow what scrolls under it: it goes away - but not
  // when it is its own list that scrolls.
  $effect(() => {
    if (!visible) return;
    const leave = () => (open = false);
    const scrolled = (e: Event) => {
      if (!(panel && e.target instanceof Node && panel.contains(e.target))) open = false;
    };
    window.addEventListener("resize", leave);
    document.addEventListener("scroll", scrolled, true);
    return () => {
      window.removeEventListener("resize", leave);
      document.removeEventListener("scroll", scrolled, true);
    };
  });

  // Keeps the highlighted line in view by moving the panel's own scroll, never
  // `scrollIntoView` (it would carry the page along; see `shellScroll`).
  $effect(() => {
    if (!visible || active < 0 || !panel) return;
    const line = panel.querySelectorAll("li")[active];
    if (!line) return;
    if (line.offsetTop < panel.scrollTop) panel.scrollTop = line.offsetTop;
    else if (line.offsetTop + line.offsetHeight > panel.scrollTop + panel.clientHeight)
      panel.scrollTop = line.offsetTop + line.offsetHeight - panel.clientHeight;
  });
</script>

<input
  class="input"
  type="text"
  autocomplete="off"
  role="combobox"
  aria-expanded={visible}
  aria-controls={listId}
  aria-autocomplete="list"
  aria-label={ariaLabel}
  {placeholder}
  bind:this={field}
  bind:value
  {oninput}
  onfocus={() => (open = true)}
  {onblur}
  onkeydown={keydown}
/>
{#if visible}
  <ul
    id={listId}
    class="panel"
    role="listbox"
    bind:this={panel}
    data-gp-overlay
    style="left:{pos.left}px; top:{pos.top}px; min-width:{pos.width}px;"
  >
    {#each shown as s, i (s)}
      <li>
        <!-- `mousedown` default prevented: the field keeps its focus, so the
             list does not close under the click. -->
        <button
          class="item"
          class:on={i === active}
          type="button"
          role="option"
          aria-selected={i === active}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => pick(s)}
        >
          {s}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  /* In a grid cell the field would keep its intrinsic width. */
  .input {
    min-width: 0;
  }
  .panel {
    position: fixed;
    z-index: 200;
    max-height: calc(30 * var(--vh));
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
    font-size: 11px;
    color: var(--txt2);
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible,
  .item.on {
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
</style>
