<script lang="ts">
  // What an event launches (SPEC-story-mode, Events, retries and failure):
  // its type, the track and car picked in the player's library, and the few
  // settings of the session the author decides. A setting left empty is Pit
  // Box's default; what is not here at all (assists, damage, tyre wear)
  // stays the player's. Only a race has laps, a grid, opponents and a start.
  import { t } from "$lib/i18n/index.svelte";
  import { WEATHER_IDS, WEATHER_LABEL_KEYS } from "$lib/launch/weatherIntents";
  import type { ModCard, ModKind } from "$lib/library/library";
  import { EVENT_TYPES, sessionFor } from "$lib/story/events";
  import { cardOf, loadLayouts, studioLibrary } from "$lib/story/library.svelte";
  import type { EventType, GridStart, Session, StoryEvent } from "$lib/story/studio";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Seg from "$lib/components/ui/Seg.svelte";
  import LibraryPicker from "./LibraryPicker.svelte";

  interface Props {
    event: StoryEvent;
    /** Whether the story has an economy: the entry fee exists only then. */
    money: boolean;
    onsave: () => void;
  }
  const { event, money, onsave }: Props = $props();

  const session = $derived(event.session);
  const track = $derived(cardOf("Track", session.track));
  const car = $derived(cardOf("Car", session.car));
  const layouts = $derived(session.track ? (studioLibrary.layouts[session.track] ?? []) : []);
  const isRace = $derived(event.type === "race");
  const libraryRead = $derived(studioLibrary.cards !== null);

  let picking = $state<ModKind | null>(null);

  $effect(() => {
    if (session.track) void loadLayouts(session.track);
  });

  /** Sets or removes one key of the session, then saves. */
  function set<K extends keyof Session>(key: K, v: Session[K] | undefined) {
    if (v === undefined || v === "") delete event.session[key];
    else event.session[key] = v;
    onsave();
  }

  async function pick(card: ModCard) {
    const kind = picking;
    picking = null;
    if (kind === "Car") return set("car", card.id_interne);
    event.session.track = card.id_interne;
    delete event.session.layout;
    // A track with layouts is launched on one of them: the first until the
    // author picks another.
    const found = await loadLayouts(card.id_interne);
    if (found.length > 0 && event.session.track === card.id_interne) event.session.layout = found[0].id;
    onsave();
  }

  function setType(type: string) {
    event.type = type as EventType;
    event.session = sessionFor(event.type, event.session);
    onsave();
  }

  /** A number field whose empty value removes the key. */
  function number(key: "laps" | "grid" | "ai_level", empty: number) {
    return (v: number) => set(key, v === empty ? undefined : v);
  }
</script>

<Field label={t("story.events.type")}>
  <Seg
    value={event.type}
    onselect={setType}
    items={EVENT_TYPES.map((type) => ({ value: type, label: t(`story.events.type_${type}`) }))}
  />
</Field>

{#snippet picked(kind: ModKind, id: string | undefined, card: ModCard | undefined)}
  <div class="picked">
    {#if id}
      <span class="what">
        <span class="name">{card?.display_name ?? id}</span>
        <span class="id mono">{id}</span>
      </span>
      {#if libraryRead && !card}
        <span class="pill pill-warn" title={t("story.events.notInLibraryHint")}>{t("story.events.notInLibrary")}</span>
      {/if}
    {:else}
      <span class="none">{t("story.events.nothingPicked")}</span>
    {/if}
    <button class="btn" onclick={() => (picking = kind)}>{t("story.events.choose")}</button>
  </div>
{/snippet}

<Field label={t("story.events.track")}>
  {@render picked("Track", session.track, track)}
  {#if layouts.length > 1 || (session.layout && !layouts.some((l) => l.id === session.layout))}
    <select
      class="input layout"
      value={session.layout ?? ""}
      aria-label={t("story.events.layout")}
      onchange={(e) => set("layout", (e.currentTarget as HTMLSelectElement).value)}
    >
      {#if session.layout && !layouts.some((l) => l.id === session.layout)}
        <option value={session.layout}>{session.layout}</option>
      {/if}
      {#each layouts as l (l.id)}
        <option value={l.id}>{l.name}</option>
      {/each}
    </select>
  {/if}
</Field>

<Field label={t("story.events.car")}>
  {@render picked("Car", session.car, car)}
</Field>

{#if isRace}
  <div class="pair">
    <Field label={t("story.events.laps")}>
      <NumberStepper value={session.laps ?? 0} emptyValue={0} min={1} max={200} width={90} onchange={number("laps", 0)} />
    </Field>
    <Field label={t("story.events.grid")}>
      <NumberStepper value={session.grid ?? 0} emptyValue={0} min={1} max={40} width={90} onchange={number("grid", 0)} />
    </Field>
    <Field label={t("story.events.aiLevel")}>
      <NumberStepper
        value={session.ai_level ?? -1}
        emptyValue={-1}
        emptyStart={90}
        min={0}
        max={100}
        width={90}
        onchange={number("ai_level", -1)}
      />
    </Field>
  </div>
  <Field label={t("story.events.start")}>
    <Seg
      value={session.start ?? "random"}
      onselect={(v) => set("start", v === "random" ? undefined : (v as GridStart))}
      items={(["random", "first", "second", "last"] as const).map((s) => ({
        value: s,
        label: t(`story.events.start_${s}`),
      }))}
    />
  </Field>
{/if}

<div class="pair">
  <Field label={t("story.events.time")}>
    <input
      class="input time"
      type="time"
      value={session.time ?? ""}
      onchange={(e) => set("time", (e.currentTarget as HTMLInputElement).value)}
    />
  </Field>
  <Field label={t("story.events.weather")}>
    <select
      class="input"
      value={session.weather ?? ""}
      onchange={(e) => set("weather", (e.currentTarget as HTMLSelectElement).value)}
    >
      <option value="">{t("story.events.byDefault")}</option>
      {#if session.weather && !(WEATHER_IDS as readonly string[]).includes(session.weather)}
        <option value={session.weather}>{session.weather}</option>
      {/if}
      {#each WEATHER_IDS as id (id)}
        <option value={id}>{t(WEATHER_LABEL_KEYS[id])}</option>
      {/each}
    </select>
  </Field>
  {#if money || event.entry_fee !== null}
    <Field label={t("story.events.entryFee")}>
      <NumberStepper
        value={event.entry_fee ?? 0}
        emptyValue={0}
        min={0}
        step={10}
        width={110}
        onchange={(v) => {
          event.entry_fee = v === 0 ? null : v;
          onsave();
        }}
      />
    </Field>
  {/if}
</div>

{#if picking}
  <LibraryPicker
    kind={picking}
    current={picking === "Car" ? session.car : session.track}
    onpick={pick}
    onclose={() => (picking = null)}
  />
{/if}

<style>
  .picked {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    font-size: 13px;
  }
  .id {
    font-size: 10.5px;
    color: var(--muted);
  }
  .none {
    flex: 1;
    color: var(--faint);
    font-size: 12.5px;
  }
  .layout {
    margin-top: 8px;
    max-width: 320px;
  }
  .pair {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
    margin-top: 20px;
  }
  .pair.pair > :global(.field) {
    margin-top: 0;
  }
  .time {
    width: 120px;
  }
</style>
