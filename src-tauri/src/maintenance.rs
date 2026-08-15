//! Nettoyage (§9.3) : détection assistée des **mods cassés** (fichiers de
//! bibliothèque manquants/invalides) et des **junctions orphelines** (pointant
//! vers une version supprimée). Porté de l'esprit de `clean.py`, mais non
//! destructif sans confirmation et respectant le garde-fou junction.

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::ModRow;
use crate::{activation, archive, deploy, inspect, modscan, overlay, submods, uijson};

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

/// Skin ou son dont la voiture/le circuit parent n'existe plus (§9.3).
#[derive(Debug, Clone, Serialize)]
pub struct OrphanSub {
    pub id: String,
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceReport {
    pub broken: Vec<BrokenMod>,
    pub orphans: Vec<OrphanJunction>,
    /// Sous-éléments sans parent (§9.3). Conservés volontairement à la
    /// suppression d'un mod — réimporter le même id les retrouve — donc jamais
    /// nettoyés automatiquement : seulement listés, pour décision.
    pub orphan_subs: Vec<OrphanSub>,
}

/// Un mod est-il cassé (fichiers de sa version active manquants/invalides) ?
/// Renvoie une clé i18n (résolue côté frontend, pas du texte affichable), ou
/// `None` si tout va bien. Contenu de base (`is_stock`) jamais cassé (vrai
/// dossier du jeu, pas géré par nous). Partagé entre `scan` (§9.3, écran
/// Maintenance) et `library::to_card` (§6.4, badge sur la carte bibliothèque).
pub fn broken_reason(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    if m.is_stock {
        return None;
    }
    let kind = kind_of(&m.kind);
    let path = m.active_version_id.as_ref().and_then(|vid| {
        let stored = overlay::get_version_path(conn, vid).ok().flatten()?;
        crate::libpath::resolve(cfg.library_path.as_deref(), &stored)
    });
    match &path {
        None => Some("maintenance.reasonNoActiveVersion".to_string()),
        Some(dir) => {
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
        if let Some(reason) = broken_reason(conn, cfg, &m) {
            broken.push(BrokenMod {
                id: m.id_interne,
                kind: m.kind,
                name: m.display_name,
                reason,
            });
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

    // --- Sous-éléments sans parent : skins/sons d'un mod supprimé ---
    let orphan_subs = overlay::orphan_subs(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| OrphanSub {
            id: s.id,
            sub_type: s.sub_type,
            parent_id: s.parent_id,
            name: s.name,
        })
        .collect();

    Ok(MaintenanceReport {
        broken,
        orphans,
        orphan_subs,
    })
}

/// Efface les sous-éléments sans parent : fichiers stockés puis ligne overlay
/// (§9.3). Contourne délibérément le garde-fou `removable` de `remove_sub` —
/// il protège un skin fourni avec un mod **vivant**, ce qui n'a plus de sens
/// quand le parent a disparu. Aucune projection à retirer non plus : le
/// dossier `skins/` de la cible n'existe plus.
pub fn purge_orphan_subs(conn: &Connection, cfg: &AppConfig) -> Result<usize, String> {
    let mut n = 0;
    for sub in overlay::orphan_subs(conn).map_err(|e| e.to_string())? {
        if let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path) {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                // Dossier déjà absent : rien d'anormal, la ligne part quand même.
                log::warn!("purge_orphan_sub {} ({}): {e}", sub.id, dir.display());
            }
        }
        overlay::delete_sub_mod(conn, &sub.id).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

/// Supprime un mod cassé : fichiers de bibliothèque (toutes versions) + junction
/// éventuelle (garde-fou) + overlay.
pub fn delete_broken(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);

    // Fichiers : versions individuelles + dossier parent du mod dans la bibliothèque.
    for v in overlay::get_versions(conn, id).map_err(|e| e.to_string())? {
        if let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path) {
            let _ = std::fs::remove_dir_all(dir);
        }
        // Archive source conservée (§10/§11), si le réglage était actif à l'import :
        // `<lib>/_source_archives/<uuid>/<nom>` — on efface tout le dossier `<uuid>`.
        if let Some(kept) = v
            .kept_archive_path
            .as_deref()
            .and_then(|p| crate::libpath::resolve(cfg.library_path.as_deref(), p))
        {
            if let Some(parent) = kept.parent() {
                let _ = std::fs::remove_dir_all(parent);
            }
        }
    }
    if let Some(lib) = &cfg.library_path {
        let _ = std::fs::remove_dir_all(lib.join(kind.content_folder()).join(id));
    }

    // Ajouts au jeu (§4.6ter) : retirés d'AC *avant* de supprimer leur source en
    // bibliothèque — c'est la liste en base qui dit quoi retirer, mais autant
    // le faire tant que tout est cohérent. C'est ce qui manquait au passage par
    // « autre mod » : les fichiers d'une voiture supprimée restaient dans AC.
    if let Err(e) = crate::extras::undeploy(conn, cfg, id) {
        log::warn!("undeploy_extras {id}: {e}");
    }
    if let Some(lib) = &cfg.library_path {
        crate::extras::remove_tree(lib, kind, id);
    }

    // Déploiement éventuel dans content/ (symlink hérité OU hardlinks, §2).
    // Contrairement à un symlink, un déploiement hardlinks ne devient pas
    // orphelin tout seul quand la bibliothèque disparaît (les données restent
    // vivantes via l'entrée dans content/) — il faut le retirer explicitement.
    if let Some(ac) = &cfg.ac_install_path {
        let link = ac.join("content").join(kind.content_folder()).join(id);
        if activation::is_junction(&link) {
            let _ = std::fs::remove_dir(&link);
        } else if deploy::is_deployed(&link) {
            let _ = deploy::remove_deployment(&link);
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

/// Réinstalle un mod depuis son archive/dossier source conservé (§10/§11) :
/// réextrait (ou recopie, si la source était un dossier) le contenu et
/// remplace les fichiers de la version active en bibliothèque. Utile en cas
/// de corruption, de modification accidentelle, ou pour repartir propre sans
/// retélécharger. Ne touche ni l'id, ni les métadonnées overlay — seuls les
/// fichiers de la version active sont remplacés, puis réindexés.
pub fn reinstall_from_archive(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);
    let versions = overlay::get_versions(conn, id).map_err(|e| e.to_string())?;
    let active_id = m.active_version_id.clone().ok_or(crate::errors::NO_ACTIVE_VERSION)?;
    let version = versions
        .iter()
        .find(|v| v.id == active_id)
        .ok_or(crate::errors::ACTIVE_VERSION_NOT_FOUND)?;
    let kept = version
        .kept_archive_path
        .as_ref()
        .ok_or(crate::errors::NO_KEPT_ARCHIVE)?;
    let kept_path =
        crate::libpath::resolve(cfg.library_path.as_deref(), kept).ok_or(crate::errors::KEPT_ARCHIVE_MISSING)?;
    if !kept_path.exists() {
        return Err(crate::errors::KEPT_ARCHIVE_MISSING.into());
    }

    let workdir = std::env::temp_dir().join(format!("pitbox-reinstall-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workdir).map_err(|e| e.to_string())?;

    let extracted_dir = if kept_path.is_dir() {
        kept_path.to_path_buf()
    } else {
        let sevenzip = cfg
            .sevenzip_exe
            .as_ref()
            .ok_or(crate::errors::SEVENZIP_NOT_CONFIGURED)?;
        archive::extract(sevenzip, &kept_path, &workdir)?;
        workdir.clone()
    };

    // Retrouve le dossier du mod dans le contenu réextrait — priorité à un
    // dossier de même id (id_interne dérivé du nom de dossier à l'import),
    // sinon premier contenu du bon type (cas d'une archive à racine décalée).
    let found = modscan::scan(&extracted_dir);
    let fm = found
        .iter()
        .find(|fm| fm.kind == kind && fm.dir.file_name().is_some_and(|n| n.to_string_lossy() == id))
        .or_else(|| found.iter().find(|fm| fm.kind == kind))
        .ok_or(crate::errors::NO_CONTENT_IN_ARCHIVE)?;

    let dest = crate::libpath::resolve(cfg.library_path.as_deref(), &version.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let _ = std::fs::remove_dir_all(&dest);
    archive::copy_dir(&fm.dir, &dest).map_err(|e| e.to_string())?;

    if kept_path.is_file() {
        let _ = std::fs::remove_dir_all(&workdir);
    }

    reindex_mod(conn, cfg, id, true)?;
    overlay::add_history(conn, id, &chrono::Local::now().to_rfc3339(), "REINSTALL", "").map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ReinstallOutcome {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairAllReport {
    pub projections: submods::RepairReport,
    /// Mods actifs redéployés depuis la bibliothèque.
    pub redeployed: usize,
    pub redeploy_errors: Vec<ReinstallOutcome>,
    pub reinstalled: Vec<String>,
    pub reinstall_errors: Vec<ReinstallOutcome>,
}

/// Réparation générale (§9.3), à la manière du « purge & deploy » des autres
/// gestionnaires de mods. Sa définition tient en une phrase : **recalculer tout
/// ce qui dérive de la bibliothèque**.
///
/// Rien de tout cela n'a besoin de connaître les règles des versions
/// précédentes de l'app : `content/` est une fonction pure de la bibliothèque,
/// recalculée à chaque activation. Un changement de règles de déploiement se
/// rattrape donc en redéployant, sans rien versionner ni comparer.
///
/// 1. Projections skin/circuit manquantes ou cassées (`submods::repair_projections`).
/// 2. **Redéploiement des mods actifs** : `activation::activate` nettoie le
///    déploiement existant et le refait selon le mode et les règles du jour —
///    y compris les ajouts au jeu (§4.6ter), que les mods importés avant leur
///    existence n'ont jamais posés.
/// 3. Si `reinstall_broken`, réinstallation depuis l'archive source conservée
///    (§10/§11) des mods détectés cassés. Un mod sans archive conservée échoue
///    avec `NO_KEPT_ARCHIVE` — attendu si le réglage n'était pas actif à son
///    import, et ça n'arrête pas le reste du lot.
///
/// Seule la 3 touche la bibliothèque elle-même ; les deux premières sont sûres
/// et idempotentes, d'où l'opt-in sur celle-là seulement.
pub fn repair_all(conn: &Connection, cfg: &AppConfig, reinstall_broken: bool) -> Result<RepairAllReport, String> {
    let projections = submods::repair_projections(conn, cfg);

    // Redéploiement : seulement ce qui est **déjà actif**. Activer au passage
    // un mod que l'utilisateur avait désactivé serait une surprise, pas une
    // réparation.
    let mut redeployed = 0usize;
    let mut redeploy_errors = Vec::new();
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        if m.is_stock || !activation::is_mod_active(cfg, kind_of(&m.kind), &m.id_interne) {
            continue;
        }
        match activation::activate(conn, cfg, &m.id_interne, None) {
            Ok(()) => redeployed += 1,
            Err(error) => {
                log::warn!("redeploy {}: {error}", m.id_interne);
                redeploy_errors.push(ReinstallOutcome {
                    id: m.id_interne,
                    error,
                });
            }
        }
    }

    let mut reinstalled = Vec::new();
    let mut reinstall_errors = Vec::new();
    if reinstall_broken {
        for b in scan(conn, cfg)?.broken {
            match reinstall_from_archive(conn, cfg, &b.id) {
                Ok(()) => reinstalled.push(b.id),
                Err(error) => {
                    log::warn!("reinstall_from_archive {}: {error}", b.id);
                    reinstall_errors.push(ReinstallOutcome { id: b.id, error });
                }
            }
        }
    }

    Ok(RepairAllReport {
        projections,
        redeployed,
        redeploy_errors,
        reinstalled,
        reinstall_errors,
    })
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RelativizeReport {
    pub converted: usize,
    pub already_relative: usize,
    /// Chemins absolus dont le marqueur attendu n'a pas été retrouvé — laissés
    /// tels quels (best-effort), listés ici pour diagnostic seulement.
    pub unrecognized: Vec<String>,
}

enum PathOutcome {
    Converted(String),
    AlreadyRelative,
    Unrecognized,
}

/// `marker` est la structure interne connue de la table (ex. `["cars", id]`)
/// — jamais l'ancienne racine de bibliothèque, qu'on ne connaît pas et n'a pas
/// besoin de connaître (voir `libpath::relative_from_marker`).
fn classify_path(stored: &str, marker: &[&str]) -> PathOutcome {
    if !std::path::Path::new(stored).is_absolute() {
        return PathOutcome::AlreadyRelative;
    }
    match crate::libpath::relative_from_marker(stored, marker) {
        Some(rel) => PathOutcome::Converted(rel),
        None => PathOutcome::Unrecognized,
    }
}

/// Convertit en chemins relatifs à la bibliothèque toutes les lignes overlay
/// encore écrites en absolu (§11) — importées avant ce format, ou copiées
/// depuis une autre machine dont l'ancienne racine ne dit plus rien ici (cause
/// réelle d'un `library files not found` en masse après une migration, même
/// avec une copie de bibliothèque parfaite : chaque ligne pointait vers un
/// chemin figé sur la machine d'origine). La partie portable est retrouvée
/// via la structure interne connue de chaque table (`<type>/<id>`,
/// `layers/<parent>`, `skins/<parent>`…), jamais reconstruite ni devinée —
/// donc pas besoin de connaître l'ancienne racine. Idempotent (déjà relatif =
/// ignoré), sûr à rejouer sur n'importe quelle machine où la bibliothèque est
/// en place.
///
/// Le contenu de base Kunos (`is_stock`) est délibérément exclu pour ses
/// **versions** : leur `library_path` pointe vers `content/`, jamais la
/// bibliothèque (§12bis.1) — les couches qui lui sont rattachées, elles,
/// vivent bien sous la bibliothèque comme n'importe quel mod et sont traitées
/// normalement.
pub fn relativize_library_paths(conn: &Connection) -> Result<RelativizeReport, String> {
    fn apply(outcome: PathOutcome, label: String, report: &mut RelativizeReport) -> Option<String> {
        match outcome {
            PathOutcome::Converted(rel) => {
                report.converted += 1;
                Some(rel)
            }
            PathOutcome::AlreadyRelative => {
                report.already_relative += 1;
                None
            }
            PathOutcome::Unrecognized => {
                report.unrecognized.push(label);
                None
            }
        }
    }

    let mut report = RelativizeReport::default();
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        if !m.is_stock {
            let folder = kind_of(&m.kind).content_folder();
            for v in overlay::get_versions(conn, &m.id_interne).map_err(|e| e.to_string())? {
                let label = format!("{}: {}", m.id_interne, v.library_path);
                if let Some(rel) = apply(
                    classify_path(&v.library_path, &[folder, &m.id_interne]),
                    label,
                    &mut report,
                ) {
                    overlay::update_version_library_path(conn, &v.id, &rel).map_err(|e| e.to_string())?;
                }
                if let Some(kept) = &v.kept_archive_path {
                    let label = format!("{} (archive source) : {kept}", m.id_interne);
                    if let Some(rel) = apply(classify_path(kept, &["_source_archives"]), label, &mut report) {
                        overlay::set_kept_archive(conn, &v.id, &rel).map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        // Couches rattachées (§4.4) : toujours sous la bibliothèque, base gérée ou stock.
        for l in overlay::list_layers(conn, &m.id_interne).map_err(|e| e.to_string())? {
            let label = format!("{} (couche {}) : {}", m.id_interne, l.name, l.library_path);
            if let Some(rel) = apply(
                classify_path(&l.library_path, &["layers", &m.id_interne]),
                label,
                &mut report,
            ) {
                overlay::update_layer_library_path(conn, &l.id, &rel).map_err(|e| e.to_string())?;
            }
        }
    }

    // Sous-éléments (skins/skins de circuit/sons) : store_root selon le type.
    for sub_type in ["SKIN", "TRACK_SKIN", "SOUND"] {
        let store_root = match sub_type {
            "SKIN" => "skins",
            "TRACK_SKIN" => "track_skins",
            _ => "sounds",
        };
        for s in overlay::list_subs_by_type(conn, sub_type).map_err(|e| e.to_string())? {
            let label = format!("{}/{} : {}", s.parent_id, s.name, s.library_path);
            if let Some(rel) = apply(
                classify_path(&s.library_path, &[store_root, &s.parent_id]),
                label,
                &mut report,
            ) {
                overlay::update_sub_mod_library_path(conn, &s.id, &rel).map_err(|e| e.to_string())?;
            }
        }
    }

    for a in overlay::list_apps(conn).map_err(|e| e.to_string())? {
        let label = format!("app {} : {}", a.id, a.library_path);
        if let Some(rel) = apply(classify_path(&a.library_path, &["apps", &a.id]), label, &mut report) {
            overlay::update_app_library_path(conn, &a.id, &rel).map_err(|e| e.to_string())?;
        }
    }

    for o in overlay::list_other_mods(conn).map_err(|e| e.to_string())? {
        let label = format!("autre {} : {}", o.id, o.library_path);
        if let Some(rel) = apply(classify_path(&o.library_path, &["others", &o.id]), label, &mut report) {
            overlay::update_other_mod_library_path(conn, &o.id, &rel).map_err(|e| e.to_string())?;
        }
    }

    Ok(report)
}

/// Retire une junction orpheline. Garde-fou : refuse si ce n'est pas une junction.
pub fn remove_orphan(cfg: &AppConfig, kind: &str, id: &str) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
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
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);
    let versions = overlay::get_versions(conn, id).map_err(|e| e.to_string())?;

    let mut fresh_for_mod = None;
    for v in &versions {
        // Bibliothèque non configurée + chemin relatif (pas encore migré) :
        // rien à relire pour cette version, best-effort comme le reste de la
        // fonction (fichier manquant/illisible == mêmes défauts vides ci-dessous).
        let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path) else {
            continue;
        };
        let dir = dir.as_path();
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
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &versions.last()?.library_path)?;
        match kind {
            ModKind::Car => uijson::read_car(&dir),
            ModKind::Track => uijson::read_track(&dir),
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
    use std::path::Path;

    #[test]
    fn detects_broken_mod_missing_files() {
        let base = crate::testutil::temp_dir("maint");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        // Mod dont la version pointe vers un dossier inexistant.
        overlay::upsert_mod(&conn, "ghost", "Car", Some("B"), Some("Ghost"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "ghost",
            Some("1.0"),
            None,
            &now,
            &base.join("nope").to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "ghost", "v1").unwrap();

        let report = scan(&conn, &cfg).unwrap();
        assert_eq!(report.broken.len(), 1);
        assert_eq!(report.broken[0].id, "ghost");

        delete_broken(&conn, &cfg, "ghost").unwrap();
        assert!(overlay::get_mod(&conn, "ghost").unwrap().is_none());
    }

    #[test]
    fn delete_broken_removes_hardlink_deployment_from_content() {
        // §2 : contrairement à un symlink, un déploiement hardlinks ne devient
        // pas orphelin tout seul quand la bibliothèque disparaît (les données
        // restent vivantes dans content/) — delete_broken doit le retirer
        // explicitement, pas seulement les fichiers de bibliothèque.
        let base = crate::testutil::temp_dir("maint-hl");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let carv = library.join("cars").join("hl_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        std::fs::write(carv.join("data.txt"), "hi").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "hl_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "hl_car",
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
        overlay::set_active_version(&conn, "hl_car", "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library),
            ..Default::default()
        };

        activation::activate(&conn, &cfg, "hl_car", None).unwrap();
        let link = ac.join("content").join("cars").join("hl_car");
        assert!(deploy::is_deployed(&link), "précondition : déployé par hardlinks");

        delete_broken(&conn, &cfg, "hl_car").unwrap();

        assert!(
            !link.exists(),
            "le déploiement content/ doit être retiré, pas laissé orphelin"
        );
    }

    /// Voiture synthétique <root>/<id> avec `ui/ui_car.json` + un fichier de plus.
    fn make_car(root: &Path, id: &str, extra_content: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(
            dir.join("ui").join("ui_car.json"),
            br#"{"name":"Test","brand":"B","tags":[]}"#,
        )
        .unwrap();
        std::fs::write(dir.join("data.txt"), extra_content).unwrap();
    }

    #[test]
    fn reinstall_restores_files_from_kept_source_folder() {
        // §10/§11 : « conserver l'archive source » (ici un dossier, pas un
        // .zip — pas besoin de 7-Zip) + « réinstaller depuis l'archive source »
        // doit remplacer le contenu de la version active par une copie fraîche.
        let base = crate::testutil::temp_dir("reinstall");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        // Dossier source conservé (simule la copie faite à l'import quand le
        // réglage est actif) : contenu de référence.
        let kept_root = base.join("kept");
        make_car(&kept_root, "reinst_car", "original");
        let kept_path = kept_root.join("reinst_car");

        // Version en bibliothèque, « corrompue » (fichier annexe manquant).
        let lib_path = base.join("library").join("cars").join("reinst_car").join("v1");
        std::fs::create_dir_all(lib_path.join("ui")).unwrap();
        std::fs::write(
            lib_path.join("ui").join("ui_car.json"),
            br#"{"name":"Test","brand":"B","tags":[]}"#,
        )
        .unwrap();
        // Pas de data.txt : contenu bibliothèque corrompu/incomplet.

        overlay::upsert_mod(&conn, "reinst_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "reinst_car",
            Some("1.0"),
            None,
            &now,
            &lib_path.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "reinst_car", "v1").unwrap();
        overlay::set_kept_archive(&conn, "v1", &kept_path.to_string_lossy()).unwrap();

        assert!(
            !lib_path.join("data.txt").exists(),
            "précondition : fichier absent avant réinstallation"
        );

        reinstall_from_archive(&conn, &cfg, "reinst_car").unwrap();

        assert!(
            lib_path.join("data.txt").is_file(),
            "réinstallation doit restaurer le fichier manquant"
        );
        assert_eq!(std::fs::read_to_string(lib_path.join("data.txt")).unwrap(), "original");
    }

    #[test]
    fn subs_survive_their_parent_and_are_purged_only_on_demand() {
        // §9.3 : skins et sons sont **volontairement** conservés à la
        // suppression de leur voiture — réimporter le même id les retrouve, ce
        // qui est le geste d'une réinstallation. Ils ne deviennent des déchets
        // que si le parent ne revient jamais, d'où le nettoyage sur décision et
        // jamais automatique.
        let base = crate::testutil::temp_dir("orphan-subs");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        let now = chrono::Local::now().to_rfc3339();

        let car = library.join("cars").join("sub_car").join("v1");
        std::fs::create_dir_all(car.join("ui")).unwrap();
        std::fs::write(car.join("ui").join("ui_car.json"), b"{}").unwrap();
        overlay::upsert_mod(&conn, "sub_car", "Car", Some("B"), Some("N"), "h", None, &now).unwrap();

        let skin = library.join("skins").join("sub_car").join("red");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"x").unwrap();
        overlay::insert_sub_mod(
            &conn,
            "skin1",
            "SKIN",
            "sub_car",
            "red",
            &crate::libpath::to_relative(Some(&library), &skin),
            None,
            &now,
        )
        .unwrap();

        // Parent supprimé : le skin reste, en base comme sur disque.
        delete_broken(&conn, &cfg, "sub_car").unwrap();
        assert!(skin.join("preview.jpg").is_file(), "fichiers du skin conservés");
        let report = scan(&conn, &cfg).unwrap();
        assert_eq!(report.orphan_subs.len(), 1, "et signalé comme orphelin");
        assert_eq!(report.orphan_subs[0].name, "red");

        // Nettoyage explicite seulement.
        assert_eq!(purge_orphan_subs(&conn, &cfg).unwrap(), 1);
        assert!(!skin.exists(), "fichiers effacés");
        assert!(scan(&conn, &cfg).unwrap().orphan_subs.is_empty());
    }

    #[test]
    fn repair_redeploys_active_mods_and_leaves_inactive_ones_alone() {
        // §9.3 : « réparer » = recalculer tout ce qui dérive de la
        // bibliothèque. C'est ce qui rattrape un changement de règles de
        // déploiement sans avoir à connaître les anciennes — `content/` est une
        // fonction pure de la bibliothèque. Un mod que l'utilisateur avait
        // désactivé ne doit pas être réactivé au passage : ce serait une
        // surprise, pas une réparation.
        let base = crate::testutil::temp_dir("repair-redeploy");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };

        for id in ["on_car", "off_car"] {
            let dir = library.join("cars").join(id).join("v1");
            std::fs::create_dir_all(dir.join("ui")).unwrap();
            std::fs::write(dir.join("ui").join("ui_car.json"), b"{}").unwrap();
            std::fs::write(dir.join("model.kn5"), b"data").unwrap();
            let now = chrono::Local::now().to_rfc3339();
            overlay::upsert_mod(&conn, id, "Car", Some("B"), Some(id), "h", None, &now).unwrap();
            let vid = format!("{id}-v1");
            overlay::insert_version(
                &conn,
                &vid,
                id,
                Some("1.0"),
                None,
                &now,
                &crate::libpath::to_relative(Some(&library), &dir),
                None,
                "sig",
                &[],
                &[],
                &[],
                &[],
                None,
            )
            .unwrap();
            overlay::set_active_version(&conn, id, &vid).unwrap();
        }
        // Un seul des deux est réellement déployé.
        activation::activate(&conn, &cfg, "on_car", None).unwrap();
        let deployed = ac.join("content").join("cars").join("on_car");
        assert!(deploy::is_deployed(&deployed), "prérequis : on_car est actif");

        // Déploiement abîmé à la main : c'est ce que la réparation doit refaire.
        std::fs::remove_file(deployed.join("model.kn5")).unwrap();

        let report = repair_all(&conn, &cfg, false).unwrap();
        assert_eq!(report.redeployed, 1, "seul le mod actif est redéployé");
        assert!(report.redeploy_errors.is_empty());
        assert!(
            deployed.join("model.kn5").is_file(),
            "le fichier retiré du déploiement est revenu"
        );
        assert!(
            !ac.join("content").join("cars").join("off_car").exists(),
            "un mod inactif n'est pas activé au passage"
        );
    }

    #[test]
    fn repair_all_reinstalls_broken_mods_only_when_requested() {
        // §9.3bis (réparation générale) : reinstall_broken=false doit se
        // limiter à la réparation des projections skins (toujours sûre) et
        // laisser les mods cassés intacts ; reinstall_broken=true doit en
        // plus rattraper ceux qui ont une archive source conservée.
        let base = crate::testutil::temp_dir("repair-all");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        let kept_root = base.join("kept");
        make_car(&kept_root, "reinst_car", "original");
        let kept_path = kept_root.join("reinst_car");

        // Dossier de bibliothèque absent : cassé pour `scan` (reasonFilesMissing).
        // `reinstall_from_archive` n'a pas besoin qu'il préexiste (il le recrée).
        let lib_path = base.join("library").join("cars").join("reinst_car").join("v1");

        overlay::upsert_mod(&conn, "reinst_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "reinst_car",
            Some("1.0"),
            None,
            &now,
            &lib_path.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "reinst_car", "v1").unwrap();
        overlay::set_kept_archive(&conn, "v1", &kept_path.to_string_lossy()).unwrap();

        assert!(
            !lib_path.join("data.txt").exists(),
            "précondition : contenu bibliothèque incomplet"
        );

        let report = repair_all(&conn, &cfg, false).unwrap();
        assert!(report.reinstalled.is_empty(), "reinstall_broken=false : rien tenté");
        assert!(report.reinstall_errors.is_empty());
        assert!(
            !lib_path.join("data.txt").exists(),
            "reinstall_broken=false ne doit pas réinstaller"
        );

        let report2 = repair_all(&conn, &cfg, true).unwrap();
        assert_eq!(report2.reinstalled, vec!["reinst_car".to_string()]);
        assert!(report2.reinstall_errors.is_empty());
        assert!(
            lib_path.join("data.txt").is_file(),
            "reinstall_broken=true doit restaurer le fichier manquant"
        );
    }

    #[test]
    fn reinstall_fails_without_kept_archive() {
        let base = crate::testutil::temp_dir("reinstall-none");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        let lib_path = base.join("library").join("cars").join("no_kept").join("v1");
        std::fs::create_dir_all(lib_path.join("ui")).unwrap();
        std::fs::write(lib_path.join("ui").join("ui_car.json"), b"{}").unwrap();

        overlay::upsert_mod(&conn, "no_kept", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "no_kept",
            Some("1.0"),
            None,
            &now,
            &lib_path.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "no_kept", "v1").unwrap();

        let err = reinstall_from_archive(&conn, &cfg, "no_kept").unwrap_err();
        assert_eq!(err, crate::errors::NO_KEPT_ARCHIVE, "clé d'erreur attendue");
    }

    #[test]
    fn delete_broken_also_removes_kept_archive_copy() {
        // §10 : supprimer un mod de la bibliothèque doit aussi libérer l'espace
        // pris par l'archive source conservée (§11), pas seulement le contenu
        // extrait — sinon la suppression laisse une copie orpheline sur le disque.
        let base = crate::testutil::temp_dir("delkept");
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(base.join("library")),
            ..Default::default()
        };
        let now = chrono::Local::now().to_rfc3339();

        let lib_path = base.join("library").join("cars").join("del_car").join("v1");
        std::fs::create_dir_all(lib_path.join("ui")).unwrap();
        std::fs::write(lib_path.join("ui").join("ui_car.json"), b"{}").unwrap();

        // Copie d'archive source conservée, dans son propre dossier `<uuid>/`.
        let kept_dir = base.join("library").join("_source_archives").join("someuuid");
        std::fs::create_dir_all(&kept_dir).unwrap();
        std::fs::write(kept_dir.join("mod.zip"), b"fake archive").unwrap();

        overlay::upsert_mod(&conn, "del_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "del_car",
            Some("1.0"),
            None,
            &now,
            &lib_path.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "del_car", "v1").unwrap();
        overlay::set_kept_archive(&conn, "v1", &kept_dir.join("mod.zip").to_string_lossy()).unwrap();

        assert!(kept_dir.exists());
        delete_broken(&conn, &cfg, "del_car").unwrap();
        assert!(
            !kept_dir.exists(),
            "la copie d'archive source doit être nettoyée avec le mod"
        );
    }

    #[test]
    fn reindex_fixes_name_read_from_non_utf8_ui_json() {
        let base = crate::testutil::temp_dir("reindex");
        let track_dir = base.join("deutschlandring");
        std::fs::create_dir_all(track_dir.join("ui")).unwrap();
        // Fichier réel : "name" correct, mais un octet Windows-1252 (° en 0xB0)
        // plus loin dans "geotags" rend tout le fichier invalide en UTF-8 strict.
        let mut bytes =
            b"{\"name\": \"Deutschlandring\", \"author\": \"Fat-Alfie\", \"tags\": [\"circuit\"], \"geotags\": [\"51.8"
                .to_vec();
        bytes.push(0xB0); // degré Windows-1252, invalide en UTF-8
        bytes.extend_from_slice(b" N\"]}");
        std::fs::write(track_dir.join("ui").join("ui_track.json"), &bytes).unwrap();
        assert!(
            String::from_utf8(bytes).is_err(),
            "le fixture doit reproduire un fichier non-UTF-8"
        );

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        // Simule l'état bugué : import précédent retombé sur le nom de dossier.
        overlay::upsert_mod(
            &conn,
            "deutschlandring",
            "Track",
            None,
            Some("deutschlandring"),
            "h",
            None,
            &now,
        )
        .unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "deutschlandring",
            None,
            None,
            &now,
            &track_dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
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
    }

    #[test]
    fn reindex_recalculates_size_only_when_requested() {
        let base = crate::testutil::temp_dir("size");
        let car_dir = base.join("abarth500");
        std::fs::create_dir_all(car_dir.join("ui")).unwrap();
        std::fs::write(car_dir.join("ui").join("ui_car.json"), b"{\"name\": \"Abarth 500\"}").unwrap();
        std::fs::write(car_dir.join("data.acd"), vec![0u8; 1000]).unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "abarth500", "Car", None, Some("Abarth 500"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "abarth500",
            None,
            None,
            &now,
            &car_dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
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
    }

    #[test]
    fn relativize_library_paths_fixes_cross_machine_absolute_paths() {
        // Scénario réel (bug rapporté) : bibliothèque robocopy'ée + overlay.sqlite
        // copié depuis un PC1 vers un PC2, mais pas au même chemin absolu (lettre
        // de lecteur ou dossier différent) — chaque ligne pointait vers un chemin
        // qui n'a plus aucun sens ici, alors même que les fichiers sont bien
        // arrivés (« library files not found » en masse malgré une copie saine).
        let base = crate::testutil::temp_dir("relativize-cross");
        let new_library = base.join("new-lib"); // ce que robocopy a réellement peuplé ici
        let carv = new_library.join("cars").join("ferrari_488").join("v1.0");
        std::fs::create_dir_all(carv.join("ui")).unwrap();
        std::fs::write(carv.join("ui").join("ui_car.json"), br#"{"name":"488"}"#).unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(
            &conn,
            "ferrari_488",
            "Car",
            Some("Ferrari"),
            Some("488"),
            "h",
            None,
            &now,
        )
        .unwrap();
        // Chemin absolu tel qu'écrit sur le PC1 d'origine — n'existe pas ici.
        overlay::insert_version(
            &conn,
            "v1",
            "ferrari_488",
            Some("1.0"),
            None,
            &now,
            r"D:\OldLibraryOnPC1\cars\ferrari_488\v1.0",
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "ferrari_488", "v1").unwrap();

        // Un second mod dont le chemin ne contient aucun marqueur reconnaissable
        // (cas hypothétique — jamais produit par l'app en usage normal) : doit
        // rester tel quel, compté à part, jamais deviné au hasard.
        overlay::upsert_mod(&conn, "junk", "Car", None, Some("Junk"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v2",
            "junk",
            None,
            None,
            &now,
            r"E:\totally\unrelated\path",
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "junk", "v2").unwrap();

        let cfg = AppConfig {
            library_path: Some(new_library.clone()),
            ..Default::default()
        };

        // Précondition : cassé, le chemin stocké ne mène nulle part sur ce PC.
        let m = overlay::get_mod(&conn, "ferrari_488").unwrap().unwrap();
        assert!(
            broken_reason(&conn, &cfg, &m).is_some(),
            "précondition : cassé avant migration"
        );

        let report = relativize_library_paths(&conn).unwrap();
        assert_eq!(report.converted, 1, "seul ferrari_488 avait un marqueur reconnaissable");
        assert_eq!(report.already_relative, 0);
        assert_eq!(report.unrecognized.len(), 1, "junk laissé de côté, signalé");

        let versions = overlay::get_versions(&conn, "ferrari_488").unwrap();
        assert_eq!(versions[0].library_path, r"cars\ferrari_488\v1.0");
        // Le chemin non reconnu n'est jamais touché.
        let junk_versions = overlay::get_versions(&conn, "junk").unwrap();
        assert_eq!(junk_versions[0].library_path, r"E:\totally\unrelated\path");

        // Réparé : le chemin relatif se résout maintenant sous la bibliothèque réelle.
        let m = overlay::get_mod(&conn, "ferrari_488").unwrap().unwrap();
        assert!(broken_reason(&conn, &cfg, &m).is_none(), "réparé après migration");

        // Idempotent : rejouer ne change plus rien.
        let report2 = relativize_library_paths(&conn).unwrap();
        assert_eq!(report2.converted, 0);
        assert_eq!(report2.already_relative, 1, "ferrari_488, désormais relatif");
        assert_eq!(report2.unrecognized.len(), 1, "junk toujours signalé, pas perdu");
    }

    #[test]
    fn relativize_library_paths_never_touches_stock_content_paths() {
        // Les versions de contenu de base (is_stock) pointent vers content/, pas
        // la bibliothèque (§12bis.1) — ne doivent jamais être réinterprétées
        // relatives, même si la structure `cars/<id>` apparaît aussi dans un
        // chemin content/ par ressemblance avec le marqueur de bibliothèque.
        let base = crate::testutil::temp_dir("relativize-stock");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        let content_path = base.join("ac").join("content").join("cars").join("abarth500");
        std::fs::create_dir_all(&content_path).unwrap();

        overlay::upsert_stock_mod(&conn, "abarth500", "Car", Some("Kunos"), Some("Abarth 500"), &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "abarth500",
            None,
            Some("Kunos"),
            &now,
            &content_path.to_string_lossy(),
            None,
            "",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "abarth500", "v1").unwrap();

        let report = relativize_library_paths(&conn).unwrap();
        assert_eq!(report.converted, 0, "contenu de base jamais converti");

        let versions = overlay::get_versions(&conn, "abarth500").unwrap();
        assert_eq!(versions[0].library_path, content_path.to_string_lossy());
    }
}
