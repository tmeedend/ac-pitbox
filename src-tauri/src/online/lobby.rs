//! The server list: the Kunos lobby, and the `/INFO` of one server, which share
//! a format ("Ce qu'un serveur AC expose", level 1, of `SPEC-play-online.md`).
//!
//! Three things about that format were learned on real answers
//! (`docs/online-join-research.md`), and each has a place below:
//! - **numbers come as strings in the lobby** (`"sessiontypes":["1","3"]`) and
//!   as numbers in `/INFO` — hence the lenient readers rather than a derive;
//! - **`session` does not mean the same thing in both**: the lobby writes the
//!   active session's TYPE (`1` = practice), `/INFO` its INDEX in
//!   `sessiontypes` (`0` = the first one). Measured on three servers, vanilla
//!   and AssettoServer alike, the same minute: lobby `1`, `/INFO` `0`, for a
//!   weekend of practice only. Hence `parse_server` and `parse_info`;
//! - **the track id carries the CSP requirement**:
//!   `csp/3465/../E/../la_canyons-freeroam`.

use serde::Serialize;
use serde_json::Value;

use crate::http;

use super::readiness::{Blocker, Level};

/// The official list every public server registers with, and the one the game
/// itself asks. It answers only to its own launcher's user agent (anything
/// else is redirected to the site root) and only for a known Steam account
/// (an unknown `guid` gets `UNKNOWN FAILURE`).
const LOBBY_URL: &str = "http://93.57.10.21/lobby.ashx/list";
const LOBBY_AGENT: &str = "Assetto Corsa Launcher";
/// About 1.5 MB on the wire (gzip), 7.8 MB once decompressed for 9 000 servers.
const LOBBY_TIMEOUT_MS: i32 = 20_000;
/// Four times what the list weighs today: the ceiling is there against a
/// runaway answer, not against the lobby growing.
const LOBBY_MAX_BODY: usize = 32 * 1024 * 1024;

/// What a game server's own HTTP port is asked with. Any agent works there;
/// this one names the app in the server's logs.
pub(super) const SERVER_AGENT: &str = "PitBox";
/// A server that takes longer than this to answer its own `/INFO` is not one
/// anyone will want to join.
pub(super) const SERVER_TIMEOUT_MS: i32 = 5_000;
pub(super) const SERVER_MAX_BODY: usize = 4 * 1024 * 1024;

/// A session type as the protocol numbers them (`Game.SessionType` in CM).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    Booking,
    Practice,
    Qualify,
    Race,
}

impl SessionKind {
    fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Booking),
            1 => Some(Self::Practice),
            2 => Some(Self::Qualify),
            3 => Some(Self::Race),
            _ => None,
        }
    }
}

/// The track a server runs, decoded from its id.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TrackRef {
    /// The id as Content Manager expects it in `race/online&track=`:
    /// `folder-layout`, the `csp/…/../` prefix removed. Passing the raw value
    /// makes CM answer "Track is missing".
    pub kunos_id: String,
    /// The folder under `content/tracks/`.
    pub id: String,
    /// The layout, `None` for a single-layout track. The split of `kunos_id`
    /// is ambiguous (`trento-bondone` is a folder), so it is first guessed on
    /// the last hyphen, then corrected against what is installed
    /// (`Installed::judge`).
    pub layout: Option<String>,
    /// Minimum CSP build the server requires, when it requires one.
    pub csp_min_build: Option<u32>,
}

/// Splits `csp/<build>/../[<flags>/../]<id>` the way CM does
/// (`ServerEntry.cs`): cut on `/../`, the last piece is the track, the first
/// the CSP build. A plain id has no requirement.
pub fn decode_track(raw: &str) -> TrackRef {
    let body = raw.strip_prefix("csp/").unwrap_or(raw);
    let pieces: Vec<&str> = body.split("/../").collect();
    let kunos_id = pieces.last().copied().unwrap_or_default().to_string();
    let csp_min_build = if pieces.len() >= 2 {
        pieces[0].parse().ok()
    } else {
        None
    };
    let (id, layout) = match kunos_id.rsplit_once('-') {
        Some((id, layout)) if !id.is_empty() && !layout.is_empty() => (id.to_string(), Some(layout.to_string())),
        _ => (kunos_id.clone(), None),
    };
    TrackRef {
        kunos_id,
        id,
        layout,
        csp_min_build,
    }
}

