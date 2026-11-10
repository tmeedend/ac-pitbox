// The table of §16.5: what plays on each screen, and above all what does NOT
// change — a chip, a search, a density, a sort.
import { describe, expect, it } from "vitest";
import {
  anchorAfter,
  anchorOnTile,
  anchorPosedIn,
  emptyAssignments,
  resolve,
  sameResolution,
  waitsBeforePlaying,
  screenKeyOf,
  withAssignment,
  withRecent,
  type Assignments,
  type CarFacts,
  type TrackFacts,
} from "./resolve";
import type { FilterMap } from "$lib/library/filters";

const A: Assignments = {
  brand: { Porsche: "D:/music/porsche", Ferrari: "D:/music/ferrari" },
  family: { race: "D:/music/race", classic: "D:/music/classic" },
  track_category: { "#drift": "D:/music/drift", "#street": "D:/music/street" },
  country: { Japan: "D:/music/japan", Italy: "D:/music/italy" },
  screen: { cars: "D:/music/cars-screen", online: "D:/music/online" },
};

const gt3rs: CarFacts = { brand: "Porsche", families: ["race", "sportscars"], country: "Germany" };
const miata: CarFacts = { brand: "Mazda", families: ["street"], country: "Japan" };
const ebisu: TrackFacts = { categories: ["#drift", "#circuit"], country: "Japan" };
const monza: TrackFacts = { categories: ["#circuit"], country: "Italy" };

const chips = (key: string, value: string, sign: 1 | -1 = 1): FilterMap => ({
  [key]: { type: "val", values: [{ value, sign }], op: "and" },
});

describe("library index and list", () => {
  it("the index plays its screen, then MENU", () => {
    expect(resolve({ kind: "index", library: "cars" }, null, A)).toEqual({ assigned: "D:/music/cars-screen", base: "menu" });
    expect(resolve({ kind: "index", library: "tracks" }, null, A)).toEqual({ assigned: null, base: "menu" });
  });

  it("a tile click anchors its term, and the list plays it", () => {
    const anchor = anchorOnTile("cars", "brand", "Porsche");
    const filters = chips("brand", "Porsche");
    const view = { kind: "list" as const, library: "cars" as const, anchorPosed: anchorPosedIn(filters, anchor) };
    expect(resolve(view, anchor, A).assigned).toBe("D:/music/porsche");
  });

  // The rule the whole design rests on: once on the list, chips, search,
  // density and sort change nothing — the list is not reached again, so
  // nothing is resolved again. What the caller does is resolve on ARRIVAL;
  // here, arriving twice with more chips gives the same answer.
  it("more chips after the tile leave the music alone", () => {
    const anchor = anchorOnTile("cars", "brand", "Porsche");
    const more: FilterMap = {
      ...chips("brand", "Porsche"),
      year: { type: "range", min: 1990, max: null },
      family: { type: "val", values: [{ value: "classic", sign: 1 }], op: "and" },
    };
    const view = { kind: "list" as const, library: "cars" as const, anchorPosed: anchorPosedIn(more, anchor) };
    expect(resolve(view, anchor, A).assigned).toBe("D:/music/porsche");
  });

  it("a term without music changes nothing: the screen's music goes on", () => {
    const anchor = anchorOnTile("cars", "brand", "Mazda");
    const view = { kind: "list" as const, library: "cars" as const, anchorPosed: true };
    expect(resolve(view, anchor, A).assigned).toBe("D:/music/cars-screen");
  });

  it("the list reached again without the anchor's chip plays the screen", () => {
    const anchor = anchorOnTile("cars", "brand", "Porsche");
    expect(anchorPosedIn(chips("brand", "Ferrari"), anchor)).toBe(false);
    expect(anchorPosedIn(chips("brand", "Porsche", -1), anchor)).toBe(false);
    const view = { kind: "list" as const, library: "cars" as const, anchorPosed: false };
    expect(resolve(view, anchor, A).assigned).toBe("D:/music/cars-screen");
  });

  it("back to the index clears the anchor of that library only", () => {
    const anchor = anchorOnTile("tracks", "country", "Japan");
    expect(anchorAfter(anchor, { kind: "index", library: "cars" })).toEqual(anchor);
    expect(anchorAfter(anchor, { kind: "screen", key: "online" })).toEqual(anchor);
    expect(anchorAfter(anchor, { kind: "index", library: "tracks" })).toBeNull();
  });

  it("a tile of the other library replaces the anchor", () => {
    anchorOnTile("cars", "brand", "Porsche");
    const next = anchorOnTile("tracks", "track_category", "#drift");
    expect(next).toEqual({ library: "tracks", taxonomy: "track_category", term: "#drift" });
  });
});

