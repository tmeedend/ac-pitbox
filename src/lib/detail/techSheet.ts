// The tech sheet of a car, as the page shows it (FICHE§7) and edits it
// (FICHE§8).
//
// The backend has already decided every value and where it comes from
// (`techsheet::effective`); what is left here is the wording: numbers in the
// user's language, the rules' codes as words, a few values run into a sentence
// per row, and the two signs of R5 — "≈" before what the tags deduced, "✎"
// after what the user corrected. No colour carries a provenance any more.
//
// Pure functions, so the page and the tests share them; the locale is a
// parameter rather than read from the i18n state for the same reason.
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/index.svelte";

/** Where a value comes from (R2). `computed` is a ratio worked out from two
 * others, never stored. */
export type TechSource = "user" | "physics" | "ui" | "table" | "rules" | "computed";

export interface Resolved {
  value: unknown;
  source: TechSource;
}

export interface TechSheet {
  values: Record<string, Resolved>;
  /** Fields the user decided — a value, or a forced "unknown". */
  edited: string[];
  /** What the mod itself says for each edited field: where "revert" goes. */
  fallback: Record<string, Resolved>;
}

/** A number the author wrote with a unit, their `+` kept. */
export interface Quantity {
  n: number;
  unit?: string;
  plus?: boolean;
}

/** One decision of the edit mode. `value: null` is "unknown". */
export interface TechEdit {
  field: string;
  value: unknown;
  revert?: boolean;
}

export function saveTechSheet(id: string, edits: TechEdit[]): Promise<void> {
  return invoke<void>("save_tech_sheet", { id, edits });
}

// --- The fields -------------------------------------------------------------

/** The key figures, in the order of the strip, with the unit a typed value
 * is stored in (the backend's `KEY_FIGURES`). */
export const KEY_FIGURES = [
  { field: "power", unit: "bhp" },
  { field: "torque", unit: "Nm" },
  { field: "weight", unit: "kg" },
  { field: "pwratio", unit: "kg/hp" },
  { field: "topspeed", unit: "km/h" },
  { field: "acceleration", unit: "s" },
] as const;

/** The closed lists (FICHE§8): the rules' codes, which are also what the
 * library filters and sorts on. */
export const CHOICES: Record<string, readonly string[]> = {
  engine_config: ["I4", "I6", "V6", "V8", "V10", "V12", "FLAT", "ROTARY", "ELECTRIC", "HYBRID", "DIESEL"],
  aspiration: ["NA", "TURBO", "TWIN_TURBO", "SUPERCHARGED"],
  engine_pos: ["FRONT", "MID", "REAR"],
  drivetrain: ["RWD", "FWD", "AWD"],
  gearbox: ["MANUAL", "SEQUENTIAL", "SEMIAUTO", "DCT", "AUTO", "PADDLES"],
};

/** The aids, in chip order. `showAbsent`: an absence worth saying — "no ABS"
 * on a 1966 car is information, "no DRS" on a hatchback is noise (FICHE§5). */
export const AIDS = [
  { field: "aid.abs", showAbsent: true },
  { field: "aid.tc", showAbsent: true },
  { field: "aid.edl", showAbsent: false },
  { field: "aid.drs", showAbsent: false },
  { field: "aid.kers", showAbsent: false },
  { field: "aid.ers", showAbsent: false },
  { field: "aid.4ws", showAbsent: false },
  { field: "aid.active_arb", showAbsent: false },
  { field: "aid.ebb", showAbsent: false },
] as const;

export function fieldLabel(field: string): string {
  return t(`techsheet.field.${field}`);
}

/** A code of the rules as a word ("RWD" → "propulsion"), the code itself when
 * the rules produce one this list does not know. */
export function choiceLabel(field: string, code: string): string {
  const key = `techsheet.value.${field}.${code}`;
  const label = t(key);
  return label === key ? code : label;
}

// --- Numbers ----------------------------------------------------------------

export function formatNumber(n: number, locale: string, digits = 2): string {
  return n.toLocaleString(locale, { maximumFractionDigits: digits });
}

export function isQuantity(v: unknown): v is Quantity {
  return typeof v === "object" && v !== null && typeof (v as Quantity).n === "number";
}

/** A key figure as the strip shows it: the number, the unit in small, the
 * author's `+` — or the author's text, when it was not a number. */
export interface Figure {
  field: string;
  source: TechSource;
  num?: string;
  unit?: string;
  text?: string;
}

export function keyFigures(sheet: TechSheet, locale: string): Figure[] {
  const out: Figure[] = [];
  for (const { field } of KEY_FIGURES) {
    const r = sheet.values[field];
    if (!r) continue;
    if (isQuantity(r.value)) {
      out.push({
        field,
        source: r.source,
        num: formatNumber(r.value.n, locale) + (r.value.plus ? "+" : ""),
        unit: r.value.unit,
      });
    } else if (typeof r.value === "string") {
      out.push({ field, source: r.source, text: r.value });
    }
  }
  return out;
}

// --- Mechanics: short sentences ------------------------------------------------

/** One piece of a row's sentence, with its own provenance. */
export interface Part {
  text: string;
  source: TechSource;
  /** A country's flag, found by the caller. */
  flag?: string | null;
}

export interface Row {
  key: "engine" | "transmission" | "fuel" | "origin";
  parts: Part[];
}

export interface RowDeps {
  locale: string;
  /** The country in the user's language, and its flag (`flags.svelte.ts`). */
  countryLabel: (name: string) => string;
  flagFor: (name: string) => string | null;
}