/// One server, as listed. Everything the list shows and the join needs; the
/// per-car slots come later, from the server itself (`server.rs`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServerSummary {
    pub ip: String,
    /// The TCP race port (`tport`), which `race/online&port=` expects.
    pub port: u16,
    /// The HTTP port (`cport`) that answers `/INFO` and `/JSON`.
    pub http_port: u16,
    pub name: String,
    /// ISO code of the country the server declares.
    pub country: Option<String>,
    pub clients: u32,
    pub max_clients: u32,
    pub password: bool,
    /// Booking mode: slots are reserved before the session, and CM's own
    /// dialog is the only way in (`race/online` does not book).
    pub booking: bool,
    pub track: TrackRef,
    pub cars: Vec<String>,
    /// The session running now.
    pub session: Option<SessionKind>,
    /// Every session of the weekend, in order.
    pub sessions: Vec<SessionKind>,
    /// Seconds left in the current session.
    pub time_left: u64,
    /// One per entry of `sessions`. Seconds, except a race that is not
    /// `timed`, counted in laps — CM's own reading (`Session.DisplayDuration`).
    pub durations: Vec<u64>,
    /// The race runs on time rather than laps.
    pub timed: bool,
    /// A timed race ends with one more lap after the clock.
    pub extra_lap: bool,
    pub inverted_grid: bool,
    pub mandatory_pit: bool,
    /// Filled by `Installed::judge`: the track can be driven (installed or in
    /// the library).
    pub track_available: bool,
    /// Filled by `Installed::judge`: how many of `cars` can be driven.
    pub cars_available: u32,
    /// Filled by `Installed::judge`: how ready the server is, the worse of
    /// its track, its best car and its CSP requirement (`readiness.rs`).
    pub level: Level,
    /// Filled by `Installed::judge`: the track's own level, which the panel
    /// combines with the chosen car's.
    pub track_level: Level,
    /// Filled by `Installed::judge`: what blocks, to be named on screen.
    pub blockers: Vec<Blocker>,
    /// Filled by `Installed::judge`: the track and every car are Kunos
    /// content, base game or DLC — the list's `content: kunos` token.
    pub official: bool,
}

/// A number, whether the server wrote it as one or as a string.
fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
}

/// A yes/no the lobby writes as a boolean and some `/INFO` as a number
/// (`"inverted":0`, or the position of the inverted grid).
fn flag(value: &Value) -> bool {
    value.as_bool().unwrap_or_else(|| number(value).is_some_and(|n| n != 0))
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

fn port(value: &Value) -> Option<u16> {
    number(value).and_then(|n| u16::try_from(n).ok()).filter(|&p| p != 0)
}

/// The "ℹ8081" some server managers append to the name — the HTTP port, for
/// CM's details wrapper. Noise in a list: the port is shown elsewhere.
fn clean_name(name: &str) -> String {
    let trimmed = name.trim_end();
    match trimmed.rfind('ℹ') {
        Some(i) if trimmed[i + 'ℹ'.len_utf8()..].chars().all(|c| c.is_ascii_digit()) => {
            trimmed[..i].trim_end().to_string()
        }
        _ => trimmed.to_string(),
    }
}

/// Reads one server's own `/INFO` answer: the lobby's format, but `session`
/// is an index into `sessiontypes` (module docs).
pub fn parse_info(info: &Value) -> Option<ServerSummary> {
    let mut server = parse_server(info)?;
    server.session = number(&info["session"])
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| server.sessions.get(i).copied());
    Some(server)
}

/// Reads one server from a lobby entry, where `session` is a type. `None`
/// when it lacks what a join needs (address, ports, track).
pub fn parse_server(entry: &Value) -> Option<ServerSummary> {
    let ip = text(&entry["ip"]).filter(|ip| !ip.is_empty())?;
    let http_port = port(&entry["cport"])?;
    let race_port = port(&entry["tport"]).or_else(|| port(&entry["port"]))?;
    let track = decode_track(entry["track"].as_str()?);
    let list = |key: &str| entry[key].as_array().cloned().unwrap_or_default();
    Some(ServerSummary {
        ip,
        port: race_port,
        http_port,
        name: clean_name(entry["name"].as_str().unwrap_or_default()),
        // A server's own `/INFO` may say `["na","na"]` where the lobby, which
        // geolocates it, says `FR`: "na" is no country.
        country: list("country")
            .get(1)
            .and_then(text)
            .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("na")),
        clients: number(&entry["clients"]).unwrap_or(0) as u32,
        max_clients: number(&entry["maxclients"]).unwrap_or(0) as u32,
        password: entry["pass"].as_bool().unwrap_or(false),
        // Absent means "pickup": the 19 booking servers out of 9 000 are the
        // ones that say so explicitly.
        booking: !entry["pickup"].as_bool().unwrap_or(true),
        track,
        cars: list("cars").iter().filter_map(text).collect(),
        session: number(&entry["session"]).and_then(SessionKind::from_code),
        sessions: list("sessiontypes")
            .iter()
            .filter_map(number)
            .filter_map(SessionKind::from_code)
            .collect(),
        time_left: number(&entry["timeleft"]).unwrap_or(0),
        durations: list("durations").iter().filter_map(number).collect(),
        timed: flag(&entry["timed"]),
        extra_lap: flag(&entry["extra"]),
        inverted_grid: flag(&entry["inverted"]),
        mandatory_pit: flag(&entry["pit"]),
        track_available: false,
        cars_available: 0,
        level: Level::default(),
        track_level: Level::default(),
        blockers: Vec::new(),
        official: false,
    })
}

