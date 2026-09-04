// Library filter model (SPEC §6.3).
//
// The bar used to hold eleven controls, permanently, over some 200 px of
// height - for a user who reaches for one or two of them. Every filter now
// lives as a CHIP, and its editor in a popover anchored to that chip, so the
// complexity of a filter no longer costs any room in the bar: a multi-token
// field with an operator takes the same 28 px as a checkbox.
//
// Three rules carry the whole thing:
//   R1  the chip is an anchor, not a control;
//   R2  clicking edits, the cross removes - the "no filter" state is the
//       ABSENCE of the entry, never a value kept around meaning "any";
//   R3  polarity belongs to the value, not to the filter: `brand = Ferrari`
//       and `brand != Abarth` live side by side in one chip. There is no
//       global include/exclude switch.
//
// This module is deliberately free of Svelte and of the DOM: catalogue,
// evaluation and summaries are plain functions, so the two library screens
// (cars and tracks) share them and they stay readable on their own.
import type { ModCard, ModKind } from "./library";
import { t } from "./i18n/index.svelte";

export type Sign = 1 | -1;
export type Operator = "and" | "or";

/** One value of a `val` filter, with the sense given to it. */
export interface SignedValue {
  value: string;
  sign: Sign;
}

/** State of ONE active filter. Absence of the key in `FilterMap` is the only
 * representation of "this filter does not apply" (R2). */
export type FilterState =
  | { type: "val"; values: SignedValue[]; op: Operator }
  | { type: "range"; min: number | null; max: number | null }
  | { type: "text"; text: string }
  | { type: "bool"; sign: Sign };

export type FilterMap = Record<string, FilterState>;

export interface FilterChoice {
  value: string;
  /** Translated label. Absent for values that come from the library itself
   * (a brand, an author, a tag - they are never translated). */
  labelKey?: string;
}

export interface FilterDef {
  key: string;
  labelKey: string;
  type: FilterState["type"];
  /** `val` with a FIXED vocabulary. Without it, the values offered are read
   * off the library (and so is their label). */
  choices?: FilterChoice[];
  /** `val`: exposes the AND/OR switch over inclusions. Only worth it on a
   * field a mod carries SEVERAL values of - on a single-valued field an AND
   * over two values could never match anything. */
  operator?: boolean;
  /** `bool`: label of the negative state ("Everything but favourites"). */
  negLabelKey?: string;
  /** Polarity taken when the filter (or one of its values) is first posed. */
  defaultSign?: Sign;
}

/** The four exclusive states of a mod, plus `broken` which cuts across them.
 *
 * `active` deliberately excludes base content, unlike the old `<select>`:
 * `c.active` is true for stock content too, so "Active" used to return the
 * whole Kunos catalogue along with the deployed mods. Same arbitration as
 * `StateBadge`, which is what the user reads in the table's State column -
 * the filter and the badge now say the same thing. */
const STATE_CHOICES: FilterChoice[] = [
  { value: "active", labelKey: "common.active" },
  { value: "inactive", labelKey: "common.inactive" },
  { value: "stock", labelKey: "common.stockState" },
  { value: "unmanaged", labelKey: "common.unmanagedState" },
  { value: "broken", labelKey: "common.brokenState" },
];

/** The catalogue for one library. It is NOT the same on both screens: brand,
 * year, class and driver outfit only exist for cars, which is why the pinned
 * set is stored per kind. Order here is the order of the add menu and of the
 * ghost chips. */
