//! Pipeline d'import (L1) : extraction d'archive, descente récursive,
//! résolution d'identité (§4.2/§4.3), rangement dans la bibliothèque,
//! écriture overlay + historique. Le fichier du mod n'est jamais modifié.

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::{self, ModKind};
use crate::rules::Rules;
use crate::{archive, harmonize, identity, inspect, uijson};

/// Événement de progression émis pendant l'import (`import:progress`).
#[derive(Debug, Clone, Serialize)]
struct Progress {
    archive: String,
    /// "extract" | "scan" | "filing" | "done"
    phase: String,
    current: usize,
    total: usize,
    label: String,
}

/// Émetteur de progression. Branché sur les événements Tauri en production,
/// remplacé par un no-op dans les tests.
type ProgressFn<'a> = dyn Fn(Progress) + 'a;

#[derive(Debug, Clone, Serialize)]
pub struct FuzzyConflict {
    pub existing_id: String,
    pub existing_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedMod {
    pub id_interne: String,
    pub kind: String,
    pub display_name: Option<String>,
    /// "IMPORT" | "UPDATE_REPLACE"
    pub outcome: String,
    pub version_label: Option<String>,
    /// Renseigné si un mod existant ressemble fortement à celui-ci (§4.2).
    pub conflict: Option<FuzzyConflict>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveResult {
    pub archive: String,
    pub mods: Vec<ImportedMod>,
    pub error: Option<String>,
    /// Ressources partagées (fonts/drivers) installées globalement (§4.8).
    #[serde(default)]
    pub shared: Vec<crate::shared::SharedResult>,
    /// Sous-éléments rattachés (skins/sons) routés vers la bibliothèque (§12bis.2).
    #[serde(default)]
    pub subs: Vec<crate::submods::SubImported>,
}

/// Importe une liste d'archives. Chaque archive est traitée indépendamment ;
/// une erreur sur l'une n'interrompt pas les autres. Émet des événements
/// `import:progress` au fil de l'eau.
pub fn import_archives(
    app: &AppHandle,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
) -> Vec<ArchiveResult> {
    let emit = |p: Progress| {
        let _ = app.emit("import:progress", p);
    };
    run_import(&emit, conn, cfg, rules, paths)
}

fn run_import(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
) -> Vec<ArchiveResult> {
    paths
        .iter()
        .map(|p| import_one(emit, conn, cfg, rules, Path::new(p)))
        .collect()
}

fn import_one(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    archive_path: &Path,
) -> ArchiveResult {
    let archive_name = archive_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| archive_path.to_string_lossy().into_owned());

    let mut result =
        ArchiveResult { archive: archive_name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new() };

    let (Some(sevenzip), Some(library)) = (&cfg.sevenzip_exe, &cfg.library_path) else {
        result.error = Some("Chemins 7-Zip ou bibliothèque non configurés.".into());
        return result;
    };

    // Dossier de travail temporaire pour l'extraction.
    let workdir = match make_temp_dir() {
        Ok(d) => d,
        Err(e) => {
            result.error = Some(format!("dossier temporaire : {e}"));
            return result;
        }
    };

    emit(Progress {
        archive: archive_name.clone(),
        phase: "extract".into(),
        current: 0,
        total: 0,
        label: "Extraction de l'archive…".into(),
    });
    if let Err(e) = archive::extract(sevenzip, archive_path, &workdir) {
        result.error = Some(e);
        let _ = std::fs::remove_dir_all(&workdir);
        return result;
    }

    let found = modscan::scan(&workdir);
    // Sous-éléments rattachés (skins/sons) — peuvent constituer une archive seuls.
    let subs = modscan::scan_subs(&workdir);
    if found.is_empty() && subs.is_empty() {
        result.error = Some("Aucune voiture, circuit, skin ou son trouvé dans l'archive.".into());
        let _ = std::fs::remove_dir_all(&workdir);
        return result;
    }
    emit(Progress {
        archive: archive_name.clone(),
        phase: "scan".into(),
        current: 0,
        total: found.len(),
        label: format!("{} mod(s) trouvé(s)", found.len()),
    });

    // Pack (§4.7) : une archive multi-voitures regroupe ses mods sous sa source.
    let pack = (found.len() > 1).then_some(archive_name.as_str());
    for (i, fm) in found.iter().enumerate() {
        let label = fm
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        emit(Progress {
            archive: archive_name.clone(),
            phase: "filing".into(),
            current: i + 1,
            total: found.len(),
            label,
        });
        // Archive : le contenu vient d'un dossier temp → toujours déplacé.
        match process_found(conn, rules, library, &archive_name, fm, false, pack) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                // On consigne l'erreur sur l'archive mais on continue les autres mods.
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }

    // Sous-éléments rattachés (skins/sons) : stockage séparé (§12bis.2). Archive
    // → contenu en temp, toujours déplacé.
    if !subs.is_empty() {
        result.subs = crate::submods::import_subs(conn, cfg, library, &archive_name, &subs, false);
    }

    // Ressources partagées globales (fonts/drivers) avant nettoyage du temp (§4.8).
    if let Some(ac) = &cfg.ac_install_path {
        result.shared = crate::shared::install(ac, &workdir);
    }

    emit(Progress {
        archive: archive_name.clone(),
        phase: "done".into(),
        current: found.len(),
        total: found.len(),
        label: "Terminé".into(),
    });
    let _ = std::fs::remove_dir_all(&workdir);
    result
}

/// Import depuis des **dossiers déjà décompressés** (§4.5). Même pipeline que
/// les archives, sans décompression. `copy=true` préserve la source (copie),
/// sinon déplacement adaptatif (rename même disque, copie+suppression sinon).
pub fn import_folders(
    app: &AppHandle,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
    copy: bool,
) -> Vec<ArchiveResult> {
    let emit = |p: Progress| {
        let _ = app.emit("import:progress", p);
    };
    paths
        .iter()
        .map(|p| import_one_folder(&emit, conn, cfg, rules, Path::new(p), copy))
        .collect()
}

fn import_one_folder(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    dir: &Path,
    copy: bool,
) -> ArchiveResult {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    let mut result =
        ArchiveResult { archive: name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new() };

    let Some(library) = &cfg.library_path else {
        result.error = Some("Bibliothèque non configurée.".into());
        return result;
    };
    if !dir.is_dir() {
        result.error = Some("Le chemin n'est pas un dossier.".into());
        return result;
    }

    let found = modscan::scan(dir);
    let subs = modscan::scan_subs(dir);
    if found.is_empty() && subs.is_empty() {
        result.error = Some("Aucune voiture, circuit, skin ou son trouvé dans le dossier.".into());
        return result;
    }
    emit(Progress {
        archive: name.clone(),
        phase: "scan".into(),
        current: 0,
        total: found.len(),
        label: format!("{} mod(s) trouvé(s)", found.len()),
    });

    // Pack (§4.7) : un dossier multi-voitures regroupe ses mods sous sa source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for (i, fm) in found.iter().enumerate() {
        emit(Progress {
            archive: name.clone(),
            phase: "filing".into(),
            current: i + 1,
            total: found.len(),
            label: fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        });
        match process_found(conn, rules, library, &name, fm, copy, pack) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }

