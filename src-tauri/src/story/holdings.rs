//! What this machine holds of the cars and tracks a story races on
//! (SPEC-story-mode, Content: story package and dependencies): where each
//! is - the library, or the game's `content/` - and whether it is the game's
//! own, with the DLC it comes from.
//!
//! What a package carries (`studio::package`), what an install imports
//! (`packages.rs`) and what Story Mode says is missing (`play.rs`) all ask
//! here. The base is read in one short call, never held while files are
//! read: the callers take the answer, release the lock, then work on the
//! folders.

use std::collections::BTreeSet;
use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;
use story_engine::needs::AcKind;

use crate::config::AppConfig;
use crate::modscan::ModKind;

/// One car or track, as this machine holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holding {
    pub kind: AcKind,
    pub id: String,
    /// Its folder: the library's version in play, or the game's own;
    /// `None`, not here (or a showcase entry without its files).
    pub folder: Option<PathBuf>,
    /// The game's own content, base game or DLC: never carried by a package.
    pub official: bool,
    /// The DLC it comes from, as the store names it; `None` for the base
    /// game or a mod.
    pub dlc: Option<String>,
}

/// How the callers ask: the cars and tracks wanted, in, their holdings out,
/// in the same order.
pub type Lookup<'a> = &'a dyn Fn(&BTreeSet<(AcKind, String)>) -> Vec<Holding>;

fn mod_kind(kind: AcKind) -> ModKind {
    match kind {
        AcKind::Car => ModKind::Car,
        AcKind::Track => ModKind::Track,
    }
}

/// The holdings of `wanted`, from the library and the game's folder.
pub fn look_up(conn: &Connection, cfg: &AppConfig, wanted: &BTreeSet<(AcKind, String)>) -> Vec<Holding> {
    wanted
        .iter()
        .map(|(kind, id)| {
            let lower = id.to_lowercase();
            Holding {
                kind: *kind,
                id: id.clone(),
                folder: library_folder(conn, cfg, *kind, id).or_else(|| game_folder(cfg, *kind, id)),
                official: crate::kunos_dates::is_official(mod_kind(*kind), &lower),
                dlc: crate::kunos_dates::pack_name(mod_kind(*kind), &lower),
            }
        })
        .collect()
}

/// The folder of the library's version of `id`, when the library has it
/// with its files.
fn library_folder(conn: &Connection, cfg: &AppConfig, kind: AcKind, id: &str) -> Option<PathBuf> {
    let row = match crate::overlay::get_mod(conn, id) {
        Ok(row) => row?,
        Err(e) => {
            log::warn!("story: {id} not read from the library: {e}");
            return None;
        }
    };
    if ModKind::from_column(&row.kind) != mod_kind(kind) || crate::skeleton::is_showcase(conn, id).unwrap_or(true) {
        return None;
    }
    crate::library::folder_path(conn, cfg, id).ok().filter(|f| f.is_dir())
}

fn game_folder(cfg: &AppConfig, kind: AcKind, id: &str) -> Option<PathBuf> {
    let folder = cfg
        .ac_install_path
        .as_ref()?
        .join("content")
        .join(kind.folder())
        .join(id);
    folder.is_dir().then_some(folder)
}

/// Every car and track wanted, held in a folder that is not checked: what a
/// test needs when it is about something else than holdings.
#[cfg(test)]
pub(crate) fn all_held(wanted: &BTreeSet<(AcKind, String)>) -> Vec<Holding> {
    held_as(wanted, |_| Some(PathBuf::from("held")))
}

/// The holdings of `wanted`, each in the folder `folder` gives (none: not
/// here), none of it the game's own.
#[cfg(test)]
pub(crate) fn held_as(
    wanted: &BTreeSet<(AcKind, String)>,
    folder: impl Fn(&(AcKind, String)) -> Option<PathBuf>,
) -> Vec<Holding> {
    (wanted.iter())
        .map(|w| Holding {
            kind: w.0,
            id: w.1.clone(),
            folder: folder(w),
            official: false,
            dlc: None,
        })
        .collect()
}
