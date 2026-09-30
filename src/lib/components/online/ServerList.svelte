<script lang="ts">
  // The server rows (SPEC-play-online.md, "Liste des serveurs"): one wide line
  // per server, read at a glance — track, session, players, readiness. The
  // lobby holds some 9 000 servers, so only the visible lines are rendered,
  // like the game folder's tree.
  import { t } from "$lib/i18n/index.svelte";
  import { isJoinable } from "$lib/online/filters";
  import { serverKey, type ServerSummary } from "$lib/online/online";

  interface Props {
    servers: ServerSummary[];
    selected: string | null;
    onselect: (server: ServerSummary) => void;
  }
  let { servers, selected, onselect }: Props = $props();

  const ROW_H = 46;
  const OVERSCAN = 8;
  /** Car ids shown on a line before "+n". */
  const CARS_SHOWN = 3;

  let scrollTop = $state(0);
  let viewport = $state(600);

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  const visible = $derived(servers.slice(first, first + Math.ceil(viewport / ROW_H) + 2 * OVERSCAN));

  function timeLeft(seconds: number): string {
    const minutes = Math.round(seconds / 60);
    return minutes >= 120 ? t("online.hours", { n: Math.round(minutes / 60) }) : t("online.minutes", { n: minutes });
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
        {@const ready = isJoinable(s)}
        <button
          type="button"
          class="row"
          class:sel={key === selected}
          role="option"
          aria-selected={key === selected}
          onclick={() => onselect(s)}
        >
          <span class="track">
            <span class="track-id mono" class:missing={!s.track_available}>{s.track.id}</span>
            {#if s.track.layout}<span class="layout mono">{s.track.layout}</span>{/if}
          </span>
          <span class="name" title={s.name}>{s.name}</span>
          <span class="session">
            {#if s.session}
              <span class="badge" class:race={s.session === "race"}>{t(`online.session.${s.session}`)}</span>
              <span class="left mono">{timeLeft(s.time_left)}</span>
            {/if}
          </span>
          <span class="players mono" class:full={s.clients >= s.max_clients} class:empty={s.clients === 0}>
            {s.clients} / {s.max_clients}
          </span>
          <span class="cars mono">
            {s.cars.slice(0, CARS_SHOWN).join(" · ")}
            {#if s.cars.length > CARS_SHOWN}<span class="more">{t("online.more", { count: s.cars.length - CARS_SHOWN })}</span>{/if}
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
            {#if s.country}<span class="flag mono">{s.country}</span>{/if}
          </span>
          <span class="state" class:ready>{ready ? t("online.ready") : t("online.missing")}</span>
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
    grid-template-columns: minmax(150px, 1.1fr) minmax(160px, 1.6fr) 150px 70px minmax(120px, 1fr) auto 110px;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 46px;
    padding: 0 16px;
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
  .track-id {
    color: var(--txt);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track-id.missing {
    color: var(--muted);
  }
  .layout {
    color: var(--muted);
    font-size: 10.5px;
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
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    color: var(--faint);
    margin-left: 4px;
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
  .state.ready {
    color: var(--green);
  }
</style>
