//! The user's own servers (`SPEC-play-online.md`, case 1): favourites and
//! recent joins, in `online.json` of `app_config_dir`.
//!
//! Same pattern as `library_columns.rs` (golden rule 6): a synchronous write,
//! so the command only returns once the file is on disk, and a structure
//! opaque here — the schema belongs to the frontend (`online/store.svelte.ts`).
//! Unlike that module, the folder is a parameter: a business module does not
//! reach for `tauri::AppHandle` (CLAUDE.md), the facade resolves it.

use std::path::Path;

use serde_json::Value;

const FILE: &str = "online.json";

/// An empty object when the file does not exist yet or cannot be read: a
/// first start, or a damaged file, never blocks the page. A damaged file is
/// logged — it is the user's favourites that just went missing.
pub fn load(config_dir: &Path) -> Value {
    let path = config_dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("online: {} is not JSON ({e}), starting empty", path.display());
            serde_json::json!({})
        }),
        Err(_) => serde_json::json!({}),
    }
}

pub fn save(config_dir: &Path, value: &Value) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let path = config_dir.join(FILE);
    std::fs::write(&path, json).map_err(|e| {
        log::warn!("online: cannot write {} — {e}", path.display());
        format!("writing {FILE}: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (golden rule 6): what is saved reads back whole after a restart.
    #[test]
    fn the_store_round_trips() {
        let dir = crate::testutil::temp_dir("online-store");
        let value = serde_json::json!({
            "favourites": [{ "ip": "5.9.255.37", "http_port": 8071 }],
            "recents": [{ "car": "ks_toyota_celica_st185", "at": "2026-10-03T08:30:00Z" }],
        });
        save(&dir, &value).unwrap();
        assert_eq!(load(&dir), value, "read back as written");
    }

    /// Rule: a missing or damaged file is an empty store, never an error.
    #[test]
    fn a_missing_or_damaged_file_is_empty() {
        let dir = crate::testutil::temp_dir("online-store-bad");
        assert_eq!(load(&dir), serde_json::json!({}), "no file yet");
        std::fs::write(dir.join(FILE), "{not json").unwrap();
        assert_eq!(load(&dir), serde_json::json!({}), "damaged file");
    }
}
