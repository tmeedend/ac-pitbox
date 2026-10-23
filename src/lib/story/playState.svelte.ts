// The game open in Story Mode, and how a move reaches it. One move at a
// time: each command resumes the save and writes it back, so two moves in
// flight would play on the same save and the second would undo the first.
// A refused move leaves the game as it was, with the reason on screen.
import { errorText } from "$lib/errors";
import { keepReadingAt, newGame, playMove, resumeGame, type Game, type Move, type Reading, type Source } from "./play";

export const play = $state<{
  source: Source | null;
  game: Game | null;
  /** Why the last move or start was refused. */
  problem: string | null;
  busy: boolean;
}>({ source: null, game: null, problem: null, busy: false });

let queue: Promise<void> = Promise.resolve();
/** The moves queued or in flight, as values: the same move asked twice (a
 * double click, a thread marking its messages seen at every update) is
 * played once. */
const pending = new Set<string>();
/** Moves on with every game opened or closed: a move answered after the
 * player left, or opened another story, is dropped rather than shown. */
let session = 0;

/** Opens a story: a new game (replacing the one under way) or the game as
 * it was left. Resolves to why it was refused, or `null`. */
export async function openGame(source: Source, fresh: boolean): Promise<string | null> {
  try {
    const game = await (fresh ? newGame(source) : resumeGame(source));
    session++;
    play.game = game;
    play.source = source;
    play.problem = null;
    return null;
  } catch (e) {
    return errorText(e);
  }
}

export function closeGame(): void {
  flushReading();
  session++;
  play.game = null;
  play.source = null;
  play.problem = null;
}

/** Plays a move after those already queued, and shows where the game now
 * stands. */
export function move(m: Move): Promise<void> {
  const key = JSON.stringify(m);
  if (pending.has(key)) return queue;
  pending.add(key);
  const mine = session;
  const run = queue.then(async () => {
    const source = play.source;
    if (!source || mine !== session) return;
    play.busy = true;
    try {
      const game = await playMove(source, m);
      if (mine !== session) return;
      play.game = game;
      play.problem = null;
    } catch (e) {
      if (mine === session) play.problem = errorText(e);
    } finally {
      play.busy = false;
      pending.delete(key);
    }
  });
  // Already caught above; kept so that one failure never stalls the queue.
  queue = run.catch((e) => console.warn("story mode: move failed", e));
  return run;
}

/** The line read, not written yet: a click every second need not rewrite
 * the file every second. */
let reading: { source: Source; at: Reading } | null = null;
let readingTimer: ReturnType<typeof setTimeout> | undefined;

/** Keeps where the dialogue under way is read, a moment after the last line
 * moved - at once when the game is left (`closeGame`). */
export function keepReading(at: Reading): void {
  if (!play.source) return;
  reading = { source: play.source, at };
  clearTimeout(readingTimer);
  readingTimer = setTimeout(flushReading, 400);
}

function flushReading(): void {
  clearTimeout(readingTimer);
  const due = reading;
  reading = null;
  if (!due) return;
  // Not a setting the player made: a failure costs a dialogue started again
  // from its first line. Logged by the backend; retried at the next line.
  keepReadingAt(due.source, due.at).catch((e) => console.warn("story mode: reading position not kept", e));
}
