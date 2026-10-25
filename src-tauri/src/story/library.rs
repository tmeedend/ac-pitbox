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
//!
//! **The story's own version of a car or track** stands in for the
//! player's during a session (`stand_in`, `stand_down`): the player's mod
//! leaves the game by its own path (`activation::deactivate`), the story's
//! version is laid as a junction, and afterwards the junction is removed by
//! the one function that refuses anything but a junction
//! (`activation::remove_junction`) and the player's mod comes back as it
//! was. A folder of the game Pit Box does not manage is never touched: the
//! player's copy plays then.

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
    /// Whether the player's `id` is in the game now.
    fn in_game(&self, kind: AcKind, id: &str) -> bool;
    /// The story's version in `version` stands in for the player's `id`.
    fn stand_in(&self, kind: AcKind, id: &str, version: &Path) -> Result<(), String>;
    /// The player's `id` back, in the game if it `was_active`.
    fn stand_down(&self, kind: AcKind, id: &str, was_active: bool) -> Result<(), String>;
}

fn mod_kind(kind: AcKind) -> crate::modscan::ModKind {
    match kind {
        AcKind::Car => crate::modscan::ModKind::Car,
        AcKind::Track => crate::modscan::ModKind::Track,
    }
}

/// `LibraryWrites::stand_in` on the game: refused, touching nothing, when
/// the game's folder holds a copy Pit Box does not manage (a mod installed
/// by hand, the game's own content).
pub fn stand_in(conn: &Connection, cfg: &AppConfig, kind: AcKind, id: &str, version: &Path) -> Result<(), String> {
    use crate::activation;
    let link = activation::content_link(cfg, mod_kind(kind), id).ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    let managed = crate::overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .is_some_and(|m| {
            !m.is_stock && !m.is_unmanaged && crate::modscan::ModKind::from_column(&m.kind) == mod_kind(kind)
        });
    let was_active = activation::is_mod_active(cfg, mod_kind(kind), id);
    if !managed {
        if std::fs::symlink_metadata(&link).is_ok() {
            return Err(crate::errors::STORY_VERSION_UNMANAGED.into());
        }
    } else if was_active {
        activation::deactivate(conn, cfg, id)?;
    }
    if let Err(e) = activation::create_junction(&link, version) {
        if was_active {
            if let Err(back) = activation::activate(conn, cfg, id, None) {
                log::warn!("story: {id} not put back in the game: {back}");
            }
        }
        return Err(e);
    }
    Ok(())
}

/// `LibraryWrites::in_game` on the game.
pub fn in_game(cfg: &AppConfig, kind: AcKind, id: &str) -> bool {
    crate::activation::is_mod_active(cfg, mod_kind(kind), id)
}

