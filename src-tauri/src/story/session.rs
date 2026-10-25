//! A story's session (SPEC-story-mode, Content: a story's own changes to a
//! mod): when an event of the story is launched, the story's layers over its
//! car and track are switched on, and the story's own version of a car or
//! track stands in for the player's other one; when the game closes, all of
//! it is undone - the player's mods are as they were for any other session.
//!
//! The model is the online session's (`online/session_layers.rs`), the other
//! way round: what is changed is written to `story/session.json` **before**
//! anything is touched, and undone from that file when the game closes, at
//! the next start of Pit Box after one closed or killed during the session,
//! and when no game is running two minutes after the launch (`GUARD`) -
//! Content Manager may never start one.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use story_engine::needs::AcKind;

use super::library::LibraryWrites;

const FILE: &str = "story/session.json";

/// How long after a launch the game must be running, or the session is
/// undone: the online join's measure (`online::session_layers::GUARD`).
pub const GUARD: Duration = Duration::from_secs(120);

/// Never two changes at once on the same file.
static SWITCHING: Mutex<()> = Mutex::new(());

/// Bumped by each launch: a guard armed for an earlier one stands down.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// What a session of a story changes, for the event launched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// The story's layers to switch on.
    pub layers: Vec<String>,
    /// The story's own versions to stand in, each with its folder.
    pub versions: Vec<(AcKind, String, PathBuf)>,
}

/// What a session changed, as written down.
#[derive(Debug, Default, Serialize, Deserialize)]
struct On {
    layers: Vec<String>,
    #[serde(default)]
    versions: Vec<Standing>,
}

/// A story's version standing in for the player's.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Standing {
    kind: AcKind,
    id: String,
    /// Whether the player's was in the game before.
    was_active: bool,
}

