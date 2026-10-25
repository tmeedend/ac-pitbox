//! What Story Mode writes in the player's library (SPEC-story-mode, Content:
//! a story's own changes to a mod): the layers a story carries, stored as
//! layers of the mods they lie over, **off** - and switched on for the
//! story's sessions only (`session.rs`). Through the library's own paths:
//! `layers::store_layer`, the switch of a sheet (`compose::set_layer_active`)
//! and its removal (`compose::remove_layer`). Nothing here writes to the
//! game's folder itself.
//!
//! A story's layer is a layer like any other, shown on the mod's sheet under
//! its story's name: the player sees what a story brought, and can remove it.

use std::path::Path;

use rusqlite::Connection;
use story_engine::needs::AcKind;

use crate::config::AppConfig;
use crate::layers::HostKind;

/// What Story Mode writes in the library, as the installer and the session
/// ask it: the app's own (the commands, with the base), or a test's.
pub trait LibraryWrites {
    /// Stores `files` as a layer named `name` of the car or track `host`,
    /// off; `origin` names where it came from. Returns the layer's id.
    fn add_layer(&self, kind: AcKind, host: &str, name: &str, files: &Path, origin: &str) -> Result<String, String>;
    fn remove_layer(&self, id: &str) -> Result<(), String>;
    fn set_layer_active(&self, id: &str, active: bool) -> Result<(), String>;
}

fn host_kind(kind: AcKind) -> HostKind {
    match kind {
        AcKind::Car => HostKind::Car,
        AcKind::Track => HostKind::Track,
    }
}

/// `LibraryWrites::add_layer` on the library: stored by the library's own
/// path, which refuses a mod Pit Box does not manage, then switched off
/// before anything composes it - a story's change never shows outside its
/// sessions.
pub fn add_layer(
    conn: &Connection,
    cfg: &AppConfig,
    kind: AcKind,
    host: &str,
    name: &str,
    files: &Path,
    origin: &str,
) -> Result<String, String> {
    let library = cfg
        .library_path
        .as_deref()
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let diff = crate::identity::DiffStats::default();
    let (id, _) = crate::layers::store_layer(
        conn,
        library,
        host,
        host_kind(kind),
        name,
        files,
        true,
        &diff,
        origin,
        crate::resources::ExtractionMode::None,
    )?;
    crate::overlay::set_layer_active(conn, &id, false).map_err(|e| e.to_string())?;
    Ok(id)
}

/// A library for tests that are not about it: what was asked, remembered.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Recorded {
    /// By id: what it lies over, its name, whether it is on.
    layers: std::cell::RefCell<std::collections::BTreeMap<String, (AcKind, String, String, bool)>>,
    fail_on: Option<String>,
}

#[cfg(test)]
impl Recorded {
    /// One holding the layers `ids`, off.
    pub fn with(ids: &[&str]) -> Recorded {
        let r = Recorded::default();
        for id in ids {
            r.layers
                .borrow_mut()
                .insert(id.to_string(), (AcKind::Car, "x".into(), id.to_string(), false));
        }
        r
    }

    /// One holding `l1` and `l2`, `id` refusing to come on.
    pub fn failing_on(id: &str) -> Recorded {
        Recorded {
            fail_on: Some(id.into()),
            ..Recorded::with(&["l1", "l2"])
        }
    }

    /// The layers on, by id.
    pub fn active(&self) -> Vec<String> {
        (self.layers.borrow().iter())
            .filter(|(_, l)| l.3)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Every layer stored: what it lies over and its name.
    pub fn stored(&self) -> Vec<(AcKind, String, String)> {
        (self.layers.borrow().values())
            .map(|l| (l.0, l.1.clone(), l.2.clone()))
            .collect()
    }
}

#[cfg(test)]
impl LibraryWrites for Recorded {
    fn add_layer(&self, kind: AcKind, host: &str, name: &str, files: &Path, _: &str) -> Result<String, String> {
        assert!(files.is_dir(), "the layer's files are there to store");
        let id = format!("layer-{}", self.layers.borrow().len() + 1);
        self.layers
            .borrow_mut()
            .insert(id.clone(), (kind, host.into(), name.into(), false));
        Ok(id)
    }

    fn remove_layer(&self, id: &str) -> Result<(), String> {
        self.layers.borrow_mut().remove(id);
        Ok(())
    }

    fn set_layer_active(&self, id: &str, active: bool) -> Result<(), String> {
        if active && self.fail_on.as_deref() == Some(id) {
            return Err("refused".into());
        }
        match self.layers.borrow_mut().get_mut(id) {
            Some(l) => {
                l.3 = active;
                Ok(())
            }
            None => Err(crate::errors::LAYER_NOT_FOUND.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    // A story's change to a mod is a layer of it, stored off: the game's
    // folder and the mod itself stay as they were (SPEC-story-mode, Content).
    #[test]
    fn a_story_layer_is_stored_off() {
        let base = temp_dir("story-layer-add");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_mod(&conn, "imola", "Track", None, Some("Imola"), "h", None, &now).unwrap();
        let files = base.join("files");
        std::fs::create_dir_all(files.join("extension")).unwrap();
        std::fs::write(files.join("extension/ext_config.ini"), "[HIDE]").unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..AppConfig::default()
        };

        let id = add_layer(
            &conn,
            &cfg,
            AcKind::Track,
            "imola",
            "No billboards",
            &files,
            "race.pitstory",
        )
        .unwrap();
        let layer = crate::overlay::get_layer(&conn, &id).unwrap().expect("stored");
        assert!(!layer.is_active, "off until a session of the story");
        assert_eq!(
            (layer.parent_id.as_str(), layer.name.as_str()),
            ("imola", "No billboards")
        );
        assert!(
            library
                .join("layers/tracks/imola/No billboards/extension/ext_config.ini")
                .is_file(),
            "its files in the library"
        );
        assert!(
            files.join("extension/ext_config.ini").is_file(),
            "copied, the package's own left alone"
        );
    }
}
