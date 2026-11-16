// The history's state across its screens (RESULTAT§7): the filter, the open
// outing and the switch, kept while one goes into a session and comes back.
import { nav, requestSection } from "$lib/shell/nav.svelte";
import type { Combo } from "./results";

export const historyState = $state<{
  /** The only filter: one car on one layout (RESULTAT§7.5). */
  combo: Combo | null;
  /** The outing open, by its key. */
  outing: string | null;
  /** Sessions without a valid lap shown. */
  showEmpty: boolean;
}>({ combo: null, outing: null, showEmpty: false });

/** Opens the history, filtered on a car and a layout or not. */
export function openHistory(combo: Combo | null = null): Promise<boolean> {
  historyState.combo = combo;
  historyState.outing = null;
  return requestSection("history");
}

/** A session of the history, on the screen of a session (RESULTAT§7.4). */
export function openSession(sessionId: number): Promise<boolean> {
  nav.resultLaunch = null;
  nav.resultSession = sessionId;
  return requestSection("result");
}

/** The history's own outing view gone back to its list. */
export function closeOuting(): void {
  historyState.outing = null;
}
