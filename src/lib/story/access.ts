// Whether the Story entry exists on this install.
//
// Story Mode is a long-running work in progress, built on `main` behind a
// switch rather than on a branch (`CLAUDE.md`, « Chantiers longs derrière un
// drapeau »). Always on under `tauri dev`; in a built installer, on only once
// Pit Box has been launched with `--enable-story-studio`. This is not a
// lock — the repository is public and anyone can turn it on — only a way to
// keep an unfinished screen out of users' way. The story itself is not in the
// build.
import { invokeSafe } from "$lib/invokeSafe";
import { peekUiPref, removeUiPref, setUiPref } from "$lib/uiPrefs.svelte";
import { StorageKey } from "$lib/storage";

/** Synchronous and reactive: read by the rail's `when`, which redraws once
 * `ui_prefs.json` has loaded. */
export function storyUnlocked(): boolean {
  return import.meta.env.DEV || peekUiPref(StorageKey.storyStudio) === "1";
}

/** Records the switch asked for on this launch's command line, if any. The
 * backend only reads the arguments: `ui_prefs.json`'s schema belongs here. */
export async function applyStudioSwitch(): Promise<void> {
  const wanted = await invokeSafe<boolean | null>("story_studio_switch", undefined, null);
  if (wanted === true) setUiPref(StorageKey.storyStudio, "1");
  else if (wanted === false) removeUiPref(StorageKey.storyStudio);
}