describe("sheets", () => {
  it("a car filed under the anchor plays the anchor", () => {
    const anchor = anchorOnTile("cars", "family", "classic");
    const classicPorsche: CarFacts = { brand: "Porsche", families: ["classic"], country: "Germany" };
    expect(resolve({ kind: "car", car: classicPorsche }, anchor, A).assigned).toBe("D:/music/classic");
  });

  it("a car not filed under the anchor goes down its cascade: brand first", () => {
    const anchor = anchorOnTile("cars", "family", "classic");
    expect(resolve({ kind: "car", car: gt3rs }, anchor, A).assigned).toBe("D:/music/porsche");
  });

  it("car cascade without anchor: brand → family → country → screen → MENU", () => {
    const noBrand = { ...A, brand: {} };
    expect(resolve({ kind: "car", car: gt3rs }, null, noBrand).assigned).toBe("D:/music/race");
    expect(resolve({ kind: "car", car: miata }, null, A).assigned).toBe("D:/music/japan");
    const bare: CarFacts = { brand: null, families: [], country: null };
    expect(resolve({ kind: "car", car: bare }, null, A).assigned).toBe("D:/music/cars-screen");
    expect(resolve({ kind: "car", car: bare }, null, emptyAssignments())).toEqual({ assigned: null, base: "menu" });
  });

  it("track cascade: main category only, then country", () => {
    expect(resolve({ kind: "track", track: ebisu }, null, A).assigned).toBe("D:/music/drift");
    const circuitThenStreet: TrackFacts = { categories: ["#circuit", "#street"], country: null };
    expect(resolve({ kind: "track", track: circuitThenStreet }, null, A).assigned).toBeNull();
    expect(resolve({ kind: "track", track: monza }, null, A).assigned).toBe("D:/music/italy");
  });

  it("a track anchor matches any of its categories, not only the main one", () => {
    const anchor = anchorOnTile("tracks", "track_category", "#street");
    const t: TrackFacts = { categories: ["#circuit", "#street"], country: "Italy" };
    expect(resolve({ kind: "track", track: t }, anchor, A).assigned).toBe("D:/music/street");
  });

  it("a car anchor says nothing about a track sheet", () => {
    const anchor = anchorOnTile("cars", "country", "Italy");
    expect(resolve({ kind: "track", track: ebisu }, anchor, A).assigned).toBe("D:/music/drift");
  });

  it("terms are matched on their id, case aside", () => {
    const car: CarFacts = { brand: "PORSCHE", families: [], country: null };
    expect(resolve({ kind: "car", car }, null, A).assigned).toBe("D:/music/porsche");
  });
});