export function filterDefs(kind: ModKind): FilterDef[] {
  const isCar = kind === "Car";
  const defs: FilterDef[] = [
    // A track carries SEVERAL categories, a car exactly one: the operator is
    // only offered where an AND can ever match.
    { key: "category", labelKey: "library.filterCategory", type: "val", operator: !isCar },
  ];
  if (isCar) defs.push({ key: "brand", labelKey: "library.filterBrand", type: "val" });
  defs.push(
    { key: "tag", labelKey: "library.filterTag", type: "val", operator: true },
    { key: "author", labelKey: "library.filterAuthor", type: "val" },
    { key: "country", labelKey: "library.filterCountry", type: "val" },
  );
  if (isCar) {
    defs.push(
      { key: "year", labelKey: "library.filterYear", type: "range" },
      { key: "carClass", labelKey: "library.filterClass", type: "val" },
    );
  }
  defs.push(
    { key: "state", labelKey: "library.filterState", type: "val", choices: STATE_CHOICES },
    { key: "description", labelKey: "library.filterDescription", type: "text" },
    { key: "favorite", labelKey: "library.favorites", type: "bool", negLabelKey: "library.favExcludedShort" },
  );
  if (isCar) {
    defs.push({ key: "driver", labelKey: "library.driverSet", type: "bool", negLabelKey: "library.driverSetExcludedShort" });
  }
  // No "base content" filter of its own: it says exactly what `state = stock`
  // says, and two controls for one question is how a bar grows back to eleven.
  // What it used to buy - one click to "everything but the Kunos cars" - is
  // bought instead by pinning State, whose editor offers that value with a
  // `−` next to it.
  defs.push({ key: "tried", labelKey: "library.tried", type: "bool", negLabelKey: "library.neverTried" });
  return defs;
}

/** Pinned out of the box. All three exist on BOTH screens, so the ghost row
 * reads the same for cars and for tracks - which is exactly why brand and
 * year are not in it. State is there for what the removed "base content"
 * checkbox used to do: on a real install the Kunos cars are the ones the user
 * knows by heart and is not looking for, and that stays the most frequent
 * negative filter of the screen. */
export function defaultPinned(): string[] {
  return ["category", "favorite", "state"];
}

/** What a mod has to be matched against for a given `val` filter, in its
 * ORIGINAL case (the suggestion list shows these strings, and "Ferrari" must
 * not become "ferrari" on screen). Comparison lowercases both sides. */
export interface FilterContext {
  isCar: boolean;
  /** All three tag origins merged - they are equivalent for filtering. */
  tagsOf: (c: ModCard) => string[];
  /** Effective description, markup stripped and lowercased; `undefined` when
   * the mod has none, which can then never match. */
  descOf: (c: ModCard) => string | undefined;
  /** Whether this car has been given a driver outfit of its own. */
  hasDriver: (id: string) => boolean;
}

const one = (v: string | null): string[] => (v ? [v] : []);

function stateValues(c: ModCard): string[] {
  const out = [c.is_unmanaged ? "unmanaged" : c.is_stock ? "stock" : c.active ? "active" : "inactive"];
  if (c.broken) out.push("broken");
  return out;
}

/** Values a card carries for a `val` filter. Returns a closure so the switch
 * is resolved ONCE per filter, not once per card: the predicate below runs
 * over the whole library on every keystroke of the search field. */
function valuesOf(key: string, ctx: FilterContext): (c: ModCard) => string[] {
  switch (key) {
    case "category":
      return ctx.isCar ? (c) => one(c.category) : (c) => c.categories;
    case "brand":
      return (c) => one(c.brand);
    case "author":
      return (c) => one(c.author);
    case "country":
      return (c) => one(c.country);
    case "carClass":
      return (c) => one(c.car_class);
    case "tag":
      return ctx.tagsOf;
    case "state":
      return stateValues;
    default:
      return () => [];
  }
}

function boolOf(key: string, ctx: FilterContext): (c: ModCard) => boolean {
  switch (key) {
    case "favorite":
      return (c) => c.is_favorite;
    case "tried":
      return (c) => c.tried;
    // `is_stock` covers everything living in content/: the game's own content
    // AND mods installed outside Pit Box. "Base content" only means the first,
    // hence the explicit removal of the second.
    case "base":
      return (c) => c.is_stock && !c.is_unmanaged;
    case "driver":
      return (c) => ctx.hasDriver(c.id_interne);
    default:
      return () => false;
  }
}

/** Every distinct value of a `val` filter, with how many mods carry it.
 *
 * Counted on the current kind, never on the filtered results: a number that
 * moves with each token dropped is useless for deciding on the next one. */
