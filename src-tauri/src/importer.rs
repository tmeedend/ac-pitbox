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
use crate::{archive, harmonize, identity, inspect, layers, uijson};

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
    /// "IMPORT" | "UPDATE_REPLACE" | "DUPLICATE" | "EXTENSION" | "AMBIGUOUS" (§4.4).
    /// - EXTENSION : rangé comme couche à part, la base n'est jamais touchée.
    /// - AMBIGUOUS : rien écrit, on attend le choix de l'utilisateur.
    pub outcome: String,
    pub version_label: Option<String>,
    /// Renseigné si un mod existant ressemble fortement à celui-ci (§4.2).
    pub conflict: Option<FuzzyConflict>,
    /// Décompte de comparaison (§4.4), pour la modale et le rapport. Renseigné
    /// pour EXTENSION et AMBIGUOUS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwritten_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_total: Option<usize>,
    /// Fichiers annexes redirigés vers le dossier ressources du mod (§4.6),
    /// selon le réglage global. 0 si rien n'a été filé (doublon, ambigu bloqué).
    pub resources_extracted: usize,
}

/// Décision explicite de l'utilisateur pour un mod resté ambigu (§4.4),
/// renvoyée par le front pour reprendre l'import.
#[derive(Debug, Clone, Deserialize)]
pub struct ImportDecision {
    pub id: String,
    /// "update" | "extension"
    pub decision: String,
}

/// Classement d'un import ciblant un contenu existant (§4.4).
#[derive(Debug, Clone, Copy, PartialEq)]
enum ImportClass {
    /// Vraie mise à jour : remplace la version active.
    Update,
    /// Couche/extension : surtout des chemins nouveaux → rangée à part.
    Extension,
    /// Indécidable automatiquement → demander à l'utilisateur.
    Ambiguous,
}

/// Classe une comparaison de contenu (§4.4). `coverage` = part de la base qui
/// serait écrasée. Peu d'écrasements + surtout du neuf = extension ; recouvrement
/// large = mise à jour ; entre les deux = ambigu (on demande).
fn classify_diff(d: &crate::identity::DiffStats) -> ImportClass {
    if d.existing_total == 0 {
        return ImportClass::Update; // rien à comparer : comportement historique
    }
    if d.overwritten == 0 {
        return ImportClass::Extension; // addition pure (ex. nouveau layout)
    }
    let coverage = d.overwritten as f64 / d.existing_total as f64;
    if coverage >= 0.6 {
        ImportClass::Update
    } else if coverage <= 0.15 && d.added >= d.overwritten {
        ImportClass::Extension
    } else {
        ImportClass::Ambiguous
    }
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
    /// Apps Python importées (§12bis.4).
    #[serde(default)]
    pub apps: Vec<crate::apps::AppImported>,
    /// Mods « autres » importés — type non reconnu, jamais perdus (§6.1bis).
    #[serde(default)]
    pub others: Vec<crate::others::OtherImported>,
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
    decisions: &[ImportDecision],
) -> Vec<ArchiveResult> {
    let emit = |p: Progress| {
        let _ = app.emit("import:progress", p);
    };
    run_import(&emit, conn, cfg, rules, paths, decisions)
}

fn run_import(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    paths: &[String],
    decisions: &[ImportDecision],
) -> Vec<ArchiveResult> {
    paths
        .iter()
        .map(|p| import_one(emit, conn, cfg, rules, Path::new(p), decisions))
        .collect()
}

/// Décision utilisateur mémorisée pour cet id, s'il y en a une (§4.4).
fn decision_for<'a>(decisions: &'a [ImportDecision], id: &str) -> Option<&'a str> {
    decisions.iter().find(|d| d.id == id).map(|d| d.decision.as_str())
}

/// Copie l'archive/dossier source dans un espace dédié de la bibliothèque
/// (§10/§11), pour permettre plus tard « Réinstaller depuis l'archive source ».
/// Uniquement si le réglage `keep_source_archive` est actif — sinon jamais
/// reposé à chaque import. Best-effort : une copie échouée (disque plein…)
/// ne doit jamais interrompre l'import (`None` en ce cas).
fn keep_source(cfg: &AppConfig, source: &Path, label: &str) -> Option<String> {
    if !cfg.prefs.keep_source_archive {
        return None;
    }
    let library = cfg.library_path.as_ref()?;
    let dest_dir = library.join("_source_archives").join(Uuid::new_v4().to_string());
    if source.is_dir() {
        let dest = dest_dir.join(label);
        archive::copy_dir(source, &dest).ok()?;
        Some(dest.to_string_lossy().into_owned())
    } else {
        std::fs::create_dir_all(&dest_dir).ok()?;
        let dest = dest_dir.join(label);
        std::fs::copy(source, &dest).ok()?;
        Some(dest.to_string_lossy().into_owned())
    }
}

