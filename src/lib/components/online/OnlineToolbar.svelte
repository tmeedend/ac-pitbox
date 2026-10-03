<script lang="ts">
  // The head of the Online page: its title, the tabs over the list
  // (SPEC-play-online.md, case 1: All, then the user's own servers), and the
  // bar — search, chips, and at its end what is about showing rather than
  // filtering: the ping sweep under way, the columns, the display menu,
  // Refresh.
  import { t } from "$lib/i18n/index.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import FilterBar from "$lib/components/filters/FilterBar.svelte";
  import ColumnsMenu from "$lib/components/ui/ColumnsMenu.svelte";
  import DisplayMenu from "$lib/components/ui/DisplayMenu.svelte";
  import { ONLINE_COLUMNS } from "$lib/online/columns";
  import type { OnlineTab } from "$lib/online/lists";
  import type { ServerFilter } from "$lib/online/serverFilter.svelte";
  import { toggleVisible, type ColumnsPrefs } from "$lib/tableColumns";

  interface Props {
    filter: ServerFilter;
    tab: OnlineTab;
    tabs: { id: string; label: string; count?: number }[];
    columns: ColumnsPrefs;
    oncolumns: (next: ColumnsPrefs) => void;
    /** All tab: servers gathered under one row per track. */
    grouped: boolean;
    loading: boolean;
    onrefresh: () => void;
  }
  let {
    filter,
    tab = $bindable(),
    tabs,
    columns,
    oncolumns,
    grouped = $bindable(),
    loading,
    onrefresh,
  }: Props = $props();
</script>

<header class="head">
  <h2 class="lbl-screen">{t("nav.online")}</h2>
  <Tabs {tabs} active={tab} onselect={(id) => (tab = id as OnlineTab)} />
  <div class="bar">
    <FilterBar
      defs={filter.defs}
      bind:filters={() => filter.barTokens, (next) => (filter.barTokens = next)}
      bind:pinned={filter.pinned}
      bind:query={filter.search}
      optionsFor={(key) => filter.optionsFor(key)}
      presets={[]}
      resultCount={filter.shown.length}
      countKey="online.count"
      placeholderKey="online.searchPlaceholder"
    >
      {#snippet end()}
        {#if filter.measuring > 0}
          <span class="measuring mono">{t("online.pingMeasuring", { count: filter.measuring })}</span>
        {/if}
        <ColumnsMenu
          items={ONLINE_COLUMNS.map((c) => ({ key: c.key, label: t(c.labelKey), fixed: c.fixed }))}
          visible={columns.visible}
          ontoggle={(key) => oncolumns(toggleVisible(columns, key))}
        />
        <!-- Grouping is a way of showing, not a filter: it lives with the
             display, and only for the public servers — one's own lists are
             short. -->
        <DisplayMenu title={t("online.displayMenu")}>
          <label>
            <input type="checkbox" bind:checked={grouped} disabled={tab !== "all"} />
            <span>{t("online.groupByTrack")}</span>
          </label>
        </DisplayMenu>
        <button class="btn" type="button" disabled={loading} onclick={onrefresh}>{t("online.refresh")}</button>
      {/snippet}
    </FilterBar>
  </div>
</header>

<style>
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
</style>
