//! Commandes Tauri, regroupées par domaine.
//!
//! Une commande n'est qu'une **façade** : elle charge la config, prend le
//! verrou SQLite et délègue au module métier correspondant. Toute logique qui
//! grossit ici doit descendre dans son module (`importer`, `activation`…).
//!
//! Ajouter une commande = 3 endroits : la fonction dans son module métier, la
//! façade ici, **et** son inscription dans `invoke_handler![…]` de `lib.rs`.
//! Oublier le troisième ne casse rien à la compilation — l'erreur n'apparaît
//! qu'à l'exécution.

pub mod activation;
pub mod addons;
pub mod bulk_ops;
pub mod cmimport;
pub mod config;
pub mod gamestate;
pub mod gridthumbs;
pub mod import;
pub mod layers;
pub mod library;
pub mod library_columns;
pub mod logos;
pub mod maintenance;
pub mod media;
pub mod music;
pub mod nationalities;
pub mod online;
pub mod others;
pub mod packs;
pub mod preview;
pub mod profiles;
pub mod rules;
pub mod saved_grids;
pub mod session;
pub mod session_state;
pub mod sessionpreset;
pub mod showcase;
pub mod techsheet;
pub mod timing;
pub mod trackstate;
pub mod transfer;
pub mod ui_prefs;
pub mod updates;
pub mod usermeta;
pub mod wiki;

use prelude::*;
use tauri::Manager;

/// Imports communs à toutes les façades. Import global volontaire : il ne
/// déclenche pas d'avertissement `unused_imports` quand un module n'en
/// utilise qu'une partie.
mod prelude {
    pub(crate) use tauri::{AppHandle, State};

    pub(crate) use crate::config::{AppConfig, ConfigValidation};
    pub(crate) use crate::detect::DetectedPaths;
    pub(crate) use crate::importer::ArchiveResult;
    pub(crate) use crate::library::{ModCard, ModDetail};
    pub(crate) use crate::overlay::Db;
    pub(crate) use crate::rule_overlay::RulesView;
    pub(crate) use crate::rules::{Rules, RulesOverlay};
    pub(crate) use crate::taxonomy::{FamilyOverlay, MapOverlay, TaxonomyOverlay, TaxonomyTables};
    pub(crate) use tauri_plugin_opener::OpenerExt;

    pub(crate) use super::{off_window, read_off_window};
}

/// Runs a façade's work off the thread that drives the window. A synchronous
/// command runs **on** that thread: the window froze, and every other command
/// waited, for as long as one of them read its files - 2.4 s for the library
/// listing on a first start.
///
/// For reads. A write keeps its synchronous command, and with it the guarantee
/// that two gestures in a row run in the order they were made.
pub(crate) async fn off_window<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// `off_window` for the usual façade: the config loaded and the base locked
/// for the whole of `work` - files included, so a synchronous command needing
/// the base still waits for it; releasing the lock before the files are read
/// is up to each module (`library::list_cards_shared`).
pub(crate) async fn read_off_window<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&rusqlite::Connection, &AppConfig) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    off_window(app, move |app| {
        let cfg = crate::config::load(app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        work(&conn, &cfg)
    })
    .await
}