/// Id interne dérivé du dossier d'un mod trouvé (nom du dossier `content/<type>s/<id>`).
fn fm_id(fm: &modscan::FoundMod) -> String {
    fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Active par défaut les mods fraîchement importés/mis à jour (§4.6bis) : on veut
/// pouvoir conduire tout de suite. Best-effort (l'échec — ex. vrai dossier Kunos
/// homonyme, dossier AC non configuré — n'interrompt pas l'import).
fn auto_activate(conn: &Connection, cfg: &AppConfig, mods: &[ImportedMod]) {
    for m in mods {
        if m.outcome == "IMPORT" || m.outcome == "UPDATE_REPLACE" {
            let _ = crate::activation::activate(conn, cfg, &m.id_interne, None);
        }
    }
}

fn import_one(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    archive_path: &Path,
    decisions: &[ImportDecision],
) -> ArchiveResult {
    let archive_name = archive_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| archive_path.to_string_lossy().into_owned());

    let mut result =
        ArchiveResult { archive: archive_name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new(), apps: Vec::new(), others: Vec::new() };

    let (Some(sevenzip), Some(library)) = (&cfg.sevenzip_exe, &cfg.library_path) else {
        result.error = Some("Chemins 7-Zip ou bibliothèque non configurés.".into());
        return result;
    };
    // Extraction des fichiers annexes (§4.6) : réglage global, jamais reposé
    // à chaque import.
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);

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
    // Sous-éléments rattachés (skins/sons) et apps — peuvent constituer une archive seuls.
    let subs = modscan::scan_subs(&workdir);
    let apps = modscan::scan_apps(&workdir);
    if found.is_empty() && subs.is_empty() && apps.is_empty() {
        // Type non reconnu : jamais perdu, rangé comme « autre mod » (§6.1bis).
        if let Some(other) = crate::others::import_other(conn, library, &archive_name, &workdir, false, res_mode) {
            let _ = crate::others::activate_other(conn, cfg, &other.id);
            result.others.push(other);
        } else {
            result.error = Some("Aucune voiture, circuit, skin, son ou app trouvé dans l'archive.".into());
        }
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

    // Conservation de l'archive source (§10/§11) : une seule copie, partagée
    // entre tous les mods trouvés dans cette archive (évite de la dupliquer
    // par voiture pour un pack multi-voitures).
    let kept_archive = keep_source(cfg, archive_path, &archive_name);

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
        let decision = decision_for(decisions, &fm_id(fm));
        match process_found(conn, cfg, rules, library, &archive_name, fm, false, pack, decision, true, kept_archive.as_deref()) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                // On consigne l'erreur sur l'archive mais on continue les autres mods.
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }

    // Activation par défaut des mods importés (§4.6bis).
    auto_activate(conn, cfg, &result.mods);

    // Sous-éléments rattachés (skins/sons) : stockage séparé (§12bis.2). Archive
    // → contenu en temp, toujours déplacé.
    if !subs.is_empty() {
        result.subs = crate::submods::import_subs(conn, cfg, library, &archive_name, &subs, false, res_mode);
    }
    // Apps Python (§12bis.4).
    if !apps.is_empty() {
        result.apps = crate::apps::import_apps(conn, library, &archive_name, &apps, false, res_mode);
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
    decisions: &[ImportDecision],
) -> Vec<ArchiveResult> {
    let emit = |p: Progress| {
        let _ = app.emit("import:progress", p);
    };
    paths
        .iter()
        .map(|p| import_one_folder(&emit, conn, cfg, rules, Path::new(p), copy, decisions))
        .collect()
}

