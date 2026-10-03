// The rows of the server table, grouped by track or not (SPEC-play-online.md,
// "Au-delà de Content Manager": "30 serveurs Shutoko = une ligne
// dépliable"). The public list is dominated by a handful of layouts —
// Shutoko, LA Canyons, the drift tracks — and a group turns thirty
// near-identical lines into one, unfolded on demand. Pure, for Vitest.
//
// The group row is a row of the table like any other, one cell per visible
// column, so the order and the hiding of columns hold for it too. How it
// sorts is the columns' business (`columns.ts`, aggregates).
import { ONLINE_COLUMNS, sortServersBy, stableSort, type GroupFacts, type Sort, type SortContext } from "./columns";
import type { ServerSummary, TrackRef } from "./online";
import { trackValue } from "./tokens";

export type ListRow =
  | {
      kind: "server";
      server: ServerSummary;
      /** Shown under its unfolded group. */
      nested: boolean;
    }
  | ({
      kind: "group";
      /** The layout, as a track token names it (`trackValue`). */
      key: string;
      track: TrackRef;
      open: boolean;
    } & GroupFacts);

export interface Arrangement {
  grouped: boolean;
  sort: Sort | null;
  /** Keys of the unfolded groups. */
  open: ReadonlySet<string>;
}

/**
 * The rows of the table. `servers` come in the default order (friends,
 * joinable, players), which decides whatever the sort leaves equal.
 *
 * Grouped, the servers sharing a layout gather under one row. Groups come by
 * total players, most first — the number of servers does not say what is
 * driven: Shutoko runs hundreds of empty ones. A sort on a column with an
 * aggregate orders the groups by it; any sort orders the servers inside each
 * group. A layout run by a single server keeps its plain row, ordered as a
 * group of one: a fold for one line would be a click for nothing.
 */
export function arrangeRows(servers: ServerSummary[], how: Arrangement, ctx: SortContext): ListRow[] {
  const sorted = sortServersBy(servers, how.sort, ctx);
  if (!how.grouped) return sorted.map((server) => ({ kind: "server", server, nested: false }));

  const byKey = new Map<string, ServerSummary[]>();
  for (const s of sorted) {
    const key = trackValue(s.track);
    const members = byKey.get(key);
    if (members) members.push(s);
    else byKey.set(key, [s]);
  }
  const groups = [...byKey.entries()].map(([key, members]) => ({
    key,
    track: members[0].track,
    servers: members,
    clients: members.reduce((n, m) => n + m.clients, 0),
    maxClients: members.reduce((n, m) => n + m.max_clients, 0),
  }));
  const col = how.sort ? ONLINE_COLUMNS.find((c) => c.key === how.sort?.key) : undefined;
  const aggregate = col?.aggregate;
  const byPlayers = stableSort(groups, (g) => g.clients, -1);
  const ordered = aggregate && how.sort ? stableSort(byPlayers, (g) => aggregate(g, ctx), how.sort.dir) : byPlayers;

  const rows: ListRow[] = [];
  for (const g of ordered) {
    if (g.servers.length === 1) {
      rows.push({ kind: "server", server: g.servers[0], nested: false });
      continue;
    }
    const open = how.open.has(g.key);
    rows.push({ kind: "group", ...g, open });
    if (open) for (const server of g.servers) rows.push({ kind: "server", server, nested: true });
  }
  return rows;
}
