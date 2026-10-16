// A lobby server for the Online tests - never imported by the app. One
// factory, with the fields every test file needs; each file passes only what
// its own cases turn on. There were four copies of it, drifting apart.
import type { ServerSummary } from "./online";

export function testServer(over: Partial<ServerSummary> = {}): ServerSummary {
  return {
    ip: "1.2.3.4",
    port: 9600,
    http_port: 8081,
    name: "Server",
    country: "DE",
    clients: 0,
    max_clients: 24,
    password: false,
    booking: false,
    track: { kunos_id: "monza", id: "monza", layout: null, csp_min_build: null },
    cars: ["ks_mazda_miata"],
    session: "practice",
    sessions: ["practice"],
    time_left: 600,
    track_available: true,
    cars_available: 1,
    official: true,
    ...over,
  };
}
