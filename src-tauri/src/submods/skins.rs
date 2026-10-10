//! Car and track liveries (§8.3): a skin pack is stored apart in the library,
//! then **projected** by a junction into the host's `skins/` — under
//! `skins/cm_skins/` for a track, CM's convention — so the game loads it.
//! Also the livery's sheet, and the repair of projections a library copy lost.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use super::track_skins::import_track_pack_extras;
use super::{
    destination, gated, host_exists, parent_skins_dir, record_sub, redeploy_host, SubGate, SubImported, SubProgress,
};
use crate::config::AppConfig;
use crate::modscan::FoundSub;
use crate::resources::{self, ExtractionMode};
use crate::{activation, library, overlay};

#[allow(clippy::too_many_arguments)]
pub(super) fn import_skin_pack(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    mode: ExtractionMode,
    gate: &SubGate,
    out: &mut Vec<SubImported>,
    progress: &mut SubProgress,
) {
    let parent = &sub.parent_id;
    // Skin de circuit (TRACK_SKIN) ou de voiture (SKIN) ? Stockage et type adaptés.
    let track = is_track_skin(conn, parent, &sub.dir);
    let sub_type = if track { "TRACK_SKIN" } else { "SKIN" };
    let store_root = if track { "track_skins" } else { "skins" };

    // `sub.dir` contient directement les dossiers de skins (les deux formes
    // d'arborescence sont déjà résolues par modscan).
    let Ok(entries) = std::fs::read_dir(&sub.dir) else {
        return;
    };
    // Une seule projection suffit à rendre le déploiement du jeu périmé — mais
    // aucune n'en change rien, et redéployer pour rien coûte un dossier entier
    // de hardlinks (§2).
    let mut projected_any = false;
    for e in entries.flatten() {
        let skin_src = e.path();
        if !skin_src.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        progress.step(&name);

        // Idempotence : ne ré-importe pas un skin déjà connu pour ce parent.
        // C'est aussi ce qui rend la reprise après arbitrage sûre — rejouer
        // l'archive entière ne duplique rien. Sauf un skin en vitrine, qui
        // revient dans sa propre ligne (ESPACE§7.5).
        let new_dest = library.join(store_root).join(parent).join(&name);
        let Some((dest, freed)) = destination(conn, library, sub_type, parent, &name, new_dest) else {
            continue;
        };

        // Voiture absente et pas encore tranché (§4.3bis) : rien n'est écrit,
        // on demande. Le défaut proposé est de ne pas importer, comme pour une
        // couche sans sa base.
        if gated(conn, gate, parent, &name) {
            out.push(SubImported {
                sub_type: sub_type.into(),
                parent_id: parent.clone(),
                name,
                projected: false,
                warning: None,
                parent_known: false,
                awaiting_decision: true,
                resources_extracted: 0,
            });
            continue;
        }

        // Fichiers annexes (§4.5.2) redirigés à part : une image à la racine
        // d'un skin est TOUJOURS un vrai aperçu, jamais une annexe (allow_root_images=false).
        let res_dir = resources::resources_dir_for(library, store_root, &[parent, &name]);
        let resources_extracted =
            match resources::file_mod(&skin_src, &dest, &res_dir, mode, !copy, resources::Source::ModFolder) {
                Ok(n) => n,
                Err(err) => {
                    out.push(SubImported {
                        sub_type: sub_type.into(),
                        parent_id: parent.clone(),
                        name,
                        projected: false,
                        warning: Some(format!("stockage : {err}")),
                        resources_extracted: 0,
                        parent_known: host_exists(conn, parent),
                        awaiting_decision: false,
                    });
                    continue;
                }
            };

        let stored = crate::libpath::to_relative(Some(library), &dest);
        record_sub(conn, freed.as_deref(), sub_type, parent, &name, &stored, source_name);

        // Projection : junction dans le skins/ de l'entité cible (voiture ou
        // circuit — pour un circuit, sous skins/cm_skins/, convention CM).
        let (projected, warning) = project_skin(conn, cfg, parent, &name, &dest, track);
        projected_any |= projected;
        out.push(SubImported {
            sub_type: sub_type.into(),
            parent_id: parent.clone(),
            name,
            projected,
            warning,
            resources_extracted,
            parent_known: host_exists(conn, parent),
            awaiting_decision: false,
        });
    }

    // Les junctions viennent d'être posées dans la bibliothèque : le jeu n'en
    // sait encore rien (voir `redeploy_host`). Une fois pour tout le pack.
    if projected_any {
        redeploy_host(conn, cfg, parent);
    }

    // Fichiers annexes au pack de skins de circuit (ex. ext_config.ini,
    // amélioration CSP du circuit lui-même, indépendante de quel(s) skin(s)
    // sont actifs) : routés comme couche, pas comme skin (§8).
    if track {
        if let Some(extra_root) = &sub.extra_root {
            import_track_pack_extras(conn, cfg, library, parent, extra_root, source_name, mode);
        }
    }
}

