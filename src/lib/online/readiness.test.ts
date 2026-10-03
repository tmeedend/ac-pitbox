import { describe, expect, it } from "vitest";
import { isJoinableLevel, joinState, layersToSetAside, worse } from "./readiness";
import type { CarSlots, ServerSummary } from "./online";

const server = (over: Partial<ServerSummary> = {}): ServerSummary =>
  ({
    track_available: true,
    track_level: "ready",
    level: "ready",
    blockers: [],
    ...over,
  }) as ServerSummary;

const car = (over: Partial<CarSlots> = {}): CarSlots => ({
  id: "ks_mazda_miata",
  total: 2,
  free: 1,
  skin: null,
  available: true,
  level: "ready",
  dlc: null,
  layers: [],
  fetch: {
    kept_archive: false,
    server_url: null,
    cup: false,
    server_version: null,
    installed_version: null,
    needed: false,
  },
  preview: null,
  ...over,
});

describe("online readiness (SPEC-play-online, Contenu manquant)", () => {
  it("takes the worse of two levels", () => {
    expect(worse("ready", "oneClick")).toBe("oneClick");
    expect(worse("blocked", "download")).toBe("blocked");
  });

  it("joins at the worse of the track and the chosen car", () => {
    expect(joinState(server(), car()).level).toBe("ready");
    const state = joinState(server({ track_level: "oneClick" }), car({ level: "oneClick" }));
    expect(state.level).toBe("oneClick");
    expect(state.toActivate).toEqual(["car", "track"]);
    expect(joinState(server(), car({ level: "download" })).level).toBe("download");
  });

  it("is blocked by a missing DLC car, named, or by the CSP whatever the car", () => {
    const dlc = joinState(server(), car({ level: "blocked", dlc: "Red Pack" }));
    expect(dlc.level).toBe("blocked");
    expect(dlc.blockers).toEqual([{ kind: "dlc", name: "Red Pack" }]);
    const csp = joinState(server({ blockers: [{ kind: "csp", required: 3465, installed: null }] }), car());
    expect(csp.level).toBe("blocked");
  });

  it("sets aside the certain layers unless kept, the possible ones only on request", () => {
    const conflicts = [
      { layer_id: "physics", name: "Physics", risk: "certain" as const },
      { layer_id: "sound", name: "Sound", risk: "possible" as const },
    ];
    expect(layersToSetAside(conflicts, { keepCertain: false, dropPossible: false })).toEqual(["physics"]);
    expect(layersToSetAside(conflicts, { keepCertain: true, dropPossible: false })).toEqual([]);
    expect(layersToSetAside(conflicts, { keepCertain: false, dropPossible: true })).toEqual(["physics", "sound"]);
  });

  it("keeps ready and one click as joinable, nothing else", () => {
    expect(isJoinableLevel("ready")).toBe(true);
    expect(isJoinableLevel("oneClick")).toBe(true);
    expect(isJoinableLevel("download")).toBe(false);
    expect(isJoinableLevel("blocked")).toBe(false);
    expect(isJoinableLevel(undefined)).toBe(false);
  });
});
