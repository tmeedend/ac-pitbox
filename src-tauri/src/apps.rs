//! Apps Python d'AC (§12bis.4) : type **autonome** (ni voiture, ni circuit, ni
//! sous-élément). Stockées dans la bibliothèque, activables/désactivables par
//! junction comme le reste, vers `<ac>/apps/python/<id>`. Pas de fiche ni de
//! tags en v1 — juste nom, état, activation.

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::FoundApp;
use crate::resources::{self, ExtractionMode};
use crate::{activation, overlay};

#[derive(Debug, Clone, Serialize)]
pub struct AppImported {
    pub name: String,
    /// Fichiers annexes redirigés vers le dossier ressources (§4.6).
    pub resources_extracted: usize,
}

/// App avec son état d'activation (junction présente) pour la vue dédiée.
#[derive(Debug, Clone, Serialize)]
pub struct AppItem {
    pub id: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub active: bool,
}

/// Lien d'activation d'une app : `<ac>/apps/python/<id>`.
fn app_link(cfg: &AppConfig, id: &str) -> Option<PathBuf> {
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("apps").join("python").join(id))
}

/// Importe les apps détectées : stockage bibliothèque + enregistrement (§12bis.4).
pub fn import_apps(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    apps: &[FoundApp],
    copy: bool,
    mode: ExtractionMode,
) -> Vec<AppImported> {
    let mut out = Vec::new();
    for app in apps {
        let dest = library.join("apps").join(&app.name);
        // Ré-import : on remplace les fichiers existants (les ressources déjà
        // extraites, elles, sont conservées — dossier séparé, mod-level).
        if dest.exists() {
            let _ = std::fs::remove_dir_all(&dest);
        }
        // Fichiers annexes (§4.6) redirigés à part : une image à la racine
        // d'une app peut être une icône réellement utilisée par le script
        // (allow_root_images=false, jamais présumée annexe).
        let res_dir = resources::resources_dir_for(library, "apps", &[&app.name]);
        let Ok(resources_extracted) = resources::file_mod(&app.dir, &dest, &res_dir, mode, !copy, false) else {
            continue;
        };
        let _ = overlay::insert_app(
            conn,
            &app.name,
            &dest.to_string_lossy(),
            Some(source_name),
            &Local::now().to_rfc3339(),
        );
        out.push(AppImported { name: app.name.clone(), resources_extracted });
    }
    out
}

/// Liste les apps avec leur état d'activation (junction présente).
pub fn list_apps(conn: &Connection, cfg: &AppConfig) -> Result<Vec<AppItem>, String> {
    let rows = overlay::list_apps(conn).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|a| {
            let active = app_link(cfg, &a.id).is_some_and(|l| activation::is_junction(&l));
            AppItem { id: a.id, source_archive: a.source_archive, imported_at: a.imported_at, active }
        })
        .collect())
}

/// Active une app : junction `<ac>/apps/python/<id>` → dossier bibliothèque.
pub fn activate_app(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let app = overlay::get_app(conn, id).map_err(|e| e.to_string())?.ok_or("app introuvable")?;
    let link = app_link(cfg, id).ok_or("dossier AC non configuré")?;

    // Garde-fou : ne jamais écraser un vrai dossier (app installée hors de l'app).
    match std::fs::symlink_metadata(&link) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                activation::remove_junction(&link)?;
            } else {
                return Err("un vrai dossier d'app existe déjà — opération refusée".into());
            }
        }
        Err(_) => {}
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    activation::create_junction(&link, Path::new(&app.library_path))
}

/// Désactive une app : retire la junction (garde-fou junction).
pub fn deactivate_app(cfg: &AppConfig, id: &str) -> Result<(), String> {
    let link = app_link(cfg, id).ok_or("dossier AC non configuré")?;
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => activation::remove_junction(&link),
        Ok(_) => Err("un vrai dossier d'app existe — non touché".into()),
        Err(_) => Ok(()), // déjà inactive
    }
}

/// Supprime proprement une app : désactive (retire la junction), efface les
/// fichiers de bibliothèque, puis la ligne overlay (§12bis.4).
pub fn remove_app(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let app = overlay::get_app(conn, id).map_err(|e| e.to_string())?.ok_or("app introuvable")?;
    // Désactive si une junction est présente (ignore l'absence).
    if let Some(link) = app_link(cfg, id) {
        if activation::is_junction(&link) {
            let _ = activation::remove_junction(&link);
        }
    }
    let _ = std::fs::remove_dir_all(Path::new(&app.library_path));
    overlay::delete_app(conn, id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;

    #[test]
    fn app_detected_and_imported() {
        let base = std::env::temp_dir().join(format!("pitbox-app-{}", uuid::Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        // App AC : apps/python/MyApp/MyApp.py
        let app = base.join("src").join("apps").join("python").join("MyApp");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("MyApp.py"), b"# app").unwrap();

        let found = modscan::scan_apps(&base.join("src"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "MyApp");

        let res = import_apps(&conn, &library, "myapp.7z", &found, true, ExtractionMode::InfoOnly);
        assert_eq!(res.len(), 1);
        assert!(library.join("apps").join("MyApp").join("MyApp.py").is_file());
        assert!(overlay::app_exists(&conn, "MyApp").unwrap());

        // Suppression propre : fichiers + overlay effacés (pas de junction ici).
        let cfg = AppConfig::default();
        remove_app(&conn, &cfg, "MyApp").unwrap();
        assert!(!library.join("apps").join("MyApp").exists());
        assert!(!overlay::app_exists(&conn, "MyApp").unwrap());

        let _ = std::fs::remove_dir_all(&base);
    }
}