/// Projette un skin stocké séparément dans le `skins/` de l'entité cible via
/// junction, pour qu'AC (ou CSP, pour un circuit) le charge (§8.3). Pour un
/// circuit, sous `skins/cm_skins/<skin>/` (convention CM, §8) — pas
/// `skins/<skin>/` directement. Best-effort.
fn project_skin(
    conn: &Connection,
    cfg: &AppConfig,
    parent_id: &str,
    skin_name: &str,
    store: &Path,
    track: bool,
) -> (bool, Option<String>) {
    // Garde-fou (§4.3bis) : rien n'est posé dans le jeu pour un hôte qui n'y
    // est pas. `parent_content_dir` retombe sur `content/<type>s/<id>` quand
    // l'overlay ne connaît pas l'id — c'est voulu pour le contenu Kunos, qui
    // vit là — mais pour un id **inconnu** ce chemin ne désigne rien, et le
    // `create_dir_all` juste en dessous le **créait** : un vrai dossier
    // `content/cars/<absent>/skins/` dans l'install, exactement le dossier
    // fantôme qu'on vient de supprimer côté apps (§8.4). Pire, ce dossier
    // ferait ensuite échouer l'import de la vraie voiture, que le garde-fou
    // `REAL_FOLDER_IN_CONTENT` refuse de recouvrir.
    if !host_exists(conn, parent_id) {
        return (
            false,
            Some("cible absente de la bibliothèque : skin non projeté".into()),
        );
    }
    // A host in the showcase kept only its `ui/` (ESPACE§3.1): a projection
    // would put a skin folder back into its skeleton, and nothing would ever
    // see it — the host cannot go into the game.
    if crate::skeleton::is_showcase(conn, parent_id).unwrap_or(false) {
        return (false, Some("host in the showcase: skin not projected".into()));
    }
    let Some(skins_dir) = parent_skins_dir(conn, cfg, parent_id) else {
        return (false, Some("cible inconnue : skin non projeté".into()));
    };
    let skins_dir = if track { skins_dir.join("cm_skins") } else { skins_dir };
    if let Err(e) = std::fs::create_dir_all(&skins_dir) {
        return (false, Some(format!("création skins/ : {e}")));
    }
    let link = skins_dir.join(skin_name);
    if link.exists() {
        // Déjà présent (vrai dossier ou junction) : on ne touche à rien.
        return (
            false,
            Some(format!("« {skin_name} » déjà présent dans skins/ — non projeté")),
        );
    }
    match activation::create_junction(&link, store) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(format!("projection : {e}"))),
    }
}

/// Projects again the complete skins attached to a host whose files just came
/// back (ESPACE§7.3). Its skins came back with their own archives, maybe
/// before it, while there was no `skins/` to project them into; the ones
/// already in place are left as they are.
pub(crate) fn project_attached(conn: &Connection, cfg: &AppConfig, parent_id: &str) {
    let mut projected_any = false;
    for s in overlay::list_subs_for_parent(conn, parent_id).unwrap_or_default() {
        let track = s.sub_type == "TRACK_SKIN";
        if !(track || s.sub_type == "SKIN") || !s.removable || s.is_skeleton() {
            continue;
        }
        let Some(store) = crate::libpath::resolve(cfg.library_path.as_deref(), &s.library_path) else {
            continue;
        };
        projected_any |= project_skin(conn, cfg, parent_id, &s.name, &store, track).0;
    }
    if projected_any {
        redeploy_host(conn, cfg, parent_id);
    }
}

/// One file of a livery, relative to its stored folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinFile {
    pub path: String,
    pub size_bytes: u64,
}

