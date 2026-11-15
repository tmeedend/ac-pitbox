// Thumbnails an older engine version rendered, still shown (GRILLE§6.5).
//
// A new version of the converter or of the renderer used to retire every
// image at once, and the next launch redid the whole library — four hundred
// cars at every start while the engine was being worked on (reported). The
// older image now stays on screen, and the library is redone only once the
// user says so. This module only keeps the count and the answer; the render
// queue (`gridThumbs.svelte.ts`) asks it before redoing an image.

/** One stale thumbnail met by the queue, as the queue will need it back. */
export interface StaleJob {
  key: string;
}

const state = $state({
  /** How many stale thumbnails have been met since launch. */
  count: 0,
  /** The user asked for them to be redone. For this run only: a later engine
   * version asks again. */
  accepted: false,
  /** "Later": not asked again until the next launch. */
  dismissed: false,
});

let jobs = new Map<string, StaleJob>();

/** Where things stand, for the notification. Reactive. */
export function staleThumbs(): Readonly<typeof state> {
  return state;
}

/** Whether a stale image is to be redone now rather than shown. */
export function staleRedoAccepted(): boolean {
  return state.accepted;
}

/** Remembers a stale thumbnail the queue showed instead of redoing. */
export function noteStale<T extends StaleJob>(job: T): void {
  if (jobs.has(job.key)) return;
  jobs.set(job.key, job);
  state.count = jobs.size;
}

/** The user's yes: returns what was met so far, for the queue to redo. */
export function acceptStaleRedo<T extends StaleJob>(): T[] {
  state.accepted = true;
  const taken = [...jobs.values()] as T[];
  jobs = new Map();
  state.count = 0;
  return taken;
}

/** "Later". */
export function dismissStale(): void {
  state.dismissed = true;
}
