//! The lobby's list as it came, kept a few minutes for everyone who reads it
//! (`SPEC-play-online.md`, v2: one list for the Online page and the track
//! sheets).
//!
//! Two floors, and this is the bottom one. Downloading the list costs the
//! network (some 9 000 servers, seconds); judging it against the library
//! (`judge_list`) costs the disk and the SQLite lock. A track sheet only
//! counts the servers running its track: it needs the first and never the
//! second. The Online page needs both, but after an import only the second
//! changes — the list itself is the same three minutes later. Keeping the raw
//! list here lets each reader pay for what it uses.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::lobby::ServerSummary;

/// How long the list stays good: a server's players move by the minute, and
/// the lobby itself refreshes about as often.
const FRESH: Duration = Duration::from_secs(3 * 60);

pub struct LobbyCache {
    /// Held across a download: a second reader waits for the first one's list
    /// rather than asking the lobby again.
    inner: Mutex<Option<(Instant, Arc<Vec<ServerSummary>>)>>,
}

/// The one list of the app.
pub static LOBBY: LobbyCache = LobbyCache::new();

impl LobbyCache {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// The list, from the cache while it is fresh; `force` asks the lobby
    /// anyway (the page's Refresh). A failed download keeps the old list for
    /// the next reader and returns the error.
    pub fn get(
        &self,
        force: bool,
        fetch: impl FnOnce() -> Result<Vec<ServerSummary>, String>,
    ) -> Result<Arc<Vec<ServerSummary>>, String> {
        self.get_at(Instant::now(), force, fetch)
    }

    fn get_at(
        &self,
        now: Instant,
        force: bool,
        fetch: impl FnOnce() -> Result<Vec<ServerSummary>, String>,
    ) -> Result<Arc<Vec<ServerSummary>>, String> {
        // A poisoned lock means a reader panicked mid-download: the cache is
        // only an optimisation, start again from what is there.
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, list)) = inner.as_ref() {
            if !force && now.duration_since(*at) < FRESH {
                return Ok(Arc::clone(list));
            }
        }
        let list = Arc::new(fetch()?);
        *inner = Some((now, Arc::clone(&list)));
        Ok(list)
    }
}

/// What is driven on a track right now, as its sheet shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrackActivity {
    /// Servers with at least one player on the track, any of its layouts.
    pub servers: u32,
    pub players: u32,
    /// Their layouts as the lobby names them (`folder-layout`, lowercase):
    /// what the Online page opens on.
    pub layouts: Vec<String>,
}

/// Counts the occupied servers running `track` (a folder) on one of its
/// `layouts` (`""` for a single-layout track) — or on the folder alone, the
/// form CM tries first — without asking any server and
/// without judging anything. The sheet knows its layouts, so the lobby's
/// `folder-layout` is matched exactly — a hyphen inside a folder name
/// (`trento-bondone`) cannot be mistaken for a layout. An empty server is not
/// activity: the sheet would announce hundreds of empty Shutoko servers.
pub fn track_activity(servers: &[ServerSummary], track: &str, layouts: &[String]) -> TrackActivity {
    let track = track.to_lowercase();
    let names: Vec<String> = layouts
        .iter()
        .map(|l| {
            if l.is_empty() {
                track.clone()
            } else {
                format!("{track}-{}", l.to_lowercase())
            }
        })
        .chain(std::iter::once(track.clone()))
        .collect();
    let mut activity = TrackActivity {
        servers: 0,
        players: 0,
        layouts: Vec::new(),
    };
    for server in servers.iter().filter(|s| s.clients > 0) {
        let kunos = server.track.kunos_id.to_lowercase();
        if !names.contains(&kunos) {
            continue;
        }
        activity.servers += 1;
        activity.players += server.clients;
        if !activity.layouts.contains(&kunos) {
            activity.layouts.push(kunos);
        }
    }
    activity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(track: &str, clients: u32) -> ServerSummary {
        let mut s = super::super::lobby::parse_server(&serde_json::json!({
            "ip": "1.2.3.4", "cport": 8081, "tport": 9600, "track": track, "cars": ["ks_mazda_miata"]
        }))
        .unwrap();
        s.clients = clients;
        s
    }

    /// Rule: the list is downloaded once for every reader within three
    /// minutes, again after, and at once when forced (the Refresh button).
    #[test]
    fn the_list_is_kept_three_minutes_unless_forced() {
        let cache = LobbyCache::new();
        let start = Instant::now();
        let downloads = std::cell::Cell::new(0);
        let get = |at: Instant, force: bool| {
            cache
                .get_at(at, force, || {
                    downloads.set(downloads.get() + 1);
                    Ok(vec![server("monza", 1)])
                })
                .unwrap();
        };
        get(start, false);
        get(start + Duration::from_secs(60), false);
        assert_eq!(
            downloads.get(),
            1,
            "a second reader within three minutes reuses the list"
        );
        get(start + Duration::from_secs(60), true);
        assert_eq!(downloads.get(), 2, "Refresh asks the lobby anyway");
        get(start + Duration::from_secs(60 + 181), false);
        assert_eq!(downloads.get(), 3, "past three minutes the list is stale");
    }

    /// Rule: a failed download returns its error and keeps the old list for
    /// the next reader.
    #[test]
    fn a_failed_download_keeps_the_old_list() {
        let cache = LobbyCache::new();
        let start = Instant::now();
        cache.get_at(start, false, || Ok(vec![server("monza", 1)])).unwrap();
        let late = start + Duration::from_secs(600);
        assert!(cache.get_at(late, false, || Err("lobby down".into())).is_err());
        let kept = cache.get_at(start, false, || Err("not asked".into())).unwrap();
        assert_eq!(kept.len(), 1, "the old list is still there");
    }

    /// Rule (SPEC-play-online.md, v2): a sheet counts the occupied servers of
    /// its track, on any of its layouts — a hyphenated folder of another track
    /// does not count, nor an empty server.
    #[test]
    fn a_track_counts_its_occupied_servers_on_its_layouts() {
        let servers = vec![
            server("csp/3424/../E/../Ph_highway", 16),
            server("shuto_revival_project_beta-main_layout", 10),
            server("SHUTO_REVIVAL_PROJECT_BETA-c1_outer", 3),
            server("shuto_revival_project_beta-main_layout", 0),
            server("shuto_revival_project_beta_2-main_layout", 8),
        ];
        let layouts = vec!["main_layout".to_string(), "c1_outer".to_string()];
        let shuto = track_activity(&servers, "shuto_revival_project_beta", &layouts);
        assert_eq!(
            (shuto.servers, shuto.players),
            (2, 13),
            "occupied servers of this track only"
        );
        assert_eq!(
            shuto.layouts,
            vec![
                "shuto_revival_project_beta-main_layout".to_string(),
                "shuto_revival_project_beta-c1_outer".to_string()
            ]
        );
        let highway = track_activity(&servers, "Ph_highway", &[String::new()]);
        assert_eq!(
            (highway.servers, highway.players),
            (1, 16),
            "single layout, CSP prefix gone, any case"
        );
    }
}
