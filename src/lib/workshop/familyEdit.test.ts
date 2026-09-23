import { describe, expect, it } from "vitest";
import {
  attachTag,
  createFamily,
  deleteFamily,
  detachTag,
  isCurated,
  restoreFamily,
  setFamilyLook,
  tagCounts,
  tagOrigins,
} from "./familyEdit";

const CATALOG = [
  { id: "prototype", icon: "proto", tags: ["lmp1", "group c"] },
  { id: "race", icon: "race", tags: ["race", "gt3"] },
];

describe("family decisions on top of the catalogue", () => {
  // REGLES§2: the overlay holds decisions, keyed by tag.
  it("records a moved tag as one decision, and drops it once sent back home", () => {
    const moved = attachTag({}, CATALOG, "#GT3", "prototype");
    expect(moved.tags).toEqual({ gt3: "prototype" });
    expect(attachTag(moved, CATALOG, "gt3", "race").tags, "back on the catalogue").toEqual({});
  });

  it("detaches a shipped tag with a tombstone, and an unknown one with nothing", () => {
    expect(detachTag({}, CATALOG, "gt3").tags).toEqual({ gt3: "" });
    expect(detachTag({ tags: { wec: "race" } }, CATALOG, "wec").tags).toEqual({});
  });

  it("keeps only the look that differs from the catalogue", () => {
    expect(setFamilyLook({}, CATALOG, "race", { icon: "race" }).meta).toEqual({});
    expect(setFamilyLook({}, CATALOG, "race", { name: "Course", icon: "rally" }).meta).toEqual({
      race: { name: "Course", icon: "rally" },
    });
    const renamed = setFamilyLook({ meta: { race: { name: "Course" } } }, CATALOG, "race", { name: " " });
    expect(renamed.meta, "an emptied name falls back on the translation").toEqual({});
  });

  // The id is what a posed chip stores: a rename must not break it.
  it("gives a new family a unique id taken from its name, fixed from then on", () => {
    const { overlay, id } = createFamily({}, ["race"], "Épreuve d'endurance");
    expect(id).toBe("epreuve-d-endurance");
    expect(createFamily(overlay, [], "Épreuve d'endurance").id).toBe("epreuve-d-endurance-2");
    expect(createFamily({}, ["race"], "Race").id).toBe("race-2");
    const renamed = setFamilyLook(overlay, CATALOG, id, { name: "Le Mans" });
    expect(renamed.created).toEqual([{ id, name: "Le Mans", tags: [] }]);
  });

  it("deletes a shipped family with a tombstone, a created one without trace", () => {
    const o = attachTag(createFamily({}, [], "Mine").overlay, CATALOG, "wec", "mine");
    expect(deleteFamily(o, CATALOG, "mine")).toMatchObject({ created: [], tags: {}, removed: [] });
    expect(deleteFamily({}, CATALOG, "race").removed).toEqual(["race"]);
  });

  // The bug of the whole-table version: restoring the family a tag had been
  // moved INTO dropped that tag from every family. Removing the decisions
  // cannot do that.
  it("restores a family by removing every decision touching it", () => {
    let o = attachTag({}, CATALOG, "gt3", "prototype");
    o = detachTag(o, CATALOG, "lmp1");
    o = setFamilyLook(o, CATALOG, "prototype", { icon: "rally" });
    expect(isCurated(o, CATALOG, "prototype")).toBe(true);
    expect(isCurated(o, CATALOG, "race"), "race lost gt3: it is touched too").toBe(true);
    const back = restoreFamily(o, CATALOG, "prototype");
    expect(back).toMatchObject({ meta: {}, tags: {}, removed: [] });
    expect(isCurated(back, CATALOG, "race")).toBe(false);
  });

  // REGLES§8.1: what is shipped and what is the user's must be readable.
  it("tells shipped tags from added ones, and keeps the removed ones in view", () => {
    const o = tagOrigins({ id: "race", tags: ["race", "wec"] }, CATALOG);
    expect(o.tags).toEqual([
      { key: "race", mine: false },
      { key: "wec", mine: true },
    ]);
    expect(o.gone, "gt3 was shipped in Race and is no longer there").toEqual(["gt3"]);
  });

  it("counts a car once per tag, whatever the spelling it carries it under", () => {
    const counts = tagCounts([["#GT3", "gt3", "race"], ["gt3"]]);
    expect(counts.get("gt3")).toBe(2);
    expect(counts.get("race")).toBe(1);
  });
});
