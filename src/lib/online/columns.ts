// The columns of the Online page's table (SPEC-play-online.md, "Liste des
// serveurs") and how they sort. Pure, for Vitest; the cells are drawn by
// `ServerTable.svelte`, through the shared `DataTable`.
//
// A column may declare an **aggregate**: how a group of servers sharing a
// layout sorts under it (`groups.ts`). Sorting by such a column orders the
// groups by their aggregate and the servers inside each by their own value;
// sorting by a column without one orders only the servers, the groups keeping
// their order.
import type { TableColumn } from "$lib/tableColumns";
import type { Looks } from "./looks";
import { trackTitle } from "./looks";
import type { Level, ServerSummary } from "./online";

/** A sort key; `null` (not measured, not declared) always goes last. */
export type SortValue = string | number | null;

export interface SortContext {
  looks: Looks;
  pingOf: (s: ServerSummary) => number | undefined;
}

/** What a group offers its column's aggregate. */
export interface GroupFacts {
  servers: ServerSummary[];
  clients: number;
  maxClients: number;
}

export interface OnlineColumn extends TableColumn {
  sortValue?: (s: ServerSummary, ctx: SortContext) => SortValue;
  /** The group's sort key under this column, when it has one. */
  aggregate?: (g: GroupFacts, ctx: SortContext) => SortValue;
}

const LEVEL_RANK: Record<Level, number> = { ready: 0, oneClick: 1, download: 2, blocked: 3 };
const SESSION_RANK = { race: 0, qualify: 1, practice: 2, booking: 3 } as const;

const trackSort = (s: ServerSummary, ctx: SortContext) => trackTitle(ctx.looks, s.track).toLowerCase();

export const ONLINE_COLUMNS: OnlineColumn[] = [
  {
    key: "track",
    labelKey: "online.colTrack",
    sortable: true,
    defaultVisible: true,
    fixed: true,
    locked: true,
    width: 260,
    sortValue: trackSort,
    aggregate: (g, ctx) => trackSort(g.servers[0], ctx),
  },
  {
    key: "server",
    labelKey: "online.colServer",
    sortable: true,
    defaultVisible: true,
    width: 300,
    sortValue: (s) => s.name.toLowerCase(),
    aggregate: (g) => g.servers.length,
  },
  {
    key: "session",
    labelKey: "online.colSession",
    sortable: true,
    defaultVisible: true,
    width: 130,
    sortValue: (s) => (s.session ? SESSION_RANK[s.session] * 1e7 + s.time_left : null),
  },
  {
    key: "players",
    labelKey: "online.colPlayers",
    sortable: true,
    defaultVisible: true,
    mono: true,
    width: 80,
    sortValue: (s) => s.clients,
    aggregate: (g) => g.clients,
  },
  {
    key: "ping",
    labelKey: "online.colPing",
    sortable: true,
    defaultVisible: true,
    mono: true,
    width: 70,
    sortValue: (s, ctx) => ctx.pingOf(s) ?? null,
  },
  {
    key: "cars",
    labelKey: "online.colCars",
    sortable: true,
    defaultVisible: true,
    width: 240,
    sortValue: (s) => s.cars.length,
  },
  {
    key: "csp",
    labelKey: "online.colCsp",
    sortable: true,
    defaultVisible: true,
    mono: true,
    width: 70,
    sortValue: (s) => s.track.csp_min_build ?? null,
  },
  {
    key: "country",
    labelKey: "online.colCountry",
    sortable: true,
    defaultVisible: true,
    width: 60,
    sortValue: (s) => s.country ?? null,
  },
  {
    key: "state",
    labelKey: "online.colState",
    sortable: true,
    defaultVisible: true,
    width: 100,
    sortValue: (s) => (s.level ? LEVEL_RANK[s.level] : null),
  },
  {
    key: "address",
    labelKey: "online.colAddress",
    sortable: true,
    defaultVisible: false,
    mono: true,
    width: 160,
    sortValue: (s) => `${s.ip}:${s.http_port}`,
  },
  {
    key: "password",
    labelKey: "online.colPassword",
    sortable: true,
    defaultVisible: false,
    width: 60,
    sortValue: (s) => (s.password ? 1 : 0),
  },
];

export interface Sort {
  key: string;
  dir: 1 | -1;
}

/** A header click: ascending, then descending, then back to the default
 * order (friends, joinable, players). Another column starts ascending. */
export function nextSort(current: Sort | null, key: string): Sort | null {
  if (current?.key !== key) return { key, dir: 1 };
  return current.dir === 1 ? { key, dir: -1 } : null;
}

/** Compares two keys in `dir`, `null` last whatever the direction. */
export function compareKeys(a: SortValue, b: SortValue, dir: 1 | -1): number {
  if (a === null || b === null) return a === b ? 0 : a === null ? 1 : -1;
  if (a < b) return -dir;
  if (a > b) return dir;
  return 0;
}

/** `items` ordered by `key`, stable: equal keys keep the order they came in —
 * the default order, which goes on deciding between them. */
export function stableSort<T>(items: T[], key: (item: T) => SortValue, dir: 1 | -1): T[] {
  return items
    .map((item, i) => ({ item, i, k: key(item) }))
    .sort((a, b) => compareKeys(a.k, b.k, dir) || a.i - b.i)
    .map(({ item }) => item);
}

/** The servers in the column's order, or as they came without a sort. */
export function sortServersBy(servers: ServerSummary[], sort: Sort | null, ctx: SortContext): ServerSummary[] {
  const col = sort ? ONLINE_COLUMNS.find((c) => c.key === sort.key) : undefined;
  if (!sort || !col?.sortValue) return servers;
  const value = col.sortValue;
  return stableSort(servers, (s) => value(s, ctx), sort.dir);
}

/** Reads a stored sort, `null` for none or anything unrecognised. */
export function parseSort(raw: string | null): Sort | null {
  if (!raw) return null;
  try {
    const v = JSON.parse(raw) as Partial<Sort>;
    const known = ONLINE_COLUMNS.some((c) => c.key === v.key && c.sortable);
    return known && (v.dir === 1 || v.dir === -1) ? { key: v.key as string, dir: v.dir } : null;
  } catch {
    return null;
  }
}
