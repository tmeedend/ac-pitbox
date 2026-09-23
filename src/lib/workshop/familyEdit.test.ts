import { describe, expect, it } from "vitest";
import { addFamily, isCurated, moveTag, patchFamily, restoreFamily, tagCounts } from "./familyEdit";

const TABLE = [
  { id: "prototype", icon: "proto", tags: ["lmp1", "group c"] },
  { id: "race", icon: "race", tags: ["race", "gt3"] },
];

describe("family table editing", () => {
  // TAXO§7.3: a tag belongs to one family - attached elsewhere, it moves.
  it("moves a tag instead of copying it", () => {
    const out = moveTag(TABLE, "#LMP1", "race");
    expect(out[0].tags).toEqual(["group c"]);
    expect(out[1].tags).toEqual(["race", "gt3", "lmp1"]);
  });

  it("does not duplicate a tag moved into the family that holds it", () => {
    expect(moveTag(TABLE, "GT3", "race")[1].tags).toEqual(["race", "gt3"]);
  });

  // The id is what a posed chip stores: a rename must not break it.
  it("gives a new family a unique id taken from its name, fixed from then on", () => {
    const { families, id } = addFamily(TABLE, "Course");
    expect(id).toBe("course");
    expect(addFamily(families, "Course").id).toBe("course-2");
    expect(addFamily(TABLE, "Épreuve d'endurance").id).toBe("epreuve-d-endurance");
    const renamed = patchFamily(families, "course", { name: "Le Mans" });
    expect(renamed.at(-1)).toMatchObject({ id: "course", name: "Le Mans" });
  });

  it("falls back on the translation when the name is emptied", () => {
    expect(patchFamily(TABLE, "race", { name: "  " })[1]).not.toHaveProperty("name");
  });

  // TAXO§6.1: the flag of the list says "you changed this".
  it("flags a family whose tags differ from the shipped ones, not one merely reordered", () => {
    const shipped = TABLE[1];
    expect(isCurated({ ...shipped, tags: ["gt3", "#Race"] }, shipped)).toBe(false);
    expect(isCurated({ ...shipped, tags: ["gt3"] }, shipped)).toBe(true);
    expect(isCurated({ ...shipped, icon: "rally" }, shipped)).toBe(true);
    expect(isCurated({ id: "mine", tags: [] }, undefined), "a family the user made").toBe(true);
  });

  it("restores a family by taking its tags back from where they were moved", () => {
    const moved = moveTag(patchFamily(TABLE, "prototype", { icon: "rally" }), "lmp1", "race");
    const back = restoreFamily(moved, TABLE, "prototype");
    expect(back[0]).toEqual({ id: "prototype", icon: "proto", tags: ["lmp1", "group c"] });
    expect(back[1].tags, "no longer in the family it was moved to").toEqual(["race", "gt3"]);
  });

  // Found while testing the tab: restoring the family a tag had been moved
  // INTO used to drop that tag from every family.
  it("sends a tag the family had gained back to the family that ships it", () => {
    const moved = moveTag(TABLE, "gt3", "prototype");
    const back = restoreFamily(moved, TABLE, "prototype");
    expect(back[0].tags).toEqual(["lmp1", "group c"]);
    expect(back[1].tags, "gt3 is home in Race again").toContain("gt3");
    const invented = moveTag(TABLE, "wip", "prototype");
    expect(restoreFamily(invented, TABLE, "prototype").flatMap((f) => f.tags), "a tag nobody ships is dropped").not.toContain("wip");
  });

  it("counts a car once per tag, whatever the spelling it carries it under", () => {
    const counts = tagCounts([["#GT3", "gt3", "race"], ["gt3"]]);
    expect(counts.get("gt3")).toBe(2);
    expect(counts.get("race")).toBe(1);
  });
});
