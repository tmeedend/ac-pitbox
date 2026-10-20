// The write queue of the Classement tabs (Brands, Countries, Families): each
// gesture is written at once, one write at a time and in the order of the
// gestures, and a failure is shown on the tab and logged. A write that fails
// in silence is the bug golden rule n°6 of CLAUDE.md was written for.
//
// It was copied five times in four components, one copy already without its
// busy flag. The tab keeps its own `busy` and `error` - its markup reads them
// everywhere - and hands the queue what to do with them.
import { errorText } from "$lib/errors";

/** What a queue tells its tab. */
export interface WriteStatus {
  /** A write starts (`true`), or ends whatever its outcome (`false`). */
  busy?: (busy: boolean) => void;
  /** After each write: `""` when it went through, the error's text otherwise. */
  error: (text: string) => void;
}

/** Queues a write; `label` names it in the console. The promise settles once
 * this write is done, and never rejects: the failure went to `error`. */
export type Enqueue = (label: string, write: () => Promise<void>) => Promise<void>;

export function writeQueue(status: WriteStatus): Enqueue {
  let queue: Promise<void> = Promise.resolve();
  return (label, write) => {
    // The `catch` is not optional: a rejected promise in the chain would
    // freeze every later write until the next start.
    queue = queue
      .then(async () => {
        status.busy?.(true);
        await write();
        status.error("");
      })
      .catch((e) => {
        console.error(label, e);
        status.error(errorText(e));
      })
      .finally(() => status.busy?.(false));
    return queue;
  };
}