/// The sheet of a separately stored livery — car (`SKIN`) or track
/// (`TRACK_SKIN`), §8.3.
///
/// It exists for the reason the sound sheet does (§8): what there is to say
/// about a livery is a **list of files**, which has no place unfolded inside a
/// list. Until now the inventory had nowhere to send a livery, so clicking its
/// name opened the host car instead — the same destination as the link on the
/// right of the row, so one of the two gestures was wasted and the livery had
/// no sheet at all.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDetail {
    pub id: String,
    /// `"SKIN"` or `"TRACK_SKIN"` — the sheet reads the same, the vocabulary
    /// does not.
    pub sub_type: String,
    pub name: String,
    /// Name declared by `ui_skin.json`, when it carries one. The very same
    /// reading as the livery picker of the host sheet — two names for one
    /// livery is a divergence no typing would catch.
    pub ui_name: Option<String>,
    pub parent_id: String,
    pub parent_name: Option<String>,
    pub source_archive: Option<String>,
    pub imported_at: String,
    /// Only a track livery has one (§8): a car livery is always loadable, the
    /// game picks at launch.
    pub is_active: bool,
    pub removable: bool,
    pub size_bytes: u64,
    pub display_name_user: Option<String>,
    pub notes_user: Option<String>,
    /// Stored folder, absolute — what "open the folder" opens.
    pub folder: String,
    /// Projected into the host's `skins/` (§8.3). False means the game
    /// cannot load it, which is worth saying rather than leaving to be
    /// guessed.
    pub projected: bool,
    /// Absolute paths, for `convertFileSrc`.
    pub preview: Option<String>,
    pub livery: Option<String>,
    pub files: Vec<SkinFile>,
}

/// Reads the sheet of a livery. Everything is read from disk at call time:
/// a livery folder can change under us, and nothing here is worth caching.
pub fn skin_detail(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<SkinDetail, String> {
    let sub = overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SUB_MOD_NOT_FOUND)?;
    if sub.sub_type != "SKIN" && sub.sub_type != "TRACK_SKIN" {
        return Err(crate::errors::NOT_A_SKIN.into());
    }
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let track = sub.sub_type == "TRACK_SKIN";

    let mut files: Vec<SkinFile> = walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(&dir).ok()?;
            Some(SkinFile {
                path: rel.to_string_lossy().replace('\\', "/"),
                size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
            })
        })
        .collect();
    files.sort_by_key(|f| f.path.to_lowercase());

    // Projection : la junction porte le nom du skin dans le `skins/` de l'hôte
    // (sous `cm_skins/` pour un circuit, convention CM, §8).
    let projected = parent_skins_dir(conn, cfg, &sub.parent_id)
        .map(|d| if track { d.join("cm_skins") } else { d })
        .map(|d| d.join(&sub.name))
        .is_some_and(|link| activation::is_junction(&link) || link.is_dir());

    let existing = |name: &str| {
        let p = dir.join(name);
        p.is_file().then(|| p.to_string_lossy().into_owned())
    };

    Ok(SkinDetail {
        id: sub.id,
        sub_type: sub.sub_type,
        ui_name: library::read_skin_name(&dir),
        parent_name: overlay::get_mod(conn, &sub.parent_id)
            .ok()
            .flatten()
            .and_then(|m| m.display_name),
        parent_id: sub.parent_id,
        source_archive: sub.source_archive,
        imported_at: sub.imported_at,
        is_active: sub.is_active,
        removable: sub.removable,
        size_bytes: files.iter().map(|f| f.size_bytes).sum(),
        display_name_user: sub.display_name_user,
        notes_user: sub.notes_user,
        projected,
        preview: existing("preview.jpg").or_else(|| existing("preview.png")),
        livery: existing("livery.png"),
        folder: dir.to_string_lossy().into_owned(),
        name: sub.name,
        files,
    })
}

/// Ouvre le dossier stocké d'une livrée dans l'explorateur.
pub fn skin_folder(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<PathBuf, String> {
    let sub = overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SUB_MOD_NOT_FOUND)?;
    crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path)
        .ok_or_else(|| crate::errors::LIBRARY_NOT_CONFIGURED.into())
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RepairReport {
    pub repaired: usize,
    pub already_ok: usize,
    /// `"<parent_id>/<name>: <raison>"`, best-effort — l'appelant ne peut rien
    /// faire de plus qu'informer (cible inconnue, création skins/ échouée…).
    pub failed: Vec<String>,
}

