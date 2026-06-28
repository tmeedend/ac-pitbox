//! Profils (§7) : ensembles nommés de mods/versions activés. Appliquer un
//! profil = **réconcilier** les junctions pour correspondre exactement au set
//! (désactiver ce qui n'y est pas, activer ce qui y est).

use std::collections::HashMap;

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::activation;
use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    pub activated: usize,
    pub deactivated: usize,
    pub errors: Vec<String>,
}

/// Crée un profil à partir de l'état actif courant (mods dont la junction existe).
pub fn create_from_active(conn: &Connection, cfg: &AppConfig, name: &str) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    overlay::create_profile(conn, &id, name, &Local::now().to_rfc3339()).map_err(|e| e.to_string())?;
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        let kind = kind_of(&m.kind);
        if let Some(vid) = &m.active_version_id {
            if activation::is_mod_active(cfg, kind, &m.id_interne) {
                overlay::add_profile_entry(conn, &id, &m.id_interne, vid).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(id)
}

/// Applique un profil : réconciliation des junctions.
pub fn apply(conn: &Connection, cfg: &AppConfig, profile_id: &str) -> Result<ApplyReport, String> {
    let entries = overlay::get_profile_entries(conn, profile_id).map_err(|e| e.to_string())?;
    let target: HashMap<String, String> =
        entries.into_iter().map(|e| (e.mod_id, e.version_id)).collect();

    let mut report = ApplyReport { activated: 0, deactivated: 0, errors: Vec::new() };

    // Désactiver les mods actuellement actifs absents du profil.
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        let kind = kind_of(&m.kind);
        if activation::is_mod_active(cfg, kind, &m.id_interne) && !target.contains_key(&m.id_interne) {
            match activation::deactivate(conn, cfg, &m.id_interne) {
                Ok(()) => report.deactivated += 1,
                Err(e) => report.errors.push(format!("{} : {e}", m.id_interne)),
            }
        }
    }

    // Activer chaque entrée du profil (avec sa version).
    for (mod_id, version_id) in &target {
        match activation::activate(conn, cfg, mod_id, Some(version_id)) {
            Ok(()) => report.activated += 1,
            Err(e) => report.errors.push(format!("{mod_id} : {e}")),
        }
    }

    Ok(report)
}
