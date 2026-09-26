import { describe, expect, it } from "vitest";
import { folderPills } from "./display";
import type { Counts } from "./gamestate";

const counts = (c: Partial<Counts>): Counts => ({
  posed: 0,
  replacesGame: 0,
  waiting: 0,
  drift: 0,
  nobody: 0,
  cmZone: 0,
  shared: 0,
  ...c,
});

describe("folder pills (DOSSIER§4.4)", () => {
  it("shows the two gravest states, with their counts", () => {
    expect(folderPills(counts({ posed: 40, drift: 2, replacesGame: 1, nobody: 9 }))).toEqual([
      { value: "drift", count: 2 },
      { value: "replacesGame", count: 1 },
    ]);
  });

  it("says 'laid' without a number when that is all there is", () => {
    expect(folderPills(counts({ posed: 12, nobody: 3 }))).toEqual([{ value: "posed", count: null }]);
  });

  it("shows nothing for a folder nobody laid anything in", () => {
    expect(folderPills(counts({ nobody: 1240 }))).toEqual([]);
  });

  it("ranks the CM zone between waiting and laid", () => {
    expect(folderPills(counts({ posed: 5, cmZone: 3 }))).toEqual([
      { value: "cmZone", count: 3 },
      { value: "posed", count: 5 },
    ]);
  });
});
