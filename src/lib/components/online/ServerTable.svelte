<script lang="ts">
  // The server table (SPEC-play-online.md, "Liste des serveurs"): one row per
  // server, read at a glance — track, session, players, readiness — in the
  // library's own table (`DataTable`, SPEC §7.4): columns chosen, moved,
  // resized, sorted on a header click. The lobby holds 9 000 servers, so the
  // table is virtual: only the rows in view are rendered.
  //
  // Grouped by track (`groups.ts`), a group row stands for its servers until
  // unfolded: the track, the number of servers, the players summed — the
  // other cells stay empty. Its servers then show under it, the track cell
  // reduced to the layout, already named above.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { StorageKey } from "$lib/storage";
  import { getUiPref, setUiPref } from "$lib/uiPrefs.svelte";
  import DataTable from "$lib/components/ui/DataTable.svelte";
  import { blockerText, durationText } from "$lib/online/labels";
  import { fromSeconds, isOpenPractice } from "$lib/online/sessions";
  import { pingOf, requestPings } from "$lib/online/pings.svelte";
  import { carName, carsOwnedFirst, layoutLook, layoutName, trackName, type Looks } from "$lib/online/looks";
  import { serverKey, type ServerSummary, type TrackRef } from "$lib/online/online";
  import { arrangeRows, type ListRow } from "$lib/online/groups";
  import { nextSort, ONLINE_COLUMNS, parseSort, type OnlineColumn, type Sort } from "$lib/online/columns";
  import { PING_FAIR_MS, PING_GOOD_MS } from "$lib/online/tokens";
  import { previewSrc } from "$lib/library/library";
  import type { ColumnsPrefs } from "$lib/tableColumns";
  import ServerCountry from "./ServerCountry.svelte";
  import LevelTag from "./LevelTag.svelte";

  interface Props {
    /** Filtered, in the default order (friends, joinable, players). */
    servers: ServerSummary[];
    grouped: boolean;
    prefs: ColumnsPrefs;
    onprefs: (next: ColumnsPrefs) => void;
    looks: Looks;
    favourites: ServerSummary[];
    /** Per server key, the friends connected there. */
    friends: Record<string, string[]>;
    /** Per server key, the car last joined with — shown in place of the
     * server's cars on the Recent tab: it is the one that will be picked. */
    lastCars: Record<string, string>;
    selected: string | null;
    onselect: (server: ServerSummary) => void;
  }
  let { servers, grouped, prefs, onprefs, looks, favourites, friends, lastCars, selected, onselect }: Props = $props();

  const ROW_H = 46;
  /** Car names shown before "+n". */
  const CARS_SHOWN = 3;

  let sort = $state<Sort | null>(null);
  /** The groups unfolded, by track key. Not remembered: a fold is a glance. */
  let open = $state<string[]>([]);

  onMount(async () => {
    sort = parseSort(await getUiPref(StorageKey.onlineSort));
  });

  function onsort(key: string) {
    sort = nextSort(sort, key);
    setUiPref(StorageKey.onlineSort, sort ? JSON.stringify(sort) : "");
  }

  const favouriteKeys = $derived(new Set(favourites.map(serverKey)));
  const rows = $derived(
    arrangeRows(servers, { grouped, sort, open: new Set(open) }, { looks, pingOf: (s) => pingOf(serverKey(s)) }),
  );

  const rowKey = (r: ListRow) => (r.kind === "group" ? `group:${r.key}` : serverKey(r.server));

  function onrowclick(r: ListRow) {
    if (r.kind === "server") onselect(r.server);
    else open = open.includes(r.key) ? open.filter((k) => k !== r.key) : [...open, r.key];
  }

  /** The friends on any server of a group, each named once. */
  function groupFriends(list: ServerSummary[]): string[] {
    return [...new Set(list.flatMap((m) => friends[serverKey(m)] ?? []))];
  }

  /** Thresholds of the spec; past them the figure goes quiet rather than
   * red — red is kept for what the session retains (SPEC §7.2ter). */
  function pingClass(ms: number): string {
    return ms < PING_GOOD_MS ? "good" : ms < PING_FAIR_MS ? "fair" : "far";
  }

  /** What is left of the session: the time, or "open" for a practice that
   * runs for hours. */
  function sessionLeft(s: ServerSummary): string {
    if (isOpenPractice(s.session, s.time_left)) return t("online.sessionOpen");
    return durationText(fromSeconds(s.time_left));
  }
