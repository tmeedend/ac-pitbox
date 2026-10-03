<script lang="ts">
  // The server rows (SPEC-play-online.md, "Liste des serveurs"): one wide line
  // per server, read at a glance — track, session, players, readiness. The
  // lobby holds some 9 000 servers, so only the visible lines are rendered,
  // like the game folder's tree.
  import { t } from "$lib/i18n/index.svelte";
  import { blockerText, durationText, levelText } from "$lib/online/labels";
  import { fromSeconds } from "$lib/online/sessions";
  import { pingOf, requestPings } from "$lib/online/pings.svelte";
  import { carName, carsOwnedFirst, layoutLook, trackTitle, type Looks } from "$lib/online/looks";
  import { serverKey, type ServerSummary } from "$lib/online/online";
  import { PING_FAIR_MS, PING_GOOD_MS } from "$lib/online/tokens";
  import { previewSrc } from "$lib/library/library";
  import ServerCountry from "./ServerCountry.svelte";

  interface Props {
    servers: ServerSummary[];
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
  let { servers, looks, favourites, friends, lastCars, selected, onselect }: Props = $props();

  const favouriteKeys = $derived(new Set(favourites.map(serverKey)));

  const ROW_H = 46;
  const OVERSCAN = 8;
  /** Car ids shown on a line before "+n". */
  const CARS_SHOWN = 3;

  let scrollTop = $state(0);
  let viewport = $state(600);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  const visible = $derived(servers.slice(first, first + Math.ceil(viewport / ROW_H) + 2 * OVERSCAN));

  // Pings for the rows on screen, once the scroll has settled: measuring
  // every row flown past would ask hundreds of servers for nothing.
  $effect(() => {
    const rows = visible;
    const timer = setTimeout(() => requestPings(rows), 400);
    return () => clearTimeout(timer);
  });

  /** Thresholds of the spec; past them the figure goes quiet rather than
   * red — red is kept for what the session retains (SPEC §7.2ter). */
  function pingClass(ms: number): string {
    return ms < PING_GOOD_MS ? "good" : ms < PING_FAIR_MS ? "fair" : "far";
  }
</script>

<div
  class="list"
  role="listbox"
  aria-label={t("nav.online")}
  bind:clientHeight={viewport}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
>
  <div class="spacer" style="height: {servers.length * ROW_H}px">
    <div class="slice" style="transform: translateY({first * ROW_H}px)">
      {#each visible as s (serverKey(s))}
        {@const key = serverKey(s)}
        {@const look = layoutLook(looks, s.track)}
        {@const cars = carsOwnedFirst(looks, s.cars)}
        {@const ping = pingOf(key)}
        <button
          type="button"
          class="row"
          class:sel={key === selected}
          role="option"
          aria-selected={key === selected}
          onclick={() => onselect(s)}
        >
          <span class="thumb">
            {#if look?.preview}
              <img src={previewSrc(look.preview)} alt="" loading="lazy" decoding="async" />
            {/if}
          </span>
          <span class="track">
            <span class="track-name" class:missing={!s.track_available} title={trackTitle(looks, s.track)}>
              {trackTitle(looks, s.track)}
            </span>
            <span class="layout mono">{s.track.layout ? `${s.track.id} · ${s.track.layout}` : s.track.id}</span>
          </span>
          <span class="name" title={s.name}>
            {#if friends[key]}
              <!-- One name reads at a glance; past one, the count does. -->
              <span class="friends" title={friends[key].join(", ")}>
                ★ {friends[key].length === 1 ? friends[key][0] : friends[key].length}
              </span>
            {:else if favouriteKeys.has(key)}
              <span class="star" aria-label={t("online.favourite")}>★</span>
            {/if}
            {s.name}
          </span>
          <span class="session">
            {#if s.session}
              <span class="badge" class:race={s.session === "race"}>{t(`online.session.${s.session}`)}</span>
              <span class="left mono">{durationText(fromSeconds(s.time_left))}</span>
            {/if}
          </span>
          <span class="players mono" class:full={s.clients >= s.max_clients} class:empty={s.clients === 0}>
            {s.clients} / {s.max_clients}
          </span>
          <span class="ping mono {ping === undefined ? '' : pingClass(ping)}">
            {ping === undefined ? "" : t("online.ping", { ms: ping })}
          </span>
          <span class="cars" class:last={!!lastCars[key]}>
            {#if lastCars[key]}
              {carName(looks, lastCars[key])}
            {:else}
              {cars
                .slice(0, CARS_SHOWN)
                .map((c) => carName(looks, c))
                .join(" · ")}
              {#if cars.length > CARS_SHOWN}
                <span class="more">{t("online.more", { count: cars.length - CARS_SHOWN })}</span>
              {/if}
            {/if}
          </span>
          <span class="flags">
            {#if s.password}
              <svg class="lock" viewBox="0 0 16 16" role="img" aria-label={t("online.password")}>
                <title>{t("online.password")}</title>
                <rect x="3.5" y="7" width="9" height="6.5" rx="1" />
                <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
              </svg>
            {/if}
            {#if s.booking}<span class="flag mono">{t("online.session.booking")}</span>{/if}
            {#if s.track.csp_min_build}<span class="flag mono">{t("online.csp", { build: s.track.csp_min_build })}</span>{/if}
            {#if s.country}<ServerCountry code={s.country} />{/if}
          </span>
          <!-- A snapshot saved before levels existed has none: no badge rather
               than a guess, until the lobby lists the server again. -->
          <span class="state {s.level ?? ''}" title={(s.blockers ?? []).map(blockerText).join("\n") || undefined}>
            {s.level ? levelText(s.level) : ""}
          </span>
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .list {
    height: 100%;
    overflow-y: auto;
    border-top: 1px solid var(--line);
  }
  .spacer {
    position: relative;
  }
  .row {
    display: grid;
    grid-template-columns: 72px minmax(150px, 1.1fr) minmax(160px, 1.6fr) 150px 70px 54px minmax(120px, 1fr) auto 110px;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 46px;
    padding: 0 16px 0 6px;
    background: none;
    border: none;
    border-bottom: 1px solid var(--line);
    color: var(--txt2);
    text-align: left;
    font-size: 12px;
  }
  .row:hover {
    background: var(--raised);
  }
  .row.sel {
    background: var(--rosso-dim);
    box-shadow: inset 2px 0 0 var(--rosso);
  }
  .track {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  /* The track's photo: what makes a server recognisable before its name is
     read. An empty frame keeps the columns aligned when there is none. */
  .thumb {
    width: 72px;
    height: 38px;
    background: var(--card);
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .track-name {
    color: var(--txt);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track-name.missing {
    color: var(--muted);
  }
  .layout {
    color: var(--muted);
    font-size: 10.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .badge {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 6px;
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
    font-size: 13px;
    color: var(--txt);
    text-align: right;
  }
  .players.empty {
    color: var(--muted);
  }
  .players.full {
    color: var(--rosso-bright);
  }
  .cars {
    color: var(--muted);
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    color: var(--faint);
    margin-left: 4px;
  }
  .ping {
    font-size: 11px;
    text-align: right;
    color: var(--faint);
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
  .cars.last {
    color: var(--txt2);
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
  .flags {
    display: flex;
    gap: 6px;
    color: var(--muted);
    font-size: 10.5px;
  }
  .lock {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
  }
  .state {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    text-align: right;
  }
  /* Green ready, blue one click, orange something to fetch; blocked stays
     muted — red is kept for what the session retains (SPEC §7.2ter). */
  .state.ready {
    color: var(--green);
  }
  .state.oneClick {
    color: var(--blue);
  }
  .state.download {
    color: var(--orange);
  }
</style>
