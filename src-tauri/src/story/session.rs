//! A story's session (SPEC-story-mode, Content: a story's own changes to a
//! mod): the story's layers over the event's car and track are switched on
//! when the session is launched, and off again when the game closes - the
//! player's mods are as they were for any other session.
//!
//! The model is the online session's (`online/session_layers.rs`), the other
//! way round: what is switched on is written to `story/session.json`
//! **before** any layer is touched, and switched off from that file when the
//! game closes, at the next start of Pit Box after one closed or killed
//! during the session, and when no game is running two minutes after the
//! launch (`GUARD`) - Content Manager may never start one.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::library::LibraryWrites;

const FILE: &str = "story/session.json";

/// How long after a launch the game must be running, or the layers go off
/// again: the online join's measure (`online::session_layers::GUARD`).
pub const GUARD: Duration = Duration::from_secs(120);

/// Never two switches at once on the same file.
static SWITCHING: Mutex<()> = Mutex::new(());

/// Bumped by each launch: a guard armed for an earlier one stands down.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The layers on for a story's session.
#[derive(Debug, Default, Serialize, Deserialize)]
struct On {
    layers: Vec<String>,
}

fn read(config_dir: &Path) -> On {
    (std::fs::read_to_string(config_dir.join(FILE)).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write(config_dir: &Path, on: &On) -> Result<(), String> {
    let path = config_dir.join(FILE);
    if on.layers.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(on).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// Switches `layers` on for a session, written down first. Returns the
/// generation to arm the guard with, `None` when there is nothing to switch.
/// A layer that does not come on stops the launch - and what came on goes off.
pub fn begin(config_dir: &Path, layers: &[String], library: &dyn LibraryWrites) -> Result<Option<u64>, String> {
    if layers.is_empty() {
        return Ok(None);
    }
    let _switching = SWITCHING.lock().unwrap_or_else(|e| e.into_inner());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut on = read(config_dir);
    for id in layers {
        if !on.layers.contains(id) {
            on.layers.push(id.clone());
        }
    }
    write(config_dir, &on)?;
    for id in layers {
        if let Err(e) = library.set_layer_active(id, true) {
            log::warn!("story: layer {id} not switched on for the session: {e}");
            end_locked(config_dir, library);
            return Err(e);
        }
    }
    Ok(Some(generation))
}

/// Switches off every layer on for a story's session; returns how many. One
/// that does not go off stays in the file, for the next try.
pub fn end(config_dir: &Path, library: &dyn LibraryWrites) -> usize {
    let _switching = SWITCHING.lock().unwrap_or_else(|e| e.into_inner());
    end_locked(config_dir, library)
}

/// The guard's end: `end`, unless the game runs or another launch came
/// since `generation`.
pub fn end_if_no_game(config_dir: &Path, generation: u64, running: bool, library: &dyn LibraryWrites) -> usize {
    let _switching = SWITCHING.lock().unwrap_or_else(|e| e.into_inner());
    if running || GENERATION.load(Ordering::SeqCst) != generation {
        return 0;
    }
    end_locked(config_dir, library)
}

fn end_locked(config_dir: &Path, library: &dyn LibraryWrites) -> usize {
    let on = read(config_dir);
    let mut off = 0;
    let mut left = Vec::new();
    for id in on.layers {
        match library.set_layer_active(&id, false) {
            Ok(()) => off += 1,
            // Removed meanwhile (the story uninstalled): nothing left on.
            Err(e) if e == crate::errors::LAYER_NOT_FOUND => {}
            Err(e) => {
                log::warn!("story: layer {id} not switched off after the session: {e}");
                left.push(id);
            }
        }
    }
    if let Err(e) = write(config_dir, &On { layers: left }) {
        log::warn!("story: {FILE} not updated: {e}");
    }
    off
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::library::Recorded;
    use crate::testutil::temp_dir;

    // A story's layers are on for its session only, and what was switched on
    // is written down first: a game closed, a Pit Box killed, a launch that
    // never ran - each ends with them off (module doc).
    #[test]
    fn a_story_layer_is_on_for_its_session_only() {
        let config = temp_dir("story-session");
        let library = Recorded::with(&["l1", "l2"]);
        let generation = begin(&config, &["l1".into(), "l2".into()], &library).unwrap();
        assert_eq!(library.active(), ["l1", "l2"], "on for the session");
        assert!(config.join(FILE).is_file(), "written down");

        assert_eq!(end(&config, &library), 2);
        assert!(library.active().is_empty(), "off when the game closed");
        assert!(!config.join(FILE).exists(), "nothing left to give back");
        assert_eq!(end(&config, &library), 0, "a second end finds nothing");

        // A launch whose game never ran: the guard switches them off; a
        // later launch keeps its own on.
        let first = begin(&config, &["l1".into()], &library).unwrap().unwrap();
        let second = begin(&config, &["l2".into()], &library).unwrap().unwrap();
        assert_eq!(
            end_if_no_game(&config, first, false, &library),
            0,
            "another launch since"
        );
        assert_eq!(end_if_no_game(&config, second, true, &library), 0, "the game runs");
        assert_eq!(end_if_no_game(&config, second, false, &library), 2);
        assert!(library.active().is_empty());
        assert_eq!(generation.map(|g| g < first), Some(true));
    }

    #[test]
    fn a_layer_that_does_not_come_on_stops_the_launch() {
        let config = temp_dir("story-session-fail");
        let library = Recorded::failing_on("l2");
        assert!(begin(&config, &["l1".into(), "l2".into()], &library).is_err());
        assert!(library.active().is_empty(), "what came on went off");
    }
}
