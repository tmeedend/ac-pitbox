//! Mod updates, from the registry Content Manager reads (§4.7).
//!
//! **Where the information comes from.** Not from the mods, nor from their
//! authors' pages: from **CUP** ("Content Update Provider"), a registry kept by
//! the author of Content Manager, where modders declare their content and
//! publish each new version. One request, `https://acstuff.club/cup/`, returns
//! a JSON object of about 40 KB — per content type, the latest version of every
//! registered id:
//!
//! ```json
//! { "car": { "hsrc_subaru_gc8": "1.71",
//!            "faz_audi_rs7_patreon": { "version": "0.7", "limited": true } },
//!   "track": { … }, "app": { … }, "luaapp": { … }, "filter": { … } }
//! ```
//!
//! The comparison with what is installed happens here, on the machine: the
//! registry never learns what the library contains. That is also why it is one
//! request for the whole library rather than one per mod.
//!
//! Per id, two more addresses: `/cup/<type>/<id>` gives the details (changelog,
//! author, information page, other ids the same archive updates), and
//! `/cup/<type>/<id>/get` **redirects to the archive** — or to wherever the
//! author hosts it. Measured on the two updates that motivated this module:
//! `hsrc_subaru_gc8` lands on a plain `.7z` served by acstuff, and
//! `lk_nissan_180sx_96` on a mega.nz page, which only a browser can download
//! (the decryption key lives in the URL fragment, the file is decrypted by
//! the page's own script). `limited` entries (128 cars out of 992 on
//! 2026-09-24, mostly paid Patreon mods) are never downloadable either.
//!
//! **The rule that follows: an archive or the browser.** Whatever `/get` leads
//! to is downloaded only if it *is* an archive — checked on the bytes, not on
//! a name — and anything else is handed to the browser, which follows the same
//! redirect with its fragment intact and shows the user the page that asks for
//! something. No list of file hosts to keep up to date: the answer is read on
//! what actually comes back.
//!
//! **Nothing here writes to the library or the game.** A downloaded archive is
//! handed to the ordinary import (§4.2), which recognises the new version of an
//! existing mod by itself (§4.1, §4.3) — the same path as dropping the archive
//! on the window. This module only knows where the file is.
//!
//! Pure except for `fetch_*` and `download_update`, which go through
//! `crate::http` and never touch the overlay: the façade reads the library
//! before calling, so the SQLite lock is never held across a network call —
//! same split as the Wikipedia client (`wiki/mod.rs`).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::http::{self, DownloadError};
use crate::overlay::ModRow;

/// The registry Content Manager queries by default
/// (`SettingsHolder.Content.CupRegistries`, acstuff.ru moved to acstuff.club).
const REGISTRY_HOST: &str = "acstuff.club";
const REGISTRY_ROOT: &str = "/cup";

/// Registry answers are small; a registry that takes longer than this is down.
const TIMEOUT_MS: i32 = 15_000;
/// Per read, not per transfer: a file host that goes silent for a minute has
/// dropped the connection, however large the archive.
const DOWNLOAD_TIMEOUT_MS: i32 = 60_000;

/// Prefix of the working folder that holds one downloaded archive. Named
/// because `discard_download` refuses to delete anything else.
const DOWNLOAD_PREFIX: &str = "pitbox-update-";

/// The registry's name for a library kind. Cars and tracks only: the registry
/// also lists Python and Lua apps, post-processing filters and showrooms, but
/// the library holds none of those as versioned mods — Pit Box's apps carry no
/// version to compare with.
fn cup_type(kind: &str) -> Option<&'static str> {
    match kind {
        "Car" => Some("car"),
        "Track" => Some("track"),
        _ => None,
    }
}

/// Identifies the app to the registry and to file hosts, like the Wikipedia
/// client does (WIKI§6.2): name, version, where to reach the project.
fn user_agent() -> String {
    format!(
        "PitBox/{} (+https://github.com/tmeedend/ac-pitbox)",
        env!("CARGO_PKG_VERSION")
    )
}

