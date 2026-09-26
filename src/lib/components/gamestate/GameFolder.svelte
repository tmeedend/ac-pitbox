<script lang="ts">
  // The Game folder tab of Files (DOSSIER§3): the Assetto Corsa folder as it is
  // on the disk and, for every path, where it comes from. **Read only**
  // (DOSSIER§R1): the only action is to scan again, which only reads.
  //
  // One tree, and Provenance is a filter (DOSSIER§R3): the "by mod" view is the
  // chip, not a second tree. The chips are the library's own filter bar.
  import { onMount, tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPrefs, setUiPref } from "$lib/uiPrefs.svelte";
  import { requestSection } from "$lib/shell/nav.svelte";
  import FilterBar from "$lib/components/filters/FilterBar.svelte";
  import { parseFilters, serializeFilters, type FilterMap, type FilterOption } from "$lib/library/filters";
  import GameFolderTree from "./GameFolderTree.svelte";
  import GameFolderResults from "./GameFolderResults.svelte";
  import GameFolderPanel from "./GameFolderPanel.svelte";
  import { gameFolderFocus, openOwnerSheet } from "$lib/gamestate/gameFolder.svelte";
  import {
    gameFolderDefs,
    normalizeProvenance,
    ownerValue,
    PROVENANCE_KEY,
    STATE_KEY,
    STATE_VALUES,
    stateOn,
    toBackendFilters,
    toggleState,
    withOwner,
  } from "$lib/gamestate/filters";
  import {
    gameFolderChildren,
    gameFolderDetail,
    gameFolderOwners,
    gameFolderSearch,
    gameFolderStatus,
    scanGameFolder,
    showGamePath,
    type Counts,
    type Detail,
    type ModHit,
    type NodeId,
    type Owner,
    type OwnerOption,
    type PathHit,
    type Progress,
    type Row,
    type SearchLimits,
    type SearchResults,
    type StateValue,
    type Status,
  } from "$lib/gamestate/gamestate";

  let status = $state<Status | null>(null);
  let progress = $state<Progress | null>(null);
  let scanError = $state("");
  /** Bumped when a new index is published: the tree asks everything again. */
  let revision = $state(0);

  let filters = $state<FilterMap>({});
  let pinned = $state<string[]>([]);
  let query = $state("");
  let owners = $state<OwnerOption[]>([]);
  /** Names of the owners a chip may name before the list is loaded - the
   * element a sheet sent us to. */
  let knownNames = $state<Record<string, string>>({});

  let selectedId = $state<NodeId | null>(null);
  let selectedPath = $state("");
  let detail = $state<Detail | null>(null);
  let pickedMod = $state<ModHit | null>(null);

  let results = $state<SearchResults | null>(null);
  /** The results list is shown; false once a result was chosen, the text
   * staying in the field (DOSSIER§7.5). */
  let showResults = $state(true);
  let limits = $state<SearchLimits>({ mods: 50, dirs: 50, files: 50 });

  let tree = $state<ReturnType<typeof GameFolderTree> | null>(null);
  let searchEl: HTMLInputElement | null = null;
  let prefsReady = false;

  const index = $derived(status?.index ?? null);
  const running = $derived(status?.running ?? null);
  const backend = $derived(toBackendFilters(filters));
  const searching = $derived(query.trim().length >= 2);

  const ownerNames = $derived.by(() => {
    const names: Record<string, string> = { ...knownNames };
    for (const o of owners) names[ownerValue(o.owner)] = o.owner.name;
    return names;
  });
  const defs = $derived(gameFolderDefs((v) => ownerNames[v] ?? v.slice(v.indexOf(":") + 1)));

  function optionsFor(key: string): FilterOption[] {
    if (key === STATE_KEY) {
      const s = index?.summary;
      return STATE_VALUES.map((v) => ({ value: v, label: t(`gamefolder.state.${v}`), count: s ? countOf(s, v) : 0 }));
    }
    if (key === PROVENANCE_KEY) {
      return owners.map((o) => ({ value: ownerValue(o.owner), label: o.owner.name, count: o.files }));
    }
    return [];
  }

  function countOf(c: Counts, v: StateValue): number {
    return v === "cmZone" ? c.cmZone : v === "shared" ? c.shared : c[v];
  }

  // Provenance names one element (DOSSIER§6.2).
  $effect(() => {
    const next = normalizeProvenance(filters);
    if (next !== filters) filters = next;
  });

  async function refreshStatus(): Promise<Status | null> {
    try {
      status = await gameFolderStatus();
      if (status.error) scanError = errorText(status.error);
      return status;
    } catch (e) {
      scanError = errorText(e);
      return null;
    }
  }

  /** A new index is there: the tree, the owners and the panel follow. */
  async function indexChanged() {
    revision += 1;
    owners = await gameFolderOwners().catch(() => []);
    if (selectedPath) void tree?.reveal(selectedPath);
    void loadDetail(selectedId ?? 0);
    if (searching) void runSearch();
  }

  async function scan() {
    scanError = "";
    progress = { phase: "disk", entries: 0 };
    if (status) status = { ...status, running: progress };
    try {
      await scanGameFolder();
    } catch (e) {
      scanError = errorText(e);
    }
  }

  onMount(() => {
    const unlisten = [
      listen<Progress>("gamefolder://progress", async (e) => {
        const first = progress?.phase !== "classify" && e.payload.phase === "classify";
        progress = e.payload;
        // The disk is read: the tree is usable before the states are known
        // (DOSSIER§5.2).
        if (first) {
          await refreshStatus();
          await indexChanged();
        }
      }),
      listen("gamefolder://done", async () => {
        progress = null;
        await refreshStatus();
        await indexChanged();
      }),
    ];
    // Staleness is only known by asking: an activation from the notification
    // stack while this screen is open must show its banner (DOSSIER§5.4).
    const poll = setInterval(() => {
      if (!running) void refreshStatus();
    }, 4000);

    void (async () => {
      const saved = await getUiPrefs([StorageKey.gameFolderView, StorageKey.gameFolderSelected]);
      const restored = parseFilters(saved[StorageKey.gameFolderView], untrack(() => defs));
      let nextFilters = restored.filters;
      query = restored.query;
      selectedPath = saved[StorageKey.gameFolderSelected] ?? "";
      // A way in (DOSSIER§3.2) replaces what was saved: it asks for one view.
      const focus = gameFolderFocus;
      if (focus.owner) {
        knownNames = { [ownerValue(focus.owner)]: focus.owner.name };
        nextFilters = withOwner({}, focus.owner);
        query = "";
      } else if (focus.drifts) {
        nextFilters = toggleState({}, "drift");
        query = "";
      }
      focus.owner = null;
      focus.drifts = false;
      filters = nextFilters;
      prefsReady = true;

      const st = await refreshStatus();
      if (st?.running) {
        progress = st.running;
        if (st.index) await indexChanged();
        return;
      }
      // First opening of the session, or something Pit Box wrote since: scan.
      // A fresh index is reused as it is (DOSSIER§5.4).
      if (!st?.index || st.index.stale) void scan();
      else await indexChanged();
    })();

    return () => {
      clearInterval(poll);
      for (const u of unlisten) void u.then((f) => f());
    };
  });

  // Filters, search and selection are remembered (DOSSIER§6.4): opening a
  // sheet from the panel and coming back finds the screen as it was left.
  $effect(() => {
    const view = serializeFilters(query, filters);
    const sel = selectedPath;
    if (!prefsReady) return;
    setUiPref(StorageKey.gameFolderView, view);
    setUiPref(StorageKey.gameFolderSelected, sel);
  });

  // --- Search (DOSSIER§7) --------------------------------------------------

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;
  /** The text last searched: only typing brings the list back - a chip, a
   * "see the others" or a new scan refresh it without undoing the jump to
   * the tree (DOSSIER§7.5). */
  let lastQuery = "";
  $effect(() => {
    // Read before any exit: the dependencies of this effect.
    const q = query;
    void backend;
    void limits;
    void revision;
    clearTimeout(searchTimer);
    const typed = q !== lastQuery;
    lastQuery = q;
    if (q.trim().length < 2) {
      results = null;
      showResults = true;
      return;
    }
    if (typed) showResults = true;
    searchTimer = setTimeout(() => void runSearch(), 150);
  });

  async function runSearch() {
    if (!index) return;
    const seq = ++searchSeq;
    try {
      const r = await gameFolderSearch(query, backend, limits);
      if (seq === searchSeq) results = r;
    } catch (e) {
      if (seq === searchSeq) scanError = errorText(e);
    }
  }

  function more(group: "mods" | "dirs" | "files") {
    limits = { ...limits, [group]: 1_000_000 };
  }

  async function pickPath(hit: PathHit) {
    const path = hit.parent ? `${hit.parent}\\${hit.row.name}` : hit.row.name;
    await goTo(path, hit.row.id);
  }

  async function pickMod(hit: ModHit) {
    if (hit.path && hit.node !== null) {
      await goTo(hit.path, hit.node);
      return;
    }
    // Not in the game: nothing to show in the tree - its summary instead.
    selectedId = null;
    detail = null;
    pickedMod = hit;
  }

  async function goTo(path: string, id: NodeId) {
    showResults = false;
    pickedMod = null;
    selectedId = id;
    selectedPath = path;
    void loadDetail(id);
    // The tree mounts on the switch: let it render and bind first.
    await tick();
    await tree?.reveal(path);
  }

  // --- Selection and panel -------------------------------------------------

  let detailSeq = 0;
  async function loadDetail(id: NodeId) {
    const seq = ++detailSeq;
    try {
      const d = await gameFolderDetail(id);
      if (seq === detailSeq) detail = d;
    } catch {
      if (seq === detailSeq) detail = null;
    }
  }

  function select(row: Row, path: string) {
    pickedMod = null;
    if (selectedId === row.id) return;
    selectedId = row.id;
    selectedPath = path;
    void loadDetail(row.id);
  }

  function filterOn(owner: Owner) {
    knownNames = { ...knownNames, [ownerValue(owner)]: owner.name };
    filters = withOwner(filters, owner);
    query = "";
  }

  // --- Keyboard (DOSSIER§8.3) ----------------------------------------------

  function onKeydown(ev: KeyboardEvent) {
    const typing = ev.target instanceof HTMLInputElement || ev.target instanceof HTMLTextAreaElement;
    if ((ev.ctrlKey && ev.key.toLowerCase() === "f") || (!typing && ev.key === "/")) {
      ev.preventDefault();
      searchEl ??= document.querySelector<HTMLInputElement>(".gamefolder .search input");
      searchEl?.focus();
    }
  }

  // --- Header ------------------------------------------------------------

  let now = $state(Date.now());
  onMount(() => {
    const clock = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(clock);
  });
  const scannedAgo = $derived.by(() => {
    if (!index) return "";
    const mins = Math.max(0, Math.round((now - new Date(index.scannedAt).getTime()) / 60_000));
    if (mins < 1) return t("gamefolder.scannedNow");
    if (mins < 60) return t("gamefolder.scannedMinutes", { count: mins });
    return t("gamefolder.scannedHours", { count: Math.round(mins / 60) });
  });

  const SUMMARY: StateValue[] = ["posed", "replacesGame", "waiting", "cmZone", "drift", "nobody", "shared"];
  const DOT: Record<string, string> = {
    posed: "var(--green)",
    replacesGame: "var(--rosso-bright)",
    waiting: "var(--yellow)",
    drift: "var(--yellow)",
    cmZone: "var(--blue)",
    nobody: "var(--muted)",
    shared: "var(--txt2)",
  };
  /** Files passing the filters: the count beside the chips. Asked of the
   * index, since the summary band counts the whole folder. */
  let fileCount = $state(0);
  $effect(() => {
    const f = backend;
    void revision;
    if (!index) return;
    gameFolderChildren(0, f, false, 0, 0)
      .then((p) => (fileCount = p.counts.posed + p.counts.replacesGame + p.counts.waiting + p.counts.drift + p.counts.nobody))
      .catch(() => {});
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="gamefolder">
  <div class="top">
    <div class="head">
      <div class="where">
        <p class="lbl-sub">{t("gamefolder.subtitle")}</p>
        {#if index}<p class="root mono">{index.root}</p>{/if}
      </div>
      <div class="scan">
        {#if running}
          <span>
            {t(running.phase === "disk" ? "gamefolder.scanningDisk" : "gamefolder.scanningClassify", {
              count: (progress ?? running).entries.toLocaleString(),
            })}
          </span>
        {:else if index}
          <span>{scannedAgo} · <b>{index.entries.toLocaleString()}</b> {t("gamefolder.entries")}</span>
        {/if}
        <button type="button" class="btn btn-ghost" disabled={!!running} onclick={() => void scan()}>⟳ {t("gamefolder.rescan")}</button>
      </div>
    </div>

    {#if scanError}<p class="errbox">{scanError}</p>{/if}
    {#if index?.stale && !running}
      <div class="warnbox stale">
        <span>{t("gamefolder.staleBanner")}</span>
        <button type="button" class="btn" onclick={() => void scan()}>{t("gamefolder.rescan")}</button>
      </div>
    {/if}

    {#if index}
      <div class="summary" class:pending={!index.classified}>
        {#each SUMMARY as v (v)}
          <button
            type="button"
            class="bt"
            class:on={stateOn(filters, v)}
            style="--c: {DOT[v]}"
            disabled={!index.classified}
            aria-pressed={stateOn(filters, v)}
            onclick={() => (filters = toggleState(filters, v))}
          >
            <span class="k"><span class="dot"></span>{t(`gamefolder.state.${v}`)}</span>
            <span class="v mono">{index.classified ? countOf(index.summary, v).toLocaleString() : t("gamefolder.classifying")}</span>
          </button>
        {/each}
      </div>
    {/if}

    <div class="bar">
      <FilterBar
        {defs}
        bind:filters
        bind:pinned
        bind:query
        {optionsFor}
        presets={[]}
        resultCount={fileCount}
        countKey="gamefolder.fileCount"
        placeholderKey="gamefolder.searchPlaceholder"
        inlineCount
      />
    </div>
  </div>

  <div class="body">
    <div class="left">
      {#if !index}
        <p class="empty">{running ? t("gamefolder.firstScan") : ""}</p>
      {:else if searching && showResults && results}
        <GameFolderResults
          {results}
          {query}
          onpickmod={(h) => void pickMod(h)}
          onpickpath={(h) => void pickPath(h)}
          onfilter={(h) => filterOn(h.owner)}
          onmore={more}
        />
      {:else}
        {#if searching && results}
          <p class="back">
            <button type="button" class="lnk" onclick={() => (showResults = true)}>
              ← {t("gamefolder.backToResults", { count: results.mods.total + results.dirs.total + results.files.total })}
            </button>
          </p>
        {/if}
        <div class="treewrap">
          <GameFolderTree bind:this={tree} filters={backend} {revision} selected={selectedId} onselect={select} />
        </div>
      {/if}
    </div>
    <GameFolderPanel
      {detail}
      mod={pickedMod}
      onopen={(o) => void openOwnerSheet(o)}
      onfilter={filterOn}
      onexplorer={(id) => void showGamePath(id).catch((e) => (scanError = errorText(e)))}
      onmaintenance={() => void requestSection("maintenance")}
    />
  </div>
</div>

<style>
  .gamefolder {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    container: gamefolder / inline-size;
  }
  .top {
    padding: 0 32px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }
  .where {
    min-width: 0;
  }
  .root {
    font-size: 11.5px;
    color: var(--muted);
    margin-top: 4px;
    word-break: break-all;
  }
  .scan {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .scan b {
    color: var(--txt2);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .stale {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .summary {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  .bt {
    padding: 9px 12px;
    border-right: 1px solid var(--line);
    background: none;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .bt:last-child {
    border-right: 0;
  }
  .bt:hover:not(:disabled) {
    background: var(--panel);
  }
  .bt:disabled {
    cursor: default;
  }
  .bt .k {
    font-size: 9px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    color: var(--muted);
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
  }
  .bt .v {
    font-size: 17px;
    font-variant-numeric: tabular-nums;
    color: var(--txt2);
  }
  .summary.pending .bt .v {
    font-size: 12px;
    color: var(--muted);
  }
  .bt.on {
    background: var(--raised);
    box-shadow: inset 0 -2px 0 var(--c);
  }
  .bt.on .v {
    color: var(--txt);
  }
  .dot {
    width: 7px;
    height: 7px;
    flex: 0 0 7px;
    background: var(--c);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 360px;
    border-top: 1px solid var(--line);
  }
  .left {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .treewrap {
    flex: 1;
    min-height: 0;
    padding-top: 6px;
  }
  .back {
    padding: 8px 16px 0;
    font-size: 11.5px;
  }
  .lnk {
    color: var(--blue);
    background: none;
    padding: 0;
    font: inherit;
  }
  .lnk:hover {
    text-decoration: underline;
  }
  .empty {
    padding: 24px 16px;
    color: var(--muted);
  }
  /* Narrow: the panel goes under the tree instead of beside it
     (DOSSIER§3.3) - measured on the room this screen really has, not on the
     window (CLAUDE.md, a layout threshold is a container query). */
  @container gamefolder (max-width: 1100px) {
    .summary {
      grid-template-columns: repeat(4, 1fr);
    }
    .body {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 1fr) minmax(0, 40%);
    }
  }
</style>
