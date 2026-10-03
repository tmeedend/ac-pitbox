import { describe, expect, it } from "vitest";
import { DEFAULT_FILTERS, filterServers, parseFilters, sortServers } from "./filters";
import type { ServerSummary } from "./online";

function server(over: Partial<ServerSummary>): ServerSummary {
  return {
    ip: "1.2.3.4",
    port: 9600,
    http_port: 8081,
    name: "Server",
    country: "DE",
    clients: 0,
    max_clients: 24,
    password: false,
    booking: false,
    track: { kunos_id: "monza", id: "monza", layout: null, csp_min_build: null },
    cars: ["ks_mazda_miata"],
    session: "practice",
    sessions: ["practice"],
    time_left: 600,
    track_available: true,
    cars_available: 1,
    ...over,
  };
}

describe("online server filters (SPEC-play-online, Filtres de base)", () => {
  it("hides full and password-protected servers by default, keeps empty ones", () => {
    const list = [
      server({ name: "full", clients: 24 }),
      server({ name: "locked", password: true }),
      server({ name: "empty", clients: 0 }),
    ];
    expect(filterServers(list, DEFAULT_FILTERS).map((s) => s.name)).toEqual(["empty"]);
  });

  it("searches every word in the name, the track and the cars", () => {
    const list = [
      server({ name: "Drift Club", track: { kunos_id: "drift", id: "drift", layout: null, csp_min_build: null } }),
      server({ name: "Nords", cars: ["ks_toyota_celica_st185"] }),
    ];
    const found = (search: string) => filterServers(list, { ...DEFAULT_FILTERS, search }).map((s) => s.name);
    expect(found("celica")).toEqual(["Nords"]);
    expect(found("DRIFT club")).toEqual(["Drift Club"]);
    expect(found("drift celica")).toEqual([]);
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

  it("falls back to the defaults for a missing or damaged stored value", () => {
    expect(parseFilters(null)).toEqual(DEFAULT_FILTERS);
    expect(parseFilters("{not json")).toEqual(DEFAULT_FILTERS);
    expect(parseFilters(JSON.stringify({ notFull: false, joinable: "yes" }))).toEqual({
      ...DEFAULT_FILTERS,
      notFull: false,
    });
  });
});
