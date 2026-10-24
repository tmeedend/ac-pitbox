// A skin on the phone's screen, pure (SPEC-story-mode, Skins: how a frame
// gets its look): CSS custom properties and classes for the phone's root,
// which the template views pick up with no code of the story's own.
//
// The colours are given as the app's own tokens (`--txt`, `--blue`…) on that
// root: every view under it already reads them, so a skin recolours them all
// without one rule per view. A slot is an image laid as a 9-slice
// (`border-image`): its fixed margins are in the image's pixels, drawn for a
// screen 1080 px wide, so on screen they are scaled by the phone's own width
// (`100cqw`, the phone being a size container).
import type { SkinShown } from "../play";

/** The width a skin's images are drawn for: the phone's screen, or the
 * scene's for what plays there (`story_engine::skin`). */
export const REFERENCE_WIDTH = 1080;
export const SCENE_WIDTH = 1920;

/** The tokens each colour of a skin stands for. */
const TOKENS: Record<string, (c: string) => [string, string][]> = {
  // `--bg` too: the scene's text boxes are drawn on it.
  background: (c) => [
    ["--panel2", c],
    ["--bg", c],
  ],
  text: (c) => [
    ["--txt", c],
    ["--txt2", c],
  ],
  muted: (c) => [["--muted", c]],
  accent: (c) => [
    ["--blue", c],
    ["--blue-border", c],
    ["--blue-dim", `color-mix(in srgb, ${c} 30%, transparent)`],
  ],
};

export interface SkinDress {
  /** Declarations for the phone's `style`. */
  style: string;
  /** `skinned`, and `has-<slot>` per slot it fills. */
  classes: string;
}

/** The family a skin's font is registered under: one per file, so two
 * stories with a skin of the same id never share a font. */
export function fontFamily(key: string): string {
  let h = 0;
  for (const c of key) h = (h * 31 + c.charCodeAt(0)) | 0;
  return `pitbox-skin-${(h >>> 0).toString(36)}`;
}

/** `skin` as the phone's root (or the scene's stage) wears it; `url` turns
 * a path inside the story into one the page can load, `font` is the family
 * its font is under, `width` the width its images are drawn for. */
export function skinDress(
  skin: SkinShown | null,
  url: (path: string) => string | null,
  font: string | null,
  width = REFERENCE_WIDTH,
): SkinDress {
  if (!skin) return { style: "", classes: "" };
  const decl: [string, string][] = [];
  for (const [name, colour] of Object.entries(skin.colors)) decl.push(...(TOKENS[name]?.(colour) ?? []));
  const classes = ["skinned"];
  for (const [name, slot] of Object.entries(skin.slots)) {
    const src = url(slot.file);
    if (!src) continue;
    classes.push(`has-${name}`);
    decl.push([`--slot-${name}`, `url("${src}")`]);
    const [t, r, b, l] = slot.slice ?? [0, 0, 0, 0];
    decl.push([`--slot-${name}-slice`, `${t} ${r} ${b} ${l} fill`]);
    const scaled = (n: number) => `calc(${n} * 100cqw / ${width})`;
    decl.push([`--slot-${name}-width`, [t, r, b, l].map(scaled).join(" ")]);
  }
  if (font) decl.push(["--skin-font", `"${font}", system-ui, sans-serif`]);
  return { style: decl.map(([k, v]) => `${k}: ${v}`).join("; "), classes: classes.join(" ") };
}
