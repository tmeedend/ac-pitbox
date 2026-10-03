<script lang="ts">
  // The Online page (docs/SPEC-play-online.md): the public servers, judged
  // against what can be driven here, and a panel to pick a car and join.
  //
  // Four tabs over the list (SPEC-play-online.md, case 1): All, and the
  // user's own servers — Favourites, Recent, and those a friend is on. The
  // four toggles sort out the 9 000 public servers; they do not apply to
  // one's own, which stay listed full, locked or empty: that is exactly when
  // one goes looking for them. The search applies everywhere.
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
  import { listServers, serverDrivers, serverKey, type ServerDrivers, type ServerSummary } from "$lib/online/online";
  import { DEFAULT_FILTERS, filterServers, parseFilters, sortServers, type OnlineFilters } from "$lib/online/filters";
  import { NO_LOOKS, searchText, type Looks } from "$lib/online/looks";
  import { friendsOnline, recentCars, tabServers, type OnlineTab } from "$lib/online/lists";
  import { loadOnlineStore, onlineStore } from "$lib/online/store.svelte";
  import ServerList from "./ServerList.svelte";
  import ServerDetail from "./ServerDetail.svelte";

  let servers = $state<ServerSummary[]>([]);
  let looks = $state<Looks>(NO_LOOKS);
  let loading = $state(true);
  let error = $state("");
  let filters = $state<OnlineFilters>({ ...DEFAULT_FILTERS });
  let selected = $state<ServerSummary | null>(null);
  let tab = $state<OnlineTab>("all");
  /** The stored toggles are read before the first save, so that restoring
   * them is not taken for a change. A `$state`: the effects that save must
   * run again once it flips. */
  let restored = $state(false);

  async function refresh() {
    loading = true;
    error = "";
    try {
      ({ servers, looks } = await listServers());
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
    const [savedFilters, savedTab] = await Promise.all([
      getUiPref(StorageKey.onlineFilters),
      getUiPref(StorageKey.onlineTab),
      loadOnlineStore(),
    ]);
    // The search is a gesture of the moment; only the toggles are remembered.
    filters = { ...parseFilters(savedFilters), search: "" };
    tab = TABS.find((x) => x === savedTab) ?? "all";
    restored = true;
    await refresh();
  });

  $effect(() => {
    const { notFull, noPassword, notEmpty, joinable } = filters;
    const current = tab;
    if (!restored) return;
    setUiPref(StorageKey.onlineFilters, JSON.stringify({ notFull, noPassword, notEmpty, joinable }));
    setUiPref(StorageKey.onlineTab, current);
  });

  const store = $derived(onlineStore());
  const lastCars = $derived(recentCars(store));
  const friends = $derived(friendsOnline(scan, store));
  const shown = $derived.by(() => {
    const text = (s: ServerSummary) => searchText(looks, s);
    if (tab === "all") return sortServers(filterServers(servers, filters, text), new Set(Object.keys(friends)));
    // One's own servers: the search only, never the toggles (see above).
    const searchOnly = { ...DEFAULT_FILTERS, notFull: false, noPassword: false, search: filters.search };
    return filterServers(tabServers(tab, servers, store, friends), searchOnly, text);
  });
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

  const TOGGLES: { key: "notFull" | "noPassword" | "notEmpty" | "joinable"; label: string }[] = [
    { key: "notFull", label: "online.filterNotFull" },
    { key: "noPassword", label: "online.filterNoPassword" },
    { key: "notEmpty", label: "online.filterNotEmpty" },
    { key: "joinable", label: "online.filterJoinable" },
  ];
</script>

<div class="online" class:with-panel={!!selected}>
  <section class="main">
    <header class="head">
      <h2 class="lbl-screen">{t("nav.online")}</h2>
      <Tabs tabs={tabItems} active={tab} onselect={(id) => (tab = id as OnlineTab)} />
      <div class="bar">
        <input class="input search" placeholder={t("online.searchPlaceholder")} bind:value={filters.search} />
        {#if tab === "all"}
          {#each TOGGLES as toggle (toggle.key)}
            <label class="tog">
              <input type="checkbox" bind:checked={filters[toggle.key]} />
              {t(toggle.label)}
            </label>
          {/each}
        {/if}
        <span class="count mono">{t("online.count", { count: shown.length })}</span>
        <button class="btn" type="button" disabled={loading} onclick={refresh}>{t("online.refresh")}</button>
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
    {:else if shown.length === 0}
      {#if tab !== "all" || !error}
        <p class="empty">{emptyText}</p>
      {/if}
    {:else}
      <div class="rows">
        <ServerList
          servers={shown}
          {looks}
          favourites={store.favourites}
          {friends}
          lastCars={tab === "recent" ? lastCars : {}}
          selected={selected ? serverKey(selected) : null}
          onselect={(s) => (selected = s)}
        />
      </div>
    {/if}
  </section>

  {#if selected}
    <ServerDetail
      server={selected}
      {looks}
      preferredCar={lastCars[serverKey(selected)] ?? null}
      onclose={() => (selected = null)}
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
    padding: 22px 24px 14px;
  }
  .bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin-top: 12px;
  }
  .search {
    width: 260px;
  }
  .tog {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--txt2);
    cursor: pointer;
  }
  .tog input {
    accent-color: var(--rosso);
  }
  .count {
    margin-left: auto;
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
