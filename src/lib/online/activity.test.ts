import { describe, expect, it, vi } from "vitest";
import type { ServerSummary } from "./online";

// `activity.ts` reads the track key from the tokens module, which labels its
// values through i18n; `vi.mock` is hoisted above the imports.
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

const { trackActivity } = await import("./activity");

function server(track: string, layout: string | null, clients: number): ServerSummary {
  return {
    ip: `${track}-${layout}-${clients}`,
    port: 9600,
    http_port: 8081,
    name: "s",
    country: null,
    clients,
    max_clients: 24,
    password: false,
    booking: false,
    track: { kunos_id: layout ? `${track}-${layout}` : track, id: track, layout, csp_min_build: null },
    cars: [],
    session: null,
    sessions: [],
    time_left: 0,
    track_available: true,
    cars_available: 1,
  };
}

describe("a track's activity online (SPEC-play-online, v2)", () => {
  it("counts occupied servers and players on any layout of the track", () => {
    const list = [
      server("shuto", "main", 10),
      server("SHUTO", "c1", 3),
      server("shuto", "main", 0),
      server("monza", null, 20),
    ];
    expect(trackActivity(list, "shuto")).toEqual({ servers: 2, players: 13, layouts: ["shuto-main", "shuto-c1"] });
  });

  it("says nothing of a track nobody drives", () => {
    expect(trackActivity([server("shuto", "main", 0)], "shuto")).toEqual({ servers: 0, players: 0, layouts: [] });
  });
});