    // Sous-éléments rattachés (skins/sons), stockage séparé (§12bis.2).
    if !subs.is_empty() {
        result.subs = crate::submods::import_subs(conn, cfg, library, &name, &subs, copy);
    }

    // Ressources partagées globales (§4.8).
    if let Some(ac) = &cfg.ac_install_path {
        result.shared = crate::shared::install(ac, dir);
    }

    emit(Progress {
        archive: name.clone(),
        phase: "done".into(),
        current: found.len(),
        total: found.len(),
        label: "Terminé".into(),
    });
    result
}

// --- Import en masse (§4.6) -------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct BulkMod {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    /// "new" | "update" | "duplicate" | "ambiguous"
    pub status: String,
    pub existing_id: Option<String>,
    pub existing_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkEntry {
    pub subfolder: String,
    pub path: String,
    /// Sous-dossier sans structure AC reconnaissable.
    pub ignored: bool,
    pub mods: Vec<BulkMod>,
}

/// Classe un mod trouvé sans rien écrire (§4.6 phase d'analyse).
fn classify(
    conn: &Connection,
    id: &str,
    kind: &str,
    brand: &str,
    name: &str,
    dir: &Path,
) -> Result<(String, Option<String>, Option<String>), String> {
    if crate::overlay::mod_exists(conn, id).map_err(|e| e.to_string())? {
        let sig = identity::content_signature(dir);
        let existing = crate::overlay::active_signature(conn, id).map_err(|e| e.to_string())?;
        if existing.as_deref() == Some(sig.as_str()) {
            Ok(("duplicate".into(), None, None))
        } else {
            Ok(("update".into(), None, None))
        }
    } else {
        let fuzzy = crate::overlay::find_fuzzy(conn, kind, brand, name, id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .next();
        match fuzzy {
            Some(m) => Ok(("ambiguous".into(), Some(m.id_interne), m.display_name)),
            None => Ok(("new".into(), None, None)),
        }
    }
}

/// Phase d'analyse (§4.6) : scanne chaque sous-dossier direct du parent et
/// classe les mods **sans rien écrire**. Un seul niveau de sous-dossiers.
pub fn analyze_bulk(conn: &Connection, _cfg: &AppConfig, parent: &Path) -> Result<Vec<BulkEntry>, String> {
    if !parent.is_dir() {
        return Err("Le chemin n'est pas un dossier.".into());
    }
    let mut subs: Vec<PathBuf> = std::fs::read_dir(parent)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();

    let mut entries = Vec::new();
    for sub in subs {
        let subfolder = sub.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let path = sub.to_string_lossy().into_owned();
        let found = modscan::scan(&sub);
        if found.is_empty() {
            entries.push(BulkEntry { subfolder, path, ignored: true, mods: Vec::new() });
            continue;
        }
        let mut mods = Vec::new();
        for fm in &found {
            let id = fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let ui = match fm.kind {
                ModKind::Car => uijson::read_car(&fm.dir),
                ModKind::Track => uijson::read_track(&fm.dir),
            }
            .unwrap_or_default();
            let kind_str = format!("{:?}", fm.kind);
            let brand = ui.brand.clone().unwrap_or_default();
            let name = ui.name.clone().unwrap_or_else(|| id.clone());
            let (status, existing_id, existing_name) =
                classify(conn, &id, &kind_str, &brand, &name, &fm.dir)?;
            mods.push(BulkMod { id, kind: kind_str, name: ui.name, status, existing_id, existing_name });
        }
        entries.push(BulkEntry { subfolder, path, ignored: false, mods });
    }
    Ok(entries)
}

/// Instruction d'exécution pour un sous-dossier (après arbitrage).
#[derive(Debug, Clone, Deserialize)]
pub struct BulkExecItem {
    pub path: String,
    /// Ids de mods à ignorer (doublons que l'utilisateur ne réimporte pas).
    #[serde(default)]
    pub skip_ids: Vec<String>,
    /// Ids de mods ambigus à écraser (sinon « garder les deux »).
    #[serde(default)]
    pub replace_ids: Vec<String>,
}

/// Exécution de l'import en masse selon les décisions (§4.6). Reprenable :
/// relancer l'analyse après interruption reclasse les mods déjà importés en
/// « doublon »/« mise à jour », donc rien n'est traité de travers.
pub fn execute_bulk(
    app: &AppHandle,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    items: &[BulkExecItem],
    copy: bool,
) -> Vec<ArchiveResult> {
    let emit = |p: Progress| {
        let _ = app.emit("import:progress", p);
    };
    let total = items.len();
    items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let label = Path::new(&it.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            emit(Progress { archive: label.clone(), phase: "filing".into(), current: i + 1, total, label });
            exec_one(conn, cfg, rules, it, copy)
        })
        .collect()
}

