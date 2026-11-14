//! How an export lands back (EXPORT§7.3, steps 5 to 7): the skeletons into the
//! library, the settings into this machine's folders, every write noted so
//! that a failure on the way takes all of them back.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use super::files::{
    logo_name, preset_name, read_json, CLASSIFICATION_FILES, CLASSIFICATION_PREFIX, CONFIG_FILE, GRIDS_FILE,
    LIBRARY_PREFIX, MUSIC_FILE, PREFERENCES_PREFIX, PREFERENCE_FILES, PREFS_ENTRY, SESSIONS_PREFIX,
};
use super::{fusion, Archive, Part, Places};

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

/// Whether the library already has something at `rel` (EXPORT R3): a folder
/// the import must not write into. A path that could leave the library is
/// treated as taken — it is never written either.
pub(super) fn occupied(library: &Path, rel: &str) -> bool {
    under(library, &rel.replace('\\', "/")).is_none_or(|p| p.exists())
}

/// Step 5 of EXPORT§7.3: the skeletons, at the library's relative paths —
/// but those under `skipped`, folders of what the base did not bring in.
/// **A file already there is never written over** (R3): the base already
/// left out what it knew to be there, and this is the net for the rest.
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
        if target.exists() {
            log::warn!("transfer: {} is already there, left as it is", target.display());
            continue;
        }
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        undo.write(&target, |f| std::io::copy(&mut entry, f).map(|_| ()))?;
    }
    Ok(())
}

/// The frozen thumbnails of the export laid in the skeletons of cars in the
/// showcase here that have none (EXPORT§7.4). `pairs` holds, for each, the
/// export's version folder and this machine's, both as stored. Returns how
/// many cars took some.
pub(super) fn adopt_thumbs(
    zip: &mut Archive,
    library: &Path,
    pairs: &[(String, String)],
    undo: &mut Undo,
) -> Result<usize, String> {
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let mut adopted = 0;
    for (theirs, ours) in pairs {
        let Some(dir) = crate::libpath::resolve(Some(library), ours) else {
            continue;
        };
        if !dir.is_dir() || !crate::skeleton::thumbs_of(&dir).is_empty() {
            continue;
        }
        let prefix = format!("{LIBRARY_PREFIX}{}/", theirs.replace('\\', "/").trim_end_matches('/'));
        let mut took = false;
        for name in names.iter().filter(|n| n.starts_with(&prefix)) {
            let file = &name[prefix.len()..];
            if file.contains('/') || !crate::skeleton::is_frozen_thumb(Path::new(file)) {
                continue;
            }
            let target = dir.join(file);
            if target.exists() {
                continue;
            }
            let bytes = read_entry(zip, name)?;
            undo.write(&target, |f| f.write_all(&bytes))?;
            took = true;
        }
        adopted += usize::from(took);
    }
    Ok(adopted)
}

const UI_PREFS: &str = "ui_prefs.json";

/// The export's `ui_prefs.json`, when it carries one.
pub(super) fn export_ui_prefs(zip: &mut Archive) -> Result<Option<serde_json::Value>, String> {
    let name = format!("{PREFERENCES_PREFIX}{UI_PREFS}");
    if zip.index_for_name(&name).is_none() {
        return Ok(None);
    }
    let bytes = read_entry(zip, &name)?;
    Ok(serde_json::from_slice(&bytes).ok())
}

/// This machine's `ui_prefs.json`, an empty object without one.
pub(super) fn local_ui_prefs(config_dir: &Path) -> serde_json::Value {
    read_json(&config_dir.join(UI_PREFS)).unwrap_or_else(|| serde_json::json!({}))
}

