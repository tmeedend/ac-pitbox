// Typed bridge to the multiplayer commands (`src-tauri/src/online/`,
// docs/SPEC-play-online.md). Mirrors `ServerSummary`, `ServerDetail` and
// `JoinRequest` on the Rust side.
import { invoke } from "@tauri-apps/api/core";
import type { Looks } from "./looks";

export type SessionKind = "booking" | "practice" | "qualify" | "race";

/** How ready something is to be joined, best first (`online/readiness.rs`). */
export type Level = "ready" | "oneClick" | "download" | "blocked";

/** Why a server is blocked. */
export type Blocker = { kind: "dlc"; name: string } | { kind: "csp"; required: number; installed: number | null };

export interface TrackRef {
  /** `folder-layout`, what Content Manager resolves itself. */
  kunos_id: string;
  id: string;
  layout: string | null;
  csp_min_build: number | null;
}

export interface ServerSummary {
  ip: string;
  port: number;
  http_port: number;
  name: string;
  country: string | null;
  clients: number;
  max_clients: number;
  password: boolean;
  booking: boolean;
  track: TrackRef;
  cars: string[];
  session: SessionKind | null;
  sessions: SessionKind[];
  time_left: number;
  track_available: boolean;
  cars_available: number;
  /** Absent from a favourite or recent saved before readiness existed: such
   * a snapshot is unknown until the lobby lists the server again. */
  level?: Level;
  track_level?: Level;
  blockers?: Blocker[];
  /** The track and every car are Kunos content (`content: kunos`). Absent
   * from a snapshot saved before servers were judged on it. */
  official?: boolean;
  /** One per session: seconds, except a race on laps (`timed` false).
   * Absent from a snapshot saved before they were read. */
  durations?: number[];
  timed?: boolean;
  extra_lap?: boolean;
  inverted_grid?: boolean;
  mandatory_pit?: boolean;
}

/** A rule departing from an AC server's defaults (`online/extended.rs`). */
export type Rule =
  | { kind: "absDenied" | "absForced" | "tcDenied" | "tcForced" }
  | { kind: "stabilityAllowed" | "autoclutchDenied" | "tyreBlanketsAllowed" | "virtualMirrorForced" }
  | { kind: "damage" | "fuel" | "tyreWear"; percent: number }
  | { kind: "tyresOut"; count: number };

export interface Extended {
  conditions: {
    weather: string | null;
    ambient: number | null;
    road: number | null;
    wind_speed: number | null;
    wind_direction: number | null;
    grip: number | null;
  };
  rules: Rule[];
  description: string | null;
}

export interface CarSlots {
  id: string;
  total: number;
  free: number;
  /** The skin the server will impose: its first free slot's. */
  skin: string | null;
  available: boolean;
  level: Level;
  /** The DLC to name when the car is blocked. */
  dlc: string | null;
  /** The car's active layers and what each risks online. */
  layers: LayerConflict[];
  /** How to get the car, and its versions. */
  fetch: Fetch;
  /** Photo of `skin`, when the car is in the game with that livery. */
  preview: string | null;
}

export interface Driver {
  name: string;
  car: string;
}

export interface ServerDetail {
  summary: ServerSummary;
  cars: CarSlots[];
  drivers: Driver[];
  features: string[];
  /** On servers that publish `/api/details` only. */
  extended: Extended | null;
  /** Web links from the name and the description. */
  links: string[];
  /** The track's active layers and what each risks online. */
  track_layers: LayerConflict[];
  /** How to get the track, and its versions. */
  track_fetch: Fetch;
}

/** Where to get a car or a track a server needs (`online/content.rs`), in
 * the spec's order: the archive kept at import, the server's link, the
 * registry. */
export interface Fetch {
  kept_archive: boolean;
  server_url: string | null;
  cup: boolean;
  server_version: string | null;
  installed_version: string | null;
  /** Joining fetches it first: missing here, or outdated, with a source. */
  needed: boolean;
}

/** An active layer on the car or the track of a join (`session_layers.rs`):
 * `certain` replaces what the server checks, `possible` something else. */
export interface LayerConflict {
  layer_id: string;
  name: string;
  risk: "certain" | "possible";
}

export interface Ping {
  ip: string;
  http_port: number;
  ms: number;
}

/** The round trip to each server, measured by a TCP connection
 * (`online/ping.rs`). Servers that do not answer are absent. */
export function pingServers(servers: ServerSummary[]): Promise<Ping[]> {
  return invoke<Ping[]>("online_ping", { servers: servers.map(({ ip, http_port }) => ({ ip, http_port })) });
}

/** A server's identity across the list and the detail panel. */
export function serverKey(s: Pick<ServerSummary, "ip" | "http_port">): string {
  return `${s.ip}:${s.http_port}`;
}

/** The list, and the names and pictures of what it references — sent once
 * rather than on every row (`online::ServerList`). */
export interface ServerList {
  servers: ServerSummary[];
  looks: Looks;
}

/** `force` asks the lobby again past the backend's cache (Refresh). */
export function listServers(force = false): Promise<ServerList> {
  return invoke<ServerList>("online_servers", { force });
}

/** What is driven on a track now (`online/lobby_cache.rs`). */
export interface TrackActivity {
  servers: number;
  players: number;
  /** Track token values (`trackValue`) of the layouts in use. */
  layouts: string[];
}

/** Counted on the backend's raw lobby list: no library, disk nor server. */
export function trackActivity(trackId: string, layouts: string[]): Promise<TrackActivity> {
  return invoke<TrackActivity>("online_track_activity", { trackId, layouts });
}

export interface ServerDrivers {
  ip: string;
  http_port: number;
  drivers: string[];
}

/** The connected drivers of each of `servers`, asked from the servers
 * themselves (`online/drivers.rs`). Servers that do not answer are absent. */
export function serverDrivers(servers: ServerSummary[]): Promise<ServerDrivers[]> {
  return invoke<ServerDrivers[]>("online_server_drivers", {
    servers: servers.map(({ ip, http_port }) => ({ ip, http_port })),
  });
}

/** The player slots of one car on a server (`online/server.rs`). */
export interface SlotCount {
  car: string;
  free: number;
  total: number;
}

/** A server's free slots per car, from its entry list alone — what the
 * "notify me" watch asks again and again. */
export function slotCounts(ip: string, httpPort: number): Promise<SlotCount[]> {
  return invoke<SlotCount[]>("online_slot_counts", { ip, httpPort });
}

export function serverDetail(ip: string, httpPort: number): Promise<ServerDetail> {
  return invoke<ServerDetail>("online_server_detail", { ip, httpPort });
}

/** `setAside`: layers of this car or track to deactivate for the session,
 * given back when the game closes. */
export function joinServer(
  server: ServerSummary,
  carId: string,
  password: string | null,
  setAside: string[] = [],
): Promise<void> {
  return invoke<void>("online_join", {
    request: {
      ip: server.ip,
      port: server.port,
      http_port: server.http_port,
      car_id: carId,
      track_id: server.track.id,
      track_kunos_id: server.track.kunos_id,
      password,
      booking: server.booking,
      set_aside: setAside,
    },
  });
}
