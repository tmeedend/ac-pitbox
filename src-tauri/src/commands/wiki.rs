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

use crate::wiki::{self, manual::Suggestion, matching::Thresholds, store::LinkSource, WikiPanel};

/// How many candidates the correction panel offers. Enough to contain the right
/// one, few enough to read without scrolling — a person is choosing, not an
/// algorithm.
const SUGGESTION_LIMIT: u32 = 10;

/// Everything the tab shows, resolving the article if needed (§5, §7).
///
/// Never an error for an absent article: absence is a **state**, carried in
/// `state`, and the tab answers it with an offer rather than a message (§1).
/// The only `Err` here is a poisoned mutex, which is a bug.
#[tauri::command]
pub async fn get_wiki_panel(
    app: AppHandle,
    db: State<'_, Db>,
    mod_key: String,
    lang: String,
) -> Result<WikiPanel, String> {
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
            wiki::Step::Settled(panel) => return Ok(panel),
            wiki::Step::Ask(ask) => ask,
        }
    };

    // 2. Off the lock, off the main thread.
    let config_dir = config_dir(&app)?;
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

/// Free search for the correction panel (§7.6).
///
/// Takes no database at all: it only asks Wikipedia. The type filter is
/// **relaxed** here, deliberately — the spec's own argument is that a user who
/// wants an entity outside the taxonomy is likelier to be right than the
/// taxonomy.
///
/// A URL pasted instead of words is recognised and resolved to its entity, the
/// URL itself never being stored (§3.1).
#[tauri::command]
pub async fn search_wiki_candidates(app: AppHandle, query: String, lang: String) -> Result<Vec<Suggestion>, String> {
    let config_dir = config_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let matching_cfg = wiki::clean::load(&config_dir);
        let cleaner = wiki::clean::Cleaner::new(&matching_cfg);
        let net = wiki::api::WikiClient::new();

        if let Some(entity_id) = wiki::manual::entity_from_url(&net, query.trim()) {
            return match net.details(std::slice::from_ref(&entity_id)) {
                wiki::api::Fetched::Found(details) => details
                    .into_iter()
                    .map(|d| Suggestion {
                        label: d.label.clone().unwrap_or_else(|| d.entity_id.clone()),
                        description: d.description,
                        entity_id: d.entity_id,
                    })
                    .collect(),
                _ => Vec::new(),
            };
        }
        wiki::manual::search(&net, &cleaner, &query, &lang, SUGGESTION_LIMIT)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Ties a mod to an entity by hand (§7.6). Stored as `manual`, which the
/// precedence of §3.1 then protects from every automatic pass and from the
/// shipped table.
///
/// The cached article of the previous entity is left alone: it belongs to that
/// entity, not to this mod, and may well be right for another one.
#[tauri::command]
pub fn set_wiki_link(db: State<Db>, mod_key: String, entity_id: String) -> Result<(), String> {
    if !wiki::api::is_entity_id(&entity_id) {
        return Err(crate::errors::WIKI_NOT_AN_ENTITY.into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    wiki::store::set_link(&conn, &mod_key, &entity_id, LinkSource::Manual).map_err(|e| e.to_string())?;
    // The mod had been written off; it has an article now.
    wiki::store::forget_no_match(&conn, &mod_key).map_err(|e| e.to_string())
}

/// Unties a mod from its entity (§7.6, "none of these"). A mod with no
/// appariement is a valid answer (§1) — and the negative cache is cleared too,
/// so the automatic matching gets another go rather than staying written off.
#[tauri::command]
pub fn clear_wiki_link(db: State<Db>, mod_key: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    wiki::store::clear_link(&conn, &mod_key).map_err(|e| e.to_string())?;
    wiki::store::forget_no_match(&conn, &mod_key).map_err(|e| e.to_string())
}

fn config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("dossier de config indisponible : {e}"))
}