/// Latest version the registry knows for one id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latest {
    pub version: String,
    /// Content Manager's `IsToUpdateManually`: the author does not let it be
    /// downloaded by a program (paid content, sign-in wall). The update is
    /// still worth announcing — it opens the author's page.
    pub limited: bool,
}

/// `(registry type, lower-case id)` → latest version. Lower case because the
/// registry is case-insensitive on ids (`CupKey` lowers them), and folder names
/// on Windows are too.
pub type Registry = HashMap<(String, String), Latest>;

/// Reads the registry list. Entries come in two shapes — a bare version
/// string, or `{ "version", "limited" }` — and an entry in neither shape is
/// skipped rather than failing the whole list, as Content Manager does: one
/// author's typo must not hide everyone else's updates.
pub fn parse_registry(body: &[u8]) -> Option<Registry> {
    let root: Value = serde_json::from_slice(body).ok()?;
    let mut out = Registry::new();
    for (kind, entries) in root.as_object()? {
        let Some(entries) = entries.as_object() else { continue };
        for (id, entry) in entries {
            let latest = match entry {
                Value::String(version) => Latest {
                    version: version.clone(),
                    limited: false,
                },
                Value::Object(o) => match o.get("version").and_then(Value::as_str) {
                    Some(version) => Latest {
                        version: version.to_string(),
                        limited: o.get("limited").and_then(Value::as_bool).unwrap_or(false),
                    },
                    None => continue,
                },
                _ => continue,
            };
            out.insert((kind.to_ascii_lowercase(), id.to_lowercase()), latest);
        }
    }
    Some(out)
}

/// Compares two version labels **the way Content Manager does**
/// (`StringExtension.CompareAsVersionTo`), so that both apps agree on what is
/// an update: a leading `v` is dropped, the labels are cut on `.`, and each
/// segment is compared in natural order — digits as numbers, the rest as text.
/// A label with more segments wins a tie (`1.2.1` > `1.2`), and a missing
/// label is older than any other.
///
/// The consequence to know about: `1.5` is older than `1.46`, because `5 < 46`.
/// That is how authors number in practice (`1.46` → `1.71` is the real
/// `hsrc_subaru_gc8` update), and disagreeing with Content Manager here would
/// announce, or hide, updates it does not.
pub fn compare_versions(a: Option<&str>, b: Option<&str>) -> Ordering {
    let (a, b) = match (a, b) {
        (None, None) => return Ordering::Equal,
        (None, Some(_)) => return Ordering::Less,
        (Some(_), None) => return Ordering::Greater,
        (Some(a), Some(b)) => (a, b),
    };
    let split = |s: &str| -> Vec<String> {
        let s = s.trim();
        let s = s.strip_prefix('v').unwrap_or(s);
        s.split('.').map(|p| p.trim().to_string()).collect()
    };
    let (ap, bp) = (split(a), split(b));
    for (x, y) in ap.iter().zip(bp.iter()) {
        let c = natural_cmp(x, y);
        if c != Ordering::Equal {
            return c;
        }
    }
    ap.len().cmp(&bp.len())
}

/// Natural order: runs of digits compare as numbers (`9 < 10`), everything
/// else character by character, case-insensitively.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (ra, rb) = (digit_run(&mut a), digit_run(&mut b));
                // Compared as digit strings rather than parsed, so a
                // twenty-digit build number cannot overflow: without leading
                // zeros, the longer run is the larger number.
                let (ta, tb) = (ra.trim_start_matches('0'), rb.trim_start_matches('0'));
                let c = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if c != Ordering::Equal {
                    return c;
                }
            }
            (Some(x), Some(y)) => {
                let c = x.to_lowercase().cmp(y.to_lowercase());
                if c != Ordering::Equal {
                    return c;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn digit_run(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        run.push(c);
        it.next();
    }
    run
}

/// One mod of the library with a newer version in the registry. Mirrored by
/// `ModUpdate` in `src/lib/library/modUpdates.svelte.ts`, which sends its
/// list back to `still_pending`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdate {
    /// "Car" | "Track", as everywhere in the library.
    pub kind: String,
    pub id: String,
    pub name: Option<String>,
    /// Version label of the active version, `None` when its `ui_*.json` gives
    /// none — which the registry, like Content Manager, counts as older than
    /// anything it knows.
    pub installed: Option<String>,
    pub available: String,
    pub limited: bool,
}