/** The four rows of the Mechanics block, empty ones left out (R4). */
export function mechanicsRows(sheet: TechSheet, deps: RowDeps): Row[] {
  const v = sheet.values;
  const num = (f: string) => (typeof v[f]?.value === "number" ? (v[f].value as number) : null);
  const part = (f: string, text: string): Part => ({ text, source: v[f].source });
  const choice = (f: string): Part | null =>
    typeof v[f]?.value === "string" ? part(f, choiceLabel(f, v[f].value as string)) : null;
  const numbered = (f: string, key: string, name = "n"): Part | null => {
    const n = num(f);
    return n == null ? null : part(f, t(key, { [name]: formatNumber(n, deps.locale, 0) }));
  };
  const gears = num("gears");
  const country = typeof v.country?.value === "string" ? (v.country.value as string) : null;

  const rows: Row[] = [
    {
      key: "engine",
      parts: [
        choice("engine_config"),
        choice("aspiration"),
        choice("engine_pos"),
        numbered("rpm_limit", "techsheet.rpm"),
      ].filter((p): p is Part => p !== null),
    },
    {
      key: "transmission",
      parts: [
        choice("drivetrain"),
        choice("gearbox"),
        gears == null
          ? null
          : part("gears", gears === 1 ? t("techsheet.gearsOne") : t("techsheet.gears", { count: gears })),
      ].filter((p): p is Part => p !== null),
    },
    {
      key: "fuel",
      parts: [numbered("fuel_tank", "techsheet.tank"), numbered("range", "techsheet.range")].filter(
        (p): p is Part => p !== null,
      ),
    },
    {
      key: "origin",
      parts: [
        country ? { ...part("country", deps.countryLabel(country)), flag: deps.flagFor(country) } : null,
        num("year") == null ? null : part("year", String(num("year"))),
      ].filter((p): p is Part => p !== null),
    },
  ];
  return rows.filter((r) => r.parts.length > 0);
}

// --- Electronics -------------------------------------------------------------

export interface Chip {
  field: string;
  present: boolean;
  source: TechSource;
  /** "MGU-K + MGU-H" on an ERS that has both. */
  sub?: string;
}

/** Present aids, and the absences worth saying; unknown ones are no chip. */
export function aidChips(sheet: TechSheet): Chip[] {
  const out: Chip[] = [];
  for (const { field, showAbsent } of AIDS) {
    const r = sheet.values[field];
    if (typeof r?.value !== "boolean") continue;
    if (!r.value && !showAbsent) continue;
    const chip: Chip = { field, present: r.value, source: r.source };
    if (field === "aid.ers" && r.value && sheet.values["aid.ers_heat"]?.value === true) {
      chip.sub = "MGU-K + MGU-H";
    }
    out.push(chip);
  }
  return out;
}

// --- Provenance (R5) ------------------------------------------------------------

/** The sign before a value: deduced from the tags. */
export const isDerived = (s: TechSource) => s === "rules";
/** The sign after a value: corrected by the user. */
export const isEdited = (s: TechSource) => s === "user";

// --- The edit mode ---------------------------------------------------------------

/** Every field the edit mode offers, by kind — mirrors the backend's checks. */
export const NUMBER_FIELDS = ["rpm_limit", "gears", "fuel_tank", "range", "year"] as const;

const isKeyFigure = (field: string) => KEY_FIGURES.some((k) => k.field === field);

/** What a field's input starts from: the number of a quantity, a plain
 * number, a code. A key figure the author wrote as text is no starting value
 * — the input stays empty, the text shown beside it. */
export function draftOf(field: string, r: Resolved | undefined): string {
  if (!r) return "";
  if (isQuantity(r.value)) return String(r.value.n);
  if (typeof r.value === "number") return String(r.value);
  if (typeof r.value === "string" && !isKeyFigure(field)) return r.value;
  return "";
}

/** A typed number, or `null` for an empty input, or `undefined` when it does
 * not read as a positive number. Decimal comma accepted. */
export function parseInput(raw: string): number | null | undefined {
  const s = raw.trim().replace(",", ".");
  if (!s) return null;
  const n = Number(s);
  return Number.isFinite(n) && n > 0 ? n : undefined;
}

/** The three states of an aid in edit mode, in click order (FICHE§8). */
export type AidState = true | false | null;
export function nextAidState(s: AidState): AidState {
  return s === true ? false : s === false ? null : true;
}

// --- The report of a reading (FICHE§9.3) -------------------------------------

export interface FieldChanges {
  gained: number;
  changed: number;
  lost: number;
}

export interface TechSheetReport {
  cars: number;
  /** Per spec column: `drivetrain`, `aspiration`, `gearbox`, `engine_config`,
   * `engine_pos`. Only the columns that moved. */
  fields: Record<string, FieldChanges>;
}

/** The library column each field is shown under — the report speaks of the
 * columns, which is where the change is seen. */
export const COLUMN_LABEL: Record<string, string> = {
  drivetrain: "columns.drivetrain",
  aspiration: "columns.aspiration",
  gearbox: "columns.gearbox",
  engine_config: "columns.engineConfig",
  engine_pos: "columns.enginePos",
};

/** One line per column that moved, its non-zero counts only, in the order
 * of the library's columns. */
export function reportLines(r: TechSheetReport): { field: string; counts: [keyof FieldChanges, number][] }[] {
  return Object.keys(COLUMN_LABEL)
    .filter((f) => r.fields[f])
    .map((f) => ({
      field: f,
      counts: (["gained", "changed", "lost"] as const)
        .map((k) => [k, r.fields[f][k]] as [keyof FieldChanges, number])
        .filter(([, n]) => n > 0),
    }));
}
