<script lang="ts">
  // The sessions driven, grouped into outings (RESULTAT§7). In the session
  // zone, the column beside it. A session opens on the screen of a session
  // (`SessionReturn`), and going back returns here: the open outing, the
  // filter and the switch live in `historyState`.
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { getModDetail, previewSrc, type ModDetail } from "$lib/library/library";
  import { getThumbnail } from "$lib/detail/thumbnails";
  import { carLines, outingsOf, visibleOutings, type Outing } from "$lib/results/outings";
  import { fmtGap, fmtLap } from "$lib/results/returnView";
  import { outingPicture, sessionHistory, type HistoryLine } from "$lib/results/results";
  import { closeOuting, historyState, openSession } from "$lib/results/historyState.svelte";

  let lines = $state<HistoryLine[] | null>(null);
  let error = $state("");
  /** Tracks of the history, for their picture and their layouts' names. */
  let tracks = $state<Record<string, ModDetail | null>>({});
  let pictures = $state<Record<string, string | null>>({});

  $effect(() => {
    const combo = historyState.combo;
    untrack(() => {
      lines = null;
      error = "";
      sessionHistory(combo)
        .then((l) => (lines = l))
        .catch((e) => (error = errorText(e)));
    });
  });

  const outings = $derived(lines ? visibleOutings(outingsOf(lines), historyState.showEmpty) : []);
  const open = $derived(outings.find((o) => o.key === historyState.outing) ?? null);

  // Each track once: its picture is the fallback of an outing's, and its
  // layouts carry their names.
  $effect(() => {
    const ids = [...new Set(outings.map((o) => o.track_id).filter((id): id is string => !!id))];
    for (const id of ids) {
      if (id in untrack(() => tracks)) continue;
      tracks = { ...untrack(() => tracks), [id]: null };
      getModDetail(id)
        .then((d) => (tracks = { ...tracks, [id]: d }))
        .catch(() => {});
    }
  });

  // An outing's picture: a screenshot of one of its sessions, asked once.
  $effect(() => {
    for (const o of outings) {
      if (o.key in untrack(() => pictures) || !o.track_id) continue;
      pictures = { ...untrack(() => pictures), [o.key]: null };
      const cars = [...new Set(o.sessions.map((s) => s.car_id).filter((c): c is string => !!c))];
      outingPicture(cars, o.track_id, o.started_at, o.ended_at)
        .then((path) => (path ? getThumbnail(path) : null))
        .then((src) => {
          if (src) pictures = { ...pictures, [o.key]: src };
        })
        .catch(() => {});
    }
  });

  function trackLabel(o: { track_id: string | null; track_name: string | null; track_layout: string | null }): string {
    const detail = o.track_id ? tracks[o.track_id] : null;
    const layout = o.track_layout
      ? (detail?.track?.layouts.find((l) => l.id === o.track_layout)?.name ?? o.track_layout)
      : null;
    return [o.track_name ?? o.track_id, layout].filter(Boolean).join(" · ");
  }

  function picture(o: Outing): string | null {
    return pictures[o.key] ?? (o.track_id ? previewSrc(tracks[o.track_id]?.preview ?? null) : null);
  }

  function carsOf(o: Outing): string {
    return [...new Set(o.sessions.map((s) => s.car_name ?? s.car_id))].join(", ");
  }

  const day = (ms: number) => new Date(ms).toLocaleDateString(undefined, { dateStyle: "medium" });
  const time = (ms: number | null) => (ms ? new Date(ms).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" }) : "");

  /** An outing with one car opens on its sessions, or on its only session. */
  function openOuting(o: Outing) {
    if (o.sessions.length === 1 && o.sessions[0].session_id != null) {
      void openSession(o.sessions[0].session_id);
      return;
    }
    historyState.outing = o.key;
  }

  const filterLabel = $derived.by(() => {
    const c = historyState.combo;
    if (!c) return "";
    const any = lines?.[0];
    return [any?.car_name ?? c.car_id, trackLabel({ track_id: c.track_id, track_name: any?.track_name ?? null, track_layout: c.track_layout })]
      .filter(Boolean)
      .join(" · ");
  });
</script>

<div class="history">
  <header class="head">
    <h1 class="lbl-screen">
      {#if open}
        <button class="crumb" type="button" onclick={closeOuting}>{t("results.history")}</button>
        <span class="sep">·</span>{trackLabel(open)}
      {:else}
        {t("results.history")}
      {/if}
    </h1>
    <label class="switch">
      <input type="checkbox" bind:checked={historyState.showEmpty} />
      <span>{t("results.showEmpty")}</span>
    </label>
  </header>

  {#if historyState.combo}
    <div class="filter">
      <span class="pill">
        {filterLabel}
        <button type="button" class="x" title={t("results.clearFilter")} onclick={() => (historyState.combo = null)}>✕</button>
      </span>
    </div>
  {/if}

  {#if error}<p class="errbox">{error}</p>{/if}

  {#if lines === null}
    <p class="quiet">{t("common.loading")}</p>
  {:else if open}
    {@const cars = carLines(open)}
    {#if cars.length >= 2}
      <table class="board">
        <thead>
          <tr>
            <th>{t("results.car")}</th>
            <th class="num">{t("results.bestLap")}</th>
            <th class="num">{t("results.gap")}</th>
            <th class="num">{t("results.validLaps")}</th>
            <th class="num">{t("results.sessions")}</th>
          </tr>
        </thead>
        <tbody>
          {#each cars as c (c.car_id)}
            <tr>
              <td>{c.car_name ?? c.car_id}</td>
              <td class="mono num">{fmtLap(c.best_lap_ms)}</td>
              <td class="mono num gap">{c.gap_ms != null ? fmtGap(c.gap_ms) : ""}</td>
              <td class="mono num">{c.valid_laps}</td>
              <td class="mono num">{c.sessions}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    <ul class="sessions">
      {#each open.sessions as s (s.session_id)}
        <li>
          <button class="row" type="button" onclick={() => s.session_id != null && void openSession(s.session_id)}>
            <span class="when mono">{time(s.ended_at)}</span>
            <span class="mode">{t(`results.mode.${s.mode}`)}</span>
            <span class="car">{s.car_name ?? s.car_id}</span>
            <span class="mono best">
              {#if s.race_position != null}{t("results.position", { n: s.race_position })} · {/if}{fmtLap(s.best_lap_ms)}
            </span>
            <span class="mono laps">{t("results.validLapsCount", { count: s.valid_laps })}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else if outings.length === 0}
    <p class="quiet">{t("results.historyEmpty")}</p>
  {:else}
    <ul class="outings">
      {#each outings as o (o.key)}
        {@const img = picture(o)}
        <li>
          <button class="outing" type="button" onclick={() => openOuting(o)}>
            <span class="pic" style:background-image={img ? `url('${img}')` : undefined}></span>
            <span class="body">
              <span class="track">{trackLabel(o)}</span>
              <span class="meta">
                {day(o.ended_at)} · {t("results.sessionsCount", { count: o.sessions.length })} · {carsOf(o)}
              </span>
            </span>
            <span class="mono best">{fmtLap(o.best_lap_ms)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .history {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 900px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .crumb {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--muted);
    cursor: pointer;
  }
  .crumb:hover {
    color: var(--txt);
  }
  .sep {
    color: var(--faint);
    margin: 0 8px;
  }
  .switch {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 12px;
    color: var(--muted);
    cursor: pointer;
  }
  .filter .x {
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    padding: 0 0 0 6px;
  }
  .quiet {
    color: var(--muted);
  }
  .outings,
  .sessions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .outing,
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    text-align: left;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt);
    padding: 8px 12px;
    cursor: pointer;
  }
  .outing:hover,
  .row:hover {
    border-color: var(--rosso-border);
  }
  .pic {
    flex: none;
    width: 120px;
    aspect-ratio: 16 / 9;
    background: var(--panel) center / cover no-repeat;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .track {
    font-weight: 600;
  }
  .meta {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .best {
    flex: none;
  }
  .row .when {
    color: var(--muted);
    flex: none;
    width: 9em;
  }
  .row .mode {
    flex: none;
    width: 8em;
  }
  .row .car {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row .laps {
    color: var(--muted);
    flex: none;
  }
  .board {
    border-collapse: collapse;
    width: 100%;
  }
  .board th {
    text-align: left;
    font-size: 11px;
    color: var(--muted);
    font-weight: normal;
    padding: 4px 10px;
    border-bottom: 1px solid var(--line);
  }
  .board td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .num {
    text-align: right;
  }
  .gap {
    color: var(--muted);
  }
</style>
