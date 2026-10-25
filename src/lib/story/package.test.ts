import { describe, expect, it } from "vitest";
import { lineFor, missingLabels, packageName, registerGroups, setText, splitFiles } from "./package";
import type { Credit, Holding, PackageView } from "./studio";

const line = (files: string[]): Credit => ({ files, author: "A", licence: "CC0", permission: "not_needed" });
const noContent = { file: "package/manifest.yaml", stamp: "", value: [], error: null };

describe("registerGroups", () => {
  // A register a file needs is shown before it exists, so its first line
  // can create it; media first, then the skins by name.
  it("lists the registers the story has and those it needs", () => {
    const view: PackageView = {
      registers: [{ file: "frames/skins/b/credits.yaml", stamp: "s", value: [line(["x.png"])], error: null }],
      content_register: noContent,
      content: [],
      register: {
        content: [],
        diagnostics: [],
        coverage: [
          { file: "frames/skins/b/x.png", register: "frames/skins/b/credits.yaml", line: 0 },
          { file: "frames/skins/b/y.png", register: "frames/skins/b/credits.yaml", line: 0 },
          { file: "media/scenes/a.webp", register: "media/credits.yaml", line: null },
        ],
      },
    };
    const groups = registerGroups(view);
    expect(groups.map((g) => g.file)).toEqual(["media/credits.yaml", "frames/skins/b/credits.yaml"]);
    expect(groups[0]).toMatchObject({ skin: null, lines: [], uncredited: ["scenes/a.webp"] });
    expect(groups[1]).toMatchObject({ skin: "b", named: [2], uncredited: [] });
  });

  it("keeps a register that does not read apart", () => {
    const view: PackageView = {
      registers: [{ file: "media/credits.yaml", stamp: "s", value: null, error: "bad" }],
      content_register: noContent,
      content: [],
      register: { diagnostics: [], coverage: [], content: [] },
    };
    expect(registerGroups(view)[0].lines).toBeNull();
  });
});

describe("a line's fields", () => {
  it("reads files as a comma-separated list", () => {
    expect(splitFiles(" a.png, icons/*.png ,, ")).toEqual(["a.png", "icons/*.png"]);
  });

  it("leaves an emptied field out of the file", () => {
    const l = lineFor("a.png");
    expect(l).toEqual({ files: ["a.png"], permission: "not_asked" });
    setText(l, "author", "  Someone ");
    setText(l, "proof", "   ");
    expect(l).toEqual({ files: ["a.png"], permission: "not_asked", author: "Someone" });
    setText(l, "author", "");
    expect("author" in l).toBe(false);
  });

  it("names what a story lacks by the DLC to buy, once", () => {
    const held = (id: string, official: boolean, dlc: string | null): Holding => ({
      kind: "car",
      id,
      folder: null,
      official,
      dlc,
    });
    expect(missingLabels([held("ks_a", true, "Pack 1"), held("ks_b", true, "Pack 1"), held("mod_c", false, null)])).toEqual(
      ["Pack 1", "mod_c"],
    );
    expect(missingLabels([held("abarth500", true, null)])).toEqual(["abarth500"]);
  });

  it("names the package after the story and its version", () => {
    expect(packageName("race", "2")).toBe("race-2.pitstory");
  });
});
