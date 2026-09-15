// The three shortcut chips of the Opponents block (CIBLE§3.2/CIBLE§3.3).
//
// **The chips are not modes.** They were — `Same car` / `By category` / `Free`
// were three tabs, and a tab did two jobs at once: define a set of cars, and
// generate a grid from that set. Only the second justified it; the first was a
// worse copy of the filter bar, and it is that duplication which forced the
// reconciliation rules ("removing a chip does not change the tab", "the grid
// goes manual"). The symptom, not the cause.
//
// So a chip now POSES A TOKEN AND NOTHING ELSE. It looks active when its token
// is there, clicking it again takes the token away, and removing the token by
// its cross deactivates the chip. There is no second state to keep in sync,
// which is the whole point.
//
// One asymmetry is deliberate and worth stating: `model` and `category` pose a
// VALUE, so they are a snapshot — change the car you drive and the token still
// says `Ferrari 488 GT3`. `performance` poses a TOLERANCE, whose reference is
// read from the context, so it follows the car. That is not an oversight: a
// band called "±15 % of my car" that kept measuring against a car one no
// longer drives would be lying, whereas a category token that stopped saying
// what it says would be the tab behaviour coming back in.
import { PERF_DEFAULT_PCT } from "$lib/detail/carSpecs";
import type { FilterMap } from "$lib/library/filters";
import type { ModCard } from "$lib/library/library";

export type ChipKind = "model" | "category" | "performance";

/**
 * What the block starts from, the first time a session type is configured:
 * nothing at all.
 *
 * **No pinned "playable" token.** One was posed here, excluding disabled and
 * broken mods so a random draw could never build an unplayable grid. It cost a
 * permanent chip in a 600 px bar to prevent something that hardly ever
 * happens — and when it does happen, the activation guard above the launch
 * button says so and repairs it in one click. A warning that appears when the
 * case occurs beats a chip that takes room every time it does not.
 */
export function defaultGridFilters(): FilterMap {
  return {};
}

/** The value a chip poses, or `null` when the reference car cannot supply one
 * (no car chosen, no category on the mod). */
function chipValue(kind: ChipKind, car: ModCard | null): string | null {
  if (kind === "performance") return car ? "" : null;
  if (!car) return null;
  return kind === "model" ? car.display_name ?? car.id_interne : car.category;
}

/** Whether the chip reads as active — that is, whether its token is posed with
 * exactly the value the chip would pose. */
export function isChipOn(filters: FilterMap, kind: ChipKind, car: ModCard | null): boolean {
  const st = filters[kind];
  if (!st) return false;
  if (kind === "performance") return st.type === "perf";
  const value = chipValue(kind, car);
  if (value == null || st.type !== "val") return false;
  return st.values.some((v) => v.sign > 0 && v.value.toLowerCase() === value.toLowerCase());
}

/**
 * Clicking a chip. Returns a NEW map — the caller assigns it, so a `$state`
 * proxy sees one write instead of several.
 *
 * Posing REPLACES whatever that filter held rather than adding to it: "same
 * category as mine" is a statement about which category, so leaving a
 * hand-typed `#gt2` next to it would make the chip mean something it does not
 * say.
 */
export function toggleChip(filters: FilterMap, kind: ChipKind, car: ModCard | null): FilterMap {
  const next = { ...filters };
  if (isChipOn(filters, kind, car)) {
    delete next[kind];
    return next;
  }
  if (kind === "performance") {
    next.performance = { type: "perf", pct: PERF_DEFAULT_PCT };
    return next;
  }
  const value = chipValue(kind, car);
  if (value == null) return filters;
  next[kind] = { type: "val", values: [{ value, sign: 1 }], op: "and" };
  return next;
}

/** Whether a chip can be clicked at all: no car chosen, a car with no category,
 * or — for the band — a car whose own specs are unreadable (CIBLE§3.4). No silent
 * fallback on a random draw: the chip goes dim and says why. */
export function chipAvailable(kind: ChipKind, car: ModCard | null, refRatio: number | null): boolean {
  if (kind === "performance") return refRatio != null;
  return chipValue(kind, car) != null;
}
