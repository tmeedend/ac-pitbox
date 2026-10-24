// The player's own preferences in Story Mode, whatever the story: kept in
// `ui_prefs.json` (golden rule 6), never in a save - they belong to the
// player, not to a game.
import { StorageKey } from "$lib/storage";
import { peekUiPref, setUiPref } from "$lib/uiPrefs.svelte";
import { AUTO_SPEEDS, type AutoSpeed } from "./player/stage";

/** The auto-advance speed the player picked, `normal` until they pick one. */
export function autoSpeed(): AutoSpeed {
  const kept = peekUiPref(StorageKey.storyAutoSpeed);
  return AUTO_SPEEDS.includes(kept as AutoSpeed) ? (kept as AutoSpeed) : "normal";
}

export function setAutoSpeed(speed: AutoSpeed): void {
  setUiPref(StorageKey.storyAutoSpeed, speed);
}

/** The scenes' ambient sound is muted: the player said so. */
export function ambientMuted(): boolean {
  return peekUiPref(StorageKey.storyAmbientMuted) === "1";
}

export function setAmbientMuted(muted: boolean): void {
  setUiPref(StorageKey.storyAmbientMuted, muted ? "1" : "0");
}
