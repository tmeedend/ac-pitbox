// What the background watch decides (SPEC-play-online.md, v2: "Prévenir
// quand un ami se connecte ou qu'un slot se libère"), apart from its timers.
// Pure, for Vitest; the watch itself is `watch.svelte.ts`.
//
// It watches only what the user named — never the 9 000 servers: that would
// be abusive towards them, and some routers block a machine that contacts
// thousands of addresses.
import type { OnlineStore } from "./lists";
import { serverKey, type ServerSummary, type SlotCount } from "./online";

/** The servers a friend is looked for on: favourites and recent joins, each
 * once — a score of them, one request each per round. A friend on a server
 * one never went to is not found: an accepted limit. */
export function watchedServers(store: OnlineStore): ServerSummary[] {
  const seen = new Set<string>();
  const out: ServerSummary[] = [];
  for (const s of [...store.favourites, ...store.recents.map((r) => r.server)]) {
    const key = serverKey(s);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(s);
  }
  return out;
}

export interface Sighting {
  key: string;
  name: string;
}

/** The friends on a server now who were not there at the previous round —
 * per server, so a friend moving from one to another is seen arriving. */
export function newSightings(before: Record<string, string[]>, now: Record<string, string[]>): Sighting[] {
  const out: Sighting[] = [];
  for (const [key, names] of Object.entries(now)) {
    const was = new Set((before[key] ?? []).map((n) => n.trim().toLowerCase()));
    for (const name of names) if (!was.has(name.trim().toLowerCase())) out.push({ key, name });
  }
  return out;
}

/** A slot is free for `car` — or, when no car was chosen, for any car. */
export function slotFreed(counts: SlotCount[], car: string | null): boolean {
  return counts.some((c) => c.free > 0 && (car === null || c.car.toLowerCase() === car.toLowerCase()));
}
