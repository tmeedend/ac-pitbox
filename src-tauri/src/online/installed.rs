//! What this machine has, to judge a server against it (the "Contenu
//! manquant" section of `SPEC-play-online.md`).
//!
//! Each car and each track layout is known with **where** it is — in the game,
//! only in the library, or in the showcase without its files — because that is
//! what tells "ready" from "one click" from "to download" (`readiness.rs`).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;

use super::lobby::ServerSummary;
use super::readiness::{self, Blocker, Level};

/// Where a car or a layout is, best first: the derived order is what keeps the
/// best of two sightings (a mod both in the game and in the library is in the
/// game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Presence {
    /// In `content/`: the game can load it now.
    Game,
    /// In the library with its files: `launch::ensure_available` lays it in
    /// the game at join time.
    Library,
    /// In the showcase (ESPACE§4.1): known, but its files are gone.
    Showcase,
}

/// Every car and every track layout this machine knows. Ids are lowercase:
/// Windows folders ignore case, and a server may write `KS_Audi_R8`.
#[derive(Debug, Default)]
pub struct Installed {
    cars: HashMap<String, Presence>,
    /// Track folder → layout → presence, `""` for a single-layout track
    /// (`inspect::track_layouts`' convention).
    tracks: HashMap<String, HashMap<String, Presence>>,
    /// The installed Custom Shaders Patch build, `None` without CSP.
    csp_build: Option<u32>,
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

/// Keeps the best of two sightings.
fn put(map: &mut HashMap<String, Presence>, key: String, presence: Presence) {
    map.entry(key)
        .and_modify(|p| *p = (*p).min(presence))
        .or_insert(presence);
}

impl Installed {
    /// Reads `content/`, the library and the CSP version. A failure on either
    /// side leaves that side empty and is logged: the list still shows, only
    /// less of it looks ready.
    pub fn scan(conn: &Connection, cfg: &AppConfig) -> Self {
        let mut installed = Self::default();
        if let Some(ac) = cfg.ac_install_path.as_ref() {
            let content = ac.join("content");
            for car in subdirs(&content.join("cars")) {
                put(&mut installed.cars, car.to_lowercase(), Presence::Game);
            }
            let tracks = content.join("tracks");
            for track in subdirs(&tracks) {
                let layouts = crate::inspect::track_layouts(&tracks.join(&track));
                installed.add_track(&track, layouts, Presence::Game);
            }
            installed.csp_build = readiness::csp_build(ac);
        }
        match crate::overlay::list_mods(conn) {
            Ok(mods) => {
                for m in mods {
                    let presence = if crate::skeleton::is_showcase(conn, &m.id_interne).unwrap_or(true) {
                        Presence::Showcase
                    } else {
                        Presence::Library
                    };
                    match ModKind::from_column(&m.kind) {
                        ModKind::Car => put(&mut installed.cars, m.id_interne.to_lowercase(), presence),
                        ModKind::Track => installed.add_track(&m.id_interne, m.layouts, presence),
                    }
                }
            }
            Err(e) => log::warn!("online: cannot read the library — {e}"),
        }
        installed
    }

    fn add_track(&mut self, id: &str, layouts: Vec<String>, presence: Presence) {
        let entry = self.tracks.entry(id.to_lowercase()).or_default();
        for layout in layouts {
            put(entry, layout.to_lowercase(), presence);
        }
    }

    #[cfg(test)]
    pub(super) fn add_car_for_tests(&mut self, id: &str, presence: Presence) {
        put(&mut self.cars, id.to_lowercase(), presence);
    }

    pub fn car(&self, id: &str) -> Option<Presence> {
        self.cars.get(&id.to_lowercase()).copied()
    }

    fn layout(&self, id: &str, layout: &str) -> Option<Presence> {
        self.tracks
            .get(&id.to_lowercase())?
            .get(&layout.to_lowercase())
            .copied()
    }

    /// Splits a `folder-layout` id against what is known, the way CM's
    /// `GetLayoutByKunosId` does: the whole id as a single-layout track first
    /// (`trento-bondone` is a folder), then every hyphen from the right.
    /// `None` when nothing known matches.
    fn resolve_track(&self, kunos_id: &str) -> Option<(String, Option<String>, Presence)> {
        if let Some(p) = self.layout(kunos_id, "") {
            return Some((kunos_id.to_string(), None, p));
        }
        let mut end = kunos_id.len();
        while let Some(i) = kunos_id[..end].rfind('-') {
            let (id, layout) = (&kunos_id[..i], &kunos_id[i + 1..]);
            if !id.is_empty() {
                if let Some(p) = self.layout(id, layout) {
                    return Some((id.to_string(), Some(layout.to_string()), p));
                }
            }
            end = i;
        }
        None
    }

