// The showcase (ESPACE§): deleting a car or a track keeps it, by default, in
// the library without its heavy files — "en vitrine".
//
// Lives in the library domain, at module level, for the reason `bulkState`
// does: deleting starts from the fiche AND from the library's context menu,
// which has no component to host a dialog or a result. The confirmation is a
// request posted here and answered by `DeleteDialog`, mounted once in the
// shell; the progress and the report go through the ordinary lot machinery.
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "$lib/errors";
import { fmtSize } from "$lib/format";
import { t } from "$lib/i18n/index.svelte";
import { cardImage } from "$lib/preferred";
import { StorageKey } from "$lib/storage";
import { deleteBrokenMod } from "$lib/workshop/maintenance";
import { bulkDelete, type BulkFailure, type BulkReport } from "./bulkEdit";
import { bulkState, runBulkOp } from "./bulkState.svelte";
import type { ModCard } from "./library";

/** What deleting needs to know of a mod: a card, or a context menu target. */
export type DeleteTarget = Pick<ModCard, "id_interne" | "kind" | "preview" | "showcase">;

/** Mirrors `showcase::PlanEntry`: what the confirmation says about a mod. */
export interface PlanEntry {
  id: string;
  name: string;
  /** Already in the showcase: a complete deletion is all that is left. */
  showcase: boolean;
  active: boolean;
  versions: number;
  /** Versions, attached content, additions and resources together. */
  size_bytes: number;
  /** Layers, skins and sounds that lose their files with it (ESPACE§5.4). */
  attached: AttachedEntry[];
  kept_archive: boolean;
  source_file_name: string | null;
  source_site: string | null;
}

/** Mirrors `showcase::AttachedEntry`: a layer, skin or sound of a mod, which
 * comes back only with its own archive (ESPACE§7.5). */
export interface AttachedEntry {
  kind: "layer" | "skin" | "sound";
  name: string;
  archive: string | null;
  size_bytes: number;
}

/** Mirrors `showcase::Sources`: where the files could come back from. */
export interface RecoverySources {
  kept_archive: boolean;
  page_url: string | null;
  author_url: string | null;
  source_site: string | null;
  file_name: string | null;
  /** Its layers, skins and sounds in the showcase, with their archives. */
  attached: AttachedEntry[];
}

interface ShowcaseOutcome {
  mod_id: string;
  freed_bytes: number;
  recycled: boolean;
  was_active: boolean;
}

interface ShowcaseReport {
  done: ShowcaseOutcome[];
  failed: BulkFailure[];
  cancelled: boolean;
}

/** What the lot report says of a showcase lot, on top of the plain counts:
 * the notification speaks in bytes and names, and offers the way back to a
 * complete deletion (ESPACE§5.2). Frontend only — `bulk_showcase` returns a
 * `ShowcaseReport`, turned into a `BulkReport` here. */
export interface ShowcaseSummary {
  freedBytes: number;
  /** False when the recycle bin refused and the files were deleted for good
   * (ESPACE R8): said after the fact, as it was said before. */
  recycled: boolean;
  /** Name of the only mod, when there was one. */
  name: string | null;
}

/** The one predicate that keeps a mod in the showcase out of a session
 * (ESPACE§9.3): the session column's pickers, the opponents' pool. */
export function isPlayable(c: { showcase: boolean }): boolean {
  return !c.showcase;
}

export function showcasePlan(ids: string[]): Promise<PlanEntry[]> {
  return invoke<PlanEntry[]>("showcase_plan", { ids });
}

export function showcaseSources(id: string): Promise<RecoverySources> {
  return invoke<RecoverySources>("showcase_sources", { id });
}

// --- The confirmation ---------------------------------------------------------

export interface DeleteChoice {
  mode: "showcase" | "complete";
  /** Keep the source archive kept at import (ESPACE§5.2). */
  keepArchive: boolean;
}

interface DeleteRequest {
  entries: PlanEntry[];
  resolve: (choice: DeleteChoice | null) => void;
}

export const deleteDialog = $state<{ request: DeleteRequest | null }>({ request: null });

