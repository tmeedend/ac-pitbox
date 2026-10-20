<script lang="ts">
  // The Drivers screen (SPEC-story-mode, Drivers): the story's roster,
  // `drivers/roster.yaml` - the drivers a race's grid can name. Optional: a
  // story without one races against filler names. Each edit saves the file at
  // once; a driver's id renames the grids and criteria citing it (NameRow).
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { AI_LEVEL_MAX, AI_LEVEL_MIN } from "$lib/launch/opponent";
  import { DRIVER_ROLES } from "$lib/story/drivers";
  import { gameCountries, loadFlags } from "$lib/flags.svelte";
  import type { Driver, DriverRole } from "$lib/story/studio";
  import { saveStoryFile, studio } from "$lib/story/studioState.svelte";
  import { nameFrom } from "$lib/story/world";
  import AnchoredPopover from "$lib/components/filters/AnchoredPopover.svelte";
  import NationalityPicker from "$lib/components/launch/NationalityPicker.svelte";
  import Field from "$lib/components/ui/Field.svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import AddName from "./AddName.svelte";
  import CountryLabel from "./CountryLabel.svelte";
  import FileStatus from "./FileStatus.svelte";
  import NameRow from "./NameRow.svelte";
  import StoryChecks from "./StoryChecks.svelte";

  const story = $derived(studio.story!);
  const roster = $derived(story.drivers.value);
  const characters = $derived(story.characters.value?.characters ?? []);
  const driverIds = $derived(roster?.drivers.map((d) => d.id) ?? []);
  /** The characters already another driver, by driver row. */
  const charactersTaken = $derived(new Set(roster?.drivers.flatMap((d) => (d.character ? [d.character] : [])) ?? []));

  /** The row whose country list is open, and the cell it hangs from. */
  let natRow = $state<number | null>(null);
  let natAnchor = $state<HTMLElement | null>(null);

  onMount(() => void loadFlags());

  function save() {
    void saveStoryFile("drivers");
  }

  function addDriver(label: string): string | null {
    if (!roster) return null;
    // Named as typed, like a character; a story's drivers start with no skill
    // of their own, so the event's applies until the author sets one.
    roster.drivers.push({ id: nameFrom(label, driverIds, "driver"), name: label });
    save();
    return null;
  }

  /** Sets or removes one key of driver `i`, then saves. */
  function set<K extends Exclude<keyof Driver, "id">>(i: number, key: K, v: Driver[K] | undefined) {
    const d = roster?.drivers[i];
    if (!d) return;
    if (v === undefined || v === "") delete d[key];
    else d[key] = v;
    save();
  }

  function openNat(i: number, anchor: HTMLElement) {
    if (natRow === i) return closeNat();
    natRow = i;
    natAnchor = anchor;
  }
  function closeNat() {
    natRow = null;
    natAnchor = null;
  }

  function value(e: Event): string {
    return (e.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }
</script>

<div class="main">
  <header>
    <span class="lbl-screen">{t("story.drivers.title")}</span>
  </header>
  {#if studio.notice}<p class="warnbox">{studio.notice}</p>{/if}

  <section class="blk">
    <div class="blk-h">
      <span class="blk-t">{t("story.drivers.roster")}</span>
      <span class="blk-n">{driverIds.length}</span>
    </div>
    <div class="blk-b">
      <FileStatus entry={story.drivers} />
      {#if roster}
        {#if roster.drivers.length}
          <div class="cols lbl-key">
            <span class="c-name">{t("story.world.name")}</span>
            <span>{t("story.world.displayName")}</span>
            <span>{t("story.drivers.character")}</span>
          </div>
        {:else}
          <p class="empty">{t("story.drivers.none")}</p>
        {/if}
        <ul class="rows">
          {#each roster.drivers as d, i (d.id + ":" + i)}
            {@const character = characters.find((c) => c.id === d.character)}
            <NameRow
              kind="driver"
              id={d.id}
              taken={driverIds}
              onremove={() => {
                roster.drivers = roster.drivers.filter((_, k) => k !== i);
                save();
              }}
            >
              <input
                class="input display"
                value={d.name ?? ""}
                placeholder={character?.name ?? ""}
                aria-label={t("story.world.displayName")}
                onchange={(e) => set(i, "name", value(e).trim())}
              />
              <select
                class="input character"
                value={d.character ?? ""}
                aria-label={t("story.drivers.character")}
                onchange={(e) => set(i, "character", value(e))}
              >
                <option value="">{t("story.drivers.noCharacter")}</option>
                {#if d.character && !character}
                  <option value={d.character}>{d.character}</option>
                {/if}
                {#each characters as c (c.id)}
                  <option value={c.id} disabled={c.id !== d.character && charactersTaken.has(c.id)}
                    >{c.name ?? c.id}</option
                  >
                {/each}
              </select>
              {#snippet below()}
                <div class="details">
                  <Field label={t("story.drivers.nationality")}>
                    <button
                      class="input nat"
                      type="button"
                      aria-expanded={natRow === i}
                      onclick={(e) => openNat(i, e.currentTarget)}
                    >
                      <CountryLabel name={d.nationality ?? null} fallback={t("launch.autoCell")} />
                    </button>
                  </Field>
                  <Field label={t("story.drivers.age")}>
                    <NumberStepper
                      value={d.age ?? 0}
                      emptyValue={0}
                      emptyStart={20}
                      min={0}
                      max={99}
                      width={80}
                      onchange={(v) => set(i, "age", v === 0 ? undefined : v)}
                    />
                  </Field>
                  <Field label={t("story.drivers.role")}>
                    <select
                      class="input role"
                      value={d.role ?? ""}
                      onchange={(e) => set(i, "role", (value(e) || undefined) as DriverRole | undefined)}
                    >
                      <option value="">—</option>
                      {#each DRIVER_ROLES as role (role)}
                        <option value={role}>{t(`story.drivers.role_${role}`)}</option>
                      {/each}
                    </select>
                  </Field>
                  <Field label={t("story.drivers.skill")}>
                    <NumberStepper
                      value={d.ai_level ?? -1}
                      emptyValue={-1}
                      emptyStart={90}
                      min={AI_LEVEL_MIN}
                      max={AI_LEVEL_MAX}
                      width={80}
                      onchange={(v) => set(i, "ai_level", v === -1 ? undefined : v)}
                    />
                  </Field>
                  <Field label={t("story.drivers.aggression")}>
                    <NumberStepper
                      value={d.aggression ?? -1}
                      emptyValue={-1}
                      emptyStart={50}
                      min={0}
                      max={100}
                      width={80}
                      onchange={(v) => set(i, "aggression", v === -1 ? undefined : v)}
                    />
                  </Field>
                </div>
              {/snippet}
            </NameRow>
          {/each}
        </ul>
        <AddName placeholder={t("story.drivers.newDriver")} onadd={addDriver} />
      {/if}
    </div>
  </section>

  <StoryChecks section="drivers" />
</div>

{#if natRow != null && natAnchor && roster}
  {@const row = natRow}
  <AnchoredPopover anchor={natAnchor} minWidth={244} onclose={closeNat}>
    <NationalityPicker
      nationalityList={gameCountries()}
      current={roster.drivers[row]?.nationality ?? null}
      autoLabel={t("launch.autoCell")}
      onpick={(name) => {
        set(row, "nationality", name ?? undefined);
        closeNat();
      }}
    />
  </AnchoredPopover>
{/if}

<style>
  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 22px 28px;
  }
  header {
    margin-bottom: 14px;
  }
  .blk {
    max-width: 820px;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .cols {
    display: flex;
    gap: 10px;
    margin-bottom: 4px;
  }
  .cols span {
    width: 190px;
  }
  .display {
    width: 190px;
  }
  .character {
    width: 190px;
  }
  .empty {
    margin: 0 0 4px;
    font-size: 12.5px;
    color: var(--muted);
  }
  /* The second line of a driver: the fields an entry list carries. */
  .details {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    margin: 8px 0 2px;
  }
  .details > :global(.field) {
    margin-top: 0;
  }
  .nat {
    display: flex;
    align-items: center;
    width: 190px;
    text-align: left;
    cursor: pointer;
  }
  .role {
    width: 130px;
  }
</style>
