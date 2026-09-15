// Cars of the current opponent grid, for the activation guard of the session
// column (SESSION§3).
//
// **Why a store at all.** The launch button lives in the session column, which
// is always on screen; the grid lives in `Launch.svelte`, which is mounted only
// while the settings screen is open. A guard that only knew the grid while that
// screen was up would let a session start, from anywhere else, with opponents
// Assetto Corsa cannot load — which is exactly the hole the guard exists to
// close.
//
// So the list is kept here, written by the settings screen while it is up, and
// seeded from disk at startup from what that screen persisted last time. Only
// the car ids: what matters to the guard is which mods have to be activated,
// and their names are read from their own detail, not cached here.

import { invoke } from "@tauri-apps/api/core";

export const gridMods = $state<{ carIds: string[] }>({ carIds: [] });

/** Called by the settings screen whenever the grid changes. */
export function setGridCars(ids: string[]): void {
  const unique = [...new Set(ids)];
  // Same list, same order: rewriting it would wake every reader for nothing.
  if (unique.length === gridMods.carIds.length && unique.every((id, i) => gridMods.carIds[i] === id)) return;
  gridMods.carIds = unique;
}

/**
 * Seeds the list from `launch_state.json` at startup.
 *
 * The grid is stored in the **selection**, not in the per-type presets: it is
 * the one the user last had in front of them, whatever the session type, which
 * is also the one the launch button would send. Never an error — a guard that
 * could not read the file simply has nothing to say, and the launch behaves as
 * it did before it existed.
 */
export async function loadGridCars(): Promise<void> {
  try {
    const state = await invoke<{ selection: { opponents?: { car_id: string }[] } | null }>("get_launch_state");
    setGridCars((state?.selection?.opponents ?? []).map((o) => o.car_id));
  } catch {
    /* nothing to say */
  }
}
