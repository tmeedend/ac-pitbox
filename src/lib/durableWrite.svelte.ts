// A settings file written by a Tauri command on every change, **sans repli
// silencieux** (règle d'or n°6).
//
// Each write sends the whole file, and nothing on screen waits for it. A
// failure is retried once — un fichier verrouillé une seconde par un antivirus
// est le cas courant — then kept in `failure`, which the screen shows
// (`PrefsToast`): a lost setting is otherwise only discovered at the next
// start, when nobody can tie it to the gesture that lost it. The Rust command
// logs each failure in the log file; a packaged app has no console.
//
// `ui_prefs.json` has its own version of this, behind a write queue
// (`uiPrefs.svelte.ts`).

export interface WriteFailure {
  /** When writes started failing, `null` while they succeed. */
  since: number | null;
  /** The last error, for whoever reads a bug report. */
  reason: string;
}

export interface DurableWriter<T> {
  save(value: T): void;
  readonly failure: WriteFailure;
}

/** `label` names the file in the console; `send` performs one write. */
export function durableWriter<T>(label: string, send: (value: T) => Promise<unknown>): DurableWriter<T> {
  const failure = $state<WriteFailure>({ since: null, reason: "" });
  /** The most recent value asked for. A retry sends this one, never the value
   * whose write failed: a newer write may have succeeded meanwhile, and
   * retrying the older one would put it back over it. */
  let latest: { value: T } | null = null;

  async function write(value: T): Promise<void> {
    try {
      await send(value);
      failure.since = null;
    } catch (first) {
      console.error(`${label}: first attempt failed, retrying`, first);
      try {
        await send(latest ? latest.value : value);
        failure.since = null;
      } catch (second) {
        console.error(`${label}: not saved to disk`, second);
        failure.since ??= Date.now();
        failure.reason = String(second);
      }
    }
  }

  return {
    save(value: T) {
      latest = { value };
      void write(value);
    },
    failure,
  };
}
