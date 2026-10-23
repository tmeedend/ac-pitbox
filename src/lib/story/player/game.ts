// A game's screen, pure (SPEC-story-mode, Story Mode: player side): how the
// items the engine delivered are laid out - the phone's message threads, one
// per sender, and the dialogue the scene plays now.
import { criteriaOf } from "../events";
import type { SeenItem } from "../play";
import { appOf } from "./apps";
import { lineKey } from "./stage";
import type { PlayerChoice, PlayerStep } from "./types";

/** A sender's messages, as one thread of the messaging app. */
export interface Thread {
  /** The sender; `null` for messages nobody signs. */
  from: string | null;
  /** Every line of every message, in the order they came. */
  steps: PlayerStep[];
  /** What the open message offers now. */
  choices: PlayerChoice[];
  /** The message those choices answer. */
  replyTo: string | null;
  /** Messages not opened yet. */
  unseen: string[];
  /** When the last message came. */
  date: string;
  clock: string;
}

/** The message threads, the latest first, as a messaging app lists them. */
export function phoneThreads(items: SeenItem[]): Thread[] {
  const threads: Thread[] = [];
  for (const item of items) {
    if (item.kind !== "message") continue;
    let thread = threads.find((t) => t.from === item.from);
    if (!thread) {
      thread = { from: item.from, steps: [], choices: [], replyTo: null, unseen: [], date: "", clock: "" };
      threads.push(thread);
    }
    thread.steps.push(...item.steps);
    if (item.open) {
      thread.choices = item.choices;
      thread.replyTo = item.content;
    }
    if (!item.seen && !thread.unseen.includes(item.content)) thread.unseen.push(item.content);
    thread.date = item.date;
    thread.clock = item.clock;
  }
  const order = (t: Thread) => `${t.date} ${t.clock}`;
  return threads.sort((a, b) => order(b).localeCompare(order(a)));
}

/** The dialogue the scene plays, as an index into `items`: the latest one,
 * while it waits for a reply or until the player has read it to its end (and
 * closed it, which marks it seen). An index rather than the item, so that a
 * new delivery of the same dialogue is told from the last. `null` when none. */
export function currentDialogue(items: SeenItem[]): number | null {
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].kind !== "dialogue") continue;
    return items[i].open || !items[i].seen ? i : null;
  }
  return null;
}

/** The dialogues the journal rereads, latest first, as indexes into
 * `items`: every one played, except the one the scene plays now - it is not
 * past yet. A dialogue that said nothing has nothing to reread. */
export function pastDialogues(items: SeenItem[]): number[] {
  const current = currentDialogue(items);
  const past: number[] = [];
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (item.kind === "dialogue" && i !== current && item.steps.length > 0) past.push(i);
  }
  return past;
}

/** Items of the types the phone has no app for yet: listed by name, not
 * shown. A dialogue is played on the scene. */
export function withoutTemplate(items: SeenItem[]): SeenItem[] {
  return items.filter((i) => appOf(i.kind) === null && i.kind !== "dialogue");
}

/** One line of an event's goal, as an i18n key and its values. */
export interface Goal {
  key: string;
  values: Record<string, string | number>;
}

/** What success takes, in the player's words: one line per criterion, the
 * drivers to beat named by `driverName`. Without a criterion, finishing is
 * enough. */
export function goals(success: string[], driverName: (id: string) => string): Goal[] {
  const c = criteriaOf(success);
  const out: Goal[] = [];
  if (c.position !== null) out.push({ key: "story.play.goalPosition", values: { n: c.position } });
  if (c.finished) out.push({ key: "story.play.goalFinished", values: {} });
  if (c.lapUnder) out.push({ key: "story.play.goalLap", values: { time: c.lapUnder } });
  for (const d of c.aheadOf) out.push({ key: "story.play.goalAhead", values: { name: driverName(d) } });
  for (const other of c.others) out.push({ key: "story.play.goalOther", values: { text: other } });
  if (out.length === 0) out.push({ key: "story.play.goalFinish", values: {} });
  return out;
}

/** The lines of item `index` the player has already read in an earlier
 * delivery of the same item, as `lineKey`s: what fast-forward goes through
 * when a conversation comes back. */
export function readBefore(items: SeenItem[], index: number): Set<string> {
  const read = new Set<string>();
  const content = items[index]?.content;
  for (const item of items.slice(0, index)) {
    if (item.content !== content) continue;
    for (const s of item.steps) if (s.kind === "said") read.add(lineKey(s));
  }
  return read;
}

/** One line of the dialogue log. */
export interface LogLine {
  step: PlayerStep;
  /** The item's sender: who says a line without a speaker. */
  from: string | null;
}

/** The dialogue log (SPEC-story-mode, In-person dialogues): every line of the
 * dialogues played, in order, the replies chosen included; for the dialogue
 * under way, item `current`, only up to the step shown, `at`. */
export function dialogueLog(items: SeenItem[], current: number | null, at: number): LogLine[] {
  const out: LogLine[] = [];
  items.forEach((item, i) => {
    if (item.kind !== "dialogue") return;
    const steps = i === current ? item.steps.slice(0, at + 1) : item.steps;
    for (const step of steps) out.push({ step, from: item.from });
  });
  return out;
}
