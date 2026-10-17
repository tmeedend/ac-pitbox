//! What happened in the session the game just ran (SESSION§2.6): the result
//! file Assetto Corsa writes when a session ends, `Documents\Assetto
//! Corsa\out\race_out.json`, read into a classification per session.
//!
//! The format is read from real files, never guessed: the one the game wrote
//! on the dev machine, and the copies Content Manager keeps of every session
//! it ran (`%LOCALAPPDATA%\AcTools Content Manager\Progress\Sessions\*.json`,
//! the game's file plus two keys of CM's own, `__raceIni` and `__quickDrive`).
//! The four in `samples/` come from there, CM's two keys removed. What they
//! taught, none of which a field name says:
//!
//! - **`players[i]` is car `i`, and car 0 is the player.** Every per-car array
//!   (`lapstotal`, `raceResult`) and every `car` field indexes `players`.
//! - **`raceResult` is the finishing order**, as car indices, and only race
//!   sessions carry it. A player who quits is put last in it, but nothing in
//!   the file says "retired": see `classify_race`.
//! - **When the player leaves a race, the game finishes it for the others**:
//!   the AI laps after that moment are extrapolated (every sector of a lap
//!   shifted by the same few milliseconds), so the AI always reach the race
//!   distance. Their totals stay consistent with the order.
//! - **`bestLaps` leaves out cut laps** (`cuts > 0`), and the extrapolated
//!   ones: it is the game's own verdict on what counts, so it is used as is
//!   rather than recomputed from `laps`.
//! - **A lap with `time: -1` is a lap begun, not driven** — an AI sitting in
//!   the pits during practice.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// The session kinds of `race.ini`'s `[SESSION_n] TYPE`, as far as real files
/// show them: 1 to 4 are attested in the samples. The others (time attack,
/// drift, drag) exist in the game but no file here carries one, so they stay
/// `Other` rather than a guessed name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Practice,
    Qualifying,
    Race,
    Hotlap,
    Other,
}

impl SessionKind {
    fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Practice,
            2 => Self::Qualifying,
            3 => Self::Race,
            4 => Self::Hotlap,
            _ => Self::Other,
        }
    }
}

/// One car's line in a session's classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Standing {
    /// 1 for the winner (race) or the fastest (any other session).
    pub position: u32,
    /// Index in the entry list; **0 is the player**.
    pub car: usize,
    pub driver_name: String,
    pub car_id: String,
    pub skin: String,
    /// Laps completed, as the game counts them (`lapstotal`).
    pub laps: u32,
    /// Best valid lap in milliseconds; `None` when no lap counted.
    pub best_lap_ms: Option<u32>,
    /// Sum of the car's completed laps, in milliseconds.
    pub total_ms: u64,
    /// Did not take the chequered flag. Only ever true in a race.
    pub dnf: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionOutcome {
    pub name: String,
    pub kind: SessionKind,
    /// Race distance in laps; 0 for a session run against the clock.
    pub race_laps: u32,
    pub duration_minutes: u32,
    pub standings: Vec<Standing>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionResult {
    pub track: String,
    /// In the order they were run: practice, qualifying, race.
    pub sessions: Vec<SessionOutcome>,
}

// --- The file as the game writes it -----------------------------------------

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    track: String,
    #[serde(default)]
    players: Vec<RawPlayer>,
    #[serde(default)]
    sessions: Vec<RawSession>,
}

#[derive(Deserialize)]
struct RawPlayer {
    #[serde(default)]
    name: String,
    #[serde(default)]
    car: String,
    #[serde(default)]
    skin: String,
}

#[derive(Deserialize)]
struct RawSession {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: u32,
    #[serde(rename = "lapsCount", default)]
    laps_count: u32,
    #[serde(default)]
    duration: u32,
    #[serde(default)]
    laps: Vec<RawLap>,
    #[serde(default)]
    lapstotal: Vec<u32>,
    #[serde(rename = "bestLaps", default)]
    best_laps: Vec<RawBestLap>,
    #[serde(rename = "raceResult", default)]
    race_result: Option<Vec<usize>>,
}

