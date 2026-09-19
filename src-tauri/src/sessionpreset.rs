//! Saved sessions, stored as Content Manager `.cmpreset` files (SESSION§3.6).
//!
//! A saved session used to be an entry in `saved_sessions.json`, readable by
//! Pit Box alone. It is now a real Quick Drive preset, written where Content
//! Manager keeps its own — `%LocalAppData%\AcTools Content Manager\Presets\
//! Quick Drive\Pit Box\` — so the same session can be started from either
//! application. The preset is built by [`crate::quickdrive::preset_value`], the
//! very function the launcher uses: what is saved and what is launched cannot
//! drift apart, because there is one builder.
//!
//! ## The `PitBox` block, and why a round trip is not enough
//!
//! The Quick Drive schema cannot hold everything a Pit Box session carries.
//! The player's skin has no field at all (measured, not assumed — see
//! SESSION§2), and neither have the weather intent, the active track skins,
//! the opponent pool filters or the driver outfit. Re-reading a preset would
//! therefore lose exactly what the user spent time on.
//!
//! So our own files carry an extra top-level `PitBox` key holding the whole
//! frontend snapshot, and reading one of our presets uses that block alone —
//! no lossy round trip. Content Manager ignores keys it does not know
//! (Newtonsoft's default), so the file stays a perfectly ordinary preset for
//! it. Should CM ever rewrite one of our files and drop the block, nothing
//! breaks: the file then reads like any other CM preset, through [`from_cm`].
//!
//! ## Reading Content Manager's own presets
//!
//! Every `.cmpreset` under `Quick Drive\` is listed, not just ours (SESSION§3.6). The
//! ones CM wrote are converted best-effort and marked as such; the three modes
//! Pit Box has no equivalent for (Drag, Drift, Time attack) are **named** in
//! the list rather than hidden, the same rule the grid import already follows.
//!
//! ## What is never done here
//!
//! A preset we did not write is never deleted and never overwritten — the
//! corollary of the game-files rule, applied to CM's folder. `delete` refuses
//! anything without a `PitBox` block, and saving always writes inside our own
//! subfolder.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::cmimport::{cell_number, cell_text, convert as convert_grid};
use crate::launch::RaceSetup;
use crate::quickdrive;

/// Our block inside an otherwise ordinary Quick Drive preset.
const PITBOX_KEY: &str = "PitBox";

/// Schema of that block. Bumped only when an older Pit Box could no longer
/// read it — a reader that finds a higher version falls back to reading the
/// preset as a plain CM one, which always works.
const PITBOX_VERSION: u64 = 1;

/// Where our own presets go inside `Quick Drive\`. A subfolder rather than the
/// root because CM users organise that folder by hand, and dropping dozens of
/// files in the middle of theirs would be rude.
const SUBFOLDER: &str = "Pit Box";

/// One entry of the saved-session list.
#[derive(Debug, Clone, Serialize)]
pub struct SessionPresetEntry {
    /// Absolute path — the identity of the entry. Two presets can share a name
    /// in two of CM's subfolders, so the name cannot be the key.
    pub path: String,
    /// What the list shows: the file name without its extension, which is also
    /// what CM shows.
    pub name: String,
    /// `"pitbox"` (written here, carries its block) or `"cm"`.
    pub origin: &'static str,
    /// Path relative to `Quick Drive\`, to tell apart two presets of the same
    /// name filed in different folders.
    pub source: String,
    /// The reloadable snapshot, shaped exactly like the frontend's
    /// `SavedSession`. `None` when the file could not be converted.
    pub session: Option<Value>,
    /// i18n key explaining a `None` session.
    pub reason: Option<String>,
    /// i18n keys for what the conversion could not carry — shown in the yellow
    /// banner after loading, never silently dropped.
    pub notes: Vec<String>,
}

/// `<LocalAppData>\AcTools Content Manager\Presets\Quick Drive`, or `None`
/// when CM has never run on this machine.
fn cm_quick_drive() -> Option<PathBuf> {
    crate::cmimport::presets_dir().map(|p| p.join("Quick Drive"))
}

/// The folder saved sessions are read from and written to.
///
/// CM's own folder when CM is installed — that is the whole point of the
/// format. Without CM, a folder of ours, so that saving a session keeps
/// working on a machine that only has the game.
pub fn root(app: &AppHandle) -> Option<PathBuf> {
    cm_quick_drive().or_else(|| app.path().app_config_dir().ok().map(|d| d.join("Quick Drive")))
}

/// Our own subfolder. `pub` because the startup backup copies it: saved
/// sessions no longer live in `app_config_dir`, and the safety net has to
/// follow them (see `backup.rs`).
pub fn own_dir(app: &AppHandle) -> Option<PathBuf> {
    root(app).map(|r| r.join(SUBFOLDER))
}

