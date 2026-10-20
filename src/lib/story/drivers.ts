// The Drivers screen's and an event grid's logic, pure (SPEC-story-mode,
// Drivers): the roster's names, and a race's opponents shown as the rows of
// the session screen's table (`OpponentTable`), whose edits come back as the
// story's own keys. Whether the grid is right is the engine's validator.
import { clampAiLevel, type Opponent } from "$lib/launch/opponent";
import type { Character, Driver, DriverRole, Session, StoryOpponent } from "./studio";

/** In the order the role control offers them: the spec's list. */
export const DRIVER_ROLES: DriverRole[] = ["friend", "rival", "star", "filler"];

/** What a grid shows for `d`: its name, else the character's it is, else
 * its id. */
export function driverLabel(d: Driver, characters: Character[]): string {
  const character = d.character ? characters.find((c) => c.id === d.character) : undefined;
  return d.name ?? character?.name ?? d.id;
}

/** `opponents` as the table's rows, showing what will race: an opponent
 * without a car shows the event's, without a skill of its own its driver's
 * (`Auto`, the event's, only without either). A driver brings their
 * nationality; the name is the grid's own cell (`EventGrid`), so the row has
 * none. */
export function gridRows(session: Session, drivers: Driver[]): Opponent[] {
  return (session.opponents ?? []).map((o) => {
    const d = o.driver ? drivers.find((x) => x.id === o.driver) : undefined;
    return {
      car_id: o.car ?? session.car ?? "",
      car_skin: o.skin ?? null,
      ai_level: o.ai_level ?? d?.ai_level ?? null,
      driver_name: null,
      nationality: d?.nationality ?? null,
      ballast: o.ballast ?? 0,
      restrictor: o.restrictor ?? 0,
    };
  });
}

/** `o` with what a cell of the table changed: the livery, the ballast, the
 * restrictor. A 0 is left out, as the engine writes it. The identity cells
 * are the grid's own (`withDriver`). */
export function patchOpponent(o: StoryOpponent, patch: Partial<Opponent>): StoryOpponent {
  const out = { ...o };
  if ("car_skin" in patch) setOrDrop(out, "skin", patch.car_skin ?? undefined);
  if (patch.ballast !== undefined) setOrDrop(out, "ballast", patch.ballast || undefined);
  if (patch.restrictor !== undefined) setOrDrop(out, "restrictor", patch.restrictor || undefined);
  return out;
}

/** `o` with its own skill, clamped to Content Manager's range like the
 * session screen's; `null` gives it back to the driver's, else the event's. */
export function withSkill(o: StoryOpponent, level: number | null): StoryOpponent {
  const out = { ...o };
  setOrDrop(out, "ai_level", level === null ? undefined : clampAiLevel(level));
  return out;
}

/** `o` driven by `driver`, or by nobody named. */
export function withDriver(o: StoryOpponent, driver: string | null): StoryOpponent {
  const out = { ...o };
  setOrDrop(out, "driver", driver ?? undefined);
  return out;
}

/** `o` in another car: the livery was the old car's, so it goes. */
export function withCar(o: StoryOpponent, car: string): StoryOpponent {
  const out = { ...o, car };
  delete out.skin;
  return out;
}

/** A copy of `o` for the row after it: the same car and livery, and nobody
 * named - a driver races once per grid. */
export function duplicateOpponent(o: StoryOpponent): StoryOpponent {
  const out = { ...o };
  delete out.driver;
  return out;
}

/** The drivers already on the grid, but for row `except`'s. */
export function driversTaken(opponents: StoryOpponent[], except: number): Set<string> {
  return new Set(opponents.flatMap((o, i) => (i !== except && o.driver ? [o.driver] : [])));
}

/** The drivers a success criterion can race against: those on the grid. */
export function namedOnGrid(session: Session): string[] {
  return (session.opponents ?? []).flatMap((o) => (o.driver ? [o.driver] : []));
}

function setOrDrop<K extends keyof StoryOpponent>(o: StoryOpponent, key: K, v: StoryOpponent[K] | undefined) {
  if (v === undefined) delete o[key];
  else o[key] = v;
}
