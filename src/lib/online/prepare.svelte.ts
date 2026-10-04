// "Prepare & join": fetching what a server needs before joining it
// (SPEC-play-online.md, "Contenu manquant : prêt à rejoindre").
//
// Nothing new is downloaded or imported here: each source goes through what
// the library already uses — the archive kept at import comes back as a mod
// in the showcase does (`reinstallFromArchive`), a link or the registry
// through the download-and-import of a mod update (`downloadAndImport`),
// whose progress and cancel show in the update toast. The content then lands
// in the library as a managed mod, never straight into `content/` — the
// reason not to let Content Manager's own "Install missing content" do it.
//
// A server may need a dozen mods: the run is followed as one, in a single
// notification (`PrepareToast`) — which server it prepares, which mod of how
// many, and the current one's download or import — the way a batch import
// shows its overall bar above the current archive (reported: each mod came
// and went in a toast of its own, with no idea of how far the whole was).
import { invoke } from "@tauri-apps/api/core";
import { canStartUpdate, downloadAndImport, type DownloadOutcome } from "$lib/library/modUpdates.svelte";
import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";
import { reinstallFromArchive } from "$lib/workshop/maintenance";
import type { ModKind } from "$lib/library/library";
import type { Fetch } from "./online";

export interface Needed {
  kind: ModKind;
  id: string;
  name: string;
  fetch: Fetch;
}

export type PrepareOutcome =
  | { status: "ready" }
  /** A source gave a page, not an archive: it is open in the browser. */
  | { status: "browser"; name: string; url: string }
  | { status: "busy" }
  | { status: "cancelled" };

/** The run under way: for which server, and where it is. */
export interface PrepareRun {
  server: string;
  total: number;
  /** 0-based index of the item being fetched. */
  index: number;
  name: string;
}

const state = $state<{ run: PrepareRun | null; stopping: boolean }>({ run: null, stopping: false });

/** Whether « Stop » was asked: the current mod's download or import is
 * cancelled, and the run goes no further. */
export function prepareStopping(): boolean {
  return state.stopping;
}

/** Stops the run: the current download or import is cancelled by the caller
 * (the notification knows which is under way), the next mods are not
 * fetched. */
export function stopPrepare(): void {
  if (state.run) state.stopping = true;
}

/** The preparation under way, if any. Reactive. */
export function prepareRun(): PrepareRun | null {
  return state.run;
}

/** Fetches each item from its first source, one after the other — the
 * import admits one run. Stops at the first page that needs a person.
 * `server`: the name the notification shows. */
export async function fetchNeeded(items: Needed[], server: string): Promise<PrepareOutcome> {
  if (!canStartUpdate()) return { status: "busy" };
  state.stopping = false;
  try {
    for (const [index, item] of items.entries()) {
      if (state.stopping) return { status: "cancelled" };
      state.run = { server, total: items.length, index, name: item.name };
      if (item.fetch.kept_archive) {
        await reinstallFromArchive(item.id);
        continue;
      }
      const update = {
        kind: item.kind,
        id: item.id,
        name: item.name,
        installed: item.fetch.installed_version,
        available: item.fetch.server_version ?? "",
        limited: false,
      };
      const url = item.fetch.server_url;
      const done = url
        ? await downloadAndImport(update, () => invoke<DownloadOutcome>("download_online_content", { url, id: item.id }))
        : await downloadAndImport(update);
      if (done.status === "browser") return { status: "browser", name: item.name, url: done.url };
      if (done.status === "cancelled") return { status: "cancelled" };
    }
    return state.stopping ? { status: "cancelled" } : { status: "ready" };
  } finally {
    state.run = null;
    state.stopping = false;
    bumpLibraryVersion();
  }
}
