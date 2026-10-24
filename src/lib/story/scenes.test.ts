import { describe, expect, it } from "vitest";
import type { Chapter } from "./studio";
import {
  PERIODS,
  SEASONS,
  newPlace,
  sceneFiles,
  setCriterion,
  soundFiles,
  storyNames,
  toggled,
  variantIds,
  type ScenesFile,
} from "./scenes";

const scenes: ScenesFile = {
  scenes: [
    {
      id: "garage",
      variants: [
        { id: "rain", weather: ["rain"], mood: ["tense"], file: "g.webp" },
        { id: "day", file: "d.webp" },
      ],
    },
    { id: "bedroom", variants: [{ id: "default", file: "b.webp", mood: ["calm"] }] },
  ],
};

describe("storyNames", () => {
  // The weathers and moods a Beat or the preview offers: the ones the
  // variants ask for and the ones the Beats set, `none` aside.
  it("gathers the names the variants ask for and the Beats set", () => {
    const chapters: Chapter[] = [
      {
        id: "c",
        nodes: [
          { id: "a", beat: { weather: "fog", mood: "none" } },
          { id: "b", beat: { mood: "tense" } },
        ],
      },
    ];
    expect(storyNames(scenes, chapters, "weather")).toEqual(["fog", "rain"]);
    expect(storyNames(scenes, [...chapters, null], "mood")).toEqual(["calm", "tense"]);
    expect(storyNames(null, [], "mood"), "a story with no scenes").toEqual([]);
  });
});

describe("variantIds", () => {
  // A Beat forces a variant of the place it sets; setting none, of the
  // current place, which only playing knows: every variant is offered.
  it("lists the place's variants, or every place's without one", () => {
    expect(variantIds(scenes, "garage")).toEqual(["rain", "day"]);
    expect(variantIds(scenes, "attic"), "an unknown place").toEqual([]);
    expect(variantIds(scenes, undefined)).toEqual(["rain", "day", "default"]);
  });
});

describe("criteria", () => {
  // A criterion's boxes keep the order of the seasons or the times of day,
  // and an empty criterion is not written: it holds always.
  it("toggles in order and leaves an empty criterion out", () => {
    expect(toggled(["winter"], "spring", true, SEASONS)).toEqual(["winter", "spring"]);
    expect(toggled(["night", "morning"], "night", false, PERIODS)).toEqual(["morning"]);
    const v = { id: "x", file: "x.webp", time: PERIODS };
    setCriterion(v, "time", []);
    expect("time" in v, "an empty criterion is removed").toBe(false);
    setCriterion(v, "season", ["summer"]);
    expect(v).toMatchObject({ season: ["summer"] });
  });
});

describe("media", () => {
  it("offers images and videos for a variant, sounds for an ambience", () => {
    const media = ["scenes/a.webp", "scenes/b.MP4", "scenes/c.ogg", "notes.txt", "p/d.png"];
    expect(sceneFiles(media)).toEqual(["scenes/a.webp", "p/d.png", "scenes/b.MP4"]);
    expect(soundFiles(media)).toEqual(["scenes/c.ogg"]);
  });

  it("starts a place sharp, with one variant to fill", () => {
    expect(newPlace("attic", "Attic")).toEqual({
      id: "attic",
      name: "Attic",
      blur: false,
      variants: [{ id: "default", file: "" }],
    });
  });
});