#[derive(Deserialize)]
struct RawLap {
    car: usize,
    time: i64,
}

#[derive(Deserialize)]
struct RawBestLap {
    car: usize,
    time: u32,
}

/// Reads the bytes of a result file. UTF-8 is what every real file holds
/// (accented driver names included); a byte sequence that is not UTF-8 is
/// read as Latin-1 rather than refused, so that one odd name never costs the
/// whole result.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

pub fn parse(bytes: &[u8]) -> Result<SessionResult, String> {
    let raw: RawFile = serde_json::from_str(&decode(bytes)).map_err(|e| format!("race_out.json: {e}"))?;
    let sessions = raw.sessions.iter().map(|s| classify(&raw.players, s)).collect();
    Ok(SessionResult {
        track: raw.track,
        sessions,
    })
}

fn classify(players: &[RawPlayer], s: &RawSession) -> SessionOutcome {
    let kind = SessionKind::from_code(s.kind);
    let lines: Vec<Standing> = players
        .iter()
        .enumerate()
        .map(|(car, p)| Standing {
            position: 0,
            car,
            driver_name: p.name.clone(),
            car_id: p.car.clone(),
            skin: p.skin.clone(),
            laps: s.lapstotal.get(car).copied().unwrap_or(0),
            best_lap_ms: s.best_laps.iter().filter(|b| b.car == car).map(|b| b.time).min(),
            total_ms: s
                .laps
                .iter()
                .filter(|l| l.car == car && l.time > 0)
                .map(|l| l.time as u64)
                .sum(),
            dnf: false,
        })
        .collect();

    let order = match s.race_result.as_deref() {
        Some(order) if kind == SessionKind::Race && is_permutation(order, lines.len()) => order.to_vec(),
        _ => order_by_best_lap(&lines),
    };
    let mut ordered: Vec<Standing> = order.iter().map(|&car| lines[car].clone()).collect();
    for (i, line) in ordered.iter_mut().enumerate() {
        line.position = i as u32 + 1;
    }
    if kind == SessionKind::Race {
        classify_race(&mut ordered);
    }
    SessionOutcome {
        name: s.name.clone(),
        kind,
        race_laps: s.laps_count,
        duration_minutes: s.duration,
        standings: ordered,
    }
}

/// Marks who did not finish, from the order and the times alone — the file
/// has no field for it.
///
/// The rule: **a finisher crosses the line after the winner**. Once the winner
/// takes the flag, every other car finishes at its next crossing, a lapped
/// car included, so its total time is at least the winner's. A car whose total
/// is shorter stopped driving before the flag fell: the player who quit (the
/// game puts them last, with fewer laps). The rule holds for a race run over
/// laps and one run against the clock alike, and needs no guess about how many
/// laps a lapped car is allowed to be down. A car with no lap at all never
/// finished, which also covers a race abandoned on the grid, where the
/// "winner" itself has nothing.
fn classify_race(ordered: &mut [Standing]) {
    let Some(winner_total) = ordered.first().map(|w| w.total_ms) else {
        return;
    };
    for line in ordered.iter_mut() {
        line.dnf = line.laps == 0 || line.total_ms < winner_total;
    }
}

fn is_permutation(order: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    order.len() == n && order.iter().all(|&c| c < n && !std::mem::replace(&mut seen[c], true))
}

/// Practice, qualifying, hotlap: the fastest first, cars without a valid lap
/// after them in entry order.
fn order_by_best_lap(lines: &[Standing]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by_key(|&c| (lines[c].best_lap_ms.is_none(), lines[c].best_lap_ms, c));
    order
}

/// `Documents\Assetto Corsa\out\race_out.json`.
pub fn result_path() -> Option<PathBuf> {
    Some(crate::media::documents_ac_dir()?.join("out").join("race_out.json"))
}

