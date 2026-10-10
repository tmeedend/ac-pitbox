//! What only track liveries have (§8): **several active at once**.
//!
//! The AC engine has no notion of picking a skin at launch for a track, and it
//! is **not** CSP composing them at load time (initial assumption, disproved).
//! **Content Manager really copies the files of the active skins** into
//! `skins/default/` — checked empirically (diff before/after a selection in
//! CM's UI; unticking every skin empties that folder entirely, there is no
//! "background" to preserve). The `skins/default/cm_skins_active.json` CM
//! drops there is only its own memory for re-ticking its boxes — its real
//! activation memory lives in its opaque `Values.data` (binary, unusable), so
//! there is nothing to read or write on that side. **Pit Box keeps its own
//! activation** (`is_active` column of `sub_mods`, independent of CM): the
//! skin is stored and projected into `skins/cm_skins/<skin>/` (CM's
//! convention, so it stays selectable from CM too), and an explicit activation
//! from Pit Box recomposes `skins/default/` as the union of the active skins,
//! reproducing what CM does (see `recompose_track_skins`).

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use super::{parent_skins_dir, redeploy_host};
use crate::config::AppConfig;
use crate::resources::ExtractionMode;
use crate::{compose, deploy, identity, layers, library, overlay};

/// Fichiers annexes reconnus d'un pack de skins de circuit : pas des skins,
/// mais une amélioration du circuit qui les accompagne (§8).
const TRACK_PACK_EXTRAS: &[&str] = &["ext_config.ini"];

/// Route les fichiers annexes d'un pack de skins de circuit (ex. `ext_config.ini`,
/// trouvé à côté de `skins/` dans le pack) comme **couche** (§4.4) à la racine
/// `extension/` du circuit — indépendante de l'activation des skins eux-mêmes.
/// Idempotent (pas de doublon si déjà rattaché), best-effort.
pub(super) fn import_track_pack_extras(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    parent_id: &str,
    extra_root: &Path,
    archive_name: &str,
    mode: ExtractionMode,
) {
    for name in TRACK_PACK_EXTRAS {
        let src = extra_root.join(name);
        if !src.is_file() {
            continue;
        }
        let layer_name = name.strip_suffix(".ini").unwrap_or(name);
        if layer_exists(conn, parent_id, layer_name) {
            continue;
        }

        // Racine de couche temporaire : `<staging>/extension/<name>`, la forme
        // attendue par `layers::store_layer` (composée telle quelle sur la base).
        let staging = std::env::temp_dir().join(format!("pitbox-track-extra-{}", Uuid::new_v4()));
        let dest_file = staging.join("extension").join(name);
        let Some(dest_parent) = dest_file.parent() else {
            continue;
        };
        if std::fs::create_dir_all(dest_parent).is_err() || std::fs::copy(&src, &dest_file).is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            continue;
        }

        let diff = library::folder_path(conn, cfg, parent_id)
            .ok()
            .map(|base| identity::diff_content(&staging, &base))
            .unwrap_or(identity::DiffStats {
                added: 1,
                overwritten: 0,
                existing_total: 0,
            });
        let _ = layers::store_layer(
            conn,
            library,
            parent_id,
            crate::layers::HostKind::Track,
            layer_name,
            &staging,
            true,
            &diff,
            archive_name,
            mode,
        );
        let _ = std::fs::remove_dir_all(&staging);
        // Couche active par défaut : composer tout de suite (comme les autres
        // extensions, §4.4), best-effort.
        let _ = compose::recompose(conn, cfg, parent_id);
    }
}

/// Une couche de ce nom existe-t-elle déjà sur ce circuit ? Seul appelant :
/// le pack de skins de circuit (§8), d'où le type figé.
fn layer_exists(conn: &Connection, parent_id: &str, name: &str) -> bool {
    overlay::list_layers(conn, parent_id, crate::layers::HostKind::Track)
        .map(|v| v.iter().any(|l| l.name == name))
        .unwrap_or(false)
}

