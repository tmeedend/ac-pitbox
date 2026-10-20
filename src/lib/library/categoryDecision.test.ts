// What a typed text points at in the tag field (REGLES§3 level 4, the manual
// tag field offers to make it a category).
import { describe, expect, it } from "vitest";
import { matchCategories } from "./categoryDecision";

const CHOICES = ["#gt1", "#gt3", "#gt4", "#gte-gt3", "#lmp1", "#sportscars"];

describe("matchCategories", () => {
  it("offers the categories whose name starts with the text, then those that contain it", () => {
    expect(matchCategories("gt", CHOICES)).toEqual(["#gt1", "#gt3", "#gt4", "#gte-gt3"]);
    expect(matchCategories("cars", CHOICES)).toEqual(["#sportscars"]);
    expect(matchCategories("t3", CHOICES)).toEqual(["#gt3", "#gte-gt3"]);
  });

  it("ignores case, a leading # and spaces", () => {
    expect(matchCategories("  #GT3 ", CHOICES)).toEqual(["#gt3", "#gte-gt3"]);
  });

  it("offers nothing for an empty text or an unknown one", () => {
    expect(matchCategories("  ", CHOICES)).toEqual([]);
    expect(matchCategories("#", CHOICES)).toEqual([]);
    expect(matchCategories("zzz", CHOICES)).toEqual([]);
  });

  it("keeps a handful of lines: the field is a suggestion, not a list", () => {
    const many = Array.from({ length: 12 }, (_, i) => `#gt${i}`);
    expect(matchCategories("gt", many)).toHaveLength(5);
    expect(matchCategories("gt", many, 3)).toHaveLength(3);
  });
});
