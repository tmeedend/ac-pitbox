import { describe, expect, it } from "vitest";
import type { ModCard } from "$lib/library/library";
import type { SkinItem } from "./launch";
import { newOpponent, type Opponent } from "./opponent";
import {
  driverKey,
  hasDuplicateDrivers,
  migrateTabPool,
  newTaken,
  pickCars,
  pickSkin,
  restoreOpponent,
} from "./gridRules";

const skin = (id: string, number: string | null = null, driver: string | null = null): SkinItem => ({
  id,
  name: id,
  preview: null,
  livery: null,
  driver,
  number,
  country: null,
});
const car = (id: string, over: Partial<ModCard> = {}): ModCard =>
  ({ id_interne: id, display_name: id, category: "#gt3", ...over }) as ModCard;
/** Always the first candidate: makes a random draw readable in a test. */
const first = () => 0;

describe("driverKey", () => {
  it("is the number and the name, case-insensitive", () => {
    expect(driverKey(skin("a", "59", "Juan"))).toBe(driverKey(skin("b", "59", "JUAN")));
  });

  // A livery that declares no driver imposes nothing on the others.
  it("is null for a silent livery", () => {
    expect(driverKey(skin("a"))).toBeNull();
  });
});

describe("pickSkin", () => {
  // The visible bug: two different liveries both saying "59 Juan" made two
  // grid rows nobody could tell apart.
  it("avoids a driver already on the grid, not only the same livery", () => {
    const taken = newTaken();
    pickSkin([skin("a", "59", "Juan")], taken, first);
    expect(pickSkin([skin("b", "59", "juan"), skin("c", "7", "Ana")], taken, first)).toBe("c");
  });

  it("falls back to a livery not yet taken, then to any, rather than none", () => {
    const taken = newTaken();
    const both = [skin("a", "1", "X"), skin("b", "1", "X")];
    expect(pickSkin(both, taken, first)).toBe("a");
    expect(pickSkin(both, taken, first)).toBe("b");
    expect(pickSkin(both, taken, first)).toBe("a");
  });

  it("gives null for a car without liveries", () => {
    expect(pickSkin([], newTaken())).toBeNull();
  });
});

describe("pickCars", () => {
  it("draws distinct cars first, avoiding those already on the grid", () => {
    const picks = pickCars([car("a"), car("b"), car("c")], 2, new Set(["a"]), first);
    expect(picks.map((c) => c.id_interne).sort()).toEqual(["b", "c"]);
  });

  // CIBLE§3.5: a thin pool duplicates rather than truncating the grid.
  it("completes a thin pool by repeating its cars", () => {
    expect(pickCars([car("a")], 3, new Set(), first).map((c) => c.id_interne)).toEqual(["a", "a", "a"]);
  });

  // A filter that keeps nothing gives an empty grid, never one drawn elsewhere.
  it("draws nothing from an empty pool", () => {
    expect(pickCars([], 5, new Set())).toEqual([]);
  });
});

describe("hasDuplicateDrivers", () => {
  const skins: Record<string, SkinItem> = { a: skin("a", "59", "Juan"), b: skin("b", "59", "Juan"), c: skin("c") };
  const opp = (id: string, over: Partial<Opponent> = {}): Opponent => ({ ...newOpponent("car", id), ...over });
  const skinOf = (o: Opponent) => (o.car_skin ? skins[o.car_skin] : undefined);

  it("sees two rows whose liveries declare the same driver", () => {
    expect(hasDuplicateDrivers([opp("a"), opp("b")], skinOf)).toBe(true);
  });

  it("lets a typed name tell them apart", () => {
    expect(hasDuplicateDrivers([opp("a"), opp("b", { driver_name: "Ana" })], skinOf)).toBe(false);
  });

  it("ignores rows with no identity at all", () => {
    expect(hasDuplicateDrivers([opp("c"), opp("c")], skinOf)).toBe(false);
  });
});

describe("restoreOpponent", () => {
  // Fields absent from an older file must come back as `Auto` (null), never as
  // `undefined`, which the backend would read as a missing field.
  it("fills the fields an older grid did not have", () => {
    const old = { car_id: "x", ai_level: 90, car_skin: "s" } as Opponent;
    expect(restoreOpponent(old)).toEqual({
      car_id: "x",
      ai_level: 90,
      car_skin: "s",
      driver_name: null,
      nationality: null,
      ballast: 0,
      restrictor: 0,
    });
  });

  it("clamps a strength saved when the floor was lower, and keeps Auto", () => {
    expect(restoreOpponent({ ...newOpponent("x", null), ai_level: 10 }).ai_level).toBeGreaterThan(10);
    expect(restoreOpponent(newOpponent("x", null)).ai_level).toBeNull();
  });
});

describe("migrateTabPool", () => {
  const player = car("ks_ferrari_488_gt3", { display_name: "Ferrari 488 GT3" });

  it("turns the old 'same car' tab into a Model token", () => {
    expect(migrateTabPool({ grid_mode: "same_car" }, player).model).toEqual({
      type: "val",
      values: [{ value: "Ferrari 488 GT3", sign: 1 }],
      op: "and",
    });
  });

  it("turns 'same category' and a year range into two tokens", () => {
    const m = migrateTabPool({ grid_mode: "same_category", year_min: 2010, year_max: 2016 }, player);
    expect(m.category).toEqual({ type: "val", values: [{ value: "#gt3", sign: 1 }], op: "and" });
    expect(m.year).toEqual({ type: "range", min: 2010, max: 2016 });
  });

  // 0 meant "no bound" on both sides already.
  it("reads a zero year as no bound", () => {
    expect(migrateTabPool({ grid_mode: "free", year_min: 0, year_max: 0 }, player)).toEqual({});
  });
});
