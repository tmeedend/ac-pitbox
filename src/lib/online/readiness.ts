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
  /** The cars still to download — the reason named when the level is
   * "download" and the track is here. */
  missingCars: string[];
  /** What would be laid in the game first, when the level is "oneClick". */
  toActivate: ("car" | "track")[];
}

/**
 * The level of joining `server` with `car`: the worst of its track, its CSP
 * requirement and **every car it runs** (`all`, the panel's list — AI traffic
 * included). The game loads the whole entry list: one car missing keeps the
 * player out, whichever is driven (the user's experience, 2026-10-04). Before
 * the panel has read the server, its list level (which judges every car too)
 * stands in.
 */
export function joinState(server: ServerSummary, car: CarSlots | null, all: CarSlots[] = []): JoinState {
  const csp = (server.blockers ?? []).filter((b) => b.kind === "csp");
  const cars = car ? [car, ...all.filter((c) => c.id !== car.id)] : all;
  if (cars.length === 0) {
    return { level: server.level ?? "download", blockers: server.blockers ?? [], missingCars: [], toActivate: [] };
  }
  const trackLevel = server.track_level ?? (server.track_available ? "ready" : "download");
  const carLevel = cars.reduce<Level>((acc, c) => worse(acc, c.level), "ready");
  const level = csp.length ? "blocked" : worse(trackLevel, carLevel);
  const blockers: Blocker[] = [...csp];
  const name = (dlc: string) => {
    if (!blockers.some((b) => b.kind === "dlc" && b.name === dlc)) blockers.push({ kind: "dlc", name: dlc });
  };
  if (trackLevel === "blocked") {
    for (const b of server.blockers ?? []) if (b.kind === "dlc") name(b.name);
  }
  for (const c of cars) if (c.level === "blocked" && c.dlc) name(c.dlc);
  const toActivate: ("car" | "track")[] = [];
  if (cars.some((c) => c.level === "oneClick")) toActivate.push("car");
  if (trackLevel === "oneClick") toActivate.push("track");
  return { level, blockers, missingCars: cars.filter((c) => c.level === "download").map((c) => c.id), toActivate };
}
