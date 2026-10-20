//! The bulk import (§4.2): a parent folder whose direct subfolders are each
//! imported as the import of that folder alone would — an analysis that
//! writes nothing, the user's decisions, then the execution, one subfolder at
//! a time under its own lock. Not to be confused with `crate::bulk`, which
//! edits mods already in the library.
//!
//! Out of `importer.rs`, a 6 000-line file where it was a subject of its own
//! and the part that grew the most. The comments are those of the original
//! code, moved with it.

use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::filing::process_found;
use super::{
    auto_activate, check_disk_space, drop_unused_kept_source, failed_result, file_tail, filing_ratio, keep_source,
    mod_progress, rescue_nested_archives, resolve_conflict, size_batch, source_label, ArchiveResult, ArchiveSource,
    SCAN_SHARE,
};
use crate::config::AppConfig;
use crate::import_bench::{Bucket, ITEM_OVERHEAD_SECS};
use crate::import_progress::{ImportCtx, ItemPlan, PHASE_FILING};
use crate::modscan::{self, ModKind};
use crate::rules::Rules;
use crate::{identity, uijson};

#[derive(Debug, Clone, Serialize)]
pub struct BulkMod {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    /// "new" | "update" | "duplicate" | "ambiguous" | "rehydrate" (ESPACE§7.3)
    pub status: String,
    pub existing_id: Option<String>,
    pub existing_name: Option<String>,
}

