<script lang="ts">
  // The Online page (docs/SPEC-play-online.md): the public servers, judged
  // against what can be driven here, and a panel to pick a car and join.
  //
  // Three tabs over the list (SPEC-play-online.md, case 1): All, and the
  // user's own servers — Favourites and Recent. The four toggles sort out the
  // 9 000 public servers; they do not apply to one's own, which stay listed
  // full, locked or empty: that is exactly when one goes looking for them.
  // The search applies everywhere. Friends and the finer readiness levels
  // come in the next lots.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import { listServers, serverKey, type ServerSummary } from "$lib/online/online";
  import { DEFAULT_FILTERS, filterServers, parseFilters, sortServers, type OnlineFilters } from "$lib/online/filters";
  import { NO_LOOKS, searchText, type Looks } from "$lib/online/looks";
  import { recentCars, tabServers, type OnlineTab } from "$lib/online/lists";
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

  const TABS: OnlineTab[] = ["all", "favourites", "recent"];
  /** Written out rather than built from the tab, so the locale check sees them. */
  const EMPTY: Record<OnlineTab, string> = {
    all: "online.empty",
    favourites: "online.emptyFavourites",
    recent: "online.emptyRecent",
  };

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
  const shown = $derived.by(() => {
    const text = (s: ServerSummary) => searchText(looks, s);
    if (tab === "all") return sortServers(filterServers(servers, filters, text));
    // One's own servers: the search only, never the toggles (see above).
    const searchOnly = { ...DEFAULT_FILTERS, notFull: false, noPassword: false, search: filters.search };
    return filterServers(tabServers(tab, servers, store), searchOnly, text);
  });
  const tabItems = $derived([
    { id: "all", label: t("online.tabAll") },
    { id: "favourites", label: t("online.tabFavourites"), count: store.favourites.length },
    { id: "recent", label: t("online.tabRecent"), count: store.recents.length },
  ]);

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
    {#if tab === "all" && loading && servers.length === 0}
      <LoadingState />
    {:else if shown.length === 0}
      {#if tab !== "all" || !error}
        <p class="empty">{t(EMPTY[tab])}</p>
      {/if}
    {:else}
      <div class="rows">
        <ServerList
          servers={shown}
          {looks}
          favourites={store.favourites}
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