/// Library mods the registry has something newer for.
///
/// Only mods **Pit Box manages** are candidates. Base content has nothing to
/// update, and a mod installed outside Pit Box (§8.2) cannot receive an import
/// on top of it (§4.3) — offering an update the import would then refuse
/// would be a button that fails. Content Manager still sees those.
pub fn find_updates(mods: &[ModRow], registry: &Registry) -> Vec<ModUpdate> {
    let mut out: Vec<ModUpdate> = mods
        .iter()
        .filter(|m| !m.is_stock && m.active_version_id.is_some())
        .filter_map(|m| {
            let latest = registry.get(&(cup_type(&m.kind)?.to_string(), m.id_interne.to_lowercase()))?;
            let installed = m.active_version_label.as_deref();
            (compare_versions(Some(&latest.version), installed) == Ordering::Greater).then(|| ModUpdate {
                kind: m.kind.clone(),
                id: m.id_interne.clone(),
                name: m.display_name.clone(),
                installed: installed.map(str::to_string),
                available: latest.version.clone(),
                limited: latest.limited,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        let key = |u: &ModUpdate| u.name.clone().unwrap_or_else(|| u.id.clone()).to_lowercase();
        key(a).cmp(&key(b))
    });
    out
}

/// The updates of `pending` the library still needs, against what is
/// installed **now** — without asking the registry again.
///
/// The registry is asked once a day, but the library changes in between: an
/// update the browser had to download (a mega.nz page, §4.7) is installed by
/// dropping the archive, and nothing tied that import to the announced update.
/// The banner kept offering a version already installed until the next day's
/// check. The versions the registry gave are still true; only the installed
/// side moved, so the same comparison (`find_updates`) is simply run again on
/// it. A mod updated to that version or beyond, removed, or no longer managed
/// drops out; one still behind stays, with its installed version refreshed.
pub fn still_pending(mods: &[ModRow], pending: &[ModUpdate]) -> Vec<ModUpdate> {
    let registry: Registry = pending
        .iter()
        .filter_map(|u| {
            let key = (cup_type(&u.kind)?.to_string(), u.id.to_lowercase());
            let latest = Latest {
                version: u.available.clone(),
                limited: u.limited,
            };
            Some((key, latest))
        })
        .collect();
    find_updates(mods, &registry)
}

/// What the registry says about one update, beyond its version number.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDetails {
    pub version: Option<String>,
    pub author: Option<String>,
    /// Free text from the author, in whatever language they wrote it — the
    /// `hsrc_subaru_gc8` one is in Russian. Shown as it is.
    pub changelog: Option<String>,
    /// The author's page (a Discord, an Instagram, a RaceDepartment thread).
    pub information_url: Option<String>,
    /// Other ids the same archive updates: `lk_nissan_180sx_96` also brings
    /// `lk_nissan_180sx_96_drift`. Informational only — the import finds every
    /// mod in the archive by itself.
    pub alternative_ids: Vec<String>,
}

pub fn parse_details(body: &[u8]) -> Option<UpdateDetails> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let text = |key: &str| {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(UpdateDetails {
        version: text("version"),
        author: text("author"),
        changelog: text("changelog"),
        information_url: text("informationUrl"),
        alternative_ids: v
            .get("alternativeIds")
            .and_then(Value::as_array)
            .map(|ids| ids.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
    })
}

/// Path of one entry in the registry. The id is encoded although registry ids
/// are folder names: a folder name may carry a space or a `#`, and neither
/// belongs raw in a path.
fn entry_path(cup: &str, id: &str) -> String {
    format!("{REGISTRY_ROOT}/{cup}/{}", http::encode_query_value(id))
}

/// Where `/get` lives for this mod — the address to download, and the one to
/// give the browser when the download is not an archive. `None` for a kind
/// the registry does not know.
pub fn download_url(kind: &str, id: &str) -> Option<String> {
    Some(format!(
        "https://{REGISTRY_HOST}{}/get",
        entry_path(cup_type(kind)?, id)
    ))
}

/// The whole registry. `Err` carries an error key (`errors.rs`): the user
/// asked, or the toast will say nothing, and either way the log says why.
pub fn fetch_registry() -> Result<Registry, &'static str> {
    let response = http::get(REGISTRY_HOST, &format!("{REGISTRY_ROOT}/"), &user_agent(), TIMEOUT_MS)
        .ok_or(crate::errors::UPDATE_REGISTRY_UNAVAILABLE)?;
    if response.status != 200 {
        log::warn!("cup: registry answered {}", response.status);
        return Err(crate::errors::UPDATE_REGISTRY_UNAVAILABLE);
    }
    parse_registry(&response.body).ok_or_else(|| {
        log::warn!("cup: registry list is not the JSON object expected");
        crate::errors::UPDATE_REGISTRY_UNAVAILABLE
    })
}

pub fn fetch_details(kind: &str, id: &str) -> Result<UpdateDetails, &'static str> {
    let cup = cup_type(kind).ok_or(crate::errors::UPDATE_REGISTRY_UNAVAILABLE)?;
    let response = http::get(REGISTRY_HOST, &entry_path(cup, id), &user_agent(), TIMEOUT_MS)
        .ok_or(crate::errors::UPDATE_REGISTRY_UNAVAILABLE)?;
    if response.status != 200 {
        log::warn!("cup: details of {cup}/{id} answered {}", response.status);
        return Err(crate::errors::UPDATE_REGISTRY_UNAVAILABLE);
    }
    parse_details(&response.body).ok_or_else(|| {
        log::warn!("cup: details of {cup}/{id} are not the JSON object expected");
        crate::errors::UPDATE_REGISTRY_UNAVAILABLE
    })
}