/// A file name Windows accepts, out of a name the user typed freely.
///
/// Reserved characters become `_` rather than being dropped: `GT3 / Spa` and
/// `GT3 Spa` must not collapse onto the same file. Trailing dots and spaces go
/// — Windows silently strips them, which would make the file we wrote
/// unfindable under the name we computed.
fn sanitize(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    while out.ends_with(' ') || out.ends_with('.') {
        out.pop();
    }
    let trimmed = out.trim_start().to_string();
    if trimmed.is_empty() {
        "session".to_string()
    } else {
        trimmed
    }
}

/// Writes a session as a Quick Drive preset and returns its path.
///
/// `snapshot` is opaque here on purpose: its schema belongs to the frontend
/// (`savedSessions.ts`), exactly as it did when the same object lived in
/// `saved_sessions.json`. Rust only decides where the file goes and what a
/// preset looks like around it.
///
/// Synchronous write (`std::fs::write`): the command returns once the bytes
/// are on disk, which is the whole reason saved sessions left `localStorage`.
pub fn save(app: &AppHandle, name: &str, setup: &RaceSetup, snapshot: &Value) -> Result<String, String> {
    let dir = own_dir(app).ok_or("dossier de presets indisponible")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let preset = preset_with_block(setup, snapshot)?;
    let path = dir.join(format!("{}.cmpreset", sanitize(name)));
    let text = serde_json::to_string(&preset).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("écriture du preset {} échouée : {e}", path.display()))?;
    Ok(path.display().to_string())
}

/// The file we write: an ordinary Quick Drive preset, plus our block.
///
/// Separate from [`save`] so the shape can be tested without a Tauri handle —
/// and because it states the invariant in one place: the preset comes from the
/// launcher's own builder, and we only add a key to it. Nothing of CM's is
/// rewritten or reordered.
fn preset_with_block(setup: &RaceSetup, snapshot: &Value) -> Result<Value, String> {
    let mut preset = quickdrive::preset_value(setup);
    let block = json!({ "version": PITBOX_VERSION, "session": snapshot });
    match preset.as_object_mut() {
        Some(map) => map.insert(PITBOX_KEY.to_string(), block),
        // `preset_value` builds an object literal; this arm cannot happen, and
        // is a failed write rather than a panic if it ever did.
        None => return Err("preset Quick Drive inattendu".into()),
    };
    Ok(preset)
}

/// Deletes one of **our** presets.
///
/// Three guards, and none of them is decoration: the file must sit under the
/// preset root, end in `.cmpreset`, and carry a `PitBox` block. A preset the
/// user composed in Content Manager is listed by Pit Box but never removed by
/// it — same principle as the game files we did not put there.
pub fn delete(app: &AppHandle, path: &str) -> Result<(), String> {
    let target = PathBuf::from(path);
    let root = root(app).ok_or("dossier de presets indisponible")?;
    if !is_inside(&root, &target) {
        return Err(crate::errors::PRESET_NOT_OURS.into());
    }
    if !target.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmpreset")) {
        return Err(crate::errors::PRESET_NOT_OURS.into());
    }
    if !read_preset(&target).is_some_and(|v| pitbox_block(&v).is_some()) {
        return Err(crate::errors::PRESET_NOT_OURS.into());
    }
    std::fs::remove_file(&target).map_err(|e| format!("suppression du preset échouée : {e}"))
}

/// Containment test on canonical paths — a relative path or a `..` must not
/// walk out of the preset folder. Falls back to the raw comparison when the
/// file cannot be canonicalised (it may simply not exist).
fn is_inside(root: &Path, target: &Path) -> bool {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let target = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    target.starts_with(&root)
}

fn read_preset(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Our snapshot out of a preset, if it has one we can read.
fn pitbox_block(preset: &Value) -> Option<&Value> {
    let block = preset.get(PITBOX_KEY)?;
    let version = block.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version == 0 || version > PITBOX_VERSION {
        return None;
    }
    block.get("session")
}

/// Every preset under `Quick Drive\`, ours first.
///
/// Never an error: no CM, no folder, or a corrupt file are all non-results
/// rather than failures — the same rule the grid import follows. A file that
/// cannot be read is listed with its reason instead of being dropped.
pub fn list(app: &AppHandle) -> Vec<SessionPresetEntry> {
    let Some(root) = root(app) else {
        return Vec::new();
    };
    migrate_legacy_file(app);
    let mut out = Vec::new();
    for path in cmpresets(&root) {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".into());
        let source = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
        let path_text = path.display().to_string();
        let Some(preset) = read_preset(&path) else {
            out.push(SessionPresetEntry {
                path: path_text,
                name,
                origin: "cm",
                source,
                session: None,
                reason: Some(crate::errors::CM_UNREADABLE.into()),
                notes: Vec::new(),
            });
            continue;
        };
        if let Some(session) = pitbox_block(&preset) {
            out.push(SessionPresetEntry {
                path: path_text,
                name,
                origin: "pitbox",
                source,
                session: Some(session.clone()),
                reason: None,
                notes: Vec::new(),
            });
            continue;
        }
        let saved_at = file_time(&path);
        match from_cm(&preset, &name, &saved_at) {
            Ok((session, notes)) => out.push(SessionPresetEntry {
                path: path_text,
                name,
                origin: "cm",
                source,
                session: Some(session),
                reason: None,
                notes,
            }),
            Err(reason) => out.push(SessionPresetEntry {
                path: path_text,
                name,
                origin: "cm",
                source,
                session: None,
                reason: Some(reason),
                notes: Vec::new(),
            }),
        }
    }
    out
}

/// All `.cmpreset` files under a folder, recursively — CM files its presets in
/// a tree, so a flat read would miss an organised user's presets.
fn cmpresets(root: &Path) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmpreset")))
        .collect()
}

