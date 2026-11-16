//! What the screens show of a recorded session: the screen of a session
//! (RESULTAT§4, RESULTAT§7.4), the lines of the history (RESULTAT§7), what
//! Edit loads (RESULTAT§8) and the result of a replay (RESULTAT§11).

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

use super::mode_of_launch;
use crate::overlay::{self, Launch, SessionRecord};
use crate::raceresult::SessionResult;

/// The server of an online session, for "Open server" (RESULTAT§8).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Server {
    pub ip: String,
    pub http_port: u16,
}

/// What a session was driven in, when it is known (RESULTAT§7.4).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Conditions {
    pub weather: Option<String>,
    pub time_hours: Option<f64>,
    pub ambient_c: Option<f64>,
    pub road_c: Option<f64>,
    pub track_state: Option<String>,
}

/// What every screen says of a session: who, where, when, and what can be
/// done with it.
#[derive(Debug, Clone, Serialize)]
pub struct SessionHead {
    pub launch_id: Option<i64>,
    pub session_id: Option<i64>,
    /// `session` | `online` (Pit Box's) | `cm` (started elsewhere).
    pub kind: String,
    pub mode: String,
    /// The launch; for a session of CM, its end less the laps the player drove.
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub car_id: Option<String>,
    pub car_name: Option<String>,
    pub car_known: bool,
    pub track_id: Option<String>,
    pub track_name: Option<String>,
    pub track_known: bool,
    pub track_layout: Option<String>,
    pub best_lap_ms: Option<i64>,
    /// "Again": a session of the session screen, or the online join still in
    /// memory (RESULTAT§4.3).
    pub can_again: bool,
    /// "Edit": a preset to load (RESULTAT§8).
    pub can_edit: bool,
    pub server: Option<Server>,
}

/// The screen of a session (RESULTAT§4).
#[derive(Debug, Clone, Serialize)]
pub struct ReturnView {
    #[serde(flatten)]
    pub head: SessionHead,
    /// `None` when the game wrote nothing for this session.
    pub result: Option<SessionResult>,
    /// The record before this session, on this car and layout.
    pub previous_best_ms: Option<i64>,
    pub new_best: bool,
    pub conditions: Option<Conditions>,
}

/// A line of the history (RESULTAT§7).
#[derive(Debug, Clone, Serialize)]
pub struct HistoryLine {
    #[serde(flatten)]
    pub head: SessionHead,
    /// Laps of the player with no cut.
    pub valid_laps: u32,
    /// The player's place when the last session was a race.
    pub race_position: Option<u32>,
}

fn kind_of(launch: Option<&Launch>) -> String {
    match launch.map(|l| l.kind.as_str()) {
        Some("online") => "online".into(),
        Some(_) => "session".into(),
        None => "cm".into(),
    }
}

fn server_of(launch: Option<&Launch>) -> Option<Server> {
    let l = launch.filter(|l| l.kind == "online")?;
    let v: Value = serde_json::from_str(l.detail.as_deref()?).ok()?;
    Some(Server {
        ip: v.get("ip")?.as_str()?.to_string(),
        http_port: u16::try_from(v.get("http_port")?.as_u64()?).ok()?,
    })
}

fn has_pitbox_block(launch: Option<&Launch>) -> bool {
    launch
        .and_then(|l| l.preset.as_deref())
        .and_then(|p| serde_json::from_str::<Value>(p).ok())
        .is_some_and(|p| crate::sessionpreset::pitbox_block(&p).is_some())
}

/// The player's place when the last session was a race.
fn race_position(result: &SessionResult) -> Option<u32> {
    let last = result
        .sessions
        .last()
        .filter(|s| s.kind == crate::raceresult::SessionKind::Race)?;
    last.standings.iter().find(|l| l.car == 0).map(|l| l.position)
}

/// The time the player drove, every session together.
fn driven_ms(result: &SessionResult) -> i64 {
    result
        .sessions
        .iter()
        .flat_map(|s| &s.player_laps)
        .filter_map(|l| l.time_ms)
        .map(i64::from)
        .sum()
}

