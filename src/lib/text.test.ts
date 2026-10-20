import { describe, expect, it } from "vitest";
import { editDistance, fold } from "./text";

describe("fold", () => {
  // Rule: case and accents never make two names differ.
  it("lowercases and drops the accents, nothing else", () => {
    expect(fold("Škoda Citroën")).toBe("skoda citroen");
    expect(fold("Mercedes-Benz 300 SL"), "punctuation and digits are the caller's").toBe("mercedes-benz 300 sl");
  });
});

describe("editDistance", () => {
  it("counts insertions, deletions and substitutions", () => {
    expect(editDistance("porsche", "porsche")).toBe(0);
    expect(editDistance("", "abc"), "three insertions").toBe(3);
    expect(editDistance("abc", ""), "three deletions").toBe(3);
    expect(editDistance("kitten", "sitting")).toBe(3);
    expect(editDistance("lamborgini", "lamborghini"), "one letter forgotten").toBe(1);
  });
});
