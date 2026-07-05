//! Nettoyage (§9.3) : détection assistée des **mods cassés** (fichiers de
//! bibliothèque manquants/invalides) et des **junctions orphelines** (pointant
//! vers une version supprimée). Porté de l'esprit de `clean.py`, mais non
//! destructif sans confirmation et respectant le garde-fou junction.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::ModRow;
use crate::{activation, inspect, overlay, uijson};

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BrokenMod {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrphanJunction {
    pub kind: String,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceReport {
    pub broken: Vec<BrokenMod>,
    pub orphans: Vec<OrphanJunction>,
}

/// Un mod est-il cassé (fichiers de sa version active manquants/invalides) ?
/// Renvoie une clé i18n (résolue côté frontend, pas du texte affichable), ou
/// `None` si tout va bien. Contenu de base (`is_stock`) jamais cassé (vrai
/// dossier du jeu, pas géré par nous). Partagé entre `scan` (§9.3, écran
/// Maintenance) et `library::to_card` (§6.4, badge sur la carte bibliothèque).
pub fn broken_reason(conn: &Connection, m: &ModRow) -> Option<String> {
    if m.is_stock {
        return None;
    }
    let kind = kind_of(&m.kind);
    let path = m
        .active_version_id
        .as_ref()
        .and_then(|vid| overlay::get_version_path(conn, vid).ok().flatten());
    match &path {
        None => Some("maintenance.reasonNoActiveVersion".to_string()),
        Some(p) => {
            let dir = Path::new(p);
            if !dir.is_dir() {
                Some("maintenance.reasonFilesMissing".to_string())
            } else if matches!(kind, ModKind::Car) && !dir.join("ui").join("ui_car.json").is_file() {
                Some("maintenance.reasonUiCarMissing".to_string())
            } else if matches!(kind, ModKind::Track) && !dir.join("ui").is_dir() {
                Some("maintenance.reasonUiDirMissing".to_string())
            } else {
                None
            }
        }
    }
}

/// Analyse la bibliothèque + `content/` sans rien supprimer (§9.3).
pub fn scan(conn: &Connection, cfg: &AppConfig) -> Result<MaintenanceReport, String> {
    let mut broken = Vec::new();
    let mut orphans = Vec::new();

    // --- Mods cassés : fichiers de la version active manquants/invalides ---
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        if let Some(reason) = broken_reason(conn, &m) {
            broken.push(BrokenMod { id: m.id_interne, kind: m.kind, name: m.display_name, reason });
        }
    }

    // --- Junctions orphelines : reparse présent mais cible illisible (supprimée) ---
    if let Some(ac) = &cfg.ac_install_path {
        for kind in [ModKind::Car, ModKind::Track] {
            let dir = ac.join("content").join(kind.content_folder());
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    // Junction dont on ne peut plus lister le contenu = cible disparue.
                    if activation::is_junction(&p) && std::fs::read_dir(&p).is_err() {
                        orphans.push(OrphanJunction {
                            kind: format!("{kind:?}"),
                            id: e.file_name().to_string_lossy().into_owned(),
                            path: p.to_string_lossy().into_owned(),
                        });
                    }
                }
            }
        }
    }

    Ok(MaintenanceReport { broken, orphans })
}

/// Supprime un mod cassé : fichiers de bibliothèque (toutes versions) + junction
/// éventuelle (garde-fou) + overlay.
pub fn delete_broken(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, id).map_err(|e| e.to_string())?.ok_or("mod introuvable")?;
    let kind = kind_of(&m.kind);

    // Fichiers : versions individuelles + dossier parent du mod dans la bibliothèque.
    for v in overlay::get_versions(conn, id).map_err(|e| e.to_string())? {
        let _ = std::fs::remove_dir_all(Path::new(&v.library_path));
    }
    if let Some(lib) = &cfg.library_path {
        let _ = std::fs::remove_dir_all(lib.join(kind.content_folder()).join(id));
    }

    // Junction éventuelle dans content/ (uniquement si c'est bien une junction).
    if let Some(ac) = &cfg.ac_install_path {
        let link = ac.join("content").join(kind.content_folder()).join(id);
        if activation::is_junction(&link) {
            let _ = std::fs::remove_dir(&link);
        }
    }

    overlay::delete_mod(conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Désinstalle **tout un pack** (§4.7) : supprime chaque mod partageant ce
/// `source_pack` (fichiers + junction + overlay). Renvoie le nombre supprimé.
pub fn delete_pack(conn: &Connection, cfg: &AppConfig, pack: &str) -> Result<usize, String> {
    let ids = overlay::list_pack_ids(conn, pack).map_err(|e| e.to_string())?;
    let mut n = 0;
    for id in &ids {
        // `delete_broken` réalise la suppression complète d'un mod (cf. §9.3).
        delete_broken(conn, cfg, id)?;
        n += 1;
    }
    Ok(n)
}

/// Retire une junction orpheline. Garde-fou : refuse si ce n'est pas une junction.
pub fn remove_orphan(cfg: &AppConfig, kind: &str, id: &str) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?;
    let link = ac.join("content").join(kind_of(kind).content_folder()).join(id);
    activation::remove_junction(&link)
}

