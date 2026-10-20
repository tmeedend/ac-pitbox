// Going to an event and coming back (SPEC-story-mode, Story Mode: player
// side): the session built from the player's own setup and the event, sent
// through the session screen's own launch (`startSession`), and the game's
// result read when the game closes. The rules are in `race.ts`; this is the
// part that reads files and asks the backend.
import { getPreferredSkin } from "$lib/preferred";
import { readSessionResult, weatherConditions, weatherOptions, type RaceSetup } from "$lib/launch/launch";
import { loadLaunchState } from "$lib/launch/launchState.svelte";
import { defaultSetup } from "$lib/launch/setupRules";
import { DEFAULT_OPPONENT_COUNT, readPreset } from "$lib/launch/typePresets";
import type { Game, Waiting } from "./play";
import { eventResultOf, hoursOf, sessionTypeOf, setupFor, type EventResult, type EventSession } from "./race";

export type EventWaiting = Extract<Waiting, { kind: "event" }>;

/** The weather when neither the event nor the player has one. */
const DEFAULT_INTENT = "clear";

export function eventSessionOf(w: EventWaiting): EventSession {
  return { type: w.event_type, session: w.session, difficultyStep: w.difficulty_step };
}

/** The session to launch for `w`: the player's own settings for that type
 * of session (what the session screen remembers), then the event's. */
export async function buildSetup(w: EventWaiting, game: Game): Promise<RaceSetup> {
  const e = eventSessionOf(w);
  const type = sessionTypeOf(e.type);
  const { presets } = await loadLaunchState();
  let base: RaceSetup = defaultSetup();
  let count = DEFAULT_OPPONENT_COUNT;
  let playerIntent: string | null = null;
  const preset = presets[type];
  if (preset) {
    const p = readPreset(preset);
    base = { ...base, ...p.setup, abs: p.abs, traction_control: p.tractionControl };
    count = p.opponentCount;
    playerIntent = p.intent || null;
  }
  // A weather is an intent - the event's, else the player's usual one, else
  // a clear sky, the first this installation has; the session wants its
  // folder, and the temperatures that go with it.
  const options = await weatherOptions();
  const option = [w.session.weather, playerIntent, DEFAULT_INTENT]
    .map((intent) => options.find((o) => o.id === intent && o.available && o.weather))
    .find((o) => o !== undefined);
  const setup = setupFor(base, e, game.drivers, game.characters, count, option?.weather ?? null);
  if (option) {
    const c = await weatherConditions(option.id, setup.time_hours, null);
    setup.ambient_c = c.ambient;
    setup.road_c = c.road;
    setup.wind_speed_kmh = c.wind_speed_kmh;
    setup.wind_direction_deg = c.wind_direction_deg;
  }
  // The livery the player last chose for that car, as on the session screen.
  setup.car_skin = setup.car_id ? (getPreferredSkin(setup.car_id)?.id ?? null) : null;
  return setup;
}

/** The result of the session launched for `w`, once the game wrote it;
 * `null` while it has not (the game was closed before the end), or when the
 * file is another session's. */
export async function sessionResult(w: EventWaiting, launchedAtMs: number, game: Game): Promise<EventResult | null> {
  const result = await readSessionResult(launchedAtMs);
  return result ? eventResultOf(result, eventSessionOf(w), game.drivers, game.characters) : null;
}
