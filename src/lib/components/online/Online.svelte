<script lang="ts">
  // The Online page (docs/SPEC-play-online.md): the public servers, judged
  // against what can be driven here, and a panel to pick a car and join.
  //
  // Four tabs over the list (SPEC-play-online.md, case 1): All, and the
  // user's own servers — Favourites, Recent, and those a friend is on. What
  // the list shows is `ServerFilter`'s (search, chips, ping sweep), who
  // drives where `FriendsScan`'s, the head `OnlineToolbar`'s. The page keeps
  // what is the page's: loading the lobby, the open server, the ways in from
  // elsewhere (a pasted link, a track sheet, a notification), and what is
  // remembered between two visits.
  import { onMount, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import { serverDetail, serverKey, type ServerSummary } from "$lib/online/online";
  import { parseJoinLink } from "$lib/online/link";
  import { NO_LOOKS, type Looks } from "$lib/online/looks";
  import { recentCars, type OnlineTab } from "$lib/online/lists";
  import { ONLINE_COLUMNS } from "$lib/online/columns";
  import { defaultPrefs, reconcilePrefs, type ColumnsPrefs } from "$lib/tableColumns";
  import { loadSavedPrefs, saveTablePrefs } from "$lib/tablePrefs.svelte";
  import { loadOnlineStore, onlineStore } from "$lib/online/store.svelte";
  import { loadLobby } from "$lib/online/lobby.svelte";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { pendingOnlineIntent, takeOnlineIntent, type OnlineIntent } from "$lib/online/intent.svelte";
  import { ServerFilter } from "$lib/online/serverFilter.svelte";
  import { FriendsScan } from "$lib/online/friendsScan.svelte";
  import OnlineToolbar from "./OnlineToolbar.svelte";
  import ServerTable from "./ServerTable.svelte";
  import ServerDetail from "./ServerDetail.svelte";

  let servers = $state<ServerSummary[]>([]);
  let looks = $state<Looks>(NO_LOOKS);
  let loading = $state(true);
  let error = $state("");
  /** The lobby did not answer: when the list shown was kept (Unix seconds). */
  let savedAt = $state<number | null>(null);
  let selected = $state<ServerSummary | null>(null);
  /** The password a pasted link carried, for the server it opened. */
  let linkPassword = $state<string | null>(null);
  let tab = $state<OnlineTab>("all");
  /** All tab: servers gathered under one row per track (v2 of the spec). */
  let grouped = $state(false);
  /** The table's columns (SPEC §7.4), kept beside the library's. */
  let columnsPrefs = $state<ColumnsPrefs>(defaultPrefs(ONLINE_COLUMNS));
  /** The stored view is read before the first save, so that restoring it is
   * not taken for a change. A `$state`: the effect that saves must run again
   * once it flips. */
  let restored = $state(false);

  const TABS: OnlineTab[] = ["all", "favourites", "recent", "friends"];

  const store = $derived(onlineStore());
  const lastCars = $derived(recentCars(store));

  const scan = new FriendsScan(() => servers);
  scan.watch();
  const filter = new ServerFilter({
    get servers() {
      return servers;
    },
    get looks() {
      return looks;
    },
    get tab() {
      return tab;
    },
    get store() {
      return store;
    },
    get friends() {
      return scan.friends;
    },
  });
  filter.sweepPings();

  function setColumnsPrefs(next: ColumnsPrefs) {
    columnsPrefs = next;
    void saveTablePrefs("online", next);
  }

  /** The list, from the cache the track sheets share while it is fresh;
   * `force` (the Refresh button) asks the lobby anyway. */
  async function refresh(force = false) {
    loading = true;
    error = "";
    try {
      ({ servers, looks, saved_at: savedAt } = await loadLobby(force));
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
    autoPick = null;
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
    filter.restore(savedFilters);
    tab = TABS.find((x) => x === savedTab) ?? "all";
    grouped = savedGrouped === "1";
    restored = true;
    await refresh();
  });

  $effect(() => {
    const saved = filter.serialized();
    const current = tab;
    const group = grouped;
    if (!restored) return;
    setUiPref(StorageKey.onlineFilters, saved);
    setUiPref(StorageKey.onlineTab, current);
    setUiPref(StorageKey.onlineGrouped, group ? "1" : "0");
  });

  // The panel is always there, so the list does not change width as servers
  // are opened and closed (reported). Until the user picks one, it opens on
  // the last server joined — the one most likely wanted again — else on the
  // first of the list; with nothing at all, it says to pick one. A track
  // sheet's way in opens on the first server of that track instead.
  let autoPick = $state<"last" | "first" | null>("last");
  $effect(() => {
    const mode = autoPick;
    const first = filter.shown[0];
    const last = store.recents[0]?.server;
    const settled = restored && !loading;
    if (!mode || !settled || selected) return;
    const pick =
      mode === "last" && last ? (servers.find((s) => serverKey(s) === serverKey(last)) ?? last) : first;
    if (!pick) return;
    untrack(() => {
      autoPick = null;
      selected = pick;
    });
  });

  // The library changed — an archive dropped after « Prepare & join » sent
  // the user to a web page, an activation, content fetched: the list's levels
  // were judged against the old library and must be judged again (reported:
  // a track installed, the row still said « To download »). The panel reads
  // its own server again on the same signal (`ServerDetail`).
  let seenLibrary = -1;
  $effect(() => {
    const version = libraryVersion();
    const ready = restored;
    if (ready && seenLibrary !== -1 && version !== seenLibrary) untrack(() => void refresh());
    if (ready) seenLibrary = version;
  });

  // A way in from elsewhere (`intent.svelte.ts`): a track sheet's line, a
  // notification. Taken once the stored view is restored, so that the
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
      autoPick = "first";
      filter.showTrack(intent.layouts);
    } else if (intent?.kind === "server") {
      autoPick = null;
      const key = serverKey(intent.server);
      selected = servers.find((s) => serverKey(s) === key) ?? intent.server;
      linkPassword = null;
    }
  }

  const shown = $derived(filter.shown);
  const tabItems = $derived([
    { id: "all", label: t("online.tabAll") },
    { id: "favourites", label: t("online.tabFavourites"), count: store.favourites.length },
    { id: "recent", label: t("online.tabRecent"), count: store.recents.length },
    { id: "friends", label: t("online.tabFriends"), count: Object.keys(scan.friends).length },
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

<div class="online">
  <section class="main">
    <OnlineToolbar
      {filter}
      bind:tab
      tabs={tabItems}
      columns={columnsPrefs}
      oncolumns={setColumnsPrefs}
      bind:grouped
      {loading}
      onrefresh={() => refresh(true)}
    />

    {#if savedAt !== null}
      <!-- The lobby is down: the list kept from last time, said as such. Its
           servers still answer the panel, which asks them directly. -->
      <p class="warnbox">{t("online.lobbyBackup", { date: new Date(savedAt * 1000).toLocaleString() })}</p>
    {/if}
    {#if error}
      <p class="errbox">{error}</p>
    {/if}
    <!-- One's own servers show at once, from their snapshots: they must not
         wait for the lobby, nor vanish when it is down. -->
    {#if (tab === "all" || tab === "friends") && loading && servers.length === 0}
      <LoadingState />
    {:else if tab === "friends" && scan.scanning && shown.length === 0}
      <LoadingState />
    {:else if filter.measuring > 0 && shown.length === 0}
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
          friends={scan.friends}
          lastCars={tab === "recent" ? lastCars : {}}
          selected={selected ? serverKey(selected) : null}
          onselect={(s) => {
            autoPick = null;
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
        autoPick = null;
        selected = null;
        linkPassword = null;
      }}
    />
  {:else}
    <aside class="no-server"><p>{t("online.pickServer")}</p></aside>
  {/if}
</div>

<style>
  .online {
    display: grid;
    grid-template-columns: 1fr 360px;
    height: 100%;
    min-height: 0;
  }
  /* The panel's place, kept while no server is open. */
  .no-server {
    display: flex;
    align-items: center;
    justify-content: center;
    border-left: 1px solid var(--line);
    background: var(--panel2);
    color: var(--muted);
    font-size: 12px;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .warnbox,
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