/// The result of a session that ended after `since`, or `None` when the file
/// is missing or older — the game rewrites it at the end of every session,
/// so an older file belongs to a previous one, and returning it would credit
/// the player with a result they did not get.
pub fn read_since(path: &Path, since: SystemTime) -> Result<Option<SessionResult>, String> {
    let modified = match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if modified < since {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&bytes).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sample(name: &str) -> SessionResult {
        let bytes: &[u8] = match name {
            "hotlap" => include_bytes!("samples/hotlap_no_lap.json"),
            "retired" => include_bytes!("samples/race_player_retired.json"),
            "weekend" => include_bytes!("samples/weekend_two_cars.json"),
            "grid" => include_bytes!("samples/weekend_race_left_on_grid.json"),
            _ => unreachable!(),
        };
        parse(bytes).expect("real sample parses")
    }

    fn player(s: &SessionOutcome) -> &Standing {
        s.standings.iter().find(|l| l.car == 0).expect("the player is car 0")
    }

    fn last(r: &SessionResult) -> &SessionOutcome {
        r.sessions.last().expect("at least one session")
    }

    /// The file exactly as the game writes it (leading CRLF, tabs, a trailing
    /// space after a comma) parses, and a hotlap without a completed lap gives
    /// the player a line with nothing on it rather than no line.
    #[test]
    fn the_game_own_file_parses_as_written() {
        let r = sample("hotlap");
        assert_eq!(r.track, "korea");
        let s = last(&r);
        assert_eq!(s.kind, SessionKind::Hotlap);
        let p = player(s);
        assert_eq!((p.position, p.laps, p.best_lap_ms, p.dnf), (1, 0, None, false));
        assert_eq!(p.car_id, "ks_ford_gt40");
    }

    /// `raceResult` is the order; the player who quits after two laps of seven
    /// is last and DNF, while every AI reaches the flag (the game finishes the
    /// race for them) — SESSION§2.6.
    #[test]
    fn a_player_who_quits_a_race_is_last_and_did_not_finish() {
        let s = sample("retired").sessions.remove(0);
        assert_eq!((s.kind, s.race_laps), (SessionKind::Race, 7));
        let order: Vec<usize> = s.standings.iter().map(|l| l.car).collect();
        assert_eq!(order, vec![1, 2, 5, 3, 7, 4, 6, 0], "the game's own order");
        let winner = &s.standings[0];
        assert_eq!(
            (winner.driver_name.as_str(), winner.laps, winner.total_ms),
            ("Ali Goz", 7, 676_902)
        );
        assert!(s.standings[..7].iter().all(|l| !l.dnf), "every AI took the flag");
        let p = player(&s);
        assert_eq!(
            (p.position, p.laps, p.dnf),
            (8, 2, true),
            "the player stopped before the flag"
        );
    }

    /// A cut lap does not count: the player's only lap of the race was cut, so
    /// they have no best lap, while the AI's best is the game's own figure.
    #[test]
    fn a_cut_lap_is_never_a_best_lap() {
        let r = sample("weekend");
        let race = last(&r);
        assert_eq!(player(race).best_lap_ms, None, "the player's lap had a cut");
        let ai = race.standings.iter().find(|l| l.car == 1).unwrap();
        assert_eq!(ai.best_lap_ms, Some(92_787), "bestLaps, as the game wrote it");
    }

    /// A weekend gives three sessions in the order they ran; practice and
    /// qualifying are ordered by best lap, and a car whose only lap was begun
    /// but not driven (`time: -1`) has no time at all.
    #[test]
    fn a_weekend_keeps_its_three_sessions_in_order() {
        let r = sample("weekend");
        let kinds: Vec<SessionKind> = r.sessions.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![SessionKind::Practice, SessionKind::Qualifying, SessionKind::Race]
        );
        let practice = &r.sessions[0];
        assert!(practice
            .standings
            .iter()
            .all(|l| l.total_ms == 0 && l.best_lap_ms.is_none()));
        assert_eq!(practice.standings[0].car, 0, "no lap anywhere: entry order");
        assert!(practice.standings.iter().all(|l| !l.dnf), "DNF is a race notion only");
    }

    /// A race the player leaves on the grid: no one completed a lap, so no one
    /// finished — not even the car the game lists first. Accented names come
    /// through intact.
    #[test]
    fn a_race_left_on_the_grid_has_no_finisher() {
        let r = sample("grid");
        let race = last(&r);
        assert_eq!(race.kind, SessionKind::Race);
        assert!(race.standings.iter().all(|l| l.dnf), "nobody drove a lap");
        assert!(r.sessions[0]
            .standings
            .iter()
            .any(|l| l.driver_name == "Stéphane Amoros"));
    }

    /// A best-lap order: the fastest first, cars without a valid lap after,
    /// in entry order.
    #[test]
    fn practice_is_ordered_by_best_lap() {
        let json = br#"{"track":"t","players":[{"name":"P","car":"a","skin":"s"},{"name":"A","car":"a","skin":"s"},{"name":"B","car":"a","skin":"s"}],
            "sessions":[{"name":"Practice","type":1,"lapsCount":0,"duration":10,"laps":[],"lapstotal":[2,3,0],
            "bestLaps":[{"car":0,"time":91000,"lap":1},{"car":1,"time":90000,"lap":2}]}]}"#;
        let s = parse(json).unwrap().sessions.remove(0);
        let order: Vec<usize> = s.standings.iter().map(|l| l.car).collect();
        assert_eq!(order, vec![1, 0, 2]);
        assert_eq!(s.standings[1].position, 2);
    }

    /// A lapped car that takes the flag is not DNF: it crosses the line after
    /// the winner, whatever its lap count.
    #[test]
    fn a_lapped_finisher_is_not_dnf() {
        let json = br#"{"track":"t","players":[{"name":"P","car":"a","skin":"s"},{"name":"A","car":"a","skin":"s"}],
            "sessions":[{"name":"Race","type":3,"lapsCount":2,"duration":0,
            "laps":[{"car":1,"time":60000},{"car":1,"time":60000},{"car":0,"time":130000}],
            "lapstotal":[1,2],"bestLaps":[],"raceResult":[1,0]}]}"#;
        let s = parse(json).unwrap().sessions.remove(0);
        let p = player(&s);
        assert_eq!(
            (p.position, p.laps, p.dnf),
            (2, 1, false),
            "one lap down, flag taken after the winner"
        );
    }

    /// A `raceResult` that does not name every car once is not trusted: the
    /// order falls back on best laps rather than dropping or doubling a car.
    #[test]
    fn a_broken_race_order_falls_back_on_best_laps() {
        let json = br#"{"track":"t","players":[{"name":"P","car":"a","skin":"s"},{"name":"A","car":"a","skin":"s"}],
            "sessions":[{"name":"Race","type":3,"lapsCount":1,"duration":0,"laps":[],"lapstotal":[0,0],
            "bestLaps":[],"raceResult":[1,1]}]}"#;
        let s = parse(json).unwrap().sessions.remove(0);
        assert_eq!(s.standings.len(), 2, "every car once");
    }

    #[test]
    fn a_file_that_is_not_json_is_an_error() {
        assert!(parse(b"not json").is_err());
    }

    /// The file of a previous session is never read as this one's: older than
    /// the launch means not ours.
    #[test]
    fn a_result_older_than_the_launch_is_ignored() {
        let base = crate::testutil::temp_dir("raceresult");
        let path = base.join("race_out.json");
        assert_eq!(
            read_since(&path, SystemTime::now()).unwrap(),
            None,
            "no file, no result"
        );
        std::fs::write(&path, include_bytes!("samples/hotlap_no_lap.json")).unwrap();
        let later = SystemTime::now() + Duration::from_secs(3600);
        assert_eq!(read_since(&path, later).unwrap(), None, "written before the launch");
        let earlier = SystemTime::now() - Duration::from_secs(3600);
        assert!(
            read_since(&path, earlier).unwrap().is_some(),
            "written after the launch"
        );
    }
}
