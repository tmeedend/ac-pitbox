// The frames an author can pick (SPEC-story-mode, Content types, templates
// and frames). None is a default: every list here is offered with nothing
// chosen, and the phone is one frame among others.
import { t } from "$lib/i18n/index.svelte";
import type { Frame } from "./studio";

/** The frames the engine knows: `story_engine::frame::BUILT_IN`, plus `none`
 * (a visual novel). Keep aligned with the Rust table. */
export const FRAMES: readonly Frame[] = ["phone", "none"];

/** i18n keys written out in full, for the key check (`check-conventions.mjs`). */
const FRAME_LABEL: Record<string, string> = {
  phone: "story.frame.phone",
  none: "story.frame.none",
};

/** A frame's name in the editor; one the engine does not know (typed by
 * hand) shows as written, for the checks to report. */
export function frameLabel(frame: Frame): string {
  const key = FRAME_LABEL[frame];
  return key ? t(key) : frame;
}

/** The frames to offer, `current` included when it is not one of them, so
 * a select never hides what the file says. */
export function framesWith(current: Frame | undefined | null): Frame[] {
  return current && !FRAMES.includes(current) ? [current, ...FRAMES] : [...FRAMES];
}
