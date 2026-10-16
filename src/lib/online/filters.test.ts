import { describe, expect, it } from "vitest";
import { searchServers, sortServers } from "./filters";
import type { ServerSummary } from "./online";
import { testServer } from "./testServer";

function server(over: Partial<ServerSummary>): ServerSummary {
  return testServer(over);
}

describe("online server search and order (SPEC-play-online, Liste des serveurs)", () => {
  it("searches every word in the name, the track and the cars", () => {
    const list = [
      server({ name: "Drift Club", track: { kunos_id: "drift", id: "drift", layout: null, csp_min_build: null } }),
      server({ name: "Nords", cars: ["ks_toyota_celica_st185"] }),
    ];
    const found = (search: string) => searchServers(list, search).map((s) => s.name);
    expect(found("celica")).toEqual(["Nords"]);
    expect(found("DRIFT club")).toEqual(["Drift Club"]);
    expect(found("drift celica")).toEqual([]);
    expect(found("  ")).toEqual(["Drift Club", "Nords"]);
  });

  it("puts ready before one click, then the busiest", () => {
    const list = [
      server({ name: "busy one click", clients: 20, level: "oneClick" }),
      server({ name: "quiet ready", clients: 1, level: "ready" }),
      server({ name: "busy blocked", clients: 30, level: "blocked" }),
    ];
    expect(sortServers(list).map((s) => s.name)).toEqual(["quiet ready", "busy one click", "busy blocked"]);
  });

  it("puts what can be joined first, then the busiest", () => {
    const list = [
      server({ name: "busy but missing track", clients: 20, track_available: false }),
      server({ name: "quiet ready", clients: 2 }),
      server({ name: "busy ready", clients: 12 }),
    ];
    expect(sortServers(list).map((s) => s.name)).toEqual(["busy ready", "quiet ready", "busy but missing track"]);
  });
});
