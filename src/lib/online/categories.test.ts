import { describe, expect, it } from "vitest";
import { keywordHints, serverCategories, words, type CategoryKnowledge } from "./categories";
import type { Looks } from "./looks";
import { testServer } from "./testServer";

const KEYWORDS = { drift: "#drift", touge: "#touge", traffic: "#freeroam", cruise: "#freeroam", "no hesi": "#freeroam" };

const knowledge = (k: Partial<CategoryKnowledge> = {}): CategoryKnowledge => ({
  vocabulary: ["#drift", "#touge", "#freeroam", "#circuit"],
  chosen: {},
  catalog: {},
  keywords: KEYWORDS,
  ...k,
});

const looks = (tracks: Record<string, string[]> = {}): Looks => ({
  cars: {},
  tracks: Object.fromEntries(Object.entries(tracks).map(([id, categories]) => [id, { name: id, categories, layouts: {} }])),
});

const server = (name: string, track: string) =>
  testServer({ name, track: { id: track, kunos_id: track, layout: null, csp_min_build: null } });

describe("track categories of a server (SPEC-play-online.md)", () => {
  it("takes the user's choice, then the library, then the catalogue", () => {
    const k = knowledge({ chosen: { spa: ["#drift"] }, catalog: { spa: ["#circuit"], monza: ["#circuit"] } });
    const l = looks({ spa: ["#circuit"], imola: ["#Circuit"] });
    expect(serverCategories(server("x", "SPA"), l, k)).toMatchObject({ base: ["#drift"], source: "chosen" });
    expect(serverCategories(server("x", "imola"), l, k)).toMatchObject({ base: ["#circuit"], source: "library" });
    expect(serverCategories(server("x", "monza"), l, k)).toMatchObject({ base: ["#circuit"], source: "catalog" });
  });

  it("lets the catalogue answer for a track the library holds without any category", () => {
    const c = serverCategories(server("x", "pk_akina"), looks({ pk_akina: [] }), knowledge({ catalog: { pk_akina: ["#touge"] } }));
    expect(c.source).toBe("catalog");
  });

  it("is unknown when no source answers and no word announces anything", () => {
    expect(serverCategories(server("Sunday league", "la_canyons"), looks(), knowledge()).all).toEqual([]);
  });

  it("adds the server's words to the track's category, never in its place", () => {
    const c = serverCategories(server("[EU] Drift & touge night", "spa"), looks({ spa: ["#circuit"] }), knowledge());
    expect(c.base).toEqual(["#circuit"]);
    expect(c.all).toEqual(["#circuit", "#drift", "#touge"]);
    expect(c.hints[0]).toEqual({ category: "#drift", word: "drift", source: "name" });
  });

  it("reads the description and the track's id, the name first", () => {
    const k = knowledge();
    const c = serverCategories(server("Midnight club", "ek_akina_drift"), looks(), k, "Chill cruise, no ghosting");
    expect(c.hints).toEqual([
      { category: "#freeroam", word: "cruise", source: "description" },
      { category: "#drift", word: "drift", source: "track" },
    ]);
  });
});

describe("keywords (SPEC-play-online.md)", () => {
  it("match whole words, without case or accents", () => {
    expect(keywordHints("DRÏFT kings", "name", KEYWORDS)).toHaveLength(1);
    expect(keywordHints("driftkings", "name", KEYWORDS)).toEqual([]);
    expect(words("ek_akina-Drift.v2")).toEqual(["ek", "akina", "drift", "v2"]);
  });

  it("ignore a keyword preceded by « no »", () => {
    expect(keywordHints("Race server | No Drift | No traffic", "name", KEYWORDS)).toEqual([]);
    expect(keywordHints("No ghosting, drift only", "name", KEYWORDS)).toHaveLength(1);
  });

  it("match a phrase of several words as one", () => {
    expect(keywordHints("NoHesi | No Hesi traffic", "name", { "no hesi": "#freeroam" })).toHaveLength(1);
    expect(keywordHints("no idea, hesi", "name", { "no hesi": "#freeroam" })).toEqual([]);
  });
});
