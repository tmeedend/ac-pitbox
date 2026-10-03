// The token filters of the Online page (SPEC-play-online.md, "Filtres de
// base"): track, car, track category, content, ping, country, session. They
// speak the library's chip model (`$lib/library/filters`), so the page reuses
// its filter bar rather than a copy — as the game folder does — and a token
// means the same thing on both screens. Pure, for Vitest.
//
// One rule the library does not have: a value the page cannot know is
// **unknown**, and an unknown value neither matches nor fails. The spec says
// it for the track category ("ni inclus ni exclu, sinon on masque en silence
// les serveurs qu'on cherche justement à découvrir"): a track missing from the
// library has no category, and most of the lobby's tracks are missing.
import {
  optionsFrom,
  sanitizeFilterMap,
  valTest,
  type FilterDef,
  type FilterMap,
  type FilterOption,
} from "$lib/library/filters";
import { trackTitle, type Looks } from "./looks";
import type { ServerSummary, TrackRef } from "./online";

export const TRACK_KEY = "track";
export const CAR_KEY = "car";
export const CATEGORY_KEY = "trackCategory";
export const CONTENT_KEY = "content";
export const PING_KEY = "ping";
export const COUNTRY_KEY = "country";
export const SESSION_KEY = "session";

/** The spec's thresholds: green under the first, orange under the second
 * (the list's colours), and a third for "not across the world". A ping token
 * offers each as "under N ms". */
export const PING_GOOD_MS = 60;
export const PING_FAIR_MS = 120;
const PING_LIMITS = [PING_GOOD_MS, PING_FAIR_MS, 200];

/** Ghost chips out of the box: the combos one drives (case 5 of the spec) and
 * the mood of a server (case 3). */
export const DEFAULT_PINNED = [TRACK_KEY, CAR_KEY, CATEGORY_KEY];

/** Labels the screen supplies: the names the library knows, and the words of
 * the current language. Absent, a value shows as stored. */
export interface TokenLabels {
  track?: (value: string) => string;
  car?: (value: string) => string;
  ping?: (value: string) => string;
}

/** The catalogue of the page, in the order of the add menu. */
export function onlineTokenDefs(labels: TokenLabels = {}): FilterDef[] {
  return [
    { key: TRACK_KEY, labelKey: "online.filterTrack", type: "val", labelOf: labels.track },
    // A server carries several cars and a track several categories: the
    // operator is offered where an AND can match.
    { key: CAR_KEY, labelKey: "online.filterCar", type: "val", operator: true, labelOf: labels.car },
    { key: CATEGORY_KEY, labelKey: "online.filterTrackCategory", type: "val", operator: true },
    {
      key: CONTENT_KEY,
      labelKey: "online.filterContent",
      type: "val",
      choices: [
        { value: "kunos", labelKey: "online.contentKunos" },
        { value: "mods", labelKey: "online.contentMods" },
      ],
    },
    {
      key: PING_KEY,
      labelKey: "online.filterPing",
      type: "val",
      choices: PING_LIMITS.map((ms) => ({ value: String(ms) })),
      labelOf: labels.ping,
    },
    // Each value is the game's English name of the country, so the flag and
    // the translated label are looked up as in the library (`flags`).
    { key: COUNTRY_KEY, labelKey: "online.filterCountry", type: "val", flags: true },
    {
      key: SESSION_KEY,
      labelKey: "online.filterSession",
      type: "val",
      // Written out, so the locale check sees every key.
      choices: [
        { value: "booking", labelKey: "online.session.booking" },
        { value: "practice", labelKey: "online.session.practice" },
        { value: "qualify", labelKey: "online.session.qualify" },
        { value: "race", labelKey: "online.session.race" },
      ],
    },
  ];
}

/** What the tokens read about a server that the server itself does not say. */
export interface TokenContext {
  looks: Looks;
  /** The measured round trip, `undefined` until measured — or when the
   * server did not answer. */
  pingOf: (s: ServerSummary) => number | undefined;
  /** The game's English name of an ISO country code, `null` when the game
   * does not know it (or its table is not loaded yet). */
  countryName: (iso2: string) => string | null;
}

