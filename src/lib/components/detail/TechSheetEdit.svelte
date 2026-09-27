<script lang="ts">
  // The tech sheet, edit mode (FICHE§8). A form over the whole sheet rather
  // than a field per cell: a sheet where every value is an input reads badly
  // and handles badly with the D-pad.
  //
  // Each field says where its value comes from, in words, and a corrected one
  // offers "↺ revenir", which goes back to what the mod says (`fallback`).
  // Nothing is written before "Enregistrer": reverts included, so "Annuler"
  // really leaves the sheet as it was.
  import { untrack } from "svelte";
  import ImageSelectDropdown from "$lib/components/ui/ImageSelectDropdown.svelte";
  import { countryLabel, flagFor, gameCountries, loadFlags } from "$lib/flags.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    AIDS,
    CHOICES,
    KEY_FIGURES,
    choiceLabel,
    draftOf,
    fieldLabel,
    nextAidState,
    parseInput,
    type AidState,
    type TechEdit,
    type TechSheet,
  } from "$lib/detail/techSheet";
  import AidChip from "./AidChip.svelte";
  import FigureGrid from "./FigureGrid.svelte";
  import SheetRow from "./SheetRow.svelte";

  interface Props {
    sheet: TechSheet;
    busy: boolean;
    onsave: (edits: TechEdit[]) => void;
    oncancel: () => void;
  }
  let { sheet, busy, onsave, oncancel }: Props = $props();

  $effect(() => {
    void loadFlags();
  });

  /** The unit shown after a field typed as a number, if it has one. */
  function numberUnit(f: string): string {
    if (f === "rpm_limit") return t("techsheet.unit.rpm");
    if (f === "fuel_tank") return "L";
    if (f === "range") return "km";
    return "";
  }
  const INTEGER_FIELDS = new Set(["gears", "year"]);
  const ROWS: { key: string; fields: string[] }[] = [
    { key: "engine", fields: ["engine_config", "aspiration", "engine_pos", "rpm_limit"] },
    { key: "transmission", fields: ["drivetrain", "gearbox", "gears"] },
    { key: "fuel", fields: ["fuel_tank", "range"] },
    { key: "origin", fields: ["country", "year"] },
  ];
  const TEXT_FIELDS = [...KEY_FIGURES.map((k) => k.field), ...ROWS.flatMap((r) => r.fields)];

  // The starting point, read once: the sheet does not change under an open
  // form (the page reloads it after a save, and closes the form).
  const initial = untrack(() => {
    const text: Record<string, string> = {};
    for (const f of TEXT_FIELDS) text[f] = draftOf(f, sheet.values[f]);
    const aids: Record<string, AidState> = {};
    for (const { field } of AIDS) {
      const v = sheet.values[field]?.value;
      aids[field] = typeof v === "boolean" ? v : null;
    }
    return { text, aids };
  });
  let text = $state({ ...initial.text });
  let aids = $state({ ...initial.aids });
  /** Fields whose correction goes at the next save. */
  let reverts = $state<string[]>([]);
  let error = $state<string | null>(null);

  const edited = (f: string) => sheet.edited.includes(f) && !reverts.includes(f);

  /** "↺ revenir": the input shows what the mod says, the decision goes at save. */
  function revert(f: string) {
    reverts = [...reverts, f];
    if (f.startsWith("aid.")) {
      const v = sheet.fallback[f]?.value;
      aids[f] = typeof v === "boolean" ? v : null;
      initial.aids[f] = aids[f];
    } else {
      text[f] = draftOf(f, sheet.fallback[f]);
      initial.text[f] = text[f];
    }
  }

  /** Typing again after a revert is a new decision, not a revert. */
  function touched(f: string) {
    reverts = reverts.filter((r) => r !== f);
  }

  /** Where the value shown comes from, in words. */
  function sourceText(f: string): string {
    if (edited(f)) return t("techsheet.source.user");
    const r = reverts.includes(f) ? sheet.fallback[f] : sheet.values[f];
    if (!r) return t("techsheet.source.absent");
    return t(`techsheet.source.${r.source}`);
  }

  /** The author's text, when a key figure was not a number: shown as the
   * input's placeholder, since there is no number to start from. */
  function placeholder(f: string): string {
    const v = sheet.values[f]?.value;
    return typeof v === "string" ? v : "";
  }

  const countries = $derived(
    gameCountries()
      .map((n) => ({ id: n.name, name: countryLabel(n.name), image: flagFor(n.name) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  function collect(): TechEdit[] | null {
    const edits: TechEdit[] = [];
    for (const f of TEXT_FIELDS) {
      if (reverts.includes(f)) {
        edits.push({ field: f, value: null, revert: true });
        continue;
      }
      if (text[f] === initial.text[f]) continue;
      if (f in CHOICES || f === "country") {
        edits.push({ field: f, value: text[f] || null });
        continue;
      }
      const n = parseInput(text[f]);
      if (n === undefined || (n !== null && INTEGER_FIELDS.has(f) && !Number.isInteger(n))) {
        error = t("techsheet.invalid", { field: fieldLabel(f) });
        return null;
      }
      edits.push({ field: f, value: n });
    }
    for (const { field } of AIDS) {
      if (reverts.includes(field)) edits.push({ field, value: null, revert: true });
      else if (aids[field] !== initial.aids[field]) edits.push({ field, value: aids[field] });
    }
    return edits;
  }

  function save() {
    error = null;
    const edits = collect();
    if (edits) onsave(edits);
  }

  function aidStateText(s: AidState): string {
    return s === true ? t("techsheet.aidPresent") : s === false ? t("techsheet.aidAbsent") : t("techsheet.aidUnknown");
  }
</script>

{#snippet source(f: string)}
  <div class="src">
    <span>{sourceText(f)}</span>
    {#if edited(f)}
      <button class="revert" type="button" onclick={() => revert(f)}>↺ {t("techsheet.revert")}</button>
    {/if}
  </div>
{/snippet}

{#snippet field(f: string)}
  <label class="field">
    <span class="lbl-key cap">{fieldLabel(f)}</span>
    {#if f in CHOICES}
      <select class="input" bind:value={text[f]} onchange={() => touched(f)}>
        <option value="">{t("techsheet.unknown")}</option>
        {#each CHOICES[f] as code (code)}
          <option value={code}>{choiceLabel(f, code)}</option>
        {/each}
        <!-- A code the rules produce that the list does not offer stays
             selectable, so opening the form never changes it. -->
        {#if initial.text[f] && !CHOICES[f].includes(initial.text[f])}
          <option value={initial.text[f]}>{initial.text[f]}</option>
        {/if}
      </select>
    {:else}
      <span class="num">
        <input
          class="input mono"
          inputmode="decimal"
          bind:value={text[f]}
          oninput={() => touched(f)}
          aria-label={fieldLabel(f)}
        />
        {#if numberUnit(f)}<span class="unit">{numberUnit(f)}</span>{/if}
      </span>
    {/if}
    {@render source(f)}
  </label>
{/snippet}

<FigureGrid>
  {#each KEY_FIGURES as k (k.field)}
    <label class="fig">
      <span class="lbl-key cap">{fieldLabel(k.field)}</span>
      <span class="num">
        <input
          class="input mono"
          inputmode="decimal"
          bind:value={text[k.field]}
          oninput={() => touched(k.field)}
          placeholder={placeholder(k.field)}
          aria-label={fieldLabel(k.field)}
        />
        <span class="unit">{k.unit}</span>
      </span>
      {@render source(k.field)}
    </label>
  {/each}
</FigureGrid>

<div class="section">
  <div class="blk-sub">{t("techsheet.mechanics")}</div>
  {#each ROWS as r (r.key)}
    <SheetRow label={t(`techsheet.row.${r.key}`)}>
      <div class="fields">
        {#each r.fields as f (f)}
          {#if f === "country"}
            <div class="field country">
              <span class="lbl-key cap">{fieldLabel(f)}</span>
              <ImageSelectDropdown
                options={countries}
                selectedId={text.country || null}
                placeholder={t("techsheet.unknown")}
                emptyText={t("techsheet.unknown")}
                fit="contain"
                onselect={(id) => {
                  text.country = id;
                  touched("country");
                }}
              />
              {@render source(f)}
            </div>
          {:else}
            {@render field(f)}
          {/if}
        {/each}
      </div>
    </SheetRow>
  {/each}
</div>

<div class="section">
  <div class="blk-sub">{t("techsheet.electronics")}</div>
  <div class="chips">
    {#each AIDS as a (a.field)}
      <div class="aid">
        <AidChip
          state={aids[a.field]}
          sub={aidStateText(aids[a.field])}
          onclick={() => {
            aids[a.field] = nextAidState(aids[a.field]);
            touched(a.field);
          }}>{fieldLabel(a.field)}</AidChip
        >
        {#if edited(a.field)}
          <button class="revert" type="button" onclick={() => revert(a.field)}>↺ {t("techsheet.revert")}</button>
        {/if}
      </div>
    {/each}
  </div>
</div>

{#if error}<div class="errbox">{error}</div>{/if}
<div class="bar">
  <button class="btn btn-ghost" type="button" onclick={oncancel} disabled={busy}>{t("techsheet.cancel")}</button>
  <button class="btn btn-primary" type="button" onclick={save} disabled={busy}>{t("techsheet.save")}</button>
</div>

<style>
  .fig {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .cap {
    text-transform: uppercase;
  }
  .num {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }
  .num .input,
  select.input {
    padding: 4px 7px;
    min-width: 0;
  }
  .unit {
    font-size: 10.5px;
    color: var(--muted);
    flex: none;
  }

  .section {
    padding: 12px 14px 6px;
  }
  .section + .section {
    border-top: 1px solid var(--line);
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 14px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 150px;
    max-width: 100%;
  }
  .field.country {
    width: 200px;
  }

  /* Where the value comes from, in words (FICHE§8) — grey, below its field. */
  .src {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    font-size: 10px;
    color: var(--muted2);
  }
  .revert {
    background: none;
    padding: 0;
    color: var(--blue);
    font-size: 10px;
    align-self: flex-start;
  }
  .revert:hover {
    color: var(--txt2);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 2px 0 6px;
  }
  .aid {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .errbox {
    margin: 0 14px 10px;
  }
  .bar {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--line);
  }
</style>
