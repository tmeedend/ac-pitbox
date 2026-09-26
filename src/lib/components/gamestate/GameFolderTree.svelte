<script lang="ts">
  // The tree of the game folder (DOSSIER§8.1). **Virtualised**: only the lines
  // in view exist in the DOM, and the children of a folder are asked of the
  // index in memory when it is unfolded - never the whole tree in the webview,
  // an install counts hundreds of thousands of entries.
  //
  // What is entirely nobody's in a folder comes as ONE folded line (DOSSIER§4.5),
  // unfolded in place on demand; a folder of more than 500 lines is served in
  // slices of 500.
  import { tick } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import StatePills from "./StatePills.svelte";
  import { filtersKey } from "$lib/gamestate/filters";
  import {
    gameFolderChildren,
    gameFolderReveal,
    type ChildrenPage,
    type Filters,
    type NodeId,
    type Row,
  } from "$lib/gamestate/gamestate";

  interface Props {
    filters: Filters;
    /** Bumped when the index changes (a new scan): everything is asked again. */
    revision: number;
    selected: NodeId | null;
    onselect: (row: Row, path: string) => void;
  }
  let { filters, revision, selected, onselect }: Props = $props();

  const ROOT = 0;
  const EMPTY_COUNTS = { posed: 0, replacesGame: 0, waiting: 0, drift: 0, nobody: 0, cmZone: 0, shared: 0 };
  const SLICE = 500;
  /** The height of an inventory line: the tree reads like the other lists. */
  const ROW_H = 28;
  /** Lines rendered beyond the visible ones, above and below. */
  const OVERSCAN = 12;

  interface Page extends ChildrenPage {
    loading: boolean;
  }
  type Line =
    | { type: "row"; key: string; depth: number; row: Row; path: string; parent: NodeId }
    | { type: "group"; key: string; depth: number; parent: NodeId; count: number; label: string; open: boolean }
    | { type: "more"; key: string; depth: number; parent: NodeId; members: boolean; remaining: number }
    | { type: "loading"; key: string; depth: number };

  let pages = $state<Record<string, Page>>({});
  let expanded = $state<Record<number, boolean>>({});
  let groupOpen = $state<Record<number, boolean>>({});
  let error = $state("");
  let scroller = $state<HTMLElement | null>(null);
  let scrollTop = $state(0);
  let viewport = $state(400);

  const pageKey = (id: NodeId, members: boolean) => `${id}|${members ? "m" : "c"}`;
  // Pages answer one filter state: a filter change asks everything again.
  let loadedFor = "";

  $effect(() => {
    const key = `${revision}|${filtersKey(filters)}`;
    if (key === loadedFor) return;
    loadedFor = key;
    pages = {};
    inFlight.clear();
    void ensure(ROOT, false);
  });

  // Every unfolded folder, and every unfolded folded line, has its page.
  $effect(() => {
    void revision;
    for (const id of Object.keys(expanded)) if (expanded[+id]) void ensure(+id, false);
    for (const id of Object.keys(groupOpen)) if (groupOpen[+id]) void ensure(+id, true);
  });

  /** Requests in flight, by page: a second caller waits for the first
   * instead of reading an empty page - `reveal` runs while the tree's own
   * effects are still loading what it needs. */
  const inFlight = new Map<string, Promise<Page | null>>();

  function ensure(id: NodeId, members: boolean): Promise<Page | null> {
    const k = pageKey(id, members);
    const have = pages[k];
    if (have && !have.loading) return Promise.resolve(have);
    const pending = inFlight.get(k);
    if (pending) return pending;
    pages[k] = { counts: EMPTY_COUNTS, rows: [], total: 0, group: null, loading: true };
    const asked = loadedFor;
    const request = gameFolderChildren(id, filters, members, 0, SLICE)
      .then((page) => {
        if (asked !== loadedFor) return null;
        pages[k] = { ...page, loading: false };
        error = "";
        return pages[k];
      })
      .catch((e) => {
        if (asked === loadedFor) {
          delete pages[k];
          error = errorText(e);
        }
        return null;
      })
      .finally(() => inFlight.delete(k));
    inFlight.set(k, request);
    return request;
  }

  async function loadMore(id: NodeId, members: boolean) {
    const k = pageKey(id, members);
    const page = pages[k];
    if (!page || page.loading) return;
    const asked = loadedFor;
    const next = await gameFolderChildren(id, filters, members, page.rows.length, SLICE).catch((e) => {
      error = errorText(e);
      return null;
    });
    if (!next || asked !== loadedFor) return;
    pages[k] = { ...page, rows: [...page.rows, ...next.rows], total: next.total };
  }

  const GROUP_LABEL: Record<string, string> = {
    originCars: "gamefolder.groupOriginCars",
    originTracks: "gamefolder.groupOriginTracks",
    nobody: "gamefolder.groupNobody",
  };

  /** The lines in view order: rows, their unfolded children, the "next 500"
   * line, then the folded line of what is nobody's and, unfolded, its
   * members. */
  const lines = $derived.by<Line[]>(() => {
    const out: Line[] = [];
    const walk = (id: NodeId, path: string, depth: number) => {
      const page = pages[pageKey(id, false)];
      if (!page || page.loading) {
        out.push({ type: "loading", key: `l${id}`, depth });
        return;
      }
      const rows = (list: Row[]) => {
        for (const row of list) {
          const p = path ? `${path}\\${row.name}` : row.name;
          out.push({ type: "row", key: `r${row.id}`, depth, row, path: p, parent: id });
          if (row.kind === "dir" && expanded[row.id]) walk(row.id, p, depth + 1);
        }
      };
      rows(page.rows);
      if (page.total > page.rows.length) {
        out.push({ type: "more", key: `m${id}`, depth, parent: id, members: false, remaining: page.total - page.rows.length });
      }
      if (page.group) {
        const open = !!groupOpen[id];
        out.push({
          type: "group",
          key: `g${id}`,
          depth,
          parent: id,
          count: page.group.count,
          label: GROUP_LABEL[page.group.label],
          open,
        });
        if (open) {
          const members = pages[pageKey(id, true)];
          if (!members || members.loading) out.push({ type: "loading", key: `lm${id}`, depth });
          else {
            rows(members.rows);
            if (members.total > members.rows.length) {
              out.push({
                type: "more",
                key: `mm${id}`,
                depth,
                parent: id,
                members: true,
                remaining: members.total - members.rows.length,
              });
            }
          }
        }
      }
    };
    walk(ROOT, "", 0);
    return out;
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  const visible = $derived(lines.slice(first, first + Math.ceil(viewport / ROW_H) + 2 * OVERSCAN));
  const cursor = $derived(lines.findIndex((l) => l.type === "row" && l.row.id === selected));

  function toggle(row: Row) {
    if (row.kind === "dir" && row.hasChildren) expanded[row.id] = !expanded[row.id];
  }

  function activate(line: Line) {
    if (line.type === "row") {
      onselect(line.row, line.path);
      toggle(line.row);
    } else if (line.type === "group") {
      groupOpen[line.parent] = !groupOpen[line.parent];
    } else if (line.type === "more") {
      void loadMore(line.parent, line.members);
    }
  }

  /** Scrolls the tree's own container - never an ancestor (CLAUDE.md,
   * programmatic scrolling). */
  function scrollToIndex(i: number) {
    if (!scroller) return;
    const top = i * ROW_H;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW_H > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = top + ROW_H - scroller.clientHeight;
  }

  async function focusIndex(i: number) {
    const line = lines[i];
    if (!line) return;
    if (line.type === "row") onselect(line.row, line.path);
    scrollToIndex(i);
    await tick();
    scroller?.querySelector<HTMLElement>(`[data-line="${line.key}"]`)?.focus();
  }

  /** The keys of a tree (DOSSIER§8.3): up and down move, right unfolds then
   * enters, left folds then climbs, Enter activates. */
  function onKeydown(ev: KeyboardEvent) {
    const here = document.activeElement?.getAttribute("data-line");
    const i = here ? lines.findIndex((l) => l.key === here) : cursor;
    const line = lines[i];
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      const next = Math.min(lines.length - 1, Math.max(0, (i < 0 ? -1 : i) + (ev.key === "ArrowDown" ? 1 : -1)));
      void focusIndex(next);
    } else if (ev.key === "ArrowRight" && line?.type === "row" && line.row.kind === "dir") {
      ev.preventDefault();
      if (!expanded[line.row.id]) toggle(line.row);
      else void focusIndex(i + 1);
    } else if (ev.key === "ArrowLeft" && line) {
      ev.preventDefault();
      if (line.type === "row" && expanded[line.row.id]) {
        expanded[line.row.id] = false;
        return;
      }
      if (line.type === "group" && line.open) {
        groupOpen[line.parent] = false;
        return;
      }
      const parent = "parent" in line ? line.parent : ROOT;
      const at = lines.findIndex((l) => l.type === "row" && l.row.id === parent);
      if (at >= 0) void focusIndex(at);
    } else if (ev.key === "Enter" && line) {
      ev.preventDefault();
      activate(line);
    }
  }

  /** Unfolds the tree down to `path` and selects it (DOSSIER§7.5): through a
   * folded line when the node is nobody's, through further slices when it is
   * beyond the first 500. `false` when the index does not have the path. */
  export async function reveal(path: string): Promise<boolean> {
    const chain = await gameFolderReveal(path).catch(() => null);
    if (!chain || chain.length === 0) return false;
    let parent = ROOT;
    for (const id of chain) {
      if (!(await findIn(parent, id))) return false;
      if (id !== chain[chain.length - 1]) expanded[id] = true;
      parent = id;
    }
    await tick();
    const i = lines.findIndex((l) => l.type === "row" && l.row.id === chain[chain.length - 1]);
    if (i < 0) return false;
    if (scroller) scroller.scrollTop = Math.max(0, i * ROW_H - viewport / 2);
    await focusIndex(i);
    return true;
  }

  /** Makes `child` a line under `parent`: its page, then the folded line, then
   * as many slices as it takes. */
  async function findIn(parent: NodeId, child: NodeId): Promise<boolean> {
    for (const members of [false, true]) {
      let page = await ensure(parent, members);
      while (page) {
        if (page.rows.some((r) => r.id === child)) {
          if (members) groupOpen[parent] = true;
          return true;
        }
        if (page.rows.length >= page.total) break;
        await loadMore(parent, members);
        page = pages[pageKey(parent, members)];
      }
      if (!members && !pages[pageKey(parent, false)]?.group) return false;
    }
    return false;
  }

  export function focus() {
    const i = cursor >= 0 ? cursor : 0;
    void focusIndex(i);
  }
</script>

{#if error}
  <p class="errbox">{error}</p>
{/if}
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="tree"
  role="tree"
  tabindex="-1"
  aria-label={t("gamefolder.treeLabel")}
  bind:this={scroller}
  bind:clientHeight={viewport}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
  onkeydown={onKeydown}
>
  <div class="spacer" style="height: {lines.length * ROW_H}px">
    <div class="slice" style="transform: translateY({first * ROW_H}px)">
      {#each visible as line (line.key)}
        {#if line.type === "row"}
          {@const r = line.row}
          <button
            type="button"
            class="line"
            class:sel={r.id === selected}
            class:nobody={r.state === "nobody" && r.kind !== "dir"}
            class:missing={!r.present}
            role="treeitem"
            aria-selected={r.id === selected}
            aria-expanded={r.kind === "dir" && r.hasChildren ? !!expanded[r.id] : undefined}
            data-line={line.key}
            style="padding-left: {12 + line.depth * 16}px"
            onclick={() => activate(line)}
          >
            <span class="chev" aria-hidden="true">{r.kind === "dir" && r.hasChildren ? (expanded[r.id] ? "▾" : "▸") : ""}</span>
            <svg class="ico" class:mod={r.modFolder} viewBox="0 0 16 16" aria-hidden="true">
              {#if r.kind === "link"}
                <path d="M6.5 9.5 9.5 6.5M7 4.5l1.2-1.2a2.3 2.3 0 0 1 3.3 3.3L10.3 7.8M9 11.5l-1.2 1.2a2.3 2.3 0 0 1-3.3-3.3l1.2-1.2" />
              {:else if r.kind === "dir"}
                <path d="M1.8 4.2a.8.8 0 0 1 .8-.8h3.3l1.4 1.5h6.1a.8.8 0 0 1 .8.8v6.9a.8.8 0 0 1-.8.8H2.6a.8.8 0 0 1-.8-.8z" />
                {#if r.modFolder}<path d="M5.5 9.4l1.6 1.5 3.4-3.4" />{/if}
              {:else}
                <path d="M4 1.8h5.3L12.2 4.7v9.5H4zM9.3 1.8v2.9h2.9" />
              {/if}
            </svg>
            <span class="nm mono">
              <span class="txt">{r.name}</span>
              {#if r.population === "origin" && line.depth === 2}
                <span class="lbl-key">{t("gamefolder.mentionOrigin")}</span>
              {:else if r.population === "unmanaged" && line.depth === 2}
                <span class="lbl-key">{t("gamefolder.mentionUnmanaged")}</span>
              {/if}
            </span>
            <span class="prov">
              {#if r.provider && r.state !== "nobody"}{r.provider}{/if}
              {#if r.others > 0}<i>+{r.others}</i>{/if}
            </span>
            <StatePills row={r} />
          </button>
        {:else if line.type === "group"}
          <button
            type="button"
            class="line group"
            data-line={line.key}
            aria-expanded={line.open}
            style="padding-left: {12 + line.depth * 16}px"
            onclick={() => activate(line)}
          >
            <span class="chev" aria-hidden="true">{line.open ? "▾" : "▸"}</span>
            <span class="nm">{t(line.label, { count: line.count.toLocaleString() })}</span>
          </button>
        {:else if line.type === "more"}
          <button
            type="button"
            class="line more"
            data-line={line.key}
            style="padding-left: {28 + line.depth * 16}px"
            onclick={() => activate(line)}
          >
            {t("gamefolder.showNext", { count: Math.min(SLICE, line.remaining).toLocaleString() })}
          </button>
        {:else}
          <div class="line loading" style="padding-left: {28 + line.depth * 16}px">{t("common.loading")}</div>
        {/if}
      {/each}
    </div>
  </div>
</div>

<style>
  .tree {
    height: 100%;
    overflow-y: auto;
    outline: none;
  }
  .spacer {
    position: relative;
  }
  .slice {
    position: absolute;
    inset: 0 0 auto 0;
  }
  .line {
    display: grid;
    grid-template-columns: 14px 16px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 28px;
    padding-right: 16px;
    background: none;
    text-align: left;
    white-space: nowrap;
    color: var(--txt2);
  }
  .line:hover {
    background: var(--panel);
  }
  /* The selection is where the panel reads from: a light background and a
     yellow edge, the app's focus colour - never red, which is an accent. */
  .line.sel {
    background: var(--raised);
    box-shadow: inset 2px 0 0 var(--yellow);
  }
  .chev {
    color: var(--muted2);
    font-size: 9px;
    text-align: center;
  }
  .ico {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ico.mod {
    stroke: var(--txt2);
  }
  .nm {
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .txt {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nm .lbl-key {
    text-transform: uppercase;
    flex: none;
  }
  .line.nobody .nm {
    color: var(--muted);
  }
  .line.missing .txt {
    font-style: italic;
    text-decoration: line-through;
    color: var(--muted);
  }
  .prov {
    font-size: 11.5px;
    color: var(--txt2);
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
  }
  .prov i {
    font-style: normal;
    color: var(--muted);
    margin-left: 4px;
  }
  .line.group {
    grid-template-columns: 14px minmax(0, 1fr);
  }
  .line.group .nm {
    font-style: italic;
    color: var(--muted);
  }
  .line.more {
    display: block;
    color: var(--blue);
    font-size: 11.5px;
  }
  .line.loading {
    display: flex;
    color: var(--muted2);
    font-size: 11.5px;
  }
  .errbox {
    margin: 8px 16px;
  }
</style>
