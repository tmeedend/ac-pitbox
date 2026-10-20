import { describe, expect, it } from "vitest";
import {
  driverLabel,
  driversTaken,
  duplicateOpponent,
  gridRows,
  namedOnGrid,
  patchOpponent,
  withCar,
  withDriver,
  withSkill,
} from "./drivers";
import type { Opponent } from "$lib/launch/opponent";
import type { Character, Driver, Session } from "./studio";

const characters: Character[] = [{ id: "rival", name: "A rival" }];
const drivers: Driver[] = [
  { id: "rival_driver", character: "rival", nationality: "Italy" },
  { id: "star", name: "A. Star" },
];

describe("driverLabel", () => {
  // A driver who is a character is named as the character, unless the
  // roster gives them a name of their own.
  it("takes the driver's name, else the character's, else the id", () => {
    expect(driverLabel(drivers[0], characters)).toBe("A rival");
    expect(driverLabel(drivers[1], characters)).toBe("A. Star");
    expect(driverLabel({ id: "x" }, characters)).toBe("x");
  });
});

describe("gridRows", () => {
  // The session screen's table shows what will race: an opponent without a
  // car drives the event's, a named driver brings their nationality and
  // their skill, an unnamed one is `Auto` (the livery's, the event's).
  it("shows each opponent as the table's row", () => {
    const session: Session = {
      car: "event_car",
      opponents: [
        { driver: "rival_driver", skin: "red", ai_level: 95 },
        { driver: "star" },
        { car: "other", ballast: 30 },
        { driver: "ghost" },
      ],
    };
    const roster: Driver[] = [...drivers.slice(0, 1), { ...drivers[1], ai_level: 99 }];
    const row = (o: Partial<Opponent>): Opponent => ({
      car_id: "event_car",
      car_skin: null,
      ai_level: null,
      driver_name: null,
      nationality: null,
      ballast: 0,
      restrictor: 0,
      ...o,
    });
    expect(gridRows(session, roster)).toEqual([
      row({ car_skin: "red", ai_level: 95, nationality: "Italy" }),
      row({ ai_level: 99 }),
      row({ car_id: "other", ballast: 30 }),
      row({}),
    ]);
  });
});

describe("opponent edits", () => {
  // What the table changes comes back as the story's keys, a 0 and an
  // `Auto` left out as the engine writes them.
  it("keep only what differs from the default", () => {
    const o = { driver: "star", car: "c", skin: "red", ballast: 10 };
    expect(patchOpponent(o, { car_skin: null, ballast: 0, restrictor: 20 })).toEqual({
      driver: "star",
      car: "c",
      restrictor: 20,
    });
    expect(withSkill(o, 60).ai_level).toBe(70);
    expect(withSkill({ ai_level: 90 }, null)).toEqual({});
    expect(withDriver(o, null)).toEqual({ car: "c", skin: "red", ballast: 10 });
  });

  // A livery belongs to its car; a driver races once per grid.
  it("drop the livery with the car, and the driver from a copy", () => {
    expect(withCar({ car: "a", skin: "red", driver: "star" }, "b")).toEqual({ car: "b", driver: "star" });
    expect(duplicateOpponent({ car: "a", skin: "red", driver: "star" })).toEqual({ car: "a", skin: "red" });
  });

  it("tell which drivers are already on the grid", () => {
    const opponents = [{ driver: "a" }, {}, { driver: "b" }];
    expect([...driversTaken(opponents, 0)]).toEqual(["b"]);
    expect(namedOnGrid({ opponents })).toEqual(["a", "b"]);
  });
});