/** Posts the confirmation and waits for the answer; `null` is "Cancel". */
function askDelete(entries: PlanEntry[]): Promise<DeleteChoice | null> {
  return new Promise((resolve) => {
    deleteDialog.request = {
      entries,
      resolve: (choice) => {
        deleteDialog.request = null;
        resolve(choice);
      },
    };
  });
}

/** `ui_prefs.json` key of the "How it works" panel's "Do not show again". */
export const HOW_IT_WORKS_KEY = StorageKey.showcaseHowItWorksHidden;

// --- Deleting -----------------------------------------------------------------

/** The image each card shows right now, as the showcase freezes it
 * (ESPACE§3.3). */
function imagesOf(mods: DeleteTarget[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const c of mods) {
    const img = cardImage(c);
    if (img && !c.showcase) out[c.id_interne] = img;
  }
  return out;
}

/** A showcase lot's report, in the shape the lot notification reads. */
function toBulkReport(r: ShowcaseReport, names: Map<string, string>): BulkReport {
  const summary: ShowcaseSummary = {
    freedBytes: r.done.reduce((n, o) => n + o.freed_bytes, 0),
    recycled: r.done.every((o) => o.recycled),
    name: r.done.length === 1 ? names.get(r.done[0].mod_id) ?? r.done[0].mod_id : null,
  };
  return {
    ok: r.done.map((o) => o.mod_id),
    failed: r.failed.map((f) => ({ id: f.id, error: errorText(f.error) })),
    skipped: [],
    cancelled: r.cancelled,
    showcase: summary,
  };
}

/**
 * "Delete…" from the fiche or the library (ESPACE§5.1): asks, then puts the
 * mods in the showcase or deletes them completely. Resolves to what was
 * chosen — a fiche stays open on a mod put in the showcase, and closes on
 * one deleted — or `null` when nothing was done. Errors are the caller's.
 */
export async function deleteMods(mods: DeleteTarget[]): Promise<DeleteChoice["mode"] | null> {
  if (!mods.length || bulkState.running) return null;
  const entries = await showcasePlan(mods.map((c) => c.id_interne));
  const choice = await askDelete(entries);
  if (!choice) return null;
  if (choice.mode === "complete") {
    const report = await deleteCompletely(entries.map((e) => e.id));
    // Said only of what happened: a lot that could not start (another one
    // began during the confirmation) or a deletion that failed leaves the
    // mod where it is, and its fiche open — the report says why.
    return report?.ok.length ? "complete" : null;
  }
  // Those already in the showcase have nothing left to free.
  const ids = entries.filter((e) => !e.showcase).map((e) => e.id);
  const names = new Map(entries.map((e) => [e.id, e.name]));
  const images = imagesOf(mods);
  const report = await runBulkOp("showcase", ids.length, async () =>
    toBulkReport(
      await invoke<ShowcaseReport>("bulk_showcase", { ids, cardImages: images, keepArchive: choice.keepArchive }),
      names,
    ),
  );
  return report?.ok.length ? "showcase" : null;
}

/** The complete deletion — the one before the showcase existed — for one
 * mod or a lot. Also what the showcase notification offers afterwards. */
export async function deleteCompletely(ids: string[]): Promise<BulkReport | null> {
  if (ids.length === 1) {
    return runBulkOp("delete", 1, async () => {
      try {
        await deleteBrokenMod(ids[0]);
        return { ok: ids, failed: [], skipped: [], cancelled: false };
      } catch (e) {
        return { ok: [], failed: [{ id: ids[0], error: errorText(e) }], skipped: [], cancelled: false };
      }
    });
  }
  return runBulkOp("delete", ids.length, () => bulkDelete(ids));
}

/** "Delete completely" from the notification that follows a showcase lot. */
export async function deleteShowcasedCompletely(ids: string[]): Promise<void> {
  const ok = await askDelete(await showcasePlan(ids));
  if (ok?.mode === "complete") await deleteCompletely(ids);
}

/** "550 MB freed", as the confirmation and the notification say it. */
export function freedLine(bytes: number): string {
  return t("showcase.freed", { size: fmtSize(bytes) });
}
