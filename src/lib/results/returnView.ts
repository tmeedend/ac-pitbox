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
  /** `null` for a cut lap whose time the game did not keep. */
  time_ms: number | null;
  cut: boolean;
  best: boolean;
}

export type ReturnContent =
  | { kind: "race"; session: SessionOutcome; lines: RaceLine[] }
  | { kind: "laps"; bestMs: number; laps: LapLine[] }
  /** No valid lap: one discreet line, the laps driven (all cut) under it. */
  | { kind: "line"; count: number; laps: LapLine[] };

/** The player's laps, every session together, the best one marked. */
function lapLines(result: SessionResult, bestMs: number | null): LapLine[] {
  let marked = false;
  return result.sessions
    .flatMap((s) => s.player_laps)
    .map((l, i) => {
      const cut = l.cuts > 0;
      // The game's best is a valid lap of that time: the first such one.
      const best = bestMs != null && !cut && !marked && l.time_ms === bestMs;
      if (best) marked = true;
      return { number: i + 1, time_ms: l.time_ms, cut, best };
    });
}

/** Race, then a session with a valid lap, then a discreet line. */
export function returnContent(result: SessionResult | null): ReturnContent {
  if (!result || result.sessions.length === 0) return { kind: "line", count: 0, laps: [] };
  const last = result.sessions[result.sessions.length - 1];
  if (last.kind === "race") return { kind: "race", session: last, lines: raceLines(last) };

  const player = result.sessions.flatMap((s) => s.standings.filter((l) => l.car === 0));
  const bests = player.map((l) => l.best_lap_ms).filter((b): b is number => b != null);
  if (bests.length > 0) {
    const bestMs = Math.min(...bests);
    return { kind: "laps", bestMs, laps: lapLines(result, bestMs) };
  }
  // The laps driven, never the session's set duration: a hotlap is "30 min"
  // whatever was driven.
  const laps = lapLines(result, null);
  return { kind: "line", count: Math.max(laps.length, player.reduce((n, l) => n + l.laps, 0)), laps };
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
