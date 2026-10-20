// REGLES§3 level 4, seen from the screens: what the library shows for a
// category is what the cards carry - `category` for a car, `categories` for a
// track - and the backend has already laid the user's decision on it. A forced
// category the rules never produce, a category added to or taken off a track,
// must therefore reach the Category filter, and the pool the opponents grid
// draws from, with nothing more.
import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));
// `columns.ts` reads the table preferences through runes modules the runner
// does not compile; nothing tested here reads a preference.
vi.mock("$lib/tablePrefs.svelte", () => ({ loadSavedPrefs: async () => null, saveTablePrefs: async () => {} }));
vi.mock("$lib/uiPrefs.svelte", () => ({ peekUiPref: () => null }));

const { buildCardIndex, buildPredicate, filterDefs, optionsOf } = await import("./filters");
const { columnsFor } = await import("./columns");
const { pickCars } = await import("$lib/launch/gridRules");
type ModCard = import("./library").ModCard;
type FilterMap = Record<string, import("./filters").FilterState>;

const card = (over: Partial<ModCard>): ModCard =>
  ({
    kind: "Car",
    category: null,
    categories: [],
    tags_from_mod: [],
    tags_from_rule: [],
    tags_manual: [],
    car_class: null,
    description: null,
    notes_user: null,
    bhp: null,
    weight: null,
    ...over,
  }) as ModCard;

function filterOn(kind: "Car" | "Track", cards: ModCard[]) {
  const defs = filterDefs(kind);
  const index = buildCardIndex(cards, defs, kind === "Car", () => false);
  const def = defs.find((d) => d.key === "category");
  if (!def) throw new Error("no category filter");
  const options = optionsOf(def, cards, index.ctx).map((o) => o.value);
  const keep = (value: string) => {
    const filters: FilterMap = { category: { type: "val", values: [{ value, sign: 1 }], op: "and" } };
    return cards.filter(buildPredicate(defs, filters, index.ctx)).map((c) => c.id_interne);
  };
  return { options, keep };
}

describe("a forced category in the screens", () => {
  it("offers a forced category the rules never produce, and keeps the car in the pool", () => {
    const cars = [
      card({ id_interne: "rules", category: "#gt3", category_forced: null, category_source: "rules" }),
      card({ id_interne: "forced", category: "#hot_rods", category_forced: "#hot_rods", category_source: "manual" }),
    ];
    const { options, keep } = filterOn("Car", cars);
    expect(options).toContain("#hot_rods");
    expect(keep("#hot_rods")).toEqual(["forced"]);
    expect(keep("#gt3")).toEqual(["rules"]);
  });

  it("reflects what was added to and taken off a track in the filter", () => {
    const tracks = [
      // computed #touge, #drift - the user took #touge off and added #rally.
      card({ id_interne: "edited", kind: "Track", categories: ["#rally", "#drift"], categories_added: ["#rally"] }),
      card({ id_interne: "plain", kind: "Track", categories: ["#touge", "#drift"] }),
    ];
    const { options, keep } = filterOn("Track", tracks);
    expect(options).toContain("#rally");
    expect(keep("#rally")).toEqual(["edited"]);
    expect(keep("#touge")).toEqual(["plain"]);
    expect(keep("#drift")).toEqual(["edited", "plain"]);
  });

  // The opponents grid draws from the cards the Category filter keeps - the
  // same predicate - so a forced category is a category the pool can be
  // narrowed to, and the draw reaches the car.
  it("lets the opponents grid draw a car from a forced category", () => {
    const cars = [
      card({ id_interne: "rules", category: "#gt3" }),
      card({ id_interne: "forced", category: "#hot_rods", category_forced: "#hot_rods" }),
    ];
    const defs = filterDefs("Car");
    const index = buildCardIndex(cars, defs, true, () => false);
    const filters: FilterMap = { category: { type: "val", values: [{ value: "#hot_rods", sign: 1 }], op: "and" } };
    const pool = cars.filter(buildPredicate(defs, filters, index.ctx));
    expect(pickCars(pool, 1, new Set()).map((c) => c.id_interne)).toEqual(["forced"]);
  });

  // The library's Category column reads what the card carries: the list a
  // track now has, in its order, not the single value the old group field wrote.
  it("shows a track's categories as they are now in the Category column", () => {
    const column = columnsFor("Track").find((c) => c.key === "category");
    if (!column) throw new Error("no category column");
    const edited = card({ kind: "Track", categories: ["#rally", "#drift"], categories_added: ["#rally"] });
    expect(column.value(edited)).toBe("#rally · #drift");
    expect(column.sortValue?.(edited)).toBe("#rally");
    expect(column.value(card({ kind: "Track", categories: [] })), "nothing: a dash").not.toContain("#");
  });
});
