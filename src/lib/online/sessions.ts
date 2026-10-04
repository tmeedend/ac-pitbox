// The sessions of a server as a timeline (SPEC-play-online.md, "Frise des
// sessions": PRACTICE 10 min → QUALI 10 min → RACE 15 laps). Durations are
// read as Content Manager reads them (`Session.DisplayDuration`): seconds,
// except a race that is not timed, counted in laps. Pure, for Vitest.
import type { ServerSummary, SessionKind } from "./online";

export type Duration = { unit: "laps"; laps: number } | { unit: "min"; minutes: number } | { unit: "h"; hours: number };

/** Past this, a practice is the server's open time, not a session one waits
 * out: « Practice 200 h » says nothing — it reads « Open » (reported). */
const OPEN_PRACTICE_S = 3 * 3600;

/** A practice that is really the server staying open. */
export function isOpenPractice(kind: SessionKind | null, seconds: number): boolean {
  return kind === "practice" && seconds > OPEN_PRACTICE_S;
}

export interface SessionStep {
  kind: SessionKind;
  active: boolean;
  duration: Duration | null;
  /** An open practice (`isOpenPractice`): no duration worth reading. */
  open: boolean;
  /** A timed race that ends with one more lap. */
  extraLap: boolean;
}

/** Seconds as minutes, or as hours from two hours on — a 24 h freeroam
 * reads "24 h", not "1440 min". */
export function fromSeconds(seconds: number): Duration {
  const minutes = Math.round(seconds / 60);
  return minutes >= 120 ? { unit: "h", hours: Math.round(minutes / 60) } : { unit: "min", minutes };
}

export function sessionTimeline(s: ServerSummary): SessionStep[] {
  return s.sessions.map((kind, i) => {
    const raw = s.durations?.[i];
    const isRace = kind === "race";
    let duration: Duration | null = null;
    if (raw !== undefined && raw > 0) {
      duration = isRace && !s.timed ? { unit: "laps", laps: raw } : fromSeconds(raw);
    }
    return {
      kind,
      active: kind === s.session,
      duration,
      open: raw !== undefined && isOpenPractice(kind, raw),
      extraLap: isRace && !!s.timed && !!s.extra_lap,
    };
  });
}