/// Recrée les junctions de projection de skins voiture/circuit manquantes ou
/// cassées (§8.3), sans jamais toucher aux fichiers stockés eux-mêmes.
/// Cas d'usage : une copie de bibliothèque (robocopy, migration vers une
/// autre machine) ne préserve pas les junctions — leur cible est un chemin
/// absolu propre à la machine source, donc non relogeable telle quelle.
/// `project_skin` est déjà un no-op quand la junction existe (`link.exists()`),
/// donc rejouable sans risque même sur une bibliothèque saine : on boucle
/// simplement sur tous les skins connus et on laisse ce garde-fou décider.
/// `on_step` reçoit `(rang 1-based, total, nom du skin)` avant chaque skin :
/// une fermeture plutôt qu'un `AppHandle`, pour la raison mesurée que
/// `bulk::ProgressSink` documente — l'import de Tauri dans un module métier
/// rend le binaire de test de la lib inexécutable.
pub fn repair_projections(conn: &Connection, cfg: &AppConfig, on_step: &dyn Fn(usize, usize, &str)) -> RepairReport {
    let mut report = RepairReport::default();
    let all: Vec<(bool, Vec<overlay::SubModRow>)> = ["SKIN", "TRACK_SKIN"]
        .iter()
        .map(|t| {
            (
                *t == "TRACK_SKIN",
                overlay::list_subs_by_type(conn, t).unwrap_or_default(),
            )
        })
        .collect();
    let total: usize = all.iter().map(|(_, v)| v.len()).sum();
    let mut seen = 0usize;
    for (track, subs) in all {
        for s in subs {
            seen += 1;
            on_step(seen, total, &s.name);
            let Some(store) = crate::libpath::resolve(cfg.library_path.as_deref(), &s.library_path) else {
                continue;
            };
            if !store.is_dir() {
                // Stockage lui-même absent : hors de portée ici, ce n'est pas
                // une junction cassée mais une perte de données réelle.
                continue;
            }
            // Nothing to repair on a host in the showcase, and not a failure
            // either (ESPACE§6): its skins come back with its files.
            if s.is_skeleton() || crate::skeleton::is_showcase(conn, &s.parent_id).unwrap_or(false) {
                continue;
            }
            let (projected, warning) = project_skin(conn, cfg, &s.parent_id, &s.name, &store, track);
            if projected {
                report.repaired += 1;
                continue;
            }
            // `project_skin` renvoie faux aussi bien quand la junction était
            // déjà en place (rien à faire, pas une erreur) qu'en cas d'échec
            // réel : on tranche en revérifiant si le lien existe à présent.
            let already_there = parent_skins_dir(conn, cfg, &s.parent_id)
                .map(|dir| if track { dir.join("cm_skins") } else { dir })
                .is_some_and(|dir| dir.join(&s.name).exists());
            if already_there {
                report.already_ok += 1;
            } else {
                let detail = format!("{}/{}: {}", s.parent_id, s.name, warning.unwrap_or_default());
                log::warn!("repair_projections {detail}");
                report.failed.push(detail);
            }
        }
    }
    report
}

