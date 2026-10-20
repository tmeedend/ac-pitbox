// The thread template, pure (SPEC-story-mode, Content types, templates and
// frames): a conversation as bubbles. The other side's lines on the left,
// the player's replies on the right; a speaker's name shows on the first of
// their bubbles in a row, and only when someone else than the sender speaks -
// a one-to-one thread needs no names.
import type { PlayerStep } from "./types";

export interface Bubble {
  side: "them" | "me";
  text: string;
  /** The speaker to name above it, if any. */
  name: string | null;
}

export function bubbles(steps: PlayerStep[], from: string | null, names: Record<string, string>): Bubble[] {
  const group = steps.some((s) => s.kind === "said" && s.speaker && s.speaker !== from);
  let last: string | null = null;
  return steps.map((s) => {
    if (s.kind === "chose") {
      last = null;
      return { side: "me", text: s.text, name: null };
    }
    const who = s.speaker ?? from;
    const name = group && who && who !== last ? (names[who] ?? who) : null;
    last = who;
    return { side: "them", text: s.text, name };
  });
}