/// A CM preset has no saved-at field, so its modification time stands in —
/// which is what the user sees in Explorer anyway. UTC ISO 8601, like the
/// dates the frontend writes, because the list sorts on that string.
fn file_time(path: &Path) -> String {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    match modified {
        Some(t) => chrono::DateTime::<chrono::Utc>::from(t)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
        None => String::new(),
    }
}

/// The five Quick Drive modes Pit Box knows, and the three it does not.
///
/// Drag, Drift and Time attack have no Pit Box equivalent — they are not
/// variations of a race but other games entirely, with their own rules. They
/// are named in the list rather than dropped: someone who has ten presets and
/// sees eight is left wondering which two vanished.
fn mode_of(preset: &Value) -> Result<(&'static str, bool), String> {
    let mode = preset.get("Mode").and_then(Value::as_str).unwrap_or_default();
    let file = mode.rsplit('/').next().unwrap_or(mode);
    match file {
        "QuickDrive_Practice.xaml" => Ok(("practice", false)),
        "QuickDrive_Hotlap.xaml" => Ok(("hotlap", false)),
        "QuickDrive_Weekend.xaml" => Ok(("race", true)),
        "QuickDrive_Race.xaml" => Ok(("race", false)),
        "QuickDrive_Trackday.xaml" => Ok(("trackday", false)),
        "" => Err(crate::errors::CM_UNREADABLE.into()),
        _ => Err(crate::errors::CM_UNSUPPORTED_MODE.into()),
    }
}

/// Reads the `ModeData` of a preset — JSON inside a string, as CM writes it.
fn mode_data(preset: &Value) -> Value {
    preset
        .get("ModeData")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}))
}

/// Same trick one level down, for `TrackPropertiesData` / `AssistsData`.
fn nested(preset: &Value, key: &str) -> Value {
    preset
        .get(key)
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}))
}