export function countValues(key: string, cards: ModCard[], ctx: FilterContext): Map<string, number> {
  const get = valuesOf(key, ctx);
  const m = new Map<string, number>();
  for (const c of cards) for (const v of get(c)) m.set(v, (m.get(v) ?? 0) + 1);
  return m;
}

/** One suggestion line of the editor. */
export interface FilterOption {
  value: string;
  label: string;
  count: number;
}

export function optionsOf(def: FilterDef, cards: ModCard[], ctx: FilterContext): FilterOption[] {
  const counts = countValues(def.key, cards, ctx);
  if (def.choices) {
    return def.choices.map((ch) => ({
      value: ch.value,
      label: ch.labelKey ? t(ch.labelKey) : ch.value,
      count: counts.get(ch.value) ?? 0,
    }));
  }
  return [...counts.entries()]
    .map(([value, count]) => ({ value, label: value, count }))
    .sort((a, b) => a.label.toLowerCase().localeCompare(b.label.toLowerCase()));
}

/** Free-text terms: one per word, AND between them, each a plain "contains".
 * Reported bug: "GT-M Evo" did not return "GT-M Adonis Evo", searched as one
 * glued substring. */
function terms(text: string): string[] {
  return text.toLowerCase().split(/\s+/).filter(Boolean);
}

/**
 * Compiles the active filters into ONE predicate.
 *
 * Everything that can be resolved per filter (splitting signs, lowercasing,
 * picking the accessor) is done here, once, and never per card: this runs over
 * the whole library on every keystroke of any field.
 */
export function buildPredicate(
  defs: FilterDef[],
  filters: FilterMap,
  ctx: FilterContext,
): (c: ModCard) => boolean {
  const tests: ((c: ModCard) => boolean)[] = [];
  const lc = (s: string) => s.toLowerCase();

  for (const def of defs) {
    const st = filters[def.key];
    if (!st || st.type !== def.type) continue;

    if (st.type === "val") {
      const inc = st.values.filter((v) => v.sign > 0).map((v) => lc(v.value));
      const exc = st.values.filter((v) => v.sign < 0).map((v) => lc(v.value));
      if (!inc.length && !exc.length) continue;
      const get = valuesOf(def.key, ctx);
      // The operator governs the INCLUSIONS only. Exclusions are always
      // conjunctive - "except A or except B" means nothing, one wants both
      // gone - and excluding always wins over including, which is what a
      // "except" is for: "with jdm, without wip" must not let through a mod
      // carrying both.
      const all = def.operator && st.op === "and";
      tests.push((c) => {
        const mine = get(c).map(lc);
        if (exc.some((x) => mine.includes(x))) return false;
        if (!inc.length) return true;
        return all ? inc.every((x) => mine.includes(x)) : inc.some((x) => mine.includes(x));
      });
    } else if (st.type === "bool") {
      const get = boolOf(def.key, ctx);
      const want = st.sign > 0;
      tests.push((c) => get(c) === want);
    } else if (st.type === "range") {
      const { min, max } = st;
      if (min == null && max == null) continue;
      tests.push((c) => {
        if (min != null && (c.year ?? 0) < min) return false;
        if (max != null && (c.year ?? 9999) > max) return false;
        return true;
      });
    } else {
      const words = terms(st.text);
      if (!words.length) continue;
      tests.push((c) => {
        const hay = ctx.descOf(c);
        return !!hay && words.every((w) => hay.includes(w));
      });
    }
  }

  if (!tests.length) return () => true;
  return (c) => tests.every((f) => f(c));
}

/** A filter present in the map but saying nothing yet - an editor opened and
 * closed without a value. It renders as a GHOST chip, exactly like a pinned
 * filter that has never been touched. */
export function isBlank(st: FilterState): boolean {
  switch (st.type) {
    case "val":
      return st.values.length === 0;
    case "range":
      return st.min == null && st.max == null;
    case "text":
      return st.text.trim() === "";
    case "bool":
      return false;
  }
}

/** The state a filter takes when posed from the add menu. */
export function blankState(def: FilterDef): FilterState {
  switch (def.type) {
    case "val":
      return { type: "val", values: [], op: "and" };
    case "range":
      return { type: "range", min: null, max: null };
    case "text":
      return { type: "text", text: "" };
    case "bool":
      return { type: "bool", sign: def.defaultSign ?? 1 };
  }
}

