//! What an export would weigh, before making it (EXPORT§7.2): counts come from
//! the base, weights from the files a skeleton keeps and the compression
//! measured on a real export.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::files::files_in;
use crate::modscan::ModKind;
use crate::skeleton;

/// What the zip makes of each kind of file, measured on the export of a real
/// library (2026-10-10, 186 mods, 16.1 MB): images do not compress, the base
/// falls to a fifth, a JSON file to under a third, a manifest to about 1 KB;
/// a frozen card image weighs about 18 KB, a reduced preview a few dozen.
pub(super) const BASE_RATIO: f64 = 0.2;
const JSON_RATIO: f64 = 0.3;
const MANIFEST_ZIPPED: u64 = 1024;
const FROZEN_IMAGE: u64 = 20 * 1024;
const REDUCED_PREVIEW: u64 = 40 * 1024;

/// What a file of `size` bytes named `name` weighs in the zip.
pub(super) fn zipped(name: &str, size: u64) -> u64 {
    let lower = name.to_lowercase();
    if lower.ends_with(".json") || lower.ends_with(".cmpreset") || lower.ends_with(".ini") {
        (size as f64 * JSON_RATIO) as u64
    } else {
        size
    }
}

/// One folder of the library, as the estimate weighs it.
pub(super) struct ToWeigh {
    dir: PathBuf,
    /// The skeleton's whitelist, for a version or a car or track layer.
    kind: Option<ModKind>,
    /// Already a skeleton: its few files leave as they are.
    freed: bool,
    /// A version, which gets a frozen card image.
    version: bool,
}

/// The folders an export would take, from the base alone: the disk is read
/// after, the lock given back ([`weigh`]).
pub(super) fn folders_to_weigh(conn: &Connection, library: &Path) -> rusqlite::Result<Vec<ToWeigh>> {
    let mut out = Vec::new();
    let mut collect = |sql: &str, version: bool| -> rusqlite::Result<()> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (stored, state, kind) = row?;
            // A mod installed by hand lives in the game: its version is an
            // absolute path outside the library, and it leaves all the same.
            let dir = match super::tables::relative(library, &stored) {
                Some(rel) => library.join(rel),
                None if version => PathBuf::from(stored),
                None => continue,
            };
            out.push(ToWeigh {
                dir,
                kind: match kind.as_deref() {
                    Some("Car") => Some(ModKind::Car),
                    Some("Track") => Some(ModKind::Track),
                    _ => None,
                },
                freed: state == crate::overlay::CONTENT_SKELETON,
                version,
            });
        }
        Ok(())
    };
    collect(
        "SELECT v.library_path, v.content_state, m.kind FROM versions v JOIN mods m ON m.id_interne = v.mod_id
         WHERE m.is_stock = 0 OR (m.is_unmanaged = 1 AND v.id = m.active_version_id)",
        true,
    )?;
    collect("SELECT library_path, content_state, parent_kind FROM layers", false)?;
    collect(
        "SELECT library_path, content_state, NULL FROM sub_mods WHERE removable = 1",
        false,
    )?;
    collect("SELECT library_path, content_state, NULL FROM apps", false)?;
    collect("SELECT library_path, content_state, NULL FROM other_mods", false)?;
    Ok(out)
}

/// What those folders will weigh in the zip, reading only what a skeleton
/// keeps — a car's or a track's `ui/` —, never the whole folder: preparing
/// the whole export to weigh it took 6 s on a real library, and added up
/// uncompressed sizes (30.7 MB announced for a 16.1 MB file).
pub(super) fn weigh(folders: &[ToWeigh]) -> u64 {
    folders
        .iter()
        .map(|f| {
            if f.freed {
                return files_in(&f.dir)
                    .iter()
                    .map(|(rel, path)| zipped(&rel.to_string_lossy(), size_of(path)))
                    .sum();
            }
            let mut bytes = MANIFEST_ZIPPED;
            if f.version {
                bytes += FROZEN_IMAGE;
            }
            if let Some(kind) = f.kind {
                for (rel, path) in files_in(&f.dir.join("ui")) {
                    let rel = Path::new("ui").join(rel);
                    if skeleton::is_showcase_file(&rel) || !skeleton::is_kept(kind, &rel) {
                        continue;
                    }
                    let size = size_of(&path);
                    bytes += if skeleton::is_reduced(kind, &rel) {
                        size.min(REDUCED_PREVIEW)
                    } else {
                        zipped(&rel.to_string_lossy(), size)
                    };
                }
            }
            bytes
        })
        .sum()
}

fn size_of(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
