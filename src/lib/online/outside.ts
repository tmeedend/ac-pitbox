// What Pit Box cannot do for a join, said before the game loads for nothing
// (SPEC-play-online.md, "Contenu manquant"): content with no source it
// knows, to be looked for by hand, and servers that only let in the players
// of their own launcher. Pure, for Vitest.
import type { Fetch, Level } from "./online";

/** A general web search for a mod, by the id the server names it with —
 * DuckDuckGo, no site in particular (the user's choice, 2026-10-04). The
 * archive found is dropped on Pit Box, imported, and recognised like any
 * other. */
export function webSearchUrl(id: string): string {
  return `https://duckduckgo.com/?q=${encodeURIComponent(`${id} assetto corsa`)}`;
}

export interface Wanted {
  id: string;
  name: string;
  level: Level;
  fetch: Fetch;
}

/** The content to download that no source covers — no archive kept, no link
 * from the server, nothing in the registry: what only a person can find.
 * Blocked content (a DLC to buy) is not among it: a search would not help. */
export function unsourced(items: Wanted[]): Wanted[] {
  return items.filter(
    (w) => w.level === "download" && !w.fetch.kept_archive && !w.fetch.server_url && !w.fetch.cup,
  );
}

/** A community whose servers let in only the players of its own launcher. */
export interface LauncherGate {
  name: string;
  /** Where the launcher is to be had. */
  url: string;
}

/**
 * Measured 2026-10-04: an official No Hesi server answers `ACP_AUTH_FAILED`
 * ("handshake failed" on the loading screen) to a player who did not come
 * through their launcher — through Content Manager's own join just the same.
 * Their name says nothing reliable ("No Hesi" is also a style of play, on
 * servers anyone can join), nor do their cars (`nohesi_*` cars run on
 * hundreds of other servers); the link to their site in the description
 * does: on the whole lobby, exactly the 31 official servers carry it.
 */
const GATES: { host: RegExp; gate: LauncherGate }[] = [
  { host: /(^|[/.])nohesi\.gg(\/|$)/i, gate: { name: "No Hesi", url: "https://nohesi.gg/get-started" } },
];

/** The launcher a server asks for, from the links of its name and
 * description; `null` for a server anyone can join. */
export function launcherGate(links: string[]): LauncherGate | null {
  for (const { host, gate } of GATES) {
    if (links.some((l) => host.test(l.replace(/^https?:\/\//i, "").split(/[?#]/)[0]))) return gate;
  }
  return null;
}
