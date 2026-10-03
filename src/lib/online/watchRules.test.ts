import { describe, expect, it } from "vitest";
import { EMPTY_STORE } from "./lists";
import type { ServerSummary } from "./online";
import { newSightings, slotFreed, watchedServers } from "./watchRules";

const server = (ip: string): ServerSummary =>
  ({ ip, http_port: 8081, track: { id: "monza", kunos_id: "monza", layout: null, csp_min_build: null } }) as ServerSummary;

describe("the background watch (SPEC-play-online, v2)", () => {
  // Never the 9 000 servers: only those the user named.
  it("watches the favourites and the recent joins, each once", () => {
    const store = {
      ...EMPTY_STORE,
      favourites: [server("1.1.1.1"), server("2.2.2.2")],
      recents: [{ server: server("2.2.2.2"), car: "", at: "" }, { server: server("3.3.3.3"), car: "", at: "" }],
    };
    expect(watchedServers(store).map((s) => s.ip)).toEqual(["1.1.1.1", "2.2.2.2", "3.3.3.3"]);
  });

  it("tells a friend arriving, not one already there, and one moving server", () => {
    const before = { a: ["Léo"], b: ["Max"] };
    const now = { a: ["léo ", "Sam"], c: ["Max"] };
    expect(newSightings(before, now)).toEqual([
      { key: "a", name: "Sam" },
      { key: "c", name: "Max" },
    ]);
  });

  it("frees a slot for the chosen car, or for any car without one", () => {
    const counts = [
      { car: "ks_mazda_miata", free: 0, total: 2 },
      { car: "bmw_m3_e30", free: 1, total: 1 },
    ];
    expect(slotFreed(counts, "KS_Mazda_Miata")).toBe(false);
    expect(slotFreed(counts, "bmw_m3_e30")).toBe(true);
    expect(slotFreed(counts, null)).toBe(true);
    expect(slotFreed([], null)).toBe(false);
  });
});
