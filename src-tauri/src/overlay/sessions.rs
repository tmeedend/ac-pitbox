//! The launch journal, the sessions recorded, and Content Manager's session
//! copies already read (RESULTAT§2, RESULTAT§3).

use super::*;
use rusqlite::OptionalExtension;

/// One line of the launch journal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Launch {
    pub id: i64,
    pub kind: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub car_id: Option<String>,
    pub track_id: Option<String>,
    pub track_layout: Option<String>,
    pub cars: Vec<String>,
    pub setup: Option<String>,
    pub preset: Option<String>,
    pub detail: Option<String>,
}

const LAUNCH_COLUMNS: &str =
    "id, kind, started_at, ended_at, car_id, track_id, track_layout, cars, setup, preset, detail";

fn launch_row(r: &rusqlite::Row) -> rusqlite::Result<Launch> {
    let cars: String = r.get(7)?;
    Ok(Launch {
        id: r.get(0)?,
        kind: r.get(1)?,
        started_at: r.get(2)?,
        ended_at: r.get(3)?,
        car_id: r.get(4)?,
        track_id: r.get(5)?,
        track_layout: r.get(6)?,
        cars: serde_json::from_str(&cars).unwrap_or_default(),
        setup: r.get(8)?,
        preset: r.get(9)?,
        detail: r.get(10)?,
    })
}

/// Notes a launch; returns its id.
pub fn insert_launch(conn: &Connection, l: &Launch) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO launch_log (kind, started_at, car_id, track_id, track_layout, cars, setup, preset, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            l.kind,
            l.started_at,
            l.car_id,
            l.track_id,
            l.track_layout,
            serde_json::to_string(&l.cars).unwrap_or_else(|_| "[]".into()),
            l.setup,
            l.preset,
            l.detail
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_launch(conn: &Connection, id: i64) -> rusqlite::Result<Option<Launch>> {
    conn.query_row(
        &format!("SELECT {LAUNCH_COLUMNS} FROM launch_log WHERE id = ?1"),
        [id],
        launch_row,
    )
    .optional()
}

/// The most recent launch whose game has not been seen closing.
pub fn open_launch(conn: &Connection) -> rusqlite::Result<Option<Launch>> {
    conn.query_row(
        &format!(
            "SELECT {LAUNCH_COLUMNS} FROM launch_log WHERE ended_at IS NULL ORDER BY started_at DESC, id DESC LIMIT 1"
        ),
        [],
        launch_row,
    )
    .optional()
}

/// Closes every launch still open: one game at a time (SESSION§2.4), so a
/// game that closes ends them all — an earlier one left open is a launch
/// whose game never came, or a Pit Box that was closed meanwhile.
pub fn end_open_launches(conn: &Connection, at: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE launch_log SET ended_at = ?1 WHERE ended_at IS NULL AND started_at <= ?1",
        [at],
    )
}

/// The launches whose session may have ended around `at`, the latest first.
pub fn launches_before(conn: &Connection, at: i64, limit: usize) -> rusqlite::Result<Vec<Launch>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {LAUNCH_COLUMNS} FROM launch_log WHERE started_at <= ?1 ORDER BY started_at DESC, id DESC LIMIT ?2"
    ))?;
    let rows = stmt.query_map(params![at, limit as i64], launch_row)?;
    rows.collect()
}

/// A recorded session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionRecord {
    pub id: i64,
    pub launch_id: Option<i64>,
    pub cm_file: Option<String>,
    pub origin: String,
    pub started_at: Option<i64>,
    pub ended_at: i64,
    pub mode: String,
    pub car_id: String,
    pub skin: String,
    pub track_id: String,
    pub track_layout: Option<String>,
    pub best_lap_ms: Option<i64>,
    pub laps: i64,
    pub result: String,
    pub quick_drive: Option<String>,
}

const RECORD_COLUMNS: &str = "id, launch_id, cm_file, origin, started_at, ended_at, mode, car_id, skin, \
     track_id, track_layout, best_lap_ms, laps, result, quick_drive";

fn record_row(r: &rusqlite::Row) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: r.get(0)?,
        launch_id: r.get(1)?,
        cm_file: r.get(2)?,
        origin: r.get(3)?,
        started_at: r.get(4)?,
        ended_at: r.get(5)?,
        mode: r.get(6)?,
        car_id: r.get(7)?,
        skin: r.get(8)?,
        track_id: r.get(9)?,
        track_layout: r.get(10)?,
        best_lap_ms: r.get(11)?,
        laps: r.get(12)?,
        result: r.get(13)?,
        quick_drive: r.get(14)?,
    })
}

pub fn insert_session(conn: &Connection, s: &SessionRecord) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO session_results (launch_id, cm_file, origin, started_at, ended_at, mode, car_id, skin,
             track_id, track_layout, best_lap_ms, laps, result, quick_drive)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            s.launch_id,
            s.cm_file,
            s.origin,
            s.started_at,
            s.ended_at,
            s.mode,
            s.car_id,
            s.skin,
            s.track_id,
            s.track_layout,
            s.best_lap_ms,
            s.laps,
            s.result,
            s.quick_drive
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn session_of_launch(conn: &Connection, launch_id: i64) -> rusqlite::Result<Option<SessionRecord>> {
    conn.query_row(
        &format!("SELECT {RECORD_COLUMNS} FROM session_results WHERE launch_id = ?1"),
        [launch_id],
        record_row,
    )
    .optional()
}

/// Ties CM's copy to the session already recorded from the game's own file.
pub fn attach_cm_file(conn: &Connection, session_id: i64, file: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE session_results SET cm_file = ?2 WHERE id = ?1 AND cm_file IS NULL",
        params![session_id, file],
    )?;
    Ok(())
}

/// The player's best valid lap on a car and a track layout, every session
/// but `except` (RESULTAT§4).
pub fn personal_best(
    conn: &Connection,
    car_id: &str,
    track_id: &str,
    layout: Option<&str>,
    except: Option<i64>,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT MIN(best_lap_ms) FROM session_results
         WHERE car_id = ?1 AND track_id = ?2 AND COALESCE(track_layout, '') = COALESCE(?3, '')
           AND id <> COALESCE(?4, -1)",
        params![car_id, track_id, layout, except],
        |r| r.get(0),
    )
}

/// Whether a copy of CM's was read already.
pub fn cm_file_seen(conn: &Connection, file: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT 1 FROM cm_session_files WHERE file = ?1", [file], |_| Ok(()))
        .optional()
        .map(|r| r.is_some())
}

/// Notes what became of one of CM's copies.
pub fn mark_cm_file(conn: &Connection, file: &str, outcome: &str, at: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO cm_session_files (file, seen_at, outcome) VALUES (?1, ?2, ?3)
         ON CONFLICT(file) DO UPDATE SET outcome = excluded.outcome, seen_at = excluded.seen_at",
        params![file, at, outcome],
    )?;
    Ok(())
}