/// Skins de circuit actuellement actifs (§8) — état géré par Pit Box
/// (colonne `is_active` de `sub_mods`), **pas** un fichier posé dans le
/// dossier du circuit : le `cm_skins_active.json` que Content Manager y
/// dépose n'est que sa propre mémoire pour re-cocher ses cases, sans effet
/// en jeu de notre côté (vérifié empiriquement) — sa vraie mémoire vit dans
/// son `Values.data` opaque, binaire, non exploitable. Rien à synchroniser
/// avec CM dans ce sens : le rendu en jeu ne dépend que de ce qu'on compose
/// nous-mêmes dans `skins/default/`.
pub fn list_active_track_skins(conn: &Connection, track_id: &str) -> Vec<String> {
    overlay::list_subs_for_parent(conn, track_id)
        .unwrap_or_default()
        .into_iter()
        // A skin in the showcase keeps its switch (ESPACE§5.4) but has no
        // files to show: it is not "active" for anything that lays it.
        .filter(|s| s.sub_type == "TRACK_SKIN" && s.is_active && !s.is_skeleton())
        .map(|s| s.name)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackSkinOption {
    pub name: String,
    pub image: Option<String>,
    pub active: bool,
}

/// Skins de circuit avec une image de prévisualisation résolue (§8),
/// pour le sélecteur multi-choix de la barre latérale — cherche un fichier
/// `preview.png`/`preview.jpg` (insensible à la casse) dans le dossier
/// stocké de chaque skin.
pub fn list_track_skin_options(conn: &Connection, cfg: &AppConfig, track_id: &str) -> Vec<TrackSkinOption> {
    overlay::list_subs_for_parent(conn, track_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sub_type == "TRACK_SKIN")
        .map(|s| {
            let image = crate::libpath::resolve(cfg.library_path.as_deref(), &s.library_path)
                .and_then(|dir| find_preview_image(&dir));
            TrackSkinOption {
                name: s.name,
                image,
                active: s.is_active,
            }
        })
        .collect()
}

fn find_preview_image(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let stem_ok = p
            .file_stem()
            .map(|s| s.to_string_lossy().eq_ignore_ascii_case("preview"))
            .unwrap_or(false);
        let ext_ok = p
            .extension()
            .map(|e| {
                let e = e.to_string_lossy().to_lowercase();
                e == "png" || e == "jpg" || e == "jpeg"
            })
            .unwrap_or(false);
        if stem_ok && ext_ok {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

/// Active/désactive un skin de circuit (§8) puis recompose
/// `skins/default/` — plusieurs skins peuvent être actifs simultanément
/// (contrairement aux skins voiture, sans notion d'exclusivité). Reproduit
/// ce que fait réellement Content Manager : il **copie** les fichiers des
/// skins actifs dans `skins/default/` (vérifié empiriquement par diff
/// avant/après une sélection dans son UI — vidé entièrement quand plus
/// aucun skin n'est actif), pas une composition dynamique par CSP au
/// chargement comme on le pensait au départ.
pub fn set_track_skin_active(
    conn: &Connection,
    cfg: &AppConfig,
    track_id: &str,
    skin_name: &str,
    active: bool,
) -> Result<(), String> {
    overlay::set_track_skin_active(conn, track_id, skin_name, active).map_err(|e| e.to_string())?;
    recompose_track_skins(conn, cfg, track_id)
}

/// Reconstruit `skins/default/` comme l'union des skins actifs (§8),
/// triés par nom pour un résultat déterministe (en cas de collision de nom
/// de fichier entre deux skins — cas non observé en pratique — le dernier
/// dans l'ordre alphabétique l'emporte). Entièrement reconstruit à chaque
/// appel — pas de mise à jour incrémentale, plus simple et plus sûr qu'un
/// suivi fin de « quels fichiers appartiennent à quel skin ». Best-effort.
fn recompose_track_skins(conn: &Connection, cfg: &AppConfig, track_id: &str) -> Result<(), String> {
    let Some(skins_dir) = parent_skins_dir(conn, cfg, track_id) else {
        return Ok(());
    };
    let default_dir = skins_dir.join("default");

    // Never a skin in the showcase (ESPACE§5.4): its folder holds only its
    // manifest, which would land in `skins/default/`.
    let mut active: Vec<overlay::SubModRow> = overlay::list_subs_for_parent(conn, track_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sub_type == "TRACK_SKIN" && s.is_active && !s.is_skeleton())
        .collect();
    active.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<String> = active.iter().map(|s| s.name.clone()).collect();
    let layers: Vec<PathBuf> = active
        .into_iter()
        .filter_map(|s| crate::libpath::resolve(cfg.library_path.as_deref(), &s.library_path))
        .collect();

    deploy::compose_layers_into(&layers, &default_dir)?;

    // cm_skins_active.json (§8) : reproduit fidèlement ce que pose
    // Content Manager lui-même — absent quand aucun skin actif (vérifié
    // empiriquement : default/ est intégralement vide dans ce cas), sinon le
    // tableau des noms actifs. N'a aucun effet sur le rendu de notre côté
    // (voir doc module) mais garde CM cohérent s'il est rouvert ensuite.
    let marker = default_dir.join("cm_skins_active.json");
    if names.is_empty() {
        let _ = std::fs::remove_file(&marker);
    } else if let Ok(json) = serde_json::to_string_pretty(&names) {
        let _ = std::fs::write(&marker, json);
    }

    // `skins/default/` vient d'être reconstruit dans la bibliothèque pour un
    // circuit géré : sans redéploiement, le jeu garde la sélection précédente.
    redeploy_host(conn, cfg, track_id);

    Ok(())
}

/// Reconnaît les skins de circuit déjà présents sur le disque, **fournis
/// avec le contenu initial du mod** (§8) — jamais importés séparément
/// par Pit Box (donc jamais passés par `import_skin_pack`), le mod se
/// dézippe normalement dans `content/`/la bibliothèque comme le reste de son
/// contenu, sans y toucher. Enregistre ceux pas encore connus comme non
/// supprimables (`removable=0`) : reconnus et activables comme n'importe
/// quel skin, mais seul le mod entier peut les retirer — même logique que
/// les skins voiture (ceux fournis avec le mod ne sont pas supprimables,
/// ceux ajoutés après le sont). Lecture live du disque à chaque appel
/// (comme les ressources), idempotent, best-effort.
pub fn sync_bundled_track_skins(conn: &Connection, cfg: &AppConfig, track_id: &str) {
    let Some(skins_dir) = parent_skins_dir(conn, cfg, track_id) else {
        return;
    };
    let cm_skins_dir = skins_dir.join("cm_skins");
    let Ok(entries) = std::fs::read_dir(&cm_skins_dir) else {
        return;
    };
    let now = Local::now().to_rfc3339();
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if overlay::sub_exists(conn, "TRACK_SKIN", track_id, &name).unwrap_or(false) {
            continue;
        }
        let id = Uuid::new_v4().to_string();
        let library_path = crate::libpath::to_relative(cfg.library_path.as_deref(), &path);
        let _ = overlay::insert_bundled_track_skin(conn, &id, track_id, &name, &library_path, &now);
    }

    reconcile_track_skin_activation(conn, track_id, &skins_dir);
}

/// Réconcilie l'état actif connu de Pit Box avec `cm_skins_active.json`
/// (§8) : si l'utilisateur a sélectionné des skins directement depuis
/// Content Manager (qui écrit aussi ce fichier — vérifié empiriquement),
/// notre propre état deviendrait sinon périmé sans jamais le refléter. Le
/// marqueur reflète toujours la **dernière** sélection appliquée à
/// `skins/default/` (la nôtre ou celle de CM), donc traité comme source de
/// vérité pour l'affichage — ne touche jamais les fichiers, juste la case à
/// cocher (`sub_mods.is_active`). Absent = aucun skin actif. Best-effort.
fn reconcile_track_skin_activation(conn: &Connection, track_id: &str, skins_dir: &Path) {
    let marker = skins_dir.join("default").join("cm_skins_active.json");
    let active_names: Vec<String> = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();

    let subs = overlay::list_subs_for_parent(conn, track_id).unwrap_or_default();
    for s in subs.iter().filter(|s| s.sub_type == "TRACK_SKIN") {
        let should_be_active = active_names.iter().any(|n| n == &s.name);
        if s.is_active != should_be_active {
            let _ = overlay::set_track_skin_active(conn, track_id, &s.name, should_be_active);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;
    use crate::submods::{import_subs, remove_sub};
    #[test]
    fn track_skin_projected_under_cm_skins_and_default_composed() {
        // Convention CM (§8) : un skin de circuit se projette sous
        // skins/cm_skins/<nom>, pas skins/<nom> directement. Activation gérée
        // par Pit Box (sub_mods.is_active, pas de notion d'exclusivité comme
        // un skin voiture) et recompose skins/default/ — ce que fait
        // réellement Content Manager (vérifié par diff avant/après une
        // sélection dans son UI : il copie les fichiers, ce qui a un effet
        // réel en jeu de notre côté). On reproduit aussi le marqueur
        // cm_skins_active.json qu'il pose à côté (sans effet sur le rendu,
        // juste pour rester cohérent si CM est rouvert ensuite) — absent
        // quand aucun skin n'est actif.
        let base = crate::testutil::temp_dir("trkact");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("tracks").join("ks_black_cat_county")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_stock_mod(
            &conn,
            "ks_black_cat_county",
            "Track",
            None,
            Some("Black Cat County"),
            &now,
            false,
        )
        .unwrap();

        let pack = base
            .join("src")
            .join("assettocorsa")
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county");
        let cf1 = pack.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&cf1).unwrap();
        std::fs::write(cf1.join("preview.png"), b"IMG").unwrap();
        std::fs::write(cf1.join("shared.dds"), b"CF1").unwrap();
        let other = pack.join("skins").join("cm_skins").join("Other Skin");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("other.dds"), b"OTHER-ONLY").unwrap();
        std::fs::write(other.join("shared.dds"), b"OTHER").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        import_subs(
            &conn,
            &cfg,
            &library,
            "black_cat_county_cf1.zip",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );

        let projected = ac
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county")
            .join("skins")
            .join("cm_skins")
            .join("Black Cat County CF1");
        assert!(
            projected.is_dir(),
            "devrait être projeté sous skins/cm_skins/, pas skins/ directement"
        );

        let default_dir = ac
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county")
            .join("skins")
            .join("default");
        assert!(
            list_active_track_skins(&conn, "ks_black_cat_county").is_empty(),
            "aucun actif au départ"
        );

        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Black Cat County CF1", true).unwrap();
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["Black Cat County CF1".to_string()]
        );
        assert!(
            default_dir.join("preview.png").is_file(),
            "fichiers de CF1 composés dans default/"
        );
        assert!(!default_dir.join("other.dds").exists(), "Other Skin pas encore actif");
        assert_eq!(
            std::fs::read_to_string(default_dir.join("cm_skins_active.json")).unwrap(),
            "[\n  \"Black Cat County CF1\"\n]",
            "marqueur cm_skins_active.json posé, même format que CM"
        );

        // Plusieurs actifs en même temps (§8, pas exclusif comme le son) —
        // le dernier activé gagne les conflits de nom de fichier.
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Other Skin", true).unwrap();
        let mut active = list_active_track_skins(&conn, "ks_black_cat_county");
        active.sort();
        assert_eq!(
            active,
            vec!["Black Cat County CF1".to_string(), "Other Skin".to_string()]
        );
        assert!(
            default_dir.join("preview.png").is_file(),
            "fichier propre à CF1 toujours présent"
        );
        assert!(
            default_dir.join("other.dds").is_file(),
            "fichier propre à Other Skin ajouté"
        );
        assert_eq!(
            std::fs::read_to_string(default_dir.join("shared.dds")).unwrap(),
            "OTHER",
            "dernier skin activé gagne le conflit de nom"
        );
        assert_eq!(
            std::fs::read_to_string(default_dir.join("cm_skins_active.json")).unwrap(),
            "[\n  \"Black Cat County CF1\",\n  \"Other Skin\"\n]",
            "marqueur mis à jour avec les deux noms, ordre alphabétique"
        );

        // Désactiver CF1 : default/ reconstruit en entier, plus aucune trace
        // de CF1 — pas une simple suppression de ses fichiers.
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Black Cat County CF1", false).unwrap();
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["Other Skin".to_string()]
        );
        assert!(
            !default_dir.join("preview.png").exists(),
            "propre à CF1, doit disparaître"
        );
        assert!(default_dir.join("other.dds").is_file());

        // Plus aucun skin actif : default/ entièrement vide (comportement CM
        // observé, aucun « fond » à préserver).
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Other Skin", false).unwrap();
        assert!(list_active_track_skins(&conn, "ks_black_cat_county").is_empty());
        assert_eq!(
            std::fs::read_dir(&default_dir).unwrap().count(),
            0,
            "default/ doit être vide quand plus aucun skin n'est actif — marqueur compris"
        );
        assert!(
            !default_dir.join("cm_skins_active.json").exists(),
            "pas de marqueur quand aucun skin actif"
        );
    }

    #[test]
    fn bundled_track_skin_recognized_but_not_removable() {
        // Un skin fourni avec le contenu initial du mod (dézippé normalement
        // dans content/, jamais passé par import_skin_pack) doit quand même
        // être reconnu et activable, mais pas supprimable individuellement —
        // même logique que les skins voiture (§8).
        let base = crate::testutil::temp_dir("bundled");
        let library = base.join("library");
        let ac = base.join("ac");
        let track_dir = ac.join("content").join("tracks").join("ks_black_cat_county");
        std::fs::create_dir_all(&library).unwrap();
        // Simule le contenu initial du mod déjà dézippé (comme le reste de
        // son contenu) — un skin livré avec, sans passer par import_subs.
        let bundled = track_dir.join("skins").join("cm_skins").join("Stock Livery");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join("preview.png"), b"IMG").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
        overlay::upsert_stock_mod(
            &conn,
            "ks_black_cat_county",
            "Track",
            None,
            Some("Black Cat County"),
            &now,
            false,
        )
        .unwrap();

        // Avant sync : pas encore reconnu.
        assert!(overlay::list_subs_for_parent(&conn, "ks_black_cat_county")
            .unwrap()
            .is_empty());

        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        let subs = overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].sub_type, "TRACK_SKIN");
        assert_eq!(subs[0].name, "Stock Livery");
        assert!(
            !subs[0].removable,
            "fourni avec le mod, pas supprimable individuellement"
        );
        assert!(!subs[0].is_active, "pas actif par défaut");

        // Idempotent : un second sync ne duplique pas.
        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        assert_eq!(
            overlay::list_subs_for_parent(&conn, "ks_black_cat_county")
                .unwrap()
                .len(),
            1
        );

        // Activable comme n'importe quel skin : recompose bien skins/default/.
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Stock Livery", true).unwrap();
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["Stock Livery".to_string()]
        );
        assert!(track_dir.join("skins").join("default").join("preview.png").is_file());

        // Mais jamais supprimable individuellement.
        let sub_id = subs[0].id.clone();
        let err = remove_sub(&conn, &cfg, &sub_id).unwrap_err();
        assert_eq!(err, crate::errors::BUNDLED_NOT_REMOVABLE, "clé d'erreur attendue");
        assert!(
            overlay::get_sub_mod(&conn, &sub_id).unwrap().is_some(),
            "toujours là, pas supprimé"
        );
    }

    #[test]
    fn sync_reconciles_activation_from_marker_written_by_cm() {
        // Si l'utilisateur sélectionne des skins directement dans Content
        // Manager (qui écrit aussi cm_skins_active.json — vérifié
        // empiriquement), l'état de Pit Box doit refléter ce changement au
        // prochain chargement de la fiche, pas rester périmé sur son propre
        // dernier état (§8).
        let base = crate::testutil::temp_dir("reconcile");
        let library = base.join("library");
        let ac = base.join("ac");
        let track_dir = ac.join("content").join("tracks").join("ks_black_cat_county");
        std::fs::create_dir_all(&library).unwrap();

        let cf1 = track_dir.join("skins").join("cm_skins").join("CF1");
        std::fs::create_dir_all(&cf1).unwrap();
        std::fs::write(cf1.join("preview.png"), b"IMG").unwrap();
        let gp = track_dir.join("skins").join("cm_skins").join("GP 1966");
        std::fs::create_dir_all(&gp).unwrap();
        std::fs::write(gp.join("preview.png"), b"IMG").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
        overlay::upsert_stock_mod(
            &conn,
            "ks_black_cat_county",
            "Track",
            None,
            Some("Black Cat County"),
            &now,
            false,
        )
        .unwrap();

        // Pit Box a activé CF1 lui-même.
        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "CF1", true).unwrap();
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["CF1".to_string()]
        );

        // L'utilisateur va ensuite dans CM et sélectionne GP 1966 à la place
        // (simulé : CM écrase le marqueur avec sa propre sélection — on ne
        // simule pas la copie des fichiers, hors de portée ici).
        let marker = track_dir.join("skins").join("default").join("cm_skins_active.json");
        std::fs::write(&marker, "[\n  \"GP 1966\"\n]").unwrap();

        // Prochain chargement de la fiche (sync) : Pit Box doit se rattraper.
        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["GP 1966".to_string()],
            "l'état de Pit Box doit refléter le marqueur, même modifié par CM"
        );
    }

    #[test]
    fn track_pack_ext_config_routed_as_layer_not_skin() {
        // ext_config.ini voisin de skins/ dans un pack de skins de circuit :
        // amélioration du circuit lui-même, indépendante des skins actifs —
        // routé comme couche (extension/ext_config.ini), pas comme un skin de
        // plus (§8).
        let base = crate::testutil::temp_dir("trkext");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("tracks").join("ks_black_cat_county")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_stock_mod(
            &conn,
            "ks_black_cat_county",
            "Track",
            None,
            Some("Black Cat County"),
            &now,
            false,
        )
        .unwrap();

        let pack = base
            .join("src")
            .join("assettocorsa")
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county");
        let skin = pack.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.png"), b"IMG").unwrap();
        std::fs::write(pack.join("ext_config.ini"), b"[BASIC]\n").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        import_subs(
            &conn,
            &cfg,
            &library,
            "black_cat_county_cf1.zip",
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );

        // ext_config.ini n'est pas devenu un skin de plus.
        let stored = overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(stored.len(), 1, "un seul skin, ext_config.ini exclu");
        assert_eq!(stored[0].name, "Black Cat County CF1");

        // Routé comme couche…
        let layers = overlay::list_layers(&conn, "ks_black_cat_county", crate::layers::HostKind::Track).unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "ext_config");

        // …et composé à la vraie place attendue par CSP.
        let composed = ac
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county")
            .join("extension")
            .join("ext_config.ini");
        assert!(
            composed.is_file(),
            "ext_config.ini devrait être composé dans extension/"
        );
    }
}