/// Archive format recognised on its first bytes, as the extension to give the
/// file — the import (and 7-Zip behind it) goes by what it is told.
fn sniff_archive(path: &Path) -> Option<&'static str> {
    use std::io::Read;
    let mut head = [0u8; 8];
    let mut file = std::fs::File::open(path).ok()?;
    let n = file.read(&mut head).ok()?;
    let head = &head[..n];
    if head.starts_with(b"PK\x03\x04") {
        Some("zip")
    } else if head.starts_with(b"7z\xBC\xAF\x27\x1C") {
        Some("7z")
    } else if head.starts_with(b"Rar!\x1A\x07") {
        Some("rar")
    } else {
        None
    }
}

/// How a download ended, as the frontend needs it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DownloadOutcome {
    /// Ready for the import. Named `<id>.<ext>` on purpose: an archive whose
    /// content sits at its root takes its identity from the archive name
    /// (`importer::incoming_name`, §4.3bis), and whatever the file host called
    /// it (`Nissan180SX_v1.38_FINAL.zip`) is not the mod's id.
    Archive {
        path: String,
    },
    /// Not an archive — a file host's page, a sign-in wall, an error. The URL
    /// is `/get` itself, so the browser follows the redirect with its fragment.
    Browser {
        url: String,
    },
    Cancelled,
}

/// Downloads the update of one mod into a fresh folder under `temp_root`.
///
/// `on_progress` follows `http::download`: bytes received, total when known,
/// `false` to stop. Every outcome but `Archive` leaves nothing on disk.
pub fn download_update(
    kind: &str,
    id: &str,
    temp_root: &Path,
    on_progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<DownloadOutcome, String> {
    let url = download_url(kind, id).ok_or(crate::errors::UPDATE_DOWNLOAD_FAILED)?;
    let dir = temp_root.join(format!("{DOWNLOAD_PREFIX}{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{id}.part"));

    let outcome = match http::download(&url, &part, &user_agent(), DOWNLOAD_TIMEOUT_MS, on_progress) {
        Ok(bytes) => match sniff_archive(&part) {
            Some(ext) => {
                let archive = dir.join(format!("{id}.{ext}"));
                std::fs::rename(&part, &archive).map_err(|e| e.to_string())?;
                log::info!("cup: {id} downloaded, {bytes} bytes");
                return Ok(DownloadOutcome::Archive {
                    path: archive.to_string_lossy().into_owned(),
                });
            }
            None => {
                log::warn!("cup: {url} gave {bytes} bytes that are no archive — handing over to the browser");
                Ok(DownloadOutcome::Browser { url })
            }
        },
        Err(DownloadError::Page) => Ok(DownloadOutcome::Browser { url }),
        // The file host refused (quota, expired link, sign-in): the page it
        // shows a person says why, which is more than a status code does.
        Err(DownloadError::Status(_)) => Ok(DownloadOutcome::Browser { url }),
        Err(DownloadError::Cancelled) => Ok(DownloadOutcome::Cancelled),
        // No network: a browser would not do better, so this one is an error.
        Err(DownloadError::Unreachable) => Err(crate::errors::UPDATE_DOWNLOAD_FAILED.to_string()),
        Err(DownloadError::Io(e)) => Err(e),
    };
    remove_download_dir(&dir);
    outcome
}

fn remove_download_dir(dir: &Path) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        log::warn!("cup: download folder {} left behind — {e}", dir.display());
    }
}

