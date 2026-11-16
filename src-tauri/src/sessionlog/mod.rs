//! The sessions the user drove (`docs/SPEC-resultats.md`, RESULTAT§).
//!
//! Three pieces, each with its own reason to exist:
//!
//! - **The launch journal** (`launch_log`): every game Pit Box starts is
//!   noted at the start, with what it is — a session, the thumbnails, Story
//!   Mode, online, a replay. Nothing in Content Manager's copies says who
//!   started a session (`cmcopy.rs`), so this is what tells them apart.
//! - **The game's own file**, read when the game closes after a launch of
//!   Pit Box: the session is recorded at once, with the setup it was started
//!   with, so it can be started again.
//! - **Content Manager's copies**, read off the window at startup and after
//!   each game: every session, whoever started it. A copy of a Pit Box launch
//!   is tied to that launch, never recorded a second time.
//!
//! Never recorded: thumbnail sessions, Story Mode's (it may drive another
//! version of the player's car, L4.5b, which would make the records lie),
//! and a session where no car began a lap.

pub mod cmcopy;
pub mod thumbs;

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::overlay::{self, Launch, SessionRecord};
use crate::raceresult::SessionResult;

/// What a launch is (RESULTAT§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchKind {
    Session,
    Gameshot,
    Story,
    Online,
    Replay,
}

impl LaunchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Gameshot => "gameshot",
            Self::Story => "story",
            Self::Online => "online",
            Self::Replay => "replay",
        }
    }

    /// Whether a session of this kind is recorded, and shown when the game
    /// closes.
    pub fn is_recorded(kind: &str) -> bool {
        kind == Self::Session.as_str() || kind == Self::Online.as_str()
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// How long after a game closed its session may still be the one of a copy:
/// CM writes it a second or so after the game, a slow disk is given more.
const COPY_DELAY_MS: i64 = 120_000;
/// A launch whose game was never seen closing (Pit Box closed meanwhile)
/// covers at most this long.
const OPEN_LAUNCH_SPAN_MS: i64 = 12 * 3_600_000;
/// How long before the game starts its launch may have been sent: CM takes
/// 0.8 to 4.1 s (`gameSession.svelte.ts`), a CM that downloads CSP content
/// much longer.
const LAUNCH_LEAD_MS: i64 = 180_000;

/// Notes a launch in the journal. Best-effort: a journal that cannot be
/// written must not stop a session that has already left — logged.
pub fn note_launch(conn: &Connection, launch: Launch) -> Option<i64> {
    match overlay::insert_launch(conn, &launch) {
        Ok(id) => Some(id),
        Err(e) => {
            log::warn!("results: launch of {} not noted in the journal ({e})", launch.kind);
            None
        }
    }
}

/// The journal line of a session started from the session screen (or from
/// Story Mode): the setup exactly as it was sent, and the preset with its
/// `PitBox` block — the one `quickdrive::preset_value` built for it.
pub fn session_launch(
    kind: LaunchKind,
    started_at: i64,
    setup_json: &serde_json::Value,
    setup: &crate::launch::RaceSetup,
    snapshot: Option<&serde_json::Value>,
) -> Launch {
    let preset = snapshot
        .and_then(|s| crate::sessionpreset::preset_with_block(setup, s).ok())
        .unwrap_or_else(|| crate::quickdrive::preset_value(setup));
    Launch {
        kind: kind.as_str().into(),
        started_at,
        car_id: Some(setup.car_id.clone()),
        track_id: Some(setup.track_id.clone()),
        track_layout: setup.track_layout.clone().filter(|l| !l.is_empty()),
        cars: std::iter::once(setup.car_id.clone())
            .chain(setup.opponents.iter().map(|o| o.car_id.clone()))
            .collect(),
        setup: Some(setup_json.to_string()),
        preset: Some(preset.to_string()),
        ..Default::default()
    }
}

/// The mode of a session started by Pit Box, in the words of
/// `session_results.mode`.
fn mode_of_setup(setup: &crate::launch::RaceSetup) -> &'static str {
    use crate::launch::SessionType;
    match setup.session_type {
        SessionType::Practice => "practice",
        SessionType::Hotlap => "hotlap",
        SessionType::Race if setup.qualify_enabled => "weekend",
        SessionType::Race => "race",
        SessionType::TrackDay => "trackday",
    }
}

fn mode_of_launch(l: &Launch) -> String {
    if l.kind == LaunchKind::Online.as_str() {
        return "online".into();
    }
    l.setup
        .as_deref()
        .and_then(|s| serde_json::from_str::<crate::launch::RaceSetup>(s).ok())
        .map(|s| mode_of_setup(&s).to_string())
        .unwrap_or_else(|| "other".into())
}