/** Label of one value, for the chip summary and the posed tokens. Only a
 * fixed vocabulary is translated - a brand, a tag or an author never is. */
export function valueLabel(def: FilterDef, value: string): string {
  const choice = def.choices?.find((c) => c.value === value);
  return choice?.labelKey ? t(choice.labelKey) : value;
}

/** What a chip shows to the right of its label. Structured rather than a
 * single string because inclusions and exclusions are not styled the same,
 * and because the `+N` overflow must still be readable in full by a screen
 * reader (`ariaSummary` below). */
export interface ChipSummary {
  /** `val` only. */
  inc: string[];
  incMore: number;
  exc: string[];
  excMore: number;
  /** Operator pill - set only when it would actually change the result, that
   * is with at least two inclusions. With one value AND and OR agree and the
   * pill would be noise. */
  op?: Operator;
  /** range / text / bool: the whole summary in one string. */
  plain?: string;
  /** `bool` in its negative state, and `val` exclusions: said by the WORD
   * ("except", "Everything but…"), never by a colour of its own - amber stays
   * reserved for real warnings. */
  negative?: boolean;
}

const CAP = 2;

export function chipSummary(def: FilterDef, st: FilterState): ChipSummary {
  const empty: ChipSummary = { inc: [], incMore: 0, exc: [], excMore: 0 };
  if (st.type === "val") {
    const labels = (sign: Sign) => st.values.filter((v) => v.sign === sign).map((v) => valueLabel(def, v.value));
    const inc = labels(1);
    const exc = labels(-1);
    return {
      inc: inc.slice(0, CAP),
      incMore: Math.max(0, inc.length - CAP),
      exc: exc.slice(0, CAP),
      excMore: Math.max(0, exc.length - CAP),
      op: def.operator && inc.length > 1 ? st.op : undefined,
    };
  }
  if (st.type === "range") {
    const dash = "…";
    return { ...empty, plain: `${st.min ?? dash} – ${st.max ?? dash}` };
  }
  if (st.type === "text") return { ...empty, plain: st.text };
  return { ...empty, plain: "", negative: st.sign < 0 };
}

/** The complete summary in words, uncapped: the `aria-label` of the chip, and
 * its tooltip. A `+2` must not hide values a screen reader cannot reach. */
export function ariaSummary(def: FilterDef, st: FilterState): string {
  const label = t(def.labelKey);
  if (st.type === "bool") return st.sign > 0 ? label : t(def.negLabelKey ?? def.labelKey);
  if (st.type === "range") return `${label} : ${st.min ?? "…"} – ${st.max ?? "…"}`;
  if (st.type === "text") return `${label} : ${st.text}`;
  const inc = st.values.filter((v) => v.sign > 0).map((v) => valueLabel(def, v.value));
  const exc = st.values.filter((v) => v.sign < 0).map((v) => valueLabel(def, v.value));
  const parts: string[] = [];
  if (inc.length) parts.push(inc.join(def.operator && st.op === "and" ? ` ${t("filters.opAnd")} ` : ", "));
  if (exc.length) parts.push(`${t("filters.except")} ${exc.join(", ")}`);
  return `${label} : ${parts.join(" · ")}`;
}

/** Decade shortcuts of the year editor, derived from the library rather than
 * hard-coded: a collection that starts in 1930 must be offered 1930, and one
 * that stops in 1999 has no use for a "2010 +" button. */
export function decadePresets(years: number[]): { label: string; min: number; max: number }[] {
  const known = years.filter((y) => y > 0);
  if (!known.length) return [];
  const from = Math.floor(Math.min(...known) / 10) * 10;
  const to = Math.floor(Math.max(...known) / 10) * 10;
  const out: { label: string; min: number; max: number }[] = [];
  for (let d = from; d <= to; d += 10) out.push({ label: `${d}s`, min: d, max: d + 9 });
  return out;
}

