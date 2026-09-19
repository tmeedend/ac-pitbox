//! `saved_sessions.json` — the file saved sessions used to live in, kept for
//! one thing only: migrating what it still holds (SESSION§3.6).
//!
//! A saved session is now a Content Manager preset (`sessionpreset.rs`), which
//! is why nothing writes here any more. The file is read once, converted, then
//! renamed — see `sessionpreset::migrate_legacy_file`.
//!
//! The history is worth keeping in one line, because it is the reason the
//! preset writer is synchronous too: before this file there was
//! `localStorage`, which is not guaranteed to reach the disk on WebView2, and
//! a session saved just before closing the app could simply never exist.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("saved_sessions.json"))
}

/// Empty object when the file is absent or unreadable — a fresh install, or a
/// corrupt file: never blocking.
pub fn load(app: &AppHandle) -> serde_json::Value {
    let Some(path) = file(app) else {
        return serde_json::json!({});
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trips_through_json() {
        let dir = crate::testutil::temp_dir("saved-sessions");
        let path = dir.join("saved_sessions.json");
        let all = serde_json::json!({
            "race::My Grid": { "name": "My Grid", "savedAt": "2026-01-01T00:00:00.000Z" },
        });
        let json = serde_json::to_string_pretty(&all).unwrap();
        std::fs::write(&path, &json).unwrap();
        let back: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back["race::My Grid"]["name"], "My Grid");
    }

    #[test]
    fn malformed_json_falls_back_to_empty_object() {
        // Same fallback as `load()` (corrupt file, or one from a future
        // incompatible format): never blocking, never a panic.
        let value: serde_json::Value = serde_json::from_str("{not json").unwrap_or_else(|_| serde_json::json!({}));
        assert!(value.as_object().unwrap().is_empty());
    }
}
