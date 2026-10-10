// Bulk import summary (§4.2): the counts shown before execution, and the
// number on the import button, must match what the execution will process.
import { describe, expect, it } from "vitest";
import type { BulkEntry, BulkMod, BulkStatus } from "$lib/library/library";
import { buildExecItems, bulkCounts, importCount } from "./bulkImport";

function mod(id: string, status: BulkStatus): BulkMod {
  return { id, kind: "Car", name: null, status, existing_id: null, existing_name: null };
}

function entry(subfolder: string, mods: BulkMod[], ignored = false): BulkEntry {
  return { subfolder, path: `C:\\src\\${subfolder}`, ignored, mods };
}

describe("bulk import summary", () => {
  it("counts each mod by status and each empty folder as ignored", () => {
    const entries = [
      entry("pack", [mod("a", "new"), mod("b", "duplicate")]),
      entry("upd", [mod("c", "update")]),
      entry("empty", [], true),
    ];
    expect(bulkCounts(entries)).toEqual({
      new: 1,
      update: 1,
      duplicate: 1,
      ambiguous: 0,
      rehydrate: 0,
      other: 0,
      ignored: 1,
    });
  });

  // Real bug: a folder of sixteen driver bodies (`<name>/content/driver/<name>.kn5`)
  // showed "0 new" and an "Import 0 mod(s)" button that could not be clicked,
  // although the execution files each folder as an "other mod" (§7.3).
  it("counts a non-empty folder with no car or track as one other mod to import", () => {
    const entries = [entry("senna", []), entry("goku", []), entry("empty", [], true)];
    expect(bulkCounts(entries).other).toBe(2);
    expect(bulkCounts(entries).ignored).toBe(1);
    expect(importCount(entries, true)).toBe(2);
    expect(buildExecItems(entries, true, {}).map((i) => i.path)).toEqual(["C:\\src\\senna", "C:\\src\\goku"]);
  });

  it("leaves duplicates out of the import count only when they are skipped", () => {
    const entries = [entry("pack", [mod("a", "new"), mod("b", "duplicate")])];
    expect(importCount(entries, true)).toBe(1);
    expect(importCount(entries, false)).toBe(2);
  });

  it("sends every non-ignored folder, with its skipped and replaced ids", () => {
    const entries = [
      entry("pack", [mod("a", "duplicate"), mod("b", "ambiguous"), mod("c", "ambiguous")]),
      entry("empty", [], true),
    ];
    expect(buildExecItems(entries, true, { b: "replace", c: "keep_both" })).toEqual([
      { path: "C:\\src\\pack", skip_ids: ["a"], replace_ids: ["b"] },
    ]);
  });
});
