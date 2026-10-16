import { describe, expect, it } from "vitest";
import shipped from "../../../src-tauri/rules/default-tag-rules.json";
import { TRACK_CATEGORY_ICONS } from "./trackCategoryIcons";
import { bareCategory, categoryIconId, categoryName, categoryTag, tagForName } from "./trackCategoryNames";

const tr = (key: string) => `T(${key})`;

describe("track category names", () => {
  // Families › Tracks: the tag is the identity, the name is display only.
  it("names a category by the user's name, then the app's, then its tag", () => {
    expect(categoryName("#drift", { "#drift": { name: "Glisse" } }, tr)).toBe("Glisse");
    expect(categoryName("#Drift", {}, tr), "a shipped category is translated").toBe("T(trackCategories.drift)");
    expect(categoryName("#gymkhana", {}, tr), "one this build does not know").toBe("gymkhana");
  });

  it("draws a category with the user's icon, then its own, then the circuit", () => {
    expect(categoryIconId("#drift", { "#drift": { icon: "touge" } })).toBe("touge");
    expect(categoryIconId("drift", {})).toBe("drift");
    expect(categoryIconId("#gymkhana", {})).toBe("circuit");
  });

  it("writes a category one way", () => {
    expect(categoryTag(" Drift ")).toBe("#drift");
    expect(categoryTag("##drift")).toBe("#drift");
    expect(bareCategory("#Touge")).toBe("touge");
    expect(categoryTag("  ")).toBe("");
  });

  // The tag of a new category is fixed at creation: what tracks store.
  it("makes a new tag from a name, unique", () => {
    expect(tagForName("Côte d'Azur", [])).toBe("#cote_d_azur");
    expect(tagForName("Drift", ["#drift"])).toBe("#drift_2");
    expect(tagForName("!!!", [])).toBe("#category");
  });

  // One pictogram per shipped category: a category added to the catalogue
  // without one would fall back on the circuit, and nobody would notice.
  it("ships a pictogram for every category of the rules' allowlist", () => {
    const allowlist = shipped.track.category_allowlist.map(bareCategory);
    expect(allowlist.length).toBeGreaterThan(10);
    for (const c of allowlist) expect(TRACK_CATEGORY_ICONS[c], c).toBeDefined();
  });
});
