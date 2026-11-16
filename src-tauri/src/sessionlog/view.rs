//! What the screens show of a recorded session (RESULTAT§4).

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use super::mode_of_launch;
use crate::overlay;
use crate::raceresult::SessionResult;

/// What the screen shown when the game closes needs (RESULTAT§4).
#[derive(Debug, Clone, Serialize)]
pub struct ReturnView {
    pub launch_id: i64,
    pub kind: String,
    pub mode: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub car_id: Option<String>,
    pub track_id: Option<String>,
    pub track_layout: Option<String>,
    /// `None` when the game wrote nothing for this session.
    pub result: Option<SessionResult>,
    pub session_id: Option<i64>,
    pub best_lap_ms: Option<i64>,
    /// The record before this session, on this car and layout.
    pub previous_best_ms: Option<i64>,
    pub new_best: bool,
    /// Whether "Again" can start this session again from here.
    pub can_again: bool,
}

/// `online_again`: whether the join of this online launch is still at hand —
/// kept in memory only (its password), so only the caller knows.
pub fn return_view(
    conn: &Connection,
    launch_id: i64,
    result_file: Option<&Path>,
    online_again: bool,
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
    let best = result.as_ref().and_then(|r| r.player_best_ms()).map(i64::from);
    let previous = match (&l.car_id, &l.track_id) {
        (Some(car), Some(track)) => overlay::personal_best(
            conn,
            car,
            track,
            l.track_layout.as_deref(),
            record.as_ref().map(|r| r.id),
        )?,
        _ => None,
    };
    let can_again = match l.kind.as_str() {
        "session" => l.setup.is_some(),
        "online" => online_again,
        _ => false,
    };
    Ok(Some(ReturnView {
        mode: mode_of_launch(&l),
        launch_id: l.id,
        kind: l.kind.clone(),
        started_at: l.started_at,
        ended_at: l.ended_at,
        car_id: l.car_id.clone(),
        track_id: l.track_id.clone(),
        track_layout: l.track_layout.clone(),
        session_id: record.as_ref().map(|r| r.id),
        // A first time on the combo is no record to beat: no badge.
        new_best: matches!((best, previous), (Some(b), Some(p)) if b < p),
        best_lap_ms: best,
        previous_best_ms: previous,
        result,
        can_again,
    }))
}