</script>

<!-- The track's photo and names: the layout readable under the track, the
     ids on hover. A group's own servers show the layout alone. -->
{#snippet trackCell(track: TrackRef, available: boolean, nested: boolean)}
  {@const look = layoutLook(looks, track)}
  {@const layout = layoutName(looks, track)}
  <span class="trackcell" title={track.kunos_id}>
    <span class="thumb">
      {#if look?.preview && !nested}
        <img src={previewSrc(look.preview)} alt="" loading="lazy" decoding="async" />
      {/if}
    </span>
    <span class="track">
      {#if nested}
        <span class="track-name sub">{layout ?? trackName(looks, track)}</span>
      {:else}
        <span class="track-name" class:missing={!available}>{trackName(looks, track)}</span>
        {#if layout}<span class="layout">{layout}</span>{/if}
      {/if}
    </span>
  </span>
{/snippet}

<!-- One name reads at a glance; past one, the count does. -->
{#snippet friendsBadge(names: string[])}
  <span class="friends" title={names.join(", ")}>★ {names.length === 1 ? names[0] : names.length}</span>
{/snippet}

<DataTable
  columns={ONLINE_COLUMNS}
  {prefs}
  {onprefs}
  {rows}
  {rowKey}
  rowState={(r) =>
    r.kind === "group"
      ? { group: true, sel: r.servers.some((m) => serverKey(m) === selected) }
      : { nested: r.nested, sel: serverKey(r.server) === selected }}
  {sort}
  {onsort}
  onrowclick={(r) => onrowclick(r)}
  rowHeight={ROW_H}
  onvisible={(shown) => requestPings(shown.flatMap((r) => (r.kind === "server" ? [r.server] : [])))}
>
  {#snippet cell(r: ListRow, col: OnlineColumn)}
    {#if r.kind === "group"}
      {#if col.key === "track"}
        {@render trackCell(r.track, r.servers.some((m) => m.track_available), false)}
      {:else if col.key === "server"}
        {@const names = groupFriends(r.servers)}
        <span class="name">
          {#if names.length}{@render friendsBadge(names)}{/if}
          <span class="fold" aria-hidden="true">{r.open ? "▾" : "▸"}</span>
          {t("online.count", { count: r.servers.length })}
        </span>
      {:else if col.key === "players"}
        <span class="players" class:empty={r.clients === 0}>{r.clients} / {r.maxClients}</span>
      {/if}
    {:else}
      {@const s = r.server}
      {@const key = serverKey(s)}
      {#if col.key === "track"}
        {@render trackCell(s.track, s.track_available, r.nested)}
      {:else if col.key === "server"}
        <span class="name" title={s.name}>
          {#if friends[key]}
            {@render friendsBadge(friends[key])}
          {:else if favouriteKeys.has(key)}
            <span class="star" aria-label={t("online.favourite")}>★</span>
          {/if}
          {s.name}
        </span>
      {:else if col.key === "session"}
        {#if s.session}
          <span class="badge" class:race={s.session === "race"}>{t(`online.session.${s.session}`)}</span>
          <span class="left mono">{sessionLeft(s)}</span>
        {/if}
        <!-- A booking server between bookings: said once, here, and not when
             the badge already says it. -->
        {#if s.booking && s.session !== "booking"}
          <span class="badge">{t("online.session.booking")}</span>
        {/if}
      {:else if col.key === "players"}
        <span class="players" class:full={s.clients >= s.max_clients} class:empty={s.clients === 0}>
          {s.clients} / {s.max_clients}
        </span>
      {:else if col.key === "ping"}
        {@const ping = pingOf(key)}
        {#if ping !== undefined}<span class="ping {pingClass(ping)}">{t("online.ping", { ms: ping })}</span>{/if}
      {:else if col.key === "cars"}
        {#if lastCars[key]}
          <span class="cars last">{carName(looks, lastCars[key])}</span>
        {:else}
          {@const cars = carsOwnedFirst(looks, s.cars)}
          <!-- The count sits outside the names, which the cell cuts short:
               it stays in view however narrow the column. -->
          <span class="cars" title={cars.map((c) => carName(looks, c)).join("\n")}>
            <span class="car-names">
              {cars
                .slice(0, CARS_SHOWN)
                .map((c) => carName(looks, c))
                .join(" · ")}
            </span>
            {#if cars.length > CARS_SHOWN}
              <span class="more">{t("online.more", { count: cars.length - CARS_SHOWN })}</span>
            {/if}
          </span>
        {/if}
      {:else if col.key === "csp"}
        {#if s.track.csp_min_build}{s.track.csp_min_build}{/if}
      {:else if col.key === "country"}
        {#if s.country}<ServerCountry code={s.country} />{/if}
      {:else if col.key === "state"}
        <!-- Quiet when all is ready: colour is for what needs something. A
             snapshot saved before levels existed has none — no badge rather
             than a guess. -->
        {#if s.level}
          <span class="state">
            <LevelTag level={s.level} title={(s.blockers ?? []).map(blockerText).join("\n") || undefined} />
          </span>
        {/if}
      {:else if col.key === "address"}
        {s.ip}:{s.http_port}
      {:else if col.key === "password"}
        {#if s.password}
          <svg class="lock" viewBox="0 0 16 16" role="img" aria-label={t("online.password")}>
            <title>{t("online.password")}</title>
            <rect x="3.5" y="7" width="9" height="6.5" rx="1" />
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
          </svg>
        {/if}
      {/if}
    {/if}
  {/snippet}
</DataTable>

<style>
  .trackcell {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  /* The track's photo: what makes a server recognisable before its name is
     read. An empty frame keeps the names aligned when there is none. */
  .thumb {
    flex: none;
    width: 64px;
    height: 34px;
    background: var(--card);
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .track {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .track-name {
    color: var(--txt);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track-name.sub {
    font-weight: 400;
    color: var(--txt2);
  }
  .track-name.missing {
    color: var(--muted);
  }
  .layout {
    color: var(--muted);
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fold {
    display: inline-block;
    width: 12px;
    color: var(--muted);
  }
  .badge {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 6px;
    margin-right: 8px;
    border: 1px solid var(--line);
    color: var(--txt2);
  }
  .badge.race {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .left {
    color: var(--muted);
    font-size: 11px;
  }
  .players {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--txt);
  }
  .players.empty {
    color: var(--muted);
  }
  .players.full {
    color: var(--rosso-bright);
  }
  .cars {
    display: flex;
    min-width: 0;
    color: var(--muted);
    font-size: 11.5px;
  }
  .car-names {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .more {
    flex: none;
    color: var(--faint);
    margin-left: 4px;
  }
  .cars.last {
    color: var(--txt2);
  }
  .ping {
    font-size: 11px;
  }
  .ping.good {
    color: var(--green);
  }
  .ping.fair {
    color: var(--orange);
  }
  .ping.far {
    color: var(--muted);
  }
  .star {
    color: var(--yellow);
    margin-right: 4px;
  }
  .friends {
    color: var(--green);
    font-weight: 600;
    margin-right: 6px;
  }
  .lock {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    color: var(--muted);
  }
  .state {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
</style>
