import { describe, expect, it, vi } from "vitest";

// `browseIndex.ts` reads `UNSET_VALUE` from `filters.ts`, which reaches i18n
// for its chip labels; nothing here translates (see `filters.test.ts`).
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

const { brandTiles, indexTiles, initials, significantBrandCount } = await import("./browseIndex");
const { buildCardIndex, buildPredicate, filterDefs, hasActiveFilter, poseValue, UNSET_VALUE } = await import("./filters");
type ModCard = import("./library").ModCard;

const opt = (value: string, count: number) => ({ value, label: value, count });

describe("indexTiles", () => {
  // INDEX§4.3: the absent value closes the grid whatever its count - it
  // is the hole in the data, not a value among the others.
  it("puts the largest first and the absent value last", () => {
    const tiles = indexTiles([opt("Italy", 38), opt(UNSET_VALUE, 40), opt("Japan", 34), opt("Monaco", 1)]);
    expect(tiles.map((t) => t.value)).toEqual(["Italy", "Japan", "Monaco", UNSET_VALUE]);
    expect(tiles.at(-1)?.unset, "the last tile is the dashed one").toBe(true);
  });

  it("gives no tile to a value no mod carries", () => {
    // A shipped family with nothing in it would lead to an empty list.
    expect(indexTiles([opt("race", 3), opt("rally", 0), opt(UNSET_VALUE, 0)]).map((t) => t.value)).toEqual(["race"]);
  });

  it("breaks ties by name, so two launches draw the same grid", () => {
    expect(indexTiles([opt("Spain", 4), opt("Brazil", 4)]).map((t) => t.value)).toEqual(["Brazil", "Spain"]);
  });
});

describe("significantBrandCount", () => {
  // INDEX§6.2: the brands that cover 80 % of the library by decreasing
  // count, bounded to [8, 24] - never a fixed top N.
  it("stops once 80 % of the library is covered", () => {
    // 100 cars: ten brands of 9, then ten of 1. Nine brands reach 81.
    const counts = [...Array(10).fill(9), ...Array(10).fill(1)];
    expect(significantBrandCount(counts, 100)).toBe(9);
  });

  it("never shows fewer than eight tiles when there are more brands", () => {
    // One brand alone covers the library: the grid would say nothing.
    expect(significantBrandCount([90, 2, 2, 2, 1, 1, 1, 1, 1, 1], 102)).toBe(8);
  });

  it("never shows more than twenty-four, however flat the library", () => {
    expect(significantBrandCount(Array(200).fill(10), 2000)).toBe(24);
  });

  it("never asks for more tiles than there are brands", () => {
    expect(significantBrandCount([3, 2], 5)).toBe(2);
  });

  it("counts brandless cars in what has to be covered", () => {
    // 20 branded cars out of 100: covering 80 % is out of reach, so every
    // brand up to the ceiling is shown rather than stopping early.
    expect(significantBrandCount(Array(12).fill(1), 100)).toBe(12);
  });
});

describe("brandTiles", () => {
  const tile = (value: string, count: number) => ({ value, label: value, count, unset: false });

  it("unfolds the rest in alphabetical order, after the significant ones", () => {
    const tiles = [
      tile("Porsche", 50),
      ...["B", "C", "D", "E", "F", "G", "H"].map((b) => tile(b, 5)),
      tile("Zonda", 1),
      tile("Abarth", 1),
    ];
    const folded = brandTiles(tiles, 100, false);
    expect(folded.shown).toHaveLength(8);
    expect(folded.hidden, "the link says what it unfolds").toBe(2);
    const open = brandTiles(tiles, 100, true);
    expect(open.shown.slice(8).map((t) => t.value)).toEqual(["Abarth", "Zonda"]);
    expect(open.hidden).toBe(0);
  });
});

describe("initials", () => {
  it("never leaves the disc empty", () => {
    expect(initials("Mercedes-Benz")).toBe("MB");
    expect(initials("Alfa Romeo")).toBe("AR");
    expect(initials("Porsche")).toBe("PO");
  });
});

describe("the tile poses a chip", () => {
  const FAMILIES = [
    { id: "prototype", tags: ["lmp1", "group c"] },
    { id: "race", tags: ["race", "gt3"] },
  ];
  const DEFS = filterDefs("Car", FAMILIES);
  const TRACK_DEFS = filterDefs("Track");

  // INDEX§7: crossing or replacing follows from the filter (a car
  // carries several families, one country) - it is not decided per taxonomy.
  it("adds a second family under AND, and replaces a country", () => {
    const one = poseValue(DEFS, {}, "family", "prototype");
    const two = poseValue(DEFS, one, "family", "race");
    expect(two.family).toEqual({
      type: "val",
      values: [{ value: "prototype", sign: 1 }, { value: "race", sign: 1 }],
      op: "and",
    });
    const it_ = poseValue(TRACK_DEFS, {}, "country", "Japan");
    expect(poseValue(TRACK_DEFS, it_, "country", "Italy").country).toEqual({
      type: "val",
      values: [{ value: "Italy", sign: 1 }],
      op: "and",
    });
  });

  it("leaves an OR the user chose in the editor alone", () => {
    const or = { family: { type: "val" as const, values: [{ value: "race", sign: 1 as const }], op: "or" as const } };
    expect(poseValue(DEFS, or, "family", "prototype").family).toMatchObject({ op: "or" });
  });

  it("turns an excluded value into an included one rather than holding both", () => {
    const exc = { family: { type: "val" as const, values: [{ value: "race", sign: -1 as const }], op: "and" as const } };
    expect(poseValue(DEFS, exc, "family", "race").family).toMatchObject({ values: [{ value: "race", sign: 1 }] });
  });

  // INDEX§3: a pinned chip left blank is a ghost - the index stays.
  it("shows the index while every chip is a ghost and the search is empty", () => {
    expect(hasActiveFilter({ country: { type: "val", values: [], op: "and" } }, "  ")).toBe(false);
    expect(hasActiveFilter({}, "spa")).toBe(true);
    expect(hasActiveFilter(poseValue(TRACK_DEFS, {}, "country", "Japan"), "")).toBe(true);
  });

  // The "Unclassified" tile poses a value like any other, and it has to match
  // exactly the cars the tile counted.
  it("matches the unclassified tile on the cars that reach no family", () => {
    const card = (id: string, tags: string[]) =>
      ({ id_interne: id, kind: "Car", tags_from_mod: tags, tags_from_rule: [], tags_manual: [] }) as unknown as ModCard;
    const cards = [card("a", ["#LMP1", "race"]), card("b", ["traffic"]), card("c", ["gt3"])];
    const index = buildCardIndex(cards, DEFS, true, () => false, null, FAMILIES);
    expect(index.optionsFor("family").map((o) => [o.value, o.count])).toEqual([
      ["prototype", 1],
      ["race", 2],
      [UNSET_VALUE, 1],
    ]);
    const posed = poseValue(DEFS, {}, "family", UNSET_VALUE);
    const keep = buildPredicate(DEFS, posed, index.ctx);
    expect(cards.filter(keep).map((c) => c.id_interne), "the tile's own cars, no more").toEqual(["b"]);
  });
});