// --- Persistence (§6.2) --------------------------------------------------
//
// Values live in `ui_prefs.json` alongside the view mode and the sort, keyed
// per kind. They survive a restart, deliberately: coming back to a library one
// had filtered is what the chips row is for - it says at a glance what is
// applied, which is precisely what the old eleven-control bar did not.

interface Snapshot {
  query: string;
  filters: FilterMap;
}

export function serializeFilters(query: string, filters: FilterMap): string {
  return JSON.stringify({ query, filters } satisfies Snapshot);
}

/** Keeps only what the current catalogue knows about, and only if the stored
 * shape still matches the declared type: a filter that changed type between
 * two versions is dropped rather than half-restored. */
function sanitize(raw: unknown, defs: FilterDef[]): FilterMap {
  const out: FilterMap = {};
  if (!raw || typeof raw !== "object") return out;
  for (const def of defs) {
    const st = (raw as Record<string, unknown>)[def.key] as FilterState | undefined;
    if (!st || typeof st !== "object" || st.type !== def.type) continue;
    if (st.type === "val" && Array.isArray(st.values)) {
      out[def.key] = {
        type: "val",
        values: st.values.filter((v) => v && typeof v.value === "string" && (v.sign === 1 || v.sign === -1)),
        op: st.op === "or" ? "or" : "and",
      };
    } else if (st.type === "range") {
      out[def.key] = { type: "range", min: numOrNull(st.min), max: numOrNull(st.max) };
    } else if (st.type === "text" && typeof st.text === "string") {
      out[def.key] = { type: "text", text: st.text };
    } else if (st.type === "bool") {
      out[def.key] = { type: "bool", sign: st.sign === -1 ? -1 : 1 };
    }
  }
  return out;
}

const numOrNull = (v: unknown): number | null => (typeof v === "number" && Number.isFinite(v) && v !== 0 ? v : null);

/**
 * Reads back the saved filters, from whichever of the three shapes they were
 * written in. Nothing is thrown away: every older form maps exactly onto the
 * new one, so a library left filtered before an update opens filtered after
 * it.
 *
 * Shape 3 (current) carries `filters`; shapes 1 and 2 are the flat snapshot of
 * the eleven-control bar, itself already holding two generations (a
 * comma-separated tag string and single-value selects, then include/exclude
 * tokens and tri-states).
 */
export function parseFilters(raw: string | null, defs: FilterDef[]): Snapshot {
  const fallback: Snapshot = { query: "", filters: {} };
  if (!raw) return fallback;
  let obj: Record<string, unknown>;
  try {
    obj = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return fallback;
  }
  const query = typeof obj.query === "string" ? obj.query : "";
  const stored = obj.filters && typeof obj.filters === "object" ? (obj.filters as FilterMap) : legacyFilters(obj);
  return { query, filters: sanitize(foldBaseIntoState(stored), defs) };
}

/**
 * Replays a stored "base content" filter as a value of `state`.
 *
 * The checkbox is gone - `state = stock` says exactly the same thing - but a
 * preference that carried it must not evaporate: `sanitize` drops keys the
 * catalogue no longer knows, so without this a saved "everything but the base
 * content" would come back as no filter at all, silently. It folds rather than
 * overwrites, because a preference can legitimately hold BOTH (the old
 * `<select>` said active/inactive while the checkbox said base content).
 */
function foldBaseIntoState(stored: FilterMap): FilterMap {
  const base = stored.base;
  if (!base || base.type !== "bool") return stored;
  const { base: _dropped, ...rest } = stored;
  const current = rest.state;
  const values = current?.type === "val" ? [...current.values] : [];
  if (!values.some((v) => v.value === "stock")) values.push({ value: "stock", sign: base.sign });
  return { ...rest, state: { type: "val", values, op: current?.type === "val" ? current.op : "and" } };
}

/** Token of the removed include/exclude field, as it was persisted. */
interface LegacyToken {
  value: string;
  mode: "inc" | "exc";
}

