// Pings of the servers on screen (SPEC-play-online.md: "Mesuré sur les lignes
// visibles, mis en cache quelques minutes"). The list asks for its visible
// rows; servers measured recently, or being measured, are not asked again.
import { pingServers, serverKey, type ServerSummary } from "./online";

/** A ping older than this is measured again when its row shows. */
const FRESH_MS = 5 * 60 * 1000;

const pings = $state<Record<string, number>>({});
/** When each server was last asked — answered or not, so a server that does
 * not answer is not asked on every scroll. */
const askedAt = new Map<string, number>();

/** The last measured ping of a server, in ms. */
export function pingOf(key: string): number | undefined {
  return pings[key];
}

/** Measures the servers among `visible` not asked in the last minutes. */
export function requestPings(visible: ServerSummary[]): void {
  const now = Date.now();
  const due = visible.filter((s) => now - (askedAt.get(serverKey(s)) ?? 0) > FRESH_MS);
  if (due.length === 0) return;
  for (const s of due) askedAt.set(serverKey(s), now);
  pingServers(due)
    .then((answers) => {
      for (const p of answers) pings[serverKey(p)] = p.ms;
    })
    .catch((e) => console.error("online_ping", e));
}