/// Reads the whole lobby answer. Entries missing what a join needs are
/// dropped, and counted in the log rather than silently.
///
/// A server listed twice keeps its first entry: the lobby does repeat some
/// (two `ip:cport` pairs out of 9 000 on 2026-09-30), and `ip:http_port` is
/// the identity the screen keys its rows on.
pub fn parse_lobby(body: &[u8]) -> Result<Vec<ServerSummary>, String> {
    let root: Value = serde_json::from_slice(body).map_err(|_| crate::errors::LOBBY_UNAVAILABLE.to_string())?;
    let entries = root
        .as_array()
        .ok_or_else(|| crate::errors::LOBBY_UNAVAILABLE.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let servers: Vec<ServerSummary> = entries
        .iter()
        .filter_map(parse_server)
        .filter(|s| seen.insert((s.ip.clone(), s.http_port)))
        .collect();
    if servers.len() < entries.len() {
        log::warn!(
            "online: {} lobby entries duplicated or without address or track, left out",
            entries.len() - servers.len()
        );
    }
    Ok(servers)
}

/// Fetches the list of public servers for the signed-in Steam account.
pub fn fetch_lobby(steam_id: u64) -> Result<Vec<ServerSummary>, String> {
    let url = format!("{LOBBY_URL}?guid={steam_id}");
    let response = http::get_url(&url, LOBBY_AGENT, LOBBY_TIMEOUT_MS, LOBBY_MAX_BODY)
        .ok_or_else(|| crate::errors::LOBBY_UNAVAILABLE.to_string())?;
    if response.status != 200 {
        log::warn!("online: the lobby answered {}", response.status);
        return Err(crate::errors::LOBBY_UNAVAILABLE.to_string());
    }
    parse_lobby(&response.body).inspect_err(|_| {
        // "UNKNOWN FAILURE" and SQL errors come back as a 200 with a text
        // body: the start of it is what tells them apart in a user's log.
        let head = String::from_utf8_lossy(&response.body[..response.body.len().min(120)]).into_owned();
        log::warn!("online: the lobby answer is not a server list — {head}");
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (online-join-research.md): the CSP prefix is stripped from what CM
    /// receives, and its build is kept — both measured on real servers.
    #[test]
    fn a_csp_track_id_is_decoded() {
        let flagged = decode_track("csp/3465/../E/../la_canyons-freeroam");
        assert_eq!(flagged.kunos_id, "la_canyons-freeroam", "prefix and flags stripped");
        assert_eq!(flagged.id, "la_canyons");
        assert_eq!(flagged.layout.as_deref(), Some("freeroam"));
        assert_eq!(flagged.csp_min_build, Some(3465), "build read from the first piece");

        let plain = decode_track("csp/2651/../la_canyons-freeroam");
        assert_eq!(plain.kunos_id, "la_canyons-freeroam", "without flags");
        assert_eq!(plain.csp_min_build, Some(2651));
    }

    /// Rule: a vanilla id carries no requirement, and a track without a
    /// hyphen has no layout.
    #[test]
    fn a_plain_track_id_has_no_requirement() {
        let nords = decode_track("ks_nordschleife-touristenfahrten");
        assert_eq!(nords.csp_min_build, None);
        assert_eq!(nords.id, "ks_nordschleife");
        assert_eq!(nords.layout.as_deref(), Some("touristenfahrten"));

        let monza = decode_track("monza");
        assert_eq!(monza.id, "monza");
        assert_eq!(monza.layout, None, "single layout");
    }

    /// A real lobby entry, verbatim but for its car list: numbers as strings,
    /// the "ℹ<port>" suffix, `session` holding a TYPE.
    const LOBBY_ENTRY: &str = r#"{"ip":"5.9.255.37","port":9690,"cport":8071,"tport":9690,
        "name":"LEGEND GARAGE | #04 | Nordschleife Tourist II ℹ8071","clients":3,"maxclients":32,
        "track":"ks_nordschleife-touristenfahrten","cars":["ks_audi_tt_cup","ks_toyota_celica_st185"],
        "timeofday":48,"session":1,"sessiontypes":["1"],"durations":["7200"],"timeleft":6958,
        "country":["Germany","DE"],"pass":false,"pickup":true,"timestamp":46,"lastupdate":0,
        "timed":true,"extra":false,"l":false,"inverted":false,"pit":false}"#;

    /// Rule: the lobby's string-typed numbers read like `/INFO`'s real ones.
    #[test]
    fn a_lobby_entry_is_read() {
        let server = parse_server(&serde_json::from_str(LOBBY_ENTRY).unwrap()).expect("complete entry");
        assert_eq!((server.port, server.http_port), (9690, 8071), "race port and HTTP port");
        assert_eq!(
            server.name, "LEGEND GARAGE | #04 | Nordschleife Tourist II",
            "info suffix dropped"
        );
        assert_eq!(server.country.as_deref(), Some("DE"), "ISO code, not the name");
        assert_eq!(
            server.sessions,
            vec![SessionKind::Practice],
            "string codes read as numbers"
        );
        assert_eq!(
            server.session,
            Some(SessionKind::Practice),
            "session is a type, not an index"
        );
        assert!(!server.booking, "pickup server");
        assert_eq!(server.cars.len(), 2);
        assert_eq!(server.durations, vec![7200], "string durations read as numbers");
        assert!(
            server.timed && !server.inverted_grid && !server.mandatory_pit,
            "flags read"
        );
    }

    /// Rule: a flag may come as a boolean or as a number, whatever the source.
    #[test]
    fn flags_read_as_booleans_or_numbers() {
        assert!(flag(&serde_json::json!(true)));
        assert!(flag(&serde_json::json!(1)), "inverted grid at position 1");
        assert!(!flag(&serde_json::json!(0)));
        assert!(!flag(&Value::Null), "absent");
    }

    /// Rule: `/INFO` writes real numbers, and its `session` is an INDEX — the
    /// same server answered `0` there and `1` in the lobby, practice both
    /// times. Read as a type, `0` showed "Booking" in the detail panel.
    #[test]
    fn an_info_session_is_an_index() {
        let info = r#"{"ip":"194.35.12.49","port":9600,"cport":8081,"tport":9600,"name":"x","clients":16,
            "maxclients":75,"track":"csp/2651/../la_canyons-freeroam","cars":[],"session":0,
            "sessiontypes":[1],"timeleft":524,"country":["Germany","DE"],"pass":true,"pickup":false}"#;
        let server = parse_info(&serde_json::from_str(info).unwrap()).expect("complete answer");
        assert_eq!(server.session, Some(SessionKind::Practice), "index 0 of [practice]");
        assert!(server.password && server.booking, "flags read");
        assert_eq!(server.track.csp_min_build, Some(2651));

        let weekend = r#"{"ip":"1.2.3.4","cport":8081,"tport":9600,"track":"monza",
            "session":2,"sessiontypes":[1,2,3]}"#;
        let server = parse_info(&serde_json::from_str(weekend).unwrap()).unwrap();
        assert_eq!(server.session, Some(SessionKind::Race), "third of three");
    }

    /// Rule: "na" is no country — measured in a server's own `/INFO`, where
    /// the lobby said FR for the same server.
    #[test]
    fn na_is_no_country() {
        let info = r#"{"ip":"1.2.3.4","cport":8001,"tport":9001,"track":"monza","country":["na","na"]}"#;
        assert_eq!(parse_info(&serde_json::from_str(info).unwrap()).unwrap().country, None);
    }

    /// Rule: an entry nobody can join is left out rather than shown broken.
    #[test]
    fn an_entry_without_ports_is_dropped() {
        let body = br#"[{"ip":"1.2.3.4","cport":0,"tport":9600,"track":"monza"},
                        {"ip":"1.2.3.4","cport":8081,"tport":9600,"track":"monza"}]"#;
        let servers = parse_lobby(body).unwrap();
        assert_eq!(servers.len(), 1, "the entry with no HTTP port is gone");
    }

    /// Rule: a server the lobby lists twice is shown once — the screen keys
    /// its rows on `ip:http_port`, and a duplicate key breaks the list.
    #[test]
    fn a_server_listed_twice_is_kept_once() {
        let body = br#"[{"ip":"1.2.3.4","cport":8081,"tport":9600,"track":"monza","name":"first"},
                        {"ip":"1.2.3.4","cport":8081,"tport":9600,"track":"monza","name":"again"}]"#;
        let servers = parse_lobby(body).unwrap();
        assert_eq!(servers.len(), 1, "one row per ip:http_port");
        assert_eq!(servers[0].name, "first", "the first entry wins");
    }

    /// Rule: the lobby's text failures ("UNKNOWN FAILURE", SQL errors, both
    /// seen with a bad guid) become the user-facing key, not a parse error.
    #[test]
    fn a_text_answer_is_an_unavailable_lobby() {
        assert_eq!(
            parse_lobby(b"UNKNOWN FAILURE").unwrap_err(),
            crate::errors::LOBBY_UNAVAILABLE
        );
    }
}
