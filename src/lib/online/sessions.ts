// The sessions of a server as a timeline (SPEC-play-online.md, "Frise des
// sessions": PRACTICE 10 min → QUALI 10 min → RACE 15 laps). Durations are
// read as Content Manager reads them (`Session.DisplayDuration`): seconds,
// except a race that is not timed, counted in laps. Pure, for Vitest.
import type { ServerSummary, SessionKind } from "./online";

export type Duration = { unit: "laps"; laps: number } | { unit: "min"; minutes: number } | { unit: "h"; hours: number };

export interface SessionStep {
  kind: SessionKind;
  active: boolean;
  duration: Duration | null;
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
    return { kind, active: kind === s.session, duration, extraLap: isRace && !!s.timed && !!s.extra_lap };
  });
}
