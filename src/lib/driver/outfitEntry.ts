// The per-car driver entry of `ui_prefs.json`, read and written
// (SESSION§5 for the cascade it feeds).
//
// Pure on purpose: `driverOverride.svelte.ts` holds runes, which the Vitest
// config cannot compile, and this is the part whose edge cases only show by
// running them.

/** The four pieces. `null` = what the car or its livery decides. */
export interface DriverOutfit {
  /** Body substituted for the car's. Named in `data.acd`, which a race
   * server checks — hence laid in the game through CSP, never in the car. */
  body: string | null;
  suit: string | null;
  gloves: string | null;
  helmet: string | null;
}

export const EMPTY_OUTFIT: DriverOutfit = { body: null, suit: null, gloves: null, helmet: null };

/** True when nothing is chosen — the car and its livery decide everything. */
export function isEmpty(outfit: DriverOutfit): boolean {
  return !outfit.body && !outfit.suit && !outfit.gloves && !outfit.helmet;
}

/**
 * What a stored entry says about this car: the outfit chosen for it, or
 * `null` when nothing is — the default outfit of its class then applies.
 *
 * An entry whose four pieces are empty means "nothing chosen" **unless** it
 * carries `origin: true`: then the user chose, on this very car, its own body
 * and its livery's outfit, and the default outfit must keep off it.
 */
export function parseEntry(raw: string | null): DriverOutfit | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<DriverOutfit> & { origin?: unknown };
    const outfit: DriverOutfit = {
      body: parsed.body ?? null,
      suit: parsed.suit ?? null,
      gloves: parsed.gloves ?? null,
      helmet: parsed.helmet ?? null,
    };
    if (!isEmpty(outfit)) return outfit;
    return parsed.origin === true ? EMPTY_OUTFIT : null;
  } catch {
    return null;
  }
}

/**
 * The entry to store for a choice, or `null` to remove it.
 *
 * An empty choice is **removed** when nothing would replace it — that is the
 * difference between "set" and "not set", which the library filter reads.
 * But when a default outfit would dress the car, removing it hands the car to
 * that default: real bug, "back to the car's body" brought Mr Bean straight
 * back on every street car, and did nothing at all on one never set. The
 * empty choice is then kept, marked.
 */
export function entryFor(outfit: DriverOutfit, defaultWouldDress: boolean): string | null {
  if (!isEmpty(outfit)) return JSON.stringify(outfit);
  return defaultWouldDress ? JSON.stringify({ ...EMPTY_OUTFIT, origin: true }) : null;
}