/// Relit le `ui_*.json` (et l'inspection CSP/skins/layouts) de chaque version
/// d'un mod depuis son `library_path` déjà en bibliothèque, et met à jour les
/// champs en cache dans l'overlay. Ne réécrit jamais les fichiers du mod
/// lui-même (lecture seule, §3.0) — sert à rattraper un mod déjà importé dont
/// le fichier source a changé, ou dont le parsing a été corrigé après coup.
///
/// `recalc_size` (§9.4) : recalcule en plus la taille sur disque de chaque
/// version. Décorrélé du reste (case à cocher dédiée côté UI, décochée par
/// défaut) car parcourir tous les fichiers de toute la bibliothèque peut être
/// lent — la plupart des réindexations n'ont pas besoin de ça (la taille ne
/// change que si les fichiers du mod ont été modifiés hors de l'app).
pub fn reindex_mod(conn: &Connection, cfg: &AppConfig, id: &str, recalc_size: bool) -> Result<(), String> {
    let m = overlay::get_mod(conn, id).map_err(|e| e.to_string())?.ok_or("mod introuvable")?;
    let kind = kind_of(&m.kind);
    let versions = overlay::get_versions(conn, id).map_err(|e| e.to_string())?;

    let mut fresh_for_mod = None;
    for v in &versions {
        let dir = Path::new(&v.library_path);
        let ui = match kind {
            ModKind::Car => uijson::read_car(dir),
            ModKind::Track => uijson::read_track(dir),
        }
        .unwrap_or_default();
        // Config CSP propre au mod + config "chargée" séparément par CSP
        // (hors du mod, §6.4bis) — sans cette seconde source, le contenu de
        // base Kunos ne remonte quasiment jamais de features CSP.
        let mut csp = inspect::csp_features(dir);
        if let Some(ac) = &cfg.ac_install_path {
            csp.extend(inspect::csp_features_loaded(ac, kind, id));
        }
        csp.sort();
        csp.dedup();
        let skins = match kind {
            ModKind::Car => inspect::car_skins(dir),
            ModKind::Track => Vec::new(),
        };
        let layouts = match kind {
            ModKind::Track => inspect::track_layouts(dir),
            ModKind::Car => Vec::new(),
        };
        overlay::update_version_reindexed_fields(
            conn,
            &v.id,
            ui.version.as_deref(),
            ui.author.as_deref(),
            &csp,
            &skins,
            &layouts,
            &ui.tags,
        )
        .map_err(|e| e.to_string())?;

        if recalc_size {
            let size_bytes = inspect::dir_size_bytes(dir) as i64;
            overlay::update_version_size(conn, &v.id, size_bytes).map_err(|e| e.to_string())?;
        }

        if m.active_version_id.as_deref() == Some(v.id.as_str()) {
            fresh_for_mod = Some(ui);
        }
    }

    // Nom/marque/année du mod : reflète la version active, sinon la dernière lue.
    let fresh_for_mod = fresh_for_mod.or_else(|| {
        let dir = Path::new(&versions.last()?.library_path);
        match kind {
            ModKind::Car => uijson::read_car(dir),
            ModKind::Track => uijson::read_track(dir),
        }
    });
    if let Some(ui) = fresh_for_mod {
        overlay::update_mod_reindexed_fields(conn, id, ui.brand.as_deref(), ui.name.as_deref(), ui.year)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Réindexe tous les mods de la bibliothèque (§9.3bis). Renvoie le nombre traité.
pub fn reindex_all(conn: &Connection, cfg: &AppConfig, recalc_size: bool) -> Result<usize, String> {
    let mods = overlay::list_mods(conn).map_err(|e| e.to_string())?;
    for m in &mods {
        reindex_mod(conn, cfg, &m.id_interne, recalc_size)?;
    }
    Ok(mods.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_broken_mod_missing_files() {
        let base = std::env::temp_dir().join(format!("pitbox-maint-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        // Mod dont la version pointe vers un dossier inexistant.
        overlay::upsert_mod(&conn, "ghost", "Car", Some("B"), Some("Ghost"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "ghost", Some("1.0"), None, &now,
            &base.join("nope").to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "ghost", "v1").unwrap();

        let report = scan(&conn, &cfg).unwrap();
        assert_eq!(report.broken.len(), 1);
        assert_eq!(report.broken[0].id, "ghost");

        delete_broken(&conn, &cfg, "ghost").unwrap();
        assert!(overlay::get_mod(&conn, "ghost").unwrap().is_none());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn reindex_fixes_name_read_from_non_utf8_ui_json() {
        let base = std::env::temp_dir().join(format!("pitbox-reindex-{}", uuid::Uuid::new_v4()));
        let track_dir = base.join("deutschlandring");
        std::fs::create_dir_all(track_dir.join("ui")).unwrap();
        // Fichier réel : "name" correct, mais un octet Windows-1252 (° en 0xB0)
        // plus loin dans "geotags" rend tout le fichier invalide en UTF-8 strict.
        let mut bytes = b"{\"name\": \"Deutschlandring\", \"author\": \"Fat-Alfie\", \"tags\": [\"circuit\"], \"geotags\": [\"51.8".to_vec();
        bytes.push(0xB0); // degré Windows-1252, invalide en UTF-8
        bytes.extend_from_slice(b" N\"]}");
        std::fs::write(track_dir.join("ui").join("ui_track.json"), &bytes).unwrap();
        assert!(String::from_utf8(bytes).is_err(), "le fixture doit reproduire un fichier non-UTF-8");

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        // Simule l'état bugué : import précédent retombé sur le nom de dossier.
        overlay::upsert_mod(&conn, "deutschlandring", "Track", None, Some("deutschlandring"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "deutschlandring", None, None, &now,
            &track_dir.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "deutschlandring", "v1").unwrap();

        reindex_mod(&conn, &AppConfig::default(), "deutschlandring", false).unwrap();

        let m = overlay::get_mod(&conn, "deutschlandring").unwrap().unwrap();
        assert_eq!(m.display_name.as_deref(), Some("Deutschlandring"));
        let versions = overlay::get_versions(&conn, "deutschlandring").unwrap();
        assert_eq!(versions[0].author.as_deref(), Some("Fat-Alfie"));
        assert_eq!(versions[0].tags_from_mod, vec!["circuit".to_string()]);
        assert_eq!(versions[0].size_bytes, None, "recalc_size=false : taille non touchée");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn reindex_recalculates_size_only_when_requested() {
        let base = std::env::temp_dir().join(format!("pitbox-size-{}", uuid::Uuid::new_v4()));
        let car_dir = base.join("abarth500");
        std::fs::create_dir_all(car_dir.join("ui")).unwrap();
        std::fs::write(car_dir.join("ui").join("ui_car.json"), b"{\"name\": \"Abarth 500\"}").unwrap();
        std::fs::write(car_dir.join("data.acd"), vec![0u8; 1000]).unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "abarth500", "Car", None, Some("Abarth 500"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "abarth500", None, None, &now,
            &car_dir.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "abarth500", "v1").unwrap();

        // Sans la case cochée : la taille reste vide (None), pas de parcours disque.
        reindex_mod(&conn, &AppConfig::default(), "abarth500", false).unwrap();
        let m = overlay::get_mod(&conn, "abarth500").unwrap().unwrap();
        assert_eq!(m.size_bytes, None);

        // Avec la case cochée : la taille est calculée et remontée agrégée sur le mod.
        reindex_mod(&conn, &AppConfig::default(), "abarth500", true).unwrap();
        let m = overlay::get_mod(&conn, "abarth500").unwrap().unwrap();
        assert!(m.size_bytes.unwrap() >= 1000, "au moins les 1000 octets de data.acd");

        let _ = std::fs::remove_dir_all(&base);
    }
}
