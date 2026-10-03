// The chip filters of the Online page (SPEC-play-online.md, "Filtres de
// base"): four yes/no chips — not full, no password, not empty, joinable —
// and the tokens: track, car, track category, content, ping, country,
// session. They
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
import { isJoinable } from "./filters";
import { trackTitle, type Looks } from "./looks";
import type { ServerSummary, TrackRef } from "./online";

export const TRACK_KEY = "track";
export const CAR_KEY = "car";
export const CATEGORY_KEY = "trackCategory";
export const CONTENT_KEY = "content";
export const PING_KEY = "ping";
export const COUNTRY_KEY = "country";
export const SESSION_KEY = "session";

/** The four yes/no chips, once checkboxes. They sort out the 9 000 public
 * servers and stay off one's own tabs, where a favourite must show full,
 * locked or empty: that is exactly when one goes looking for it. */
export const NOT_FULL_KEY = "notFull";
export const NO_PASSWORD_KEY = "noPassword";
export const NOT_EMPTY_KEY = "notEmpty";
export const JOINABLE_KEY = "joinable";
export const BOOL_KEYS = [NOT_FULL_KEY, NO_PASSWORD_KEY, NOT_EMPTY_KEY, JOINABLE_KEY];

/** The spec's thresholds: green under the first, orange under the second
 * (the list's colours), and a third for "not across the world". A ping token
 * offers each as "under N ms". */
export const PING_GOOD_MS = 60;
export const PING_FAIR_MS = 120;
const PING_LIMITS = [PING_GOOD_MS, PING_FAIR_MS, 200];

/** Pinned out of the box: the four yes/no chips, always in reach as the
 * checkboxes were; the combos one drives (case 5 of the spec) and the mood of
 * a server (case 3). */
export const DEFAULT_PINNED = [...BOOL_KEYS, TRACK_KEY, CAR_KEY, CATEGORY_KEY];

/** Posed out of the box: a full server is no place to go, unless one asks. */
export const DEFAULT_TOKENS: FilterMap = { [NOT_FULL_KEY]: { type: "bool", sign: 1 } };

/** Labels the screen supplies: the names the library knows, and the words of
 * the current language. Absent, a value shows as stored. */
export interface TokenLabels {
  track?: (value: string) => string;
  car?: (value: string) => string;
  ping?: (value: string) => string;
}

/** The catalogue of the page, in the order of the add menu. Without the
 * yes/no chips (`bools: false`) on one's own tabs. */
export function onlineTokenDefs(labels: TokenLabels = {}, bools = true): FilterDef[] {
  const yesNo: FilterDef[] = [
    { key: NOT_FULL_KEY, labelKey: "online.filterNotFull", negLabelKey: "online.filterFull", type: "bool" },
    { key: NO_PASSWORD_KEY, labelKey: "online.filterNoPassword", negLabelKey: "online.filterPassword", type: "bool" },
    { key: NOT_EMPTY_KEY, labelKey: "online.filterNotEmpty", negLabelKey: "online.filterEmpty", type: "bool" },
    { key: JOINABLE_KEY, labelKey: "online.filterJoinable", negLabelKey: "online.filterNotJoinable", type: "bool" },
  ];
  return [
    ...(bools ? yesNo : []),
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

/** What a yes/no chip asks of a server, in its positive sense. */
function boolOf(key: string): (s: ServerSummary) => boolean {
  switch (key) {
    case NOT_FULL_KEY:
      return (s) => s.clients < s.max_clients;
    case NO_PASSWORD_KEY:
      return (s) => !s.password;
    case NOT_EMPTY_KEY:
      return (s) => s.clients > 0;
    case JOINABLE_KEY:
      return isJoinable;
    default:
      return () => true;
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
    if (st?.type === "bool" && def.type === "bool") {
      const get = boolOf(def.key);
      const want = st.sign > 0;
      tests.push((s) => get(s) === want);
      continue;
    }
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

/** The shape written today. Version 2 is the one where the four checkboxes
 * became chips; a value without it was written before. */
export const TOKENS_VERSION = 2;

export function serializeTokens(tokens: FilterMap, pinned: string[]): string {
  return JSON.stringify({ v: TOKENS_VERSION, tokens, pinned });
}

/**
 * Reads the chips and pins stored under `StorageKey.onlineFilters`. Nothing
 * saved, or damaged, gives the defaults — never an empty list.
 *
 * **A value written before the chips is replayed, never dropped.** It held
 * four booleans side by side (`notFull: true…`), each a checkbox ticked or
 * not: a ticked one comes back as its chip, posed positive; an unticked one,
 * which filtered nothing, as no chip. And the four new chips join its pins,
 * since they were all on screen as checkboxes.
 */
export function parseTokens(raw: string | null): StoredTokens {
  const defs = onlineTokenDefs();
  const fallback = { tokens: { ...DEFAULT_TOKENS }, pinned: [...DEFAULT_PINNED] };
  if (!raw) return fallback;
  let stored: Record<string, unknown>;
  try {
    stored = JSON.parse(raw) as Record<string, unknown>;
  } catch {
    return fallback;
  }
  if (!stored || typeof stored !== "object") return fallback;
  const tokens = sanitizeFilterMap(stored.tokens, defs);
  const known = (k: unknown): k is string => typeof k === "string" && defs.some((d) => d.key === k);
  let pinned = Array.isArray(stored.pinned) ? stored.pinned.filter(known) : [...DEFAULT_PINNED];
  if (stored.v !== TOKENS_VERSION) {
    for (const key of BOOL_KEYS) {
      if (stored[key] === true && !tokens[key]) tokens[key] = { type: "bool", sign: 1 };
    }
    pinned = [...BOOL_KEYS.filter((k) => !pinned.includes(k)), ...pinned];
  }
  return { tokens, pinned };
}
