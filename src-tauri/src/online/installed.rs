//! What can be driven here, to judge a server against it (the "Contenu
//! manquant" section of `SPEC-play-online.md`).
//!
//! Lot 1 answers a single question per car and per track: can Pit Box put it
//! in the game? Yes when it is already in `content/`, or when the library holds
//! it with its files (`launch::ensure_available` activates it at join time). A
//! mod in the showcase has no files, so it does not count. The finer levels of
//! the spec (download, other version, DLC) come later.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;

use super::lobby::ServerSummary;

/// Every car and every track (with its layouts) that can be driven. Ids are
/// lowercase: Windows folders ignore case, and a server may write `KS_Audi_R8`.
#[derive(Debug, Default)]
pub struct Installed {
    cars: HashSet<String>,
    /// Track folder → its layouts, `""` for a single-layout track
    /// (`inspect::track_layouts`' convention).
    tracks: HashMap<String, HashSet<String>>,
}

fn subdirs(dir: &Path) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect(),
        Err(e) => {
            log::warn!("online: cannot list {} — {e}", dir.display());
            Vec::new()
        }
    }
}

impl Installed {
    /// Reads `content/` and the library. A failure on either side leaves that
    /// side empty and is logged: the list still shows, only less of it looks
    /// ready.
    pub fn scan(conn: &Connection, cfg: &AppConfig) -> Self {
        let mut installed = Self::default();
        if let Some(ac) = cfg.ac_install_path.as_ref() {
            let content = ac.join("content");
            for car in subdirs(&content.join("cars")) {
                installed.cars.insert(car.to_lowercase());
            }
            let tracks = content.join("tracks");
            for track in subdirs(&tracks) {
                let layouts = crate::inspect::track_layouts(&tracks.join(&track));
                installed.add_track(&track, layouts);
            }
        }
        match crate::overlay::list_mods(conn) {
            Ok(mods) => {
                for m in mods {
                    let id = m.id_interne.to_lowercase();
                    let known = match ModKind::from_column(&m.kind) {
                        ModKind::Car => installed.cars.contains(&id),
                        ModKind::Track => installed.tracks.contains_key(&id),
                    };
                    // Already in the game: nothing more to learn. Otherwise it
                    // counts only with its files (ESPACE§4.1).
                    if known || crate::skeleton::is_showcase(conn, &m.id_interne).unwrap_or(true) {
                        continue;
                    }
                    match ModKind::from_column(&m.kind) {
                        ModKind::Car => {
                            installed.cars.insert(id);
                        }
                        ModKind::Track => installed.add_track(&id, m.layouts),
                    }
                }
            }
            Err(e) => log::warn!("online: cannot read the library — {e}"),
        }
        installed
    }

    fn add_track(&mut self, id: &str, layouts: Vec<String>) {
        let entry = self.tracks.entry(id.to_lowercase()).or_default();
        entry.extend(layouts.into_iter().map(|l| l.to_lowercase()));
    }

    #[cfg(test)]
    pub(super) fn add_car_for_tests(&mut self, id: &str) {
        self.cars.insert(id.to_lowercase());
    }

    pub fn has_car(&self, id: &str) -> bool {
        self.cars.contains(&id.to_lowercase())
    }

    fn has_layout(&self, id: &str, layout: &str) -> bool {
        self.tracks
            .get(&id.to_lowercase())
            .is_some_and(|layouts| layouts.contains(&layout.to_lowercase()))
    }

    /// Splits a `folder-layout` id against what is installed, the way CM's
    /// `GetLayoutByKunosId` does: the whole id as a single-layout track first
    /// (`trento-bondone` is a folder), then every hyphen from the right.
    /// `None` when nothing installed matches.
    fn resolve_track(&self, kunos_id: &str) -> Option<(String, Option<String>)> {
        if self.has_layout(kunos_id, "") {
            return Some((kunos_id.to_string(), None));
        }
        let mut end = kunos_id.len();
        while let Some(i) = kunos_id[..end].rfind('-') {
            let (id, layout) = (&kunos_id[..i], &kunos_id[i + 1..]);
            if !id.is_empty() && self.has_layout(id, layout) {
                return Some((id.to_string(), Some(layout.to_string())));
            }
            end = i;
        }
        None
    }

    /// Fills what the lobby cannot know: whether the track and how many of the
    /// cars can be driven here — and, when the track is installed, the right
    /// split of its id.
    pub fn judge(&self, server: &mut ServerSummary) {
        if let Some((id, layout)) = self.resolve_track(&server.track.kunos_id) {
            server.track.id = id;
            server.track.layout = layout;
            server.track_available = true;
        } else {
            server.track_available = false;
        }
        server.cars_available = server.cars.iter().filter(|c| self.has_car(c)).count() as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::lobby::decode_track;

    fn installed() -> Installed {
        let mut i = Installed::default();
        i.cars.insert("ks_mazda_miata".into());
        i.add_track(
            "ks_nordschleife",
            vec!["nordschleife".into(), "touristenfahrten".into()],
        );
        i.add_track("trento-bondone", vec!["".into()]);
        i.add_track("monza", vec!["".into()]);
        i
    }

    /// Rule (CM's `GetLayoutByKunosId`): a hyphen may belong to the folder
    /// name, so the whole id is tried as a track before any split.
    #[test]
    fn a_hyphen_in_a_folder_name_is_not_a_layout() {
        assert_eq!(
            installed().resolve_track("trento-bondone"),
            Some(("trento-bondone".into(), None)),
            "the folder itself, single layout"
        );
        assert_eq!(
            installed().resolve_track("ks_nordschleife-touristenfahrten"),
            Some(("ks_nordschleife".into(), Some("touristenfahrten".into()))),
            "split on the hyphen that names an installed layout"
        );
    }

    /// Rule: a track is available only in the layout the server runs.
    #[test]
    fn another_layout_of_an_installed_track_is_not_available() {
        let mut server = crate::online::lobby::parse_server(&serde_json::json!({
            "ip": "1.2.3.4", "cport": 8081, "tport": 9600,
            "track": "ks_nordschleife-endurance", "cars": ["ks_mazda_miata", "rss_gtm_lanzo_v8"]
        }))
        .unwrap();
        installed().judge(&mut server);
        assert!(!server.track_available, "endurance is not installed");
        assert_eq!(server.cars_available, 1, "one car of two");
        assert_eq!(
            server.track,
            decode_track("ks_nordschleife-endurance"),
            "guess kept as is"
        );
    }

    /// Rule: ids compare without case — Windows folders do.
    #[test]
    fn case_does_not_matter() {
        assert!(installed().has_car("KS_Mazda_Miata"));
        assert!(installed().resolve_track("MONZA").is_some());
    }
}
