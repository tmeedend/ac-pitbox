import { describe, expect, it } from "vitest";
import {
  EMPTY_STORE,
  isFavourite,
  parseStore,
  recentCars,
  recordJoin,
  RECENTS_KEPT,
  tabServers,
  toggleFavourite,
} from "./lists";
import type { ServerSummary } from "./online";

function server(ip: string, over: Partial<ServerSummary> = {}): ServerSummary {
  return {
    ip,
    port: 9600,
    http_port: 8081,
    name: ip,
    country: null,
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

describe("online favourites and recents (SPEC-play-online, case 1)", () => {
  it("toggles a favourite on and off by its address", () => {
    const on = toggleFavourite(EMPTY_STORE, server("1.1.1.1"));
    expect(isFavourite(on, "1.1.1.1:8081")).toBe(true);
    const off = toggleFavourite(on, server("1.1.1.1", { name: "renamed" }));
    expect(isFavourite(off, "1.1.1.1:8081")).toBe(false);
  });

  it("keeps one line per server, the newest join first, with its car", () => {
    let store = recordJoin(EMPTY_STORE, server("1.1.1.1"), "ks_mazda_miata", "2026-10-01");
    store = recordJoin(store, server("2.2.2.2"), "bmw_m3_e30", "2026-10-02");
    store = recordJoin(store, server("1.1.1.1"), "ks_toyota_ae86", "2026-10-03");
    expect(store.recents.map((r) => r.server.ip)).toEqual(["1.1.1.1", "2.2.2.2"]);
    expect(recentCars(store)["1.1.1.1:8081"]).toBe("ks_toyota_ae86");
  });

  it("keeps a bounded number of recent joins", () => {
    let store = EMPTY_STORE;
    for (let i = 0; i < RECENTS_KEPT + 5; i++) store = recordJoin(store, server(`10.0.0.${i}`), "car", `${i}`);
    expect(store.recents).toHaveLength(RECENTS_KEPT);
    expect(store.recents[0].server.ip).toBe(`10.0.0.${RECENTS_KEPT + 4}`);
  });

  it("shows the lobby's fresh entry, and the snapshot when the lobby lost it", () => {
    const store = { ...EMPTY_STORE, favourites: [server("1.1.1.1", { clients: 1 }), server("9.9.9.9")] };
    const shown = tabServers("favourites", [server("1.1.1.1", { clients: 12 })], store);
    expect(shown.map((s) => [s.ip, s.clients])).toEqual([
      ["1.1.1.1", 12],
      ["9.9.9.9", 0],
    ]);
  });

  it("reads a damaged or older file without losing the good entries", () => {
    expect(parseStore(null)).toEqual(EMPTY_STORE);
    const store = parseStore({ favourites: [server("1.1.1.1"), { ip: 3 }], recents: [{ car: "x" }] });
    expect(store.favourites).toHaveLength(1);
    expect(store.recents).toEqual([]);
  });
});
