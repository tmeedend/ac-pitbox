// Filtering and ordering of the server list (SPEC-play-online.md, "Filtres de
// base" and "Liste des serveurs"). Pure, for Vitest: the list holds 9 000
// servers, and the order is what makes it usable at all.
import { serverKey, type ServerSummary } from "./online";

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

/** `text` gives what the search reads for a server (`looks.searchText`, which
 * adds the names the library knows); by default its name and ids. */
export function filterServers(
  servers: ServerSummary[],
  f: OnlineFilters,
  text: (s: ServerSummary) => string = (s) => `${s.name} ${s.track.kunos_id} ${s.cars.join(" ")}`.toLowerCase(),
): ServerSummary[] {
  // One term per word, all required — the same reading as the library search.
  const terms = f.search.toLowerCase().split(/\s+/).filter(Boolean);
  return servers.filter(
    (s) =>
      (!f.notFull || s.clients < s.max_clients) &&
      (!f.noPassword || !s.password) &&
      (!f.notEmpty || s.clients > 0) &&
      (!f.joinable || isJoinable(s)) &&
      (terms.length === 0 || terms.every((term) => text(s).includes(term))),
  );
}

/** Servers with a friend on them first (keys in `withFriends`), then
 * joinable, then by players: a full-looking list of servers one cannot enter
 * is the first thing the spec sets out to avoid. Stable for equal keys, so a
 * refresh does not shuffle rows under the cursor. */
export function sortServers(servers: ServerSummary[], withFriends: Set<string> = new Set()): ServerSummary[] {
  const friendly = (s: ServerSummary) => Number(withFriends.has(serverKey(s)));
  return servers
    .map((s, i) => ({ s, i }))
    .sort(
      (a, b) =>
        friendly(b.s) - friendly(a.s) ||
        Number(isJoinable(b.s)) - Number(isJoinable(a.s)) ||
        b.s.clients - a.s.clients ||
        a.i - b.i,
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