fn record_of(
    result: &SessionResult,
    origin: &str,
    launch: Option<&Launch>,
    ended_at: i64,
    track: (&str, Option<&str>),
    mode: &str,
) -> SessionRecord {
    let player = result.player();
    SessionRecord {
        launch_id: launch.map(|l| l.id),
        origin: origin.into(),
        started_at: launch.map(|l| l.started_at),
        ended_at,
        mode: mode.into(),
        car_id: player.map(|p| p.car_id.clone()).unwrap_or_default(),
        skin: player.map(|p| p.skin.clone()).unwrap_or_default(),
        track_id: track.0.into(),
        track_layout: track.1.map(str::to_string),
        best_lap_ms: result.player_best_ms().map(i64::from),
        laps: i64::from(result.player_laps()),
        result: serde_json::to_string(result).unwrap_or_default(),
        ..Default::default()
    }
}

/// What the game's closing did to the journal.
#[derive(Debug, Clone, PartialEq)]
pub struct Closed {
    pub launch: Launch,
    /// The session recorded from the game's file, if it was one to record.
    pub session_id: Option<i64>,
}

/// The game has closed: the open launch ends now, and when it was a session
/// of the user's, the game's file is read and the session recorded
/// (RESULTAT§3.1). `None` when no launch of Pit Box was open.
///
/// `game_started_at` is when the game was seen starting: the open launch is
/// this game's only if it came just before. A launch whose game never came (CM
/// refused it) must not take the next session the user starts from CM.
pub fn game_closed(
    conn: &Connection,
    result_file: Option<&Path>,
    now: i64,
    game_started_at: Option<i64>,
) -> rusqlite::Result<Option<Closed>> {
    let Some(mut launch) = overlay::open_launch(conn)? else {
        return Ok(None);
    };
    overlay::end_open_launches(conn, now)?;
    if game_started_at.is_some_and(|start| launch.started_at < start - LAUNCH_LEAD_MS) {
        log::info!("results: launch {} had no game of its own, closed", launch.id);
        return Ok(None);
    }
    launch.ended_at = Some(now);
    if !LaunchKind::is_recorded(&launch.kind) {
        return Ok(Some(Closed {
            launch,
            session_id: None,
        }));
    }
    let since = std::time::UNIX_EPOCH + std::time::Duration::from_millis(launch.started_at.max(0) as u64);
    let result = match result_file.map(|p| crate::raceresult::read_since(p, since)) {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            log::warn!("results: the game's result file unreadable ({e})");
            None
        }
        None => None,
    };
    let session_id = match result {
        Some(result) => record_launch_result(conn, &launch, &result, now)?,
        None => None,
    };
    Ok(Some(Closed { launch, session_id }))
}

/// Records the session of a Pit Box launch, once: a second reading (the
/// game's file, then CM's copy, or the other way round) finds it there.
fn record_launch_result(
    conn: &Connection,
    launch: &Launch,
    result: &SessionResult,
    ended_at: i64,
) -> rusqlite::Result<Option<i64>> {
    if let Some(existing) = overlay::session_of_launch(conn, launch.id)? {
        return Ok(Some(existing.id));
    }
    if !result.any_lap_begun() {
        return Ok(None);
    }
    // Not the session that was sent: left to CM's copy, recorded as CM's.
    let car = result.player().map(|p| p.car_id.as_str()).unwrap_or_default();
    if launch.car_id.as_deref().is_some_and(|c| !c.is_empty() && c != car) {
        log::warn!("results: the game drove {car}, not the car of launch {}", launch.id);
        return Ok(None);
    }
    let track = launch.track_id.clone().unwrap_or_else(|| result.track.clone());
    let record = record_of(
        result,
        "pitbox",
        Some(launch),
        ended_at,
        (&track, launch.track_layout.as_deref()),
        &mode_of_launch(launch),
    );
    overlay::insert_session(conn, &record).map(Some)
}

/// The launch of the journal a copy of CM's belongs to, if any: the copy was
/// written while that launch's game ran or shortly after (RESULTAT§3.3),
/// then on the same track with the player in the same car.
fn launch_of_copy(conn: &Connection, copy: &cmcopy::CmCopy) -> rusqlite::Result<Option<Launch>> {
    let candidates = overlay::launches_before(conn, copy.at_ms, 4)?;
    for (i, l) in candidates.iter().enumerate() {
        // The next launch after this one, which bounds a launch never closed.
        let next_start = i.checked_sub(1).map(|j| candidates[j].started_at);
        let end = l
            .ended_at
            .unwrap_or_else(|| next_start.unwrap_or(l.started_at + OPEN_LAUNCH_SPAN_MS));
        if copy.at_ms > end + COPY_DELAY_MS {
            // Older launches end earlier still.
            break;
        }
        if belongs(l, copy) {
            return Ok(Some(l.clone()));
        }
    }
    Ok(None)
}

