<script lang="ts">
  // The Online page (docs/SPEC-play-online.md): the public servers, judged
  // against what can be driven here, and a panel to pick a car and join.
  //
  // Four tabs over the list (SPEC-play-online.md, case 1): All, and the
  // user's own servers — Favourites, Recent, and those a friend is on. The
  // four yes/no chips (not full, no password…) sort out the 9 000 public
  // servers; they are not offered on one's own tabs, whose servers stay listed
  // full, locked or empty: that is exactly when one goes looking for them. The
  // search and the tokens apply everywhere: they are what the user asked for,
  // and the chips say so on every tab.
  //
  // Friends cost a request per busy server (the lobby names nobody), so the
  // scan only runs once someone has been marked, after each list load.
  import { onMount, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import FilterBar from "$lib/components/filters/FilterBar.svelte";
  import type { FilterMap } from "$lib/library/filters";
  import { gameCountryByIso2, withCountryLabels } from "$lib/flags.svelte";
  import {
    serverDetail,
    serverDrivers,
    serverKey,
    type ServerDrivers,
    type ServerSummary,
  } from "$lib/online/online";
  import { parseJoinLink } from "$lib/online/link";
  import { searchServers, sortServers } from "$lib/online/filters";
  import { carName, NO_LOOKS, searchText, type Looks } from "$lib/online/looks";
  import {
    asksPing,
    TRACK_KEY,
    DEFAULT_PINNED,
    DEFAULT_TOKENS,
    onlineTokenDefs,
    parseTokens,
    serializeTokens,
    splitPing,
    tokenOptions,
    tokenPredicate,
    trackLabel,
    trackValue,
    type TokenContext,
  } from "$lib/online/tokens";
  import { measureAll, pingOf, pingsLeft, stopMeasuring } from "$lib/online/pings.svelte";
  import { friendsOnline, recentCars, tabServers, type OnlineTab } from "$lib/online/lists";
  import { ONLINE_COLUMNS } from "$lib/online/columns";
  import { defaultPrefs, loadSavedPrefs, reconcilePrefs, saveTablePrefs, toggleVisible, type ColumnsPrefs } from "$lib/tableColumns";
  import ColumnsMenu from "$lib/components/ui/ColumnsMenu.svelte";
  import DisplayMenu from "$lib/components/ui/DisplayMenu.svelte";
  import { loadOnlineStore, onlineStore } from "$lib/online/store.svelte";
  import { loadLobby } from "$lib/online/lobby.svelte";
  import { pendingOnlineIntent, takeOnlineIntent, type OnlineIntent } from "$lib/online/intent.svelte";
  import ServerTable from "./ServerTable.svelte";
  import ServerDetail from "./ServerDetail.svelte";

  let servers = $state<ServerSummary[]>([]);
  let looks = $state<Looks>(NO_LOOKS);
  let loading = $state(true);
  let error = $state("");
  let search = $state("");
  let tokens = $state<FilterMap>({ ...DEFAULT_TOKENS });
  let pinned = $state<string[]>([...DEFAULT_PINNED]);
  let selected = $state<ServerSummary | null>(null);
  let tab = $state<OnlineTab>("all");
  /** All tab: servers gathered under one row per track (v2 of the spec). */
  let grouped = $state(false);
  /** The table's columns (SPEC §7.4), kept beside the library's. */
  let columnsPrefs = $state<ColumnsPrefs>(defaultPrefs(ONLINE_COLUMNS));
  function setColumnsPrefs(next: ColumnsPrefs) {
    columnsPrefs = next;
    void saveTablePrefs("online", next);
  }
  /** The stored filters are read before the first save, so that restoring
   * them is not taken for a change. A `$state`: the effects that save must
   * run again once it flips. */
  let restored = $state(false);

  /** The list, from the cache the track sheets share while it is fresh;
   * `force` (the Refresh button) asks the lobby anyway. */
  async function refresh(force = false) {
    loading = true;
    error = "";
    try {
      ({ servers, looks } = await loadLobby(force));
      // Keep the open server in step with the fresh list, when it is still there.
      if (selected) {
        const key = serverKey(selected);
        selected = servers.find((s) => serverKey(s) === key) ?? selected;
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  const TABS: OnlineTab[] = ["all", "favourites", "recent", "friends"];

  /** The password a pasted link carried, for the server it opened. */
  let linkPassword = $state<string | null>(null);

  /** A connection link pasted anywhere on the page opens its server (case 1
   * of the spec). Anything else pastes as usual — and a password field keeps
   * what is pasted into it, whatever it looks like. */
  async function onPaste(e: ClipboardEvent) {
    const target = e.target as HTMLInputElement | null;
    if (target?.type === "password") return;
    const link = parseJoinLink(e.clipboardData?.getData("text") ?? "");
    if (!link) return;
    e.preventDefault();
    error = "";
    const key = `${link.ip}:${link.httpPort}`;
    linkPassword = link.password;
    const listed = servers.find((s) => serverKey(s) === key);
    if (listed) {
      selected = listed;
      return;
    }
    // Not in the lobby — a private server, or one the lobby lost: asked
    // directly, as its panel would.
    try {
      selected = (await serverDetail(link.ip, link.httpPort)).summary;
    } catch (err) {
      error = errorText(err);
    }
  }

  /** Who drives where, for the servers that had players at the last load. */
  let scan = $state<ServerDrivers[]>([]);
  let scanning = $state(false);
  /** The list the last scan was started for: a new load scans again. */
  let scannedFor: ServerSummary[] | null = null;

  async function scanFriends(list: ServerSummary[]) {
    scanning = true;
    try {
      scan = await serverDrivers(list.filter((s) => s.clients > 0));
    } catch (e) {
      console.error("online_server_drivers", e);
    } finally {
      scanning = false;
    }
  }

  // Every dependency read before the exit (CLAUDE.md): the list, and whether
  // anyone is marked — marking the first friend starts the scan.
  $effect(() => {
    const list = servers;
    const wanted = onlineStore().friends.length > 0;
    if (!wanted || list.length === 0 || scannedFor === list) return;
    scannedFor = list;
    untrack(() => void scanFriends(list));
  });

  onMount(async () => {
    const [savedFilters, savedTab, savedGrouped, savedColumns] = await Promise.all([
      getUiPref(StorageKey.onlineFilters),
      getUiPref(StorageKey.onlineTab),
      getUiPref(StorageKey.onlineGrouped),
      loadSavedPrefs("online"),
      loadOnlineStore(),
    ]);
    columnsPrefs = reconcilePrefs(savedColumns, ONLINE_COLUMNS);
    // The search is a gesture of the moment; the chips and their pins are
    // remembered — the checkboxes they replaced included (`parseTokens`).
    ({ tokens, pinned } = parseTokens(savedFilters));
    tab = TABS.find((x) => x === savedTab) ?? "all";
    grouped = savedGrouped === "1";
    restored = true;
    await refresh();
  });

  // A way in from elsewhere (`intent.svelte.ts`): a track sheet's line, a
  // notification. Taken once the stored filters are restored, so that the
  // restore does not overwrite it — and at once when the page is already up.
  $effect(() => {
    const intent = pendingOnlineIntent();
    if (!restored || !intent) return;
    untrack(() => applyIntent(takeOnlineIntent()));
  });

  function applyIntent(intent: OnlineIntent | null) {
    if (intent?.kind === "track") {
      tab = "all";
      selected = null;
      const values = intent.layouts.map((value) => ({ value, sign: 1 as const }));
      tokens = { ...tokens, [TRACK_KEY]: { type: "val", values, op: "or" } };
    } else if (intent?.kind === "server") {
      const key = serverKey(intent.server);
      selected = servers.find((s) => serverKey(s) === key) ?? intent.server;
      linkPassword = null;
    }
  }

  $effect(() => {
    const saved = serializeTokens(tokens, pinned);
    const current = tab;
    const group = grouped;
    if (!restored) return;
    setUiPref(StorageKey.onlineFilters, saved);
    setUiPref(StorageKey.onlineTab, current);
    setUiPref(StorageKey.onlineGrouped, group ? "1" : "0");
  });

  const store = $derived(onlineStore());
  const lastCars = $derived(recentCars(store));
  const friends = $derived(friendsOnline(scan, store));

  // The tokens (SPEC-play-online.md, "Filtres de base"). Track names are read
  // off the servers that name them, one's own snapshots included: a token
  // restored for a server the lobby no longer lists keeps its name.
  const trackNames = $derived.by(() => {
    const names: Record<string, string> = {};
    const own = [...store.favourites, ...store.recents.map((r) => r.server)];
    for (const s of [...own, ...servers]) names[trackValue(s.track)] = trackLabel(looks, s.track);
    return names;
  });
  const defs = $derived(
    withCountryLabels(
      onlineTokenDefs(
        {
          track: (v) => trackNames[v] ?? v,
          car: (v) => carName(looks, v),
          ping: (v) => t("online.pingUnder", { ms: v }),
        },
        tab === "all",
      ),
    ),
  );
  const ctx: TokenContext = $derived({
    looks,
    pingOf: (s) => pingOf(serverKey(s)),
    // The game's English name is what flags and labels are keyed on. Fills in
    // once the game's table is loaded (the bar loads it).
    countryName: (iso2) => gameCountryByIso2(iso2)?.name ?? null,
  });

  /** Every filter but the ping: the servers a ping token has to measure. */
  const unpinged = $derived.by(() => {
    const text = (s: ServerSummary) => searchText(looks, s);
    const tokensOk = tokenPredicate(defs, splitPing(tokens).rest, ctx);
    if (tab === "all") {
      const kept = searchServers(servers, search, text).filter(tokensOk);
      return sortServers(kept, new Set(Object.keys(friends)));
    }
    // One's own servers: their catalogue has no yes/no chips (see above).
    return searchServers(tabServers(tab, servers, store, friends), search, text).filter(tokensOk);
  });
  const wantsPing = $derived(asksPing(tokens));
  const shown = $derived.by(() => {
    if (!wantsPing) return unpinged;
    const pingOk = tokenPredicate(defs, splitPing(tokens).ping, ctx);
    return unpinged.filter(pingOk);
  });

  // A ping token cannot keep a server it has not measured: the candidates are
  // measured, top of the list first, and show as they answer. Waits for the
  // typing to settle, like the visible rows wait for the scroll.
  $effect(() => {
    const list = unpinged;
    if (!wantsPing) {
      stopMeasuring();
      return;
    }
    const timer = setTimeout(() => untrack(() => void measureAll(list)), 400);
    return () => clearTimeout(timer);
  });
  // Leaving the page ends the sweep: nobody is reading the list any more.
  $effect(() => () => stopMeasuring());
  const measuring = $derived(wantsPing ? pingsLeft() : 0);
  const tabItems = $derived([
    { id: "all", label: t("online.tabAll") },
    { id: "favourites", label: t("online.tabFavourites"), count: store.favourites.length },
    { id: "recent", label: t("online.tabRecent"), count: store.recents.length },
    { id: "friends", label: t("online.tabFriends"), count: Object.keys(friends).length },
  ]);
  /** What an empty tab says. Written out rather than built from the tab, so
   * the locale check sees every key. */
  const emptyText = $derived.by(() => {
    if (tab === "favourites") return t("online.emptyFavourites");
    if (tab === "recent") return t("online.emptyRecent");
    if (tab === "friends") return store.friends.length ? t("online.emptyFriends") : t("online.emptyNoFriends");
    return t("online.empty");
  });
</script>

<svelte:window onpaste={onPaste} />

<div class="online" class:with-panel={!!selected}>
  <section class="main">
    <header class="head">
      <h2 class="lbl-screen">{t("nav.online")}</h2>
      <Tabs tabs={tabItems} active={tab} onselect={(id) => (tab = id as OnlineTab)} />
      <div class="bar">
        <FilterBar
          {defs}
          bind:filters={tokens}
          bind:pinned
          bind:query={search}
          optionsFor={(key) => {
            const def = defs.find((d) => d.key === key);
            return def ? tokenOptions(def, servers, ctx) : [];
          }}
          presets={[]}
          resultCount={shown.length}
          countKey="online.count"
          placeholderKey="online.searchPlaceholder"
        >
          {#snippet end()}
            {#if measuring > 0}
              <span class="measuring mono">{t("online.pingMeasuring", { count: measuring })}</span>
            {/if}
            <ColumnsMenu
              items={ONLINE_COLUMNS.map((c) => ({ key: c.key, label: t(c.labelKey), fixed: c.fixed }))}
              visible={columnsPrefs.visible}
              ontoggle={(key) => setColumnsPrefs(toggleVisible(columnsPrefs, key))}
            />
            <!-- Grouping is a way of showing, not a filter: it lives with
                 the display, and only for the public servers — one's own
                 lists are short. -->
            <DisplayMenu title={t("online.displayMenu")}>
              <label>
                <input type="checkbox" bind:checked={grouped} disabled={tab !== "all"} />
                <span>{t("online.groupByTrack")}</span>
              </label>
            </DisplayMenu>
            <button class="btn" type="button" disabled={loading} onclick={() => refresh(true)}>{t("online.refresh")}</button>
          {/snippet}
        </FilterBar>
      </div>
    </header>

    {#if error}
      <p class="errbox">{error}</p>
    {/if}
    <!-- One's own servers show at once, from their snapshots: they must not
         wait for the lobby, nor vanish when it is down. -->
    {#if (tab === "all" || tab === "friends") && loading && servers.length === 0}
      <LoadingState />
    {:else if tab === "friends" && scanning && shown.length === 0}
      <LoadingState />
    {:else if measuring > 0 && shown.length === 0}
      <LoadingState />
    {:else if shown.length === 0}
      {#if tab !== "all" || !error}
        <p class="empty">{emptyText}</p>
      {/if}
    {:else}
      <div class="rows">
        <ServerTable
          servers={shown}
          grouped={tab === "all" && grouped}
          prefs={columnsPrefs}
          onprefs={setColumnsPrefs}
          {looks}
          favourites={store.favourites}
          {friends}
          lastCars={tab === "recent" ? lastCars : {}}
          selected={selected ? serverKey(selected) : null}
          onselect={(s) => {
            selected = s;
            linkPassword = null;
          }}
        />
      </div>
    {/if}
  </section>

  {#if selected}
    <ServerDetail
      server={selected}
      {looks}
      preferredCar={lastCars[serverKey(selected)] ?? null}
      presetPassword={linkPassword}
      onclose={() => {
        selected = null;
        linkPassword = null;
      }}
    />
  {/if}
</div>

<style>
  .online {
    display: grid;
    grid-template-columns: 1fr;
    height: 100%;
    min-height: 0;
  }
  .online.with-panel {
    grid-template-columns: 1fr 360px;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .head {
    padding: 22px 24px 0;
  }
  .bar {
    margin-top: 12px;
  }
  .measuring {
    font-size: 11px;
    color: var(--muted);
  }
  .errbox {
    margin: 0 24px 12px;
  }
  .rows {
    flex: 1;
    min-height: 0;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    padding: 50px 0;
  }
</style>
