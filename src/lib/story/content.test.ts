import { describe, expect, it } from "vitest";
import { contentIds, findContent, newContent, placementOf, setPlacement, setType } from "./content";
import type { ContentFile, ContentHeader, EditedFile } from "./studio";

describe("placement", () => {
  // The engine reads the placement from which keys are present: switching
  // must leave exactly those of the new one.
  it("is told by the header's keys, and switching keeps only the new one's", () => {
    const h: ContentHeader = { id: "a", type: "mail", when: ["reputation >= 1"], delay: "2 days", priority: 1 };
    expect(placementOf(h)).toBe("reactive");
    setPlacement(h, "pool");
    expect(h).toEqual({ id: "a", type: "mail", pool: "", when: ["reputation >= 1"] });
    expect(placementOf(h)).toBe("pool");
    setPlacement(h, "flow");
    expect(h).toEqual({ id: "a", type: "mail" });
    setPlacement(h, "reactive");
    expect(h.when, "`when: []` is what makes it reactive").toEqual([]);
  });

  it("a type other than dialogue drops the characters present and the scene", () => {
    const h: ContentHeader = { id: "a", type: "dialogue", characters: ["friend"], scene: "garage" };
    setType(h, "message");
    expect(h).toEqual({ id: "a", type: "message" });
  });

  it("a type other than illustration drops the picture", () => {
    const h: ContentHeader = { id: "a", type: "illustration", picture: "big/moment.png" };
    setType(h, "illustration");
    expect(h.picture).toBe("big/moment.png");
    setType(h, "dialogue");
    expect(h).toEqual({ id: "a", type: "dialogue" });
  });

  it("a type played elsewhere than on the scene drops the transition", () => {
    const h: ContentHeader = { id: "a", type: "illustration", transition: "cut" };
    setType(h, "dialogue");
    expect(h.transition).toBe("cut");
    setType(h, "mail");
    expect(h).toEqual({ id: "a", type: "mail" });
  });
});

describe("finding an item", () => {
  const files: EditedFile<ContentFile>[] = [
    { file: "content/a.story", stamp: "", error: null, value: { items: [newContent("one"), newContent("two")] } },
    { file: "content/broken.story", stamp: "", error: "oops", value: null },
  ];

  it("gives its file and index, and every id of the files that read", () => {
    expect(findContent(files, "two")).toEqual({ file: "content/a.story", index: 1 });
    expect(findContent(files, "none")).toBeNull();
    expect(contentIds(files)).toEqual(["one", "two"]);
  });
});
