// Recovering the files of several mods in the showcase at once (ESPACE§7.2).
//
// One mod at a time, never during an import: each download ends in the
// ordinary import, which admits one run, and which is what recognizes the
// version and brings its files back (ESPACE§7.3). For each mod, the source
// that needs nothing from the user comes first:
//
//   1. the source archive kept at import — one click, offline;
//   2. Content Manager's registry, when it serves the archive itself.
//
// Anything else is **listed, never opened**: a registry that points to a
// page (a file host, a Patreon post) would otherwise open one browser tab per
// mod, and the other sources — pack page, author's page, searches — are on
// the mod's fiche, one click away. Nothing downloads without the click that
// started the lot.
//
// Module-level state for the reason the lots have theirs: it starts from the
// library's context menu, which has no component to host it, and its report
// lives in the notification stack.
import { errorText } from "$lib/errors";
import { reinstallFromArchive } from "$lib/workshop/maintenance";
import type { ModKind } from "./library";
import { bumpLibraryVersion } from "./libraryVersion.svelte";
import { cancelUpdateDownload, downloadAndImport, modUpdateDetails, modUpdates } from "./modUpdates.svelte";
import { showcasePlan, showcaseSources } from "./showcase.svelte";

export interface RecoveryTarget {
  id_interne: string;
  kind: ModKind;
  display_name: string | null;
}

interface Named {
  id: string;
  kind: ModKind;
  name: string;
}

/** What became of one mod. `page`: the registry offers a page, not an
 * archive. `manual`: no source Pit Box can use on its own — its fiche lists
 * the others. */
export type RecoveryResult = Named &
  (
    | { status: "recovered" }
    | { status: "page"; url: string }
    | { status: "manual" }
    | { status: "failed"; error: string }
  );

export const recovery = $state<{
  running: boolean;
  cancelling: boolean;
  index: number;
  total: number;
  current: string | null;
  /** The last lot's report, until closed. */
  results: RecoveryResult[] | null;
}>({
  running: false,
  cancelling: false,
  index: 0,
  total: 0,
  current: null,
  results: null,
});

/** Stops the lot after the mod in hand — and that mod's download, if any. */
export function cancelRecovery(): void {
  recovery.cancelling = true;
  if (modUpdates.busy) cancelUpdateDownload();
}

export function dismissRecovery(): void {
  recovery.results = null;
}

/** Recovers the files of each mod, one after the other. The caller checks
 * `canStartUpdate()` first: an import running now would refuse the first
 * download. */
export async function recoverMods(targets: RecoveryTarget[]): Promise<void> {
  if (recovery.running || !targets.length) return;
  recovery.running = true;
  recovery.cancelling = false;
  recovery.total = targets.length;
  recovery.results = null;
  const results: RecoveryResult[] = [];
  try {
    for (const [i, t] of targets.entries()) {
      if (recovery.cancelling) break;
      const named: Named = { id: t.id_interne, kind: t.kind, name: t.display_name ?? t.id_interne };
      recovery.index = i + 1;
      recovery.current = named.name;
      results.push(await recoverOne(named));
    }
  } finally {
    recovery.results = results;
    recovery.running = false;
    recovery.current = null;
    bumpLibraryVersion();
  }
}

async function recoverOne(m: Named): Promise<RecoveryResult> {
  try {
    const sources = await showcaseSources(m.id);
    if (sources.kept_archive) {
      await reinstallFromArchive(m.id);
      return { ...m, status: "recovered" };
    }
    // Not in the registry is the common case, not an error.
    const registry = await modUpdateDetails(m).catch(() => null);
    if (!registry) return { ...m, status: "manual" };
    const done = await downloadAndImport({
      kind: m.kind,
      id: m.id,
      name: m.name,
      installed: null,
      available: registry.version ?? "",
      limited: false,
    });
    if (done.status === "browser") return { ...m, status: "page", url: done.url };
    if (done.status === "cancelled") {
      recovery.cancelling = true;
      return { ...m, status: "manual" };
    }
    // An import that stopped on a question has recovered nothing yet: only
    // the mod itself can say whether it left the showcase.
    const [after] = await showcasePlan([m.id]);
    return after?.showcase ? { ...m, status: "manual" } : { ...m, status: "recovered" };
  } catch (e) {
    return { ...m, status: "failed", error: errorText(e) };
  }
}
