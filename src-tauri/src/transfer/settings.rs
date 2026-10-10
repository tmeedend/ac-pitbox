//! The settings an export carries (EXPORT§5.2): classification, sessions and
//! preferences, file by file — and never a path of the machine (R2).

use std::path::{Path, PathBuf};

use super::files::{
    flat_files, logo_name, preset_name, read_json, Entry, CLASSIFICATION_FILES, CLASSIFICATION_PREFIX, CONFIG_FILE,
    GRIDS_FILE, MUSIC_FILE, PREFERENCES_PREFIX, PREFERENCE_FILES, PREFS_ENTRY, SESSIONS_PREFIX,
};
use super::{Part, Places};

pub(super) struct Settings {
    pub entries: Vec<Entry>,
    pub presets: usize,
    pub grids: usize,
}

/// Whether `s` is an absolute path: `C:\…`, `C:/…` or `\\server\…`.
fn is_absolute_path(s: &str) -> bool {
    let b = s.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || s.starts_with(r"\\")
}

/// R2 on a JSON setting: every string that is an absolute path of this
/// machine becomes `null` — at any depth, **inside the strings that hold
/// JSON themselves too**. `ui_prefs.json` stores its values serialized, and
/// a preferred livery's preview path in it, `\` escaped twice over, went out
/// unseen in a first export of the real library; Content Manager's presets
/// name their sibling presets by path the same way. A string that holds no
/// path is left byte for byte as it was. Returns whether anything changed.
fn scrub_paths(value: &mut serde_json::Value) -> bool {
    use serde_json::Value;
    match value {
        Value::String(s) if is_absolute_path(s) => {
            *value = Value::Null;
            true
        }
        Value::String(s) if s.starts_with('{') || s.starts_with('[') => {
            let Ok(mut inner) = serde_json::from_str::<Value>(s) else {
                return false;
            };
            if !scrub_paths(&mut inner) {
                return false;
            }
            *s = inner.to_string();
            true
        }
        // Every value visited — `any` would stop at the first path found.
        Value::Array(items) => {
            let mut changed = false;
            for v in items {
                changed |= scrub_paths(v);
            }
            changed
        }
        Value::Object(map) => {
            let mut changed = false;
            for v in map.values_mut() {
                changed |= scrub_paths(v);
            }
            changed
        }
        _ => false,
    }
}

/// A JSON setting as it leaves: read, cleaned of this machine's paths
/// ([`scrub_paths`]), written back only if something changed. A file that
/// does not read as JSON does not leave — what it holds cannot be checked.
fn json_entry(part: Part, name: String, path: &Path) -> Option<Entry> {
    let mut value = read_json(path)?;
    if !scrub_paths(&mut value) {
        return Some(Entry::file(part, name, path.to_path_buf()));
    }
    match serde_json::to_vec_pretty(&value) {
        Ok(json) => Some(Entry::bytes(part, name, json)),
        Err(e) => {
            log::warn!("transfer: {} not exported: {e}", path.display());
            None
        }
    }
}

/// The settings of the parts asked for, file by file (EXPORT§5.2).
pub(super) fn settings(places: &Places, parts: &[Part]) -> Result<Settings, String> {
    let mut settings = Settings {
        entries: Vec::new(),
        presets: 0,
        grids: 0,
    };
    if parts.contains(&Part::Classification) {
        classification(&places.config_dir, &mut settings.entries);
    }
    if parts.contains(&Part::Sessions) {
        sessions(places, &mut settings);
    }
    if parts.contains(&Part::Preferences) {
        preferences(&places.config_dir, &mut settings.entries)?;
    }
    Ok(settings)
}

/// The rules, the taxonomies, the logo choices and the user's own logos.
fn classification(dir: &Path, out: &mut Vec<Entry>) {
    for name in CLASSIFICATION_FILES {
        let path = dir.join(name);
        if path.is_file() {
            out.extend(json_entry(
                Part::Classification,
                format!("{CLASSIFICATION_PREFIX}{name}"),
                &path,
            ));
        }
    }
    for path in flat_files(&dir.join(crate::logos::LOGOS_DIR)) {
        if let Some(name) = logo_name(&path) {
            out.push(Entry::file(
                Part::Classification,
                format!("{CLASSIFICATION_PREFIX}{}/{name}", crate::logos::LOGOS_DIR),
                path,
            ));
        }
    }
}

/// The saved grids, and the sessions saved as Content Manager presets.
fn sessions(places: &Places, settings: &mut Settings) {
    let path = places.config_dir.join(GRIDS_FILE);
    if let Some(value) = read_json(&path) {
        settings.grids = value.as_object().map_or(0, |o| o.len());
        settings.entries.extend(json_entry(
            Part::Sessions,
            format!("{SESSIONS_PREFIX}{GRIDS_FILE}"),
            &path,
        ));
    }
    let Some(presets_dir) = &places.presets_dir else { return };
    for path in flat_files(presets_dir) {
        let Some(name) = preset_name(&path) else { continue };
        if let Some(entry) = json_entry(Part::Sessions, format!("{SESSIONS_PREFIX}presets/{name}"), &path) {
            settings.entries.push(entry);
            settings.presets += 1;
        }
    }
}

/// The application's and the screens' preferences, without a path of this
/// machine: the six paths of `config.json` (its `prefs` alone leaves), the
/// music's own folders (the other machine plays its defaults until the user
/// picks its own), the previews of the preferred liveries and layouts in
/// `ui_prefs.json` (the cards there show their own images).
fn preferences(dir: &Path, out: &mut Vec<Entry>) -> Result<(), String> {
    if let Some(mut prefs) = read_json(&dir.join(CONFIG_FILE)).and_then(|c| c.get("prefs").cloned()) {
        scrub_paths(&mut prefs);
        let json = serde_json::to_vec_pretty(&prefs).map_err(|e| e.to_string())?;
        out.push(Entry::bytes(
            Part::Preferences,
            format!("{PREFERENCES_PREFIX}{PREFS_ENTRY}"),
            json,
        ));
    }
    for name in PREFERENCE_FILES.iter().chain([&MUSIC_FILE]) {
        let path = dir.join(name);
        if path.is_file() {
            out.extend(json_entry(
                Part::Preferences,
                format!("{PREFERENCES_PREFIX}{name}"),
                &path,
            ));
        }
    }
    Ok(())
}

/// The image each car's and track's card shows, as the screens chose it: the
/// preview of the preferred livery or layout, kept in `ui_prefs.json` under
/// `pitbox.skin.<id>` and `pitbox.layout.<id>` (`preferred.ts`), each value a
/// JSON object serialized into a string. Read before [`scrub_paths`] takes
/// the paths out; only the previews still on disk.
pub(super) fn preferred_previews(config_dir: &Path) -> std::collections::HashMap<String, PathBuf> {
    let Some(serde_json::Value::Object(prefs)) = read_json(&config_dir.join("ui_prefs.json")) else {
        return Default::default();
    };
    prefs
        .iter()
        .filter_map(|(key, value)| {
            let id = key
                .strip_prefix("pitbox.skin.")
                .or_else(|| key.strip_prefix("pitbox.layout."))?;
            let inner: serde_json::Value = serde_json::from_str(value.as_str()?).ok()?;
            let preview = PathBuf::from(inner.get("preview")?.as_str()?);
            preview.is_file().then(|| (id.to_string(), preview))
        })
        .collect()
}
