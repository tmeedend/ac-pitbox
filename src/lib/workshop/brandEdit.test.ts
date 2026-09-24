// Brand merges (TAXO§7): proposed on proximity, never made silently, and an
// ignored proposal does not come back.
import { describe, expect, it } from "vitest";
import { brandProposals, mergeBrand, mergeKey } from "./brandEdit";

describe("brand proposals", () => {
  it("proposes a name inside another and a typo, into the brand the library uses most", () => {
    const p = brandProposals(
      [
        { name: "Lotus", cars: 21 },
        { name: "Lotus Classic Cars", cars: 4 },
        { name: "Alfa", cars: 4 },
        { name: "Alfa Romeo", cars: 38 },
        { name: "Mercedes-Benz", cars: 47 },
        { name: "Mercedes", cars: 12 },
        { name: "Lamborgini", cars: 1 },
        { name: "Lamborghini", cars: 9 },
        { name: "BMW", cars: 20 },
        { name: "BAR", cars: 2 },
      ],
      [],
    );
    expect(p.map((x) => `${x.from}>${x.to}`)).toEqual([
      "Mercedes>Mercedes-Benz",
      "Alfa>Alfa Romeo",
      "Lotus Classic Cars>Lotus",
      "Lamborgini>Lamborghini",
    ]);
  });

  it("an ignored pair does not come back, in either direction", () => {
    const brands = [
      { name: "Nissan", cars: 30 },
      { name: "Nissan Nismo", cars: 3 },
    ];
    expect(brandProposals(brands, [mergeKey("Nissan Nismo", "Nissan")])).toEqual([]);
    expect(brandProposals(brands, [mergeKey("Nissan", "Nissan Nismo")])).toEqual([]);
  });

  it("a tie goes to the fuller name", () => {
    const p = brandProposals(
      [
        { name: "Alfa", cars: 3 },
        { name: "Alfa Romeo", cars: 3 },
      ],
      [],
    );
    expect(p).toEqual([{ from: "Alfa", to: "Alfa Romeo", fromCars: 3, toCars: 3 }]);
  });
});

describe("merging a brand", () => {
  it("its spellings follow it, and it becomes one", () => {
    const effective = { "alfa romeo spa": "Alfa" };
    const o = mergeBrand({}, {}, effective, "Alfa", "Alfa Romeo");
    expect(o.set).toEqual({ "alfa romeo spa": "Alfa Romeo", alfa: "Alfa Romeo" });
  });

  it("renaming onto a name that was a spelling makes it a name", () => {
    const o = mergeBrand({ set: { lotus: "Lotus Cars" } }, {}, { lotus: "Lotus Cars" }, "Lotus Cars", "Lotus");
    expect(o.set).toEqual({ "lotus cars": "Lotus" });
  });
});