    /// The level of a car here, with the DLC to name when it blocks.
    pub fn car_level(&self, id: &str) -> (Level, Option<String>) {
        readiness::content_level(self.car(id), ModKind::Car, id)
    }

    /// Fills what the lobby cannot know: how ready the track, the cars and the
    /// CSP are here — and, when the track is known, the right split of its id.
    pub fn judge(&self, server: &mut ServerSummary) {
        let mut blockers = Vec::new();

        let track_level = match self.resolve_track(&server.track.kunos_id) {
            Some((id, layout, presence)) => {
                server.track.id = id;
                server.track.layout = layout;
                readiness::content_level(Some(presence), ModKind::Track, &server.track.id).0
            }
            // The folder without this layout: something to fetch, whatever
            // the pack — official layouts ship together, so it is a mod.
            None if self.tracks.contains_key(&server.track.id.to_lowercase()) => Level::Download,
            None => {
                let (level, dlc) = readiness::content_level(None, ModKind::Track, &server.track.id);
                blockers.extend(dlc.map(|name| Blocker::Dlc { name }));
                level
            }
        };

        let car_levels: Vec<(Level, Option<String>)> = server.cars.iter().map(|c| self.car_level(c)).collect();
        // Every car of the server is needed, not only the one driven: the
        // game loads the whole entry list, and one car missing keeps the
        // player out (the user's experience, 2026-10-04 — CM's own
        // "missing cars" check reads every car too). So the server is as
        // ready as its worst car — its AI traffic included.
        let car_level = car_levels
            .iter()
            .map(|(level, _)| *level)
            .max()
            .unwrap_or(Level::Download);
        for (level, dlc) in &car_levels {
            if let (Level::Blocked, Some(name)) = (level, dlc) {
                if !blockers
                    .iter()
                    .any(|b| matches!(b, Blocker::Dlc { name: n } if n == name))
                {
                    blockers.push(Blocker::Dlc { name: name.clone() });
                }
            }
        }

        if let Some(required) = server.track.csp_min_build {
            if self.csp_build.is_none_or(|installed| installed < required) {
                blockers.push(Blocker::Csp {
                    required,
                    installed: self.csp_build,
                });
            }
        }

        server.track_level = track_level;
        server.track_available = track_level <= Level::OneClick;
        server.cars_available = car_levels.iter().filter(|(level, _)| *level <= Level::OneClick).count() as u32;
        server.level = if blockers.iter().any(|b| matches!(b, Blocker::Csp { .. })) {
            Level::Blocked
        } else {
            track_level.max(car_level)
        };
        server.blockers = blockers;
        server.official = is_official_content(server);
    }
}

/// The track and every car are official content (`kunos_dates`), owned here
/// or not. A server without a car names nothing to judge, and is not.
fn is_official_content(server: &ServerSummary) -> bool {
    let official = |kind, id: &str| crate::kunos_dates::is_official(kind, &id.to_lowercase());
    official(ModKind::Track, &server.track.id)
        && !server.cars.is_empty()
        && server.cars.iter().all(|c| official(ModKind::Car, c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::lobby::decode_track;

    fn installed() -> Installed {
        let mut i = Installed::default();
        put(&mut i.cars, "ks_mazda_miata".into(), Presence::Game);
        put(&mut i.cars, "rss_gtm_lanzo_v8".into(), Presence::Library);
        put(&mut i.cars, "old_mod".into(), Presence::Showcase);
        i.add_track(
            "ks_nordschleife",
            vec!["nordschleife".into(), "touristenfahrten".into()],
            Presence::Game,
        );
        i.add_track("trento-bondone", vec!["".into()], Presence::Game);
        i.add_track("monza", vec!["".into()], Presence::Game);
        i.add_track("rt_suzuka", vec!["".into()], Presence::Library);
        i
    }

    fn server(track: &str, cars: &[&str]) -> ServerSummary {
        crate::online::lobby::parse_server(&serde_json::json!({
            "ip": "1.2.3.4", "cport": 8081, "tport": 9600, "track": track, "cars": cars
        }))
        .unwrap()
    }

    /// Rule (CM's `GetLayoutByKunosId`): a hyphen may belong to the folder
    /// name, so the whole id is tried as a track before any split.
    #[test]
    fn a_hyphen_in_a_folder_name_is_not_a_layout() {
        assert_eq!(
            installed().resolve_track("trento-bondone"),
            Some(("trento-bondone".into(), None, Presence::Game)),
            "the folder itself, single layout"
        );
        assert_eq!(
            installed().resolve_track("ks_nordschleife-touristenfahrten"),
            Some((
                "ks_nordschleife".into(),
                Some("touristenfahrten".into()),
                Presence::Game
            )),
            "split on the hyphen that names an installed layout"
        );
    }

    /// Rule: a track is available only in the layout the server runs.
    #[test]
    fn another_layout_of_an_installed_track_is_not_available() {
        let mut s = server("ks_nordschleife-endurance", &["ks_mazda_miata", "unknown_car"]);
        installed().judge(&mut s);
        assert!(!s.track_available, "endurance is not installed");
        assert_eq!(s.track_level, Level::Download, "a layout to fetch, not a DLC");
        assert_eq!(s.cars_available, 1, "one car of two");
        assert_eq!(s.track, decode_track("ks_nordschleife-endurance"), "guess kept as is");
    }

    /// Rule (SPEC-play-online, "Contenu manquant"): in the game is ready, in
    /// the library one click, in the showcase to download — and the server is
    /// as ready as the worst of its track and every one of its cars.
    #[test]
    fn levels_follow_where_the_content_is() {
        let i = installed();
        let mut ready = server("monza", &["ks_mazda_miata"]);
        i.judge(&mut ready);
        assert_eq!(ready.level, Level::Ready);

        let mut one_click = server("rt_suzuka", &["ks_mazda_miata"]);
        i.judge(&mut one_click);
        assert_eq!(one_click.level, Level::OneClick, "the track is only in the library");

        let mut showcase = server("monza", &["old_mod"]);
        i.judge(&mut showcase);
        assert_eq!(showcase.level, Level::Download, "a showcase car has no files");
        assert_eq!(showcase.cars_available, 0);

        let mut mixed = server("monza", &["ks_mazda_miata", "rss_gtm_lanzo_v8", "old_mod"]);
        i.judge(&mut mixed);
        assert_eq!(
            mixed.level,
            Level::Download,
            "every car is needed: one to download keeps the server out, however ready the others"
        );
        assert_eq!(mixed.cars_available, 2);
    }

    /// Rule: missing Kunos DLC content blocks, and names the DLC.
    #[test]
    fn a_missing_dlc_track_blocks_with_its_name() {
        let mut s = server("ks_barcelona-layout_gp", &["ks_mazda_miata"]);
        installed().judge(&mut s);
        assert_eq!(s.level, Level::Blocked);
        assert!(
            s.blockers
                .iter()
                .any(|b| matches!(b, Blocker::Dlc { name } if !name.is_empty())),
            "the DLC is named: {:?}",
            s.blockers
        );
    }

    /// Rule: a server that requires a newer CSP than the installed one, or CSP
    /// when there is none, blocks — whatever the content.
    #[test]
    fn a_csp_requirement_blocks() {
        let mut i = installed();
        i.csp_build = Some(3000);
        let mut s = server("csp/3465/../monza", &["ks_mazda_miata"]);
        i.judge(&mut s);
        assert_eq!(s.level, Level::Blocked);
        assert_eq!(
            s.blockers,
            vec![Blocker::Csp {
                required: 3465,
                installed: Some(3000)
            }]
        );

        i.csp_build = Some(4157);
        let mut s = server("csp/3465/../monza", &["ks_mazda_miata"]);
        i.judge(&mut s);
        assert_eq!(s.level, Level::Ready, "a newer CSP passes");
    }

    /// Rule (SPEC-play-online.md, "Filtres de base", `content: kunos`): a
    /// server is official when its track and every one of its cars are Kunos
    /// content — owned or not, a DLC counts — and one mod is enough to make it
    /// a mod server.
    #[test]
    fn official_means_track_and_every_car_are_kunos() {
        let i = installed();
        let mut kunos = server("monza", &["KS_Mazda_Miata"]);
        i.judge(&mut kunos);
        assert!(kunos.official, "Kunos track and car, whatever their case");
        let mut dlc = server("ks_barcelona-layout_gp", &["ks_mazda_miata"]);
        i.judge(&mut dlc);
        assert!(dlc.official, "a DLC track not owned here is still official");
        let mut modded = server("monza", &["ks_mazda_miata", "rss_gtm_lanzo_v8"]);
        i.judge(&mut modded);
        assert!(!modded.official, "one mod car makes a mod server");
        let mut mod_track = server("rt_suzuka", &["ks_mazda_miata"]);
        i.judge(&mut mod_track);
        assert!(!mod_track.official, "a mod track makes a mod server");
    }

    /// Rule: ids compare without case — Windows folders do.
    #[test]
    fn case_does_not_matter() {
        assert_eq!(installed().car("KS_Mazda_Miata"), Some(Presence::Game));
        assert!(installed().resolve_track("MONZA").is_some());
    }
}
