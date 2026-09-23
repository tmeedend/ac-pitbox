import { describe, expect, it } from "vitest";
import {
  addAlias,
  aliasesOf,
  attachCountry,
  closestCountry,
  ignoreCountry,
  isCountryCurated,
  restoreCountry,
} from "./countryEdit";

const SHIPPED = { map: { usa: "United States", "u.s.a.": "United States", holland: "Netherlands" } };

describe("country alias editing", () => {
  it("stores a spelling in its compared form, and never one equal to the name", () => {
    const a = addAlias(SHIPPED, "  Nippon ", "Japan");
    expect(a.map.nippon).toBe("Japan");
    expect(addAlias(SHIPPED, "JAPAN", "Japan"), "dead weight, not written").toBe(SHIPPED);
  });

  // Merging must carry the spellings along, or a mod written `Holland` keeps
  // landing on the old name while the others moved.
  it("attaches a country with every spelling that led to it", () => {
    const a = attachCountry(SHIPPED, "Netherlands", "Pays-Bas");
    expect(aliasesOf(a, "Pays-Bas")).toEqual(["holland", "netherlands"]);
    expect(aliasesOf(a, "Netherlands")).toEqual([]);
  });

  it("frees the target when it was itself a spelling of something else", () => {
    const a = attachCountry({ map: { usa: "United States" } }, "United States", "USA");
    expect(a.map).toEqual({ "united states": "USA" });
  });

  it("forgets an ignored value once it is attached", () => {
    const a = attachCountry(ignoreCountry(SHIPPED, "Freedonia"), "Freedonia", "France");
    expect(a.ignored).toEqual([]);
  });

  it("flags a country whose spellings differ from the shipped ones, and restores them", () => {
    const a = addAlias(SHIPPED, "america", "United States");
    expect(isCountryCurated(a, SHIPPED, "United States")).toBe(true);
    expect(isCountryCurated(a, SHIPPED, "Netherlands")).toBe(false);
    const back = restoreCountry(a, SHIPPED, "United States");
    expect(isCountryCurated(back, SHIPPED, "United States")).toBe(false);
    expect(back.map.holland, "other countries untouched").toBe("Netherlands");
  });
});

describe("closestCountry", () => {
  const GAME = ["Switzerland", "Sweden", "Czech Republic", "Germany", "Japan"];

  // TAXO§7.2: a proposal on a close call only.
  it("proposes the game country behind a typo or a truncated name", () => {
    expect(closestCountry("Swizerland", GAME)).toBe("Switzerland");
    expect(closestCountry("Czech", GAME)).toBe("Czech Republic");
  });

  it("proposes nothing rather than a guess", () => {
    expect(closestCountry("Nürburgring Land", GAME)).toBeNull();
    expect(closestCountry("Sw", GAME), "too short to mean anything").toBeNull();
    expect(closestCountry("UK", GAME)).toBeNull();
  });
});
