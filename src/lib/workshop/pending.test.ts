import { describe, expect, it } from "vitest";
import { folderName, groupPending, type PendingFolder } from "./pending";

function folder(id: string, relPath: string, over: Partial<PendingFolder> = {}): PendingFolder {
  return {
    id,
    archive: `${id}.7z`,
    rel_path: relPath,
    owner_id: id,
    owner_kind: "cars",
    shape: "unknown",
    title: null,
    description: null,
    readme: null,
    skin_target: null,
    replaced: 0,
    file_count: 3,
    size_bytes: 1000,
    suggestion: "",
    actions: ["resources", "discard"],
    ...over,
  };
}

// Rule (§4.6ter): the same question asked by twenty mods is asked once.
describe("groupPending", () => {
  it("groups folders the author named the same, whatever wraps them", () => {
    const groups = groupPending(
      [
        folder("a", "VRC Car A v1.2/Wallpapers"),
        folder("b", "vrc car b/wallpapers"),
        folder("c", "VRC Car A v1.2/Optional Textures"),
      ],
      new Set(),
    );
    expect(groups.map((g) => g.folders.map((f) => f.id))).toEqual([["a", "b"], ["c"]]);
  });

  it("never groups a folder that replaces base-game files", () => {
    const groups = groupPending(
      [folder("a", "Patch", { replaced: 2 }), folder("b", "Patch", { replaced: 2 })],
      new Set(),
    );
    expect(groups).toHaveLength(2);
  });

  it("keeps apart folders that do not offer the same answers", () => {
    const groups = groupPending(
      [folder("a", "Extras", { actions: ["game", "resources", "discard"] }), folder("b", "Extras")],
      new Set(),
    );
    expect(groups).toHaveLength(2);
  });

  it("asks a split group folder by folder", () => {
    const folders = [folder("a", "Wallpapers"), folder("b", "Wallpapers")];
    const [g] = groupPending(folders, new Set());
    const split = groupPending(folders, new Set([g.key]));
    expect(split.map((x) => x.folders.length)).toEqual([1, 1]);
  });
});

describe("folderName", () => {
  it("is the last segment of the archive path", () => {
    expect(folderName("VRC Car/Wallpapers/")).toBe("Wallpapers");
    expect(folderName("Wallpapers")).toBe("Wallpapers");
  });
});
