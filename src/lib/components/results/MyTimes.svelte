<script lang="ts">
  // "My times" on a sheet (RESULTAT§10): the car on each layout it was driven
  // on, or the cars driven on the track, grouped by layout. Hidden when
  // empty, like the layers. A line opens the history on its combo.
  import { t } from "$lib/i18n/index.svelte";
  import { myTimes, type ComboTimes } from "$lib/results/results";
  import { fmtLap } from "$lib/results/returnView";
  import { openHistory } from "$lib/results/historyState.svelte";

  interface Props {
    modId: string;
    isCar: boolean;
    /** Layout id → name, on a track's sheet. */
    layoutNames?: Record<string, string>;
  }
  let { modId, isCar, layoutNames = {} }: Props = $props();

  let rows = $state<ComboTimes[]>([]);

  $effect(() => {
    const id = modId;
    const car = isCar;
    rows = [];
    myTimes(id, car)
      .then((r) => {
        if (id === modId) rows = r;
      })
      .catch((e) => console.warn("my_times", e));
  });

  const layoutName = (id: string | null) => (id ? (layoutNames[id] ?? id) : null);

  /** Car: the latest driven first. Track: by layout, the fastest first. */
  const groups = $derived.by(() => {
    if (isCar) return [{ layout: null as string | null, rows: [...rows].sort((a, b) => b.last_at - a.last_at) }];
    const byLayout = new Map<string, ComboTimes[]>();
    for (const r of rows) byLayout.set(r.track_layout ?? "", [...(byLayout.get(r.track_layout ?? "") ?? []), r]);
    return [...byLayout.entries()].map(([layout, list]) => ({
      layout: byLayout.size > 1 ? layoutName(layout || null) : null,
      rows: list.sort((a, b) => (a.best_lap_ms ?? Infinity) - (b.best_lap_ms ?? Infinity)),
    }));
  });

  function label(r: ComboTimes): string {
    if (!isCar) return r.car_name ?? r.car_id;
    return [r.track_name ?? r.track_id, layoutName(r.track_layout)].filter(Boolean).join(" · ");
  }
  const day = (ms: number) => new Date(ms).toLocaleDateString(undefined, { dateStyle: "medium" });
</script>

{#if rows.length}
  <section class="blk">
    <header class="blk-h">
      <span class="blk-t">{t("results.myTimes")}</span>
      <span class="blk-n">{rows.length}</span>
    </header>
    <div class="blk-b">
      {#each groups as g (g.layout)}
        {#if g.layout}<div class="layout lbl">{g.layout}</div>{/if}
        <ul class="times">
          {#each g.rows as r (`${r.car_id}/${r.track_id}/${r.track_layout ?? ""}`)}
            <li>
              <button
                type="button"
                onclick={() => void openHistory({ car_id: r.car_id, track_id: r.track_id, track_layout: r.track_layout })}
              >
                <span class="what">{label(r)}</span>
                <span class="mono best">{fmtLap(r.best_lap_ms)}</span>
                <span class="when">{day(r.best_at)}</span>
                <span class="n mono">{t("results.sessionsCount", { count: r.sessions })}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    </div>
  </section>
{/if}

<style>
  .times {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .times button {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 10px;
    background: none;
    border: none;
    border-bottom: 1px solid var(--line);
    padding: 5px 0;
    color: var(--txt2);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .times button:hover {
    color: var(--txt);
  }
  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .best {
    color: var(--txt);
  }
  .when,
  .n {
    color: var(--muted);
  }
  .layout {
    margin-top: 6px;
  }
</style>
