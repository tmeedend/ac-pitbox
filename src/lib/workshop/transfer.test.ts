import { describe, expect, it } from "vitest";
import { effectiveParts, exportFileName, initialImportParts, mergeToShow, partAllowed, type Part } from "./transfer";

describe("the parts of a library transfer (EXPORT§3)", () => {
  it("never carries profiles without the library they name", () => {
    expect(partAllowed("profiles", ["profiles", "preferences"])).toBe(false);
    expect(effectiveParts(["profiles", "preferences"])).toEqual(["preferences"]);
  });

  it("keeps the parts in their own order, whatever the order checked", () => {
    expect(effectiveParts(["preferences", "profiles", "library"])).toEqual(["library", "profiles", "preferences"]);
  });

  it("proposes a file named after the day", () => {
    expect(exportFileName(new Date(2026, 9, 9))).toBe("Pit Box - 2026-10-09.pitbox");
  });

  it("says what the merge will do as soon as anything has two sides (EXPORT§7.4)", () => {
    const plan = {
      merging: false,
      added: 3,
      kept: 0,
      blocked: [],
      attached_skipped: 0,
      filled: 0,
      notes_joined: 0,
      conflicts: {},
    };
    expect(mergeToShow(plan)).toBe(false);
    expect(mergeToShow({ ...plan, notes_joined: 1 })).toBe(true);
    expect(mergeToShow({ ...plan, conflicts: { names: 2 } })).toBe(true);
    expect(mergeToShow({ ...plan, merging: true })).toBe(true);
  });

  it("leaves Preferences unchecked when merging, which would replace this machine's (EXPORT§7.3)", () => {
    const all: Part[] = ["library", "classification", "preferences"];
    expect(initialImportParts(all, true)).toEqual(["library", "classification"]);
    expect(initialImportParts(all, false)).toEqual(all);
  });
});
