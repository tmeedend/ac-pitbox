//! Édition groupée (§6.3bis) : actions appliquées à une sélection multiple de
//! mods, limitées aux champs communs à tout mod (tags manuels, favori,
//! catégorie, activation, suppression, export). Ne touche jamais aux champs
//! propres à un type précis (specs voiture, skin piloté, version active) —
//! ceux-ci restent réservés à la fiche détail d'un seul mod.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::{activation, export, maintenance, overlay};

#[derive(Debug, Clone, Serialize)]
pub struct BulkFailure {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BulkReport {
    pub ok: Vec<String>,
    pub failed: Vec<BulkFailure>,
}

impl BulkReport {
    fn push(&mut self, id: &str, result: Result<(), String>) {
        match result {
            Ok(()) => self.ok.push(id.to_string()),
            Err(error) => self.failed.push(BulkFailure { id: id.to_string(), error }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkExportItem {
    pub id: String,
    pub report: Option<export::ExportReport>,
    pub error: Option<String>,
}

pub fn set_favorite(conn: &Connection, ids: &[String], favorite: bool) -> Result<(), String> {
    for id in ids {
        overlay::set_favorite(conn, id, favorite).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn set_category(conn: &Connection, ids: &[String], category: Option<&str>) -> Result<(), String> {
    for id in ids {
        overlay::set_mod_field(conn, id, "category", category).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn add_tag(conn: &Connection, ids: &[String], tag: &str) -> Result<(), String> {
    let tag = tag.trim().to_lowercase();
    if tag.is_empty() {
        return Ok(());
    }
    for id in ids {
        let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else { continue };
        if !m.tags_manual.contains(&tag) {
            let mut tags = m.tags_manual;
            tags.push(tag.clone());
            overlay::set_manual_tags(conn, id, &tags).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn remove_tag(conn: &Connection, ids: &[String], tag: &str) -> Result<(), String> {
    let tag = tag.trim().to_lowercase();
    for id in ids {
        let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else { continue };
        if m.tags_manual.contains(&tag) {
            let tags: Vec<String> = m.tags_manual.into_iter().filter(|t| t != &tag).collect();
            overlay::set_manual_tags(conn, id, &tags).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn activate(conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    let mut report = BulkReport::default();
    for id in ids {
        report.push(id, activation::activate(conn, cfg, id, None));
    }
    report
}

pub fn deactivate(conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    let mut report = BulkReport::default();
    for id in ids {
        report.push(id, activation::deactivate(conn, cfg, id));
    }
    report
}

pub fn delete(conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    let mut report = BulkReport::default();
    for id in ids {
        report.push(id, maintenance::delete_broken(conn, cfg, id));
    }
    report
}

pub fn export(conn: &Connection, cfg: &AppConfig, ids: &[String], dest_dir: &Path) -> Vec<BulkExportItem> {
    ids.iter()
        .map(|id| match export::export_mod(conn, cfg, id, dest_dir) {
            Ok(report) => BulkExportItem { id: id.clone(), report: Some(report), error: None },
            Err(error) => BulkExportItem { id: id.clone(), report: None, error: Some(error) },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_mod(conn: &Connection, id: &str, tags: &[&str]) {
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(conn, id, "Car", Some("B"), Some(id), "h", None, &now).unwrap();
        overlay::set_manual_tags(conn, id, &tags.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
    }

    #[test]
    fn bulk_tags_favorite_and_category() {
        let base = std::env::temp_dir().join(format!("pitbox-bulk-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        seed_mod(&conn, "modA", &["endurance"]);
        seed_mod(&conn, "modB", &[]);
        let ids = vec!["modA".to_string(), "modB".to_string()];

        add_tag(&conn, &ids, "GT3").unwrap();
        let a = overlay::get_mod(&conn, "modA").unwrap().unwrap();
        let b = overlay::get_mod(&conn, "modB").unwrap().unwrap();
        assert_eq!(a.tags_manual, vec!["endurance".to_string(), "gt3".to_string()]);
        assert_eq!(b.tags_manual, vec!["gt3".to_string()]);

        // Idempotent : pas de doublon si le tag est déjà présent sur un des deux.
        add_tag(&conn, &ids, "gt3").unwrap();
        let a2 = overlay::get_mod(&conn, "modA").unwrap().unwrap();
        assert_eq!(a2.tags_manual, vec!["endurance".to_string(), "gt3".to_string()]);

        remove_tag(&conn, &ids, "gt3").unwrap();
        let a3 = overlay::get_mod(&conn, "modA").unwrap().unwrap();
        let b3 = overlay::get_mod(&conn, "modB").unwrap().unwrap();
        assert_eq!(a3.tags_manual, vec!["endurance".to_string()]);
        assert!(b3.tags_manual.is_empty());

        set_favorite(&conn, &ids, true).unwrap();
        assert!(overlay::get_mod(&conn, "modA").unwrap().unwrap().is_favorite);
        assert!(overlay::get_mod(&conn, "modB").unwrap().unwrap().is_favorite);

        set_category(&conn, &ids, Some("#gt")).unwrap();
        assert_eq!(overlay::get_mod(&conn, "modA").unwrap().unwrap().category.as_deref(), Some("#gt"));
        assert_eq!(overlay::get_mod(&conn, "modB").unwrap().unwrap().category.as_deref(), Some("#gt"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bulk_activate_deactivate_delete_reports_per_id() {
        if !cfg!(windows) {
            return;
        }
        let base = std::env::temp_dir().join(format!("pitbox-bulkact-{}", uuid::Uuid::new_v4()));
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();

        // "good" a bien ses fichiers ; "ghost" n'a aucune version (échec garanti
        // d'activate(), sans dépendre du comportement de mklink sur cible absente).
        for id in ["good", "ghost"] {
            overlay::upsert_mod(&conn, id, "Car", Some("B"), Some(id), "h", None, &now).unwrap();
        }
        let good_dir = library.join("cars").join("good").join("v1");
        std::fs::create_dir_all(&good_dir).unwrap();
        overlay::insert_version(
            &conn, "v1", "good", Some("1.0"), None, &now,
            &good_dir.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "good", "v1").unwrap();

        let cfg = AppConfig { ac_install_path: Some(ac), library_path: Some(library), ..Default::default() };
        let ids = vec!["good".to_string(), "ghost".to_string()];

        let report = activate(&conn, &cfg, &ids);
        assert_eq!(report.ok, vec!["good".to_string()]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].id, "ghost");

        // deactivate() est un no-op réussi si aucune junction n'existe déjà
        // (cas de "ghost", dont l'activation a échoué juste au-dessus).
        let report = deactivate(&conn, &cfg, &ids);
        assert_eq!(report.ok.len(), 2);
        assert!(report.failed.is_empty());

        let report = delete(&conn, &cfg, &ids);
        assert_eq!(report.ok.len(), 2, "delete_broken supprime même un mod sans fichiers valides");
        assert!(overlay::get_mod(&conn, "good").unwrap().is_none());
        assert!(overlay::get_mod(&conn, "ghost").unwrap().is_none());

        let _ = std::fs::remove_dir_all(&base);
    }
}