fn import_one_folder(
    emit: &ProgressFn,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    dir: &Path,
    copy: bool,
    decisions: &[ImportDecision],
) -> ArchiveResult {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string_lossy().into_owned());
    let mut result =
        ArchiveResult { archive: name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new(), apps: Vec::new(), others: Vec::new() };

    let Some(library) = &cfg.library_path else {
        result.error = Some("Bibliothèque non configurée.".into());
        return result;
    };
    if !dir.is_dir() {
        result.error = Some("Le chemin n'est pas un dossier.".into());
        return result;
    }
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);

    let found = modscan::scan(dir);
    let subs = modscan::scan_subs(dir);
    let apps = modscan::scan_apps(dir);
    if found.is_empty() && subs.is_empty() && apps.is_empty() {
        // Type non reconnu : jamais perdu, rangé comme « autre mod » (§6.1bis).
        if let Some(other) = crate::others::import_other(conn, library, &name, dir, copy, res_mode) {
            let _ = crate::others::activate_other(conn, cfg, &other.id);
            result.others.push(other);
        } else {
            result.error = Some("Aucune voiture, circuit, skin, son ou app trouvé dans le dossier.".into());
        }
        return result;
    }
    emit(Progress {
        archive: name.clone(),
        phase: "scan".into(),
        current: 0,
        total: found.len(),
        label: format!("{} mod(s) trouvé(s)", found.len()),
    });

    // Conservation du dossier source (§10/§11) : copié AVANT le rangement (qui
    // peut déplacer/consommer `dir` si `copy=false`), une seule fois pour tout
    // le dossier (partagé si plusieurs mods dedans).
    let kept_archive = keep_source(cfg, dir, &name);

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
        let decision = decision_for(decisions, &fm_id(fm));
        match process_found(conn, cfg, rules, library, &name, fm, copy, pack, decision, true, kept_archive.as_deref()) {
            Ok(imported) => result.mods.push(imported),
            Err(e) => {
                result.error.get_or_insert_with(String::new).push_str(&format!("{e}; "));
            }
        }
    }

    // Activation par défaut des mods importés (§4.6bis).
    auto_activate(conn, cfg, &result.mods);

    // Sous-éléments rattachés (skins/sons), stockage séparé (§12bis.2).
    if !subs.is_empty() {
        result.subs = crate::submods::import_subs(conn, cfg, library, &name, &subs, copy, res_mode);
    }
    // Apps Python (§12bis.4).
    if !apps.is_empty() {
        result.apps = crate::apps::import_apps(conn, library, &name, &apps, copy, res_mode);
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
        ArchiveResult { archive: name.clone(), mods: Vec::new(), error: None, shared: Vec::new(), subs: Vec::new(), apps: Vec::new(), others: Vec::new() };

    let Some(library) = &cfg.library_path else {
        result.error = Some("Bibliothèque non configurée.".into());
        return result;
    };

    let found = modscan::scan(dir);
    // Conservation du dossier source (§10/§11), avant tout rangement.
    let kept_archive = keep_source(cfg, dir, &name);
    // Pack (§4.7) : plusieurs mods issus du même dossier partagent leur source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for fm in &found {
        let id = fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if it.skip_ids.iter().any(|s| s == &id) {
            continue;
        }
        // Import en masse (§4.6) : jamais de blocage au fil de l'eau. Un cas
        // ambigu retombe sur le défaut sûr (extension, jamais destructif).
        match process_found(conn, cfg, rules, library, &name, fm, copy, pack, None, false, kept_archive.as_deref()) {
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
    // Activation par défaut des mods importés (§4.6bis).
    auto_activate(conn, cfg, &result.mods);
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
                &serde_json::json!({ "key": "keptBoth", "old": old_id }).to_string(),
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
                // Retire un déploiement orphelin dans content/ (garde-fou : symlink
                // hérité ou déploiement hardlinks marqué, jamais un vrai dossier).
                if let Some(ac) = &cfg.ac_install_path {
                    let cpath = ac.join("content").join(kind.content_folder()).join(old_id);
                    if let Ok(meta) = std::fs::symlink_metadata(&cpath) {
                        if meta.file_type().is_symlink() {
                            let _ = std::fs::remove_dir(&cpath);
                        } else if crate::deploy::is_deployed(&cpath) {
                            let _ = crate::deploy::remove_deployment(&cpath);
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
                &serde_json::json!({ "key": "replaced", "old": old_id }).to_string(),
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
    cfg: &AppConfig,
    rules: &Rules,
    library: &Path,
    archive_name: &str,
    fm: &modscan::FoundMod,
    copy: bool,
    // Source de pack commune (§4.7) si l'import contient plusieurs mods.
    pack: Option<&str>,
    // Décision utilisateur pour un cas ambigu (§4.4) : "update" | "extension".
    decision: Option<&str>,
    // Import unitaire (true) : bloque et demande sur cas ambigu. Import en masse
    // (false) : jamais de blocage, un cas ambigu retombe sur le défaut sûr.
    block_ambiguous: bool,
    // Archive/dossier source déjà conservé pour cet import (§10/§11), partagé
    // entre tous les mods d'une même archive/dossier. `None` si le réglage est
    // désactivé ou la copie a échoué.
    kept_archive: Option<&str>,
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
    // Extraction des fichiers annexes (§4.6) : réglage global, jamais reposé
    // à chaque import.
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);
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
    // Date de publication estimée (§6.2), lue sur les fichiers avant rangement.
    let published_at = inspect::estimate_published_at(&fm.dir);

    // --- Résolution d'identité (§4.2/§4.4) ---
    let existing = crate::overlay::get_mod(conn, &id_interne).map_err(|e| e.to_string())?;
    let is_update = existing.is_some();

    // Contenu ciblant un id déjà connu : décider mise à jour vs couche/extension
    // AVANT d'agir (§4.4), pour ne jamais détruire du contenu par un faux « MAJ ».
    if let Some(existing) = &existing {
        // Ré-import à l'identique : même id ET même signature → ni version ni
        // couche (évite le faux « MAJ » quand on réimporte la même archive).
        let active_sig = crate::overlay::active_signature(conn, &id_interne).map_err(|e| e.to_string())?;
        if active_sig.as_deref() == Some(signature.as_str()) {
            return Ok(ImportedMod {
                id_interne,
                kind: kind_str,
                display_name: Some(name),
                outcome: "DUPLICATE".into(),
                version_label: ui.version,
                conflict: None,
                added_count: None,
                overwritten_count: None,
                existing_total: None,
                resources_extracted: 0,
            });
        }

        // Règle absolue (§4.4) : le contenu de base Kunos (is_stock) ne reçoit
        // JAMAIS de remplacement — toujours une couche par-dessus. Sinon,
        // comparer les fichiers pour classer update / extension / ambigu.
        let (class, diff) = if existing.is_stock {
            // Toujours une extension (règle absolue). On calcule néanmoins le
            // décompte pour l'affichage, en comparant au dossier de base Kunos
            // (content/<type>s/<id>) : le stock n'a pas de version bibliothèque.
            let diff = cfg.ac_install_path.as_ref().and_then(|ac| {
                let base = ac.join("content").join(fm.kind.content_folder()).join(&id_interne);
                base.is_dir().then(|| identity::diff_content(&fm.dir, &base))
            });
            (ImportClass::Extension, diff)
        } else {
            let diff = match crate::overlay::active_library_path(conn, &id_interne).map_err(|e| e.to_string())? {
                Some(active_path) => identity::diff_content(&fm.dir, Path::new(&active_path)),
                // Pas de dossier de base à comparer : on ne peut pas prouver que
                // c'est une extension → comportement historique (mise à jour).
                None => crate::identity::DiffStats { added: 0, overwritten: 0, existing_total: 0 },
            };
            (classify_diff(&diff), Some(diff))
        };

        // La décision explicite de l'utilisateur (§4.4) prime sur l'auto-classement.
        let resolved = match decision {
            Some("update") => ImportClass::Update,
            Some("extension") => ImportClass::Extension,
            _ => class,
        };

        match resolved {
            ImportClass::Update => { /* poursuit vers le chemin UPDATE_REPLACE ci-dessous */ }
            ImportClass::Extension => {
                // Range comme couche à part — ne touche jamais la base (§4.4).
                let name_layer = layer_name(fm, archive_name);
                let (_, resources_extracted) = layers::store_layer(
                    conn,
                    library,
                    &id_interne,
                    fm.kind,
                    &name_layer,
                    &fm.dir,
                    copy,
                    diff.as_ref().unwrap_or(&crate::identity::DiffStats { added: 0, overwritten: 0, existing_total: 0 }),
                    archive_name,
                    res_mode,
                )?;
                // Couche active par défaut : composer tout de suite pour qu'elle
                // apparaisse en jeu (§4.4). Best-effort, comme auto_activate.
                let _ = crate::compose::recompose(conn, cfg, &id_interne);
                return Ok(ImportedMod {
                    id_interne,
                    kind: kind_str,
                    display_name: Some(name),
                    outcome: "EXTENSION".into(),
                    version_label: ui.version,
                    conflict: None,
                    added_count: diff.map(|d| d.added),
                    overwritten_count: diff.map(|d| d.overwritten),
                    existing_total: diff.map(|d| d.existing_total),
                    resources_extracted,
                });
            }
            ImportClass::Ambiguous => {
                if block_ambiguous {
                    // Rien écrit : on attend le choix de l'utilisateur (§4.4).
                    return Ok(ImportedMod {
                        id_interne,
                        kind: kind_str,
                        display_name: Some(name),
                        outcome: "AMBIGUOUS".into(),
                        version_label: ui.version,
                        conflict: None,
                        added_count: diff.map(|d| d.added),
                        overwritten_count: diff.map(|d| d.overwritten),
                        existing_total: diff.map(|d| d.existing_total),
                        resources_extracted: 0,
                    });
                }
                // Import en masse : défaut sûr = extension (jamais destructif).
                let name_layer = layer_name(fm, archive_name);
                let (_, resources_extracted) = layers::store_layer(
                    conn, library, &id_interne, fm.kind, &name_layer, &fm.dir, copy,
                    diff.as_ref().unwrap_or(&crate::identity::DiffStats { added: 0, overwritten: 0, existing_total: 0 }),
                    archive_name,
                    res_mode,
                )?;
                let _ = crate::compose::recompose(conn, cfg, &id_interne);
                return Ok(ImportedMod {
                    id_interne,
                    kind: kind_str,
                    display_name: Some(name),
                    outcome: "EXTENSION".into(),
                    version_label: ui.version,
                    conflict: None,
                    added_count: diff.map(|d| d.added),
                    overwritten_count: diff.map(|d| d.overwritten),
                    existing_total: diff.map(|d| d.existing_total),
                    resources_extracted,
                });
            }
        }
    }

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
    // Fichiers annexes (§4.6) redirigés vers le dossier ressources du mod,
    // jamais dans le contenu de jeu, selon le réglage global.
    let resources_dest = crate::resources::resources_dir(library, fm.kind, &id_interne);
    let resources_extracted = crate::resources::file_mod(&fm.dir, &dest, &resources_dest, res_mode, !copy, true)?;
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
        published_at.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // Archive/dossier source conservé (§10/§11), s'il y en a un pour cet import.
    if let Some(kept) = kept_archive {
        crate::overlay::set_kept_archive(conn, &version_id, kept).map_err(|e| e.to_string())?;
    }

    // Taille sur disque (§9.4) : calculée maintenant, le dossier final venant
    // d'être créé par la copie/le déplacement ci-dessus.
    let size_bytes = inspect::dir_size_bytes(&dest) as i64;
    crate::overlay::update_version_size(conn, &version_id, size_bytes).map_err(|e| e.to_string())?;

    crate::overlay::set_active_version(conn, &id_interne, &version_id).map_err(|e| e.to_string())?;

    // Harmonisation des tags + extraction specs/pays (§5.4), stockée en overlay.
    let class = ui.class.clone().unwrap_or_default();
    let h = harmonize::compute(rules, fm.kind, &ui.tags, &name, &class, ui.country.as_deref());
    harmonize::store(conn, &id_interne, &h, ui.country.as_deref()).map_err(|e| e.to_string())?;

    let outcome = if is_update { "UPDATE_REPLACE" } else { "IMPORT" };
    // Détails structurés (§ i18n) : rendus localisés côté front via `history.<key>`.
    let details = match (&is_update, &ui.version) {
        (true, Some(v)) => serde_json::json!({ "key": "updated", "version": v }).to_string(),
        (true, None) => serde_json::json!({ "key": "updatedNoVersion" }).to_string(),
        (false, _) => serde_json::json!({ "key": "imported", "archive": archive_name }).to_string(),
    };
    crate::overlay::add_history(conn, &id_interne, &now, outcome, &details).map_err(|e| e.to_string())?;

    Ok(ImportedMod {
        id_interne,
        kind: kind_str,
        display_name: Some(name),
        outcome: outcome.to_string(),
        version_label: ui.version,
        conflict,
        added_count: None,
        overwritten_count: None,
        existing_total: None,
        resources_extracted,
    })
}

/// Nom de couche lisible : id du dossier entrant s'il diffère de l'archive,
/// sinon nom de l'archive (assaini). Évite d'écraser une couche existante.
fn layer_name(fm: &modscan::FoundMod, archive_name: &str) -> String {
    let dir = fm.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let base = if dir.is_empty() { archive_name.to_string() } else { dir };
    sanitize(&base)
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
pub(crate) fn unique_dir(base: &Path) -> PathBuf {
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

    /// Voiture synthétique <root>/<id> avec `ui/ui_car.json` + les fichiers
    /// relatifs listés (contenu = leur nom, pour varier les tailles/signatures).
    fn make_car_with_files(root: &Path, id: &str, files: &[&str]) {
        let base = root.join(id);
        std::fs::create_dir_all(base.join("ui")).unwrap();
        std::fs::write(base.join("ui").join("ui_car.json"), br#"{"name":"Spa Test","brand":"B","tags":[]}"#).unwrap();
        for f in files {
            let p = base.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, f.as_bytes()).unwrap();
        }
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
    fn ancillary_files_extracted_to_resources_by_default() {
        // §4.6 : réglage par défaut "info_only" — fichiers légers extraits vers
        // le dossier ressources du mod, jamais dans le contenu de jeu.
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("src");
        make_car_with_files(
            &src,
            "annex_car",
            &["model.kn5", "changelog.txt", "presentation.pdf", "livery_template.psd"],
        );
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "IMPORT");
        assert_eq!(r.mods[0].resources_extracted, 2, "changelog.txt + presentation.pdf, pas le .psd");

        let versions = crate::overlay::get_versions(&conn, "annex_car").unwrap();
        let content_dir = Path::new(&versions[0].library_path);
        assert!(content_dir.join("model.kn5").is_file());
        assert!(!content_dir.join("changelog.txt").exists(), "jamais dans le contenu de jeu");
        assert!(!content_dir.join("livery_template.psd").exists());

        let resources = library.join("resources").join("cars").join("annex_car");
        assert!(resources.join("changelog.txt").is_file());
        assert!(resources.join("presentation.pdf").is_file());
        assert!(!resources.join("livery_template.psd").exists(), "mode par défaut : pas les fichiers lourds");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ancillary_extraction_mode_none_drops_files() {
        // §4.6 : mode "Aucun" — rien n'est extrait vers la bibliothèque, mais le
        // contenu de jeu reste propre quand même (règle absolue, indépendante du réglage).
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        cfg.prefs.resource_extraction_mode = "none".into();
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("src");
        make_car_with_files(&src, "annex_car2", &["model.kn5", "changelog.txt"]);
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].resources_extracted, 0);

        let versions = crate::overlay::get_versions(&conn, "annex_car2").unwrap();
        let content_dir = Path::new(&versions[0].library_path);
        assert!(content_dir.join("model.kn5").is_file());
        assert!(!content_dir.join("changelog.txt").exists(), "toujours hors contenu de jeu");
        assert!(!library.join("resources").join("cars").join("annex_car2").exists(), "rien extrait en mode Aucun");
        // Copie (pas déplacement) : la source garde son annexe intacte.
        assert!(src.join("annex_car2").join("changelog.txt").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn extension_detected_not_replaced() {
        // Bug Spa (§4.4) : un import qui ajoute surtout des chemins nouveaux et
        // n'écrase que peu de fichiers est une COUCHE, pas une mise à jour — la
        // base ne doit jamais être remplacée/perdue.
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        // Base : 10 fichiers (dont model.kn5 pour la signature).
        let src1 = base.join("src1");
        make_car_with_files(
            &src1,
            "spa",
            &["model.kn5", "data/a.ini", "data/b.ini", "data/c.ini", "data/d.ini", "data/e.ini", "data/f.ini", "data/g.ini", "data/h.ini"],
        );
        let r1 = import_one_folder(&noop, &conn, &cfg, &rules, &src1, true, &[]);
        assert_eq!(r1.mods[0].outcome, "IMPORT");
        let base_version = crate::overlay::get_versions(&conn, "spa").unwrap();
        assert_eq!(base_version.len(), 1);
        let base_path = base_version[0].library_path.clone();

        // Extension : réécrit seulement ui_car.json (1/10) et ajoute 3 chemins
        // nouveaux (dont 2 .kn5 → signature différente, pas un doublon).
        let src2 = base.join("src2");
        make_car_with_files(&src2, "spa", &["new/layout1.kn5", "new/layout2.kn5", "new/extra.ini"]);
        let r2 = import_one_folder(&noop, &conn, &cfg, &rules, &src2, true, &[]);
        assert_eq!(r2.mods[0].outcome, "EXTENSION", "couche, pas mise à jour");
        assert_eq!(r2.mods[0].overwritten_count, Some(1));
        assert_eq!(r2.mods[0].added_count, Some(3));

        // La base est intacte : toujours une seule version, dossier + model.kn5 présents.
        assert_eq!(crate::overlay::get_versions(&conn, "spa").unwrap().len(), 1, "aucune version ajoutée");
        assert!(Path::new(&base_path).join("model.kn5").is_file(), "contenu de base préservé");
        // La couche est rangée à part.
        let layers = crate::overlay::list_layers(&conn, "spa").unwrap();
        assert_eq!(layers.len(), 1);
        assert!(Path::new(&layers[0].library_path).join("new").join("layout1.kn5").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stock_never_replaced() {
        // Règle absolue (§4.4) : un contenu de base Kunos ne reçoit jamais de
        // remplacement — tout import dessus est une couche, quelle que soit la
        // proportion de fichiers écrasés.
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        // Contenu de base indexé (is_stock=1), sans version bibliothèque.
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_stock_mod(&conn, "ks_spa", "Car", Some("Kunos"), Some("Spa"), &now).unwrap();

        // Import « version complète améliorée » par-dessus : recouvrement total.
        let src = base.join("src");
        make_car_with_files(&src, "ks_spa", &["model.kn5", "data/surfaces.ini"]);
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "EXTENSION", "le stock ne peut jamais être remplacé");
        assert_eq!(crate::overlay::get_versions(&conn, "ks_spa").unwrap().len(), 0, "aucune version : base intacte");
        assert_eq!(crate::overlay::list_layers(&conn, "ks_spa").unwrap().len(), 1);
        assert!(crate::overlay::get_mod(&conn, "ks_spa").unwrap().unwrap().is_stock, "reste contenu de base");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ambiguous_blocks_then_resolves() {
        // Cas ambigu (§4.4) : import unitaire → rien écrit, on attend le choix ;
        // reprise avec la décision "update" → vraie mise à jour.
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        // Base : 5 fichiers.
        let src1 = base.join("src1");
        make_car_with_files(&src1, "amb", &["model.kn5", "data/a.ini", "data/b.ini", "data/c.ini"]);
        import_one_folder(&noop, &conn, &cfg, &rules, &src1, true, &[]);

        // Entrant : écrase 2/5 (ui_car.json + data/a.ini), signature changée
        // (model2.kn5), 1 chemin nouveau → coverage 0.4 = bande ambiguë.
        let src2 = base.join("src2");
        make_car_with_files(&src2, "amb", &["model2.kn5", "data/a.ini", "new/y.ini"]);
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src2, true, &[]);
        assert_eq!(r.mods[0].outcome, "AMBIGUOUS");
        assert_eq!(r.mods[0].overwritten_count, Some(2));
        assert_eq!(r.mods[0].existing_total, Some(5));
        assert_eq!(crate::overlay::get_versions(&conn, "amb").unwrap().len(), 1, "rien écrit tant qu'on n'a pas décidé");
        assert_eq!(crate::overlay::list_layers(&conn, "amb").unwrap().len(), 0);

        // Reprise avec la décision "update".
        let decisions = vec![ImportDecision { id: "amb".into(), decision: "update".into() }];
        let r2 = import_one_folder(&noop, &conn, &cfg, &rules, &src2, true, &decisions);
        assert_eq!(r2.mods[0].outcome, "UPDATE_REPLACE");
        assert_eq!(crate::overlay::get_versions(&conn, "amb").unwrap().len(), 2, "nouvelle version après décision");

        let _ = std::fs::remove_dir_all(&base);
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
        let res = run_import(&noop, &conn, &cfg, &rules, &[zip_str.clone()], &[]);
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

        // --- 2e import de la MÊME archive : DOUBLON (pas de réimport) ---
        let res_dup = run_import(&noop, &conn, &cfg, &rules, &[zip_str.clone()], &[]);
        assert_eq!(res_dup[0].mods[0].outcome, "DUPLICATE");
        assert_eq!(
            crate::overlay::list_mods(&conn).unwrap()[0].version_count,
            1,
            "réimport à l'identique → toujours une seule version"
        );

        // --- 3e import avec contenu modifié : MISE À JOUR (nouvelle version) ---
        std::fs::write(src.join("test_car").join("model.kn5"), b"DIFFERENT_KN5_CONTENT_XXL").unwrap();
        let zip2 = base.join("test_car_v2.zip");
        zip_dir(&sevenzip, &src, &zip2);
        let res2 = run_import(&noop, &conn, &cfg, &rules, &[zip2.to_string_lossy().into_owned()], &[]);
        assert_eq!(res2[0].mods[0].outcome, "UPDATE_REPLACE");
        let mods2 = crate::overlay::list_mods(&conn).unwrap();
        assert_eq!(mods2.len(), 1, "toujours un seul mod logique");
        assert_eq!(mods2[0].version_count, 2, "deux versions coexistent");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn unrecognized_folder_becomes_other_mod() {
        // Type non reconnu (ni voiture, circuit, skin, son, app) : jamais
        // perdu, rangé comme « autre mod » et activé par défaut (§6.1bis).
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("extension").join("config")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("MyShaderPack");
        let leaf = src.join("extension").join("config").join("new_thing");
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join("settings.ini"), b"x").unwrap();

        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert!(r.error.is_none(), "erreur inattendue: {:?}", r.error);
        assert_eq!(r.mods.len(), 0);
        assert_eq!(r.others.len(), 1);
        assert_eq!(r.others[0].id, "MyShaderPack");
        assert!(library.join("others").join("MyShaderPack").exists());
        assert!(
            crate::activation::is_junction(&ac.join("extension").join("config").join("new_thing")),
            "activé par défaut comme les autres types (§4.6bis)"
        );

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
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src_copy, true, &[]);
        assert_eq!(r.mods.len(), 1);
        assert_eq!(r.mods[0].outcome, "IMPORT");
        assert!(src_copy.join("copy_car").join("ui").join("ui_car.json").is_file(), "source préservée (copie)");
        assert!(library.join("cars").join("copy_car").exists(), "rangé dans la bibliothèque");

        // Déplacement : la source est retirée.
        let src_move = base.join("src_move");
        make_fake_car(&src_move, "move_car");
        let r2 = import_one_folder(&noop, &conn, &cfg, &rules, &src_move, false, &[]);
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
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &pack, true, &[]);
        assert_eq!(r.mods.len(), 2);
        // Les deux mods partagent la même source de pack (§4.7).
        for id in ["ferrari_a", "ferrari_b"] {
            let m = crate::overlay::get_mod(&conn, id).unwrap().unwrap();
            assert_eq!(m.source_pack.as_deref(), Some("ferrari_pack"), "{id} doit pointer le pack");
        }

        // Un import mono-voiture ne crée pas de pack.
        let solo = base.join("solo");
        make_fake_car(&solo, "solo_car");
        import_one_folder(&noop, &conn, &cfg, &rules, &solo, true, &[]);
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
    fn published_at_estimated_on_import() {
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("src_pub");
        make_fake_car(&src, "pub_car");
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods.len(), 1);

        // Date de publication estimée (§6.2) depuis la date de modification des
        // fichiers, remontée à la fois sur la version et sur le mod (version active).
        let versions = crate::overlay::get_versions(&conn, "pub_car").unwrap();
        assert!(versions[0].published_at.is_some(), "date de publication absente sur la version");
        let m = crate::overlay::get_mod(&conn, "pub_car").unwrap().unwrap();
        assert!(m.published_at.is_some(), "date de publication absente sur le mod (version active)");

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
        let r1 = run_import(&noop, &conn, &cfg, &rules, &[zip_a], &[]);
        assert!(r1[0].mods[0].conflict.is_none());

        // 2e : conflit flou vers car_a.
        let r2 = run_import(&noop, &conn, &cfg, &rules, &[zip_b], &[]);
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

    #[test]
    fn resolve_conflict_replace_removes_hardlink_deployment_of_old_mod() {
        // §2 : si l'ancien mod (fuzzy conflict) était actif par hardlinks, la
        // résolution "replace" doit retirer son déploiement dans content/, pas
        // seulement ses fichiers de bibliothèque (sinon copie orpheline vivante).
        let base = make_temp_dir().unwrap();
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        let now = chrono::Local::now().to_rfc3339();

        let old_dir = library.join("cars").join("car_a").join("v1");
        std::fs::create_dir_all(&old_dir).unwrap();
        std::fs::write(old_dir.join("f.txt"), "old").unwrap();
        crate::overlay::upsert_mod(&conn, "car_a", "Car", Some("B"), Some("Same"), "h", None, &now).unwrap();
        crate::overlay::insert_version(
            &conn, "v1", "car_a", Some("1.0"), None, &now, &old_dir.to_string_lossy(), None, "sig",
            &[], &[], &[], &[], None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "car_a", "v1").unwrap();
        crate::activation::activate(&conn, &cfg, "car_a", None).unwrap();
        let link = ac.join("content").join("cars").join("car_a");
        assert!(crate::deploy::is_deployed(&link), "précondition : car_a actif par hardlinks");

        // "car_b" existe juste en overlay (le conflit ne dépend que de l'id passé).
        crate::overlay::upsert_mod(&conn, "car_b", "Car", Some("B"), Some("Same"), "h2", None, &now).unwrap();

        resolve_conflict(&conn, &cfg, "car_b", "car_a", "replace").unwrap();

        assert!(!link.exists(), "déploiement content/ de l'ancien mod retiré, pas laissé orphelin");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn keep_source_archive_pref_off_leaves_kept_path_empty() {
        // §10/§11 : réglage désactivé par défaut — aucune copie, aucun chemin
        // enregistré (comportement historique, pas d'espace disque en plus).
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("src");
        make_fake_car(&src, "nokeep_car");
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "IMPORT");

        let versions = crate::overlay::get_versions(&conn, "nokeep_car").unwrap();
        assert!(versions[0].kept_archive_path.is_none());
        assert!(!library.join("_source_archives").exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn keep_source_archive_pref_on_copies_folder_and_records_path() {
        // §10/§11 : réglage activé — le dossier source est copié à part dans la
        // bibliothèque et son chemin enregistré sur la version, pour permettre
        // plus tard « Réinstaller depuis l'archive source » (maintenance.rs).
        let base = make_temp_dir().unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mut cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        cfg.prefs.keep_source_archive = true;
        let rules = crate::rules::default_rules();
        let noop = |_p: Progress| {};

        let src = base.join("src");
        make_fake_car(&src, "keep_car");
        let r = import_one_folder(&noop, &conn, &cfg, &rules, &src, true, &[]);
        assert_eq!(r.mods[0].outcome, "IMPORT");

        let versions = crate::overlay::get_versions(&conn, "keep_car").unwrap();
        let kept = versions[0].kept_archive_path.as_ref().expect("archive source conservée");
        let kept_path = Path::new(kept);
        // La copie porte sur `dir` (le dossier passé à l'import, ici `src`, qui
        // peut contenir plusieurs mods) — le mod retrouvé est donc un niveau
        // plus bas, comme au premier import (même logique de descente).
        assert!(kept_path.is_dir(), "dossier source copié tel quel");
        assert!(kept_path.join("keep_car").join("ui").join("ui_car.json").is_file());
        assert!(kept_path.starts_with(&library), "copie rangée dans la bibliothèque, pas ailleurs");
        // Source d'origine toujours présente : `copy=true` préserve la source.
        assert!(src.join("keep_car").join("model.kn5").is_file(), "source d'origine intacte (copie)");

        let _ = std::fs::remove_dir_all(&base);
    }
}
