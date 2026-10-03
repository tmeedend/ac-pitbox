<script lang="ts" generics="T, C extends TableColumn">
  // The table view (SPEC §7.4), whatever it lists: the library's mods, the
  // Online page's servers. It knows the columns and their gestures — reorder
  // by dragging a header, resize at the junction of two, sort on a click —
  // and nothing of the rows: each cell is drawn by the screen (`cell`), which
  // also owns the sort itself and the persistence of the prefs.
  //
  // Two layouts. By default the table sizes its columns to their content and
  // renders every row: the library's few hundred mods, scrolled by the
  // screen's own container. `rowHeight` makes it **virtual**: it scrolls
  // itself and renders only the rows in view — the lobby holds 9 000 servers —
  // and its columns then take the widths their definitions give, since rows
  // that come and go cannot size them.
  import { onDestroy, type Snippet } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { zoomFactor } from "$lib/shell/zoom.svelte";
  import Tooltip from "./Tooltip.svelte";
  import { moveColumn, visibleColumns, type ColumnsPrefs, type TableColumn } from "$lib/tableColumns";

  interface Props {
    columns: C[];
    prefs: ColumnsPrefs;
    /** The prefs after a gesture (reorder, resize): the screen keeps and saves them. */
    onprefs: (next: ColumnsPrefs) => void;
    rows: T[];
    rowKey: (row: T) => string;
    /** `data-id` of a row, what a screen looks it up by to scroll to it. */
    rowId?: (row: T) => string;
    /** The states a row can be in; their look belongs to the table. */
    rowState?: (row: T) => { sel?: boolean; multisel?: boolean; session?: boolean; nested?: boolean; group?: boolean };
    /** The current sort, drawn as an arrow; `null` for the screen's default order. */
    sort: { key: string; dir: 1 | -1 } | null;
    onsort: (key: string) => void;
    cell: Snippet<[T, C]>;
    onrowclick?: (row: T, e: MouseEvent) => void;
    onrowdblclick?: (row: T) => void;
    onrowcontextmenu?: (e: MouseEvent, row: T) => void;
    /** Virtual layout: every row this tall, in px. */
    rowHeight?: number;
    /** Virtual layout: the rows in view, once the scroll has settled. */
    onvisible?: (rows: T[]) => void;
  }
  let {
    columns,
    prefs,
    onprefs,
    rows,
    rowKey,
    rowId,
    rowState,
    sort,
    onsort,
    cell,
    onrowclick,
    onrowdblclick,
    onrowcontextmenu,
    rowHeight,
    onvisible,
  }: Props = $props();

  const shownColumns = $derived(visibleColumns(columns, prefs));

  // --- Virtual layout ---
  const OVERSCAN = 8;
  let scrollTop = $state(0);
  let viewport = $state(600);
  const first = $derived(rowHeight ? Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN) : 0);
  const count = $derived(rowHeight ? Math.ceil(viewport / rowHeight) + 2 * OVERSCAN : rows.length);
  const visible = $derived(rowHeight ? rows.slice(first, first + count) : rows);
  const below = $derived(rowHeight ? Math.max(0, rows.length - first - visible.length) * rowHeight : 0);

  $effect(() => {
    const shown = visible;
    if (!rowHeight || !onvisible) return;
    const report = onvisible;
    const timer = setTimeout(() => report(shown), 400);
    return () => clearTimeout(timer);
  });

  /** Width of a column: being dragged, chosen by hand, else (virtual) its
   * definition's. */
  function widthOf(col: C): number | undefined {
    if (live?.key === col.key) return live.width;
    return prefs.widths[col.key] ?? (rowHeight ? (col.width ?? 120) : undefined);
  }
  function widthStyle(col: C): string | undefined {
    const w = widthOf(col);
    return w ? `width:${w}px; max-width:${w}px;` : undefined;
  }

  // The native click that follows a mousedown + move + mouseup on a header
  // (resize or reorder) must never sort — reported: resizing a column changed
  // the sort, because the pointer often ends over a neighbouring `<th>` after
  // a horizontal drag, and the click after a mouseup targets that header.
  // `click` is dispatched right after `mouseup`, in the same synchronous
  // sequence: the flag is already set when `onclick` runs.
  let suppressSortClick = false;
  function markSuppressSortClick() {
    suppressSortClick = true;
    setTimeout(() => (suppressSortClick = false), 0);
  }

  // --- Reorder by dragging a header ---
  // Mouse listeners, not native HTML5 drag: dropped after two attempts under
  // WebView2 (a "no entry" cursor that stayed, whatever `setData` and
  // `effectAllowed` said). A threshold of a few pixels tells a drag from a
  // click, or sorting would never fire again.
  const HEADER_DRAG_THRESHOLD = 4;
  let dragKey = $state<string | null>(null);
  let dropTarget = $state<{ key: string; before: boolean } | null>(null);

  function startHeaderDrag(e: MouseEvent, col: C) {
    if (e.button !== 0 || col.locked) return;
    const key = col.key;
    const startX = e.clientX;
    const startY = e.clientY;
    let moved = false;
    function onMove(ev: MouseEvent) {
      if (!moved) {
        if (Math.abs(ev.clientX - startX) < HEADER_DRAG_THRESHOLD && Math.abs(ev.clientY - startY) < HEADER_DRAG_THRESHOLD) return;
        moved = true;
        dragKey = key;
      }
      const target = (document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null)?.closest<HTMLElement>(
        "th[data-col-key]",
      );
      const targetKey = target?.dataset.colKey;
      if (!target || !targetKey || targetKey === key) {
        dropTarget = null;
        return;
      }
      const rect = target.getBoundingClientRect();
      dropTarget = { key: targetKey, before: ev.clientX - rect.left < rect.width / 2 };
    }
    function onUp() {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (moved) {
        if (dropTarget) {
          const order = moveColumn(prefs.order, columns, key, dropTarget.key, dropTarget.before);
          if (order !== prefs.order) onprefs({ ...prefs, order });
        }
        markSuppressSortClick();
      }
      dragKey = null;
      dropTarget = null;
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // --- Resize: a handle at the right of the header, followed with `window`
  // listeners for the length of the gesture — the pointer often leaves it. ---
  const MIN_COLUMN_WIDTH = 50;
  let resizingKey = $state<string | null>(null);
  /** The width under the pointer: handed to the screen — and so written to
   * disk — once, on release, not at every pixel of the drag. */
  let live = $state<{ key: string; width: number } | null>(null);
  let resizeStartX = 0;
  let resizeStartWidth = 0;
  /** A **measured** width (real window pixels) brought back to CSS pixels,
   * the only ones a style takes — see `zoomFactor`. Without it, grabbing a
   * handle at 110 % widened the column by 10 % at once. */
  const measured = (width: number) => width / zoomFactor();

  function startResize(e: MouseEvent, key: string, currentWidth: number) {
    e.preventDefault();
    e.stopPropagation();
    // Idempotent first: a previous gesture that never released (a mouseup
    // missed outside the window) must not stack duplicate listeners.
    stopResizeListeners();
    resizingKey = key;
    resizeStartX = e.clientX;
    resizeStartWidth = measured(currentWidth);
    window.addEventListener("mousemove", onResizeMove);
    window.addEventListener("mouseup", onResizeUp);
  }
  function onResizeMove(e: MouseEvent) {
    if (!resizingKey) return;
    const width = Math.max(MIN_COLUMN_WIDTH, Math.round(resizeStartWidth + measured(e.clientX - resizeStartX)));
    live = { key: resizingKey, width };
  }
  function stopResizeListeners() {
    window.removeEventListener("mousemove", onResizeMove);
    window.removeEventListener("mouseup", onResizeUp);
  }
  function onResizeUp() {
    if (!resizingKey) return;
    resizingKey = null;
    stopResizeListeners();
    if (live) onprefs({ ...prefs, widths: { ...prefs.widths, [live.key]: live.width } });
    live = null;
    markSuppressSortClick();
  }
  /** Keyboard resize (left/right arrows on the focused handle): without it
   * the handle is mouse-only. */
  function adjustColumnWidth(key: string, currentWidth: number, delta: number) {
    const next = Math.max(MIN_COLUMN_WIDTH, Math.round(measured(currentWidth) + delta));
    onprefs({ ...prefs, widths: { ...prefs.widths, [key]: next } });
  }
  /** Double-click (or Enter) on the handle: back to the natural width. */
  function resetColumnWidth(key: string) {
    const { [key]: _removed, ...rest } = prefs.widths;
    onprefs({ ...prefs, widths: rest });
  }
  onDestroy(stopResizeListeners);
</script>

<div
  class="table-wrap"
  class:virtual={!!rowHeight}
  bind:clientHeight={viewport}
  onscroll={rowHeight ? (e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop) : undefined}
>
  <table>
    <thead>
      <tr>
        {#each shownColumns as col (col.key)}
          <th
            data-col-key={col.key}
            class:draggable={!col.locked}
            class:sortable={col.sortable}
            class:dragging={dragKey === col.key}
            class:resizing={resizingKey === col.key}
            class:drop-before={dropTarget?.key === col.key && dropTarget.before}
            class:drop-after={dropTarget?.key === col.key && !dropTarget.before}
            style={widthStyle(col)}
            title={col.locked ? undefined : t("library.dragColumnTooltip")}
            onclick={() => col.sortable && !suppressSortClick && onsort(col.key)}
            onmousedown={(e) => startHeaderDrag(e, col)}
          >
            <span class="th-label">
              <!-- The label of a sortable column is a real button: without it
                   the only focusable thing of the header was the resize
                   handle, and sorting stayed out of reach of the gamepad and
                   the keyboard. No handler on it — the click bubbles to the
                   `<th>`, which sorts (with its anti-drag guard), and so does
                   the `mousedown`: dragging by the label keeps working. -->
              {#if col.sortable}
                <button class="th-sort" type="button">{t(col.labelKey)}</button>
              {:else}
                {t(col.labelKey)}
              {/if}
              {#if col.tooltipKey}
                <Tooltip text={t(col.tooltipKey)} side="bottom" align={col.tooltipAlign ?? "center"}>
                  <button
                    type="button"
                    class="th-info"
                    onclick={(e) => e.stopPropagation()}
                    onmousedown={(e) => e.stopPropagation()}>ⓘ</button
                  >
                </Tooltip>
              {/if}
              {#if sort?.key === col.key}<span class="arrow">{sort.dir === 1 ? "▲" : "▼"}</span>{/if}
            </span>
            <!-- Resize handle (§7.4). A permanent mark, not only on hover:
                 nothing else suggests one can resize here. `role="separator"`
                 + `tabindex` + arrow keys is the WAI-ARIA APG's "focusable
                 separator" pattern, which Svelte's a11y linter does not count
                 as interactive — hence the two ignores, not a real issue. -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <span
              class="col-resize"
              data-gp-skip
              draggable="false"
              role="separator"
              aria-orientation="vertical"
              tabindex="0"
              title={t("library.resizeColumnTooltip")}
              onmousedown={(e) => startResize(e, col.key, (e.currentTarget as HTMLElement).closest("th")!.getBoundingClientRect().width)}
              onclick={(e) => e.stopPropagation()}
              ondblclick={(e) => {
                e.stopPropagation();
                resetColumnWidth(col.key);
              }}
              onkeydown={(e) => {
                if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
                  e.preventDefault();
                  e.stopPropagation();
                  const width = (e.currentTarget as HTMLElement).closest("th")!.getBoundingClientRect().width;
                  adjustColumnWidth(col.key, width, e.key === "ArrowRight" ? 10 : -10);
                } else if (e.key === "Enter") {
                  e.stopPropagation();
                  resetColumnWidth(col.key);
                }
              }}
            ></span>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#if rowHeight && first > 0}
        <tr class="spacer" aria-hidden="true"><td colspan={shownColumns.length} style="height:{first * rowHeight}px"></td></tr>
      {/if}
      {#each visible as row (rowKey(row))}
        {@const state = rowState?.(row) ?? {}}
        <tr
          data-id={rowId?.(row)}
          tabindex="0"
          class={state}
          style={rowHeight ? `height:${rowHeight}px` : undefined}
          onclick={(e) => onrowclick?.(row, e)}
          ondblclick={() => onrowdblclick?.(row)}
          oncontextmenu={(e) => onrowcontextmenu?.(e, row)}
        >
          {#each shownColumns as col (col.key)}
            <td class:mono={col.mono} class:col-resized={!!widthOf(col)} style={widthStyle(col)}>
              {@render cell(row, col)}
            </td>
          {/each}
        </tr>
      {/each}
      {#if below > 0}
        <tr class="spacer" aria-hidden="true"><td colspan={shownColumns.length} style="height:{below}px"></td></tr>
      {/if}
    </tbody>
  </table>
</div>

<style>
  .table-wrap {
    border: 1px solid var(--line);
    /* No overflow here by default: it would be a nested scroll container and
       the sticky headers would stick to it (invisible) instead of the screen's
       scroller, which handles the wide tables' horizontal scroll. */
  }
  /* The virtual table is its own scroller, on both axes. */
  .table-wrap.virtual {
    height: 100%;
    overflow: auto;
    border-width: 1px 0 0;
  }
  table {
    /* `max-content` rather than `100%`: with many visible columns, a table
       capped at its container's width compresses every column instead of
       overflowing — to the point that a column just ticked could become
       nearly invisible rather than trigger the horizontal scroll (reported).
       `min-width: 100%` keeps a table of few columns spread over the width. */
    width: max-content;
    min-width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    /* No text highlighting when clicking to select (rows and headers). */
    user-select: none;
  }
  /* Rows that come and go cannot size their columns: the widths are the
     definitions', and the layout does not wait for the content. */
  .virtual table {
    table-layout: fixed;
  }
  th {
    /* Anchors the resize handle (absolute). */
    position: relative;
    text-align: left;
    padding: 8px 10px;
    color: var(--muted);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
    white-space: nowrap;
  }
  /* Headers stuck to the top of the scroller. `box-shadow` is the reliable
     separator: collapsed borders do not follow a sticky cell under
     Chromium/WebView2. */
  thead th {
    position: sticky;
    top: 0;
    z-index: 5;
    box-shadow: inset 0 -1px 0 var(--line);
  }
  th.sortable {
    cursor: pointer;
    user-select: none;
  }
  th.sortable:hover {
    color: var(--txt2);
  }
  th .arrow {
    margin-left: 4px;
    /* Sorting is the table's structure: no red (§7.2ter). */
    color: var(--txt2);
  }
  .th-label {
    padding-right: 8px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  /* A button only to be reachable (gamepad/keyboard focus): it must change
     nothing of the header's look, already clickable all over. */
  .th-sort {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    color: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    text-align: left;
    cursor: inherit;
  }
  .th-info {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--faint);
    font-size: 10px;
    line-height: 1;
    cursor: help;
  }
  /* Focus stays yellow, as everywhere: red is the session's (§7.2ter). */
  .th-info:hover {
    color: var(--muted);
  }
  th.draggable {
    cursor: grab;
  }
  th.dragging {
    opacity: 0.4;
  }
  /* Drop mark, on the side the dragged column would land. Neutral: moving a
     column concerns neither the session nor what it holds (§7.2ter). */
  th.drop-before::before,
  th.drop-after::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--txt2);
    z-index: 3;
  }
  th.drop-before::before {
    left: 0;
  }
  th.drop-after::after {
    right: 0;
  }
  /* Resize handle: a strip at the junction of two columns, with a permanent
     mark — a cursor change alone does not make the function discoverable. */
  .col-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    right: -4px;
    width: 8px;
    cursor: col-resize;
    z-index: 2;
  }
  .col-resize::after {
    content: "";
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 3px;
    width: 2px;
    background: var(--line);
  }
  /* Hover lightens the handle, never reddens it (§7.2ter). */
  .col-resize:hover::after,
  .col-resize:focus-visible::after,
  th.resizing .col-resize::after {
    background: var(--faint);
  }
  .col-resize:focus-visible {
    outline: none;
  }
  td.col-resized {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  td {
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    color: var(--txt2);
    white-space: nowrap;
  }
  .virtual td {
    padding-top: 0;
    padding-bottom: 0;
  }
  tr.spacer td {
    padding: 0;
    border: none;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--raised);
  }
  tbody tr.sel {
    background: var(--rosso-dim);
  }
  tbody tr.multisel {
    background: var(--blue-dim);
    box-shadow: inset 2px 0 0 var(--blue);
  }
  tbody tr.session {
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  /* A group's rows sit under it, a rule on the left saying whose they are. */
  tbody tr.nested {
    box-shadow: inset 2px 0 0 var(--line);
  }
  tbody tr.group td {
    color: var(--txt);
  }
</style>
