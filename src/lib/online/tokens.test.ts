import { describe, expect, it, vi } from "vitest";
import type { FilterMap, SignedValue } from "$lib/library/filters";
import { NO_LOOKS, type Looks } from "./looks";
import type { ServerSummary } from "./online";
import {
  DEFAULT_PINNED,
  onlineTokenDefs,
  parseTokens,
  splitPing,
  tokenOptions,
  tokenPredicate,
  trackLabel,
  type TokenContext,
} from "./tokens";

// The chip model labels its values through i18n, which needs the Svelte
// runtime; `vi.mock` is hoisted above the imports.
vi.mock("$lib/i18n/index.svelte", () => ({ t: (key: string) => key }));

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
    official: true,
    ...over,
  };
}

const track = (kunos_id: string, id = kunos_id, layout: string | null = null) => ({
  kunos_id,
  id,
  layout,
  csp_min_build: null,
});

const looks: Looks = {
  cars: {},
  tracks: {
    shuto_revival_project_beta: { name: "Shutoko Revival Project", categories: ["Freeroam"], layouts: {} },
    monza: { name: "Monza", categories: [], layouts: {} },
  },
};

function context(over: Partial<TokenContext> = {}): TokenContext {
  return {
    looks,
    pingOf: () => undefined,
    countryName: (iso2) => ({ DE: "Germany", JP: "Japan" })[iso2] ?? null,
    ...over,
  };
}

const defs = onlineTokenDefs();
const val = (key: string, ...values: (string | SignedValue)[]): FilterMap => ({
  [key]: {
    type: "val",
    op: "and",
    values: values.map((v) => (typeof v === "string" ? { value: v, sign: 1 } : v)),
  },
});
const names = (list: ServerSummary[], tokens: FilterMap, ctx = context()) =>
  list.filter(tokenPredicate(defs, tokens, ctx)).map((s) => s.name);

describe("online tokens (SPEC-play-online, Filtres de base)", () => {
  it("keeps the servers on a track's layout, and counts servers per layout", () => {
    const list = [
      server({ name: "spa", track: track("spa") }),
      server({ name: "monza", track: track("monza") }),
      server({ name: "monza again", track: track("MONZA") }),
    ];
    expect(names(list, val("track", "monza"))).toEqual(["monza", "monza again"]);
    const options = tokenOptions(defs[0], list, context());
    expect(options.find((o) => o.value === "monza")?.count, "Monza · 2 servers").toBe(2);
  });

  it("matches a car among the server's cars, and crosses two with AND", () => {
    const list = [
      server({ name: "both", cars: ["ks_mazda_miata", "KS_Toyota_AE86"] }),
      server({ name: "miata", cars: ["ks_mazda_miata"] }),
    ];
    expect(names(list, val("car", "ks_toyota_ae86")), "ids compare without case").toEqual(["both"]);
    expect(names(list, val("car", "ks_mazda_miata", "ks_toyota_ae86"))).toEqual(["both"]);
  });

  // The spec: a track missing from the library has no category, and counts as
  // unknown — neither included nor excluded.
  it("never hides a server whose track category is unknown", () => {
    const list = [
      server({ name: "shutoko", track: track("shuto_revival_project_beta") }),
      server({ name: "no category here", track: track("monza") }),
      server({ name: "not in the library", track: track("lac_canyons") }),
    ];
    expect(names(list, val("trackCategory", "Freeroam"))).toEqual(["shutoko", "not in the library"]);
    expect(names(list, val("trackCategory", { value: "freeroam", sign: -1 }))).toEqual([
      "no category here",
      "not in the library",
    ]);
  });

  it("tells Kunos servers from mod servers, and does not judge an old snapshot", () => {
    const list = [
      server({ name: "kunos" }),
      server({ name: "mods", official: false }),
      server({ name: "snapshot", official: undefined }),
    ];
    expect(names(list, val("content", "kunos"))).toEqual(["kunos", "snapshot"]);
    expect(names(list, val("content", "mods"))).toEqual(["mods", "snapshot"]);
  });

  it("keeps only servers measured under the ping limit", () => {
    const list = [
      server({ name: "near", ip: "1.1.1.1" }),
      server({ name: "far", ip: "2.2.2.2" }),
      server({ name: "not measured", ip: "3.3.3.3" }),
    ];
    const ms: Record<string, number> = { "1.1.1.1": 25, "2.2.2.2": 140 };
    const ctx = context({ pingOf: (s) => ms[s.ip] });
    expect(names(list, val("ping", "60"), ctx)).toEqual(["near"]);
    expect(names(list, val("ping", "200"), ctx)).toEqual(["near", "far"]);
  });

  it("reads the country as the game names it, for its flag", () => {
    const list = [server({ name: "de" }), server({ name: "jp", country: "JP" }), server({ name: "none", country: null })];
    expect(names(list, val("country", "Japan"))).toEqual(["jp"]);
    const countries = tokenOptions(defs.find((d) => d.key === "country")!, list, context());
    expect(countries.map((o) => o.value)).toEqual(["Germany", "Japan"]);
  });

  it("filters on the session running now", () => {
    const list = [server({ name: "race", session: "race" }), server({ name: "practice" })];
    expect(names(list, val("session", "race"))).toEqual(["race"]);
  });

  it("sets the ping token apart from the others", () => {
    const tokens = { ...val("ping", "60"), ...val("track", "monza") };
    expect(Object.keys(splitPing(tokens).rest)).toEqual(["track"]);
    expect(Object.keys(splitPing(tokens).ping)).toEqual(["ping"]);
    expect(splitPing(val("track", "monza")).ping).toEqual({});
  });

  it("names an unknown track by the lobby's id, layout included", () => {
    expect(trackLabel(NO_LOOKS, track("ks_nordschleife-endurance", "ks_nordschleife", "endurance"))).toBe(
      "ks_nordschleife-endurance",
    );
    expect(trackLabel(looks, track("monza"))).toBe("Monza");
  });

  // Stored beside the toggles, in a value older versions wrote without them.
  it("reads tokens and pins back, and falls back for an older or damaged value", () => {
    const older = JSON.stringify({ notFull: true, noPassword: true, notEmpty: false, joinable: false });
    expect(parseTokens(older)).toEqual({ tokens: {}, pinned: DEFAULT_PINNED });
    expect(parseTokens("{not json")).toEqual({ tokens: {}, pinned: DEFAULT_PINNED });
    const saved = JSON.stringify({ tokens: { ...val("track", "monza"), unknown: { type: "val" } }, pinned: ["car", "x"] });
    expect(parseTokens(saved)).toEqual({ tokens: val("track", "monza"), pinned: ["car"] });
  });
});
