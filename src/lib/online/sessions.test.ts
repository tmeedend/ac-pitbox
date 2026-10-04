import { describe, expect, it } from "vitest";
import { isOpenPractice, sessionTimeline } from "./sessions";
import type { ServerSummary } from "./online";

const server = (over: Partial<ServerSummary>) => ({ sessions: [], session: null, ...over }) as ServerSummary;

describe("online session timeline (SPEC-play-online, Frise des sessions)", () => {
  it("reads practice and qualify in time, a lap race in laps, as CM does", () => {
    const steps = sessionTimeline(
      server({ sessions: ["practice", "qualify", "race"], durations: [600, 600, 5], timed: false, session: "qualify" }),
    );
    expect(steps.map((s) => s.duration)).toEqual([
      { unit: "min", minutes: 10 },
      { unit: "min", minutes: 10 },
      { unit: "laps", laps: 5 },
    ]);
    expect(steps.map((s) => s.active)).toEqual([false, true, false]);
  });

  it("reads a timed race in time, with its extra lap, and long sessions in hours", () => {
    const steps = sessionTimeline(
      server({ sessions: ["practice", "race"], durations: [86400, 1800], timed: true, extra_lap: true }),
    );
    expect(steps[0].duration).toEqual({ unit: "h", hours: 24 });
    expect(steps[1]).toMatchObject({ duration: { unit: "min", minutes: 30 }, extraLap: true });
  });

  it("leaves a duration out when the server gives none", () => {
    expect(sessionTimeline(server({ sessions: ["race"], durations: [0] }))[0].duration).toBeNull();
    expect(sessionTimeline(server({ sessions: ["practice"] }))[0].duration).toBeNull();
  });
  // Reported: « Practice 200 h » in the panel while the table said « Open ».
  it("reads a practice of more than three hours as open", () => {
    expect(isOpenPractice("practice", 200 * 3600)).toBe(true);
    expect(isOpenPractice("practice", 3 * 3600)).toBe(false);
    expect(isOpenPractice("race", 200 * 3600), "a long race is still a race").toBe(false);
  });
});
