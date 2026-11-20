// The interior view of a car's sheet (PREVIEW§15.1, FMOD§6octies): one choice
// for the picture and the engine sound together, kept across restarts.
//
// **One state for both**, and not one per screen: the button over the 3D
// preview and the one beside the engine sounds are the same switch, and the
// user asked for the two to stay together — a cockpit camera with the sound
// of a bystander would contradict itself. The preview reads `interiorView()`
// to fly in or out; the sound follows through `switchEngineView`.
import { StorageKey } from "$lib/storage";
import { peekUiPref, setUiPref } from "$lib/uiPrefs.svelte";
import { switchEngineView } from "./enginePlayer.svelte";

/** What the user chose in this run, `null` until they choose: until then the
 * stored value speaks. Kept apart so that a toggle takes effect at once, before
 * the write to `ui_prefs.json` has come back through the cache. */
let chosen = $state<boolean | null>(null);

/** Whether the 3D preview on screen can seat a camera — the car declares where
 * its driver's eyes are. `false` while no preview is up: the sound switch
 * still works without one. */
let seatAvailable = $state(false);

/** Is the sheet in the interior view? Synchronous, for templates and effects
 * alike — an effect that reads it **outside `untrack`** subscribes to every
 * preference through `peekUiPref` (see CLAUDE.md, "Lire une préférence"). */
export function interiorView(): boolean {
  return chosen ?? peekUiPref(StorageKey.interiorView) === "1";
}

export function setInteriorView(on: boolean): void {
  chosen = on;
  setUiPref(StorageKey.interiorView, on ? "1" : "0");
  void switchEngineView(on);
}

export function cockpitSeatAvailable(): boolean {
  return seatAvailable;
}

/** Set by the 3D preview once it knows whether its car can be sat in. */
export function setCockpitSeatAvailable(available: boolean): void {
  seatAvailable = available;
}
