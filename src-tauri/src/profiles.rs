//! Profils (§7) : ensembles nommés de mods/versions activés, **plus** l'état
//! des Autres mods et des Apps (§7.3/§12bis.4) capturés dans le même
//! instantané. Appliquer un profil = **réconcilier** chacun des trois :
//! désactiver ce qui n'y est pas, activer ce qui y est.
//!
//! Autres mods et Apps n'ont pas de notion de version (juste actif/inactif),
//! contrairement aux mods (voiture/circuit) qui portent une version précise —
//! d'où deux tables séparées côté overlay (`profile_entries` /
//! `profile_extra_entries`) plutôt qu'un `version_id` optionnel.

use std::collections::{HashMap, HashSet};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::activation;
use crate::apps;
use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::others;
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

/// Crée un profil à partir de l'état actif courant : mods (voiture/circuit),
/// Autres mods et Apps confondus.
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

    // Autres mods (§7.3) : is_active est un simple drapeau overlay, pas de version.
    for o in overlay::list_other_mods(conn).map_err(|e| e.to_string())? {
        if o.is_active {
            overlay::add_profile_extra_entry(conn, &id, "other", &o.id).map_err(|e| e.to_string())?;
        }
    }

    // Apps (§12bis.4) : état dérivé de la junction, pas de version non plus.
    for a in apps::list_apps(conn, cfg).map_err(|e| e.to_string())? {
        if a.active {
            overlay::add_profile_extra_entry(conn, &id, "app", &a.id).map_err(|e| e.to_string())?;
        }
    }

    Ok(id)
}

/// Applique un profil : réconciliation des junctions, sur les trois types.
pub fn apply(conn: &Connection, cfg: &AppConfig, profile_id: &str) -> Result<ApplyReport, String> {
    let entries = overlay::get_profile_entries(conn, profile_id).map_err(|e| e.to_string())?;
    let target: HashMap<String, String> = entries.into_iter().map(|e| (e.mod_id, e.version_id)).collect();

    let extra = overlay::get_profile_extra_entries(conn, profile_id).map_err(|e| e.to_string())?;
    let target_other: HashSet<String> = extra
        .iter()
        .filter(|e| e.kind == "other")
        .map(|e| e.entry_id.clone())
        .collect();
    let target_apps: HashSet<String> = extra
        .iter()
        .filter(|e| e.kind == "app")
        .map(|e| e.entry_id.clone())
        .collect();

    let mut report = ApplyReport {
        activated: 0,
        deactivated: 0,
        errors: Vec::new(),
    };

    // --- Mods (voiture/circuit) ---
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        let kind = kind_of(&m.kind);
        if activation::is_mod_active(cfg, kind, &m.id_interne) && !target.contains_key(&m.id_interne) {
            match activation::deactivate(conn, cfg, &m.id_interne) {
                Ok(()) => report.deactivated += 1,
                Err(e) => report.errors.push(format!("{} : {e}", m.id_interne)),
            }
        }
    }
    for (mod_id, version_id) in &target {
        match activation::activate(conn, cfg, mod_id, Some(version_id)) {
            Ok(()) => report.activated += 1,
            Err(e) => report.errors.push(format!("{mod_id} : {e}")),
        }
    }

    // --- Autres mods (§7.3) ---
    for o in overlay::list_other_mods(conn).map_err(|e| e.to_string())? {
        if o.is_active && !target_other.contains(&o.id) {
            match others::deactivate_other(conn, &o.id) {
                Ok(()) => report.deactivated += 1,
                Err(e) => report.errors.push(format!("{} : {e}", o.id)),
            }
        }
    }
    for id in &target_other {
        match others::activate_other(conn, cfg, id) {
            Ok(_) => report.activated += 1,
            Err(e) => report.errors.push(format!("{id} : {e}")),
        }
    }

    // --- Apps (§12bis.4) ---
    for a in apps::list_apps(conn, cfg).map_err(|e| e.to_string())? {
        if a.active && !target_apps.contains(&a.id) {
            match apps::deactivate_app(conn, cfg, &a.id) {
                Ok(()) => report.deactivated += 1,
                Err(e) => report.errors.push(format!("{} : {e}", a.id)),
            }
        }
    }
    for id in &target_apps {
        match apps::activate_app(conn, cfg, id) {
            Ok(()) => report.activated += 1,
            Err(e) => report.errors.push(format!("{id} : {e}")),
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un profil doit rétablir les trois types capturés : mod (voiture),
    /// Autre mod et App — pas seulement les mods, qui étaient jusque-là les
    /// seuls réellement sauvegardés (Autres mods et Apps ignorés en silence).
    #[test]
    fn profile_round_trips_mod_other_and_app() {
        if !cfg!(windows) {
            return;
        }
        let base = crate::testutil::temp_dir("profile-extra");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        std::fs::create_dir_all(ac.join("extension").join("config").join("tracks")).unwrap();
        let carv = library.join("cars").join("test_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        let other_src = library.join("other").join("MyShaderMod");
        std::fs::create_dir_all(
            other_src
                .join("extension")
                .join("config")
                .join("tracks")
                .join("newtrack"),
        )
        .unwrap();
        std::fs::write(
            other_src
                .join("extension")
                .join("config")
                .join("tracks")
                .join("newtrack")
                .join("track.ini"),
            b"x",
        )
        .unwrap();
        let app_dir = library.join("apps").join("MyApp");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(app_dir.join("MyApp.py"), b"# app").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = chrono::Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "test_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "test_car",
            Some("1.0"),
            None,
            &now,
            &carv.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "test_car", "v1").unwrap();
        overlay::insert_other_mod(&conn, "MyShaderMod", &other_src.to_string_lossy(), None, &now).unwrap();
        overlay::insert_app(&conn, "MyApp", &app_dir.to_string_lossy(), None, &now).unwrap();

        // Active les trois avant la capture.
        activation::activate(&conn, &cfg, "test_car", None).unwrap();
        others::activate_other(&conn, &cfg, "MyShaderMod").unwrap();
        apps::activate_app(&conn, &cfg, "MyApp").unwrap();

        let profile_id = create_from_active(&conn, &cfg, "Tout actif").unwrap();
        let row = overlay::list_profiles(&conn)
            .unwrap()
            .into_iter()
            .find(|p| p.id == profile_id)
            .unwrap();
        assert_eq!(
            row.entry_count, 3,
            "les 3 types comptent dans le profil, pas seulement le mod"
        );

        // Désactive tout, comme sur une machine fraîche après un simple
        // copiage de fichiers (aucune junction ne survit).
        activation::deactivate(&conn, &cfg, "test_car").unwrap();
        others::deactivate_other(&conn, "MyShaderMod").unwrap();
        apps::deactivate_app(&conn, &cfg, "MyApp").unwrap();
        assert!(!activation::is_mod_active(&cfg, ModKind::Car, "test_car"));
        assert!(!overlay::list_other_mods(&conn).unwrap()[0].is_active);
        assert!(!apps::list_apps(&conn, &cfg).unwrap()[0].active);

        let report = apply(&conn, &cfg, &profile_id).unwrap();
        assert!(report.errors.is_empty(), "erreurs inattendues : {:?}", report.errors);
        assert_eq!(report.activated, 3);

        assert!(
            activation::is_mod_active(&cfg, ModKind::Car, "test_car"),
            "mod réactivé"
        );
        assert!(
            overlay::list_other_mods(&conn).unwrap()[0].is_active,
            "autre mod réactivé"
        );
        assert!(apps::list_apps(&conn, &cfg).unwrap()[0].active, "app réactivée");
    }
}
