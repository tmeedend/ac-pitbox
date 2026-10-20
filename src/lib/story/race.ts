// A story event as a Pit Box session, and the session's result as the
// engine's, pure (SPEC-story-mode, Events, retries and failure; Drivers).
//
// The author decides what the event says - track, car, laps, grid, skill,
// start, time of day, weather - and everything else is the player's: the
// session they start from is their own session screen's for that type
// (assists, damage, tyre wear, penalties, track state). A race starts on the
// grid, with no practice or qualifying, since the author sets the start.
import { clampAiLevel, type Opponent } from "$lib/launch/opponent";
import type { RaceSetup, SessionOutcome, SessionResult, SessionType } from "$lib/launch/launch";
import { driverLabel } from "./drivers";
import type { Character, Driver, EventType, Session } from "./studio";

/** Skill points one step of adaptive difficulty takes off the opponents
 * (SPEC-story-mode, Hard events). Small, since the whole of Content
 * Manager's range is thirty points. */
export const DIFFICULTY_STEP_POINTS = 2;

/** The session screen's type an event runs as. A time attack is a hotlap
 * against the clock, a test drive a practice. */
export function sessionTypeOf(type: EventType): SessionType {
  switch (type) {
    case "race":
      return "race";
    case "hot_lap":
    case "time_attack":
      return "hotlap";
    case "track_day":
      return "trackday";
    case "test_drive":
      return "practice";
  }
}

/** `HH:MM` as the hours of the day the session screen counts in. */
export function hoursOf(clock: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(clock);
  return m ? Number(m[1]) + Number(m[2]) / 60 : null;
}

/** What the event asks of the session, besides the player's own setup. */
export interface EventSession {
  type: EventType;
  session: Session;
  /** Steps of adaptive difficulty earned by failing (`difficulty_step`). */
  difficultyStep: number;
}

/** The opponents a race lines up: the author's, in order, then as many in
 * the event's car as `grid` asks beyond them, under the liveries' names. A
 * named driver brings their name, nationality, skill and aggression; a row's
 * own skill wins over its driver's. Skills are lowered by the adaptive
 * difficulty, within Content Manager's range. `fallbackCount`: how many
 * opponents when the event says neither `grid` nor `opponents`. */
export function opponentsFor(
  e: EventSession,
  drivers: Driver[],
  characters: Character[],
  fallbackCount: number,
): Opponent[] {
  const s = e.session;
  const less = e.difficultyStep * DIFFICULTY_STEP_POINTS;
  const lowered = (level: number | null | undefined) => (level == null ? null : clampAiLevel(level - less));
  const listed = (s.opponents ?? []).map((o): Opponent => {
    const d = o.driver ? drivers.find((x) => x.id === o.driver) : undefined;
    return {
      car_id: o.car ?? s.car ?? "",
      car_skin: o.skin ?? null,
      ai_level: lowered(o.ai_level ?? d?.ai_level),
      aggression: d?.aggression ?? null,
      driver_name: d ? driverLabel(d, characters) : null,
      nationality: d?.nationality ?? null,
      ballast: o.ballast ?? 0,
      restrictor: o.restrictor ?? 0,
    };
  });
  const total = s.grid != null ? s.grid - 1 : listed.length > 0 ? listed.length : fallbackCount;
  const filler = Math.max(0, total - listed.length);
  return [
    // A grid smaller than its list is the checks' error; the size wins.
    ...listed.slice(0, Math.max(0, total)),
    ...Array.from({ length: filler }, (): Opponent => ({
      car_id: s.car ?? "",
      car_skin: null,
      ai_level: null,
      driver_name: null,
      nationality: null,
      ballast: 0,
      restrictor: 0,
    })),
  ];
}

/** The session to launch: `base` (the player's own for that type), with
 * what the event decides. `weather` is the weather folder of the event's
 * intent, `null` to keep the player's. */
export function setupFor(
  base: RaceSetup,
  e: EventSession,
  drivers: Driver[],
  characters: Character[],
  fallbackCount: number,
  weather: string | null,
): RaceSetup {
  const s = e.session;
  const type = sessionTypeOf(e.type);
  const race = type === "race";
  const less = e.difficultyStep * DIFFICULTY_STEP_POINTS;
  return {
    ...base,
    car_id: s.car ?? base.car_id,
    track_id: s.track ?? base.track_id,
    track_layout: s.track ? (s.layout ?? null) : base.track_layout,
    session_type: type,
    laps: s.laps ?? base.laps,
    time_hours: (s.time ? hoursOf(s.time) : null) ?? base.time_hours,
    weather: weather ?? base.weather,
    start_mode: s.start ?? "random",
    // The author's skill is a level, not a band: every unnamed opponent
    // drives at it. Lowered by the help after failures, like the named ones.
    ai_level: clampAiLevel((s.ai_level ?? base.ai_level) - less),
    ai_spread: s.ai_level != null ? 0 : base.ai_spread,
    opponents: race ? opponentsFor(e, drivers, characters, fallbackCount) : [],
    practice_enabled: false,
    qualify_enabled: false,
  };
}

/** What the engine needs of a session's result (`story_engine::state::
 * EventResult`). */
export interface EventResult {
  finished: boolean;
  position: number | null;
  best_lap_ms: number | null;
  /** The named drivers the player finished ahead of. */
  ahead_of: string[];
}

/** The session that decides an event: the race of a race, the last session
 * otherwise. `null` when the game wrote none. */
function decisive(result: SessionResult, type: SessionType): SessionOutcome | null {
  const sessions = type === "race" ? result.sessions.filter((s) => s.kind === "race") : result.sessions;
  return sessions[sessions.length - 1] ?? null;
}

/** Whether `result` is the event's session: the player's car and the
 * track (the game writes `<track>-<layout>`) are the event's. Another
 * session - one run from the session screen after the event's was left - is
 * never credited to the story. */
function isTheEvents(result: SessionResult, session: SessionOutcome, e: EventSession): boolean {
  const s = e.session;
  const car = session.standings.find((x) => x.car === 0)?.car_id;
  const trackOk = !s.track || result.track === s.track || result.track.startsWith(`${s.track}-`);
  return trackOk && (!s.car || car === s.car);
}

/** The game's result as the engine's. The player is car 0. A named driver is
 * found by the name the grid gave them, not by their place: the launch
 * leaves out an opponent whose car cannot go on track, which moves every car
 * after it. A race is finished without retiring; another session, with a lap
 * driven. `null` when the result is not the event's session. */
export function eventResultOf(
  result: SessionResult,
  e: EventSession,
  drivers: Driver[],
  characters: Character[],
): EventResult | null {
  const type = sessionTypeOf(e.type);
  const session = decisive(result, type);
  const me = session?.standings.find((x) => x.car === 0);
  if (!session || !me || !isTheEvents(result, session, e)) return null;
  const race = type === "race";
  const named = (e.session.opponents ?? []).flatMap((o) => {
    const d = o.driver ? drivers.find((x) => x.id === o.driver) : undefined;
    return d ? [{ id: d.id, name: driverLabel(d, characters) }] : [];
  });
  const behind = (x: { position: number; dnf: boolean }) => (me.dnf ? false : x.dnf || x.position > me.position);
  return {
    finished: race ? !me.dnf : me.laps > 0,
    position: race ? me.position : null,
    best_lap_ms: me.best_lap_ms,
    ahead_of: race
      ? named.flatMap((n) => {
          const x = session.standings.find((st) => st.car > 0 && st.driver_name === n.name);
          return x && behind(x) ? [n.id] : [];
        })
      : [],
  };
}
