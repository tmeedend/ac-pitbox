// What the session column's navigation list shows, and what it commands (lot 5
// §1).
//
// **Why a store, like `gridMods` and `playerHandicap`.** The session type used
// to be a segmented control inside the settings screen; it is now the
// navigation of the session column, which is on screen at all times, while the
// settings screen (`Launch.svelte`) is mounted only while it is open. The list
// has to be able to show — and change — the type from anywhere, so the type
// cannot live in a component that may not be there.
//
// **One direction only.** This store is the live value: the settings screen
// copies it into its own `setup` and never writes back except on a load (its
// own mount, a preset, a saved session), which is the same rule
// `playerHandicap` follows. Two effects facing each other wake each other up.
//
// The summary line is here for the same reason: it has to read right before
// the settings screen has ever been opened, which is the very case it exists
// for — one must be able to see how many opponents a session carries without
// going to look.

import { invoke } from "@tauri-apps/api/core";
import type { SessionType } from "./launch";

/** Which page of the settings screen the navigation points at. `opponents` is
 * a sub-entry of the type, never a type of its own. */
export type SessionPage = "setup" | "opponents";

/** The four types, in the order the list shows them: what the application can
 * do, never folded behind a picker (§1.1). */
export const SESSION_TYPES: SessionType[] = ["practice", "hotlap", "race", "trackday"];

/** Only these two field a grid of opponents. */
export function hasOpponents(type: SessionType): boolean {
  return type === "race" || type === "trackday";
}

export const sessionNav = $state<{
  type: SessionType;
  page: SessionPage;
  /** Number of opponents on the grid, and the difficulty band the draw spreads
   * over — the `6 AI · 87% ± 3` of the sub-entry (§1.2). */
  count: number;
  center: number;
  spread: number;
  /** An alert living on the opponents page (thin pool, duplicate drivers).
   * Raised here because an alert on a page one is not looking at is worth no
   * more than no alert at all (§1.3). */
  alert: boolean;
}>({
  type: "practice",
  page: "setup",
  count: 0,
  center: 95,
  spread: 3,
  alert: false,
});

/** Whether the user has already picked a type by hand. The seeding below must
 * not overwrite a deliberate pick with what was on disk: the two races each
 * other only during the few milliseconds after startup, but losing that race
 * would send the session screen to the wrong type without a word. */
let picked = false;

/**
 * Seeds the list from `launch_state.json` at startup, exactly as `gridMods`
 * does for the activation guard, and for the same reason: the column shows the
 * type and its summary before the settings screen has ever been mounted.
 *
 * Never an error — an unreadable file simply leaves the defaults, which is
 * what the column showed before the list existed.
 */
async function loadSessionNav(): Promise<void> {
  try {
    const state = await invoke<{
      selection: { session_type?: SessionType; opponents?: unknown[] } | null;
      presets: Record<string, { ai_level?: number; ai_spread?: number }> | null;
    }>("get_launch_state");
    const type = state?.selection?.session_type;
    if (!picked && type && SESSION_TYPES.includes(type)) sessionNav.type = type;
    sessionNav.count = state?.selection?.opponents?.length ?? 0;
    const preset = state?.presets?.[sessionNav.type];
    if (preset?.ai_level != null) sessionNav.center = preset.ai_level;
    if (preset?.ai_spread != null) sessionNav.spread = preset.ai_spread;
  } catch {
    /* nothing to say */
  }
}

/**
 * Picks a type from the list.
 *
 * Landing on the settings page of that type, always: `Opponents` is a sub-entry
 * of the type one is on, so leaving Race for Practice cannot keep pointing at a
 * page Practice does not have (§1.4). What the grid holds is untouched — the
 * type decides what is shown and what is sent to the game, never what is
 * remembered.
 */
export function pickSessionType(type: SessionType): void {
  picked = true;
  sessionNav.type = type;
  sessionNav.page = "setup";
}

/** The sub-entry. Only ever reachable under a type that fields a grid. */
export function openOpponentsPage(): void {
  if (hasOpponents(sessionNav.type)) sessionNav.page = "opponents";
}

/** Back to the settings of the current type — what clicking the parent entry
 * does from the opponents page. */
export function openSetupPage(): void {
  sessionNav.page = "setup";
}

/**
 * Hydration, started once at module load like `nav`'s own.
 *
 * Exported so the settings screen can **await** it before reading the type:
 * it reads the same file, and the two of them racing would have it start on
 * the persisted type while the user had already picked another from the
 * column.
 */
export const sessionNavReady: Promise<void> = loadSessionNav();
