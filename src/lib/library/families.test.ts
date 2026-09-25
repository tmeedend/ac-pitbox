import { describe, expect, it } from "vitest";
import catalog from "../../../src-tauri/rules/taxonomy-catalog.json";
import { familiesOfTags, familyLookup, familyTag, familyTagsOf } from "./families";
import { FAMILY_ICONS } from "./familyIcons";

const FAMILIES = [
  { id: "prototype", tags: ["#LMP1", "group c"] },
  { id: "classic", tags: ["vintage", "lmp1"] },
  { id: "race", tags: ["race"] },
];

describe("families", () => {
  // A road car whose file only says `"class": "street"` is a road car: the
  // class is read with the tags (35 unclassified cars of the first Mod
  // Organizer survey).
  it("reads the class along with the tags", () => {
    const car = { tags_from_mod: ["rwd"], tags_from_rule: [], tags_manual: [], car_class: "street" };
    const fams = [{ id: "street", tags: ["street"] }];
    expect(familiesOfTags(familyTagsOf(car), familyLookup(fams), fams)).toEqual(["street"]);
    expect(familyTagsOf({ ...car, car_class: null })).toEqual(["rwd"]);
  });

  // The same tag reaches a car as `#rally` through a rule and as `rally` from
  // its file: a tile count must not depend on where the tag came from.
  it("compares tags without case and without their leading #", () => {
    expect(familyTag("  #LMP1 ")).toBe("lmp1");
    const lookup = familyLookup(FAMILIES);
    expect(familiesOfTags(["lmp1"], lookup, FAMILIES)).toEqual(["prototype"]);
    expect(familiesOfTags(["#Race", "RACE"], lookup, FAMILIES)).toEqual(["race"]);
  });

  // TAXO§7.3: one tag, one family - a hand-edited table listing it
  // twice keeps it in the first.
  it("keeps a tag listed twice in the first family only", () => {
    expect(familiesOfTags(["lmp1"], familyLookup(FAMILIES), FAMILIES)).toEqual(["prototype"]);
  });

  // INDEX§6.1: a car belongs to as many families as its tags reach.
  it("gives a car every family its tags reach, in the order of the table", () => {
    const lookup = familyLookup(FAMILIES);
    expect(familiesOfTags(["race", "vintage", "#lmp1"], lookup, FAMILIES)).toEqual(["prototype", "classic", "race"]);
    expect(familiesOfTags(["traffic"], lookup, FAMILIES)).toEqual([]);
  });
});

describe("the shipped family table", () => {
  const shipped = catalog.families;

  // Otherwise the counters of the index stop meaning anything: a tag counted
  // in two families would be read as two different cars' worth of overlap.
  it("never lists one tag in two families", () => {
    const seen = new Map<string, string>();
    for (const f of shipped) {
      for (const tag of f.tags) {
        const key = familyTag(tag);
        expect(seen.get(key), `"${tag}" is in ${seen.get(key)} and ${f.id}`).toBeUndefined();
        seen.set(key, f.id);
      }
    }
  });

  it("names only silhouettes the app embeds", () => {
    for (const f of shipped) expect(FAMILY_ICONS[f.icon], `icon of ${f.id}`).toBeDefined();
  });
});
