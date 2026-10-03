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

/** Fetches each item from its first source, one after the other — the
 * import admits one run. Stops at the first page that needs a person. */
export async function fetchNeeded(items: Needed[]): Promise<PrepareOutcome> {
  if (!canStartUpdate()) return { status: "busy" };
  try {
    for (const item of items) {
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
    return { status: "ready" };
  } finally {
    bumpLibraryVersion();
  }
}
