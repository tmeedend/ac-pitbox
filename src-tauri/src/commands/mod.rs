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
pub mod config;
pub mod import;
pub mod layers;
pub mod library;
pub mod library_columns;
pub mod maintenance;
pub mod media;
pub mod music;
pub mod others;
pub mod packs;
pub mod preview;
pub mod profiles;
pub mod rules;
pub mod saved_sessions;
pub mod session;
pub mod session_state;
pub mod ui_prefs;

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
    pub(crate) use crate::rules::Rules;
    pub(crate) use tauri_plugin_opener::OpenerExt;

    pub(crate) use super::mod_kind;
}

/// Convertit le `kind` textuel de l'overlay en `ModKind`. Tout ce qui n'est
/// pas `"Track"` est traité comme une voiture (le champ ne prend que ces deux
/// valeurs, écrites par l'app elle-même).
pub(crate) fn mod_kind(kind: &str) -> crate::modscan::ModKind {
    if kind == "Track" {
        crate::modscan::ModKind::Track
    } else {
        crate::modscan::ModKind::Car
    }
}
