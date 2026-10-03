import { describe, expect, it, vi } from "vitest";
import type { ServerSummary } from "./online";
import { NO_LOOKS } from "./looks";

// `groups.ts` reads the track key from the tokens module, which labels its
// values through i18n; `vi.mock` is hoisted above the imports.
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

const { arrangeRows } = await import("./groups");
const { nextSort } = await import("./columns");

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

const ctx = { looks: NO_LOOKS, pingOf: (s: ServerSummary) => (s.name === "fast" ? 20 : undefined) };
const rows = (list: ServerSummary[], how: Partial<Parameters<typeof arrangeRows>[1]> = {}) =>
  arrangeRows(list, { grouped: true, sort: null, open: new Set(), ...how }, ctx).map((r) =>
    r.kind === "group" ? `[${r.key} ×${r.servers.length}]` : r.nested ? `  ${r.server.name}` : r.server.name,
  );

describe("server table rows (SPEC-play-online, Au-delà de CM)", () => {
  // Shutoko runs hundreds of empty servers: the count says nothing of what is driven.
  const list = [
    server("shutoko 1", "shuto", { clients: 3 }),
    server("monza", "monza", { clients: 20 }),
    server("shutoko 2", "shuto", { clients: 2 }),
    server("shutoko 3", "SHUTO", { clients: 0 }),
    server("spa 1", "spa", { clients: 4 }),
    server("spa 2", "spa", { clients: 4 }),
  ];

  it("orders groups by total players, a lone server as a group of one", () => {
    expect(rows(list)).toEqual(["monza", "[spa ×2]", "[shuto ×3]"]);
  });

  it("sums the players of a group", () => {
    const group = arrangeRows(list, { grouped: true, sort: null, open: new Set() }, ctx)[2];
    expect(group.kind === "group" && [group.clients, group.maxClients]).toEqual([5, 30]);
  });

  it("lists an unfolded group's servers under it, in the default order", () => {
    expect(rows(list, { open: new Set(["shuto"]) })).toEqual([
      "monza",
      "[spa ×2]",
      "[shuto ×3]",
      "  shutoko 1",
      "  shutoko 2",
      "  shutoko 3",
    ]);
  });

  it("sorts groups by a column's aggregate, and the servers inside by their value", () => {
    expect(rows(list, { sort: { key: "server", dir: -1 } }), "most servers first").toEqual([
      "[shuto ×3]",
      "[spa ×2]",
      "monza",
    ]);
    expect(rows(list, { sort: { key: "players", dir: 1 }, open: new Set(["shuto"]) })).toEqual([
      "[shuto ×3]",
      "  shutoko 3",
      "  shutoko 2",
      "  shutoko 1",
      "[spa ×2]",
      "monza",
    ]);
  });

  it("keeps the groups' order under a column without an aggregate", () => {
    const pinged = [...list, server("fast", "shuto", { clients: 0 })];
    expect(rows(pinged, { sort: { key: "ping", dir: 1 }, open: new Set(["shuto"]) })).toEqual([
      "monza",
      "[spa ×2]",
      "[shuto ×4]",
      "  fast",
      "  shutoko 1",
      "  shutoko 2",
      "  shutoko 3",
    ]);
  });

  it("lists every server ungrouped, sorted with unmeasured pings last both ways", () => {
    const pinged = [server("slow", "a"), server("fast", "b")];
    const flat = (dir: 1 | -1) =>
      arrangeRows(pinged, { grouped: false, sort: { key: "ping", dir }, open: new Set() }, ctx).map(
        (r) => r.kind === "server" && r.server.name,
      );
    expect(flat(1)).toEqual(["fast", "slow"]);
    expect(flat(-1)).toEqual(["fast", "slow"]);
  });

  it("cycles a header click: ascending, descending, back to the default", () => {
    expect(nextSort(null, "players")).toEqual({ key: "players", dir: 1 });
    expect(nextSort({ key: "players", dir: 1 }, "players")).toEqual({ key: "players", dir: -1 });
    expect(nextSort({ key: "players", dir: -1 }, "players")).toBeNull();
    expect(nextSort({ key: "players", dir: -1 }, "ping")).toEqual({ key: "ping", dir: 1 });
  });
});
