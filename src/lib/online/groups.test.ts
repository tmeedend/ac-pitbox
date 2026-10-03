import { describe, expect, it, vi } from "vitest";
import type { ServerSummary } from "./online";

// `groups.ts` reads the track key from the tokens module, which labels its
// values through i18n; `vi.mock` is hoisted above the imports.
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

const { groupByTrack, serverRows } = await import("./groups");

function server(name: string, track: string, over: Partial<ServerSummary> = {}): ServerSummary {
  return {
    ip: name,
    port: 9600,
    http_port: 8081,
    name,
    country: null,
    clients: 1,
    max_clients: 10,
    password: false,
    booking: false,
    track: { kunos_id: track, id: track, layout: null, csp_min_build: null },
    cars: [],
    session: null,
    sessions: [],
    time_left: 0,
    track_available: true,
    cars_available: 1,
    ...over,
  };
}

const shape = (rows: ReturnType<typeof groupByTrack>) =>
  rows.map((r) => (r.kind === "group" ? `[${r.key} ×${r.servers.length}]` : r.nested ? `  ${r.server.name}` : r.server.name));

describe("servers grouped by track (SPEC-play-online, Au-delà de CM)", () => {
  const list = [
    server("shutoko 1", "shuto", { clients: 20, level: "oneClick" }),
    server("monza", "monza"),
    server("shutoko 2", "shuto", { clients: 5, level: "ready" }),
    server("shutoko 3", "SHUTO", { clients: 0, level: "blocked" }),
  ];

  it("folds a layout run by several servers where its first server stood", () => {
    expect(shape(groupByTrack(list, new Set()))).toEqual(["[shuto ×3]", "monza"]);
  });

  it("sums the players and keeps the best level of the group", () => {
    const group = groupByTrack(list, new Set())[0];
    expect(group.kind === "group" && [group.clients, group.maxClients, group.level]).toEqual([25, 30, "ready"]);
  });

  it("lists an unfolded group's servers under it, in the list's order", () => {
    expect(shape(groupByTrack(list, new Set(["shuto"])))).toEqual([
      "[shuto ×3]",
      "  shutoko 1",
      "  shutoko 2",
      "  shutoko 3",
      "monza",
    ]);
  });

  it("keeps a single server's plain row, and the plain list ungrouped", () => {
    expect(shape(groupByTrack([server("monza", "monza")], new Set()))).toEqual(["monza"]);
    expect(shape(serverRows(list))).toEqual(["shutoko 1", "monza", "shutoko 2", "shutoko 3"]);
  });
});
