// Filtering and ordering of the server list (SPEC-play-online.md, "Filtres de
// base" and "Liste des serveurs"). Pure, for Vitest: the list holds 9 000
// servers, and the order is what makes it usable at all.
import type { ServerSummary } from "./online";

export interface OnlineFilters {
  search: string;
  notFull: boolean;
  noPassword: boolean;
  notEmpty: boolean;
  /** Track and at least one car drivable here. */
  joinable: boolean;
}

/** The spec's defaults: full and password-protected servers hidden, empty
 * ones kept — an empty server that is ready is still a place to drive alone. */
export const DEFAULT_FILTERS: OnlineFilters = {
  search: "",
  notFull: true,
  noPassword: true,
  notEmpty: false,
  joinable: false,
};

/** Lot 1's reading of "ready": the track and at least one car can be driven.
 * The spec's four levels (ready, one click, download, blocked) come later. */
export function isJoinable(s: ServerSummary): boolean {
  return s.track_available && s.cars_available > 0;
}

function matches(s: ServerSummary, terms: string[]): boolean {
  if (terms.length === 0) return true;
  const hay = `${s.name} ${s.track.kunos_id} ${s.cars.join(" ")}`.toLowerCase();
  return terms.every((term) => hay.includes(term));
}

export function filterServers(servers: ServerSummary[], f: OnlineFilters): ServerSummary[] {
  // One term per word, all required — the same reading as the library search.
  const terms = f.search.toLowerCase().split(/\s+/).filter(Boolean);
  return servers.filter(
    (s) =>
      (!f.notFull || s.clients < s.max_clients) &&
      (!f.noPassword || !s.password) &&
      (!f.notEmpty || s.clients > 0) &&
      (!f.joinable || isJoinable(s)) &&
      matches(s, terms),
  );
}

/** Joinable first, then by players: a full-looking list of servers one
 * cannot enter is the first thing the spec sets out to avoid. Stable for
 * equal keys, so a refresh does not shuffle rows under the cursor. */
export function sortServers(servers: ServerSummary[]): ServerSummary[] {
  return servers
    .map((s, i) => ({ s, i }))
    .sort(
      (a, b) =>
        Number(isJoinable(b.s)) - Number(isJoinable(a.s)) || b.s.clients - a.s.clients || a.i - b.i,
    )
    .map(({ s }) => s);
}

/** Reads the stored filters, keeping the defaults for anything missing or
 * malformed — an older or damaged value must never empty the list. */
export function parseFilters(raw: string | null): OnlineFilters {
  if (!raw) return { ...DEFAULT_FILTERS };
  try {
    const stored = JSON.parse(raw) as Partial<OnlineFilters>;
    const pick = <K extends keyof OnlineFilters>(key: K): OnlineFilters[K] =>
      typeof stored[key] === typeof DEFAULT_FILTERS[key] ? (stored[key] as OnlineFilters[K]) : DEFAULT_FILTERS[key];
    return {
      search: pick("search"),
      notFull: pick("notFull"),
      noPassword: pick("noPassword"),
      notEmpty: pick("notEmpty"),
      joinable: pick("joinable"),
    };
  } catch {
    return { ...DEFAULT_FILTERS };
  }
}
