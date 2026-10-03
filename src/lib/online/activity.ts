// What is being driven online on a track, as its sheet says it
// (SPEC-play-online.md, v2: "Online : 4 serveurs maintenant"). Counted here,
// on the lobby's list, without a single request to the servers themselves.
// Pure, for Vitest.
import type { ServerSummary } from "./online";
import { trackValue } from "./tokens";

export interface TrackActivity {
  /** Servers with at least one player on the track, any layout. */
  servers: number;
  players: number;
  /** Their layouts, as a track token names them: what the Online page is
   * opened on. */
  layouts: string[];
}

/** The occupied servers running `trackId` (a folder), any of its layouts. An
 * empty server is not activity: the sheet would announce hundreds of empty
 * Shutoko servers. */
export function trackActivity(servers: ServerSummary[], trackId: string): TrackActivity {
  const id = trackId.toLowerCase();
  const busy = servers.filter((s) => s.clients > 0 && s.track.id.toLowerCase() === id);
  return {
    servers: busy.length,
    players: busy.reduce((n, s) => n + s.clients, 0),
    layouts: [...new Set(busy.map((s) => trackValue(s.track)))],
  };
}
