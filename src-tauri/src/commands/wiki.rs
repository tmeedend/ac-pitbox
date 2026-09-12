//! Wikipedia tab commands (`docs/SPEC-wikipedia-fiche-detail.md` §7).
//!
//! **The SQLite lock is never held across the network**, and that shapes the
//! whole file. A resolution can spend fifteen seconds on timeouts in the worst
//! case; holding the overlay's mutex that long would freeze every other command
//! in the app — the blocked-library bug `commands/ui_prefs.rs` documents. So
//! the work comes in three beats: read under the lock, ask the network on a
//! blocking thread with no database in sight, write under the lock again.
//!
//! `async` + `spawn_blocking` for the same reason as the disk-touching
//! commands: WinHTTP is synchronous (`wiki/http.rs`), so it must not run on the
//! main thread.

use super::prelude::*;
use tauri::Manager;

use crate::wiki::{self, matching::Thresholds, store::CachedArticle};

/// The article to show on a fiche, resolving it if needed (§5, §7.5).
///
/// `None` is the ordinary "there is nothing to show" and the interface hides
/// the tab (§7.1) — never an error, never a message (§1). The only `Err` this
/// can return is a poisoned mutex, which is a bug, not an absent article.
#[tauri::command]
pub async fn get_wiki_article(
    app: AppHandle,
    db: State<'_, Db>,
    mod_key: String,
    lang: String,
) -> Result<Option<CachedArticle>, String> {
    let cfg = crate::config::load(&app);
    let thresholds = Thresholds::from_prefs(&cfg.prefs);

    // 1. Under the lock: everything the network step will need.
    let ask = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        match wiki::plan(
            &conn,
            cfg.ac_install_path.as_deref(),
            &mod_key,
            &lang,
            cfg.prefs.wiki_online,
        ) {
            wiki::Step::Settled(article) => return Ok(article),
            wiki::Step::Ask(ask) => ask,
        }
    };

    // 2. Off the lock, off the main thread.
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("dossier de config indisponible : {e}"))?;
    let resolved = tauri::async_runtime::spawn_blocking(move || {
        let matching_cfg = wiki::clean::load(&config_dir);
        let cleaner = wiki::clean::Cleaner::new(&matching_cfg);
        let net = wiki::api::WikiClient::new();
        wiki::fetch(&net, &cleaner, &matching_cfg.weights, &thresholds, ask)
    })
    .await
    .map_err(|e| e.to_string())?;

    // 3. Back under the lock, only to write.
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(wiki::commit(&conn, resolved))
}
