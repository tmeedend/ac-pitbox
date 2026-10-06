// A table read once per session — but only once it has actually come.
//
// The pattern it replaces, `loading ??= invokeSafe(cmd, undefined, fallback)`,
// remembered the FALLBACK for the whole session: a backend busy for five
// seconds at startup, and the country index showed every track country as a
// raw English name with a "?" instead of its flag until Pit Box was restarted
// (bug reported, "random, after a while"). Two things go wrong there, and this
// keeps both apart:
//
// - **The wait is bounded, the read is not.** Whoever awaits the load is let
//   go after `waitMs` — a screen must not hang on a table that is decoration —
//   but the call keeps running, and its answer is applied whenever it comes:
//   the flags appear late rather than never.
// - **A failure is not remembered.** An error, or an answer that cannot be
//   used, clears the memo: the next caller asks again.

export interface ReadOnceOptions<T> {
  /** Whether an answer is worth keeping; one that is not is asked again next
   * time, like an error. Defaults to every answer. */
  usable?: (value: T) => boolean;
  /** How long a caller waits before being let go. */
  waitMs?: number;
  /** Names the read in the console when it fails. */
  label: string;
}

const DEFAULT_WAIT_MS = 5000;

/** Returns the loader: call it as often as you like, it reads at most once at
 * a time, and never again after a usable answer has been applied. */
export function readOnce<T>(
  fetch: () => Promise<T>,
  apply: (value: T) => void,
  { usable = () => true, waitMs = DEFAULT_WAIT_MS, label }: ReadOnceOptions<T>,
): () => Promise<void> {
  let pending: Promise<void> | null = null;
  return () => {
    if (!pending) {
      const read: Promise<void> = fetch().then(
        (value) => {
          if (usable(value)) apply(value);
          else if (pending === read) pending = null;
        },
        (e) => {
          console.warn(`${label}: read failed, asked again next time`, e);
          if (pending === read) pending = null;
        },
      );
      pending = read;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const waited = new Promise<void>((resolve) => (timer = setTimeout(resolve, waitMs)));
    return Promise.race([pending, waited]).finally(() => clearTimeout(timer));
  };
}
