//! Content Manager's copy of a session (RESULTAT§3.2):
//! `%LOCALAPPDATA%\AcTools Content Manager\Progress\Sessions\<yymmdd-hhmmss>.json`.
//!
//! Read from the real files of the dev machine (105 of them, August to
//! November 2026), never guessed. What they showed:
//!
//! - **The file is the game's `race_out.json`, plus two keys of CM's own**:
//!   `__raceIni` (the `race.ini` the game was started with) and `__quickDrive`
//!   (the Quick Drive preset, as a JSON string). The name is the time the copy
//!   was written, a second or so after the game wrote its own file.
//! - **`__quickDrive` is CM's own rewrite of the preset**, not the file Pit
//!   Box handed it: its keys are CM's, and the `PitBox` block of a Pit Box
//!   launch is gone. So nothing in a copy says who started the session — the
//!   launch journal does (RESULTAT§2).
//! - **An online session has no `__quickDrive`**, and its `race.ini` says
//!   `[REMOTE] ACTIVE=1`. Car 0 is still the player.
//! - **The track of the session is in `race.ini`** (`[RACE] TRACK` and
//!   `CONFIG_TRACK`): the top-level `track` joins the two with a dash, which a
//!   track id may contain too.

use std::path::Path;

use serde_json::Value;

use crate::raceresult::SessionResult;

/// One copy, read.
#[derive(Debug, Clone)]
pub struct CmCopy {
    /// The file name: its identity in the folder.
    pub file: String,
    /// When CM wrote it, in ms since the epoch: from the name, else the file.
    pub at_ms: i64,
    pub result: SessionResult,
    pub track_id: String,
    pub track_layout: Option<String>,
    /// `practice` | `hotlap` | `race` | `weekend` | `trackday` | `online` | `other`.
    pub mode: String,
    /// CM's Quick Drive preset, as CM wrote it; `None` online.
    pub quick_drive: Option<String>,
}

impl CmCopy {
    pub fn player_car(&self) -> &str {
        self.result.player().map(|p| p.car_id.as_str()).unwrap_or_default()
    }
}

/// `yymmdd-hhmmss`, local time, in ms since the epoch.
fn time_of_name(stem: &str) -> Option<i64> {
    use chrono::TimeZone;
    let naive = chrono::NaiveDateTime::parse_from_str(stem, "%y%m%d-%H%M%S").ok()?;
    chrono::Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|t| t.timestamp_millis())
}

/// A value of a `race.ini` section, as CM copies it into `__raceIni`.
pub fn ini_value<'a>(ini: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in ini.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            inside = name.eq_ignore_ascii_case(section);
        } else if inside {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim());
                }
            }
        }
    }
    None
}

/// The Quick Drive mode a preset names, in the words of `session_results.mode`.
pub fn mode_of_quick_drive(quick_drive: &str) -> &'static str {
    let mode = serde_json::from_str::<Value>(quick_drive)
        .ok()
        .and_then(|v| v.get("Mode").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    match mode.rsplit('/').next().unwrap_or_default() {
        "QuickDrive_Practice.xaml" => "practice",
        "QuickDrive_Hotlap.xaml" => "hotlap",
        "QuickDrive_Race.xaml" => "race",
        "QuickDrive_Weekend.xaml" => "weekend",
        "QuickDrive_Trackday.xaml" => "trackday",
        _ => "other",
    }
}

/// Reads one copy. `modified_ms` stands in for a name that is not CM's.
pub fn parse(file: &str, bytes: &[u8], modified_ms: i64) -> Result<CmCopy, String> {
    let result = crate::raceresult::parse(bytes)?;
    let raw: Value = serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
        .or_else(|_| serde_json::from_str(&String::from_utf8_lossy(bytes)))
        .map_err(|e| format!("{file}: {e}"))?;
    let race_ini = raw.get("__raceIni").and_then(Value::as_str).unwrap_or_default();
    let quick_drive = raw.get("__quickDrive").and_then(Value::as_str).map(str::to_string);

    let (track_id, track_layout) = match ini_value(race_ini, "RACE", "TRACK") {
        Some(track) if !track.is_empty() => (
            track.to_string(),
            ini_value(race_ini, "RACE", "CONFIG_TRACK")
                .filter(|l| !l.is_empty())
                .map(str::to_string),
        ),
        // No race.ini: the game's own field, read as a track without layout.
        _ => (result.track.clone(), None),
    };
    let online = ini_value(race_ini, "REMOTE", "ACTIVE") == Some("1");
    let mode = if online {
        "online"
    } else {
        quick_drive.as_deref().map(mode_of_quick_drive).unwrap_or("other")
    };
    let stem = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    Ok(CmCopy {
        file: file.to_string(),
        at_ms: time_of_name(stem).unwrap_or(modified_ms),
        result,
        track_id,
        track_layout,
        mode: mode.to_string(),
        quick_drive,
    })
}