/** The value of a track token: the layout, as the lobby names it. */
export function trackValue(track: TrackRef): string {
  return track.kunos_id.toLowerCase();
}

/** A track as a token shows it: its name here, else the lobby's id — the
 * folder alone would not tell two layouts of an unknown track apart. */
export function trackLabel(looks: Looks, track: TrackRef): string {
  return looks.tracks[track.id.toLowerCase()] ? trackTitle(looks, track) : track.kunos_id;
}

/** Values a server carries for a token, `null` when they are unknown. */
function valuesOf(key: string, ctx: TokenContext): (s: ServerSummary) => string[] | null {
  switch (key) {
    case TRACK_KEY:
      return (s) => [trackValue(s.track)];
    case CAR_KEY:
      return (s) => s.cars.map((c) => c.toLowerCase());
    case CATEGORY_KEY:
      return (s) => ctx.looks.tracks[s.track.id.toLowerCase()]?.categories ?? null;
    case CONTENT_KEY:
      // A snapshot saved before servers were judged on it does not say.
      return (s) => (s.official === undefined ? null : [s.official ? "kunos" : "mods"]);
    case PING_KEY:
      // Not measured is not near: the list measures the servers a ping
      // token keeps asking about (`measureAll`), and shows them as they answer.
      return (s) => {
        const ms = ctx.pingOf(s);
        return ms === undefined ? [] : PING_LIMITS.filter((limit) => ms < limit).map(String);
      };
    case COUNTRY_KEY:
      return (s) => (s.country ? [ctx.countryName(s.country) ?? s.country] : []);
    case SESSION_KEY:
      return (s) => (s.session ? [s.session] : []);
    default:
      return () => [];
  }
}

/** Compiles the posed tokens into one predicate. Resolved once per filter,
 * never per server: this runs over the 9 000 servers of the lobby. */
export function tokenPredicate(
  defs: FilterDef[],
  tokens: FilterMap,
  ctx: TokenContext,
): (s: ServerSummary) => boolean {
  const tests: ((s: ServerSummary) => boolean)[] = [];
  for (const def of defs) {
    const st = tokens[def.key];
    if (st?.type !== "val") continue;
    const test = valTest(def, st);
    if (!test) continue;
    const get = valuesOf(def.key, ctx);
    tests.push((s) => {
      const values = get(s);
      return values === null || test(values);
    });
  }
  return (s) => tests.every((test) => test(s));
}

/** The suggestions of a token, counted over `servers`. An unknown value
 * counts nowhere. */
export function tokenOptions(def: FilterDef, servers: ServerSummary[], ctx: TokenContext): FilterOption[] {
  const get = valuesOf(def.key, ctx);
  return optionsFrom(def, servers, (s) => get(s) ?? []);
}

/** The ping token apart from the others: it is the one the list cannot
 * answer before measuring, so the servers it is asked about are those that
 * pass every other filter. */
export function splitPing(tokens: FilterMap): { rest: FilterMap; ping: FilterMap } {
  const { [PING_KEY]: ping, ...rest } = tokens;
  return { rest, ping: ping ? { [PING_KEY]: ping } : {} };
}

/** Whether a ping token says something. */
export function asksPing(tokens: FilterMap): boolean {
  const st = tokens[PING_KEY];
  return st?.type === "val" && st.values.length > 0;
}

export interface StoredTokens {
  tokens: FilterMap;
  pinned: string[];
}

/** Reads the tokens and pinned chips stored beside the toggles
 * (`StorageKey.onlineFilters`). A value written before tokens existed, or
 * damaged, gives none posed and the default pins — never an empty list. */
export function parseTokens(raw: string | null): StoredTokens {
  const defs = onlineTokenDefs();
  const fallback = { tokens: {}, pinned: [...DEFAULT_PINNED] };
  if (!raw) return fallback;
  try {
    const stored = JSON.parse(raw) as { tokens?: unknown; pinned?: unknown };
    const pinned = Array.isArray(stored.pinned)
      ? stored.pinned.filter((k): k is string => typeof k === "string" && defs.some((d) => d.key === k))
      : fallback.pinned;
    return { tokens: sanitizeFilterMap(stored.tokens, defs), pinned };
  } catch {
    return fallback;
  }
}
