import { describe, expect, it } from "vitest";
import { highlightParts } from "./highlight";

const marked = (text: string, q: string) =>
  highlightParts(text, q)
    .filter((p) => p.hit)
    .map((p) => p.text);

describe("search highlight (DOSSIER§7.3)", () => {
  it("marks every word, whatever the case", () => {
    expect(marked("RSS GTM Lanzo V10", "lanzo gtm")).toEqual(["GTM", "Lanzo"]);
  });

  it("finds an accented name from a plain query, and marks it as displayed", () => {
    expect(marked("Clément.dds", "clement")).toEqual(["Clément"]);
  });

  it("reads a separator as one path pattern, either slash", () => {
    expect(marked(String.raw`content\cars\x\skins\red`, "skins/red")).toEqual([String.raw`skins\red`]);
  });

  it("gives the text back whole when nothing matches", () => {
    expect(highlightParts("data.acd", "zonda")).toEqual([{ text: "data.acd", hit: false }]);
  });
});
