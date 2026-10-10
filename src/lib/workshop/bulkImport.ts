// Bulk import summary (§4.2): what the analysis found, and how many items the
// execution will actually process. Pure, so the counting rules are testable.

import type { BulkEntry, BulkExecItem, BulkStatus } from "$lib/library/library";

export type BulkCounts = Record<BulkStatus | "ignored", number>;

export function bulkCounts(entries: BulkEntry[]): BulkCounts {
  const c: BulkCounts = { new: 0, update: 0, duplicate: 0, ambiguous: 0, rehydrate: 0, ignored: 0 };
  for (const e of entries) {
    if (e.ignored) c.ignored++;
    for (const m of e.mods) c[m.status]++;
  }
  return c;
}

/** Number of mods the execution will import (the button's count). */
export function importCount(entries: BulkEntry[], skipDuplicates: boolean): number {
  let n = 0;
  for (const e of entries) {
    if (e.ignored) continue;
    for (const m of e.mods) {
      if (m.status === "duplicate" && skipDuplicates) continue;
      n++;
    }
  }
  return n;
}

export function buildExecItems(
  entries: BulkEntry[],
  skipDuplicates: boolean,
  decisions: Record<string, "keep_both" | "replace">,
): BulkExecItem[] {
  return entries
    .filter((e) => !e.ignored)
    .map((e) => ({
      path: e.path,
      skip_ids: skipDuplicates ? e.mods.filter((m) => m.status === "duplicate").map((m) => m.id) : [],
      replace_ids: e.mods.filter((m) => m.status === "ambiguous" && decisions[m.id] === "replace").map((m) => m.id),
    }));
}
