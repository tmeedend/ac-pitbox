// Servers grouped by track (SPEC-play-online.md, "Au-delà de Content
// Manager": "30 serveurs Shutoko = une ligne dépliable"). The public list is
// dominated by a handful of layouts — Shutoko, LA Canyons, the drift tracks —
// and a group turns thirty near-identical lines into one, unfolded on demand.
// Pure, for Vitest.
import type { Level, ServerSummary, TrackRef } from "./online";
import { better } from "./readiness";
import { trackValue } from "./tokens";

export type ListRow =
  | {
      kind: "server";
      server: ServerSummary;
      /** Shown under its unfolded group. */
      nested: boolean;
    }
  | {
      kind: "group";
      /** The layout, as a track token names it (`trackValue`). */
      key: string;
      track: TrackRef;
      servers: ServerSummary[];
      clients: number;
      maxClients: number;
      /** The best level among its servers: a group is as joinable as its
       * most joinable server. Absent when none of them has one. */
      level?: Level;
      open: boolean;
    };

function best(levels: (Level | undefined)[]): Level | undefined {
  return levels.reduce<Level | undefined>((acc, l) => (l === undefined ? acc : acc ? better(acc, l) : l), undefined);
}

/** One row per server, as the list shows them ungrouped. */
export function serverRows(servers: ServerSummary[]): ListRow[] {
  return servers.map((server) => ({ kind: "server", server, nested: false }));
}

/**
 * The servers of `servers` sharing a layout, gathered under one row placed
 * where the first of them stood — the list's order (friends, joinable,
 * players) keeps deciding what comes first. A layout run by a single server
 * keeps its plain row: a group of one is a fold for nothing. The groups whose
 * key is in `open` are followed by their servers, in the list's order.
 */
export function groupByTrack(servers: ServerSummary[], open: ReadonlySet<string>): ListRow[] {
  const byKey = new Map<string, ServerSummary[]>();
  for (const s of servers) {
    const key = trackValue(s.track);
    const list = byKey.get(key);
    if (list) list.push(s);
    else byKey.set(key, [s]);
  }
  const rows: ListRow[] = [];
  const placed = new Set<string>();
  for (const s of servers) {
    const key = trackValue(s.track);
    const members = byKey.get(key) ?? [s];
    if (members.length === 1) {
      rows.push({ kind: "server", server: s, nested: false });
      continue;
    }
    if (placed.has(key)) continue;
    placed.add(key);
    const isOpen = open.has(key);
    rows.push({
      kind: "group",
      key,
      track: s.track,
      servers: members,
      clients: members.reduce((n, m) => n + m.clients, 0),
      maxClients: members.reduce((n, m) => n + m.max_clients, 0),
      level: best(members.map((m) => m.level)),
      open: isOpen,
    });
    if (isOpen) for (const m of members) rows.push({ kind: "server", server: m, nested: true });
  }
  return rows;
}