function legacyValues(raw: unknown, olderSelect?: unknown, olderCsv?: unknown): SignedValue[] {
  if (Array.isArray(raw)) {
    return (raw as LegacyToken[])
      .filter((tk) => tk && typeof tk.value === "string")
      .map((tk) => ({ value: tk.value, sign: tk.mode === "exc" ? -1 : 1 }));
  }
  // Generation 1: a single-valued `<select>`, `"all"` meaning no filter.
  if (typeof olderSelect === "string" && olderSelect !== "" && olderSelect !== "all") {
    return [{ value: olderSelect, sign: 1 }];
  }
  // Generation 1 for tags: a comma-separated string, all of them included.
  if (typeof olderCsv === "string") {
    return olderCsv
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
      .map((value) => ({ value, sign: 1 as Sign }));
  }
  return [];
}

function legacyTri(raw: unknown, older?: boolean, olderSign: Sign = 1): FilterState | undefined {
  const v = raw === 1 || raw === -1 ? raw : older ? olderSign : 0;
  return v === 0 ? undefined : { type: "bool", sign: v };
}

function legacyFilters(sf: Record<string, unknown>): Record<string, FilterState> {
  const out: Record<string, FilterState> = {};
  const putVal = (key: string, values: SignedValue[], op: Operator = "and") => {
    if (values.length) out[key] = { type: "val", values, op };
  };
  putVal("category", legacyValues(sf.catTokens, sf.category));
  putVal("brand", legacyValues(sf.brandTokens, sf.brand));
  putVal("author", legacyValues(sf.authorTokens, sf.author));
  putVal("country", legacyValues(sf.countryTokens, sf.country));
  putVal("tag", legacyValues(sf.tagTokens, undefined, sf.tag), sf.tagMode === "or" ? "or" : "and");
  if (typeof sf.class === "string" && sf.class !== "all") {
    putVal("carClass", [{ value: sf.class, sign: 1 }]);
  }
  // The old `<select>` only knew active/inactive, and its "active" also
  // returned base content (see STATE_CHOICES). Restoring it as the new,
  // narrower "active" is the correction, not a loss: it is what the user was
  // asking for and did not get.
  if (sf.state === "active" || sf.state === "inactive") {
    putVal("state", [{ value: sf.state, sign: 1 }]);
  }
  if (typeof sf.desc === "string" && sf.desc.trim()) out.description = { type: "text", text: sf.desc };
  const fav = legacyTri(sf.favState, !!sf.fav);
  if (fav) out.favorite = fav;
  const driver = legacyTri(sf.driverState);
  if (driver) out.driver = driver;
  // The sense flipped: the old checkbox was "never tried" (ticked = never),
  // the tri-state is "already tried". A ticked saved preference is therefore
  // the NEGATIVE state.
  const tried = legacyTri(sf.triedState, !!sf.neverTried, -1);
  if (tried) out.tried = tried;
  // Left under its old key on purpose: `foldBaseIntoState` folds it into
  // `state` right after, and it does so for BOTH the legacy shapes and the
  // short-lived one where it was a chip of its own - one place, one rule.
  const base = legacyTri(sf.stockState, !!sf.hideBaseContent, -1);
  if (base) out.base = base;
  // A bound saved before "empty" existed carried the range's own edge as its
  // "no bound" sentinel; `numOrNull` drops a 0 the same way. Neither ever
  // filtered anything, and neither must start to.
  const min = numOrNull(sf.yearMin) === 1950 ? null : numOrNull(sf.yearMin);
  const max = numOrNull(sf.yearMax) === new Date().getFullYear() ? null : numOrNull(sf.yearMax);
  if (min != null || max != null) out.year = { type: "range", min, max };
  return out;
}

/** Pinned filters, per kind. Unknown keys are dropped (a catalogue may have
 * lost a filter), and an absent preference falls back on the factory set. */
export function parsePinned(raw: string | null, defs: FilterDef[]): string[] {
  if (!raw) return defaultPinned().filter((k) => defs.some((d) => d.key === k));
  try {
    const list = JSON.parse(raw) as unknown;
    if (!Array.isArray(list)) throw new Error("not a list");
    return list.filter((k): k is string => typeof k === "string" && defs.some((d) => d.key === k));
  } catch {
    return defaultPinned().filter((k) => defs.some((d) => d.key === k));
  }
}