/// Détermine si un pack de skins cible un **circuit** (TRACK_SKIN) : parent connu
/// comme circuit dans l'overlay, ou chemin sous un dossier `tracks/`.
fn is_track_skin(conn: &Connection, parent_id: &str, src: &Path) -> bool {
    if let Ok(Some(m)) = overlay::get_mod(conn, parent_id) {
        return m.kind == "Track";
    }
    src.components()
        .any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("tracks"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;
    use crate::submods::{import_subs, import_subs_reported, remove_sub};
    use chrono::Local;
    #[test]
    fn skin_pack_routed_and_stored() {
        let base = crate::testutil::temp_dir("sub");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // Pack de skins : <carId>/skins/<skin>/preview.jpg (pas de ui/ → sous-élément).
        let pack = base.join("src").join("ferrari_488");
        let skin = pack.join("skins").join("af_corse_51");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();

        // Détection : un sous-élément SKIN, parent = ferrari_488.
        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].parent_id, "ferrari_488");
        // Pas confondu avec une voiture.
        assert!(modscan::scan(&base.join("src")).is_empty());

        // Import (copie) : stocké à part + enregistré dans sub_mods.
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            "ferrari_skins.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].sub_type, "SKIN");
        assert!(library
            .join("skins")
            .join("ferrari_488")
            .join("af_corse_51")
            .join("preview.jpg")
            .is_file());
        let stored = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name, "af_corse_51");

        // Idempotence : ré-import → pas de doublon.
        let res2 = import_subs(
            &conn,
            &cfg,
            &library,
            "ferrari_skins.7z",
            &modscan::scan_subs(&base.join("src")),
            true,
            ExtractionMode::InfoOnly,
        );
        assert!(res2.is_empty());
        assert_eq!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().len(), 1);

        // Suppression propre : fichiers stockés + ligne overlay effacés.
        let sub_id = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap()[0]
            .id
            .clone();
        remove_sub(&conn, &cfg, &sub_id).unwrap();
        assert!(!library.join("skins").join("ferrari_488").join("af_corse_51").exists());
        assert!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().is_empty());
    }

    #[test]
    fn track_skin_routed_by_parent_kind() {
        let base = crate::testutil::temp_dir("tsk");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = Local::now().to_rfc3339();

        // Le circuit « spa » est connu comme Track dans l'overlay.
        overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, &now).unwrap();

        // Pack de skins pour spa : spa/skins/<skin>.
        let pack = base.join("src").join("spa");
        let skin = pack.join("skins").join("night");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("ui_track_skin.json"), b"{}").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        import_subs(
            &conn,
            &cfg,
            &library,
            "spa_skins.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );

        // Classé TRACK_SKIN, stocké sous track_skins/.
        let stored = overlay::list_subs_for_parent(&conn, "spa").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].sub_type, "TRACK_SKIN");
        assert!(library.join("track_skins").join("spa").join("night").is_dir());
    }

    #[test]
    fn track_skin_cm_skins_wrapper_routed_to_real_parent() {
        // Convention CM réelle (mod Black Cat County) : les livrées de circuit
        // vivent sous `skins/cm_skins/<skin>`, pas directement `skins/<skin>`.
        // Sans traitement dédié, `skins_are_per_car_folders` confond "cm_skins"
        // avec un dossier de voiture/circuit cible et route le pack vers un
        // parent inexistant nommé "cm_skins" au lieu du vrai circuit.
        let base = crate::testutil::temp_dir("tskcm");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(
            &conn,
            "ks_black_cat_county",
            "Track",
            None,
            Some("Black Cat County"),
            "h",
            None,
            &now,
        )
        .unwrap();

        let track = base
            .join("src")
            .join("assettocorsa")
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county");
        let skin = track.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(track.join("ext_config.ini"), b"[BASIC]\n").unwrap();
        std::fs::write(skin.join("preview.png"), b"IMG").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].parent_id, "ks_black_cat_county");

        import_subs(
            &conn,
            &cfg,
            &library,
            "black_cat_county_cf1.zip",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );

        let stored = overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].sub_type, "TRACK_SKIN");
        assert_eq!(stored[0].name, "Black Cat County CF1");
        assert!(library
            .join("track_skins")
            .join("ks_black_cat_county")
            .join("Black Cat County CF1")
            .join("preview.png")
            .is_file());
    }

    #[test]
    fn skin_pack_multi_car_shape() {
        // Forme `skins/<voiture>/<skin>` : un pack couvrant plusieurs voitures.
        let base = crate::testutil::temp_dir("subB");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

        let src = base.join("src");
        for (car, skin) in [("ferrari_488", "af_corse_51"), ("lambo_huracan", "team_a")] {
            let d = src.join("skins").join(car).join(skin);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("preview.jpg"), b"IMG").unwrap();
        }

        let subs = modscan::scan_subs(&src);
        assert_eq!(subs.len(), 2, "deux voitures cibles");
        let mut parents: Vec<String> = subs.iter().map(|s| s.parent_id.clone()).collect();
        parents.sort();
        assert_eq!(parents, vec!["ferrari_488", "lambo_huracan"]);

        let res = import_subs(&conn, &cfg, &library, "pack.7z", &subs, true, ExtractionMode::InfoOnly);
        assert_eq!(res.len(), 2);
        assert!(library
            .join("skins")
            .join("ferrari_488")
            .join("af_corse_51")
            .join("preview.jpg")
            .is_file());
        assert!(library
            .join("skins")
            .join("lambo_huracan")
            .join("team_a")
            .join("preview.jpg")
            .is_file());
    }

    /// Règle (§4.2bis) : chaque livrée d'un pack est signalée une fois, avec un
    /// dénominateur. Un pack de deux cents skins tient dans une seule entrée
    /// `FoundSub` : sans ce signalement, la barre de l'item restait immobile
    /// pendant tout son rangement, là où l'utilisateur se demande si l'app est
    /// bloquée.
    #[test]
    fn every_skin_of_a_pack_is_reported_once() {
        let base = crate::testutil::temp_dir("subprog");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

        let src = base.join("src");
        for skin in ["skin_a", "skin_b", "skin_c"] {
            let d = src.join("skins").join("ferrari_488").join(skin);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("preview.jpg"), b"IMG").unwrap();
        }
        let subs = modscan::scan_subs(&src);

        let seen = std::cell::RefCell::new(Vec::new());
        import_subs_reported(
            &conn,
            &cfg,
            &library,
            "pack.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
            &SubGate::open(),
            &|done, total, name: &str| seen.borrow_mut().push((done, total, name.to_string())),
        );

        let seen = seen.into_inner();
        assert_eq!(seen.len(), 3, "un signalement par livrée");
        assert!(
            seen.iter().all(|(_, total, _)| *total == 3),
            "dénominateur connu dès le premier signalement : {seen:?}"
        );
        let counts: Vec<usize> = seen.iter().map(|(done, _, _)| *done).collect();
        assert_eq!(counts, vec![1, 2, 3], "compteur croissant, sans trou");
        let mut names: Vec<&str> = seen.iter().map(|(_, _, n)| n.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["skin_a", "skin_b", "skin_c"], "chaque livrée nommée");
    }

    #[test]
    fn repair_projections_recreates_missing_car_skin_junction() {
        // §8.3 : une copie de bibliothèque (robocopy sans /XJ, migration
        // vers une autre machine) ne préserve pas les junctions — leur cible
        // est un chemin absolu propre à la machine source. repair_projections
        // doit recréer celle d'un skin dont le stockage survit mais dont la
        // projection dans skins/ a disparu, sans jamais toucher au stockage.
        let base = crate::testutil::temp_dir("repair-proj");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        let carv = library.join("cars").join("ferrari_488").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
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
        overlay::insert_version(
            &conn,
            "v1",
            "ferrari_488",
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
        overlay::set_active_version(&conn, "ferrari_488", "v1").unwrap();

        // Skin stocké à part + projeté, comme le ferait import_skin_pack.
        let store = library.join("skins").join("ferrari_488").join("af_corse_51");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("preview.jpg"), b"IMG").unwrap();
        overlay::insert_sub_mod(
            &conn,
            "s1",
            "SKIN",
            "ferrari_488",
            "af_corse_51",
            &store.to_string_lossy(),
            None,
            &now,
        )
        .unwrap();
        let (projected, _) = project_skin(&conn, &cfg, "ferrari_488", "af_corse_51", &store, false);
        assert!(projected, "précondition : projection initiale réussie");

        let link = carv.join("skins").join("af_corse_51");
        assert!(activation::is_junction(&link), "précondition : junction en place");

        // Simule la perte de la junction (copie sans /XJ).
        activation::remove_junction(&link).unwrap();
        assert!(!link.exists(), "précondition : junction disparue");
        assert!(store.join("preview.jpg").is_file(), "précondition : stockage intact");

        let report = repair_projections(&conn, &cfg, &|_, _, _| {});
        assert_eq!(report.repaired, 1);
        assert_eq!(report.already_ok, 0);
        assert!(report.failed.is_empty());
        assert!(activation::is_junction(&link), "junction recréée");

        // Rejouable sans risque sur une bibliothèque déjà saine : la seconde
        // passe ne doit rien recréer, juste confirmer que tout est en place.
        let report2 = repair_projections(&conn, &cfg, &|_, _, _| {});
        assert_eq!(report2.repaired, 0);
        assert_eq!(report2.already_ok, 1);
        assert!(report2.failed.is_empty());
    }

    /// Règle (§8.3) : un skin importé sur une voiture **active** est dans le
    /// jeu à la fin de l'import, pas seulement en bibliothèque.
    ///
    /// Bug réel : pour un mod géré, `parent_content_dir` désigne le dossier de
    /// bibliothèque, et `content/` n'en est qu'une copie en hardlinks figée au
    /// dernier déploiement (§2). Les 20 livrées d'un pack F1 étaient stockées,
    /// projetées, listées dans la fiche et rendues dans l'aperçu 3D — tout cela
    /// lit la bibliothèque — et n'existaient nulle part dans le jeu.
    #[test]
    fn an_imported_skin_reaches_the_game_not_just_the_library() {
        let base = crate::testutil::temp_dir("skin-to-game");
        let library = base.join("library");
        let ac = base.join("ac");
        let carv = library.join("cars").join("ferrari_488").join("v1");
        std::fs::create_dir_all(carv.join("ui")).unwrap();
        std::fs::write(carv.join("ui").join("ui_car.json"), b"{}").unwrap();
        std::fs::write(carv.join("ferrari.kn5"), b"FAKE").unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
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
        overlay::insert_version(
            &conn,
            "v1",
            "ferrari_488",
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
        activation::activate(&conn, &cfg, "ferrari_488", Some("v1")).unwrap();
        let deployed = ac.join("content").join("cars").join("ferrari_488");
        assert!(
            deployed.join("ferrari.kn5").is_file(),
            "précondition : voiture déployée"
        );

        // Pack de skins pour cette voiture : <carId>/skins/<skin>.
        let skin_src = base.join("src").join("ferrari_488").join("skins").join("af_corse_51");
        std::fs::create_dir_all(&skin_src).unwrap();
        std::fs::write(skin_src.join("preview.jpg"), b"IMG").unwrap();
        let subs = modscan::scan_subs(&base.join("src"));
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            "ferrari_skins.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert!(res[0].projected, "précondition : junction posée en bibliothèque");

        assert!(
            deployed.join("skins").join("af_corse_51").join("preview.jpg").is_file(),
            "la livrée est dans le jeu à la fin de l'import"
        );

        // Et elle en repart avec elle : le retrait rejoue le même chemin.
        let sub_id = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap()[0]
            .id
            .clone();
        remove_sub(&conn, &cfg, &sub_id).unwrap();
        assert!(
            !deployed.join("skins").join("af_corse_51").exists(),
            "livrée supprimée retirée du jeu aussi"
        );
        assert!(deployed.join("ferrari.kn5").is_file(), "la voiture elle-même intacte");
    }

    /// Règle (REFONTE§4.2) : une livrée a sa propre fiche, et celle-ci dit ce
    /// que la ligne d'inventaire ne peut pas dire — ses fichiers, et si le jeu
    /// la voit.
    ///
    /// Ce dernier point est le seul fait de la fiche qui demande une action :
    /// stockée en bibliothèque mais non projetée, une livrée est parfaitement
    /// normale partout ailleurs dans l'app et n'existe pas pour le jeu.
    #[test]
    fn a_skin_sheet_lists_its_files_and_says_whether_the_game_sees_it() {
        let base = crate::testutil::temp_dir("skin-sheet");
        let library = base.join("library");
        let ac = base.join("ac");
        let carv = library.join("cars").join("ferrari_488").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
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
        overlay::insert_version(
            &conn,
            "v1",
            "ferrari_488",
            None,
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
        overlay::set_active_version(&conn, "ferrari_488", "v1").unwrap();

        let store = library.join("skins").join("ferrari_488").join("af_corse_51");
        std::fs::create_dir_all(store.join("nested")).unwrap();
        std::fs::write(store.join("preview.jpg"), vec![0u8; 700]).unwrap();
        std::fs::write(store.join("ui_skin.json"), br#"{"skinname":"AF Corse #51"}"#).unwrap();
        std::fs::write(store.join("nested").join("body.dds"), vec![0u8; 324]).unwrap();
        overlay::insert_sub_mod(
            &conn,
            "s1",
            "SKIN",
            "ferrari_488",
            "af_corse_51",
            &store.to_string_lossy(),
            Some("pack.7z"),
            &now,
        )
        .unwrap();

        let d = skin_detail(&conn, &cfg, "s1").unwrap();
        assert_eq!(
            d.ui_name.as_deref(),
            Some("AF Corse #51"),
            "le nom déclaré par ui_skin.json"
        );
        assert_eq!(d.parent_name.as_deref(), Some("488"));
        assert_eq!(d.files.len(), 3, "les sous-dossiers comptent aussi");
        assert!(
            d.files.iter().any(|f| f.path == "nested/body.dds"),
            "chemin relatif en séparateurs /, sous-dossiers compris"
        );
        assert_eq!(d.size_bytes, 700 + 324 + 27, "somme des fichiers réels");
        assert!(d.preview.is_some(), "l'aperçu est celui du dossier");
        assert!(!d.projected, "pas encore posée : le jeu ne la voit pas");

        project_skin(&conn, &cfg, "ferrari_488", "af_corse_51", &store, false);
        assert!(
            skin_detail(&conn, &cfg, "s1").unwrap().projected,
            "posée : la fiche cesse de le signaler"
        );
    }

    /// Règle (§4.3bis) : rien n'est posé dans le jeu pour un hôte qui n'y est pas.
    ///
    /// Bug réel de la même famille que le dossier fantôme de CamTool :
    /// `parent_content_dir` retombe sur `content/<type>s/<id>` pour un id
    /// inconnu — voulu pour le contenu Kunos, qui vit là — et la projection
    /// **créait** ce dossier. Un `content/cars/<absent>/skins/` apparaissait
    /// donc dans l'install, et faisait ensuite échouer l'import de la vraie
    /// voiture, que le garde-fou `REAL_FOLDER_IN_CONTENT` refuse de recouvrir.
    #[test]
    fn a_skin_for_an_absent_car_creates_nothing_in_the_game() {
        let base = crate::testutil::temp_dir("skin-absent-car");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // Un pack de livrées pour une voiture qui n'est pas dans la bibliothèque.
        let pack = base.join("src").join("rss_formula_hybrid_2025");
        let skin = pack.join("skins").join("Alpine");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();

        let subs = crate::modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1, "le pack est bien détecté");
        let out = import_subs(&conn, &cfg, &library, "pack.7z", &subs, true, ExtractionMode::InfoOnly);

        assert_eq!(out.len(), 1, "la livrée est rangée en bibliothèque, jamais perdue");
        assert!(!out[0].projected, "mais pas projetée : la voiture n'est pas là");
        assert!(!out[0].parent_known, "et le rapport peut le dire");
        assert!(
            !ac.join("content").join("cars").join("rss_formula_hybrid_2025").exists(),
            "aucun dossier de voiture fantôme créé dans l'install"
        );
    }

    /// Règle (§4.3bis) : une livrée dont la voiture manque n'est pas rangée en
    /// silence — rien n'est écrit et on demande. Une livrée est du contenu posé
    /// *dans* une voiture : même question, et même réponse, qu'une couche dont
    /// le contenu de base manque.
    #[test]
    fn a_livery_without_its_car_waits_for_a_decision_then_lands() {
        let base = crate::testutil::temp_dir("skin-gate");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let pack = base.join("src").join("rss_formula_hybrid_2025");
        let skin = pack.join("skins").join("Alpine");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();
        let subs = crate::modscan::scan_subs(&base.join("src"));

        // On demande : rien n'est écrit.
        let gate = SubGate {
            ask: true,
            approved: &[],
        };
        let out = import_subs_reported(
            &conn,
            &cfg,
            &library,
            "pack.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
            &gate,
            &|_, _, _: &str| {},
        );
        assert_eq!(out.len(), 1, "la livrée est signalée");
        assert!(out[0].awaiting_decision, "on attend la décision");
        assert!(
            !library.join("skins").join("rss_formula_hybrid_2025").exists(),
            "rien n'a été rangé tant qu'on n'a pas tranché"
        );

        // « Garder » : la clé tranchée est <parent>/<nom>, stable d'une
        // extraction à l'autre — c'est ce qui permet la reprise.
        let approved = vec!["rss_formula_hybrid_2025/Alpine".to_string()];
        let gate = SubGate {
            ask: true,
            approved: &approved,
        };
        let out = import_subs_reported(
            &conn,
            &cfg,
            &library,
            "pack.7z",
            &subs,
            true,
            ExtractionMode::InfoOnly,
            &gate,
            &|_, _, _: &str| {},
        );
        assert_eq!(out.len(), 1);
        assert!(!out[0].awaiting_decision, "tranché : plus en attente");
        assert!(!out[0].parent_known, "la voiture manque toujours");
        assert!(
            library
                .join("skins")
                .join("rss_formula_hybrid_2025")
                .join("Alpine")
                .is_dir(),
            "rangée sous l'id visé, prête pour le jour où la voiture arrive"
        );
    }
}