fn head(
    conn: &Connection,
    launch: Option<&Launch>,
    record: Option<&SessionRecord>,
    result: Option<&SessionResult>,
    last_join: Option<i64>,
) -> rusqlite::Result<SessionHead> {
    let car_id = record
        .map(|r| r.car_id.clone())
        .or_else(|| launch.and_then(|l| l.car_id.clone()));
    let track_id = record
        .map(|r| r.track_id.clone())
        .or_else(|| launch.and_then(|l| l.track_id.clone()));
    let name = |id: &Option<String>| -> rusqlite::Result<Option<String>> {
        match id {
            Some(id) => overlay::mod_name(conn, id),
            None => Ok(None),
        }
    };
    let (car_name, car_known) = match record {
        Some(r) => (r.car_name.clone(), r.car_known),
        None => {
            let n = name(&car_id)?;
            (n.clone(), n.is_some())
        }
    };
    let (track_name, track_known) = match record {
        Some(r) => (r.track_name.clone(), r.track_known),
        None => {
            let n = name(&track_id)?;
            (n.clone(), n.is_some())
        }
    };
    let ended_at = record.map(|r| r.ended_at).or_else(|| launch.and_then(|l| l.ended_at));
    let started_at = launch
        .map(|l| l.started_at)
        .or_else(|| record.and_then(|r| r.started_at))
        .or_else(|| ended_at.map(|end| end - result.map(driven_ms).unwrap_or(0)))
        .unwrap_or_default();
    let can_again = match launch.map(|l| l.kind.as_str()) {
        // A mod gone cannot be launched: Edit, and its partial loading, remain
        // (RESULTAT§7.6).
        Some("session") => launch.is_some_and(|l| l.setup.is_some()) && car_known && track_known,
        Some("online") => launch.is_some_and(|l| Some(l.id) == last_join),
        _ => false,
    };
    let can_edit = match launch {
        Some(_) => has_pitbox_block(launch),
        None => record.is_some_and(|r| r.quick_drive.is_some()),
    };
    Ok(SessionHead {
        launch_id: launch.map(|l| l.id),
        session_id: record.map(|r| r.id),
        kind: kind_of(launch),
        mode: match (launch, record) {
            (Some(l), _) => mode_of_launch(l),
            (None, Some(r)) => r.mode.clone(),
            (None, None) => "other".into(),
        },
        started_at,
        ended_at,
        car_id,
        car_name,
        car_known,
        track_id,
        track_name,
        track_known,
        track_layout: record
            .map(|r| r.track_layout.clone())
            .unwrap_or_else(|| launch.and_then(|l| l.track_layout.clone())),
        best_lap_ms: result.and_then(|r| r.player_best_ms()).map(i64::from),
        can_again,
        can_edit,
        server: server_of(launch),
    })
}

