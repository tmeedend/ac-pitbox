//! Who is driving where, to find friends (`SPEC-play-online.md`, case 1:
//! "Amis présents").
//!
//! The lobby counts players but never names them: only each server's `/JSON`
//! does. Asking every busy server is affordable — measured on 2026-10-03, 341
//! of 9 000 servers had players, and 32 requests at a time answered them all
//! in 4.5 s. This module returns every connected driver; matching them against
//! the friends list is the frontend's, which owns that list.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Requests in flight at once. Enough to scan a busy evening in seconds,
/// few enough not to look like a flood to any one host (most hosts run a
/// handful of servers, and the list is not grouped by host).
const PARALLEL: usize = 32;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerAddr {
    pub ip: String,
    pub http_port: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServerDrivers {
    pub ip: String,
    pub http_port: u16,
    pub drivers: Vec<String>,
}

/// Asks each server for its connected drivers, `PARALLEL` at a time. A server
/// that does not answer is left out: it is one fewer place to find a friend,
/// not an error — the lobby is full of servers that just went down.
pub fn scan(servers: &[ServerAddr], steam_id: u64) -> Vec<ServerDrivers> {
    scan_with(servers, |s| {
        super::server::fetch_entry_list(&s.ip, s.http_port, steam_id)
            .ok()
            .map(|entries| super::server::drivers(&entries).into_iter().map(|d| d.name).collect())
    })
}

/// The scan itself, the request given as a parameter so the fan-out can be
/// tested without a network.
fn scan_with(servers: &[ServerAddr], fetch: impl Fn(&ServerAddr) -> Option<Vec<String>> + Sync) -> Vec<ServerDrivers> {
    let next = AtomicUsize::new(0);
    let found = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL.min(servers.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(server) = servers.get(i) else { break };
                let Some(drivers) = fetch(server) else { continue };
                if drivers.is_empty() {
                    continue;
                }
                let entry = ServerDrivers {
                    ip: server.ip.clone(),
                    http_port: server.http_port,
                    drivers,
                };
                // A poisoned lock means another worker panicked: keep what it
                // left, the scan is best-effort anyway.
                found.lock().unwrap_or_else(|e| e.into_inner()).push(entry);
            });
        }
    });
    found.into_inner().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(i: u16) -> ServerAddr {
        ServerAddr {
            ip: format!("10.0.0.{i}"),
            http_port: 8000 + i,
        }
    }

    /// Rule: every server is asked exactly once, whatever the parallelism —
    /// more servers than workers is the normal case.
    #[test]
    fn every_server_is_asked_once() {
        let servers: Vec<ServerAddr> = (0..100).map(addr).collect();
        let asked = Mutex::new(Vec::new());
        let found = scan_with(&servers, |s| {
            asked.lock().unwrap().push(s.http_port);
            Some(vec![format!("driver {}", s.http_port)])
        });
        let mut asked = asked.into_inner().unwrap();
        asked.sort();
        assert_eq!(asked, (8000..8100).collect::<Vec<u16>>(), "each server once");
        assert_eq!(found.len(), 100, "one entry per answering server");
    }

    /// Rule: a server that does not answer, or has nobody on it, is simply
    /// absent from the result.
    #[test]
    fn silent_and_empty_servers_are_left_out() {
        let servers: Vec<ServerAddr> = (0..3).map(addr).collect();
        let found = scan_with(&servers, |s| match s.http_port {
            8000 => None,
            8001 => Some(Vec::new()),
            _ => Some(vec!["Léo".into()]),
        });
        assert_eq!(
            found,
            vec![ServerDrivers {
                ip: "10.0.0.2".into(),
                http_port: 8002,
                drivers: vec!["Léo".into()]
            }]
        );
    }
}
