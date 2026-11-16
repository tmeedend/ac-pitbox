// What the screen shown on return says, decided from the result alone
// (RESULTAT§4.2). Pure: the cases are in `returnView.test.ts`.
import type { SessionOutcome, SessionResult, Standing } from "$lib/launch/launch";

/** `1:52.304`; under a minute `52.304`. */
export function fmtLap(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms) || ms <= 0) return "—";
  const m = Math.floor(ms / 60_000);
  const s = Math.floor((ms % 60_000) / 1000);
  const rest = String(Math.round(ms % 1000)).padStart(3, "0");
  return m > 0 ? `${m}:${String(s).padStart(2, "0")}.${rest}` : `${s}.${rest}`;
}

/** `+0.512`, `-1.203`: a signed gap in seconds. */
export function fmtGap(ms: number): string {
  const sign = ms < 0 ? "-" : "+";
  const abs = Math.abs(ms);
  const m = Math.floor(abs / 60_000);
  const s = ((abs % 60_000) / 1000).toFixed(3);
  return m > 0 ? `${sign}${m}:${s.padStart(6, "0")}` : `${sign}${s}`;
}

/** A line of the race classification, as shown. */
export interface RaceLine {
  standing: Standing;
  isPlayer: boolean;
  /** To the winner: `{ laps }` down, or a time; nothing for the winner or a
   * car that did not finish. */
  gap: { laps: number } | { ms: number } | null;
}

export interface LapLine {
  number: number;
  time_ms: number;
  cut: boolean;
  best: boolean;
}

export type ReturnContent =
  | { kind: "race"; session: SessionOutcome; lines: RaceLine[] }
  | { kind: "laps"; bestMs: number; laps: LapLine[] }
  | { kind: "line"; laps: number; minutes: number };

/** Race, then a session with a valid lap, then a discreet line. */
export function returnContent(result: SessionResult | null): ReturnContent {
  if (!result || result.sessions.length === 0) return { kind: "line", laps: 0, minutes: 0 };
  const last = result.sessions[result.sessions.length - 1];
  if (last.kind === "race") return { kind: "race", session: last, lines: raceLines(last) };

  const player = result.sessions.flatMap((s) => s.standings.filter((l) => l.car === 0));
  const bests = player.map((l) => l.best_lap_ms).filter((b): b is number => b != null);
  if (bests.length > 0) {
    const bestMs = Math.min(...bests);
    const driven = result.sessions.flatMap((s) => s.player_laps);
    let marked = false;
    const laps = driven.map((l, i) => {
      const cut = l.cuts > 0;
      // The game's best is a valid lap of that time: the first such one.
      const best = !cut && !marked && l.time_ms === bestMs;
      if (best) marked = true;
      return { number: i + 1, time_ms: l.time_ms, cut, best };
    });
    return { kind: "laps", bestMs, laps };
  }
  return {
    kind: "line",
    laps: player.reduce((n, l) => n + l.laps, 0),
    minutes: result.sessions.reduce((n, s) => n + s.duration_minutes, 0),
  };
}

function raceLines(session: SessionOutcome): RaceLine[] {
  const winner = session.standings[0];
  return session.standings.map((standing, i) => {
    let gap: RaceLine["gap"] = null;
    if (i > 0 && winner && !standing.dnf) {
      gap = standing.laps < winner.laps ? { laps: winner.laps - standing.laps } : { ms: standing.total_ms - winner.total_ms };
    }
    return { standing, isPlayer: standing.car === 0, gap };
  });
}
