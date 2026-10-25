// The header form's decisions (name, brand, year): what a typed text changes.
// A rule per test, since a wrong one here silently pins or loses a correction.
import { describe, expect, it } from "vitest";
import { categoriesChange, hasCategory, categoryChange, parseYear, textChange, yearChange, yearRefused, type IdentityField } from "./identityEdit";

const name = (over: Partial<IdentityField<string>> = {}): IdentityField<string> => ({
  value: "Ferrari 458 Italia",
  original: "Ferrari 458 Italia",
  overridden: false,
  ...over,
});
const year = (over: Partial<IdentityField<number>> = {}): IdentityField<number> => ({
  value: 2011,
  original: 2011,
  overridden: false,
  ...over,
});

describe("parseYear", () => {
  it("accepts four digits in a plausible range, and nothing else", () => {
    expect(parseYear(" 1998 ")).toBe(1998);
    expect(parseYear("98")).toBeNull();
    expect(parseYear("19980")).toBeNull();
    expect(parseYear("20x1")).toBeNull();
    expect(parseYear("1799")).toBeNull();
    expect(parseYear("2101")).toBeNull();
  });
});

describe("textChange", () => {
  it("a new text is a decision", () => {
    expect(textChange("  Abarth 500 ", name())).toEqual({ change: "Abarth 500" });
  });

  it("the shown text typed again changes nothing", () => {
    expect(textChange("Ferrari 458 Italia", name())).toBeNull();
    expect(textChange("Mine", name({ value: "Mine", overridden: true }))).toBeNull();
  });

  it("emptying a decision, or typing the original back, goes back to it", () => {
    const decided = name({ value: "Mine", overridden: true });
    expect(textChange("   ", decided)).toEqual({ change: null });
    expect(textChange("Ferrari 458 Italia", decided)).toEqual({ change: null });
  });

  it("emptying what was never a decision changes nothing", () => {
    expect(textChange("", name())).toBeNull();
  });

  it("a shown value that is not the original can be pinned by typing the original", () => {
    expect(textChange("Ferrari 458 Italia", name({ value: "Ferrari 458" }))).toEqual({ change: "Ferrari 458 Italia" });
  });
});

describe("yearChange", () => {
  it("a new valid year is a decision", () => {
    expect(yearChange("1998", year())).toEqual({ change: 1998 });
  });

  it("an invalid text is no change: the form refuses it before saving", () => {
    expect(yearChange("19x8", year())).toBeNull();
  });

  it("emptying a decision, or typing the original back, goes back to it", () => {
    const decided = year({ value: 1998, overridden: true });
    expect(yearChange("", decided)).toEqual({ change: null });
    expect(yearChange("2011", decided)).toEqual({ change: null });
  });

  it("the shown year typed again changes nothing", () => {
    expect(yearChange("2011", year())).toBeNull();
    expect(yearChange("1998", year({ value: 1998, overridden: true }))).toBeNull();
  });

  it("a year the mod does not give can be set, and a forced unknown can be left", () => {
    expect(yearChange("1925", year({ value: null, original: null }))).toEqual({ change: 1925 });
    expect(yearChange("", year({ value: null, original: 2011, overridden: true }))).toEqual({ change: null });
  });
});

describe("yearRefused", () => {
  it("refuses a typed year the form cannot store", () => {
    expect(yearRefused("20x1", year())).toBe(true);
    expect(yearRefused("1799", year())).toBe(true);
  });

  it("accepts a valid year and an empty field", () => {
    expect(yearRefused("1998", year())).toBe(false);
    expect(yearRefused("  ", year())).toBe(false);
  });

  it("never refuses the year already shown, even one only the tech sheet takes", () => {
    expect(yearRefused("99", year({ value: 99, overridden: true }))).toBe(false);
    expect(yearRefused("3000", year({ value: 3000, overridden: true }))).toBe(false);
  });
});

describe("categoryChange", () => {
  const cat = (over: Partial<IdentityField<string>> = {}): IdentityField<string> => ({
    value: "#gt3",
    original: "#gt3",
    overridden: false,
    ...over,
  });

  it("another category is a decision, handed back as typed", () => {
    expect(categoryChange(" #drift ", cat())).toEqual({ change: "#drift" });
    expect(categoryChange("Vintage Racers", cat())).toEqual({ change: "Vintage Racers" });
  });

  it("the category shown, spelled another way, changes nothing", () => {
    expect(categoryChange("GT3", cat())).toBeNull();
    expect(categoryChange("#GT3", cat({ value: "#drift", original: "#drift", overridden: true }))).toEqual({
      change: "#GT3",
    });
    expect(categoryChange("drift", cat({ value: "#drift", original: "#gt3", overridden: true }))).toBeNull();
  });

  it("emptying a decision, or typing the computed category back, goes back to it", () => {
    const filed = cat({ value: "#drift", overridden: true });
    expect(categoryChange("", filed)).toEqual({ change: null });
    expect(categoryChange("gt3", filed)).toEqual({ change: null });
  });

  it("emptying a category that was never filed changes nothing", () => {
    expect(categoryChange("", cat())).toBeNull();
    expect(categoryChange("", cat({ value: null, original: null }))).toBeNull();
  });

  it("a car the rules leave without category can be filed", () => {
    expect(categoryChange("#gt3", cat({ value: null, original: null }))).toEqual({ change: "#gt3" });
  });
});

describe("categoriesChange", () => {
  it("a category put in the list is an addition, one taken out a removal", () => {
    expect(categoriesChange(["#rally", "#touge"], ["#rally"])).toEqual({ add: ["#touge"], remove: [] });
    expect(categoriesChange(["#rally"], ["#rally", "#touge"])).toEqual({ add: [], remove: ["#touge"] });
  });

  it("the same list, in another order or spelling, changes nothing", () => {
    expect(categoriesChange(["#touge", "#Rally"], ["#rally", "#touge"])).toBeNull();
    expect(categoriesChange([], [])).toBeNull();
  });

  it("swapping a category is one removal and one addition", () => {
    expect(categoriesChange(["#drift"], ["#touge"])).toEqual({ add: ["#drift"], remove: ["#touge"] });
  });
});

describe("hasCategory", () => {
  it("finds a category whatever its spelling", () => {
    expect(hasCategory(["#rally", "#touge"], "Touge")).toBe(true);
    expect(hasCategory(["#rally"], "#drift")).toBe(false);
    expect(hasCategory([], "#rally")).toBe(false);
  });
});
