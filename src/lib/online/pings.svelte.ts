// Pings of the servers (SPEC-play-online.md: "Mesuré sur les lignes visibles,
// mis en cache quelques minutes"). The list asks for its visible rows; a ping
// token asks for every server it filters (`measureAll`). Servers measured
// recently, or being measured, are not asked again.
import { pingServers, serverKey, type Ping, type ServerSummary } from "./online";

/** A ping older than this is measured again when its row shows. */
const FRESH_MS = 5 * 60 * 1000;

/** Servers per request of a sweep: two rounds of the backend's 32 parallel
 * connections, so the list fills in every few seconds rather than once at
 * the end of thousands. */
const SWEEP_BATCH = 64;

const pings = $state<Record<string, number>>({});
/** When each server was last asked — answered or not, so a server that does
 * not answer is not asked on every scroll. */
const askedAt = new Map<string, number>();

/** Servers a sweep still has to ask. */
const sweep = $state({ left: 0 });
/** Bumped to stop the running sweep: a newer one, or none wanted. */
let generation = 0;

/** The last measured ping of a server, in ms. */
export function pingOf(key: string): number | undefined {
  return pings[key];
}

/** How many servers the running sweep has yet to ask. */
export function pingsLeft(): number {
  return sweep.left;
}

/** The servers of `list` not asked in the last minutes. */
function due(list: ServerSummary[]): ServerSummary[] {
  const now = Date.now();
  return list.filter((s) => now - (askedAt.get(serverKey(s)) ?? 0) > FRESH_MS);
}

/** The same, marked as asked. */
function takeDue(list: ServerSummary[]): ServerSummary[] {
  const taken = due(list);
  const now = Date.now();
  for (const s of taken) askedAt.set(serverKey(s), now);
  return taken;
}

function record(answers: Ping[]) {
  for (const p of answers) pings[serverKey(p)] = p.ms;
}

/** Measures the servers among `visible` not asked in the last minutes. */
export function requestPings(visible: ServerSummary[]): void {
  const taken = takeDue(visible);
  if (taken.length === 0) return;
  pingServers(taken)
    .then(record)
    .catch((e) => console.error("online_ping", e));
}

/** Measures every server of `servers`, a batch at a time and in their order —
 * the top of the list first. Replaces any sweep under way. */
export async function measureAll(servers: ServerSummary[]): Promise<void> {
  const mine = ++generation;
  let rest = due(servers);
  sweep.left = rest.length;
  while (rest.length > 0 && mine === generation) {
    // Taken batch by batch, not up front: the visible rows may have been
    // measured meanwhile.
    const batch = takeDue(rest.slice(0, SWEEP_BATCH));
    rest = rest.slice(SWEEP_BATCH);
    if (batch.length > 0) {
      try {
        record(await pingServers(batch));
      } catch (e) {
        console.error("online_ping", e);
      }
    }
    if (mine === generation) sweep.left = rest.length;
  }
}

/** Stops the sweep under way, if any. */
export function stopMeasuring(): void {
  generation++;
  sweep.left = 0;
}
