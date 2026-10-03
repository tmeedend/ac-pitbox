// The Online page's judged list, kept three minutes so that coming back to
// the page does not judge it again. The download itself is cached below, on
// the Rust side (`online/lobby_cache.rs`), and shared with the track sheets:
// judging again after a library change costs no download.
import { libraryVersion } from "$lib/library/libraryVersion.svelte";
import { listServers, type ServerList } from "./online";

/** How long a list stays good. A server's players move by the minute; the
 * lobby itself refreshes about as often. */
const FRESH_MS = 3 * 60 * 1000;

// `$state.raw`: 9 000 servers deep-proxied would cost far more than they
// change — the list is only ever replaced whole.
// `version`: the library the list was judged against (readiness, names). A
// change to it — an import, an activation, content fetched for a join —
// makes the list stale however young it is.
let cache = $state.raw<{ list: ServerList | null; at: number; version: number }>({ list: null, at: 0, version: -1 });
let inflight: Promise<ServerList> | null = null;

/** The last list, however old, `null` before the first. Reactive. */
export function cachedLobby(): ServerList | null {
  return cache.list;
}

/** The list, from the cache while it is fresh — `force` asks the lobby
 * anyway (the page's Refresh). Two callers at once share one request. */
export function loadLobby(force = false): Promise<ServerList> {
  const version = libraryVersion();
  const fresh = cache.list && Date.now() - cache.at < FRESH_MS && cache.version === version;
  if (!force && fresh && cache.list) return Promise.resolve(cache.list);
  inflight ??= listServers(force)
    .then((list) => {
      cache = { list, at: Date.now(), version };
      return list;
    })
    .finally(() => {
      inflight = null;
    });
  return inflight;
}