fn number(v: &Value, key: &str) -> Option<f64> {
    match v.get(key)? {
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn flag(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

/// `0,1,2` back to the three assist levels — the game's own scale, see
/// `AssistLevel`.
fn assist_level(v: &Value, key: &str) -> &'static str {
    match number(v, key).unwrap_or(1.0).round() as i64 {
        0 => "off",
        2 => "on",
        _ => "factory",
    }
}

/// The track state a preset describes, as `(grip, track_state)`.
///
/// The name never travels in a preset (SESSION§2), so it is recovered by
/// **matching the four numbers** against the game's table: an exact match is
/// that state, by name, which is what the screen will show. Anything else is a
/// user preset of CM's, kept with its own numbers and named after the preset
/// file CM recorded next to it — the screen then shows a `cm` state, which is
/// exactly what it is.
fn track_state_of(preset: &Value) -> (u32, Value) {
    let props = nested(preset, "TrackPropertiesData");
    let weather_defined = flag(&props, "w").unwrap_or(false);
    let start = (number(&props, "s").unwrap_or(1.0) * 100.0).round() as u32;
    let transfer = (number(&props, "t").unwrap_or(1.0) * 100.0).round() as u32;
    let randomness = (number(&props, "r").unwrap_or(0.0) * 100.0).round() as u32;
    let lap_gain = number(&props, "g").unwrap_or(1.0).round() as u32;
    let description = props.get("d").and_then(Value::as_str).unwrap_or_default().to_string();

    if weather_defined {
        let auto = quickdrive::weather_state_option();
        return (
            crate::quickdrive::GRIP_WEATHER,
            json!({
                "origin": auto.origin, "name": auto.name, "start": auto.start, "transfer": auto.transfer,
                "randomness": auto.randomness, "lap_gain": auto.lap_gain, "weather_defined": true,
                "description": auto.description,
            }),
        );
    }
    let builtin = quickdrive::TRACK_STATES
        .iter()
        .find(|s| s.start == start && s.transfer == transfer && s.randomness == randomness && s.lap_gain == lap_gain);
    let (origin, name, description) = match builtin {
        Some(s) => ("builtin", s.name.to_string(), s.description.to_string()),
        None => (
            "cm",
            preset
                .get("TrackPropertiesPresetFilename")
                .and_then(Value::as_str)
                .and_then(|p| Path::new(p).file_stem().map(|s| s.to_string_lossy().to_string()))
                .unwrap_or_else(|| description.clone()),
            description,
        ),
    };
    (
        start,
        json!({
            "origin": origin, "name": name, "start": start, "transfer": transfer,
            "randomness": randomness, "lap_gain": lap_gain, "weather_defined": false,
            "description": description,
        }),
    )
}

/// A centre-and-spread pair out of the two bounds a preset carries
/// (SETUP§2.9). Integer halves: a 85–95 range is 90 ± 5, and an odd width
/// rounds the spread down rather than widening a range the user set.
fn center_spread(min: f64, max: f64) -> (u32, u32) {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    (((lo + hi) / 2.0).round() as u32, ((hi - lo) / 2.0).floor() as u32)
}

/// Converts a Content Manager preset into a frontend `SavedSession`.
///
/// **Best effort, and it says what it lost.** Everything the Quick Drive
/// schema carries is read; what it does not carry (the player's skin above
/// all) comes back as a note rather than as a silently wrong value. The
/// session is still loadable — a preset with no player skin simply keeps the
/// skin currently selected, which is what CM itself does.
fn from_cm(preset: &Value, name: &str, saved_at: &str) -> Result<(Value, Vec<String>), String> {
    let (session_type, qualify_enabled) = mode_of(preset)?;
    let mut notes = vec![crate::errors::CM_NOTE_NO_PLAYER_SKIN.to_string()];
    let data = mode_data(preset);
    let assists = nested(preset, "AssistsData");

    // Track: `<track>/<layout>`, the same convention `track_id` writes.
    let track = preset.get("TrackId").and_then(Value::as_str).unwrap_or_default();
    let (track_id, track_layout) = match track.split_once('/') {
        Some((t, l)) => (t.to_string(), Value::from(l)),
        None => (track.to_string(), Value::Null),
    };

    // The grid, two layers of JSON-in-a-string down. Only a manual grid is a
    // line-up (SESSION§3.5); a draw mode leaves the field empty and says so, rather
    // than inventing opponents nobody arranged.
    let raw_grid = crate::cmimport::race_grid_of(preset);
    let grid = raw_grid
        .as_ref()
        .and_then(|g| convert_grid(g, name, "").map(|c| (g, c)));
    let mut opponents: Vec<Value> = Vec::new();
    let (mut ai_level, mut ai_spread) = (95u32, 0u32);
    let (mut aggression, mut aggression_spread) = (0u32, 0u32);
    if let Some((raw, converted)) = grid {
        opponents = converted
            .opponents
            .iter()
            .map(|o| {
                json!({
                    "car_id": o.car_id,
                    "ai_level": o.ai_level,
                    "car_skin": o.car_skin,
                    "driver_name": o.driver_name,
                    "nationality": o.nationality,
                    "ballast": o.ballast,
                    "restrictor": o.restrictor,
                })
            })
            .collect();
        let (c, s) = center_spread(f64::from(converted.ai_level_min), f64::from(converted.ai_level_max));
        ai_level = c;
        ai_spread = s;
        let agg_max = cell_number(raw.get("AiAggression")).unwrap_or(0).clamp(0, 100) as f64;
        let agg_min = cell_number(raw.get("AiAggressionMin")).unwrap_or(0).clamp(0, 100) as f64;
        let (c, s) = center_spread(agg_min, agg_max);
        aggression = c;
        aggression_spread = s;
    } else if raw_grid.is_some() {
        notes.push(crate::errors::CM_NOTE_DRAWN_GRID.to_string());
    }

    // Starting position is a rank in the file, and the four Pit Box modes do
    // not all survive the trip: only first, second and last are recognisable.
    // Anything else was a rank CM drew, which is precisely what `random` means.
    let raw_grid_value = raw_grid.clone().unwrap_or_else(|| json!({}));
    let start_mode = match cell_number(raw_grid_value.get("StartingPosition")).unwrap_or(0) {
        1 => "first",
        2 => "second",
        n if n as usize == opponents.len() + 1 && !opponents.is_empty() => "last",
        _ => "random",
    };

    // Ballast and restrictor of the player sit in the grid for a race and in
    // the `ModeData` for the solo modes — the preset of reference shows both.
    let player_ballast = number(&raw_grid_value, "PlayerBallast")
        .or_else(|| number(&data, "PlayerBallast"))
        .unwrap_or(0.0)
        .clamp(0.0, 200.0) as u32;
    let player_restrictor = number(&raw_grid_value, "PlayerRestrictor")
        .or_else(|| number(&data, "PlayerRestrictor"))
        .unwrap_or(0.0)
        .clamp(0.0, 100.0) as u32;

    let practice_length = number(&data, "PracticeLength").unwrap_or(0.0).round() as u32;
    let (grip, track_state) = track_state_of(preset);
    let season_date = preset
        .get("dtv")
        .and_then(Value::as_str)
        .filter(|_| flag(preset, "udt").unwrap_or(false))
        .map(|d| d.split('T').next().unwrap_or(d).to_string());

    let setup = json!({
        "car_id": preset.get("CarId").and_then(Value::as_str).unwrap_or_default(),
        // No field in the schema, and that is the whole reason our own presets
        // carry a block of their own (SESSION§2).
        "car_skin": Value::Null,
        "driver": Value::Null,
        "track_id": track_id,
        "track_layout": track_layout,
        "session_type": session_type,
        "opponents": opponents,
        "ai_level": ai_level,
        "ai_spread": ai_spread,
        "aggression": aggression,
        "aggression_spread": aggression_spread,
        "player_ballast": player_ballast,
        "player_restrictor": player_restrictor,
        "start_mode": start_mode,
        "laps": number(&data, "LapsNumber").unwrap_or(0.0).max(0.0) as u32,
        "weather": preset.get("WeatherId").and_then(Value::as_str).unwrap_or_default(),
        "time_hours": number(preset, "Time").unwrap_or(43200.0) / 3600.0,
        "ambient_c": number(preset, "Temperature").map(|t| t.round() as i64),
        // `crt` only says a road temperature was set, never which one: the
        // schema has no field for the value itself, so there is nothing to read
        // back and the screen recomputes it from the weather.
        "road_c": Value::Null,
        "wind_speed_kmh": number(preset, "wsf").map(|w| w.round().max(0.0) as u32),
        "wind_direction_deg": number(preset, "wd").map(|w| w.round().max(0.0) as u32),
        "season": Value::Null,
        "season_date": season_date,
        "penalties": flag(&data, "Penalties").unwrap_or(true),
        "jump_start_penalty": number(&data, "JumpStartPenalty").unwrap_or(0.0).max(0.0) as u32,
        "track_state": track_state,
        "grip": grip,
        "practice_enabled": practice_length > 0,
        "practice_minutes": if practice_length > 0 { practice_length } else { 15 },
        "qualify_enabled": qualify_enabled,
        "qualify_minutes": number(&data, "QualificationLength").unwrap_or(30.0).max(5.0) as u32,
        "ghost_car": flag(&data, "GhostCar").unwrap_or(false),
        "ghost_advantage": number(&data, "GhostCarAdvantage").unwrap_or(0.0),
        "practice_start": match cell_text(data.get("StartType")).as_deref() {
            Some("TRACK") => "track",
            Some("HOTLAP_START") => "hotlap",
            _ => "pit",
        },
        "damage": number(&assists, "Damage").unwrap_or(100.0).clamp(0.0, 100.0) as u32,
        "fuel_rate": (number(&assists, "FuelConsumption").unwrap_or(1.0) * 100.0).clamp(0.0, 200.0) as u32,
        "tyre_wear": (number(&assists, "TyreWear").unwrap_or(1.0) * 100.0).clamp(0.0, 200.0) as u32,
        "tyre_blankets": flag(&assists, "TyreBlankets").unwrap_or(false),
        "abs": assist_level(&assists, "Abs"),
        "traction_control": assist_level(&assists, "TractionControl"),
        "ideal_line": flag(&assists, "IdealLine").unwrap_or(false),
    });

    // Shaped exactly like the frontend's `SavedSession`: the screen must not
    // have to know whether an entry came from us or from CM.
    Ok((
        json!({
            "name": name,
            "savedAt": saved_at,
            "setup": setup,
            "opponentCount": setup["opponents"].as_array().map(Vec::len).unwrap_or(0),
            "season": "",
            "intent": "",
        }),
        notes,
    ))
}

/// One-shot migration of `saved_sessions.json` (SESSION§3.6).
///
/// Runs on every listing but does its work once: the legacy file is renamed
/// afterwards, so a session deleted since is not resurrected at the next
/// launch. Best effort by design — an entry whose `setup` no longer
/// deserialises is logged and skipped rather than blocking the other ones,
/// and a failure to rename simply leaves the file for next time.
fn migrate_legacy_file(app: &AppHandle) {
    let Ok(config) = app.path().app_config_dir() else {
        return;
    };
    let legacy = config.join("saved_sessions.json");
    if !legacy.is_file() {
        return;
    }
    let all = crate::saved_sessions::load(app);
    let entries = all.as_object().map(|m| m.len()).unwrap_or(0);
    for entry in all.as_object().into_iter().flat_map(|m| m.values()) {
        let name = entry.get("name").and_then(Value::as_str).unwrap_or("session");
        let setup = match entry.get("setup").cloned().map(serde_json::from_value::<RaceSetup>) {
            Some(Ok(s)) => s,
            _ => {
                log::warn!("migration d'une session enregistrée : `setup` illisible pour « {name} »");
                continue;
            }
        };
        if let Err(e) = save(app, name, &setup, entry) {
            log::warn!("migration de la session « {name} » vers un preset : {e}");
        }
    }
    let done = legacy.with_file_name("saved_sessions.migrated.json");
    if let Err(e) = std::fs::rename(&legacy, &done) {
        // Not renaming means migrating again next time, which is harmless
        // (same names, same files) but worth a line: it is also what a
        // read-only config folder would look like.
        log::warn!("renommage de saved_sessions.json après migration de {entries} session(s) : {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A race preset written by CM itself, trimmed of the machine paths its
    /// original carried. Everything else is verbatim, including the two number
    /// spellings the format mixes.
    const CM_RACE: &str = r#"{"Mode":"/Pages/Drive/QuickDrive_Race.xaml","ModeData":"{\"Penalties\":true,\"JumpStartPenalty\":0,\"LapsNumber\":7,\"RaceGridSerialized\":\"{\\\"ModeId\\\":\\\"manual\\\",\\\"FilterValue\\\":\\\"\\\",\\\"CarIds\\\":[\\\"ks_praga_r1\\\",\\\"ks_lotus_25\\\"],\\\"SkinIds\\\":[\\\"red\\\",null],\\\"AiLevels\\\":[\\\"88\\\",\\\"-1\\\"],\\\"Ballasts\\\":[\\\"0\\\",\\\"20\\\"],\\\"Restrictors\\\":[\\\"0\\\",\\\"0\\\"],\\\"Names\\\":[\\\"Bob\\\",null],\\\"Nationalities\\\":[\\\"Italy\\\",null],\\\"PlayerBallast\\\":5.0,\\\"PlayerRestrictor\\\":10.0,\\\"OpponentsNumber\\\":2,\\\"StartingPosition\\\":3,\\\"AiLevel\\\":95.0,\\\"AiLevelMin\\\":85.0,\\\"AiAggression\\\":40.0,\\\"AiAggressionMin\\\":20.0}\",\"Version\":2}","CarId":"ks_praga_r1","TrackId":"spa/layout_gp","WeatherId":"3_clear","RealConditions":false,"Temperature":26.0,"Time":43200,"TimeMultipler":1,"udt":true,"dtv":"2026-02-19T00:00:00","tpc":false,"TrackPropertiesData":"{\"s\":0.95,\"t\":0.9,\"r\":0.02,\"g\":132,\"d\":\"A clean track, gets better with more laps.\",\"w\":false}","asc":false,"AssistsData":"{\"IdealLine\":false,\"AutoBlip\":false,\"StabilityControl\":0.0,\"AutoBrake\":false,\"AutoShifter\":false,\"SlipSteam\":1.0,\"AutoClutch\":false,\"Abs\":2,\"TractionControl\":0,\"VisualDamage\":true,\"Damage\":50.0,\"TyreWear\":1.5,\"FuelConsumption\":1.0,\"TyreBlankets\":true}","ico":false,"wsf":0.0,"wst":0.0,"wd":225.0,"crt":false}"#;

    fn cm_race() -> Value {
        serde_json::from_str(CM_RACE).expect("the reference preset parses")
    }

    /// Protected rule: a Content Manager race preset comes back as a session —
    /// grid, difficulty band, assists and track state included. Everything
    /// here is read from the file, nothing defaulted.
    #[test]
    fn a_content_manager_race_preset_reads_back_as_a_session() {
        let (session, notes) = from_cm(&cm_race(), "grid night", "2026-01-01T00:00:00.000Z").unwrap();
        let s = &session["setup"];
        assert_eq!(s["session_type"], "race");
        assert_eq!(
            s["qualify_enabled"], false,
            "QuickDrive_Race is the race without qualifying"
        );
        assert_eq!(s["car_id"], "ks_praga_r1");
        assert_eq!(
            (s["track_id"].as_str(), s["track_layout"].as_str()),
            (Some("spa"), Some("layout_gp"))
        );
        assert_eq!(s["opponents"].as_array().unwrap().len(), 2, "one line per CarId");
        assert_eq!(s["opponents"][0]["ai_level"], 88);
        assert_eq!(s["opponents"][1]["ai_level"], Value::Null, "-1 is Auto");
        assert_eq!(s["opponents"][0]["car_skin"], "red");
        assert_eq!(
            (s["ai_level"].as_u64(), s["ai_spread"].as_u64()),
            (Some(90), Some(5)),
            "85-95 is 90 ± 5"
        );
        assert_eq!(
            (s["aggression"].as_u64(), s["aggression_spread"].as_u64()),
            (Some(30), Some(10))
        );
        assert_eq!(
            (s["player_ballast"].as_u64(), s["player_restrictor"].as_u64()),
            (Some(5), Some(10))
        );
        assert_eq!(s["laps"], 7);
        assert_eq!(s["damage"], 50);
        assert_eq!(s["tyre_wear"], 150, "a 1.5 rate is 150 %");
        assert_eq!(s["abs"], "on", "2 is On, and 0 is Off — the game's own scale");
        assert_eq!(s["traction_control"], "off");
        assert_eq!(s["tyre_blankets"], true);
        assert_eq!(s["season_date"], "2026-02-19", "the date drops the time CM appends");
        assert_eq!(s["grip"], 95);
        assert_eq!(
            s["track_state"]["name"], "Green",
            "the four numbers name the state, the file does not"
        );
        assert_eq!(s["track_state"]["origin"], "builtin");
        assert!(
            notes.contains(&crate::errors::CM_NOTE_NO_PLAYER_SKIN.to_string()),
            "the player skin is the one thing the format cannot carry, and it is said"
        );
    }

    /// Protected rule: the three modes with no Pit Box equivalent are named,
    /// not hidden. Someone with ten presets who sees eight wonders which two
    /// vanished.
    #[test]
    fn a_mode_pit_box_has_no_equivalent_for_is_named_not_dropped() {
        for mode in [
            "QuickDrive_Drag.xaml",
            "QuickDrive_Drift.xaml",
            "QuickDrive_TimeAttack.xaml",
        ] {
            let preset = json!({ "Mode": format!("/Pages/Drive/{mode}") });
            assert_eq!(
                from_cm(&preset, "n", "t").unwrap_err(),
                crate::errors::CM_UNSUPPORTED_MODE,
                "{mode} is reported with a reason"
            );
        }
    }

    /// Protected rule: a draw mode is not a line-up (SESSION§3.5). Its `CarIds` are
    /// candidates, so the session loads with an empty grid and says why —
    /// inventing opponents nobody arranged would be worse than none.
    #[test]
    fn a_drawn_grid_yields_no_opponents_and_says_so() {
        let grid = json!({ "ModeId": "similar_p_w_ratio", "CarIds": ["ks_praga_r1"], "OpponentsNumber": 11 });
        let preset = json!({
            "Mode": "/Pages/Drive/QuickDrive_Race.xaml",
            "ModeData": json!({ "RaceGridSerialized": grid.to_string(), "LapsNumber": 3 }).to_string(),
        });
        let (session, notes) = from_cm(&preset, "n", "t").unwrap();
        assert_eq!(session["setup"]["opponents"].as_array().unwrap().len(), 0);
        assert_eq!(session["setup"]["laps"], 3, "the rest of the preset still reads");
        assert!(notes.contains(&crate::errors::CM_NOTE_DRAWN_GRID.to_string()));
    }

    /// Protected rule: a weekend preset is a race **with** qualifying, and a
    /// zero practice length means the phase is skipped — the very distinction
    /// `PracticeLength: 0` was introduced for (SESSION§2.1).
    #[test]
    fn a_weekend_preset_carries_its_two_phases() {
        let with_practice = json!({
            "Mode": "/Pages/Drive/QuickDrive_Weekend.xaml",
            "ModeData": json!({ "PracticeLength": 20, "QualificationLength": 45 }).to_string(),
        });
        let (s, _) = from_cm(&with_practice, "n", "t").unwrap();
        assert_eq!(s["setup"]["qualify_enabled"], true);
        assert_eq!(s["setup"]["practice_enabled"], true);
        assert_eq!(s["setup"]["practice_minutes"], 20);
        assert_eq!(s["setup"]["qualify_minutes"], 45);

        let skipped = json!({
            "Mode": "/Pages/Drive/QuickDrive_Weekend.xaml",
            "ModeData": json!({ "PracticeLength": 0, "QualificationLength": 30 }).to_string(),
        });
        let (s, _) = from_cm(&skipped, "n", "t").unwrap();
        assert_eq!(
            s["setup"]["practice_enabled"], false,
            "0 skips the phase, it is not a duration"
        );
    }

    /// Protected rule: « Auto » is the `WeatherDefined` flag, not a grip — it
    /// must come back as the sentinel the screen offers, never as a track at
    /// 100 %.
    #[test]
    fn the_weather_driven_state_comes_back_as_auto() {
        let preset = json!({
            "Mode": "/Pages/Drive/QuickDrive_Hotlap.xaml",
            "TrackPropertiesData": r#"{"s":0.95,"t":0.9,"r":0.02,"g":132,"d":"","w":true}"#,
        });
        let (s, _) = from_cm(&preset, "n", "t").unwrap();
        assert_eq!(s["setup"]["grip"], quickdrive::GRIP_WEATHER);
        assert_eq!(s["setup"]["track_state"]["weather_defined"], true);
    }

    /// Protected rule: a track state CM's user composed himself is kept with
    /// its own four numbers and named after his preset — not snapped onto the
    /// nearest built-in, which would change the session silently.
    #[test]
    fn a_user_track_state_keeps_its_numbers_and_its_name() {
        let preset = json!({
            "Mode": "/Pages/Drive/QuickDrive_Hotlap.xaml",
            "TrackPropertiesPresetFilename": r"C:\Presets\Track States\Damp evening.cmpreset",
            "TrackPropertiesData": r#"{"s":0.91,"t":0.7,"r":0.05,"g":200,"d":"","w":false}"#,
        });
        let (s, _) = from_cm(&preset, "n", "t").unwrap();
        let ts = &s["setup"]["track_state"];
        assert_eq!(ts["origin"], "cm", "no built-in has these four numbers");
        assert_eq!(ts["name"], "Damp evening");
        assert_eq!((ts["start"].as_u64(), ts["transfer"].as_u64()), (Some(91), Some(70)));
        assert_eq!(
            ts["randomness"], 5,
            "the three percentages are stored divided by a hundred"
        );
        assert_eq!(ts["lap_gain"], 200, "LAP_GAIN alone is raw");
    }

    /// Protected rule: the name the user types becomes a file name Windows
    /// accepts, and two different names never collapse onto one file.
    #[test]
    fn a_typed_name_becomes_a_file_name_without_collapsing() {
        assert_eq!(sanitize("GT3 / Spa"), "GT3 _ Spa");
        assert_ne!(sanitize("GT3 / Spa"), sanitize("GT3 Spa"));
        assert_eq!(sanitize("soirée : 20 h"), "soirée _ 20 h");
        assert_eq!(sanitize("trailing. "), "trailing", "Windows strips these itself");
        assert_eq!(sanitize("   "), "session", "an empty name still has to be a file");
    }

    /// Protected rule: our own preset is read from its block, never
    /// re-derived from the CM fields — that block is the only place the player
    /// skin and the weather intent exist.
    #[test]
    fn our_own_block_is_what_comes_back() {
        let snapshot = json!({ "name": "Spa dusk", "setup": { "car_skin": "red_pack" }, "intent": "rain" });
        let preset = json!({
            "Mode": "/Pages/Drive/QuickDrive_Race.xaml",
            "CarId": "ks_praga_r1",
            PITBOX_KEY: { "version": PITBOX_VERSION, "session": snapshot },
        });
        let back = pitbox_block(&preset).expect("our block is read");
        assert_eq!(back["setup"]["car_skin"], "red_pack");
        assert_eq!(back["intent"], "rain");
    }

    /// Protected rule: what we write stays an ordinary Quick Drive preset —
    /// Content Manager must be able to run it — and the player's skin travels
    /// **only** in our block, because the format has no field for it
    /// (SESSION§2). If that skin ever showed up in the CM part, it would mean
    /// someone invented a field CM does not read.
    #[test]
    fn our_preset_is_still_an_ordinary_quick_drive_preset() {
        let setup: RaceSetup = serde_json::from_value(json!({
            "car_id": "ks_praga_r1", "car_skin": "pitbox_only_skin",
            "track_id": "spa", "track_layout": "layout_gp", "session_type": "race",
        }))
        .unwrap();
        let snapshot = json!({ "name": "Spa dusk", "setup": { "car_skin": "pitbox_only_skin" } });
        let preset = preset_with_block(&setup, &snapshot).unwrap();

        assert_eq!(
            preset["Mode"], "/Pages/Drive/QuickDrive_Weekend.xaml",
            "a race with qualifying"
        );
        assert_eq!(preset["CarId"], "ks_praga_r1");
        assert_eq!(preset["TrackId"], "spa/layout_gp");
        assert!(preset["ModeData"].is_string(), "CM's own fields are untouched");
        assert_eq!(pitbox_block(&preset).unwrap()["setup"]["car_skin"], "pitbox_only_skin");

        let mut cm_part = preset.clone();
        cm_part.as_object_mut().unwrap().remove(PITBOX_KEY);
        assert!(
            !cm_part.to_string().contains("pitbox_only_skin"),
            "the player skin exists nowhere in the Quick Drive schema"
        );
    }

    /// Protected rule: a block from a future Pit Box is ignored rather than
    /// half-read — the file then behaves like any CM preset, which always
    /// works.
    #[test]
    fn a_newer_block_falls_back_to_reading_the_preset() {
        let preset = json!({ PITBOX_KEY: { "version": PITBOX_VERSION + 1, "session": { "name": "x" } } });
        assert!(pitbox_block(&preset).is_none());
    }

    /// Protected rule: a preset we did not write is never deleted. It is the
    /// corollary of the game-files rule applied to CM's own folder, and the
    /// guard lives in `delete` because the list shows both kinds side by side.
    #[test]
    fn a_preset_we_did_not_write_is_never_deleted() {
        let dir = crate::testutil::temp_dir("session-preset-guard");
        let theirs = dir.join("theirs.cmpreset");
        std::fs::write(&theirs, CM_RACE).unwrap();
        // The guard reads the file itself, so it is tested through the same
        // predicate `delete` uses rather than through a Tauri handle.
        assert!(
            !read_preset(&theirs).is_some_and(|v| pitbox_block(&v).is_some()),
            "no block, no deletion"
        );
        assert!(theirs.is_file(), "the file is still there");
    }
}
