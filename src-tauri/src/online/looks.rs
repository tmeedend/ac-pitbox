//! How the cars and tracks of the servers look here: names and pictures, read
//! from the library (SPEC-play-online.md, "Liste des serveurs": "le visuel
//! reconnaissable remplace la lecture").
//!
//! Only what the servers reference is read: the lobby names a few hundred
//! tracks, and reading the layouts of every track of the library to show
//! twenty rows would be work for nothing. What the library does not hold has
//! no picture here (`picture.rs` looks for one), only the name CSP's table
//! gives it — else the screen falls back on the id.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;

/// One layout of a track, as its `ui_track.json` and images describe it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LayoutLook {
    pub name: String,
    /// Absolute paths, for `convertFileSrc`.
    pub preview: Option<String>,
    pub outline: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TrackLook {
    /// The track's name in the library (the user's correction included).
    pub name: String,
    /// Its track categories in the library (freeroam, drift…), what the
    /// category token of the list reads (`SPEC-play-online.md`, "Filtres de
    /// base"). Empty when the library gives it none.
    pub categories: Vec<String>,
    /// Keyed by layout folder, `""` for a single-layout track.
    pub layouts: HashMap<String, LayoutLook>,
}

/// Names and pictures, keyed by lowercase id — servers do not always write
/// ids in the case of the folder.
#[derive(Debug, Default, Serialize)]
pub struct Looks {
    pub cars: HashMap<String, String>,
    pub tracks: HashMap<String, TrackLook>,
    /// Names of the tracks the library does not hold, from CSP's table and
    /// Content Manager's copy (`sun::track_names`): `Imola` rather than
    /// `imola`, `Mt. Akina` rather than `pk_akina`.
    pub track_names: HashMap<String, String>,
}

impl Looks {
    /// Reads the looks of `cars` and `tracks` (lowercase ids) that the library
    /// holds, and CSP's name of every track of `named` the library does not
    /// hold. A library that cannot be read gives no looks, logged: the list
    /// still works on ids.
    pub fn scan(
        conn: &Connection,
        cfg: &AppConfig,
        cars: &HashSet<String>,
        tracks: &HashSet<String>,
        named: &HashSet<String>,
    ) -> Self {
        let mut looks = Self::default();
        let mods = match crate::overlay::list_mods(conn) {
            Ok(mods) => mods,
            Err(e) => {
                log::warn!("online: cannot read the library for names — {e}");
                return looks;
            }
        };
        for m in mods {
            let id = m.id_interne.to_lowercase();
            let name = m.display_name.clone().unwrap_or_else(|| m.id_interne.clone());
            match ModKind::from_column(&m.kind) {
                ModKind::Car if cars.contains(&id) => {
                    looks.cars.insert(id, name);
                }
                ModKind::Track if tracks.contains(&id) => {
                    let detail = crate::library::track_layouts_detail(conn, cfg, &m);
                    let layouts = detail
                        .layouts
                        .into_iter()
                        .map(|l| {
                            (
                                l.id.to_lowercase(),
                                LayoutLook {
                                    name: l.name,
                                    preview: l.preview,
                                    outline: l.outline,
                                },
                            )
                        })
                        .collect();
                    let categories = m.categories.clone();
                    looks.tracks.insert(
                        id,
                        TrackLook {
                            name,
                            categories,
                            layouts,
                        },
                    );
                }
                _ => {}
            }
        }
        if let Some(ac) = cfg.ac_install_path.as_deref() {
            let mut names = crate::sun::track_names(ac);
            for id in named.iter().filter(|id| !looks.tracks.contains_key(*id)) {
                if let Some(name) = names.remove(id) {
                    looks.track_names.insert(id.clone(), name);
                }
            }
        }
        looks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SPEC-play-online, "Vignette d'un circuit non installé": a track the
    // library does not hold is named by CSP's table - whether it can be
    // driven or not -, never given a picture here.
    #[test]
    fn a_track_the_library_lacks_takes_csp_name() {
        let base = crate::testutil::temp_dir("looks-csp");
        let config = base.join("ac").join("extension").join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("data_track_params.ini"),
            "[pk_akina]
LATITUDE=36.4
NAME=PK Akina
",
        )
        .unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(base.join("ac")),
            ..Default::default()
        };
        let none = HashSet::new();
        let named = HashSet::from(["pk_akina".to_string(), "nohesi_415".to_string()]);
        let looks = Looks::scan(&conn, &cfg, &none, &none, &named);
        assert_eq!(looks.track_names.get("pk_akina").map(String::as_str), Some("PK Akina"));
        assert!(
            !looks.track_names.contains_key("nohesi_415"),
            "a track no table names keeps its id"
        );
        assert!(looks.tracks.is_empty(), "no picture without the library");
    }
}
