// Typed bridge to the multiplayer commands (`src-tauri/src/online/`,
// docs/SPEC-play-online.md). Mirrors `ServerSummary`, `ServerDetail` and
// `JoinRequest` on the Rust side.
import { invoke } from "@tauri-apps/api/core";
import type { Looks } from "./looks";

export type SessionKind = "booking" | "practice" | "qualify" | "race";

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
}

export interface CarSlots {
  id: string;
  total: number;
  free: number;
  /** The skin the server will impose: its first free slot's. */
  skin: string | null;
  available: boolean;
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

export function listServers(): Promise<ServerList> {
  return invoke<ServerList>("online_servers");
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

export function serverDetail(ip: string, httpPort: number): Promise<ServerDetail> {
  return invoke<ServerDetail>("online_server_detail", { ip, httpPort });
}

export function joinServer(server: ServerSummary, carId: string, password: string | null): Promise<void> {
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
    },
  });
}
