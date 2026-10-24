// The frames an author can pick (SPEC-story-mode, Content types, templates
// and frames). None is a default: every list here is offered with nothing
// chosen, and the phone is one frame among others. The story's own frames
// (`frames/frames.yaml`) come first; one named like a shipped frame
// replaces it.
import { t } from "$lib/i18n/index.svelte";
import type { Frame, FramesFile } from "./studio";

/** The frames Pit Box ships: `story_engine::frame::BUILT_IN`, plus `none`
 * (a visual novel). Keep aligned with the Rust table. */
export const FRAMES: readonly Frame[] = ["phone", "none"];

/** i18n keys written out in full, for the key check (`check-conventions.mjs`). */
const FRAME_LABEL: Record<string, string> = {
  phone: "story.frame.phone",
  none: "story.frame.none",
};

/** A frame's name in the editor: the story's for one it declares, Pit Box's
 * for one it ships; one nobody knows (typed by hand) shows as written, for
 * the checks to report. */
export function frameLabel(frame: Frame, declared: FramesFile | null = null): string {
  const own = declared?.frames.find((f) => f.id === frame);
  if (own) return own.name ?? own.id;
  const key = FRAME_LABEL[frame];
  return key ? t(key) : frame;
}

/** The frames to offer: the story's own, then Pit Box's, `current` included
 * when it is none of them, so a select never hides what the file says. */
export function framesWith(current: Frame | undefined | null, declared: FramesFile | null = null): Frame[] {
  const own = declared?.frames.map((f) => f.id) ?? [];
  const all = [...own, ...FRAMES.filter((f) => !own.includes(f))];
  return current && !all.includes(current) ? [current, ...all] : all;
}
