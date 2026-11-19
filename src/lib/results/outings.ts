// The history's outings (RESULTAT§7.2): sessions in a row on the same track
// and layout, no more than two hours apart. Pure: the cases are in
// `outings.test.ts`.
import type { HistoryLine } from "./results";

/** Between the end of a session and the start of the next one. */
export const OUTING_GAP_MS = 2 * 3_600_000;

export interface Outing {
  /** Stable while the history does not change: its latest session. */
  key: string;
  track_id: string | null;
  track_name: string | null;
  track_known: boolean;
  track_layout: string | null;
  /** The latest first, as the history. */
  sessions: HistoryLine[];
  started_at: number;
  ended_at: number;
  best_lap_ms: number | null;
}

/** One car of an outing, for the comparison (RESULTAT§7.3). */
export interface CarLine {
  car_id: string;
  car_name: string | null;
  car_known: boolean;
  best_lap_ms: number | null;
  /** To the outing's best; `null` for the best car or a car without a time. */
  gap_ms: number | null;
  valid_laps: number;
  sessions: number;
}

const endOf = (l: HistoryLine) => l.ended_at ?? l.started_at;
const sameLayout = (a: HistoryLine, o: Outing) =>
  a.track_id === o.track_id && (a.track_layout ?? "") === (o.track_layout ?? "");

function minBest(values: (number | null)[]): number | null {
  const known = values.filter((v): v is number => v != null);
  return known.length ? Math.min(...known) : null;
}

/** Groups history lines (the latest first) into outings. */
export function outingsOf(lines: HistoryLine[]): Outing[] {
  const out: Outing[] = [];
  for (const line of lines) {
    const current = out[out.length - 1];
    if (current && sameLayout(line, current) && current.started_at - endOf(line) <= OUTING_GAP_MS) {
      current.sessions.push(line);
      current.started_at = Math.min(current.started_at, line.started_at);
      continue;
    }
    out.push({
      key: String(line.session_id ?? line.launch_id ?? endOf(line)),
      track_id: line.track_id,
      track_name: line.track_name,
      track_known: line.track_known,
      track_layout: line.track_layout,
      sessions: [line],
      started_at: line.started_at,
      ended_at: endOf(line),
      best_lap_ms: null,
    });
  }
  for (const o of out) o.best_lap_ms = minBest(o.sessions.map((s) => s.best_lap_ms));
  return out;
}

/** What the history shows: sessions without a valid lap hidden unless asked,
 * and an outing left with none hidden with them (RESULTAT§7.5). */
export function visibleOutings(outings: Outing[], showEmpty: boolean): Outing[] {
  if (showEmpty) return outings;
  return outings
    .map((o) => ({ ...o, sessions: o.sessions.filter((s) => s.best_lap_ms != null) }))
    .filter((o) => o.sessions.length > 0);
}

/** The cars of an outing, the fastest first (RESULTAT§7.3). */
export function carLines(outing: Outing): CarLine[] {
  const byCar = new Map<string, CarLine>();
  for (const s of outing.sessions) {
    const id = s.car_id ?? "";
    const line = byCar.get(id) ?? {
      car_id: id,
      car_name: s.car_name,
      car_known: s.car_known,
      best_lap_ms: null,
      gap_ms: null,
      valid_laps: 0,
      sessions: 0,
    };
    line.best_lap_ms = minBest([line.best_lap_ms, s.best_lap_ms]);
    line.valid_laps += s.valid_laps;
    line.sessions += 1;
    byCar.set(id, line);
  }
  const lines = [...byCar.values()];
  const best = minBest(lines.map((l) => l.best_lap_ms));
  for (const l of lines) l.gap_ms = l.best_lap_ms != null && best != null && l.best_lap_ms !== best ? l.best_lap_ms - best : null;
  return lines.sort((a, b) => (a.best_lap_ms ?? Infinity) - (b.best_lap_ms ?? Infinity));
}