describe("session settings and other screens", () => {
  it("session: track category → track country → brand → family → GRID", () => {
    expect(resolve({ kind: "session", car: gt3rs, track: ebisu }, null, A)).toEqual({ assigned: "D:/music/drift", base: "grid" });
    expect(resolve({ kind: "session", car: gt3rs, track: monza }, null, A).assigned).toBe("D:/music/italy");
    const nowhere: TrackFacts = { categories: [], country: null };
    expect(resolve({ kind: "session", car: gt3rs, track: nowhere }, null, A).assigned).toBe("D:/music/porsche");
    const noBrand = { ...A, brand: {} };
    expect(resolve({ kind: "session", car: gt3rs, track: nowhere }, null, noBrand).assigned).toBe("D:/music/race");
    expect(resolve({ kind: "session", car: null, track: null }, null, A)).toEqual({ assigned: null, base: "grid" });
  });

  it("the anchor plays no part in the session settings, and stays in memory", () => {
    const anchor = anchorOnTile("cars", "brand", "Ferrari");
    expect(resolve({ kind: "session", car: gt3rs, track: monza }, anchor, A).assigned).toBe("D:/music/italy");
    expect(anchorAfter(anchor, { kind: "session", car: gt3rs, track: monza })).toEqual(anchor);
  });

  it("another screen plays its own music, then MENU", () => {
    expect(resolve({ kind: "screen", key: "online" }, null, A).assigned).toBe("D:/music/online");
    expect(resolve({ kind: "screen", key: "apps" }, null, A)).toEqual({ assigned: null, base: "menu" });
    expect(resolve({ kind: "screen", key: null }, null, A)).toEqual({ assigned: null, base: "menu" });
  });

  it("Classement and Fichiers are one screen each, the session settings none", () => {
    const sorting = ["rules", "brands"];
    const files = ["import", "maintenance"];
    expect(screenKeyOf("brands", sorting, files)).toBe("sorting");
    expect(screenKeyOf("maintenance", sorting, files)).toBe("files");
    expect(screenKeyOf("online", sorting, files)).toBe("online");
    expect(screenKeyOf("race", sorting, files)).toBeNull();
    expect(screenKeyOf("story", sorting, files)).toBeNull();
  });
});

describe("same source, two contexts", () => {
  // The engine makes no transition when the source does not change (§16.6);
  // the front does not even send a context that resolves the same.
  it("two screens resolving to one source are the same resolution", () => {
    const shared = { ...A, screen: { cars: "D:/music/same", tracks: "D:/music/same" } };
    const onCars = resolve({ kind: "index", library: "cars" }, null, shared);
    const onTracks = resolve({ kind: "index", library: "tracks" }, null, shared);
    expect(sameResolution(onCars, onTracks)).toBe(true);
    const online = resolve({ kind: "screen", key: "online" }, null, shared);
    expect(sameResolution(onCars, online)).toBe(false);
    expect(sameResolution({ assigned: null, base: "menu" }, { assigned: null, base: "grid" })).toBe(false);
  });
});

describe("when the music changes", () => {
  // A click on the rail or a tile is heard at once; only sheets wait, since
  // one may be scrolling through them (§16.6).
  it("only a sheet waits before playing", () => {
    expect(waitsBeforePlaying({ kind: "car", car: gt3rs })).toBe(true);
    expect(waitsBeforePlaying({ kind: "track", track: monza })).toBe(true);
    expect(waitsBeforePlaying({ kind: "screen", key: "online" })).toBe(false);
    expect(waitsBeforePlaying({ kind: "index", library: "cars" })).toBe(false);
    expect(waitsBeforePlaying({ kind: "list", library: "cars", anchorPosed: true })).toBe(false);
    expect(waitsBeforePlaying({ kind: "session", car: null, track: null })).toBe(false);
  });
});

describe("editing", () => {
  it("assigning and unassigning returns a new table", () => {
    const a = withAssignment(emptyAssignments(), "brand", "Porsche", "D:/x");
    expect(a.brand).toEqual({ Porsche: "D:/x" });
    expect(withAssignment(a, "brand", "Porsche", null).brand).toEqual({});
    expect(emptyAssignments().brand).toEqual({});
  });

  it("recent sources: newest first, once, bounded", () => {
    expect(withRecent(["a", "b", "c"], "b")).toEqual(["b", "a", "c"]);
    expect(withRecent(["a"], "A")).toEqual(["A"]);
    expect(withRecent(["1", "2", "3"], "4", 3)).toEqual(["4", "1", "2"]);
  });
});
