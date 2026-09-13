<script lang="ts">
  // Opponent picker (§9.3) — the same filter bar the car library uses, over
  // the same library, in a modal.
  //
  // It used to offer a search box and nothing else, so picking an opponent out
  // of ~300 mods was harder than picking the car one drives — the inverse of
  // what one would expect. It now consumes `FilterBar` and the shared search
  // index, which is why this file carries no filtering logic of its own.
  //
  // **The whole car library comes in, not the pool of the active grid mode.**
  // The chips do the narrowing, and that is what makes them meaningful:
  // removing `Category` has to actually widen the list. The tab says what the
  // `+` of the grid draws from; this modal says what one takes by hand, and
  // the two are allowed to diverge.
  //
  // One component, two modes. "Replace" is not a second modal: it drops the
  // tick column, selects one row, and opens scrolled to the car already there.
  import { onMount, tick, untrack } from "svelte";
  import { matchesQuery } from "$lib/cardSearch";
  import { hasOwnDriver } from "$lib/driverOverride.svelte";
  import { buildCardIndex, buildPredicate, filterDefs, type FilterMap } from "$lib/filters";
  import type { ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import FilterBar from "./filters/FilterBar.svelte";
  import StateBadge from "./StateBadge.svelte";

  interface Props {
    /** The whole car library — see the header. */
    pool: ModCard[];
    mode: "add" | "replace";
    /** Chips derived from the active grid mode, posed at opening (§9.3). */
    initialFilters: FilterMap;
    /** Car the performance band is measured against (§3.4) — the one being
     * driven. The modal offers the same catalogue as the library, so the same
     * reference has to reach it, or a `Performance` chip posed here would say
     * it has nothing to compare to. */
    perfRefId?: string | null;
    /** "add": how many opponents the grid holds, and how many are asked for. */
    gridCount?: number;
    gridTarget?: number;
    /** "replace": which row, and what sits in it today. */
    slotNumber?: number;
    currentCarId?: string | null;
    onadd: (carIds: string[]) => void;
    onreplace: (carId: string) => void;
    onclose: () => void;
  }
  let {
    pool,
    mode,
    initialFilters,
    perfRefId = null,
    gridCount = 0,
    gridTarget = 0,
    slotNumber = 1,
    currentCarId = null,
    onadd,
    onreplace,
    onclose,
  }: Props = $props();

  const defs = untrack(() => filterDefs("Car"));
  // Captured once: the modal is created anew at each opening, and its filters
  // are deliberately NOT persisted — it always opens on the chips of the
  // current pool, never on those of the previous time (§9.3).
  let filters = $state<FilterMap>(untrack(() => ({ ...initialFilters })));
  let pinned = $state<string[]>(untrack(() => Object.keys(initialFilters)));
  let query = $state("");

  const index = $derived(buildCardIndex(pool, defs, true, hasOwnDriver, perfRefId));
  const matchesFilters = $derived(buildPredicate(defs, filters, index.ctx));
  const results = $derived(
    pool
      .filter((c) => matchesFilters(c) && matchesQuery(c, query))
      .sort((a, b) =>
        `${a.brand ?? ""} ${a.display_name ?? a.id_interne}`.localeCompare(
          `${b.brand ?? ""} ${b.display_name ?? b.id_interne}`,
        ),
      ),
  );

  // --- Selection ---
  let selected = $state<Set<string>>(new Set());
  let single = $state<string | null>(untrack(() => currentCarId));
  /** Anchor of a Shift+click range — the last row toggled on its own. */
  let anchor: string | null = null;

  function toggle(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
    anchor = id;
  }
  /** Shift+click extends from the last row toggled, in the order displayed. */
  function extendTo(id: string) {
    if (!anchor) {
      toggle(id);
      return;
    }
    const ids = results.map((c) => c.id_interne);
    const from = ids.indexOf(anchor);
    const to = ids.indexOf(id);
    if (from < 0 || to < 0) {
      toggle(id);
      return;
    }
    const [lo, hi] = from <= to ? [from, to] : [to, from];
    const next = new Set(selected);
    for (const x of ids.slice(lo, hi + 1)) next.add(x);
    selected = next;
  }
  function onRowClick(c: ModCard, e: MouseEvent) {
    if (mode === "replace") {
      single = c.id_interne;
      return;
    }
    if (e.shiftKey) extendTo(c.id_interne);
    else toggle(c.id_interne);
  }

  const canConfirm = $derived(mode === "add" ? selected.size > 0 : single != null);
  function confirm() {
    if (mode === "replace") {
      if (single) onreplace(single);
      return;
    }
    // In the order displayed, which is the order they were read in.
    const chosen = results.filter((c) => selected.has(c.id_interne)).map((c) => c.id_interne);
    if (chosen.length) onadd(chosen);
  }

  // --- Focus: trapped while open, handed back to whatever opened it ---
  let root: HTMLDivElement | null = null;
  const opener = untrack(() => (typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null));

  onMount(() => {
    // The row already in the slot, scrolled into view — "replace" opens on
    // what it is about to replace, never at the top of a list of 300.
    void tick().then(() => {
      root?.querySelector<HTMLElement>(".row.on")?.scrollIntoView({ block: "center" });
      root?.querySelector<HTMLElement>("input, button")?.focus();
    });
    return () => opener?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key === "Enter" && canConfirm) {
      // Not while typing in the search box or a filter editor: Enter belongs
      // to the field that has the caret.
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      e.preventDefault();
      confirm();
      return;
    }
    if (e.key !== "Tab" || !root) return;
    // Focus trap: a modal one can tab out of leaves the caret on an element
    // hidden behind the scrim.
    const focusable = [...root.querySelectorAll<HTMLElement>("a[href], button:not([disabled]), input, select, [tabindex]")].filter(
      (el) => el.offsetParent !== null,
    );
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop">
  <div class="modal" bind:this={root} role="dialog" aria-modal="true" aria-label={t("launch.opponentPickTitle")}>
    <header>
      <span class="title">
        {mode === "add" ? t("launch.pickerAddTitle") : t("launch.pickerReplaceTitle", { n: slotNumber })}
      </span>
      <span class="ctx">
        {#if mode === "add"}
          {t("launch.pickerGridCount", { count: gridCount, target: gridTarget })}
        {:else}
          {t("launch.pickerCurrently", { car: pool.find((c) => c.id_interne === currentCarId)?.display_name ?? "—" })}
        {/if}
      </span>
      <button class="x" type="button" aria-label={t("common.close")} onclick={onclose}>✕</button>
    </header>

    <div class="filters">
      <FilterBar
        {defs}
        bind:filters
        bind:pinned
        bind:query
        optionsFor={index.optionsFor}
        presets={index.yearPresets}
        resultCount={results.length}
        perfRef={index.ctx.perfRef}
        perfUnreadable={index.perfUnreadable}
      />
    </div>

    <div class="list">
      <div class="head">
        {#if mode === "add"}<span class="c-tick"></span>{/if}
        <span class="lbl-key c-brand">{t("columns.brand")}</span>
        <span class="lbl-key c-model">{t("columns.name")}</span>
        <span class="lbl-key c-cat">{t("columns.category")}</span>
        <span class="lbl-key c-year">{t("columns.year")}</span>
        <span class="lbl-key c-state">{t("columns.active")}</span>
      </div>
      {#each results as c (c.id_interne)}
        {@const on = mode === "add" ? selected.has(c.id_interne) : single === c.id_interne}
        <div
          class="row"
          class:on
          role="button"
          tabindex="0"
          onclick={(e) => onRowClick(c, e)}
          ondblclick={() => mode === "replace" && onreplace(c.id_interne)}
          onkeydown={(e) => {
            if (e.key === " ") {
              e.preventDefault();
              onRowClick(c, e as unknown as MouseEvent);
            }
          }}
        >
          {#if mode === "add"}
            <span class="c-tick"><input type="checkbox" checked={on} tabindex="-1" /></span>
          {/if}
          <span class="c-brand">{c.brand ?? "—"}</span>
          <span class="c-model">{c.display_name ?? c.id_interne}</span>
          <span class="c-cat mono">{c.category ?? "—"}</span>
          <span class="c-year mono">{c.year ?? "—"}</span>
          <span class="c-state"><StateBadge active={c.active} stock={c.is_stock} unmanaged={c.is_unmanaged} /></span>
        </div>
      {/each}
      {#if !results.length}<div class="empty">{t("launch.opponentPickEmpty")}</div>{/if}
    </div>

    <footer>
      {#if mode === "add"}
        <span class="count mono">{t("launch.pickerSelected", { count: selected.size })}</span>
        {#if selected.size}
          <button class="clear" type="button" onclick={() => (selected = new Set())}
            >{t("launch.pickerClearSelection")}</button
          >
        {/if}
      {/if}
      <span class="sp"></span>
      <button class="btn" type="button" onclick={onclose}>{t("common.cancel")}</button>
      <button class="btn go" type="button" disabled={!canConfirm} onclick={confirm}>
        {mode === "add" ? t("launch.pickerAddN", { count: selected.size }) : t("launch.pickerReplace")}
      </button>
    </footer>
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
  /* Neutral frame: a modal is not "retained for the session", so it has no
     claim on the red scale (§7.2ter). It carried a full red border. */
  .modal {
    width: 880px;
    max-width: 92vw;
    height: 620px;
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 12px 14px 10px;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-size: 13px;
    font-weight: 600;
  }
  .ctx {
    flex: 1;
    font-size: 11px;
    color: var(--muted);
  }
  .x {
    background: transparent;
    border: none;
    color: var(--muted2);
    font-size: 13px;
    line-height: 1;
    padding: 2px 4px;
  }
  .x:hover {
    background: transparent;
    color: var(--txt);
  }
  .filters {
    padding: 9px 14px;
    border-bottom: 1px solid var(--line);
  }
  .list {
    flex: 1;
    overflow: auto;
    border-bottom: 1px solid var(--line);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 14px;
    background: var(--raised);
    border-bottom: 1px solid var(--line);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 26px;
    font-size: 11.5px;
    border-bottom: 1px solid var(--panel2);
    cursor: pointer;
  }
  .row:hover:not(.on) {
    background: var(--raised);
  }
  /* Level 2 of the red scale — what is retained, never a full fill (§7.2ter). */
  .row.on {
    background: var(--rosso-dim);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .c-tick {
    width: 14px;
    flex: none;
  }
  .c-tick input {
    appearance: none;
    width: 12px;
    height: 12px;
    border: 1px solid var(--faint);
    background: transparent;
    display: block;
    position: relative;
  }
  .c-tick input:checked {
    background: var(--rosso);
    border-color: var(--rosso);
  }
  .c-tick input:checked::after {
    content: "";
    position: absolute;
    left: 3.5px;
    top: 0.5px;
    width: 3px;
    height: 7px;
    border: solid #fff;
    border-width: 0 1.3px 1.3px 0;
    transform: rotate(42deg);
  }
  .c-brand {
    width: 92px;
    flex: none;
    color: var(--muted);
  }
  .c-model {
    flex: 1;
    min-width: 0;
  }
  .c-brand,
  .c-model {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c-cat {
    width: 104px;
    flex: none;
    font-size: 10px;
    color: var(--muted);
  }
  .c-year {
    width: 40px;
    flex: none;
    font-size: 10px;
    color: var(--muted);
    text-align: right;
  }
  .c-state {
    width: 74px;
    flex: none;
    display: flex;
    justify-content: flex-end;
  }
  .empty {
    padding: 18px 14px;
    color: var(--muted);
    font-size: 11px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
  }
  .count {
    font-size: 11px;
  }
  .clear {
    background: transparent;
    border: none;
    color: var(--muted);
    font-size: 11px;
  }
  .clear:hover {
    background: transparent;
    color: var(--txt);
  }
  .sp {
    flex: 1;
  }
  /* Level 3 — active but secondary: this confirms a selection, it does not
     start the session. */
  .go:not(:disabled) {
    color: var(--rosso-bright);
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
  }
</style>