/// Deletes a downloaded archive and its folder once the import is done with
/// it. The path comes back from the frontend, so it is checked to be exactly
/// what `download_update` makes — a `pitbox-update-*` folder directly under
/// `temp_root` — and anything else is refused: this function must not be able
/// to delete a file the app did not download.
pub fn discard_download(archive: &Path, temp_root: &Path) -> Result<(), String> {
    let dir: PathBuf = archive
        .parent()
        .filter(|d| d.parent() == Some(temp_root))
        .filter(|d| {
            d.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(DOWNLOAD_PREFIX))
        })
        .ok_or_else(|| format!("not a downloaded update: {}", archive.display()))?
        .to_path_buf();
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

/// Removes the download folders a previous run left behind — a crash or a
/// forced close between download and import. Called once at startup, when
/// no download can be in progress, and only on `pitbox-update-*` folders.
pub fn sweep_leftovers(temp_root: &Path) {
    let Ok(entries) = std::fs::read_dir(temp_root) else {
        return;
    };
    for entry in entries.flatten() {
        let is_ours = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(DOWNLOAD_PREFIX));
        // `file_type` does not follow links: a junction someone named like
        // ours is not a folder of ours, and is left alone.
        if is_ours && entry.file_type().is_ok_and(|t| t.is_dir()) {
            remove_download_dir(&entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, id: &str, version: Option<&str>) -> ModRow {
        ModRow {
            id_interne: id.into(),
            kind: kind.into(),
            brand: None,
            display_name: Some(id.into()),
            year: None,
            car_class: None,
            category: None,
            categories: vec![],
            country: None,
            is_favorite: false,
            active_version_id: Some("v1".into()),
            version_count: 1,
            created_at: None,
            tags_from_mod: vec![],
            tags_from_rule: vec![],
            tags_manual: vec![],
            drivetrain: None,
            engine_pos: None,
            aspiration: None,
            engine_config: None,
            gearbox: None,
            source_pack: None,
            source_url: None,
            author: None,
            active_version_label: version.map(str::to_string),
            updated_at: None,
            layouts: vec![],
            csp_features: vec![],
            display_name_user: None,
            display_name_file: None,
            description_user: None,
            is_stock: false,
            is_unmanaged: false,
            notes_user: None,
            published_at: None,
            size_bytes: None,
        }
    }

    /// Rule (§4.7): the registry list is read in both of its shapes, keyed by
    /// type and lower-case id, and one malformed entry costs only itself.
    /// Shapes and values are the real ones, 2026-09-24.
    #[test]
    fn the_registry_reads_both_entry_shapes() {
        let body = br#"{
            "car": {
                "hsrc_subaru_gc8": "1.71",
                "LK_Nissan_180SX_96": "1.38",
                "faz_audi_rs7_patreon": { "version": "0.7", "limited": true },
                "broken_entry": 12,
                "no_version": { "limited": true }
            },
            "track": { "some_track": "2.0" },
            "filter": "not an object"
        }"#;
        let r = parse_registry(body).expect("valid list");
        let get = |t: &str, id: &str| r.get(&(t.to_string(), id.to_string())).cloned();
        assert_eq!(
            get("car", "hsrc_subaru_gc8"),
            Some(Latest {
                version: "1.71".into(),
                limited: false
            }),
            "bare version string"
        );
        assert_eq!(
            get("car", "faz_audi_rs7_patreon"),
            Some(Latest {
                version: "0.7".into(),
                limited: true
            }),
            "object with limited"
        );
        assert!(get("car", "lk_nissan_180sx_96").is_some(), "ids are lower-cased");
        assert!(get("car", "broken_entry").is_none(), "a number is not a version");
        assert!(
            get("car", "no_version").is_none(),
            "an object without version is skipped"
        );
        assert!(get("track", "some_track").is_some(), "tracks are read too");
        assert_eq!(r.len(), 4, "nothing else slipped in");
        assert!(parse_registry(b"<html>").is_none(), "a page is not a registry");
    }

    /// Rule (§4.7): versions compare exactly as in Content Manager — its own
    /// test cases (`StringExtensionTest.Version`), plus the two real updates.
    #[test]
    fn versions_compare_like_content_manager() {
        let gt = |a: &str, b: &str| compare_versions(Some(a), Some(b)) == Ordering::Greater;
        let lt = |a: &str, b: &str| compare_versions(Some(a), Some(b)) == Ordering::Less;
        assert!(gt("0.1.2", "0.0.8"));
        assert!(gt("0.1.2", "0.0test.8"));
        assert!(lt("0.1.2", "0.2.8"));
        assert!(lt("0.1.2", "0.q2.8"));
        assert!(gt("0.1.2", "0"));
        assert!(gt("1.1.2", "0"));
        assert!(lt("1.1.2", "2"));
        assert!(lt("1.8", "2.0"));
        assert!(gt("2.0", "1.8"));
        assert!(gt("1.38", "1.37"), "lk_nissan_180sx_96");
        assert!(gt("1.71", "1.46"), "hsrc_subaru_gc8");
        assert!(lt("1.5", "1.46"), "segments are numbers, not decimals");
        assert!(gt("1.10", "1.9"), "natural order, not text order");
        assert_eq!(
            compare_versions(Some("v1.2"), Some("1.2")),
            Ordering::Equal,
            "leading v"
        );
        assert_eq!(compare_versions(Some(" 1.2 "), Some("1.2")), Ordering::Equal, "spaces");
        assert!(gt("1.2.1", "1.2"), "more segments win a tie");
        assert_eq!(
            compare_versions(Some("1.0"), None),
            Ordering::Greater,
            "no label is older than any"
        );
        assert!(
            gt("1.123456789012345678901234", "1.9"),
            "a long digit run neither overflows nor loses"
        );
        assert_eq!(
            compare_versions(Some("1.007"), Some("1.7")),
            Ordering::Equal,
            "leading zeros do not count"
        );
    }

    /// Rule (§4.7): an update is announced only for a managed mod whose
    /// registry version is newer — never for base content, a mod installed
    /// outside Pit Box, the same version, or an older one.
    #[test]
    fn only_newer_versions_of_managed_mods_are_updates() {
        let mut registry = Registry::new();
        for (t, id, v) in [
            ("car", "newer", "1.71"),
            ("car", "same", "1.0"),
            ("car", "older", "1.0"),
            ("car", "stock", "9"),
            ("car", "unmanaged", "9"),
            ("car", "unversioned", "1.0"),
            ("track", "a_track", "2.0"),
            ("track", "same_id_other_kind", "9"),
        ] {
            registry.insert(
                (t.into(), id.into()),
                Latest {
                    version: v.into(),
                    limited: false,
                },
            );
        }
        let mut stock = row("Car", "stock", Some("1.0"));
        stock.is_stock = true;
        let mut unmanaged = row("Car", "unmanaged", Some("1.0"));
        unmanaged.is_stock = true;
        unmanaged.is_unmanaged = true;
        let mods = vec![
            row("Car", "Newer", Some("1.46")),
            row("Car", "same", Some("1.0")),
            row("Car", "older", Some("2.0")),
            stock,
            unmanaged,
            row("Car", "unversioned", None),
            row("Track", "a_track", Some("1.0")),
            row("Car", "same_id_other_kind", Some("1.0")),
        ];
        let ids: Vec<String> = find_updates(&mods, &registry).into_iter().map(|u| u.id).collect();
        assert_eq!(
            ids,
            vec!["a_track", "Newer", "unversioned"],
            "sorted by name, id matched case-insensitively, kind respected"
        );
    }

    /// Rule (§4.7): once the library changes, an announced update is weighed
    /// again against the installed version, without the registry. Real case:
    /// an update the browser had to download, installed by dropping the
    /// archive, stayed announced until the next day's check.
    #[test]
    fn an_update_installed_by_hand_is_no_longer_pending() {
        let pending = |id: &str, available: &str| ModUpdate {
            kind: "Track".into(),
            id: id.into(),
            name: Some(id.into()),
            installed: Some("1.0".into()),
            available: available.into(),
            limited: false,
        };
        let announced = vec![
            pending("installed_by_hand", "1.2"),
            pending("went_beyond", "1.2"),
            pending("still_behind", "1.2"),
            pending("removed", "1.2"),
        ];
        let mods = vec![
            row("Track", "Installed_By_Hand", Some("1.2")),
            row("Track", "went_beyond", Some("1.3")),
            row("Track", "still_behind", Some("1.1")),
            row("Track", "never_announced", None),
        ];
        let left = still_pending(&mods, &announced);
        assert_eq!(
            left.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
            vec!["still_behind"],
            "installed, overtaken or removed updates are gone; nothing new is invented"
        );
        assert_eq!(
            left[0].installed.as_deref(),
            Some("1.1"),
            "the installed version is the one read now"
        );
        assert_eq!(left[0].available, "1.2", "the registry's version is kept");
    }

    /// Rule (§4.7): the details are read from the real answer, alternative ids
    /// included, and blank fields count as absent.
    #[test]
    fn details_read_the_real_answer() {
        let body = br#"{"alternativeIds":["lk_nissan_180sx_96_drift"],"changelog":"- Changed tires\n- Misc. changes","author":"Lenny","informationUrl":"https://www.instagram.com/nclenny/","version":"1.38","active":true,"cleanInstallation":true,"name":"  "}"#;
        let d = parse_details(body).expect("valid details");
        assert_eq!(d.version.as_deref(), Some("1.38"));
        assert_eq!(d.author.as_deref(), Some("Lenny"));
        assert_eq!(d.alternative_ids, vec!["lk_nissan_180sx_96_drift".to_string()]);
        assert_eq!(d.information_url.as_deref(), Some("https://www.instagram.com/nclenny/"));
        assert!(d.changelog.unwrap().starts_with("- Changed tires"));
    }

    /// Rule (§4.7): `/get` is built for cars and tracks only, with the id
    /// encoded — a folder name is not guaranteed to be a clean path segment.
    #[test]
    fn the_download_address_is_the_registry_redirect() {
        assert_eq!(
            download_url("Car", "hsrc_subaru_gc8").as_deref(),
            Some("https://acstuff.club/cup/car/hsrc_subaru_gc8/get")
        );
        assert_eq!(
            download_url("Track", "my track#1").as_deref(),
            Some("https://acstuff.club/cup/track/my%20track%231/get")
        );
        assert_eq!(download_url("App", "x"), None, "apps are not looked up");
    }

    /// Rule (§4.7): an archive is recognised on its bytes, whatever it is
    /// called — a file host's page saved under an archive name is not one.
    #[test]
    fn an_archive_is_recognised_on_its_bytes() {
        let base = crate::testutil::temp_dir("cup-sniff");
        let write = |name: &str, bytes: &[u8]| {
            let p = base.join(name);
            std::fs::write(&p, bytes).unwrap();
            p
        };
        assert_eq!(sniff_archive(&write("a", b"PK\x03\x04rest")), Some("zip"));
        assert_eq!(sniff_archive(&write("b", b"7z\xBC\xAF\x27\x1C\x00\x04")), Some("7z"));
        assert_eq!(sniff_archive(&write("c", b"Rar!\x1A\x07\x01\x00")), Some("rar"));
        assert_eq!(
            sniff_archive(&write("d.zip", b"<!DOCTYPE html>")),
            None,
            "a page, whatever its name"
        );
        assert_eq!(sniff_archive(&write("e", b"PK")), None, "too short to tell");
    }

    /// Rule (§4.7): discarding a download deletes the folder `download_update`
    /// made, and nothing else — the path comes back from the frontend.
    #[test]
    fn discarding_a_download_never_reaches_beyond_it() {
        let base = crate::testutil::temp_dir("cup-discard");
        let dir = base.join(format!("{DOWNLOAD_PREFIX}abc"));
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("car.7z");
        std::fs::write(&archive, b"7z").unwrap();

        let other = base.join("mine");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("car.7z"), b"7z").unwrap();
        assert!(
            discard_download(&other.join("car.7z"), &base).is_err(),
            "a folder without the prefix is refused"
        );
        assert!(other.join("car.7z").is_file(), "and left untouched");

        let nested = dir.join("deeper");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(
            discard_download(&nested.join("x.zip"), &base).is_err(),
            "only a folder directly under the root"
        );
        assert!(
            discard_download(&archive, Path::new(r"C:\elsewhere")).is_err(),
            "under another root"
        );
        assert!(dir.is_dir(), "refusals delete nothing");

        discard_download(&archive, &base).expect("our own download");
        assert!(!dir.exists(), "folder removed with its archive");
        assert!(other.is_dir(), "the neighbour survives");
    }

    /// Against the live registry — not run by default (`cargo test -- --ignored
    /// live_`), since it needs the network and the registry's answers change
    /// with every author's release. It proves what no offline test can: that
    /// the WinHTTP download follows `/get` to a file host, streams an archive,
    /// stops on request, and recognises a file host's page for what it is.
    /// The two ids are the updates that motivated §4.7.
    #[test]
    #[ignore]
    fn live_the_registry_leads_to_an_archive_or_the_browser() {
        let registry = fetch_registry().expect("registry reachable");
        assert!(registry.len() > 500, "the whole list, not a fragment");

        let base = crate::testutil::temp_dir("cup-live");
        // A plain archive on acstuff's own host: stopped after the first
        // megabyte, which is enough to see bytes flow with a known length.
        let mut seen = (0u64, None);
        let outcome = download_update("Car", "hsrc_subaru_gc8", &base, &mut |received, total| {
            seen = (received, total);
            received < 1 << 20
        })
        .expect("download started");
        assert_eq!(outcome, DownloadOutcome::Cancelled, "stopped on request");
        assert!(seen.0 > 0, "bytes arrived");
        assert!(
            seen.1.is_some_and(|t| t > seen.0),
            "the length was announced: {:?}",
            seen.1
        );

        // A mega.nz link: a page, never a file.
        let outcome = download_update("Car", "lk_nissan_180sx_96", &base, &mut |_, _| true).expect("reachable");
        assert_eq!(
            outcome,
            DownloadOutcome::Browser {
                url: "https://acstuff.club/cup/car/lk_nissan_180sx_96/get".into()
            }
        );
        assert_eq!(
            std::fs::read_dir(&base).unwrap().count(),
            0,
            "nothing left behind by either outcome"
        );
    }

    /// Rule (§4.7): the startup sweep removes our download folders and
    /// nothing else in the temp folder.
    #[test]
    fn the_startup_sweep_only_takes_our_folders() {
        let base = crate::testutil::temp_dir("cup-sweep");
        let ours = base.join(format!("{DOWNLOAD_PREFIX}left"));
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::write(ours.join("car.part"), b"half").unwrap();
        let theirs = base.join("pitbox-import-xyz");
        std::fs::create_dir_all(&theirs).unwrap();
        let file = base.join(format!("{DOWNLOAD_PREFIX}file"));
        std::fs::write(&file, b"x").unwrap();

        sweep_leftovers(&base);
        assert!(!ours.exists(), "our leftover is gone");
        assert!(theirs.is_dir(), "another prefix is left alone");
        assert!(file.is_file(), "a file carrying the prefix is not a folder of ours");
    }
}
