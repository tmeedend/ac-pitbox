//! Édition groupée (§6.3bis) : actions appliquées à une sélection multiple de
//! mods, limitées aux champs communs à tout mod (tags manuels, favori,
//! catégorie, activation, suppression, export). Ne touche jamais aux champs
//! propres à un type précis (specs voiture, skin piloté, version active) —
//! ceux-ci restent réservés à la fiche détail d'un seul mod.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::{activation, export, maintenance, overlay};

/// Progression d'un lot. Émise sous `bulk:progress` par la façade — miroir de
/// `BulkProgress` dans `src/lib/bulkEdit.ts`, les deux se changent ensemble.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    /// Rang 1-based de l'élément en cours de traitement.
    pub index: usize,
    pub total: usize,
    /// Ce qu'on est en train de faire : `activate`, `deactivate`, `delete`,
    /// `export`. Le libellé affiché est choisi côté front, à partir de ça.
    pub op: String,
    /// Id du mod en cours — le seul repère qui parle à l'utilisateur pendant
    /// qu'un lot de quarante défile.
    pub id: String,
}

/// Délai minimal entre deux émissions. Deux mille tags écrits en SQLite se
/// bouclent en quelques dizaines de millisecondes : sans plancher, le lot
/// noierait l'IPC sous des événements que personne ne peut lire — exactement
/// la réactivité que la progression est censée protéger (même raison
/// qu'`import_progress::EMIT_INTERVAL_MS`).
const EMIT_INTERVAL_MS: u128 = 80;

/// Où part la progression. Une fonction, pas un `AppHandle` : **ce module ne
/// doit pas connaître Tauri**.
///
/// Ce n'est pas qu'une question de propreté, c'est une contrainte mesurée :
/// importer `tauri::{AppHandle, Emitter}` ici suffit à rendre le binaire de
/// test de la lib inexécutable — il ne démarre plus du tout
/// (`STATUS_ENTRYPOINT_NOT_FOUND`, 0xc0000139, avant le premier test), alors
/// que le même import dans `commands/` ou dans `import_progress.rs` ne pose
/// rien. Constaté par bissection : 253 tests passent sans cet import, zéro
/// avec. L'émission vit donc dans la façade (`commands/bulk_ops.rs`), qui
/// passe la fermeture ci-dessous.
pub type ProgressSink<'a> = &'a dyn Fn(Progress);

/// Contexte d'un lot : à qui annoncer la progression, et comment savoir qu'on
/// a demandé l'arrêt. `silent()` pour les tests, qui n'ont ni destinataire ni
/// bouton d'annulation.
pub struct BulkCtx<'a> {
    sink: Option<ProgressSink<'a>>,
    op: &'static str,
    cancel: Option<Arc<AtomicBool>>,
    last_emit: Mutex<Option<Instant>>,
}

impl<'a> BulkCtx<'a> {
    pub fn new(sink: ProgressSink<'a>, op: &'static str, cancel: Arc<AtomicBool>) -> Self {
        Self {
            sink: Some(sink),
            op,
            cancel: Some(cancel),
            last_emit: Mutex::new(None),
        }
    }

    /// Sans émission ni annulation : les tests n'ont ni `AppHandle` ni bouton
    /// d'arrêt. `cfg(test)` parce que c'est bien son seul usage — le laisser
    /// visible en production offrirait un lot muet, qu'on finirait par
    /// appeler par mégarde.
    #[cfg(test)]
    pub fn silent() -> Self {
        Self {
            sink: None,
            op: "",
            cancel: None,
            last_emit: Mutex::new(None),
        }
    }

    /// Arrêt déjà demandé, pour éprouver ce que « annulé » veut dire.
    #[cfg(test)]
    pub fn stopped() -> Self {
        Self {
            sink: None,
            op: "",
            cancel: Some(Arc::new(AtomicBool::new(true))),
            last_emit: Mutex::new(None),
        }
    }

