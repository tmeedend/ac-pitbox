// The user's own servers (SPEC-play-online.md, case 1: "rejoindre un serveur
// connu"): favourites and recent joins, and what each tab of the page lists.
// Pure, for Vitest; the persisted state lives in `store.svelte.ts`.
//
// Each entry keeps a snapshot of the server as it was last seen: a favourite
// must still show when the lobby is down, or no longer lists it. The fresh
// lobby entry replaces the snapshot whenever there is one.
import { sortServers } from "./filters";
import { serverKey, type ServerSummary } from "./online";

export type OnlineTab = "all" | "favourites" | "recent";

export interface RecentJoin {
  server: ServerSummary;
  /** The car joined with, `""` on a booking server (picked in CM). */
  car: string;
  /** ISO date of the join. */
  at: string;
}

/** The schema of `online.json` (the Rust side keeps it opaque). */
export interface OnlineStore {
  favourites: ServerSummary[];
  recents: RecentJoin[];
}

/** Joins kept: enough for "yesterday's server", short enough to scan. */
export const RECENTS_KEPT = 20;

export const EMPTY_STORE: OnlineStore = { favourites: [], recents: [] };

function isServer(v: unknown): v is ServerSummary {
  const s = v as ServerSummary | null;
  return !!s && typeof s.ip === "string" && typeof s.http_port === "number" && typeof s.track?.id === "string";
}

/** Reads `online.json`, dropping what it cannot use: an older or damaged
 * entry must not take the whole list down with it. */
export function parseStore(raw: unknown): OnlineStore {
  const value = (raw ?? {}) as Partial<Record<keyof OnlineStore, unknown>>;
  const favourites = Array.isArray(value.favourites) ? value.favourites.filter(isServer) : [];
  const recents = Array.isArray(value.recents)
    ? (value.recents as Partial<RecentJoin>[]).filter(
        (r): r is RecentJoin => isServer(r?.server) && typeof r.car === "string" && typeof r.at === "string",
      )
    : [];
  return { favourites, recents };
}

export function isFavourite(store: OnlineStore, key: string): boolean {
  return store.favourites.some((s) => serverKey(s) === key);
}

export function toggleFavourite(store: OnlineStore, server: ServerSummary): OnlineStore {
  const key = serverKey(server);
  return isFavourite(store, key)
    ? { ...store, favourites: store.favourites.filter((s) => serverKey(s) !== key) }
    : { ...store, favourites: [...store.favourites, server] };
}

/** One line per server, the newest join first: joining the same server again
 * moves it up with the car of that last join. */
export function recordJoin(store: OnlineStore, server: ServerSummary, car: string, at: string): OnlineStore {
  const key = serverKey(server);
  const others = store.recents.filter((r) => serverKey(r.server) !== key);
  return { ...store, recents: [{ server, car, at }, ...others].slice(0, RECENTS_KEPT) };
}

/** The lobby's entry when it has one, else the snapshot. */
function fresh(snapshot: ServerSummary, lobby: Map<string, ServerSummary>): ServerSummary {
  return lobby.get(serverKey(snapshot)) ?? snapshot;
}

/** The servers of a tab. Favourites follow the list's order (joinable, then
 * busiest); recents stay in the order they were joined. */
export function tabServers(tab: OnlineTab, servers: ServerSummary[], store: OnlineStore): ServerSummary[] {
  if (tab === "all") return servers;
  const lobby = new Map(servers.map((s) => [serverKey(s), s]));
  if (tab === "favourites") return sortServers(store.favourites.map((s) => fresh(s, lobby)));
  return store.recents.map((r) => fresh(r.server, lobby));
}

/** The car last joined with, per server key — what the Recent tab shows and
 * what the panel picks again. */
export function recentCars(store: OnlineStore): Record<string, string> {
  return Object.fromEntries(store.recents.filter((r) => r.car).map((r) => [serverKey(r.server), r.car]));
}
