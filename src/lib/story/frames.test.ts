import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => `<${key}>` }));

import { FRAMES, frameLabel, framesWith } from "./frames";

describe("framesWith", () => {
  it("offers the known frames, and keeps an unknown one written by hand", () => {
    expect(framesWith(undefined)).toEqual([...FRAMES]);
    expect(framesWith("phone")).toEqual([...FRAMES]);
    expect(framesWith("pager")).toEqual(["pager", ...FRAMES]);
  });
});

describe("frameLabel", () => {
  it("names a known frame, and shows an unknown one as written", () => {
    expect(frameLabel("none")).toBe("<story.frame.none>");
    expect(frameLabel("pager")).toBe("pager");
  });
});