    /// Vrai dès que l'utilisateur a demandé l'arrêt. Constaté **entre deux
    /// mods**, jamais au milieu de l'un d'eux : interrompre une activation en
    /// cours laisserait des junctions à moitié posées (§9.3).
    fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed))
    }

    /// Annonce l'élément qui commence. Le premier et le dernier passent
    /// toujours : sans le premier, un lot court n'afficherait jamais rien.
    fn tick(&self, index: usize, total: usize, id: &str) {
        let Some(sink) = self.sink else { return };
        let now = Instant::now();
        let mut last = self.last_emit.lock().unwrap_or_else(|e| e.into_inner());
        let due =
            index == 1 || index == total || last.is_none_or(|t| now.duration_since(t).as_millis() >= EMIT_INTERVAL_MS);
        if !due {
            return;
        }
        *last = Some(now);
        drop(last);
        sink(Progress {
            index,
            total,
            op: self.op.to_string(),
            id: id.to_string(),
        });
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BulkFailure {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BulkReport {
    pub ok: Vec<String>,
    pub failed: Vec<BulkFailure>,
    /// Lot interrompu : ce qui reste après le dernier `ok`/`failed` n'a pas
    /// été traité du tout. Un rapport qui ne le dirait pas se lirait comme un
    /// lot complet dont la moitié aurait échoué en silence.
    pub cancelled: bool,
}

impl BulkReport {
    fn push(&mut self, id: &str, result: Result<(), String>) {
        match result {
            Ok(()) => self.ok.push(id.to_string()),
            Err(error) => self.failed.push(BulkFailure {
                id: id.to_string(),
                error,
            }),
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
        let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else {
            continue;
        };
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
        let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else {
            continue;
        };
        if m.tags_manual.contains(&tag) {
            let tags: Vec<String> = m.tags_manual.into_iter().filter(|t| t != &tag).collect();
            overlay::set_manual_tags(conn, id, &tags).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Boucle commune aux trois lots qui touchent au disque : progression émise
/// avant chaque mod, arrêt constaté entre deux, rapport par id. Écrite une
/// fois plutôt que trois — c'est ici que se décide ce qu'« annulé » veut dire,
/// et trois copies auraient fini par ne plus le dire pareil.
fn run_each(ctx: &BulkCtx, ids: &[String], mut step: impl FnMut(&str) -> Result<(), String>) -> BulkReport {
    let mut report = BulkReport::default();
    for (i, id) in ids.iter().enumerate() {
        if ctx.cancelled() {
            report.cancelled = true;
            break;
        }
        ctx.tick(i + 1, ids.len(), id);
        report.push(id, step(id));
    }
    report
}

pub fn activate(ctx: &BulkCtx, conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    run_each(ctx, ids, |id| activation::activate(conn, cfg, id, None))
}

pub fn deactivate(ctx: &BulkCtx, conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    run_each(ctx, ids, |id| activation::deactivate(conn, cfg, id))
}

pub fn delete(ctx: &BulkCtx, conn: &Connection, cfg: &AppConfig, ids: &[String]) -> BulkReport {
    run_each(ctx, ids, |id| maintenance::delete_broken(conn, cfg, id))
}

pub fn export(
    ctx: &BulkCtx,
    conn: &Connection,
    cfg: &AppConfig,
    ids: &[String],
    dest_dir: &Path,
) -> Vec<BulkExportItem> {
    let mut out = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        if ctx.cancelled() {
            break;
        }
        ctx.tick(i + 1, ids.len(), id);
        out.push(match export::export_mod(conn, cfg, id, dest_dir) {
            Ok(report) => BulkExportItem {
                id: id.clone(),
                report: Some(report),
                error: None,
            },
            Err(error) => BulkExportItem {
                id: id.clone(),
                report: None,
                error: Some(error),
            },
        });
    }
    out
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
        let base = crate::testutil::temp_dir("bulk");
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
        assert_eq!(
            overlay::get_mod(&conn, "modA").unwrap().unwrap().category.as_deref(),
            Some("#gt")
        );
        assert_eq!(
            overlay::get_mod(&conn, "modB").unwrap().unwrap().category.as_deref(),
            Some("#gt")
        );
    }

    /// Règle : un lot arrêté le DIT, et ce qui n'a pas été traité n'apparaît
    /// ni en succès ni en échec — un rapport muet se lirait comme un lot
    /// complet dont tout aurait mystérieusement disparu (§6.3bis).
    #[test]
    fn cancelled_bulk_processes_nothing_and_says_so() {
        let base = crate::testutil::temp_dir("bulkcancel");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        seed_mod(&conn, "modA", &[]);
        let cfg = AppConfig::default();
        let ids = vec!["modA".to_string()];

        let report = activate(&BulkCtx::stopped(), &conn, &cfg, &ids);
        assert!(report.cancelled, "report says the batch was stopped");
        assert!(
            report.ok.is_empty() && report.failed.is_empty(),
            "nothing was attempted"
        );
    }

    #[test]
    fn bulk_activate_deactivate_delete_reports_per_id() {
        if !cfg!(windows) {
            return;
        }
        let base = crate::testutil::temp_dir("bulkact");
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
            &conn,
            "v1",
            "good",
            Some("1.0"),
            None,
            &now,
            &good_dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "good", "v1").unwrap();

        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(library),
            ..Default::default()
        };
        let ids = vec!["good".to_string(), "ghost".to_string()];

        let ctx = BulkCtx::silent();
        let report = activate(&ctx, &conn, &cfg, &ids);
        assert_eq!(report.ok, vec!["good".to_string()]);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].id, "ghost");

        // deactivate() est un no-op réussi si aucune junction n'existe déjà
        // (cas de "ghost", dont l'activation a échoué juste au-dessus).
        let report = deactivate(&ctx, &conn, &cfg, &ids);
        assert_eq!(report.ok.len(), 2);
        assert!(report.failed.is_empty());

        let report = delete(&ctx, &conn, &cfg, &ids);
        assert_eq!(
            report.ok.len(),
            2,
            "delete_broken supprime même un mod sans fichiers valides"
        );
        assert!(overlay::get_mod(&conn, "good").unwrap().is_none());
        assert!(overlay::get_mod(&conn, "ghost").unwrap().is_none());
    }
}
