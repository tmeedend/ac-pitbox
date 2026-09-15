import { describe, it, expect } from "vitest";
import { withoutBrand } from "./displayName";

// Brand-prefix removal (SPEC §7.4). The two rules of `afterPrefix` contradict
// each other on purpose — comparison ignores hyphens, the cut only falls on a
// space — and that is exactly what no amount of typing catches: only running
// the cases does.
describe("withoutBrand", () => {
  it("removes the brand when the name repeats it", () => {
    expect(withoutBrand("Nissan Skyline GT-R R34", "Nissan")).toBe("Skyline GT-R R34");
  });

  it("keeps the whole name when removal would leave nothing", () => {
    // A card with no label is not denser, it has lost its identity.
    expect(withoutBrand("Ferrari", "Ferrari")).toBe("Ferrari");
  });

  it("cuts on a space, never inside a hyphenated brand", () => {
    // Rule 1: "Mercedes" must not turn "Mercedes-Benz SLS" into "Benz SLS".
    expect(withoutBrand("Mercedes-Benz SLS AMG", "Mercedes")).toBe("Mercedes-Benz SLS AMG");
  });

  it("compares without hyphens, so a spaced spelling is still recognised", () => {
    // Rule 2, the other half: the brand is hyphenated, the mod's name is not.
    expect(withoutBrand("Mercedes Benz SLS AMG", "Mercedes-Benz")).toBe("SLS AMG");
  });

  it("prefers the longest alias, so AMG stays part of the model", () => {
    // "mercedes amg" must win over "mercedes", otherwise "Mercedes AMG GT"
    // loses its identity and reads "AMG GT".
    expect(withoutBrand("Mercedes AMG GT", "Mercedes-Benz")).toBe("GT");
  });

  it("recognises the short forms listed as aliases", () => {
    expect(withoutBrand("Alfa Romeo 155 TI", "Alfa Romeo")).toBe("155 TI");
    expect(withoutBrand("Alfa 155 TI", "Alfa Romeo")).toBe("155 TI");
    expect(withoutBrand("VW Golf GTI", "Volkswagen")).toBe("Golf GTI");
  });

  it("leaves the name alone when the brand is unknown or absent", () => {
    expect(withoutBrand("Skyline GT-R R34", null)).toBe("Skyline GT-R R34");
    expect(withoutBrand("Skyline GT-R R34", "   ")).toBe("Skyline GT-R R34");
    expect(withoutBrand("RSS Formula Hybrid 2021", "Ferrari")).toBe("RSS Formula Hybrid 2021");
  });

  it("only removes a prefix, never a brand quoted mid-name", () => {
    expect(withoutBrand("Tribute to Ferrari F40", "Ferrari")).toBe("Tribute to Ferrari F40");
  });

  it("is case-insensitive on both sides", () => {
    expect(withoutBrand("PORSCHE 911 GT3 RS", "porsche")).toBe("911 GT3 RS");
  });
});
