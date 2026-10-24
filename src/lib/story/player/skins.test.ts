import { describe, expect, it } from "vitest";
import type { SkinShown } from "../play";
import { fontFamily, skinDress } from "./skins";

const skin: SkinShown = {
  id: "chat",
  colors: { accent: "#0a84ff", text: "#eeeeee", glow: "#ffffff" },
  font: "frames/skins/chat/Inter.ttf",
  slots: {
    bubble_out: { file: "frames/skins/chat/out.png", slice: [40, 30, 40, 30] },
    background: { file: "frames/skins/chat/bg.webp", slice: null },
    reply: { file: "" },
  },
};

describe("skinDress", () => {
  // No skin: Pit Box's theme, nothing set.
  it("sets nothing without a skin", () => {
    expect(skinDress(null, (p) => p, null)).toEqual({ style: "", classes: "" });
  });

  // The colours stand for the app's tokens, a colour the app has no token
  // for is left out; each slot is marked and laid as a 9-slice whose
  // margins scale with the phone's width; the font comes first in the
  // family list.
  it("gives the colours as tokens, the slots as 9-slices, and the font", () => {
    const d = skinDress(skin, (p) => (p ? `u/${p}` : null), "fam");
    expect(d.classes).toBe("skinned has-bubble_out has-background");
    expect(d.style).toContain("--blue: #0a84ff");
    expect(d.style).toContain("--blue-dim: color-mix(in srgb, #0a84ff 30%, transparent)");
    expect(d.style).toContain("--txt: #eeeeee");
    expect(d.style).not.toContain("glow");
    expect(d.style).toContain('--slot-bubble_out: url("u/frames/skins/chat/out.png")');
    expect(d.style).toContain("--slot-bubble_out-slice: 40 30 40 30 fill");
    expect(d.style).toContain("--slot-bubble_out-width: calc(40 * 100cqw / 1080) calc(30 * 100cqw / 1080)");
    expect(d.style).toContain("--slot-background-slice: 0 0 0 0 fill");
    expect(d.style).toContain('--skin-font: "fam", system-ui, sans-serif');
  });
});

describe("fontFamily", () => {
  it("names a font file once, apart from another story's of the same name", () => {
    expect(fontFamily("a|frames/skins/chat/Inter.ttf")).toBe(fontFamily("a|frames/skins/chat/Inter.ttf"));
    expect(fontFamily("a|frames/skins/chat/Inter.ttf")).not.toBe(fontFamily("b|frames/skins/chat/Inter.ttf"));
    expect(fontFamily("x")).toMatch(/^pitbox-skin-[a-z0-9]+$/);
  });
});
