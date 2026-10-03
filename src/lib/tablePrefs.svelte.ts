// Where the table views keep their columns (SPEC §7.4): `library_columns.json`
// of `app_config_dir`, written on the Rust side, one key per screen (`cars`,
// `tracks`, `online`) — golden rule 6: never `localStorage`.
//
// The file is read once and kept here; a save changes one screen in that copy
// and writes the whole of it. Reading it again before each write let two saves
// in a row (a column shown, then resized) each start from the same old file,
// the second writing back over the first. The write never fails in silence:
// retried once, then shown by `PrefsToast` (`durableWriter`).
import { invoke } from "@tauri-apps/api/core";
import { durableWriter } from "$lib/durableWrite.svelte";
import { invokeSafe } from "$lib/invokeSafe";
import type { ColumnsPrefs, TableScreen } from "$lib/tableColumns";

type AllPrefs = Partial<Record<TableScreen, Partial<ColumnsPrefs>>>;

let all: Promise<AllPrefs> | null = null;

const writer = durableWriter<AllPrefs>("save_library_columns", (prefs) =>
  invoke<void>("save_library_columns", { prefs }),
);

function loadAll(): Promise<AllPrefs> {
  // A read that fails gives no saved columns — the defaults — rather than a
  // screen stuck waiting.
  all ??= invokeSafe<AllPrefs>("get_library_columns", undefined, {}).then((prefs) => prefs ?? {});
  return all;
}

/** The saved prefs of a screen, untouched: `undefined` when it has none yet. */
export async function loadSavedPrefs(screen: TableScreen): Promise<Partial<ColumnsPrefs> | undefined> {
  return (await loadAll())[screen];
}

export async function saveTablePrefs(screen: TableScreen, prefs: ColumnsPrefs): Promise<void> {
  const current = await loadAll();
  current[screen] = prefs;
  writer.save({ ...current });
}

export function tablePrefsWriteFailure() {
  return writer.failure;
}
