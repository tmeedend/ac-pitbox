//! How ready a server is to be joined from here (`SPEC-play-online.md`,
//! "Contenu manquant : prêt à rejoindre").
//!
//! Four levels, best first, and a server is as ready as the worst of what it
//! needs: its track, the car one joins with, and the CSP it requires. What
//! downloads would bring, and the layers that would have to step aside, are
//! later lots — a server that needs either reads "to download" or "ready" for
//! now.

use std::path::Path;

use serde::Serialize;

use crate::modscan::ModKind;

use super::installed::Presence;

/// The derived order is the spec's, best first: comparing two levels gives the
/// worse with `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    /// Everything is in the game.
    Ready,
    /// Something is only in the library: joining lays it in the game first.
    OneClick,
    /// Something is not here, or here without its files.
    #[default]
    Download,
    /// Something cannot be had from here: a Kunos DLC not owned, a CSP too old.
    Blocked,
}

/// Why a server is blocked — what the screen names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Blocker {
    /// Official content missing here: a DLC not owned. Its name is the one
    /// the store sells it under (`kunos_content_dates.json`).
    Dlc { name: String },
    /// The server requires CSP at least `required`; `installed` is `None`
    /// without CSP.
    Csp { required: u32, installed: Option<u32> },
}

/// The level of one car or track, and the DLC to name when it blocks.
///
/// Absent official content belongs to a DLC the user does not own — had it
/// been bought, Steam would have put it in `content/`. Absent base-game
/// content (deleted by hand) and absent mods are something to fetch.
pub fn content_level(presence: Option<Presence>, kind: ModKind, id: &str) -> (Level, Option<String>) {
    match presence {
        Some(Presence::Game) => (Level::Ready, None),
        Some(Presence::Library) => (Level::OneClick, None),
        Some(Presence::Showcase) => (Level::Download, None),
        None => match crate::kunos_dates::pack_name(kind, &id.to_lowercase()) {
            Some(dlc) => (Level::Blocked, Some(dlc)),
            None => (Level::Download, None),
        },
    }
}

/// The installed CSP build, read where Content Manager reads it:
/// `extension/config/data_manifest.ini`, `SHADERS_PATCH_BUILD=4157`. `None`
/// without CSP or without a readable build.
pub fn csp_build(ac: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(ac.join("extension").join("config").join("data_manifest.ini")).ok()?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("SHADERS_PATCH_BUILD="))
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: the CSP build is read from the manifest CSP ships; no manifest
    /// means no CSP.
    #[test]
    fn the_csp_build_is_read_from_its_manifest() {
        let ac = crate::testutil::temp_dir("online-csp");
        assert_eq!(csp_build(&ac), None, "no CSP");
        let config = ac.join("extension").join("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("data_manifest.ini"),
            "[ℹ]\r\nFULLNAME=Information\r\n[VERSION]\r\nSHADERS_PATCH=0.3.0-preview622\r\nSHADERS_PATCH_BUILD=4157\r\n",
        )
        .unwrap();
        assert_eq!(csp_build(&ac), Some(4157));
    }

    /// Rule: absent official DLC content blocks and is named; an absent mod
    /// is something to fetch.
    #[test]
    fn absent_content_is_a_dlc_or_a_download() {
        let (level, dlc) = content_level(None, ModKind::Track, "ks_barcelona");
        assert_eq!(level, Level::Blocked);
        assert!(dlc.is_some(), "the DLC is named");
        assert_eq!(
            content_level(None, ModKind::Car, "rss_gtm_lanzo_v8"),
            (Level::Download, None)
        );
        assert_eq!(
            content_level(None, ModKind::Car, "abarth500"),
            (Level::Download, None),
            "base game content deleted by hand"
        );
    }

    /// Rule: levels compare best first, so `max` gives the worse.
    #[test]
    fn levels_order_from_best_to_worst() {
        assert_eq!(Level::Ready.max(Level::OneClick), Level::OneClick);
        assert_eq!(Level::Download.max(Level::Blocked), Level::Blocked);
    }
}
