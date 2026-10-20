<script lang="ts">
  // A race's grid, the author's (SPEC-story-mode, Drivers): the opponents
  // placed one by one - a car, a livery, a driver of the roster, a skill -
  // in the session screen's own table and car picker, so the two grids look
  // and work alike. Only the identity differs: a story's driver is picked in
  // its roster, never typed, and brings their name and nationality with them.
  // The slots `grid` asks for beyond these drive the event's car under filler
  // names, which is what the launcher does (L3).
  import { onMount, untrack } from "svelte";
  import { hasOwnDriver } from "$lib/driver/driverOverride.svelte";
  import { gameCountries, loadFlags, withCountryLabels } from "$lib/flags.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { Opponent } from "$lib/launch/opponent";
  import { buildCardIndex, filterDefs, type FilterMap } from "$lib/library/filters";
  import {
    driverLabel,
    driversTaken,
    duplicateOpponent,
    gridRows,
    patchOpponent,
    withCar,
    withDriver,
    withSkill,
  } from "$lib/story/drivers";
  import { loadSkins, studioLibrary } from "$lib/story/library.svelte";
  import type { StoryEvent, StoryOpponent } from "$lib/story/studio";
  import { studio } from "$lib/story/studioState.svelte";
  import OpponentPicker from "$lib/components/launch/OpponentPicker.svelte";
  import OpponentTable from "$lib/components/launch/OpponentTable.svelte";
  import CountryLabel from "./CountryLabel.svelte";

  interface Props {
    event: StoryEvent;
    onsave: () => void;
  }
  const { event, onsave }: Props = $props();

  const session = $derived(event.session);
  const opponents = $derived(session.opponents ?? []);
  const drivers = $derived(studio.story?.drivers.value?.drivers ?? []);
  const characters = $derived(studio.story?.characters.value?.characters ?? []);
  const rows = $derived(gridRows(session, drivers));
  const cars = $derived((studioLibrary.cards ?? []).filter((c) => c.kind === "Car"));
  const defs = untrack(() => withCountryLabels(filterDefs("Car")));
  const index = $derived(buildCardIndex(cars, defs, true, hasOwnDriver, session.car ?? null));

  // The picker's filters, kept while the event is open: narrowing the
  // library for one opponent is usually what the next one wants too.
  let filters = $state<FilterMap>({});
  let pinned = $state<string[]>([]);
  let query = $state("");
  /** `add`, or the row whose car is being replaced. */
  let picking = $state<"add" | number | null>(null);

  onMount(() => void loadFlags());

  // The liveries of every car on the grid, for the thumbnails and the
  // livery list: read once per car, whenever one appears.
  $effect(() => {
    for (const r of rows) if (r.car_id) void loadSkins(r.car_id);
  });

  function setOpponents(next: StoryOpponent[]) {
    if (next.length) event.session.opponents = next;
    else delete event.session.opponents;
    onsave();
  }

  function update(i: number, change: (o: StoryOpponent) => StoryOpponent) {
    setOpponents(opponents.map((o, k) => (k === i ? change(o) : o)));
  }

  function add(carIds: string[]) {
    picking = null;
    setOpponents([...opponents, ...carIds.map((car) => ({ car }))]);
  }

  function replace(carId: string) {
    const i = picking;
    picking = null;
    if (typeof i === "number") update(i, (o) => withCar(o, carId));
  }

  /** The livery's country, for a row nobody named: what the game shows. */
  function liveryCountry(row: Opponent): string | null {
    return studioLibrary.skins[row.car_id]?.find((s) => s.id === row.car_skin)?.country ?? null;
  }
</script>

{#snippet identity(row: Opponent, i: number)}
  {@const o = opponents[i]}
  {@const taken = driversTaken(opponents, i)}
  <select
    class="who input"
    value={o?.driver ?? ""}
    aria-label={t("launch.colDriver")}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    onchange={(e) => update(i, (x) => withDriver(x, (e.currentTarget as HTMLSelectElement).value || null))}
  >
    <option value="">{t("launch.autoCell")}</option>
    {#if o?.driver && !drivers.some((d) => d.id === o.driver)}
      <option value={o.driver}>{o.driver}</option>
    {/if}
    {#each drivers as d (d.id)}
      <option value={d.id} disabled={taken.has(d.id)}>{driverLabel(d, characters)}</option>
    {/each}
  </select>
  <span class="where">
    <CountryLabel name={row.nationality ?? liveryCountry(row)} fallback={t("launch.autoCell")} />
  </span>
{/snippet}

<div class="grid">
  {#if rows.length}
    <div class="table">
      <div class="inner">
        <OpponentTable
          opponents={rows}
          carPool={cars}
          skinsByCarId={studioLibrary.skins}
          {index}
          nationalityList={gameCountries()}
          {identity}
          onremove={(i) => setOpponents(opponents.filter((_, k) => k !== i))}
          onduplicate={(i) =>
            setOpponents([...opponents.slice(0, i + 1), duplicateOpponent(opponents[i]), ...opponents.slice(i + 1)])}
          onsetlevel={(i, level) => update(i, (o) => withSkill(o, level))}
          onsetcell={(i, patch) => update(i, (o) => patchOpponent(o, patch))}
          onopenpicker={(i) => (picking = i)}
        />
      </div>
    </div>
  {/if}
  <button class="add" type="button" disabled={studioLibrary.cards === null} onclick={() => (picking = "add")}
    >+ {t("story.events.addOpponents")}</button
  >
</div>

{#if picking !== null}
  <OpponentPicker
    pool={cars}
    mode={picking === "add" ? "add" : "replace"}
    bind:filters
    bind:pinned
    bind:query
    perfRefId={session.car ?? null}
    gridCount={opponents.length}
    gridTarget={Math.max(opponents.length, (session.grid ?? 0) - 1)}
    slotNumber={typeof picking === "number" ? picking + 1 : 1}
    currentCarId={typeof picking === "number" ? (rows[picking]?.car_id ?? null) : null}
    onadd={add}
    onreplace={replace}
    onclose={() => (picking = null)}
  />
{/if}

<style>
  .grid {
    border: 1px solid var(--line);
  }
  /* The session screen's columns are fixed and the car's name takes what is
     left: below this width the name would be squeezed to nothing, so the
     table scrolls instead. */
  .table {
    overflow-x: auto;
  }
  .inner {
    min-width: 1040px;
  }
  .who {
    width: 156px;
    flex: none;
    font-size: 12px;
    padding: 3px 4px;
  }
  .where {
    width: 156px;
    flex: none;
    display: flex;
    padding: 0 4px;
  }
  /* Secondary, like the session screen's "Choose from the pool": the red of
     this screen is not for adding a row. */
  .add {
    background: var(--panel2);
    padding: 9px 12px;
    border-top: 1px solid var(--line);
    color: var(--txt2);
    font-size: 11.5px;
    text-align: left;
    width: 100%;
  }
  .grid > .add:first-child {
    border-top: 0;
  }
  .add:hover:not(:disabled) {
    background: var(--raised);
  }
</style>
