import { describe, expect, it } from "vitest";
import { criteriaOf, eventPlaces, sessionFor, successFor, successOf } from "./events";
import type { Chapter } from "./studio";

describe("eventPlaces", () => {
  // The list reads in the order the story plays: an event under the chapter
  // that first launches it, then the side events, then what nothing uses.
  it("orders the events as the story plays them, each once", () => {
    const chapters: Chapter[] = [
      {
        id: "ch01",
        title: "One",
        nodes: [
          { id: "a", event: "race_01" },
          { id: "b", side_events: { open: ["track_day"] } },
        ],
      },
      { id: "ch02", nodes: [{ id: "c", event: "race_02" }, { id: "d", event: "race_01" }] },
    ];
    const places = eventPlaces(chapters, ["hot_lap", "race_01", "race_02", "track_day"]);
    expect(places.chapters).toEqual([
      { index: 0, title: "One", ids: ["race_01"] },
      { index: 1, title: "ch02", ids: ["race_02"] },
    ]);
    expect(places.side).toEqual(["track_day"]);
    expect(places.unused).toEqual(["hot_lap"]);
  });

  it("ignores a node pointing to an event that does not exist", () => {
    const chapters: Chapter[] = [{ id: "ch01", nodes: [{ id: "a", event: "ghost" }] }];
    expect(eventPlaces(chapters, []).chapters).toEqual([]);
  });
});

describe("criteria", () => {
  // The form edits what it knows and keeps the rest: a sentence it has no
  // control for, written by hand, survives an edit of the position.
  it("reads and writes back the sentences the form knows, keeping the others", () => {
    const c = criteriaOf(["ahead_of rival", "finished", "position <= 3", "best_lap < 1:18.200", "something else"]);
    expect(c).toEqual({
      position: 3,
      finished: true,
      lapUnder: "1:18.200",
      aheadOf: ["rival"],
      others: ["something else"],
    });
    c.position = 1;
    c.aheadOf.push("star");
    expect(successOf(c)).toEqual([
      "position <= 1",
      "finished",
      "best_lap < 1:18.200",
      "ahead_of rival",
      "ahead_of star",
      "something else",
    ]);
    expect(successOf(criteriaOf([]))).toEqual([]);
  });
});

describe("sessionFor", () => {
  // A hot lap has no grid: the validator refuses race-only keys there, so a
  // change of type drops them rather than leave an error behind.
  it("drops what only a race has when the type changes", () => {
    const race = {
      track: "t",
      car: "c",
      laps: 8,
      grid: 12,
      ai_level: 90,
      start: "last" as const,
      time: "10:00",
      opponents: [{ driver: "rival" }],
    };
    expect(sessionFor("hot_lap", race)).toEqual({ track: "t", car: "c", time: "10:00" });
    expect(sessionFor("race", race)).toBe(race);
  });

  // Its grid gone, a hot lap cannot be won by beating a driver: the
  // criterion goes with the grid, the others stay as written.
  it("drops the drivers to beat with the grid", () => {
    const success = ["position <= 3", "ahead_of rival", "finished"];
    expect(successFor("hot_lap", success)).toEqual(["position <= 3", "finished"]);
    expect(successFor("race", success)).toBe(success);
  });
});
