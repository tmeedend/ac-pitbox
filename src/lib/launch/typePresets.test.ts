import { describe, expect, it } from "vitest";
import type { RaceSetup } from "./launch";
import { presetFromSetup, readPreset, type TypePreset } from "./typePresets";

const setup = (over: Partial<RaceSetup> = {}): RaceSetup => ({
  car_id: "car",
  car_skin: null,
  driver: null,
  track_id: "track",
  track_layout: null,
  session_type: "race",
  opponents: [],
  ai_level: 90,
  ai_spread: 4,
  aggression: 20,
  aggression_spread: 5,
  player_ballast: 0,
  player_restrictor: 0,
  start_mode: "second",
  laps: 12,
  weather: "3_clear",
  time_hours: 15,
  ambient_c: null,
  road_c: null,
  wind_speed_kmh: null,
  wind_direction_deg: null,
  season: null,
  season_date: null,
  penalties: true,
  jump_start_penalty: 1,
  track_state: null,
  grip: 98,
  practice_enabled: true,
  practice_minutes: 30,
  qualify_enabled: false,
  qualify_minutes: 15,
  ghost_car: true,
  ghost_advantage: 2,
  practice_start: "track",
  damage: 100,
  fuel_rate: 200,
  tyre_wear: 150,
  tyre_blankets: true,
  abs: "on",
  traction_control: "off",
  ideal_line: true,
  ...over,
});

/** The smallest preset an old version could have written: only the fields
 * that were never optional. */
const oldest = (over: Partial<TypePreset> = {}): TypePreset =>
  ({ opponent_count: 7, laps: 5, time_hours: 13, penalties: false, grip: 96, intent: "clear", ...over }) as TypePreset;

describe("presetFromSetup / readPreset", () => {
  // Whatever is written must come back unchanged, the grid included.
  it("gives back what it wrote", () => {
    const s = setup();
    const p = presetFromSetup(s, { count: 12, pool: "#gt3", pinned: ["category"] }, "clear", "summer");
    const v = readPreset(p);
    for (const [k, value] of Object.entries(v.setup)) {
      expect(value, k).toEqual(s[k as keyof RaceSetup]);
    }
    expect(v.opponentCount).toBe(12);
    expect(v.abs).toBe("on");
    expect(v.tractionControl).toBe("off");
    expect(v.season).toBe("summer");
    expect(v.intent).toBe("clear");
    expect(p.grid_filters).toBe("#gt3");
    expect(p.grid_pinned).toEqual(["category"]);
  });

  // The written preset is a copy: editing the screen afterwards must not
  // change what was saved.
  it("copies the track state rather than sharing it", () => {
    const s = setup({ track_state: { origin: "builtin", name: "Green" } as RaceSetup["track_state"] });
    const p = presetFromSetup(s, { count: 0, pool: "", pinned: [] }, "", "");
    expect(p.track_state).toEqual(s.track_state);
    expect(p.track_state).not.toBe(s.track_state);
  });
});

describe("readPreset — older presets", () => {
  // Before centre ± spread, a preset carried two bounds: converting them keeps
  // a difficulty set months ago instead of resetting it.
  it("turns the old bounds into a centre and a spread", () => {
    const v = readPreset(oldest({ ai_level_min: 90, ai_level_max: 98 }));
    expect(v.setup.ai_level).toBe(94);
    expect(v.setup.ai_spread).toBe(4);
  });

  // A floor of 60 was once offered; Content Manager does not go below 70.
  it("raises a strength saved under the current floor", () => {
    expect(readPreset(oldest({ ai_level: 60 })).setup.ai_level).toBe(70);
  });

  it("keeps the four start positions and drops only the old 'custom'", () => {
    expect(readPreset(oldest({ start_mode: "second" })).setup.start_mode).toBe("second");
    expect(readPreset(oldest({ start_mode: "custom" as TypePreset["start_mode"] })).setup.start_mode).toBe("random");
  });

  // `abs_auto: true` meant Abs 1, which is today's `factory` — never `on`.
  it("reads the old assist booleans as the level they sent", () => {
    const v = readPreset(oldest({ abs_auto: true, traction_control_auto: false }));
    expect(v.abs).toBe("factory");
    expect(v.tractionControl).toBe("off");
    expect(readPreset(oldest()).abs).toBe("factory");
  });

  it("recalibrates a grip the game no longer offers", () => {
    expect(readPreset(oldest({ grip: 92 })).setup.grip).toBe(95);
  });

  it("fills the fields an old preset did not have with their defaults", () => {
    const v = readPreset(oldest());
    expect(v.setup).toMatchObject({
      aggression: 0,
      aggression_spread: 0,
      ghost_advantage: 0,
      jump_start_penalty: 0,
      track_state: null,
      practice_enabled: false,
      practice_minutes: 20,
      qualify_enabled: true,
      qualify_minutes: 10,
      ghost_car: false,
      practice_start: "pit",
      damage: 50,
      fuel_rate: 100,
      tyre_wear: 100,
      tyre_blankets: false,
      ideal_line: false,
    });
    expect(v.season).toBe("");
  });
});
