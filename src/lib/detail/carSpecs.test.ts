// Every case below is a real value read off the 311 cars of a live library
// (2026-09-13), not an invented one. The point of the suite is the REJECTIONS:
// a wrong ratio puts a car in a band it does not belong to, in silence.
import { describe, expect, it } from "vitest";
import { carPerf, clampPerfPct, formatRatio, perfBand, readPower, readWeight } from "./carSpecs";

describe("readPower", () => {
  it("accepts bhp, glued or spaced", () => {
    expect(readPower("740bhp")).toBe(740);
    expect(readPower("509 bhp")).toBe(509);
  });

  // 19 cars of the library write `hp`. In an English-language AC mod it is the
  // same word as `bhp`, and no conversion is applied: the factor is 1.
  it("accepts hp as the same unit, with no conversion", () => {
    expect(readPower("500hp")).toBe(500);
    expect(readPower("385hp")).toBe(385);
  });

  // `495+bhp` — the author saying "at least". The number is still a number.
  it("accepts the author's approximation marks", () => {
    expect(readPower("495+bhp")).toBe(495);
    expect(readPower("700+bhp")).toBe(700);
  });

  // The three families of rejection, one case each. See the header of
  // `carSpecs.ts` for why none of them can be converted honestly.
  it("rejects wheel horsepower, which is another quantity", () => {
    expect(readPower("186 whp")).toBeNull();
    expect(readPower("697 wHP")).toBeNull();
  });
  it("rejects metric horsepower rather than deciding what the author meant", () => {
    expect(readPower("325ps")).toBeNull();
    expect(readPower("327л.с.")).toBeNull();
  });
  it("rejects the author's own unknown", () => {
    expect(readPower("--bhp")).toBeNull();
    expect(readPower("")).toBeNull();
    expect(readPower(null)).toBeNull();
  });
});

describe("readWeight", () => {
  it("accepts kg, glued or spaced, footnote mark included", () => {
    expect(readWeight("900kg")).toBe(900);
    expect(readWeight("1383 kg")).toBe(1383);
    expect(readWeight("1525 kg*")).toBe(1525);
  });
  it("rejects the author's own unknown", () => {
    expect(readWeight("--kg")).toBeNull();
  });
  // One car in 311 writes `1195.7` with no unit at all. Kilograms is the
  // overwhelming convention, but the field is free text and pounds would read
  // exactly the same — so it is unreadable, not assumed.
  it("rejects a bare number, which could be pounds", () => {
    expect(readWeight("1195.7")).toBeNull();
  });
  it("rejects a unit it does not know for certain", () => {
    expect(readWeight("1470кг")).toBeNull();
  });
});

describe("carPerf", () => {
  it("yields a ratio only when both sides are readable", () => {
    expect(carPerf("345bhp", "1176kg").ratio).toBeCloseTo(3.409, 3);
    // ferrari_laferrari: a readable power, `--kg` for weight.
    expect(carPerf("963bhp", "--kg").ratio).toBeNull();
    // lk_nissan_180sx_96: a readable weight, metric horsepower.
    expect(carPerf("205ps", "1220kg").ratio).toBeNull();
    // ks_ferrari_sf70h: neither.
    expect(carPerf("--bhp", "--kg").ratio).toBeNull();
  });
  it("keeps the side it could read, so the grid can still show one of them", () => {
    const p = carPerf("963bhp", "--kg");
    expect(p.bhp).toBe(963);
    expect(p.kg).toBeNull();
  });
});

describe("perfBand", () => {
  // The band is expressed on the ratio itself, so a car with a LOWER kg/bhp
  // (a faster one) sits on the low side of the band.
  it("brackets the reference symmetrically", () => {
    const { min, max } = perfBand(4, 15);
    expect(min).toBeCloseTo(3.4, 6);
    expect(max).toBeCloseTo(4.6, 6);
  });
});

describe("clampPerfPct", () => {
  it("snaps to the step and stays inside the offered range", () => {
    expect(clampPerfPct(17)).toBe(15);
    expect(clampPerfPct(1)).toBe(5);
    expect(clampPerfPct(999)).toBe(50);
  });
  // A stored 0 is not a tolerance, it is an empty field: it falls back on the
  // default rather than on a band that can never match anything.
  it("reads a zero as no value rather than a zero-wide band", () => {
    expect(clampPerfPct(0)).toBe(15);
  });
});

describe("formatRatio", () => {
  it("says nothing rather than inventing a number", () => {
    expect(formatRatio(null)).toBe("—");
    expect(formatRatio(3.40925)).toBe("3.41");
  });
});
