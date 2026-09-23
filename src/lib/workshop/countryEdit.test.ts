import { describe, expect, it } from "vitest";
import { attachCountry, closestCountry, keyOrigins, keysTo, removeEntry, restoreEntries, setEntry, touches } from "./countryEdit";

const ALIASES = { usa: "United States", "u.s.a.": "United States", holland: "Netherlands" };
const TAGS = { germany: "Germany", usa: "United States" };

describe("country decisions on top of the catalogue", () => {
  it("stores a spelling in its compared form, never one equal to the name, never one the catalogue says", () => {
    expect(setEntry({}, ALIASES, "  Nippon ", "Japan", true).set).toEqual({ nippon: "Japan" });
    expect(setEntry({}, ALIASES, "JAPAN", "Japan", true), "dead weight, not written").toEqual({});
    expect(setEntry({}, TAGS, "japan", "Japan").set, "a tag named like its country is meaningful").toEqual({ japan: "Japan" });
    expect(setEntry({ removed: ["usa"] }, ALIASES, "USA", "United States"), "back on the catalogue").toEqual({
      set: {},
      removed: [],
    });
  });

  // An absence would let the next catalogue bring the entry back.
  it("removes a shipped spelling with a tombstone", () => {
    expect(removeEntry({}, ALIASES, "U.S.A.").removed).toEqual(["u.s.a."]);
    expect(removeEntry({ set: { nippon: "Japan" } }, ALIASES, "nippon")).toEqual({ set: {}, removed: [] });
  });

  // Merging carries the spellings AND the tags along, or a mod written
  // `Holland` keeps landing on the old name while the others moved.
  it("attaches a country with every spelling and tag leading to it", () => {
    const out = attachCountry(
      { overlay: {}, catalog: ALIASES, effective: ALIASES },
      { overlay: {}, catalog: TAGS, effective: TAGS },
      "United States",
      "USA",
    );
    expect(out.aliases.set).toEqual({ "u.s.a.": "USA", "united states": "USA" });
    expect(out.aliases.removed, "the target was a spelling: it is a name now").toEqual(["usa"]);
    expect(out.tags.set).toEqual({ usa: "USA" });
  });

  it("restores a country by removing every decision touching it", () => {
    let o = setEntry({}, ALIASES, "america", "United States");
    o = removeEntry(o, ALIASES, "usa");
    o = setEntry(o, ALIASES, "nippon", "Japan");
    expect(touches(o, ALIASES, "United States")).toBe(true);
    const back = restoreEntries(o, ALIASES, "United States");
    expect(touches(back, ALIASES, "United States")).toBe(false);
    expect(back.set, "other countries untouched").toEqual({ nippon: "Japan" });
  });

  it("tells shipped spellings from added ones, and keeps the removed ones in view", () => {
    const eff = { "u.s.a.": "United States", america: "United States", holland: "Netherlands" };
    expect(keyOrigins(eff, ALIASES, "United States")).toEqual({
      keys: [
        { key: "america", mine: true },
        { key: "u.s.a.", mine: false },
      ],
      gone: ["usa"],
    });
  });

  it("lists the keys leading to a name", () => {
    expect(keysTo(ALIASES, "United States")).toEqual(["u.s.a.", "usa"]);
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
