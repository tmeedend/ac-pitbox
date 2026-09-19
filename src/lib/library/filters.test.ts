import { describe, expect, it, vi } from "vitest";

// `filters.ts` reaches i18n for the labels it puts on a chip; nothing tested
// here translates anything. The stub is what keeps this a pure-logic test,
// rather than a reason to teach the runner to compile runes (see
// `vitest.config.ts`).
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

// Imported after the stub is registered, `vi.mock` being hoisted above a plain
// top-level import.
const { buildPredicate, filterDefs, parseFilters, serializeFilters } = await import("./filters");
type FilterMap = Record<string, import("./filters").FilterState>;

const CAR_DEFS = filterDefs("Car");

/** What `parseFilters` returns for one key, or `undefined`. */
function restored(raw: string): FilterMap {
  return parseFilters(raw, CAR_DEFS).filters;
}

describe("parseFilters", () => {
  // The filter bar is persisted in `ui_prefs.json` and read back at every
  // start. Every failure below is silent by nature: the library simply opens
  // unfiltered, and nothing on screen says a preference was lost.
  it("reads back what the current version wrote, sign and operator included", () => {
    const filters: FilterMap = {
      tag: { type: "val", values: [{ value: "gt3", sign: 1 }, { value: "wip", sign: -1 }], op: "or" },
      year: { type: "range", min: 1990, max: null },
      favorite: { type: "bool", sign: -1 },
      description: { type: "text", text: "turbo" },
    };
    const back = parseFilters(serializeFilters("zonda", filters), CAR_DEFS);

    expect(back.query).toBe("zonda");
    expect(back.filters).toEqual(filters);
  });

  /** Generation 2: the eleven-control bar, with include/exclude tokens and
   * tri-states. */
  it("restores the tokens and tri-states of the eleven-control bar", () => {
    const filters = restored(
      JSON.stringify({
        query: "",
        brandTokens: [{ value: "Ferrari", mode: "inc" }, { value: "Fiat", mode: "exc" }],
        tagTokens: [{ value: "gt3", mode: "inc" }],
        tagMode: "or",
        favState: 1,
        driverState: -1,
        yearMin: 1990,
        yearMax: 1999,
      }),
    );

    expect(filters.brand).toEqual({
      type: "val",
      values: [{ value: "Ferrari", sign: 1 }, { value: "Fiat", sign: -1 }],
      op: "and",
    });
    expect(filters.tag).toEqual({ type: "val", values: [{ value: "gt3", sign: 1 }], op: "or" });
    expect(filters.favorite).toEqual({ type: "bool", sign: 1 });
    expect(filters.driver).toEqual({ type: "bool", sign: -1 });
    expect(filters.year).toEqual({ type: "range", min: 1990, max: 1999 });
  });

  /** Generation 1: single-valued selects and a comma-separated tag string. */
  it("restores the first generation's selects and comma-separated tags", () => {
    const filters = restored(JSON.stringify({ brand: "Ferrari", tag: "gt3, historique ,", class: "GT3" }));

    expect(filters.brand).toEqual({ type: "val", values: [{ value: "Ferrari", sign: 1 }], op: "and" });
    expect(filters.tag).toEqual({
      type: "val",
      values: [{ value: "gt3", sign: 1 }, { value: "historique", sign: 1 }],
      op: "and",
    });
    expect(filters.carClass).toEqual({ type: "val", values: [{ value: "GT3", sign: 1 }], op: "and" });
  });

  it("keeps `all` and an empty select for what they meant: no filter at all", () => {
    expect(restored(JSON.stringify({ brand: "all", class: "all", tag: "" }))).toEqual({});
  });

  /** The bound that said "no bound" must not start filtering. */
  it("drops a year bound that was the range's own edge", () => {
    expect(restored(JSON.stringify({ yearMin: 1950, yearMax: new Date().getFullYear() })).year).toBeUndefined();
  });

  /** The checkbox is gone; what it asked for is not. */
  it("folds the base-content checkbox into the state filter", () => {
    const filters = restored(JSON.stringify({ hideBaseContent: true }));
    expect(filters.state).toEqual({ type: "val", values: [{ value: "stock", sign: -1 }], op: "and" });
    expect(filters.base, "the old key does not survive under its own name").toBeUndefined();
  });

  it("folds it WITHOUT dropping a state the same preference carried", () => {
    const filters = restored(JSON.stringify({ state: "inactive", hideBaseContent: true }));
    expect(filters.state).toEqual({
      type: "val",
      values: [{ value: "inactive", sign: 1 }, { value: "stock", sign: -1 }],
      op: "and",
    });
  });

  it("drops what the catalogue no longer knows, and what changed type", () => {
    const filters = restored(
      JSON.stringify({
        query: "",
        filters: {
          gone: { type: "val", values: [{ value: "x", sign: 1 }], op: "and" },
          year: { type: "val", values: [{ value: "1990", sign: 1 }], op: "and" },
          tag: { type: "val", values: [{ value: "gt3", sign: 1 }, { value: "oops" }], op: "and" },
        },
      }),
    );

    expect(filters.gone, "a filter the catalogue lost").toBeUndefined();
    expect(filters.year, "a filter that changed type is dropped, never half-restored").toBeUndefined();
    expect(filters.tag, "a malformed value goes, the sound ones stay").toEqual({
      type: "val",
      values: [{ value: "gt3", sign: 1 }],
      op: "and",
    });
  });

  it("survives a preference that is not JSON at all", () => {
    expect(parseFilters("{not json", CAR_DEFS)).toEqual({ query: "", filters: {} });
    expect(parseFilters(null, CAR_DEFS)).toEqual({ query: "", filters: {} });
  });
});

describe("buildPredicate", () => {
  interface Fake {
    id_interne: string;
    tags: string[];
  }
  const ctx = {
    isCar: true,
    tagsOf: (c: unknown) => (c as Fake).tags,
    descOf: () => undefined,
    noteOf: () => undefined,
    hasDriver: () => false,
    ratioOf: () => null,
    perfRef: null,
  } as unknown as import("./filters").FilterContext;

  const car = (tags: string[]) => ({ id_interne: tags.join("_"), tags }) as unknown as import("./library").ModCard;

  const keep = (filters: FilterMap, tags: string[]) => buildPredicate(CAR_DEFS, filters, ctx)(car(tags));

  it("requires every included value under AND, and any of them under OR", () => {
    const values = [{ value: "gt3", sign: 1 as const }, { value: "endurance", sign: 1 as const }];
    expect(keep({ tag: { type: "val", values, op: "and" } }, ["gt3"])).toBe(false);
    expect(keep({ tag: { type: "val", values, op: "and" } }, ["gt3", "endurance"])).toBe(true);
    expect(keep({ tag: { type: "val", values, op: "or" } }, ["gt3"])).toBe(true);
  });

  /** The rule the editor writes out in so many words: an exclusion is never
   * subject to the operator. Under OR, a card matching one included value but
   * also an excluded one must still go. */
  it("applies an exclusion whatever the operator", () => {
    const values = [{ value: "gt3", sign: 1 as const }, { value: "wip", sign: -1 as const }];
    expect(keep({ tag: { type: "val", values, op: "or" } }, ["gt3"])).toBe(true);
    expect(keep({ tag: { type: "val", values, op: "or" } }, ["gt3", "wip"])).toBe(false);
    expect(keep({ tag: { type: "val", values, op: "and" } }, ["gt3", "wip"])).toBe(false);
  });

  it("keeps everything when a filter holds nothing", () => {
    expect(keep({ tag: { type: "val", values: [], op: "and" } }, [])).toBe(true);
  });
});
