import { describe, expect, it } from "vitest";
import { effectiveParts, exportFileName, partAllowed } from "./transfer";

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
});
