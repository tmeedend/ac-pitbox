//! Layers set aside for an online session (`SPEC-play-online.md`, "Couches et
//! versions").
//!
//! A server checks a car's `data.acd` (or `data/`) and a track's
//! `surfaces.ini` against its own: a layer that replaces one of them gets the
//! player kicked — a certain failure. A layer that touches anything else
//! (textures, a CSP extension) may or may not pass — a possible one. Sound
//! alone (`sfx/`) passes: servers do not check it (the user's experience,
//! 2026-10-04), and Pit Box's own sound mods only ever lay files there. Pit Box
//! deactivates the layers the user lets it for the session, and gives them back
//! when the game closes.
//!
//! **The giving back does not depend on a clean exit.** What was set aside is
//! written to `online_layers.json` *before* any layer is touched; the game's
//! end (the process watch, `music/watch.rs`) restores from that file, and so
//! does the watch's first announcement at startup — a Pit Box closed during
//! the session, or killed, finds its layers back on the next start, like the
//! sweep of `gamebackup`.
//!
//! Layers go through `compose::set_layer_active`, the same path as the switch
//! on a sheet: nothing here writes to `content/` itself.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::layers::HostKind;
use crate::modscan::ModKind;

const FILE: &str = "online_layers.json";

/// Restores run from two places (the game's end, and the startup
/// announcement): never both at once on the same file.
static RESTORING: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    /// Replaces what the server checks: the join fails if it stays.
    Certain,
    /// Touches something else: may pass, may not.
    Possible,
}

/// One active layer on the car or the track of a join.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerConflict {
    pub layer_id: String,
    pub name: String,
    pub risk: Risk,
}

/// The risk a layer's files carry for an online join on a `host` of that
/// kind, `None` for a layer that brings no file at all. Paths are relative to
/// the mod's folder, `/`-separated (`layers::list_files`).
pub fn classify(kind: ModKind, rel_paths: &[String]) -> Option<Risk> {
    if rel_paths.is_empty() {
        return None;
    }
    let sound = |path: &str| path.to_ascii_lowercase().starts_with("sfx/");
    if kind == ModKind::Car && rel_paths.iter().all(|p| sound(p)) {
        return None;
    }
    let checked = |path: &str| {
        let path = path.to_ascii_lowercase();
        match kind {
            ModKind::Car => path == "data.acd" || path.starts_with("data/"),
            // `data/surfaces.ini`, or a layout's own `<layout>/data/surfaces.ini`.
            ModKind::Track => path == "surfaces.ini" || path.ends_with("/surfaces.ini"),
        }
    };
    Some(if rel_paths.iter().any(|p| checked(p)) {
        Risk::Certain
    } else {
        Risk::Possible
    })
}

/// The active layers of `id` and the risk each carries. A layer whose files
/// cannot be listed is reported as possible: better asked about than missed.
pub fn conflicts(conn: &Connection, cfg: &AppConfig, kind: ModKind, id: &str) -> Vec<LayerConflict> {
    let layers = match crate::overlay::active_layers(conn, id, HostKind::from(kind)) {
        Ok(layers) => layers,
        Err(e) => {
            log::warn!("online: cannot read the layers of {id} — {e}");
            return Vec::new();
        }
    };
    layers
        .into_iter()
        .filter(|l| !l.is_skeleton())
        .filter_map(|layer| {
            let paths: Vec<String> = match crate::layers::list_files(conn, cfg, &layer.id) {
                Ok(files) => files.into_iter().map(|f| f.rel_path).collect(),
                Err(e) => {
                    log::warn!("online: cannot list layer {} — {e}", layer.id);
                    vec![String::new()]
                }
            };
            let risk = classify(kind, &paths)?;
            Some(LayerConflict {
                name: layer.display_name_user.clone().unwrap_or_else(|| layer.name.clone()),
                layer_id: layer.id,
                risk,
            })
        })
        .collect()
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct SetAside {
    /// Layer ids deactivated for a session, to reactivate after it.
    #[serde(default)]
    layers: Vec<String>,
}

fn read(config_dir: &Path) -> SetAside {
    let path = config_dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("online: {} is not readable ({e}), nothing to restore", path.display());
            SetAside::default()
        }),
        Err(_) => SetAside::default(),
    }
}