fn read_entry(zip: &mut Archive, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// What [`unpack_settings`] did.
#[derive(Debug, Default)]
pub(super) struct SettingsOutcome {
    /// Presets renamed because one of the same name was already there.
    pub presets_renamed: usize,
    /// Grids given another name, theirs being taken here.
    pub grids_renamed: Vec<String>,
    /// Classification files kept as they are here, holding decisions.
    pub classification_kept: Vec<String>,
    /// What merging `ui_prefs.json` did to the per-mod preferences.
    pub prefs: fusion::PrefsMerge,
}

/// Step 6 and 7 of EXPORT§7.3: the settings of the parts asked for, into
/// this machine's folders, merged with what is there (EXPORT§7.4) — a preset,
/// a grid, a logo or a file of decisions already there is never written over.
pub(super) fn unpack_settings(
    zip: &mut Archive,
    places: &Places,
    parts: &[Part],
    prefs_side: fusion::Side,
    undo: &mut Undo,
) -> Result<SettingsOutcome, String> {
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let mut out = SettingsOutcome::default();
    for name in names {
        if let Some(rest) = name.strip_prefix(CLASSIFICATION_PREFIX) {
            if parts.contains(&Part::Classification) {
                classification(zip, &name, rest, &places.config_dir, &mut out, undo)?;
            }
        } else if let Some(rest) = name.strip_prefix(SESSIONS_PREFIX) {
            if parts.contains(&Part::Sessions) {
                sessions(zip, &name, rest, places, &mut out, undo)?;
            }
        } else if let Some(rest) = name.strip_prefix(PREFERENCES_PREFIX) {
            let taken = PrefsTaken {
                library: parts.contains(&Part::Library),
                preferences: parts.contains(&Part::Preferences),
                side: prefs_side,
            };
            preferences(zip, &name, rest, &places.config_dir, taken, &mut out, undo)?;
        }
    }
    Ok(out)
}

/// A file of classification: merged entry by entry one day (EXPORT§9);
/// meanwhile, this machine's decisions are never replaced by another's, and
/// a logo of the same name here stays.
fn classification(
    zip: &mut Archive,
    name: &str,
    rest: &str,
    dir: &Path,
    out: &mut SettingsOutcome,
    undo: &mut Undo,
) -> Result<(), String> {
    let bytes = read_entry(zip, name)?;
    if CLASSIFICATION_FILES.contains(&rest) {
        let target = dir.join(rest);
        match std::fs::read(&target) {
            Ok(here) if fusion::has_decisions(&here) => out.classification_kept.push(rest.to_string()),
            _ => undo.write(&target, |f| f.write_all(&bytes))?,
        }
    } else if let Some(file) = rest
        .strip_prefix(&format!("{}/", crate::logos::LOGOS_DIR))
        .and_then(|f| logo_name(Path::new(f)))
    {
        let target = dir.join(crate::logos::LOGOS_DIR).join(&file);
        if !target.exists() {
            undo.write(&target, |f| f.write_all(&bytes))?;
        }
    }
    Ok(())
}

/// The saved grids, merged by name, and the sessions' presets, renamed
/// rather than written over.
fn sessions(
    zip: &mut Archive,
    name: &str,
    rest: &str,
    places: &Places,
    out: &mut SettingsOutcome,
    undo: &mut Undo,
) -> Result<(), String> {
    if rest == GRIDS_FILE {
        let theirs: serde_json::Value = serde_json::from_slice(&read_entry(zip, name)?).map_err(|e| e.to_string())?;
        let target = places.config_dir.join(GRIDS_FILE);
        let ours = read_json(&target).unwrap_or_else(|| serde_json::json!({}));
        let (merged, renamed) = fusion::merge_named(&ours, &theirs);
        out.grids_renamed = renamed;
        let json = serde_json::to_vec_pretty(&merged).map_err(|e| e.to_string())?;
        return undo.write(&target, |f| f.write_all(&json));
    }
    let Some(file) = rest.strip_prefix("presets/").and_then(|f| preset_name(Path::new(f))) else {
        return Ok(());
    };
    let Some(presets) = &places.presets_dir else {
        log::warn!("transfer: no presets folder here, {file} not imported");
        return Ok(());
    };
    let (target, was_renamed) = free_name(presets, &file);
    out.presets_renamed += usize::from(was_renamed);
    let bytes = read_entry(zip, name)?;
    undo.write(&target, |f| f.write_all(&bytes))
}

/// What of the preferences an import takes, and which side wins a per-mod
/// preference set on both.
#[derive(Clone, Copy)]
struct PrefsTaken {
    library: bool,
    preferences: bool,
    side: fusion::Side,
}

/// The preferences: `ui_prefs.json` merged — its per-mod preferences and grid
/// presets following the library, the application's own the Preferences
/// (EXPORT§7.4) —, the other files replaced with the Preferences.
fn preferences(
    zip: &mut Archive,
    name: &str,
    rest: &str,
    dir: &Path,
    taken: PrefsTaken,
    out: &mut SettingsOutcome,
    undo: &mut Undo,
) -> Result<(), String> {
    let PrefsTaken {
        library,
        preferences,
        side: prefs_side,
    } = taken;
    if rest == UI_PREFS {
        if !(library || preferences) {
            return Ok(());
        }
        let theirs: serde_json::Value = serde_json::from_slice(&read_entry(zip, name)?).map_err(|e| e.to_string())?;
        let ours = local_ui_prefs(dir);
        out.prefs = fusion::merge_ui_prefs(&ours, &theirs, library, preferences, prefs_side);
        if out.prefs.value == ours {
            return Ok(());
        }
        let json = serde_json::to_vec_pretty(&out.prefs.value).map_err(|e| e.to_string())?;
        return undo.write(&dir.join(UI_PREFS), |f| f.write_all(&json));
    }
    if !preferences {
        return Ok(());
    }
    let bytes = read_entry(zip, name)?;
    if rest == PREFS_ENTRY {
        merge_prefs(dir, &bytes, undo)?;
    } else if PREFERENCE_FILES.contains(&rest) || rest == MUSIC_FILE {
        undo.write(&dir.join(rest), |f| f.write_all(&bytes))?;
    }
    Ok(())
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