fn exec_one(conn: &Connection, cfg: &AppConfig, rules: &Rules, it: &BulkExecItem, copy: bool) -> ArchiveResult {
    let dir = Path::new(&it.path);
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut result =
        ArchiveResult { archive: name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new() };

    let Some(library) = &cfg.library_path else {
        result.error = Some("Bibliothèque non configurée.".into());
        return result;
    };

    let found = modscan::scan(dir);
    // Pack (§4.7) : plusieurs mods issus du même dossier partagent leur source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for fm in &found {
        let id = fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if it.skip_ids.iter().any(|s| s == &id) {
            continue;
        }
        match process_found(conn, rules, library, &name, fm, copy, pack) {
            Ok(imported) => {
                if let Some(conflict) = imported.conflict.clone() {
                    let action = if it.replace_ids.iter().any(|r| r == &imported.id_interne) {
                        "replace"
                    } else {
                        "keep_both"
                    };
                    let _ = resolve_conflict(conn, cfg, &imported.id_interne, &conflict.existing_id, action);
                }
                result.mods.push(imported);
            }
            Err(e) => {
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }
    // Ressources partagées globales (§4.8).
    if let Some(ac) = &cfg.ac_install_path {
        result.shared = crate::shared::install(ac, dir);
    }
    result
}

/// Résout un conflit flou (§4.2) selon le choix utilisateur.
/// `keep_both` : conserve les deux mods séparément.
/// `replace`   : supprime l'ancien mod (fichiers + overlay) au profit du nouveau.
pub fn resolve_conflict(
    conn: &Connection,
    cfg: &AppConfig,
    new_id: &str,
    old_id: &str,
    action: &str,
) -> Result<(), String> {
    let now = Local::now().to_rfc3339();
    match action {
        "keep_both" => {
            crate::overlay::add_history(
                conn,
                new_id,
                &now,
                "UPDATE_KEPT_BOTH",
                &format!("conservé séparément de {old_id}"),
            )
            .map_err(|e| e.to_string())?;
        }
        "replace" => {
            if let Some(old) = crate::overlay::get_mod(conn, old_id).map_err(|e| e.to_string())? {
                let kind = if old.kind == "Track" { ModKind::Track } else { ModKind::Car };
                // Supprime les fichiers bibliothèque de l'ancien mod.
                if let Some(lib) = &cfg.library_path {
                    let dir = lib.join(kind.content_folder()).join(old_id);
                    let _ = std::fs::remove_dir_all(&dir);
                }
                // Retire une éventuelle junction orpheline dans content/ (garde-fou :
                // uniquement si c'est bien un point de reparse, jamais un vrai dossier).
                if let Some(ac) = &cfg.ac_install_path {
                    let cpath = ac.join("content").join(kind.content_folder()).join(old_id);
                    if let Ok(meta) = std::fs::symlink_metadata(&cpath) {
                        if meta.file_type().is_symlink() {
                            let _ = std::fs::remove_dir(&cpath);
                        }
                    }
                }
                crate::overlay::delete_mod(conn, old_id).map_err(|e| e.to_string())?;
            }
            crate::overlay::add_history(
                conn,
                new_id,
                &now,
                "UPDATE_REPLACE",
                &format!("a remplacé {old_id}"),
            )
            .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("action inconnue : {action}")),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_found(
    conn: &Connection,
    rules: &Rules,
    library: &Path,
    archive_name: &str,
    fm: &modscan::FoundMod,
    copy: bool,
    // Source de pack commune (§4.7) si l'import contient plusieurs mods.
    pack: Option<&str>,
) -> Result<ImportedMod, String> {
    let id_interne = fm
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or("dossier de mod sans nom")?;

    let ui = match fm.kind {
        ModKind::Car => uijson::read_car(&fm.dir),
        ModKind::Track => uijson::read_track(&fm.dir),
    }
    .unwrap_or_default();

    let kind_str = format!("{:?}", fm.kind); // "Car" | "Track"
    let brand = ui.brand.clone().unwrap_or_default();
    let name = ui.name.clone().unwrap_or_else(|| id_interne.clone());
    let signature = identity::content_signature(&fm.dir);
    let id_hash = identity::identity_hash(&id_interne, &brand, &name);
    let now = Local::now().to_rfc3339();

    // Features lues à la volée (lecture seule).
    let csp = inspect::csp_features(&fm.dir);
    let skins = match fm.kind {
        ModKind::Car => inspect::car_skins(&fm.dir),
        ModKind::Track => Vec::new(),
    };
    let layouts = match fm.kind {
        ModKind::Track => inspect::track_layouts(&fm.dir),
        ModKind::Car => Vec::new(),
    };

    // --- Résolution d'identité (§4.2) ---
    let is_update = crate::overlay::mod_exists(conn, &id_interne).map_err(|e| e.to_string())?;
    let conflict = if is_update {
        None
    } else {
        crate::overlay::find_fuzzy(conn, &kind_str, &brand, &name, &id_interne)
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .map(|m| FuzzyConflict { existing_id: m.id_interne, existing_name: m.display_name })
    };

    // --- Rangement bibliothèque ---
    let version_folder = version_folder_name(ui.version.as_deref(), &now);
    let dest = unique_dir(
        &library
            .join(fm.kind.content_folder())
            .join(&id_interne)
            .join(&version_folder),
    );
    // Copie (préserve la source) ou déplacement adaptatif (rename / copie+suppr).
    if copy {
        archive::copy_dir(&fm.dir, &dest).map_err(|e| format!("copie bibliothèque : {e}"))?;
    } else {
        archive::move_dir(&fm.dir, &dest).map_err(|e| format!("rangement bibliothèque : {e}"))?;
    }
    let library_path = dest.to_string_lossy().into_owned();

    // --- Écriture overlay ---
    crate::overlay::upsert_mod(
        conn,
        &id_interne,
        &kind_str,
        ui.brand.as_deref(),
        Some(&name),
        &id_hash,
        ui.year,
        &now,
    )
    .map_err(|e| e.to_string())?;

    // Source de pack (§4.7) — uniquement quand l'import regroupe plusieurs mods.
    if pack.is_some() {
        crate::overlay::set_source(conn, &id_interne, pack, None).map_err(|e| e.to_string())?;
    }

    let version_id = Uuid::new_v4().to_string();
    crate::overlay::insert_version(
        conn,
        &version_id,
        &id_interne,
        ui.version.as_deref(),
        ui.author.as_deref(),
        &now,
        &library_path,
        Some(archive_name),
        &signature,
        &csp,
        &skins,
        &layouts,
        &ui.tags,
    )
    .map_err(|e| e.to_string())?;

    crate::overlay::set_active_version(conn, &id_interne, &version_id).map_err(|e| e.to_string())?;

    // Harmonisation des tags + extraction specs/pays (§5.4), stockée en overlay.
    let class = ui.class.clone().unwrap_or_default();
    let h = harmonize::compute(rules, fm.kind, &ui.tags, &name, &class, ui.country.as_deref());
    harmonize::store(conn, &id_interne, &h, ui.country.as_deref()).map_err(|e| e.to_string())?;

    let outcome = if is_update { "UPDATE_REPLACE" } else { "IMPORT" };
    let details = match (&is_update, &ui.version) {
        (true, Some(v)) => format!("nouvelle version {v}"),
        (true, None) => "nouvelle version".to_string(),
        (false, _) => format!("depuis {archive_name}"),
    };
    crate::overlay::add_history(conn, &id_interne, &now, outcome, &details).map_err(|e| e.to_string())?;

    Ok(ImportedMod {
        id_interne,
        kind: kind_str,
        display_name: Some(name),
        outcome: outcome.to_string(),
        version_label: ui.version,
        conflict,
    })
}

/// Nom de dossier de version lisible hors de l'app : label assaini, sinon date.
fn version_folder_name(version: Option<&str>, now_rfc3339: &str) -> String {
    match version.map(sanitize).filter(|s| !s.is_empty()) {
        Some(v) => format!("v{v}"),
        None => {
            // "2026-06-26T22:40:13+02:00" -> "2026-06-26_2240"
            let compact: String = now_rfc3339
                .chars()
                .take(16)
                .map(|c| if c == 'T' { '_' } else { c })
                .filter(|c| *c != ':')
                .collect();
            format!("import-{compact}")
        }
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Garantit un chemin libre : ajoute un suffixe court si déjà pris.
fn unique_dir(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    let suffix = &Uuid::new_v4().to_string()[..8];
    let mut p = base.as_os_str().to_owned();
    p.push("-");
    p.push(suffix);
    PathBuf::from(p)
}

fn make_temp_dir() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("pitbox-import-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::process::Command;

    /// Crée un dossier voiture synthétique <root>/<id>/ui/ui_car.json + un .kn5.
    fn make_fake_car(root: &Path, id: &str) {
        let ui = root.join(id).join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(
            ui.join("ui_car.json"),
            r#"{"name":"My Test Car","brand":"TestBrand","tags":["gt3","turbo","italy"],"class":"race","year":2020,"version":"1.0","author":"Tester"}"#,
        )
        .unwrap();
        std::fs::write(root.join(id).join("model.kn5"), b"FAKE_KN5_DATA").unwrap();
    }

    /// Zippe <src>/* dans <zip> via 7-Zip.
    fn zip_dir(sevenzip: &Path, src: &Path, zip: &Path) {
        let status = Command::new(sevenzip)
            .current_dir(src)
            .arg("a")
            .arg(zip)
            .arg("*")
            .status()
            .unwrap();
        assert!(status.success(), "7z a a échoué");
    }

    #[test]
    fn full_import_pipeline() {
        // Nécessite 7-Zip ; sinon le test est ignoré silencieusement.
        let Some(sevenzip) = crate::detect::find_7zip() else {
            eprintln!("7-Zip introuvable — test ignoré");
            return;
        };

        let base = make_temp_dir().unwrap();
        let src = base.join("src");
        std::fs::create_dir_all(&src).unwrap();
        make_fake_car(&src, "test_car");
        let zip = base.join("test_car.zip");
        zip_dir(&sevenzip, &src, &zip);

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db_path = base.join("overlay.sqlite");
        let conn = crate::overlay::open(&db_path).unwrap();

        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // Émetteur no-op pour les tests (pas d'AppHandle).
        let noop = |_p: Progress| {};
        let rules = crate::rules::default_rules();

        // --- 1er import : NOUVEAU ---
        let zip_str = zip.to_string_lossy().into_owned();
        let res = run_import(&noop, &conn, &cfg, &rules, &[zip_str.clone()]);
        assert_eq!(res.len(), 1);
        assert!(res[0].error.is_none(), "erreur: {:?}", res[0].error);
        assert_eq!(res[0].mods.len(), 1);
        assert_eq!(res[0].mods[0].outcome, "IMPORT");
        assert_eq!(res[0].mods[0].id_interne, "test_car");

        // Fichier rangé dans la bibliothèque + ui_car.json préservé.
        let mods = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].display_name.as_deref(), Some("My Test Car"));
        assert_eq!(mods[0].version_count, 1);
        assert_eq!(mods[0].tags_from_mod, vec!["gt3", "turbo", "italy"]);

        // Harmonisation (§5.4) : gt3 -> #gt3 (catégorie), turbo -> aspiration,
        // italy -> pays ; les tags techniques/pays sont retirés du vocabulaire.
        assert_eq!(mods[0].category.as_deref(), Some("#gt3"));
        assert!(mods[0].tags_from_rule.contains(&"#gt3".to_string()));
        assert_eq!(mods[0].aspiration.as_deref(), Some("TURBO"));
        assert_eq!(mods[0].country.as_deref(), Some("Italy"));
        assert!(!mods[0].tags_from_rule.contains(&"turbo".to_string()));
        assert!(!mods[0].tags_from_rule.contains(&"italy".to_string()));
        let versions = crate::overlay::get_versions(&conn, "test_car").unwrap();
        let lib_ui = Path::new(&versions[0].library_path).join("ui").join("ui_car.json");
        assert!(lib_ui.is_file(), "ui_car.json doit exister dans la bibliothèque");

        // --- 2e import du même id : MISE À JOUR ---
        let res2 = run_import(&noop, &conn, &cfg, &rules, &[zip_str]);
        assert_eq!(res2[0].mods[0].outcome, "UPDATE_REPLACE");
        let mods2 = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods2.len(), 1, "toujours un seul mod logique");
        assert_eq!(mods2[0].version_count, 2, "deux versions coexistent");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn folder_import_copy_and_move() {
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        // Copie : la source est préservée.
        let src_copy = base.join("src_copy");
        make_fake_car(&src_copy, "copy_car");
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src_copy, true);
        assert_eq!(r.mods.len(), 1);
        assert_eq!(r.mods[0].outcome, "IMPORT");
        assert!(src_copy.join("copy_car").join("ui").join("ui_car.json").is_file(), "source préservée (copie)");
        assert!(library.join("cars").join("copy_car").exists(), "rangé dans la bibliothèque");

        // Déplacement : la source est retirée.
        let src_move = base.join("src_move");
        make_fake_car(&src_move, "move_car");
        let r2 = import_one_folder(&noop, &conn, &cfg, &rules, &src_move, false);
        assert_eq!(r2.mods.len(), 1);
        assert!(!src_move.join("move_car").exists(), "source retirée (déplacement)");
        assert!(library.join("cars").join("move_car").exists(), "rangé dans la bibliothèque");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pack_source_on_multi_car_folder() {
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        // Dossier « pack » contenant deux voitures.
        let pack = base.join("ferrari_pack");
        make_fake_car(&pack, "ferrari_a");
        make_fake_car(&pack, "ferrari_b");
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &pack, true);
        assert_eq!(r.mods.len(), 2);
        // Les deux mods partagent la même source de pack (§4.7).
        for id in ["ferrari_a", "ferrari_b"] {
            let m = crate::overlay::get_mod(&conn, id).unwrap().unwrap();
            assert_eq!(m.source_pack.as_deref(), Some("ferrari_pack"), "{id} doit pointer le pack");
        }

        // Un import mono-voiture ne crée pas de pack.
        let solo = base.join("solo");
        make_fake_car(&solo, "solo_car");
        import_one_folder(&noop, &conn, &cfg, &rules, &solo, true);
        let m = crate::overlay::get_mod(&conn, "solo_car").unwrap().unwrap();
        assert_eq!(m.source_pack, None, "mono-voiture → pas de pack");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bulk_analyze_and_execute() {
        let base = make_temp_dir().unwrap();
        let parent = base.join("catalog");
        std::fs::create_dir_all(&parent).unwrap();
        make_fake_car(&parent, "bulk_car"); // sous-dossier = voiture
        // Sous-dossier sans structure AC → ignoré.
        let notes = parent.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("readme.txt"), b"hi").unwrap();

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();

        // Analyse : 1 nouveau (bulk_car), 1 ignoré (notes). Aucune écriture.
        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        assert_eq!(entries.len(), 2);
        let car = entries.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert!(!car.ignored);
        assert_eq!(car.mods[0].status, "new");
        assert!(entries.iter().find(|e| e.subfolder == "notes").unwrap().ignored);
        assert!(!library.join("cars").join("bulk_car").exists(), "analyse n'écrit rien");

        // Exécution (copie).
        let item = BulkExecItem { path: car.path.clone(), skip_ids: vec![], replace_ids: vec![] };
        let r = exec_one(&conn, &cfg, &rules, &item, true);
        assert_eq!(r.mods.len(), 1);
        assert!(library.join("cars").join("bulk_car").exists());

        // Reprenable : ré-analyse → bulk_car devient un doublon (même contenu).
        let entries2 = analyze_bulk(&conn, &cfg, &parent).unwrap();
        let car2 = entries2.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert_eq!(car2.mods[0].status, "duplicate");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn fuzzy_conflict_and_replace() {
        let Some(sevenzip) = crate::detect::find_7zip() else {
            return;
        };
        let base = make_temp_dir().unwrap();
        let noop = |_p: Progress| {};
        let rules = crate::rules::default_rules();

        // Deux voitures avec des id de dossier différents mais même brand+name.
        let zip_for = |id: &str| -> String {
            let src = base.join(format!("src_{id}"));
            std::fs::create_dir_all(&src).unwrap();
            make_fake_car(&src, id);
            let zip = base.join(format!("{id}.zip"));
            zip_dir(&sevenzip, &src, &zip);
            zip.to_string_lossy().into_owned()
        };
        let zip_a = zip_for("car_a");
        let zip_b = zip_for("car_b");

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            sevenzip_exe: Some(sevenzip),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // 1er : pas de conflit (rien d'existant).
        let r1 = run_import(&noop, &conn, &cfg, &rules, &[zip_a]);
        assert!(r1[0].mods[0].conflict.is_none());

        // 2e : conflit flou vers car_a.
        let r2 = run_import(&noop, &conn, &cfg, &rules, &[zip_b]);
        let conflict = r2[0].mods[0].conflict.as_ref().expect("conflit attendu");
        assert_eq!(conflict.existing_id, "car_a");

        // Résolution "replace" : car_a disparaît, seul car_b reste.
        resolve_conflict(&conn, &cfg, "car_b", "car_a", "replace").unwrap();
        let mods = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].id_interne, "car_b");
        assert!(!library.join("cars").join("car_a").exists(), "fichiers car_a supprimés");

        let _ = std::fs::remove_dir_all(&base);
    }
}
