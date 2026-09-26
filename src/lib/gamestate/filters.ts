// The filters of the game folder screen (DOSSIER§6), in the library's own chip
// model (`$lib/library/filters`): the screen reuses its filter bar rather than
// a copy, so it speaks the same `FilterMap`. This module translates between
// the two, and holds the two rules the bar does not know: Provenance takes a
// single element, and a summary count toggles its State value.
import type { FilterChoice, FilterDef, FilterMap, SignedValue } from "$lib/library/filters";
import type { Filters, OwnerKind, OwnerRef, StateValue } from "./gamestate";

export const STATE_KEY = "state";
export const PROVENANCE_KEY = "provenance";

/** In the order of the summary band (DOSSIER§3.3). */
export const STATE_VALUES: readonly StateValue[] = [
  "posed",
  "replacesGame",
  "waiting",
  "cmZone",
  "drift",
  "nobody",
  "shared",
];

const OWNER_KINDS: readonly OwnerKind[] = ["car", "track", "app", "skin", "trackSkin", "sound", "other", "pack"];

/** A Provenance value: an owner, as a chip stores it. Stable across scans,
 * where the owner's index number is not. */
export function ownerValue(o: OwnerRef): string {
  return `${o.kind}:${o.id}`;
}

export function parseOwnerValue(v: string): OwnerRef | null {
  const at = v.indexOf(":");
  if (at < 0) return null;
  const kind = v.slice(0, at) as OwnerKind;
  if (!OWNER_KINDS.includes(kind)) return null;
  return { kind, id: v.slice(at + 1) };
}

/** The catalogue of the screen: State, then Provenance. Population is not a
 * filter in the first lot (DOSSIER§6.3). */
export function gameFolderDefs(ownerLabel: (value: string) => string): FilterDef[] {
  const choices: FilterChoice[] = STATE_VALUES.map((v) => ({ value: v, labelKey: `gamefolder.state.${v}` }));
  return [
    { key: STATE_KEY, labelKey: "gamefolder.filterState", type: "val", choices },
    { key: PROVENANCE_KEY, labelKey: "gamefolder.filterProvenance", type: "val", labelOf: ownerLabel },
  ];
}

function values(map: FilterMap, key: string): SignedValue[] {
  const st = map[key];
  return st?.type === "val" ? st.values : [];
}

/** What the backend is asked: included states (OR), excluded states, and one
 * owner - the last one posed with a plus. */
export function toBackendFilters(map: FilterMap): Filters {
  const states = values(map, STATE_KEY);
  const pick = (sign: 1 | -1) =>
    states
      .filter((v) => v.sign === sign)
      .map((v) => v.value as StateValue)
      .filter((v) => STATE_VALUES.includes(v));
  const owners = values(map, PROVENANCE_KEY).filter((v) => v.sign === 1);
  const last = owners[owners.length - 1];
  return {
    states: pick(1),
    excluded: pick(-1),
    owner: last ? parseOwnerValue(last.value) : null,
  };
}

/** Provenance names ONE element (DOSSIER§6.2): the bar lets a second value be
 * added like any other, and the newest one replaces the previous. A minus has
 * no meaning here - "everything but this mod" is the whole folder. Returns the
 * same map when there is nothing to change, so that an effect calling it does
 * not loop. */
export function normalizeProvenance(map: FilterMap): FilterMap {
  const st = map[PROVENANCE_KEY];
  if (st?.type !== "val") return map;
  const positive = st.values.filter((v) => v.sign === 1);
  const last = positive[positive.length - 1];
  if (st.values.length === 1 && last) return map;
  const next = { ...map };
  if (last) next[PROVENANCE_KEY] = { ...st, values: [last] };
  else delete next[PROVENANCE_KEY];
  return next;
}

/** Poses Provenance on `owner` ("Filter on this mod", a way in). */
export function withOwner(map: FilterMap, owner: OwnerRef): FilterMap {
  return { ...map, [PROVENANCE_KEY]: { type: "val", values: [{ value: ownerValue(owner), sign: 1 }], op: "or" } };
}

/** A count of the summary band IS the shortcut of its chip (DOSSIER§3.3): a
 * click poses the value, a second one removes it. */
export function toggleState(map: FilterMap, value: StateValue): FilterMap {
  const current = values(map, STATE_KEY);
  const on = current.some((v) => v.value === value && v.sign === 1);
  const rest = current.filter((v) => v.value !== value);
  const nextValues: SignedValue[] = on ? rest : [...rest, { value, sign: 1 }];
  const next = { ...map };
  if (nextValues.length) next[STATE_KEY] = { type: "val", values: nextValues, op: "or" };
  else delete next[STATE_KEY];
  return next;
}

/** Is `value` posed with a plus? - what lights a count of the summary band. */
export function stateOn(map: FilterMap, value: StateValue): boolean {
  return values(map, STATE_KEY).some((v) => v.value === value && v.sign === 1);
}

/** A stable key for "the same filters": what the tree's cache is keyed on. */
export function filtersKey(f: Filters): string {
  return JSON.stringify([f.states, f.excluded, f.owner ? ownerValue(f.owner) : null]);
}
