//! The Assetto Corsa content a story races on (SPEC-story-mode, Content:
//! story package and dependencies): every event's track and car, and every
//! opponent's own car. What a package carries of it, and what a player must
//! already have, is the app's to decide - only it knows the game's own
//! content and the player's library.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::Story;

/// A kind of game content a story names by folder id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcKind {
    Car,
    Track,
}

impl AcKind {
    /// Its folder under the game's `content/`.
    pub fn folder(self) -> &'static str {
        match self {
            AcKind::Car => "cars",
            AcKind::Track => "tracks",
        }
    }
}

/// Every car and track the story's events name, by kind then id.
pub fn needed(story: &Story) -> BTreeSet<(AcKind, String)> {
    let mut out = BTreeSet::new();
    for event in &story.events {
        let s = &event.session;
        if let Some(track) = &s.track {
            out.insert((AcKind::Track, track.clone()));
        }
        let opponents = s.opponents.iter().filter_map(|o| o.car.as_ref());
        for car in s.car.iter().chain(opponents) {
            out.insert((AcKind::Car, car.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // The sample races on the base game alone: four cars (one an opponent's
    // own) and four tracks.
    #[test]
    fn a_story_needs_its_events_tracks_and_cars() {
        let story = Story::from_sources(crate::sample::files()).unwrap();
        let needed: Vec<String> = (needed(&story).into_iter())
            .map(|(k, id)| format!("{}/{id}", k.folder()))
            .collect();
        assert_eq!(
            needed,
            [
                "cars/abarth500",
                "cars/bmw_1m",
                "cars/bmw_m3_e30",
                "cars/lotus_elise_sc",
                "tracks/imola",
                "tracks/ks_vallelunga",
                "tracks/magione",
                "tracks/mugello"
            ]
        );
    }
}