/// `%LOCALAPPDATA%\AcTools Content Manager\Progress\Sessions`, when CM ran
/// on this machine.
pub fn sessions_dir() -> Option<std::path::PathBuf> {
    let dir = dirs::data_local_dir()?
        .join("AcTools Content Manager")
        .join("Progress")
        .join("Sessions");
    dir.is_dir().then_some(dir)
}

/// The copies of a folder, as `(file name, modified ms)`, oldest first.
pub fn list(dir: &Path) -> Vec<(String, i64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, i64)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".json") {
                return None;
            }
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp_millis())
                .unwrap_or(0);
            Some((name, modified))
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample(name: &str) -> CmCopy {
        let (file, bytes): (&str, &[u8]) = match name {
            "trackday" => (
                "261115-210327.json",
                include_bytes!("../raceresult/samples/cm_thumbnail_trackday.json"),
            ),
            "shot" => (
                "261116-092303.json",
                include_bytes!("../raceresult/samples/cm_thumbnail_practice.json"),
            ),
            "laps" => (
                "260912-184107.json",
                include_bytes!("../raceresult/samples/cm_practice_laps.json"),
            ),
            "online" => (
                "260821-224425.json",
                include_bytes!("../raceresult/samples/cm_online.json"),
            ),
            _ => unreachable!(),
        };
        parse(file, bytes, 0).expect("real copy parses")
    }

    // RESULTAT§3.2: the track and its layout come from race.ini, not from the
    // dashed top-level field.
    #[test]
    fn the_track_and_layout_come_from_race_ini() {
        let c = sample("online");
        assert_eq!(
            (c.track_id.as_str(), c.track_layout.as_deref()),
            ("pk_akina", Some("akina_downhill"))
        );
        assert_eq!(c.result.track, "pk_akina-akina_downhill", "the field joins them");
        let p = sample("laps");
        assert_eq!((p.track_id.as_str(), p.track_layout), ("monza", None));
    }

    // RESULTAT§3.2: an online copy has no Quick Drive preset; its race.ini says
    // REMOTE, and car 0 is still the player.
    #[test]
    fn an_online_copy_is_told_by_its_race_ini() {
        let c = sample("online");
        assert_eq!(c.mode, "online");
        assert!(c.quick_drive.is_none(), "no __quickDrive online");
        assert_eq!(c.player_car(), "ks_toyota_ae86_drift");
        assert_eq!(c.result.player().unwrap().driver_name, "Player");
    }

    // The time of a copy is its name's; a name that is not CM's falls back on
    // the file's time.
    #[test]
    fn the_time_of_a_copy_is_read_in_its_name() {
        let c = sample("laps");
        let local = chrono::DateTime::from_timestamp_millis(c.at_ms)
            .unwrap()
            .with_timezone(&chrono::Local);
        assert_eq!(local.format("%y%m%d-%H%M%S").to_string(), "260912-184107");
        let other = parse(
            "renamed.json",
            include_bytes!("../raceresult/samples/cm_practice_laps.json"),
            42,
        )
        .unwrap();
        assert_eq!(other.at_ms, 42, "no time in the name");
    }

    // The Quick Drive mode names the session kind; a thumbnail session is a
    // practice (one car) or a track day (several).
    #[test]
    fn the_mode_comes_from_the_quick_drive_preset() {
        assert_eq!(sample("trackday").mode, "trackday");
        assert_eq!(sample("shot").mode, "practice");
        assert_eq!(sample("laps").mode, "practice");
    }

    // The player's laps of a real practice: four driven, the best is the
    // game's own.
    #[test]
    fn a_practice_copy_carries_the_player_laps() {
        let c = sample("laps");
        assert!(c.result.any_lap_begun());
        assert_eq!(c.result.player_best_ms(), Some(161_626));
        assert_eq!(c.result.sessions[0].player_laps.len(), 4);
        assert!(
            !sample("trackday").result.any_lap_begun(),
            "a thumbnail session: no lap at all"
        );
    }
}
