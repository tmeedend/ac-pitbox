import { describe, expect, it } from "vitest";
import type { SessionOutcome, SessionResult, Standing } from "$lib/launch/launch";
import { fmtGap, fmtLap, returnContent } from "./returnView";

function standing(car: number, over: Partial<Standing> = {}): Standing {
  return {
    position: car + 1,
    car,
    driver_name: `D${car}`,
    car_id: "c",
    skin: "s",
    laps: 0,
    best_lap_ms: null,
    total_ms: 0,
    dnf: false,
    ...over,
  };
}

function session(over: Partial<SessionOutcome>): SessionOutcome {
  return {
    name: "S",
    kind: "practice",
    race_laps: 0,
    duration_minutes: 0,
    standings: [],
    player_laps: [],
    any_lap_begun: true,
    ...over,
  };
}

function result(...sessions: SessionOutcome[]): SessionResult {
  return { track: "t", entries: [], sessions };
}

describe("lap times", () => {
  it("reads as the game shows them", () => {
    expect(fmtLap(112_304)).toBe("1:52.304");
    expect(fmtLap(52_004)).toBe("52.004");
    expect(fmtLap(null)).toBe("—");
    expect(fmtGap(512)).toBe("+0.512");
    expect(fmtGap(-1203)).toBe("-1.203");
    expect(fmtGap(61_500)).toBe("+1:01.500");
  });
});

describe("what the return screen shows (RESULTAT§4.2)", () => {
  it("a race: the classification, gaps to the winner, laps down, no gap for a DNF", () => {
    const race = session({
      kind: "race",
      standings: [
        standing(1, { laps: 5, total_ms: 500_000 }),
        standing(0, { laps: 5, total_ms: 503_250 }),
        standing(2, { laps: 4, total_ms: 480_000 }),
        standing(3, { laps: 2, total_ms: 200_000, dnf: true }),
      ],
    });
    const c = returnContent(result(session({}), race));
    expect(c.kind).toBe("race");
    if (c.kind !== "race") return;
    expect(c.lines.map((l) => l.gap)).toEqual([null, { ms: 3250 }, { laps: 1 }, null]);
    expect(c.lines.find((l) => l.isPlayer)?.standing.car).toBe(0);
  });

  it("a practice with a valid lap: the best, and the laps with the cut one marked", () => {
    const c = returnContent(
      result(
        session({
          standings: [standing(0, { laps: 3, best_lap_ms: 90_000 })],
          player_laps: [
            { time_ms: 95_000, cuts: 0 },
            { time_ms: 89_000, cuts: 2 },
            { time_ms: 90_000, cuts: 0 },
          ],
        }),
      ),
    );
    expect(c).toEqual({
      kind: "laps",
      bestMs: 90_000,
      laps: [
        { number: 1, time_ms: 95_000, cut: false, best: false },
        { number: 2, time_ms: 89_000, cut: true, best: false },
        { number: 3, time_ms: 90_000, cut: false, best: true },
      ],
    });
  });

  // Real case: a hotlap of one cut lap, set to 30 min. The line counts the
  // laps driven, never the set duration, and the cut lap is listed.
  it("no valid lap: one discreet line, the cut laps under it", () => {
    const c = returnContent(
      result(
        session({
          kind: "hotlap",
          duration_minutes: 30,
          standings: [standing(0, { laps: 1 })],
          player_laps: [{ time_ms: 101_624, cuts: 1 }],
        }),
      ),
    );
    expect(c).toEqual({
      kind: "line",
      count: 1,
      laps: [{ number: 1, time_ms: 101_624, cut: true, best: false }],
    });
    expect(returnContent(null)).toEqual({ kind: "line", count: 0, laps: [] });
  });
});
