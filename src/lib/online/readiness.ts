// How ready a server is to be joined from here (SPEC-play-online.md,
// "Contenu manquant"). The backend judges the track, every car and the CSP
// (`online/readiness.rs`); this module combines that with the car picked in
// the panel. Pure, for Vitest.
import type { Blocker, CarSlots, LayerConflict, Level, ServerSummary } from "./online";

const RANK: Record<Level, number> = { ready: 0, oneClick: 1, download: 2, blocked: 3 };

/** The worse of two levels. */
export function worse(a: Level, b: Level): Level {
  return RANK[a] >= RANK[b] ? a : b;
}

/** Ready or one click away — what the "Joinable" toggle keeps. */
export function isJoinableLevel(level: Level | undefined): boolean {
  return level === "ready" || level === "oneClick";
}

/** The layers a join sets aside: those certain to fail it, unless the user
 * keeps them; those that might, if the user asks to join without them. */
export function layersToSetAside(
  conflicts: LayerConflict[],
  choice: { keepCertain: boolean; dropPossible: boolean },
): string[] {
  return conflicts
    .filter((c) => (c.risk === "certain" ? !choice.keepCertain : choice.dropPossible))
    .map((c) => c.layer_id);
}

export interface JoinState {
  level: Level;
  /** What blocks, when the level is "blocked". */
  blockers: Blocker[];
  /** What would be laid in the game first, when the level is "oneClick". */
  toActivate: ("car" | "track")[];
}

/** The level of joining `server` with `car`: the worse of its track, that car
 * and its CSP requirement. Without a car yet, the server's own level (its
 * best car) stands in. */
export function joinState(server: ServerSummary, car: CarSlots | null): JoinState {
  const csp = (server.blockers ?? []).filter((b) => b.kind === "csp");
  const trackDlc = (server.blockers ?? []).filter((b) => b.kind === "dlc" && server.track_level === "blocked");
  if (!car) {
    return { level: server.level ?? "download", blockers: server.blockers ?? [], toActivate: [] };
  }
  const trackLevel = server.track_level ?? (server.track_available ? "ready" : "download");
  const level = csp.length ? "blocked" : worse(trackLevel, car.level);
  const blockers: Blocker[] = [...csp, ...trackDlc];
  if (car.level === "blocked" && car.dlc) blockers.push({ kind: "dlc", name: car.dlc });
  const toActivate: ("car" | "track")[] = [];
  if (car.level === "oneClick") toActivate.push("car");
  if (trackLevel === "oneClick") toActivate.push("track");
  return { level, blockers, toActivate };
}
