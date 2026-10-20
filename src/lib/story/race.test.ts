import { describe, expect, it } from "vitest";
import type { RaceSetup, SessionResult, Standing } from "$lib/launch/launch";
import { defaultSetup } from "$lib/launch/setupRules";
import { eventResultOf, hoursOf, opponentsFor, sessionTypeOf, setupFor, type EventSession } from "./race";
import type { Character, Driver } from "./studio";

const characters: Character[] = [{ id: "rival", name: "A rival" }];
const drivers: Driver[] = [
  { id: "rival_driver", character: "rival", nationality: "Italy", ai_level: 96, aggression: 70 },
  { id: "star", name: "A. Star", ai_level: 99 },
];

function race(o: Partial<EventSession["session"]> = {}, difficultyStep = 0): EventSession {
  return { type: "race", session: { track: "t", car: "c", ...o }, difficultyStep };
}

describe("opponentsFor", () => {
  // The author's opponents first, with their drivers' identity and skill,
  // then the event's car under the liveries' names up to the grid's size
  // (SPEC-story-mode, Drivers).
  it("lines up the author's grid, then fills it in the event's car", () => {
    const grid = opponentsFor(
      race({ grid: 5, opponents: [{ driver: "rival_driver", skin: "red" }, { car: "x", ai_level: 90 }] }),
      drivers,
      characters,
      7,
    );
    expect(grid).toHaveLength(4);
    expect(grid[0]).toMatchObject({
      car_id: "c",
      car_skin: "red",
      ai_level: 96,
      aggression: 70,
      driver_name: "A rival",
      nationality: "Italy",
    });
    expect(grid[1]).toMatchObject({ car_id: "x", ai_level: 90, driver_name: null });
    expect(grid[2]).toMatchObject({ car_id: "c", car_skin: null, ai_level: null, driver_name: null });
  });

  it("is the listed field without a grid size, the player's usual one without either, never more than the size", () => {
    expect(opponentsFor(race({ grid: 2, opponents: [{}, {}, {}] }), drivers, characters, 7)).toHaveLength(1);
    expect(opponentsFor(race({ opponents: [{ driver: "star" }] }), drivers, characters, 7)).toHaveLength(1);
    expect(opponentsFor(race(), drivers, characters, 7)).toHaveLength(7);
  });

  // Help after failures lowers every set skill, within CM's range.
  it("lowers the skills by the difficulty steps", () => {
    const grid = opponentsFor(race({ opponents: [{ driver: "star" }, { ai_level: 71 }] }, 2), drivers, characters, 0);
    expect(grid.map((o) => o.ai_level)).toEqual([95, 70]);
  });
});

describe("setupFor", () => {
  // The author's settings over the player's own session: assists and damage
  // stay the player's; a race starts on the grid.
  it("sets what the event decides and keeps the rest", () => {
    const base: RaceSetup = { ...defaultSetup(), damage: 30, abs: "off", qualify_enabled: true };
    const s = setupFor(base, race({ laps: 8, ai_level: 92, start: "last", time: "16:30", layout: "gp" }), drivers, characters, 7, "3_clear");
    expect(s).toMatchObject({
      car_id: "c",
      track_id: "t",
      track_layout: "gp",
      session_type: "race",
      laps: 8,
      ai_level: 92,
      ai_spread: 0,
      start_mode: "last",
      time_hours: 16.5,
      weather: "3_clear",
      qualify_enabled: false,
      damage: 30,
      abs: "off",
    });
    expect(s.opponents).toHaveLength(7);
    const lap = setupFor(base, { type: "hot_lap", session: { track: "t", car: "c" }, difficultyStep: 0 }, [], [], 7, null);
    expect([lap.session_type, lap.opponents.length, lap.weather]).toEqual(["hotlap", 0, base.weather]);
  });

  it("maps the event types and the clock", () => {
    expect((["race", "hot_lap", "time_attack", "track_day", "test_drive"] as const).map(sessionTypeOf)).toEqual([
      "race",
      "hotlap",
      "hotlap",
      "trackday",
      "practice",
    ]);
    expect(hoursOf("09:15")).toBe(9.25);
    expect(hoursOf("late")).toBeNull();
  });
});

describe("eventResultOf", () => {
  const NAMES = ["Player", "A rival", "Anon", "A. Star"];
  function standing(car: number, position: number, o: Partial<Standing> = {}): Standing {
    const driver_name = NAMES[car] ?? "";
    return { position, car, driver_name, car_id: "c", skin: "", laps: 8, best_lap_ms: 80_000, total_ms: 0, dnf: false, ...o };
  }
  function result(standings: Standing[], kind: "race" | "hotlap" = "race"): SessionResult {
    return {
      track: "t-gp",
      sessions: [
        { name: "Qualify", kind: "qualifying", race_laps: 0, duration_minutes: 10, standings: [] },
        { name: "Race", kind, race_laps: 8, duration_minutes: 0, standings },
      ],
    };
  }
  const grid = race({ opponents: [{ driver: "rival_driver" }, {}, { driver: "star" }] });

  const of = (r: SessionResult, e: EventSession = grid) => eventResultOf(r, e, drivers, characters);

  // The named drivers beaten are found by the names the grid gave them,
  // wherever the launch put their car.
  it("reads the player's place and the named drivers behind them", () => {
    const r = of(result([standing(3, 1), standing(0, 2), standing(1, 3), standing(2, 4)]));
    expect(r).toEqual({ finished: true, position: 2, best_lap_ms: 80_000, ahead_of: ["rival_driver"] });
    const moved = of(result([standing(0, 1), standing(1, 2, { driver_name: "A. Star" })]));
    expect(moved?.ahead_of).toEqual(["star"]);
  });

  it("counts a retired driver as beaten, and beats nobody once retired", () => {
    expect(of(result([standing(0, 1), standing(3, 2, { dnf: true })]))?.ahead_of).toEqual(["star"]);
    const out = of(result([standing(1, 1), standing(0, 2, { dnf: true })]));
    expect(out).toMatchObject({ finished: false, ahead_of: [] });
  });

  // A session run elsewhere after the event's was left is not the event's.
  it("takes no result from another track or car", () => {
    expect(of({ ...result([standing(0, 1)]), track: "other" })).toBeNull();
    expect(of(result([standing(0, 1, { car_id: "other" })]))).toBeNull();
  });

  it("finishes another session by driving a lap, and has no result without the player", () => {
    const lap = { type: "hot_lap" as const, session: {}, difficultyStep: 0 };
    expect(of(result([standing(0, 1, { laps: 0, best_lap_ms: null })], "hotlap"), lap)).toEqual({
      finished: false,
      position: null,
      best_lap_ms: null,
      ahead_of: [],
    });
    expect(of(result([]))).toBeNull();
  });
});
