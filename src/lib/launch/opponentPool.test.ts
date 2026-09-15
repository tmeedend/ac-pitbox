import { describe, expect, it } from "vitest";
import type { ModCard } from "$lib/library/library";
import { chipAvailable, defaultGridFilters, isChipOn, toggleChip } from "./opponentPool";

const car = (over: Partial<ModCard> = {}): ModCard =>
  ({ id_interne: "ks_ferrari_488_gt3", display_name: "Ferrari 488 GT3", category: "#gt3", ...over }) as ModCard;

describe("defaultGridFilters", () => {
  // A pinned "playable" token used to sit here. It is gone on purpose: a mod
  // disabled under a grid is rare, and the activation guard already says so
  // and repairs it — where the chip cost room in the bar every single time.
  it("starts on nothing at all", () => {
    expect(defaultGridFilters()).toEqual({});
  });
});

describe("toggleChip", () => {
  it("poses the token, and takes it away on a second click", () => {
    const on = toggleChip({}, "category", car());
    expect(on.category).toEqual({ type: "val", values: [{ value: "#gt3", sign: 1 }], op: "and" });
    expect(isChipOn(on, "category", car())).toBe(true);
    expect(toggleChip(on, "category", car())).toEqual({});
  });

  // "Same category as mine" is a statement about WHICH category. Adding to a
  // hand-typed `#gt2` would make the chip mean something it does not say.
  it("replaces what the filter held rather than adding to it", () => {
    const hand = { category: { type: "val" as const, values: [{ value: "#gt2", sign: 1 as const }], op: "and" as const } };
    expect(toggleChip(hand, "category", car()).category).toEqual({
      type: "val",
      values: [{ value: "#gt3", sign: 1 }],
      op: "and",
    });
  });

  it("leaves the rest of the map alone", () => {
    const before = { year: { type: "range" as const, min: 2010, max: 2016 } };
    const after = toggleChip(before, "model", car());
    expect(after.year).toBe(before.year);
    expect(after.model).toEqual({ type: "val", values: [{ value: "Ferrari 488 GT3", sign: 1 }], op: "and" });
  });

  it("does nothing when the reference cannot supply a value", () => {
    const before = defaultGridFilters();
    expect(toggleChip(before, "category", car({ category: null }))).toBe(before);
    expect(toggleChip(before, "model", null)).toBe(before);
  });

  // The band poses a tolerance, not a value: its reference is read from the
  // context, so it follows the car one drives.
  it("poses the band as a tolerance, with no value of its own", () => {
    const on = toggleChip({}, "performance", car());
    expect(on.performance).toEqual({ type: "perf", pct: 15 });
  });
});

describe("isChipOn", () => {
  // The chip reads the token, so a token posed by hand lights the chip up too.
  // That is the point: there is no second state to keep in sync.
  it("lights up for a token posed by hand with the same value", () => {
    const byHand = { category: { type: "val" as const, values: [{ value: "#GT3", sign: 1 as const }], op: "and" as const } };
    expect(isChipOn(byHand, "category", car())).toBe(true);
  });
  it("stays off for an exclusion of the same value", () => {
    const exc = { category: { type: "val" as const, values: [{ value: "#gt3", sign: -1 as const }], op: "and" as const } };
    expect(isChipOn(exc, "category", car())).toBe(false);
  });
  it("stays off when the car changed under a posed token", () => {
    const on = toggleChip({}, "model", car());
    expect(isChipOn(on, "model", car({ display_name: "BMW Z4 GT3" }))).toBe(false);
  });
});

describe("chipAvailable", () => {
  it("refuses the band when the driven car has no readable ratio", () => {
    expect(chipAvailable("performance", car(), null)).toBe(false);
    expect(chipAvailable("performance", car(), 3.4)).toBe(true);
  });
  it("refuses a category chip on a car that declares none", () => {
    expect(chipAvailable("category", car({ category: null }), 3.4)).toBe(false);
  });
});
