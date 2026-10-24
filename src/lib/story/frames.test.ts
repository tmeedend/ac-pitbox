import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => `<${key}>` }));

import { FRAMES, frameLabel, framesWith } from "./frames";

describe("framesWith", () => {
  it("offers the known frames, and keeps an unknown one written by hand", () => {
    expect(framesWith(undefined)).toEqual([...FRAMES]);
    expect(framesWith("phone")).toEqual([...FRAMES]);
    expect(framesWith("pager")).toEqual(["pager", ...FRAMES]);
  });

  // The story's own frames first; one named like a shipped frame is the
  // story's, offered once.
  it("offers the story's own frames first", () => {
    const own = {
      frames: [
        { id: "basic", kind: "phone", apps: [] },
        { id: "phone", kind: "phone", name: "Mine", apps: [] },
      ],
    };
    expect(framesWith(undefined, own)).toEqual(["basic", "phone", "none"]);
    expect(frameLabel("phone", own)).toBe("Mine");
    expect(frameLabel("basic", own), "unnamed: its id").toBe("basic");
  });
});

describe("frameLabel", () => {
  it("names a known frame, and shows an unknown one as written", () => {
    expect(frameLabel("none")).toBe("<story.frame.none>");
    expect(frameLabel("pager")).toBe("pager");
  });
});