fn write(config_dir: &Path, set_aside: &SetAside) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(set_aside).map_err(|e| e.to_string())?;
    std::fs::write(config_dir.join(FILE), json).map_err(|e| format!("writing {FILE}: {e}"))
}

/// Deactivates `layer_ids` for the session. The list is on disk first: if
/// anything stops halfway, the restore still knows every layer it has to give
/// back. A layer that cannot be deactivated stops the join — joining with it
/// is the failure this exists to avoid.
pub fn set_aside(conn: &Connection, cfg: &AppConfig, config_dir: &Path, layer_ids: &[String]) -> Result<(), String> {
    if layer_ids.is_empty() {
        return Ok(());
    }
    let _guard = RESTORING.lock().unwrap_or_else(|e| e.into_inner());
    let mut pending = read(config_dir);
    for id in layer_ids {
        if !pending.layers.contains(id) {
            pending.layers.push(id.clone());
        }
    }
    write(config_dir, &pending)?;
    for id in layer_ids {
        crate::compose::set_layer_active(conn, cfg, id, false)?;
        log::info!("online: layer {id} set aside for the session");
    }
    Ok(())
}

/// Reactivates every layer set aside and forgets them; returns the names of
/// those reactivated. A layer deleted since is skipped; one that fails to
/// come back stays in the file, for the next try, and is logged.
pub fn restore(conn: &Connection, cfg: &AppConfig, config_dir: &Path) -> Vec<String> {
    let _guard = RESTORING.lock().unwrap_or_else(|e| e.into_inner());
    let pending = read(config_dir);
    if pending.layers.is_empty() {
        return Vec::new();
    }
    let mut restored = Vec::new();
    let mut left = Vec::new();
    for id in pending.layers {
        let layer = match crate::overlay::get_layer(conn, &id) {
            Ok(Some(layer)) => layer,
            Ok(None) => continue, // deleted meanwhile: nothing to give back
            Err(e) => {
                log::warn!("online: cannot read layer {id} to restore it — {e}");
                left.push(id);
                continue;
            }
        };
        if layer.is_active {
            continue; // already back, by hand
        }
        match crate::compose::set_layer_active(conn, cfg, &id, true) {
            Ok(()) => restored.push(layer.display_name_user.unwrap_or(layer.name)),
            Err(e) => {
                log::warn!("online: cannot reactivate layer {id} — {e}");
                left.push(id);
            }
        }
    }
    let rest = SetAside { layers: left };
    if let Err(e) = if rest.layers.is_empty() {
        std::fs::remove_file(config_dir.join(FILE)).map_err(|e| e.to_string())
    } else {
        write(config_dir, &rest)
    } {
        log::warn!("online: cannot update {FILE} — {e}");
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    /// Rule (SPEC-play-online, "Couches et versions"): a car layer replacing
    /// `data.acd` or anything under `data/` is a certain failure.
    #[test]
    fn a_car_physics_layer_is_a_certain_failure() {
        assert_eq!(classify(ModKind::Car, &paths(&["data.acd"])), Some(Risk::Certain));
        assert_eq!(
            classify(ModKind::Car, &paths(&["skins/x/a.dds", "DATA/engine.ini"])),
            Some(Risk::Certain)
        );
        assert_eq!(
            classify(ModKind::Car, &paths(&["sfx/car.bank", "SFX/GUIDs.txt"])),
            None,
            "sound alone passes: servers do not check it"
        );
        assert_eq!(
            classify(ModKind::Car, &paths(&["sfx/car.bank", "skins/x/a.dds"])),
            Some(Risk::Possible),
            "sound with something else may not"
        );
    }

    /// Rule: a track layer replacing a `surfaces.ini`, the track's or a
    /// layout's, is a certain failure; anything else a possible one.
    #[test]
    fn a_track_surfaces_layer_is_a_certain_failure() {
        assert_eq!(
            classify(ModKind::Track, &paths(&["data/surfaces.ini"])),
            Some(Risk::Certain)
        );
        assert_eq!(
            classify(ModKind::Track, &paths(&["gp/data/Surfaces.ini"])),
            Some(Risk::Certain)
        );
        assert_eq!(
            classify(ModKind::Track, &paths(&["skins/default/grass.dds"])),
            Some(Risk::Possible)
        );
        assert_eq!(
            classify(ModKind::Track, &[]),
            None,
            "a layer with no file risks nothing"
        );
    }

    fn write_file(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// A track `spa` deployed in a synthetic game, with an active layer
    /// `grip` that replaces its `surfaces.ini`. Returns the database, the
    /// configuration, the folder for `online_layers.json`, and the
    /// `surfaces.ini` the game reads.
    struct SpaWithGrip {
        _base: crate::testutil::TempDir,
        conn: Connection,
        cfg: AppConfig,
        config_dir: std::path::PathBuf,
        surfaces: std::path::PathBuf,
    }

    fn spa_with_grip(tag: &str) -> SpaWithGrip {
        let base = crate::testutil::temp_dir(tag);
        let ac = base.join("ac");
        let library = base.join("library");
        let config_dir = base.join("config");
        std::fs::create_dir_all(ac.join("content").join("tracks")).unwrap();
        let version = library.join("tracks").join("spa").join("v1");
        write_file(&version.join("data").join("surfaces.ini"), "base");
        write_file(&version.join("ui").join("ui_track.json"), "{}");

        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        crate::overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, "now").unwrap();
        crate::overlay::insert_version(
            &conn,
            "v1",
            "spa",
            Some("1.0"),
            None,
            "now",
            &version.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "spa", "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        crate::activation::activate(&conn, &cfg, "spa", None).unwrap();

        let layer_dir = library.join("layers").join("spa").join("grip");
        write_file(&layer_dir.join("data").join("surfaces.ini"), "layer");
        let priority = crate::overlay::next_layer_priority(&conn, "spa", HostKind::Track).unwrap();
        crate::overlay::insert_layer(
            &conn,
            "grip",
            "spa",
            "Track",
            "grip",
            &layer_dir.to_string_lossy(),
            None,
            0,
            1,
            priority,
            "now",
        )
        .unwrap();
        crate::compose::recompose(&conn, &cfg, "spa").unwrap();
        let surfaces = ac
            .join("content")
            .join("tracks")
            .join("spa")
            .join("data")
            .join("surfaces.ini");
        SpaWithGrip {
            _base: base,
            conn,
            cfg,
            config_dir,
            surfaces,
        }
    }

    /// Rule (SPEC-play-online, "Couches et versions"), end to end on a real
    /// file system: a layer replacing a track's `surfaces.ini` is found as a
    /// certain failure, set aside — the game then sees the base file — and
    /// given back whole, the list of what to give back gone with it.
    #[test]
    fn a_layer_set_aside_comes_back_after_the_session() {
        let SpaWithGrip {
            _base,
            conn,
            cfg,
            config_dir,
            surfaces,
        } = spa_with_grip("online-layers-e2e");
        assert_eq!(
            std::fs::read_to_string(&surfaces).unwrap(),
            "layer",
            "the layer is in the game"
        );

        let found = conflicts(&conn, &cfg, ModKind::Track, "spa");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].risk, Risk::Certain, "surfaces.ini is what the server checks");

        set_aside(&conn, &cfg, &config_dir, &["grip".to_string()]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&surfaces).unwrap(),
            "base",
            "the server sees the base"
        );
        assert!(config_dir.join(FILE).is_file(), "the list to give back is on disk");

        let restored = restore(&conn, &cfg, &config_dir);
        assert_eq!(restored, vec!["grip"], "named for the notification");
        assert_eq!(
            std::fs::read_to_string(&surfaces).unwrap(),
            "layer",
            "the layer is back"
        );
        assert!(!config_dir.join(FILE).exists(), "nothing left to give back");
        assert!(
            restore(&conn, &cfg, &config_dir).is_empty(),
            "a second restore does nothing"
        );
    }

    /// Rule: what is set aside is on disk before anything is touched, and the
    /// list survives a restart — the startup restore reads the same file.
    #[test]
    fn the_set_aside_list_is_persisted_and_merged() {
        let dir = crate::testutil::temp_dir("online-layers");
        write(
            &dir,
            &SetAside {
                layers: vec!["a".into()],
            },
        )
        .unwrap();
        let mut pending = read(&dir);
        pending.layers.push("b".into());
        write(&dir, &pending).unwrap();
        assert_eq!(read(&dir).layers, vec!["a", "b"], "read back after a restart");
        std::fs::write(dir.join(FILE), "{broken").unwrap();
        assert_eq!(
            read(&dir),
            SetAside::default(),
            "a damaged file restores nothing, quietly"
        );
    }
}