fn conditions_of(launch: Option<&Launch>, record: Option<&SessionRecord>) -> Option<Conditions> {
    let num = |v: &Value, k: &str| -> Option<f64> {
        match v.get(k)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    };
    if let Some(setup) = launch
        .and_then(|l| l.setup.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    {
        return Some(Conditions {
            weather: setup.get("weather").and_then(Value::as_str).map(str::to_string),
            time_hours: num(&setup, "time_hours"),
            ambient_c: num(&setup, "ambient_c"),
            road_c: num(&setup, "road_c"),
            track_state: setup
                .get("track_state")
                .and_then(|s| s.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    let qd: Value = serde_json::from_str(record?.quick_drive.as_deref()?).ok()?;
    Some(Conditions {
        weather: qd.get("WeatherId").and_then(Value::as_str).map(str::to_string),
        time_hours: num(&qd, "Time").map(|s| s / 3600.0),
        ambient_c: num(&qd, "Temperature"),
        ..Default::default()
    })
}

fn view(
    conn: &Connection,
    launch: Option<&Launch>,
    record: Option<&SessionRecord>,
    result: Option<SessionResult>,
    last_join: Option<i64>,
) -> rusqlite::Result<ReturnView> {
    let head = head(conn, launch, record, result.as_ref(), last_join)?;
    let previous = match (&head.car_id, &head.track_id) {
        (Some(car), Some(track)) => {
            overlay::personal_best(conn, car, track, head.track_layout.as_deref(), record.map(|r| r.id))?
        }
        _ => None,
    };
    Ok(ReturnView {
        // A first time on the combo is no record to beat: no badge.
        new_best: matches!((head.best_lap_ms, previous), (Some(b), Some(p)) if b < p),
        previous_best_ms: previous,
        conditions: conditions_of(launch, record),
        head,
        result,
    })
}

/// The screen shown when the game closes after a launch (RESULTAT§4).
/// `last_join`: the online launch whose join is still in memory.
pub fn return_view(
    conn: &Connection,
    launch_id: i64,
    result_file: Option<&Path>,
    last_join: Option<i64>,
) -> rusqlite::Result<Option<ReturnView>> {
    let Some(l) = overlay::get_launch(conn, launch_id)? else {
        return Ok(None);
    };
    let record = overlay::session_of_launch(conn, launch_id)?;
    // A session not recorded (no lap begun) still has its line on the screen:
    // the game's file, which is this session's while no other game has run.
    let since = std::time::UNIX_EPOCH + std::time::Duration::from_millis(l.started_at.max(0) as u64);
    let result: Option<SessionResult> = match &record {
        Some(r) => serde_json::from_str(&r.result).ok(),
        None => result_file
            .and_then(|p| crate::raceresult::read_since(p, since).ok())
            .flatten(),
    };
    view(conn, Some(&l), record.as_ref(), result, last_join).map(Some)
}

/// A session of the history, on the same screen (RESULTAT§7.4).
pub fn session_view(
    conn: &Connection,
    session_id: i64,
    last_join: Option<i64>,
) -> rusqlite::Result<Option<ReturnView>> {
    let Some(record) = overlay::get_session(conn, session_id)? else {
        return Ok(None);
    };
    let launch = match record.launch_id {
        Some(id) => overlay::get_launch(conn, id)?,
        None => None,
    };
    let result = serde_json::from_str(&record.result).ok();
    view(conn, launch.as_ref(), Some(&record), result, last_join).map(Some)
}

/// The history, the latest first, or one car on one layout (RESULTAT§7).
pub fn history(
    conn: &Connection,
    combo: Option<(&str, &str, Option<&str>)>,
    last_join: Option<i64>,
) -> rusqlite::Result<Vec<HistoryLine>> {
    let mut out = Vec::new();
    for record in overlay::list_sessions(conn, combo)? {
        let launch = match record.launch_id {
            Some(id) => overlay::get_launch(conn, id)?,
            None => None,
        };
        let result: Option<SessionResult> = serde_json::from_str(&record.result).ok();
        let valid_laps = result
            .iter()
            .flat_map(|r| &r.sessions)
            .flat_map(|s| &s.player_laps)
            .filter(|l| l.cuts == 0 && l.time_ms.is_some())
            .count() as u32;
        let race_position = result.as_ref().and_then(race_position);
        out.push(HistoryLine {
            head: head(conn, launch.as_ref(), Some(&record), result.as_ref(), last_join)?,
            valid_laps,
            race_position,
        });
    }
    Ok(out)
}

/// What "Edit" loads (RESULTAT§8): the session as the screen saved it — the
/// `PitBox` block of the launch's preset — or CM's preset read "at best", with
/// what it could not carry. `None` when there is nothing to load.
pub fn edit_snapshot(conn: &Connection, session_id: i64) -> Result<Option<(Value, Vec<String>)>, String> {
    let record = overlay::get_session(conn, session_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SESSION_NOT_RELAUNCHABLE)?;
    let launch = match record.launch_id {
        Some(id) => overlay::get_launch(conn, id).map_err(|e| e.to_string())?,
        None => None,
    };
    let saved_at = chrono::DateTime::from_timestamp_millis(record.ended_at)
        .map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_default();
    let name = [record.car_name.as_deref(), record.track_name.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    if let Some(preset) = launch
        .as_ref()
        .and_then(|l| l.preset.as_deref())
        .and_then(|p| serde_json::from_str::<Value>(p).ok())
    {
        return Ok(crate::sessionpreset::pitbox_block(&preset).map(|s| (s.clone(), Vec::new())));
    }
    match record.quick_drive.as_deref().map(serde_json::from_str::<Value>) {
        Some(Ok(preset)) => crate::sessionpreset::from_cm(&preset, &name, &saved_at).map(Some),
        _ => Ok(None),
    }
}

/// The result of the session a replay was recorded in (RESULTAT§11).
#[derive(Debug, Clone, Serialize)]
pub struct ReplayResult {
    pub session_id: i64,
    pub race_position: Option<u32>,
    pub best_lap_ms: Option<i64>,
}

pub fn replay_result(
    conn: &Connection,
    car_id: &str,
    track_id: &str,
    at_ms: i64,
) -> rusqlite::Result<Option<ReplayResult>> {
    let Some(record) = overlay::session_at(conn, car_id, track_id, at_ms)? else {
        return Ok(None);
    };
    let result: Option<SessionResult> = serde_json::from_str(&record.result).ok();
    let race_position = result.as_ref().and_then(race_position);

    Ok(Some(ReplayResult {
        session_id: record.id,
        race_position,
        best_lap_ms: record.best_lap_ms,
    }))
}