fn belongs(l: &Launch, copy: &cmcopy::CmCopy) -> bool {
    if l.kind == LaunchKind::Gameshot.as_str() {
        return thumbs::looks_like_thumbnail(copy) && l.cars.iter().any(|c| c == copy.player_car());
    }
    let same_track = l
        .track_id
        .as_deref()
        .is_none_or(|t| t.eq_ignore_ascii_case(&copy.track_id));
    // A story may drive its own version of the player's car (L4.5b of Story
    // Mode), under another id: its window and track are enough.
    if l.kind == LaunchKind::Story.as_str() {
        return same_track;
    }
    let same_car = l
        .car_id
        .as_deref()
        .is_none_or(|c| c.is_empty() || c == copy.player_car());
    same_track && same_car
}

/// What reading one copy did, as `cm_session_files.outcome` keeps it.
pub fn take_copy(conn: &Connection, copy: &cmcopy::CmCopy) -> rusqlite::Result<&'static str> {
    let launch = launch_of_copy(conn, copy)?;
    if let Some(l) = &launch {
        match l.kind.as_str() {
            "gameshot" => return Ok("thumbnail"),
            "story" => return Ok("story"),
            "replay" => return Ok("replay"),
            _ => {}
        }
    }
    if !copy.result.any_lap_begun() {
        return Ok("no_lap");
    }
    match launch {
        Some(l) => match overlay::session_of_launch(conn, l.id)? {
            Some(existing) => {
                overlay::attach_cm_file(conn, existing.id, &copy.file)?;
                Ok("attached")
            }
            None => {
                let ended = l.ended_at.unwrap_or(copy.at_ms);
                let mut record = record_of(
                    &copy.result,
                    "pitbox",
                    Some(&l),
                    ended,
                    (&copy.track_id, copy.track_layout.as_deref()),
                    &mode_of_launch(&l),
                );
                record.cm_file = Some(copy.file.clone());
                overlay::insert_session(conn, &record)?;
                Ok("recorded")
            }
        },
        None => {
            let mut record = record_of(
                &copy.result,
                "cm",
                None,
                copy.at_ms,
                (&copy.track_id, copy.track_layout.as_deref()),
                &copy.mode,
            );
            record.cm_file = Some(copy.file.clone());
            record.quick_drive = copy.quick_drive.clone();
            overlay::insert_session(conn, &record)?;
            Ok("recorded")
        }
    }
}

/// Runs a closure with the base locked.
pub type WithBase<'a> = &'a dyn Fn(&mut dyn FnMut(&Connection));

/// Reads the copies of CM's folder not read yet (RESULTAT§3.2). The file
/// work happens without the base: `lock` is taken per copy, for its few
/// writes only. Returns how many were read.
pub fn scan(dir: &Path, lock: WithBase) -> usize {
    let mut fresh = Vec::new();
    lock(&mut |conn| {
        for (file, modified) in cmcopy::list(dir) {
            match overlay::cm_file_seen(conn, &file) {
                Ok(false) => fresh.push((file, modified)),
                Ok(true) => {}
                Err(e) => log::warn!("results: {file} not checked ({e})"),
            }
        }
    });
    let count = fresh.len();
    for (file, modified) in fresh {
        let read = std::fs::read(dir.join(&file))
            .map_err(|e| e.to_string())
            .and_then(|bytes| cmcopy::parse(&file, &bytes, modified));
        lock(&mut |conn| {
            let outcome = match &read {
                Ok(copy) => take_copy(conn, copy).unwrap_or_else(|e| {
                    log::warn!("results: {file} not recorded ({e})");
                    "error"
                }),
                Err(e) => {
                    log::warn!("results: CM's copy {file} unreadable ({e})");
                    "unreadable"
                }
            };
            // A copy that failed on the base is read again next time.
            if outcome != "error" {
                if let Err(e) = overlay::mark_cm_file(conn, &file, outcome, now_ms()) {
                    log::warn!("results: {file} not marked as read ({e})");
                }
            }
        });
    }
    count
}

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

#[cfg(test)]
mod tests;
