import { describe, expect, it } from "vitest";
import { EMPTY_OUTFIT, entryFor, parseEntry } from "./outfitEntry";

// Rule (SESSION§5): the outfit chosen on a car wins over the default outfit
// of its class — including the choice of wearing nothing but the original.

describe("parseEntry", () => {
  it("reads a chosen outfit", () => {
    expect(parseEntry('{"body":"mrbean","suit":null}')).toEqual({ ...EMPTY_OUTFIT, body: "mrbean" });
  });

  it("reads an empty entry as nothing chosen, as every file written so far", () => {
    expect(parseEntry('{"body":null,"suit":null,"gloves":null,"helmet":null}')).toBeNull();
    expect(parseEntry(null)).toBeNull();
    expect(parseEntry("not json")).toBeNull();
  });

  it("reads a marked empty entry as the original chosen on purpose", () => {
    expect(parseEntry('{"body":null,"suit":null,"gloves":null,"helmet":null,"origin":true}')).toEqual(EMPTY_OUTFIT);
  });
});

describe("entryFor", () => {
  it("stores a chosen outfit as is", () => {
    const outfit = { ...EMPTY_OUTFIT, helmet: "HELMET_2012/red" };
    expect(parseEntry(entryFor(outfit, true))).toEqual(outfit);
    expect(parseEntry(entryFor(outfit, false))).toEqual(outfit);
  });

  it("removes an empty choice when no default outfit would replace it", () => {
    expect(entryFor(EMPTY_OUTFIT, false)).toBeNull();
  });

  it("keeps an empty choice when a default outfit would dress the car", () => {
    // Real bug: "back to the car's body" removed the entry, and the street
    // default (a body) came straight back — the button seemed dead.
    const stored = entryFor(EMPTY_OUTFIT, true);
    expect(stored).not.toBeNull();
    expect(parseEntry(stored)).toEqual(EMPTY_OUTFIT);
  });
});