/// `LibraryWrites::stand_down` on the game: the story's junction removed -
/// only a junction, whatever else stands there stays - and the player's mod
/// back if it was in the game and is not. Safe to run again: a session that
/// stopped half-way ends the same way.
pub fn stand_down(conn: &Connection, cfg: &AppConfig, kind: AcKind, id: &str, was_active: bool) -> Result<(), String> {
    use crate::activation;
    let link = activation::content_link(cfg, mod_kind(kind), id).ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    if activation::is_junction(&link) {
        activation::remove_junction(&link)?;
    }
    if was_active && !activation::is_mod_active(cfg, mod_kind(kind), id) {
        activation::activate(conn, cfg, id, None)?;
    }
    Ok(())
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
    /// The cars and tracks whose story version stands in now.
    standing: std::cell::RefCell<Vec<String>>,
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

    /// The cars and tracks whose story version stands in now.
    pub fn standing(&self) -> Vec<String> {
        self.standing.borrow().clone()
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

    fn in_game(&self, _: AcKind, _: &str) -> bool {
        true
    }

    fn stand_in(&self, _: AcKind, id: &str, version: &Path) -> Result<(), String> {
        assert!(version.is_dir(), "the story's version is there to stand in");
        self.standing.borrow_mut().push(id.into());
        Ok(())
    }

    fn stand_down(&self, _: AcKind, id: &str, _: bool) -> Result<(), String> {
        self.standing.borrow_mut().retain(|s| s != id);
        Ok(())
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

    /// A car of the library, version 1, deployed in the game as the player
    /// has it; `body` is what its one file holds.
    fn players_car(base: &Path, body: &str) -> (Connection, AppConfig) {
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content/cars")).unwrap();
        let v1 = library.join("cars/mod_car/v1");
        std::fs::create_dir_all(v1.join("data")).unwrap();
        std::fs::write(v1.join("data/engine.ini"), body).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_mod(&conn, "mod_car", "Car", Some("B"), Some("Car"), "h", None, &now).unwrap();
        crate::overlay::insert_version(
            &conn,
            "v1",
            "mod_car",
            Some("1.0"),
            None,
            &now,
            &v1.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "mod_car", "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(library),
            ..AppConfig::default()
        };
        (conn, cfg)
    }

    // The story's version stands in for the player's for a session, and the
    // player's comes back as it was: deployed if it was, its library copy
    // untouched (SPEC-story-mode, Content; CLAUDE.md, golden rules 2 and 5).
    #[test]
    fn the_storys_version_stands_in_and_the_players_comes_back() {
        let base = temp_dir("story-stand-in");
        let (conn, cfg) = players_car(&base, "[PLAYER]");
        crate::activation::activate(&conn, &cfg, "mod_car", None).unwrap();
        let played = base.join("ac/content/cars/mod_car/data/engine.ini");
        assert_eq!(std::fs::read_to_string(&played).unwrap(), "[PLAYER]");
        let story = base.join("story_version/mod_car");
        std::fs::create_dir_all(story.join("data")).unwrap();
        std::fs::write(story.join("data/engine.ini"), "[STORY]").unwrap();

        let was = in_game(&cfg, AcKind::Car, "mod_car");
        assert!(was, "the player's is in the game");
        stand_in(&conn, &cfg, AcKind::Car, "mod_car", &story).unwrap();
        assert_eq!(
            std::fs::read_to_string(&played).unwrap(),
            "[STORY]",
            "the story's version plays"
        );
        assert!(crate::activation::is_junction(&base.join("ac/content/cars/mod_car")));

        stand_down(&conn, &cfg, AcKind::Car, "mod_car", was).unwrap();
        assert_eq!(
            std::fs::read_to_string(&played).unwrap(),
            "[PLAYER]",
            "the player's back"
        );
        assert!(!crate::activation::is_junction(&base.join("ac/content/cars/mod_car")));
        assert_eq!(
            std::fs::read_to_string(story.join("data/engine.ini")).unwrap(),
            "[STORY]",
            "the story's copy left as it was"
        );
        assert_eq!(
            std::fs::read_to_string(base.join("library/cars/mod_car/v1/data/engine.ini")).unwrap(),
            "[PLAYER]",
            "the library untouched"
        );

        // A player's mod switched off stays off.
        crate::activation::deactivate(&conn, &cfg, "mod_car").unwrap();
        let was = in_game(&cfg, AcKind::Car, "mod_car");
        assert!(!was);
        stand_in(&conn, &cfg, AcKind::Car, "mod_car", &story).unwrap();
        stand_down(&conn, &cfg, AcKind::Car, "mod_car", was).unwrap();
        assert!(
            !base.join("ac/content/cars/mod_car").exists(),
            "off, as the player left it"
        );
    }

    // A folder of the game Pit Box does not manage is never touched.
    #[test]
    fn a_folder_pit_box_does_not_manage_is_left_alone() {
        let base = temp_dir("story-stand-in-unmanaged");
        let (conn, cfg) = players_car(&base, "[PLAYER]");
        let by_hand = base.join("ac/content/cars/other_car");
        std::fs::create_dir_all(&by_hand).unwrap();
        std::fs::write(by_hand.join("x.ini"), "mine").unwrap();
        let story = base.join("story_version/other_car");
        std::fs::create_dir_all(&story).unwrap();

        assert_eq!(
            stand_in(&conn, &cfg, AcKind::Car, "other_car", &story),
            Err(crate::errors::STORY_VERSION_UNMANAGED.to_string())
        );
        assert_eq!(std::fs::read_to_string(by_hand.join("x.ini")).unwrap(), "mine");
        stand_down(&conn, &cfg, AcKind::Car, "other_car", false).unwrap();
        assert!(
            by_hand.join("x.ini").is_file(),
            "nor removed afterwards: only a junction is"
        );
    }
}
