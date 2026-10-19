import { describe, expect, it } from "vitest";
import { fileCount, isName, nameFrom, nameProblem, parseList, usagesOf, variableNames } from "./world";
import type { NameUsages } from "./studio";

describe("names", () => {
  // The engine refuses a rename to anything else (story_engine::refs).
  it("start with a letter and hold letters, digits and _", () => {
    for (const ok of ["friend", "garage_visits", "Rival2"]) expect(isName(ok), ok).toBe(true);
    for (const bad of ["", "2fast", "_x", "a b", "a-b", "é", "true", "false"]) expect(isName(bad), bad).toBe(false);
  });

  it("say why one cannot be given", () => {
    expect(nameProblem("a b", [])).toBe("story.world.nameInvalid");
    expect(nameProblem("friend", ["friend"])).toBe("story.world.nameTaken");
    expect(nameProblem("pal", ["friend"])).toBeNull();
  });

  // "À la création d'un personnage, l'identifiant est tiré du nom."
  it("are built from what the author typed, unique", () => {
    expect(nameFrom("Un ami", [], "character")).toBe("un_ami");
    expect(nameFrom("Élodie  (sœur)", [], "character")).toBe("elodie_s_ur");
    expect(nameFrom("Un ami", ["un_ami", "un_ami_2"], "character")).toBe("un_ami_3");
    expect(nameFrom("2 CV", [], "character")).toBe("character_2_cv");
    expect(nameFrom("???", [], "character")).toBe("character");
    expect(nameFrom("True", [], "flag")).toBe("flag_true");
  });

  it("are shared by flags and counters", () => {
    expect(variableNames({ flags: ["met"], counters: { visits: 0 } })).toEqual(["met", "visits"]);
    expect(variableNames({})).toEqual([]);
  });
});

describe("usages", () => {
  const all: NameUsages[] = [
    {
      kind: "counter",
      id: "visits",
      usages: [
        { file: "flow/01.yaml", line: 3, node: "a", item: null, text: "x" },
        { file: "flow/01.yaml", line: 9, node: "b", item: null, text: "y" },
        { file: "content/x.story", line: 4, node: null, item: "m", text: "z" },
      ],
    },
    { kind: "flag", id: "visits", usages: [] },
  ];

  it("are found by kind and name", () => {
    expect(usagesOf(all, "counter", "visits")).toHaveLength(3);
    expect(usagesOf(all, "flag", "visits")).toHaveLength(0);
    expect(usagesOf(all, "character", "nobody")).toHaveLength(0);
  });

  it("count the files they are in", () => {
    expect(fileCount(usagesOf(all, "counter", "visits"))).toBe(2);
  });
});

it("a comma-separated list drops blanks", () => {
  expect(parseList(" A, B ,, ")).toEqual(["A", "B"]);
});