/// A skin pack, a sound or an app found in a subfolder (§8.3, §8.4): filed by
/// the execution like the import of that folder alone would. No status — the
/// duplicate and update questions are those of cars and tracks.
#[derive(Debug, Clone, Serialize)]
pub struct BulkAddon {
    /// "skin" | "sound" | "app"
    pub kind: &'static str,
    /// The car or track it attaches to; the app's own name for an app.
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkEntry {
    pub subfolder: String,
    pub path: String,
    /// Sous-dossier sans structure AC reconnaissable.
    pub ignored: bool,
    pub mods: Vec<BulkMod>,
    pub addons: Vec<BulkAddon>,
}

fn bulk_addons(conn: &Connection, dir: &Path, source_name: &str) -> Vec<BulkAddon> {
    let subs = modscan::scan_subs(dir).into_iter().map(|s| BulkAddon {
        kind: match s.kind {
            modscan::SubKind::Skin => "skin",
            modscan::SubKind::Sound => "sound",
        },
        target: crate::submods::target_of(conn, &s, source_name),
    });
    let apps = modscan::scan_apps(dir).into_iter().map(|a| BulkAddon {
        kind: "app",
        target: a.name,
    });
    subs.chain(apps).collect()
}

/// Classe un mod trouvé sans rien écrire (§4.2 phase d'analyse).
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
        let skeleton = crate::overlay::version_by_signature(conn, id, &sig)
            .map_err(|e| e.to_string())?
            .is_some_and(|v| v.is_skeleton());
        if skeleton {
            // Same classification as the import itself (ESPACE§7.3).
            Ok(("rehydrate".into(), None, None))
        } else if existing.as_deref() == Some(sig.as_str()) {
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

/// Ce dossier contient-il ne serait-ce qu'un fichier, à quelque profondeur
/// que ce soit ? Distingue un sous-dossier réellement vide (rien à importer,
/// « ignoré » à bon droit) d'un sous-dossier dont le contenu n'a simplement
/// pas la structure d'une voiture ou d'un circuit — celui-ci finira en
/// « autre mod » (§7.3) plutôt que d'être écarté avant même l'exécution.
fn dir_has_any_file(dir: &Path) -> bool {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .any(|entry| entry.file_type().is_file())
}

/// Phase d'analyse (§4.2) : scanne chaque sous-dossier direct du parent et
/// classe les mods **sans rien écrire**. Un seul niveau de sous-dossiers.
pub fn analyze_bulk(conn: &Connection, _cfg: &AppConfig, parent: &Path) -> Result<Vec<BulkEntry>, String> {
    if !parent.is_dir() {
        return Err(crate::errors::NOT_A_DIRECTORY.into());
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
        let subfolder = sub
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let path = sub.to_string_lossy().into_owned();
        let found = modscan::scan(&sub);
        // « Ignoré » veut dire « rien à importer », pas seulement « pas de
        // voiture ni de circuit ». Un sous-dossier sans structure reconnue
        // mais non vide (mannequin de pilote nu, notice, images d'un mod
        // « autre ») est traité en exécution comme `import_folder` le ferait
        // seul (§7.3) — encore fallait-il ne pas l'écarter ici avant même
        // d'y arriver. Bug réel : un dossier ne contenant qu'un
        // `jp_police_man.kn5` (mannequin de pilote sans structure de voiture)
        // s'affichait « ignoré » et n'était jamais envoyé à l'exécution.
        if found.is_empty() && !dir_has_any_file(&sub) {
            entries.push(BulkEntry {
                subfolder,
                path,
                ignored: true,
                mods: Vec::new(),
                addons: Vec::new(),
            });
            continue;
        }
        let mut mods = Vec::new();
        for fm in &found {
            let id = fm
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ui = match fm.kind {
                ModKind::Car => uijson::read_car(&fm.dir),
                ModKind::Track => uijson::read_track(&fm.dir),
            }
            .unwrap_or_default();
            let kind_str = format!("{:?}", fm.kind);
            let brand = ui.brand.clone().unwrap_or_default();
            let name = ui.name.clone().unwrap_or_else(|| id.clone());
            let (status, existing_id, existing_name) = classify(conn, &id, &kind_str, &brand, &name, &fm.dir)?;
            mods.push(BulkMod {
                id,
                kind: kind_str,
                name: ui.name,
                status,
                existing_id,
                existing_name,
            });
        }
        let addons = bulk_addons(conn, &sub, &subfolder);
        entries.push(BulkEntry {
            subfolder,
            path,
            ignored: false,
            mods,
            addons,
        });
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

/// Exécution de l'import en masse selon les décisions (§4.2). Reprenable :
/// relancer l'analyse après interruption reclasse les mods déjà importés en
/// « doublon »/« mise à jour », donc rien n'est traité de travers.
pub fn execute_bulk(
    ctx: &ImportCtx,
    db: &crate::overlay::Db,
    cfg: &AppConfig,
    rules: &Rules,
    items: &[BulkExecItem],
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    // Même protocole de progression que les autres chemins d'import (§4.2bis) :
    // c'est celui qui traite le plus de mods d'un coup, il serait le dernier à
    // devoir se contenter d'un compteur d'entrées.
    let paths: Vec<String> = items.iter().map(|it| it.path.clone()).collect();
    let sizes = size_batch(ctx, &paths);
    let bucket = if copy { Bucket::FolderCopy } else { Bucket::FolderMove };
    check_disk_space(cfg, &sizes, if copy { 1.0 } else { 0.0 })?;
    ctx.plan(
        paths
            .iter()
            .zip(&sizes)
            .map(|(p, &size)| ItemPlan {
                label: source_label(Path::new(p)),
                extract_w: 0.0,
                file_w: ITEM_OVERHEAD_SECS + ctx.estimate(bucket, size),
            })
            .collect(),
    );

    let mut results = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        if ctx.cancelled() {
            break;
        }
        let label = source_label(Path::new(&it.path));
        ctx.set_current(i, PHASE_FILING, label.clone());
        let started = Instant::now();
        // Source kept (§10/§11) outside the lock, as `import_folders` does: a
        // full copy, sometimes of several GB, during which every screen
        // reading the overlay would wait.
        let kept = keep_source(cfg, Path::new(&it.path), &label);
        // Verrou par entrée, jamais un seul pour tout le lot : un import en
        // masse ne doit pas geler les écrans qui lisent l'overlay.
        let result = match db.0.lock() {
            Ok(conn) => exec_one(ctx, &conn, cfg, rules, i, it, copy, kept.as_deref()),
            Err(e) => failed_result(&label, e.to_string()),
        };
        ctx.record(bucket, sizes[i], started.elapsed().as_secs_f64());
        results.push(result);
        ctx.finish_item(i);
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn exec_one(
    ctx: &ImportCtx,
    conn: &Connection,
    cfg: &AppConfig,
    rules: &Rules,
    index: usize,
    it: &BulkExecItem,
    copy: bool,
    // Copied by the caller, before taking the lock (§10/§11).
    kept_archive: Option<&str>,
) -> ArchiveResult {
    let dir = Path::new(&it.path);
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut result = ArchiveResult {
        archive: name.clone(),
        mods: Vec::new(),
        error: None,
        subs: Vec::new(),
        apps: Vec::new(),
        others: Vec::new(),
        extras: 0,
        pending: 0,
        reapplied: Vec::new(),
        app_layers: Vec::new(),
    };

    let Some(library) = &cfg.library_path else {
        result.error = Some(crate::errors::LIBRARY_NOT_CONFIGURED.into());
        return result;
    };
    let res_mode = crate::resources::ExtractionMode::parse(&cfg.prefs.resource_extraction_mode);

    let found = modscan::scan(dir);
    let subs = modscan::scan_subs(dir);
    let apps = modscan::scan_apps(dir);
    if found.is_empty() && subs.is_empty() && apps.is_empty() {
        // Rien de reconnu (§7.3) : même repli que l'import d'un dossier isolé
        // (`import_folder`), dont ce chemin en masse s'était affranchi. Un
        // mannequin de pilote nu (`jp_police_man.kn5`, aucune structure de
        // voiture/circuit) tombait ici en « dossier proposé » (§4.6ter, un
        // arbitrage qui attend l'utilisateur) faute d'appeler `import_other` —
        // qui, lui, sait déjà le ranger via `others::relocate_driver_models`.
        ctx.sub(0, 1, name.clone());
        ctx.file_ratio(index, SCAN_SHARE);
        let rescued = rescue_nested_archives(
            ctx,
            index,
            conn,
            cfg,
            rules,
            library,
            &name,
            dir,
            copy,
            res_mode,
            &mut result,
        );
        if !rescued {
            if let Some(other) = crate::others::import_other(conn, library, &name, dir, copy, res_mode) {
                if let Err(e) = crate::others::activate_other(conn, cfg, &other.id) {
                    log::warn!("auto_activate_other {}: {e}", other.id);
                }
                result.others.push(other);
            } else {
                result.error = Some(crate::errors::NOTHING_TO_IMPORT_IN_FOLDER.into());
            }
        }
        drop_unused_kept_source(conn, cfg, kept_archive);
        return result;
    }

    ctx.sub(0, found.len(), name.clone());
    ctx.file_ratio(index, SCAN_SHARE);
    // Pack (§4.4) : plusieurs mods issus du même dossier partagent leur source.
    let pack = (found.len() > 1).then_some(name.as_str());
    for (i, fm) in found.iter().enumerate() {
        let id = fm
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        ctx.sub(i + 1, found.len(), id.clone());
        if it.skip_ids.iter().any(|s| s == &id) {
            ctx.file_ratio(index, filing_ratio(i + 1, found.len()));
            continue;
        }
        // Import en masse (§4.2) : jamais de blocage au fil de l'eau. Un cas
        // ambigu retombe sur le défaut sûr (extension, jamais destructif).
        match process_found(
            conn,
            cfg,
            rules,
            library,
            &name,
            fm,
            copy,
            pack,
            None,
            false,
            ArchiveSource {
                kept: kept_archive,
                origin: None,
            },
            &mod_progress(ctx, index, i, found.len()),
        ) {
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
        // Fin de la part de ce mod, quoi qu'il se soit passé pendant : un
        // `rename` sur le même volume ne signale aucun octet, donc le
        // rapporteur d'octets seul laisserait la barre au début de la part.
        ctx.file_ratio(index, filing_ratio(i + 1, found.len()));
    }
    // Activation par défaut des mods importés (§4.2).
    auto_activate(conn, cfg, &result.mods);

    // Skins, sounds, apps and leftovers (§8.3, §8.4, §7.3): the same tail as a
    // folder imported alone, never asking (§4.2bis). This path once had its
    // own copy that only swept leftovers — skins, sounds and apps were marked
    // consumed and never filed, so a folder of car sounds imported nothing,
    // without an error.
    file_tail(
        ctx,
        index,
        conn,
        cfg,
        rules,
        library,
        &name,
        dir,
        &found,
        &subs,
        &apps,
        pack,
        copy,
        res_mode,
        false,
        &[],
        &mut result,
    );
    drop_unused_kept_source(conn, cfg, kept_archive);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::tests::make_fake_car;

    #[test]
    fn bulk_analyze_and_execute() {
        let base = crate::testutil::temp_dir("import");
        let parent = base.join("catalog");
        std::fs::create_dir_all(&parent).unwrap();
        make_fake_car(&parent, "bulk_car"); // sous-dossier = voiture
                                            // Sous-dossier sans structure AC mais non vide : pas ignoré, il
                                            // finira en « autre mod » à l'exécution — comme un import de
                                            // dossier isolé le ferait (§7.3). Seul un dossier réellement vide
                                            // est ignoré (voir `bulk_unrecognized_folder_is_not_lost`).
        let notes = parent.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("readme.txt"), b"hi").unwrap();
        // Sous-dossier réellement vide → ignoré.
        let empty = parent.join("empty");
        std::fs::create_dir_all(&empty).unwrap();

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        // Analyse : 1 nouveau (bulk_car), 1 non ignoré sans mods (notes),
        // 1 ignoré (empty). Aucune écriture.
        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        assert_eq!(entries.len(), 3);
        let car = entries.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert!(!car.ignored);
        assert_eq!(car.mods[0].status, "new");
        let notes_entry = entries.iter().find(|e| e.subfolder == "notes").unwrap();
        assert!(!notes_entry.ignored, "non vide : ce n'est pas rien à importer");
        assert!(notes_entry.mods.is_empty(), "rien de reconnu comme voiture/circuit");
        assert!(
            entries.iter().find(|e| e.subfolder == "empty").unwrap().ignored,
            "vide : ici, ignoré est le mot juste"
        );
        assert!(!library.join("cars").join("bulk_car").exists(), "analyse n'écrit rien");

        // Exécution (copie).
        let item = BulkExecItem {
            path: car.path.clone(),
            skip_ids: vec![],
            replace_ids: vec![],
        };
        let r = exec_one(&ImportCtx::silent(), &conn, &cfg, &rules, 0, &item, true, None);
        assert_eq!(r.mods.len(), 1);
        assert!(library.join("cars").join("bulk_car").exists());

        // Reprenable : ré-analyse → bulk_car devient un doublon (même contenu).
        let entries2 = analyze_bulk(&conn, &cfg, &parent).unwrap();
        let car2 = entries2.iter().find(|e| e.subfolder == "bulk_car").unwrap();
        assert_eq!(car2.mods[0].status, "duplicate");
    }

    /// Règle : un sous-dossier sans structure de voiture/circuit mais non vide
    /// n'est pas perdu (§7.3) — il devient un « autre mod », comme le ferait
    /// un import de dossier isolé (`import_folder`). Couvre le double bug
    /// remonté par l'utilisateur : un mannequin de pilote nu importé en masse
    /// (1) s'affichait « ignoré » en analyse et (2) même arbitré manuellement,
    /// `exec_one` n'avait aucun repli vers `import_other`.
    #[test]
    fn bulk_unrecognized_folder_is_not_lost() {
        let base = crate::testutil::temp_dir("import-bulk-other");
        let parent = base.join("catalog");
        let sub = parent.join("notes");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("readme.txt"), b"hi").unwrap();

        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        let entry = entries.iter().find(|e| e.subfolder == "notes").unwrap();
        assert!(!entry.ignored, "non ignoré en analyse (précondition du bug)");

        let item = BulkExecItem {
            path: entry.path.clone(),
            skip_ids: vec![],
            replace_ids: vec![],
        };
        let r = exec_one(&ImportCtx::silent(), &conn, &cfg, &rules, 0, &item, true, None);
        assert_eq!(r.others.len(), 1, "rangé comme autre mod, pas perdu");
        assert!(
            crate::overlay::other_exists(&conn, &r.others[0].id).unwrap(),
            "et vraiment inséré dans l'overlay"
        );
    }

    /// Rule (§4.2, §8.3): a bulk import files the sounds, skins and apps it
    /// finds, the way the import of the same folder alone does. Real bug: a
    /// parent folder of eighteen car sounds (`<name>/content/cars/<car>/sfx/`)
    /// imported nothing at all — the bulk path marked them consumed and never
    /// handed them to `submods`.
    #[test]
    fn bulk_files_a_car_sound() {
        let base = crate::testutil::temp_dir("import-bulk-sound");
        let parent = base.join("sounds");
        let sfx = parent
            .join("Sound - a3dr_viper_rt10 by AmplifiedNL")
            .join("content")
            .join("cars")
            .join("a3dr_viper_rt10")
            .join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"{guid} event:/cars/a3dr_viper_rt10/engine_ext").unwrap();
        std::fs::write(sfx.join("a3dr_viper_rt10.bank"), b"BANK").unwrap();
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        assert_eq!(entries.len(), 1);
        let addons = &entries[0].addons;
        assert_eq!(addons.len(), 1, "the analysis announces the sound");
        assert_eq!(
            (addons[0].kind, addons[0].target.as_str()),
            ("sound", "a3dr_viper_rt10")
        );
        let item = BulkExecItem {
            path: entries[0].path.clone(),
            skip_ids: vec![],
            replace_ids: vec![],
        };
        let r = exec_one(&ImportCtx::silent(), &conn, &cfg, &rules, 0, &item, true, None);
        assert_eq!(r.subs.len(), 1, "the sound is filed: {r:?}");
        assert_eq!(r.subs[0].sub_type, "SOUND");
        assert_eq!(r.subs[0].parent_id, "a3dr_viper_rt10");
        assert!(r.others.is_empty(), "not an other mod: {:?}", r.others);
    }

    /// Rule (§4.2, §7.3): a parent folder of driver bodies, each delivered as
    /// `<name>/content/driver/<name>.kn5`, is a batch of "other mods" — one per
    /// subfolder, the body kept at its game path. Real case: sixteen bodies
    /// analysed as "0 new" (the screen only counted cars and tracks).
    #[test]
    fn bulk_driver_bodies_become_one_other_mod_each() {
        let base = crate::testutil::temp_dir("import-bulk-drivers");
        let parent = base.join("drivers");
        for name in ["senna", "goku"] {
            let dir = parent.join(name).join("content").join("driver");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{name}.kn5")), b"KN5").unwrap();
        }
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let rules = crate::rules::default_rules();

        let entries = analyze_bulk(&conn, &cfg, &parent).unwrap();
        assert_eq!(entries.len(), 2, "one entry per subfolder");
        for e in &entries {
            assert!(
                !e.ignored && e.mods.is_empty(),
                "{}: other content, not ignored",
                e.subfolder
            );
            let item = BulkExecItem {
                path: e.path.clone(),
                skip_ids: vec![],
                replace_ids: vec![],
            };
            let r = exec_one(&ImportCtx::silent(), &conn, &cfg, &rules, 0, &item, true, None);
            assert_eq!(r.others.len(), 1, "{}: filed as one other mod", e.subfolder);
            let body = library
                .join("others")
                .join(&r.others[0].id)
                .join("content")
                .join("driver")
                .join(format!("{}.kn5", e.subfolder));
            assert!(body.is_file(), "body kept at its game path: {}", body.display());
        }
    }

    /// Rule (§10/§11): a bulk import keeps each subfolder's source like the
    /// import of that folder alone — copied once, recorded on the version.
    #[test]
    fn bulk_import_keeps_each_subfolder_source() {
        let base = crate::testutil::temp_dir("import-bulk-keep");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let db_path = base.join("overlay.sqlite");
        let db = crate::overlay::Db(std::sync::Mutex::new(crate::overlay::open(&db_path).unwrap()));
        let mut cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        cfg.prefs.keep_source_archive = true;
        let rules = crate::rules::default_rules();
        let parent = base.join("catalog");
        make_fake_car(&parent.join("sub_a"), "bulk_keep_car");

        let items = [BulkExecItem {
            path: parent.join("sub_a").to_string_lossy().into_owned(),
            skip_ids: vec![],
            replace_ids: vec![],
        }];
        let r = execute_bulk(&ImportCtx::silent(), &db, &cfg, &rules, &items, true).unwrap();
        assert_eq!(r[0].mods.len(), 1, "car imported");

        let conn = db.0.lock().unwrap();
        let versions = crate::overlay::get_versions(&conn, "bulk_keep_car").unwrap();
        let kept = versions[0].kept_archive_path.as_ref().expect("source kept");
        let kept_path = library.join(kept);
        assert!(
            kept_path.join("bulk_keep_car").join("ui").join("ui_car.json").is_file(),
            "the subfolder is copied whole: {}",
            kept_path.display()
        );
        let copies = std::fs::read_dir(library.join("_source_archives")).unwrap().count();
        assert_eq!(copies, 1, "one copy per subfolder, no stray one");
    }
}
