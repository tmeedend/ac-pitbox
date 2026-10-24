// The dialogue stage, pure (SPEC-story-mode, In-person dialogues): who stands
// where, with which face, and who speaks, after a given line. The rules, so
// that a dialogue shows the same in the preview and in Story Mode:
//
// - every character present is on stage from the first line; one who speaks
//   without being listed steps in when they first speak;
// - without `#position:`, they stand in the order listed: left, right, then
//   centre; a position given stays for their next lines;
// - a face is `neutral` until a line names an expression, which stays for
//   that character's next lines;
// - the speaker is the line's, or the item's sender for a line without one;
// - a reply chosen, shown only when replaying a past dialogue (the journal),
//   is the player's: nobody on stage speaks it;
// - a line tagged `#leave` is the speaker's last on stage: they are gone from
//   the next line on, until they speak again and step back in.
import type { PlayerStep, Position } from "./types";

export interface OnStage {
  who: string;
  expression: string;
  position: Position;
  speaking: boolean;
}

export interface Stage {
  cast: OnStage[];
  /** The line shown in the text box; none before the first, or while a
   * reply is shown. */
  line: Extract<PlayerStep, { kind: "said" }> | null;
  /** The reply the player chose, when the step shown is one. */
  reply: string | null;
}

const ORDER: Position[] = ["left", "right", "center"];

/** The stage once the said lines of `steps` up to `upTo` (an index into
 * `steps`, included) are played. */
export function stageAt(present: string[], from: string | null, steps: PlayerStep[], upTo: number): Stage {
  let cast: OnStage[] = present.map((who, i) => ({
    who,
    expression: "neutral",
    position: ORDER[Math.min(i, ORDER.length - 1)],
    speaking: false,
  }));
  let line: Stage["line"] = null;
  /** Who said their last line on stage. */
  let leaving: string | null = null;
  for (const step of steps.slice(0, upTo + 1)) {
    if (step.kind !== "said") continue;
    line = step;
    const who = step.speaker ?? from;
    if (leaving !== null) {
      const gone = leaving;
      cast = cast.filter((c) => c.who !== gone);
      leaving = null;
    }
    for (const c of cast) c.speaking = false;
    if (!who) continue;
    let actor = cast.find((c) => c.who === who);
    if (!actor) {
      actor = { who, expression: "neutral", position: ORDER[Math.min(cast.length, ORDER.length - 1)], speaking: false };
      cast.push(actor);
    }
    actor.speaking = true;
    if (step.expression) actor.expression = step.expression;
    if (step.position) actor.position = step.position;
    if (step.leave) leaving = who;
  }
  const shown = steps[upTo];
  if (shown?.kind === "chose") {
    for (const c of cast) c.speaking = false;
    return { cast, line: null, reply: shown.text };
  }
  return { cast, line, reply: null };
}

/** The index of the next said line after `at`, or `null` when there is none:
 * a reply already chosen is passed, as the stage only plays lines. */
export function nextLine(steps: PlayerStep[], at: number): number | null {
  for (let i = at + 1; i < steps.length; i++) if (steps[i].kind === "said") return i;
  return null;
}

/** The index of the said line before `at`, or `null` when there is none. */
export function previousLine(steps: PlayerStep[], at: number): number | null {
  for (let i = Math.min(at, steps.length) - 1; i >= 0; i--) if (steps[i].kind === "said") return i;
  return null;
}

/** How fast the player lets lines go by on their own. */
export type AutoSpeed = "slow" | "normal" | "fast";

export const AUTO_SPEEDS: AutoSpeed[] = ["slow", "normal", "fast"];

const AUTO_FACTOR: Record<AutoSpeed, number> = { slow: 1.6, normal: 1, fast: 0.55 };

/** How long a line stays before auto-advance moves on: a pause, then time to
 * read it, longer for a longer line. */
export function autoDelay(text: string, speed: AutoSpeed): number {
  return Math.round((1000 + 45 * text.length) * AUTO_FACTOR[speed]);
}

/** A said line as fast-forward tells it from another: who says it and what. */
export function lineKey(step: Extract<PlayerStep, { kind: "said" }>): string {
  return `${step.speaker ?? ""}\u0000${step.text}`;
}
