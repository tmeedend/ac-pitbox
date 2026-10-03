// The search and the default order of the server list (SPEC-play-online.md,
// "Filtres de base" and "Liste des serveurs"). Pure, for Vitest: the list
// holds 9 000 servers, and the order is what makes it usable at all. Every
// other filter is a chip (`tokens.ts`).
import { serverKey, type ServerSummary } from "./online";
import { isJoinableLevel } from "./readiness";

/** Ready or one click away (`readiness.ts`). A snapshot saved before levels
 * existed falls back on what it does carry: the track and a car drivable. */
export function isJoinable(s: ServerSummary): boolean {
  return s.level ? isJoinableLevel(s.level) : s.track_available && s.cars_available > 0;
}

/** Ready first, then one click, then the rest — the spec's order of the
 * joinable ("READY, puis 1 CLICK"); beyond them the players decide. */
function joinRank(s: ServerSummary): number {
  if (!isJoinable(s)) return 2;
  return s.level === "oneClick" ? 1 : 0;
}

/** The servers whose text holds every word of `search` — one term per word,
 * all required, the same reading as the library search. `text` gives what is
 * read for a server (`looks.searchText`, which adds the names the library
 * knows); by default its name and ids. */
export function searchServers(
  servers: ServerSummary[],
  search: string,
  text: (s: ServerSummary) => string = (s) => `${s.name} ${s.track.kunos_id} ${s.cars.join(" ")}`.toLowerCase(),
): ServerSummary[] {
  const terms = search.toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return servers;
  return servers.filter((s) => terms.every((term) => text(s).includes(term)));
}

/** Servers with a friend on them first (keys in `withFriends`), then ready,
 * then one click, then by players: a full-looking list of servers one cannot
 * enter is the first thing the spec sets out to avoid. Stable for equal keys,
 * so a refresh does not shuffle rows under the cursor. */
export function sortServers(servers: ServerSummary[], withFriends: Set<string> = new Set()): ServerSummary[] {
  const friendly = (s: ServerSummary) => Number(withFriends.has(serverKey(s)));
  return servers
    .map((s, i) => ({ s, i }))
    .sort(
      (a, b) =>
        friendly(b.s) - friendly(a.s) ||
        joinRank(a.s) - joinRank(b.s) ||
        b.s.clients - a.s.clients ||
        a.i - b.i,
    )
    .map(({ s }) => s);
}
