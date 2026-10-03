<script lang="ts">
  // The Online page (docs/SPEC-play-online.md): the public servers, judged
  // against what can be driven here, and a panel to pick a car and join.
  //
  // Lot 1: the list, four toggles and a search, the detail panel and the
  // join. Favourites, recents, friends, previews and the finer readiness
  // levels come in the next lots (SPEC-play-online.md).
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import { listServers, serverKey, type ServerSummary } from "$lib/online/online";
  import { DEFAULT_FILTERS, filterServers, parseFilters, sortServers, type OnlineFilters } from "$lib/online/filters";
  import { NO_LOOKS, searchText, type Looks } from "$lib/online/looks";
  import ServerList from "./ServerList.svelte";
  import ServerDetail from "./ServerDetail.svelte";

  let servers = $state<ServerSummary[]>([]);
  let looks = $state<Looks>(NO_LOOKS);
  let loading = $state(true);
  let error = $state("");
  let filters = $state<OnlineFilters>({ ...DEFAULT_FILTERS });
  let selected = $state<ServerSummary | null>(null);
  /** The stored toggles are read before the first save, so that restoring
   * them is not taken for a change. */
  let restored = false;

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

  onMount(async () => {
    const saved = parseFilters(await getUiPref(StorageKey.onlineFilters));
    // The search is a gesture of the moment; only the toggles are remembered.
    filters = { ...saved, search: "" };
    restored = true;
    await refresh();
  });

  $effect(() => {
    const { notFull, noPassword, notEmpty, joinable } = filters;
    if (!restored) return;
    setUiPref(StorageKey.onlineFilters, JSON.stringify({ notFull, noPassword, notEmpty, joinable }));
  });

  const shown = $derived(sortServers(filterServers(servers, filters, (s) => searchText(looks, s))));

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
      <div class="bar">
        <input class="input search" placeholder={t("online.searchPlaceholder")} bind:value={filters.search} />
        {#each TOGGLES as toggle (toggle.key)}
          <label class="tog">
            <input type="checkbox" bind:checked={filters[toggle.key]} />
            {t(toggle.label)}
          </label>
        {/each}
        <span class="count mono">{t("online.count", { count: shown.length })}</span>
        <button class="btn" type="button" disabled={loading} onclick={refresh}>{t("online.refresh")}</button>
      </div>
    </header>

    {#if error}
      <p class="errbox">{error}</p>
    {/if}
    {#if loading && servers.length === 0}
      <LoadingState />
    {:else if shown.length === 0 && !error}
      <p class="empty">{t("online.empty")}</p>
    {:else}
      <div class="rows">
        <ServerList
          servers={shown}
          {looks}
          selected={selected ? serverKey(selected) : null}
          onselect={(s) => (selected = s)}
        />
      </div>
    {/if}
  </section>

  {#if selected}
    <ServerDetail server={selected} {looks} onclose={() => (selected = null)} />
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
