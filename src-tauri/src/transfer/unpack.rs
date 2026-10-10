//! How an export lands back (EXPORT§7.3, steps 5 to 7): the skeletons into the
//! library, the settings into this machine's folders, every write noted so
//! that a failure on the way takes all of them back.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use super::files::{
    logo_name, preset_name, read_json, CLASSIFICATION_FILES, CLASSIFICATION_PREFIX, CONFIG_FILE, GRIDS_FILE,
    LIBRARY_PREFIX, MUSIC_FILE, PREFERENCES_PREFIX, PREFERENCE_FILES, PREFS_ENTRY, SESSIONS_PREFIX,
};
use super::{Archive, Part, Places};

/// `rel` under `base`, or `None` for anything that could leave it: a zip
/// entry is data, and `..` or an absolute path in one must never reach
/// outside the folder it lands in.
fn under(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if rel.as_os_str().is_empty() || !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(base.join(rel))
}

/// The skeleton entries of the zip, and where each lands in the library.
fn library_targets(zip: &mut Archive, library: &Path) -> Result<Vec<(usize, PathBuf)>, String> {
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.name().strip_prefix(LIBRARY_PREFIX) else {
            continue;
        };
        let target = under(library, rel).ok_or(crate::errors::TRANSFER_NOT_AN_EXPORT)?;
        out.push((i, target));
    }
    Ok(out)
}

/// R3, on the disk: no folder the import would write is there already.
pub(super) fn check_free(zip: &mut Archive, library: &Path) -> Result<(), String> {
    for (_, target) in library_targets(zip, library)? {
        if target.exists() {
            log::warn!("transfer: {} is already there, import refused", target.display());
            return Err(crate::errors::TRANSFER_FOLDER_NOT_EMPTY.into());
        }
    }
    Ok(())
}

/// Step 5 of EXPORT§7.3: the skeletons, at the library's relative paths —
/// but those under `skipped`, folders of what the base did not bring in.
pub(super) fn unpack_library(
    zip: &mut Archive,
    library: &Path,
    skipped: &[String],
    undo: &mut Undo,
) -> Result<(), String> {
    let skipped: Vec<PathBuf> = skipped.iter().map(|rel| library.join(rel)).collect();
    for (i, target) in library_targets(zip, library)? {
        if skipped.iter().any(|dir| target.starts_with(dir)) {
            continue;
        }
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        undo.write(&target, |f| std::io::copy(&mut entry, f).map(|_| ()))?;
    }
    Ok(())
}