fn read(config_dir: &Path) -> On {
    (std::fs::read_to_string(config_dir.join(FILE)).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write(config_dir: &Path, on: &On) -> Result<(), String> {
    let path = config_dir.join(FILE);
    if on.layers.is_empty() && on.versions.is_empty() {
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

/// Makes the changes of `plan`, each written down first. Returns the
/// generation to arm the guard with, `None` when there is nothing to change.
/// A layer that does not come on, or a version that cannot stand in for a
/// reason other than the player's copy not being Pit Box's, stops the launch -
/// and what was changed is undone. Where the player's copy is not Pit Box's,
/// it plays as it is.
pub fn begin(config_dir: &Path, plan: &Plan, library: &dyn LibraryWrites) -> Result<Option<u64>, String> {
    if plan.layers.is_empty() && plan.versions.is_empty() {
        return Ok(None);
    }
    let _switching = SWITCHING.lock().unwrap_or_else(|e| e.into_inner());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut on = read(config_dir);
    for id in &plan.layers {
        if !on.layers.contains(id) {
            on.layers.push(id.clone());
        }
    }
    write(config_dir, &on)?;
    let undo = |e: String| {
        end_locked(config_dir, library);
        Err(e)
    };
    for (kind, id, folder) in &plan.versions {
        if on.versions.iter().any(|s| &s.id == id && s.kind == *kind) {
            continue;
        }
        // Written down before the player's leaves the game: whatever stops
        // half-way, the end knows to bring it back.
        let was_active = library.in_game(*kind, id);
        on.versions.push(Standing {
            kind: *kind,
            id: id.clone(),
            was_active,
        });
        write(config_dir, &on)?;
        match library.stand_in(*kind, id, folder) {
            Ok(()) => {}
            Err(e) if e == crate::errors::STORY_VERSION_UNMANAGED => {
                log::warn!("story: the story's version of {id} cannot stand in, the player's plays");
                on.versions.retain(|s| &s.id != id || s.kind != *kind);
                write(config_dir, &on)?;
            }
            Err(e) => {
                log::warn!("story: the story's version of {id} not laid for the session: {e}");
                return undo(e);
            }
        }
    }
    // The layers last: none lies over a version standing in (`play::session_plan`).
    for id in &plan.layers {
        if let Err(e) = library.set_layer_active(id, true) {
            log::warn!("story: layer {id} not switched on for the session: {e}");
            return undo(e);
        }
    }
    Ok(Some(generation))
}

/// Whether a story's session has changes not undone yet: while it has, an
/// install or an uninstall waits (`packages`), or it would remove what the
/// running game reads.
pub fn under_way(config_dir: &Path) -> bool {
    let on = read(config_dir);
    !on.layers.is_empty() || !on.versions.is_empty()
}

/// Undoes every change of a story's session; returns how many. One that
/// cannot be undone stays in the file, for the next try.
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
    let mut undone = 0;
    let mut left = On::default();
    // The versions first: a layer switched off recomposes its host, which
    // must find the player's mod back, not the story's junction.
    for standing in on.versions {
        match library.stand_down(standing.kind, &standing.id, standing.was_active) {
            Ok(()) => undone += 1,
            Err(e) => {
                log::warn!("story: {} not given back after the session: {e}", standing.id);
                left.versions.push(standing);
            }
        }
    }
    for id in on.layers {
        match library.set_layer_active(&id, false) {
            Ok(()) => undone += 1,
            // Removed meanwhile (the story uninstalled): nothing left on.
            Err(e) if e == crate::errors::LAYER_NOT_FOUND => {}
            Err(e) => {
                log::warn!("story: layer {id} not switched off after the session: {e}");
                left.layers.push(id);
            }
        }
    }
    if let Err(e) = write(config_dir, &left) {
        log::warn!("story: {FILE} not updated: {e}");
    }
    undone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::library::Recorded;
    use crate::testutil::temp_dir;

    fn layers(ids: &[&str]) -> Plan {
        Plan {
            layers: ids.iter().map(|s| s.to_string()).collect(),
            versions: Vec::new(),
        }
    }

    // A story's layers are on for its session only, and what was switched on
    // is written down first: a game closed, a Pit Box killed, a launch that
    // never ran - each ends with them off (module doc).
    #[test]
    fn a_story_layer_is_on_for_its_session_only() {
        let config = temp_dir("story-session");
        let library = Recorded::with(&["l1", "l2"]);
        let generation = begin(&config, &layers(&["l1", "l2"]), &library).unwrap();
        assert_eq!(library.active(), ["l1", "l2"], "on for the session");
        assert!(config.join(FILE).is_file(), "written down");

        assert_eq!(end(&config, &library), 2);
        assert!(library.active().is_empty(), "off when the game closed");
        assert!(!config.join(FILE).exists(), "nothing left to give back");
        assert_eq!(end(&config, &library), 0, "a second end finds nothing");

        // A launch whose game never ran: the guard switches them off; a
        // later launch keeps its own on.
        let first = begin(&config, &layers(&["l1"]), &library).unwrap().unwrap();
        let second = begin(&config, &layers(&["l2"]), &library).unwrap().unwrap();
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
        assert!(begin(&config, &layers(&["l1", "l2"]), &library).is_err());
        assert!(library.active().is_empty(), "what came on went off");
    }

    // The story's own version stands in for the session and is given back
    // when it ends, written down before the player's leaves the game.
    #[test]
    fn a_storys_version_stands_in_for_its_session_only() {
        let config = temp_dir("story-session-version");
        let library = Recorded::default();
        let folder = config.join("version/mod_car");
        std::fs::create_dir_all(&folder).unwrap();
        let plan = Plan {
            layers: Vec::new(),
            versions: vec![(AcKind::Car, "mod_car".into(), folder)],
        };
        begin(&config, &plan, &library).unwrap();
        assert_eq!(library.standing(), ["mod_car"]);
        let written = std::fs::read_to_string(config.join(FILE)).unwrap();
        assert!(written.contains("mod_car"), "written down: {written}");

        assert!(under_way(&config), "an install waits meanwhile");
        assert_eq!(end(&config, &library), 1);
        assert!(library.standing().is_empty(), "the player's back");
        assert!(!under_way(&config));
        assert!(!config.join(FILE).exists());
    }
}
