//! The ping of a server, which no server reports: Pit Box measures it itself
//! (`SPEC-play-online.md`, "Ce qu'un serveur AC expose").
//!
//! A TCP connection to the server's HTTP port: the handshake is one round
//! trip, and that port is open on every listed server — unlike ICMP, which
//! hosts often filter, and which a standard user cannot send raw anyway.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::drivers::ServerAddr;
use super::fanout::fan_out;

/// Past this, the server is far enough that the exact figure no longer
/// matters — and waiting longer would hold the whole batch.
const TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ping {
    pub ip: String,
    pub http_port: u16,
    pub ms: u32,
}

fn address(ip: &str, port: u16) -> Option<SocketAddr> {
    (ip, port).to_socket_addrs().ok()?.next()
}

fn ping_one(server: &ServerAddr) -> Option<u32> {
    let addr = address(&server.ip, server.http_port)?;
    let start = Instant::now();
    TcpStream::connect_timeout(&addr, TIMEOUT).ok()?;
    Some(start.elapsed().as_millis().min(u128::from(u32::MAX)) as u32)
}

/// The servers that answered, with their round trip in milliseconds.
pub fn ping(servers: &[ServerAddr]) -> Vec<Ping> {
    fan_out(servers, |server| {
        ping_one(server).map(|ms| Ping {
            ip: server.ip.clone(),
            http_port: server.http_port,
            ms,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: a server that accepts the connection is measured — here a
    /// listener on this machine, answered in well under the timeout.
    #[test]
    fn a_listening_server_is_measured() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let pings = ping(&[ServerAddr {
            ip: "127.0.0.1".into(),
            http_port: port,
        }]);
        assert_eq!(pings.len(), 1, "measured");
        assert!(pings[0].ms < 1500, "within the timeout: {}", pings[0].ms);
    }

    /// Rule: an address that cannot be read is left out, not an error.
    #[test]
    fn an_unreadable_address_is_left_out() {
        let pings = ping(&[ServerAddr {
            ip: "not an address".into(),
            http_port: 1,
        }]);
        assert!(pings.is_empty());
    }
}
