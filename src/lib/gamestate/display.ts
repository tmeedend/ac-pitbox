// What a line of the game folder shows of its state (DOSSIER§4.4): pure rules,
// kept out of the components so that the order of gravity is written once.
import type { Counts, StateValue } from "./gamestate";

/** The order of gravity (DOSSIER§4.4): what a folder shows first. */
export const GRAVITY: readonly StateValue[] = ["drift", "replacesGame", "waiting", "cmZone", "posed"];

export interface Pill {
  value: StateValue;
  /** `null`: the pill says the state, not a number. */
  count: number | null;
}

const countOf = (c: Counts, v: StateValue): number => (v === "cmZone" ? c.cmZone : v === "shared" ? c.shared : c[v]);

/** A folder's pills: at most two, by gravity. A folder holding nothing but
 * laid files shows one green pill without a number; one holding nothing but
 * files nobody laid shows none. */
export function folderPills(c: Counts): Pill[] {
  const present = GRAVITY.filter((v) => countOf(c, v) > 0);
  if (present.length === 1 && present[0] === "posed") return [{ value: "posed", count: null }];
  return present.slice(0, 2).map((v) => ({ value: v, count: countOf(c, v) }));
}
