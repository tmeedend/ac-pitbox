//! Multiplayer servers (`docs/SPEC-play-online.md`): the public list, one
//! server's details, and joining through Content Manager.
//!
//! Everything here talks to the network, and WinHTTP blocks: the command
//! facades run it in `spawn_blocking`, and take the SQLite lock only for the
//! short read that judges servers against the library — never across a
//! request.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;

pub mod content;
pub mod drivers;
pub mod extended;
mod fanout;
mod installed;
pub mod join;
pub mod lobby;
pub mod lobby_cache;
mod looks;
pub mod ping;
pub mod readiness;
pub mod server;
pub mod session_layers;
mod steam;
pub mod store;

pub use installed::Installed;
pub use looks::Looks;
pub use steam::active_steam_id;

/// The list as the screen receives it: the servers, and how what they
/// reference looks here — sent once rather than repeated on 7 000 rows.
#[derive(Debug, Serialize)]
pub struct ServerList {
    pub servers: Vec<lobby::ServerSummary>,
    pub looks: Looks,
}

/// Judges freshly fetched servers against what can be driven here, and reads
/// the names and pictures of what they reference.
pub fn judge_list(conn: &Connection, cfg: &AppConfig, mut servers: Vec<lobby::ServerSummary>) -> ServerList {
    let installed = Installed::scan(conn, cfg);
    for server in &mut servers {
        installed.judge(server);
    }
    let (cars, tracks) = referenced(&servers);
    let looks = Looks::scan(conn, cfg, &cars, &tracks);
    ServerList { servers, looks }
}

/// The car and track ids (lowercase) whose look is worth reading: every car
/// named by a server, and only the tracks that can be driven — the others
/// have no folder here to read a picture from.
fn referenced(servers: &[lobby::ServerSummary]) -> (HashSet<String>, HashSet<String>) {
    let cars = servers
        .iter()
        .flat_map(|s| s.cars.iter().map(|c| c.to_lowercase()))
        .collect();
    let tracks = servers
        .iter()
        .filter(|s| s.track_available)
        .map(|s| s.track.id.to_lowercase())
        .collect();
    (cars, tracks)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: only what the servers name is read, and a track that cannot be
    /// driven is not — the lobby names hundreds of tracks nobody has here.
    #[test]
    fn only_referenced_content_is_looked_up() {
        let server = |track: &str, available: bool| {
            let mut s = lobby::parse_server(&serde_json::json!({
                "ip": "1.2.3.4", "cport": 8081, "tport": 9600, "track": track, "cars": ["KS_Mazda_Miata"]
            }))
            .unwrap();
            s.track_available = available;
            s
        };
        let (cars, tracks) = referenced(&[server("monza", true), server("rt_suzuka", false)]);
        assert!(cars.contains("ks_mazda_miata"), "car ids lowercased");
        assert_eq!(tracks.len(), 1, "the missing track is not looked up");
        assert!(tracks.contains("monza"));
    }
}