fn read_entry(zip: &mut Archive, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Step 6 and 7 of EXPORT§7.3: the settings of the parts asked for, into
/// this machine's folders. Returns how many presets were renamed — a preset
/// of the same name already there is never overwritten.
pub(super) fn unpack_settings(
    zip: &mut Archive,
    places: &Places,
    parts: &[Part],
    undo: &mut Undo,
) -> Result<usize, String> {
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let dir = &places.config_dir;
    let mut renamed = 0;
    for name in names {
        if let Some(rest) = name.strip_prefix(CLASSIFICATION_PREFIX) {
            if !parts.contains(&Part::Classification) {
                continue;
            }
            let target = if CLASSIFICATION_FILES.contains(&rest) {
                Some(dir.join(rest))
            } else {
                rest.strip_prefix(&format!("{}/", crate::logos::LOGOS_DIR))
                    .and_then(|f| logo_name(Path::new(f)))
                    .map(|f| dir.join(crate::logos::LOGOS_DIR).join(f))
            };
            if let Some(target) = target {
                let bytes = read_entry(zip, &name)?;
                undo.write(&target, |f| f.write_all(&bytes))?;
            }
        } else if let Some(rest) = name.strip_prefix(SESSIONS_PREFIX) {
            if !parts.contains(&Part::Sessions) {
                continue;
            }
            if rest == GRIDS_FILE {
                let bytes = read_entry(zip, &name)?;
                undo.write(&dir.join(GRIDS_FILE), |f| f.write_all(&bytes))?;
            } else if let Some(file) = rest.strip_prefix("presets/").and_then(|f| preset_name(Path::new(f))) {
                let Some(presets) = &places.presets_dir else {
                    log::warn!("transfer: no presets folder here, {file} not imported");
                    continue;
                };
                let (target, was_renamed) = free_name(presets, &file);
                renamed += usize::from(was_renamed);
                let bytes = read_entry(zip, &name)?;
                undo.write(&target, |f| f.write_all(&bytes))?;
            }
        } else if let Some(rest) = name.strip_prefix(PREFERENCES_PREFIX) {
            if !parts.contains(&Part::Preferences) {
                continue;
            }
            let bytes = read_entry(zip, &name)?;
            if rest == PREFS_ENTRY {
                merge_prefs(dir, &bytes, undo)?;
            } else if PREFERENCE_FILES.contains(&rest) || rest == MUSIC_FILE {
                undo.write(&dir.join(rest), |f| f.write_all(&bytes))?;
            }
        }
    }
    Ok(renamed)
}

/// The preferences laid into this machine's `config.json`: its paths do not
/// move (EXPORT§7.3, step 7).
fn merge_prefs(dir: &Path, bytes: &[u8], undo: &mut Undo) -> Result<(), String> {
    let prefs: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let path = dir.join(CONFIG_FILE);
    let mut config = read_json(&path).unwrap_or_else(|| serde_json::json!({}));
    if let Some(o) = config.as_object_mut() {
        o.insert("prefs".into(), prefs);
    }
    let json = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    undo.write(&path, |f| f.write_all(&json))
}

/// `name` in `dir`, or `name (2)`, `(3)`… when taken: never a preset of the
/// user's overwritten. The suffix has no word in it — the backend does not
/// know the user's language.
fn free_name(dir: &Path, name: &str) -> (PathBuf, bool) {
    let first = dir.join(name);
    if !first.exists() {
        return (first, false);
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 2;
    loop {
        let candidate = dir.join(format!("{stem} ({n}).{ext}"));
        if !candidate.exists() {
            return (candidate, true);
        }
        n += 1;
    }
}

/// Everything an import wrote, to take it back if it fails on the way
/// (EXPORT§7.3): files created are removed, files replaced get their former
/// bytes back, and folders created empty go.
#[derive(Default)]
pub(super) struct Undo {
    created: Vec<PathBuf>,
    replaced: Vec<(PathBuf, Vec<u8>)>,
    folders: Vec<PathBuf>,
}

impl Undo {
    fn write(
        &mut self,
        target: &Path,
        fill: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
    ) -> Result<(), String> {
        if let Some(parent) = target.parent() {
            let missing: Vec<PathBuf> = parent
                .ancestors()
                .take_while(|d| !d.exists())
                .map(Path::to_path_buf)
                .collect();
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            self.folders.extend(missing);
        }
        if target.exists() {
            let former = std::fs::read(target).map_err(|e| format!("{}: {e}", target.display()))?;
            self.replaced.push((target.to_path_buf(), former));
        } else {
            self.created.push(target.to_path_buf());
        }
        let mut file = std::fs::File::create(target).map_err(|e| format!("{}: {e}", target.display()))?;
        fill(&mut file).map_err(|e| format!("{}: {e}", target.display()))
    }

    pub fn undo(self) {
        for path in &self.created {
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!("transfer undo: {} not removed: {e}", path.display());
            }
        }
        for (path, former) in &self.replaced {
            if let Err(e) = std::fs::write(path, former) {
                log::warn!("transfer undo: {} not restored: {e}", path.display());
            }
        }
        // Deepest first: a folder empties before its parent is tried.
        let mut folders = self.folders;
        folders.sort_by(|a, b| b.components().count().cmp(&a.components().count()).then(a.cmp(b)));
        folders.dedup();
        for dir in folders {
            if dir.is_dir() {
                if let Err(e) = std::fs::remove_dir(&dir) {
                    log::warn!("transfer undo: {} not removed: {e}", dir.display());
                }
            }
        }
    }
}
