import { describe, expect, it } from "vitest";
import type { HistoryLine } from "./results";
import { carLines, OUTING_GAP_MS, outingsOf, visibleOutings } from "./outings";

const H = 3_600_000;
let next = 1;
function line(over: Partial<HistoryLine>): HistoryLine {
  return {
    launch_id: null,
    session_id: next++,
    kind: "session",
    mode: "practice",
    started_at: 0,
    ended_at: 0,
    car_id: "car_a",
    car_name: "Car A",
    car_known: true,
    track_id: "monza",
    track_name: "Monza",
    track_known: true,
    track_layout: null,
    best_lap_ms: 100_000,
    can_again: false,
    can_edit: false,
    server: null,
    valid_laps: 3,
    race_position: null,
    ...over,
  };
}

describe("outings (RESULTAT§7.2)", () => {
  it("sessions in a row on one layout, up to two hours apart, are one outing", () => {
    const lines = [
      line({ started_at: 10 * H, ended_at: 10.5 * H }),
      // 1 h 30 before: same outing.
      line({ started_at: 8 * H, ended_at: 8.5 * H, car_id: "car_b" }),
      // More than two hours before the outing's start: another one.
      line({ started_at: 5 * H, ended_at: 8 * H - OUTING_GAP_MS - 1 }),
      // Another layout: another outing, whatever the time.
      line({ started_at: 1 * H, ended_at: 1.5 * H, track_layout: "junior" }),
    ];
    expect(outingsOf(lines).map((o) => o.sessions.length)).toEqual([2, 1, 1]);
  });

  it("sessions without a valid lap are hidden, and an outing of only those with them", () => {
    const outings = outingsOf([
      line({ started_at: 10 * H, ended_at: 10.5 * H }),
      line({ started_at: 9 * H, ended_at: 9.5 * H, best_lap_ms: null }),
      line({ started_at: 1 * H, ended_at: 1.5 * H, track_id: "spa", best_lap_ms: null }),
    ]);
    expect(visibleOutings(outings, false).map((o) => o.sessions.length)).toEqual([1]);
    expect(visibleOutings(outings, true).map((o) => o.sessions.length)).toEqual([2, 1]);
  });
});

describe("the cars of an outing (RESULTAT§7.3)", () => {
  it("one line per car, the fastest first, with its gap, valid laps and sessions", () => {
    const [outing] = outingsOf([
      line({ started_at: 3 * H, ended_at: 3.5 * H, car_id: "car_b", best_lap_ms: 101_500, valid_laps: 2 }),
      line({ started_at: 2 * H, ended_at: 2.5 * H, best_lap_ms: 100_200, valid_laps: 4 }),
      line({ started_at: 1 * H, ended_at: 1.5 * H, best_lap_ms: 100_000, valid_laps: 1 }),
    ]);
    expect(carLines(outing).map((c) => [c.car_id, c.best_lap_ms, c.gap_ms, c.valid_laps, c.sessions])).toEqual([
      ["car_a", 100_000, null, 5, 2],
      ["car_b", 101_500, 1_500, 2, 1],
    ]);
  });
});
