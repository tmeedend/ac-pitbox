// The header form's decisions (name, brand, year): what a typed text changes.
// A rule per test, since a wrong one here silently pins or loses a correction.
import { describe, expect, it } from "vitest";
import { parseYear, textChange, yearChange, type IdentityField } from "./identityEdit";

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
