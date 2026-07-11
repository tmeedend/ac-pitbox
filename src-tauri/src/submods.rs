//! Sous-éléments rattachés (§12bis.2) : skins et sons. Routés à l'import vers
//! un stockage **séparé** dans la bibliothèque (`<lib>/skins/<parent>/<skin>` et
//! `<lib>/sounds/<parent>/<nom>`), tracés dans l'overlay `sub_mods`, sans jamais
//! polluer la bibliothèque principale (§12bis.3).
//!
//! Asymétrie (§12bis.2) :
//! - **Skin voiture** : pas d'activation filesystem. Pour qu'AC le charge, il
//!   est **projeté** par junction dans le dossier `skins/` de la voiture cible
//!   (`<parent skins>/<skin>` → stockage séparé). Tous les skins présents sont
//!   disponibles, le jeu choisit via `SkinId` au lancement.
//! - **Skin circuit** : contrairement aux skins voiture, **plusieurs actifs en
//!   même temps** — le moteur AC n'a aucune notion de sélection au lancement
//!   pour un circuit, et ce n'est **pas** CSP qui compose dynamiquement au
//!   chargement (hypothèse initiale infirmée). C'est **Content Manager qui
//!   copie réellement les fichiers des skins actifs** dans `skins/default/`
//!   — vérifié empiriquement (diff avant/après une sélection dans l'UI CM ;
//!   décocher tous les skins vide entièrement ce dossier, aucun « fond » à
//!   préserver). Le `skins/default/cm_skins_active.json` que CM y dépose
//!   n'est que sa propre mémoire pour re-cocher ses cases — sa vraie mémoire
//!   d'activation vit dans son `Values.data` opaque (binaire, non
//!   exploitable), donc rien à lire/écrire de ce côté. **Pit Box gère sa
//!   propre activation** (colonne `is_active` de `sub_mods`, indépendante de
//!   CM) : le skin est stocké et projeté dans `skins/cm_skins/<skin>/` (pas
//!   `skins/<skin>/` directement, convention CM, pour rester sélectionnable
//!   depuis CM aussi), et une activation explicite depuis Pit Box recompose
//!   directement `skins/default/` = union des skins actifs, reproduisant le
//!   comportement de CM (§4.6bis, voir `recompose_track_skins`).
//! - **Son** : exclusif (un seul actif). La bascule réelle des fichiers `sfx/`
//!   est un lot suivant — ici on **stocke et enregistre** le son (inactif).

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::{FoundSub, ModKind, SubKind};
use crate::resources::{self, ExtractionMode};
use crate::{activation, archive, compose, deploy, identity, layers, library, overlay};

#[derive(Debug, Clone, Serialize)]
pub struct SubImported {
    /// "SKIN" | "SOUND"
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    /// Skin projeté (visible par AC) ; faux si le parent est inconnu/conflit.
    pub projected: bool,
    pub warning: Option<String>,
    /// Fichiers annexes redirigés vers le dossier ressources (§4.6).
    pub resources_extracted: usize,
}

/// Importe les sous-éléments détectés (§12bis.2). `copy` préserve la source.
#[allow(clippy::too_many_arguments)]
pub fn import_subs(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    subs: &[FoundSub],
    copy: bool,
    mode: ExtractionMode,
) -> Vec<SubImported> {
    let mut out = Vec::new();
    for sub in subs {
        match sub.kind {
            SubKind::Skin => import_skin_pack(conn, cfg, library, source_name, sub, copy, mode, &mut out),
            SubKind::Sound => import_sound(conn, library, source_name, sub, copy, mode, &mut out),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn import_skin_pack(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    mode: ExtractionMode,
    out: &mut Vec<SubImported>,
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
    for e in entries.flatten() {
        let skin_src = e.path();
        if !skin_src.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();

        // Idempotence : ne ré-importe pas un skin déjà connu pour ce parent.
        if overlay::sub_exists(conn, sub_type, parent, &name).unwrap_or(false) {
            continue;
        }

        let dest = library.join(store_root).join(parent).join(&name);
        // Fichiers annexes (§4.6) redirigés à part : une image à la racine
        // d'un skin est TOUJOURS un vrai aperçu, jamais une annexe (allow_root_images=false).
        let res_dir = resources::resources_dir_for(library, store_root, &[parent, &name]);
        let resources_extracted = match resources::file_mod(&skin_src, &dest, &res_dir, mode, !copy, false) {
            Ok(n) => n,
            Err(err) => {
                out.push(SubImported {
                    sub_type: sub_type.into(),
                    parent_id: parent.clone(),
                    name,
                    projected: false,
                    warning: Some(format!("stockage : {err}")),
                    resources_extracted: 0,
                });
                continue;
            }
        };

        let id = Uuid::new_v4().to_string();
        let _ = overlay::insert_sub_mod(
            conn,
            &id,
            sub_type,
            parent,
            &name,
            &dest.to_string_lossy(),
            Some(source_name),
            &Local::now().to_rfc3339(),
        );

        // Projection : junction dans le skins/ de l'entité cible (voiture ou
        // circuit — pour un circuit, sous skins/cm_skins/, convention CM).
        let (projected, warning) = project_skin(conn, cfg, parent, &name, &dest, track);
        out.push(SubImported {
            sub_type: sub_type.into(),
            parent_id: parent.clone(),
            name,
            projected,
            warning,
            resources_extracted,
        });
    }

    // Fichiers annexes au pack de skins de circuit (ex. ext_config.ini,
    // amélioration CSP du circuit lui-même, indépendante de quel(s) skin(s)
    // sont actifs) : routés comme couche, pas comme skin (§4.6bis).
    if track {
        if let Some(extra_root) = &sub.extra_root {
            import_track_pack_extras(conn, cfg, library, parent, extra_root, source_name, mode);
        }
    }
}

/// Projette un skin stocké séparément dans le `skins/` de l'entité cible via
/// junction, pour qu'AC (ou CSP, pour un circuit) le charge (§12bis.2). Pour un
/// circuit, sous `skins/cm_skins/<skin>/` (convention CM, §4.6bis) — pas
/// `skins/<skin>/` directement. Best-effort.
fn project_skin(
    conn: &Connection,
    cfg: &AppConfig,
    parent_id: &str,
    skin_name: &str,
    store: &Path,
    track: bool,
) -> (bool, Option<String>) {
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
        return (false, Some(format!("« {skin_name} » déjà présent dans skins/ — non projeté")));
    }
    match activation::create_junction(&link, store) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(format!("projection : {e}"))),
    }
}

/// Fichiers annexes reconnus d'un pack de skins de circuit : pas des skins,
/// mais une amélioration du circuit qui les accompagne (§4.6bis).
const TRACK_PACK_EXTRAS: &[&str] = &["ext_config.ini"];

/// Route les fichiers annexes d'un pack de skins de circuit (ex. `ext_config.ini`,
/// trouvé à côté de `skins/` dans le pack) comme **couche** (§4.4) à la racine
/// `extension/` du circuit — indépendante de l'activation des skins eux-mêmes.
/// Idempotent (pas de doublon si déjà rattaché), best-effort.
fn import_track_pack_extras(
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
        let Some(dest_parent) = dest_file.parent() else { continue };
        if std::fs::create_dir_all(dest_parent).is_err() || std::fs::copy(&src, &dest_file).is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            continue;
        }

        let diff = library::folder_path(conn, cfg, parent_id)
            .ok()
            .map(|base| identity::diff_content(&staging, &base))
            .unwrap_or(identity::DiffStats { added: 1, overwritten: 0, existing_total: 0 });
        let _ = layers::store_layer(
            conn, library, parent_id, ModKind::Track, layer_name, &staging, true, &diff, archive_name, mode,
        );
        let _ = std::fs::remove_dir_all(&staging);
        // Couche active par défaut : composer tout de suite (comme les autres
        // extensions, §4.4), best-effort.
        let _ = compose::recompose(conn, cfg, parent_id);
    }
}

fn layer_exists(conn: &Connection, parent_id: &str, name: &str) -> bool {
    overlay::list_layers(conn, parent_id).map(|v| v.iter().any(|l| l.name == name)).unwrap_or(false)
}

fn parent_skins_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Option<PathBuf> {
    parent_subdir(conn, cfg, parent_id, "skins")
}

/// Skins de circuit actuellement actifs (§4.6bis) — état géré par Pit Box
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
        .filter(|s| s.sub_type == "TRACK_SKIN" && s.is_active)
        .map(|s| s.name)
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackSkinOption {
    pub name: String,
    pub image: Option<String>,
    pub active: bool,
}

/// Skins de circuit avec une image de prévisualisation résolue (§4.6bis),
/// pour le sélecteur multi-choix de la barre latérale — cherche un fichier
/// `preview.png`/`preview.jpg` (insensible à la casse) dans le dossier
/// stocké de chaque skin.
pub fn list_track_skin_options(conn: &Connection, track_id: &str) -> Vec<TrackSkinOption> {
    overlay::list_subs_for_parent(conn, track_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sub_type == "TRACK_SKIN")
        .map(|s| {
            let image = find_preview_image(Path::new(&s.library_path));
            TrackSkinOption { name: s.name, image, active: s.is_active }
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

/// Active/désactive un skin de circuit (§4.6bis) puis recompose
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

/// Reconstruit `skins/default/` comme l'union des skins actifs (§4.6bis),
/// triés par nom pour un résultat déterministe (en cas de collision de nom
/// de fichier entre deux skins — cas non observé en pratique — le dernier
/// dans l'ordre alphabétique l'emporte). Entièrement reconstruit à chaque
/// appel — pas de mise à jour incrémentale, plus simple et plus sûr qu'un
/// suivi fin de « quels fichiers appartiennent à quel skin ». Best-effort.
fn recompose_track_skins(conn: &Connection, cfg: &AppConfig, track_id: &str) -> Result<(), String> {
    let Some(skins_dir) = parent_skins_dir(conn, cfg, track_id) else { return Ok(()) };
    let default_dir = skins_dir.join("default");

    let mut active: Vec<overlay::SubModRow> = overlay::list_subs_for_parent(conn, track_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sub_type == "TRACK_SKIN" && s.is_active)
        .collect();
    active.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<String> = active.iter().map(|s| s.name.clone()).collect();
    let layers: Vec<PathBuf> = active.into_iter().map(|s| PathBuf::from(s.library_path)).collect();

    deploy::compose_layers_into(&layers, &default_dir)?;

    // cm_skins_active.json (§4.6bis) : reproduit fidèlement ce que pose
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

    Ok(())
}

/// Reconnaît les skins de circuit déjà présents sur le disque, **fournis
/// avec le contenu initial du mod** (§4.6bis) — jamais importés séparément
/// par Pit Box (donc jamais passés par `import_skin_pack`), le mod se
/// dézippe normalement dans `content/`/la bibliothèque comme le reste de son
/// contenu, sans y toucher. Enregistre ceux pas encore connus comme non
/// supprimables (`removable=0`) : reconnus et activables comme n'importe
/// quel skin, mais seul le mod entier peut les retirer — même logique que
/// les skins voiture (ceux fournis avec le mod ne sont pas supprimables,
/// ceux ajoutés après le sont). Lecture live du disque à chaque appel
/// (comme les ressources), idempotent, best-effort.
pub fn sync_bundled_track_skins(conn: &Connection, cfg: &AppConfig, track_id: &str) {
    let Some(skins_dir) = parent_skins_dir(conn, cfg, track_id) else { return };
    let cm_skins_dir = skins_dir.join("cm_skins");
    let Ok(entries) = std::fs::read_dir(&cm_skins_dir) else { return };
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
        let _ = overlay::insert_bundled_track_skin(conn, &id, track_id, &name, &path.to_string_lossy(), &now);
    }

    reconcile_track_skin_activation(conn, track_id, &skins_dir);
}

/// Réconcilie l'état actif connu de Pit Box avec `cm_skins_active.json`
/// (§4.6bis) : si l'utilisateur a sélectionné des skins directement depuis
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

/// Dossier `<sub>/` de l'entité cible (voiture ou circuit) : version active en
/// bibliothèque si c'est un mod géré, sinon `content/<type>s/<id>/<sub>` (base
/// Kunos). Le type est déduit de l'overlay (Car → cars, Track → tracks).
fn parent_subdir(conn: &Connection, cfg: &AppConfig, parent_id: &str, sub: &str) -> Option<PathBuf> {
    let m = overlay::get_mod(conn, parent_id).ok().flatten();
    if let Some(m) = &m {
        if !m.is_stock {
            if let Some(vid) = &m.active_version_id {
                if let Ok(Some(p)) = overlay::get_version_path(conn, vid) {
                    return Some(Path::new(&p).join(sub));
                }
            }
        }
    }
    let folder = if m.as_ref().map(|m| m.kind.as_str()) == Some("Track") { "tracks" } else { "cars" };
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join(folder).join(parent_id).join(sub))
}

/// Détermine si un pack de skins cible un **circuit** (TRACK_SKIN) : parent connu
/// comme circuit dans l'overlay, ou chemin sous un dossier `tracks/`.
fn is_track_skin(conn: &Connection, parent_id: &str, src: &Path) -> bool {
    if let Ok(Some(m)) = overlay::get_mod(conn, parent_id) {
        return m.kind == "Track";
    }
    src.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("tracks"))
}

/// Tente d'identifier la voiture ciblée par un mod de son quand la détection
/// par arborescence (modscan) retombe sur le dossier générique "sfx" (nom de
/// dossier standard AC, `content/cars/<id>/sfx`, jamais un nom de voiture) :
/// l'id d'une voiture connue apparaît-il, seul, dans le nom d'archive/dossier
/// importé (souvent explicite, ex. « Sound - <id> by <auteur> ») ?
fn guess_sound_parent(conn: &Connection, source_name: &str) -> Option<String> {
    let lower = source_name.to_lowercase();
    let mut matches = overlay::list_mods(conn)
        .ok()?
        .into_iter()
        .filter(|m| m.kind == "Car" && lower.contains(&m.id_interne.to_lowercase()))
        .map(|m| m.id_interne);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn import_sound(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    mode: ExtractionMode,
    out: &mut Vec<SubImported>,
) {
    // "sfx" n'identifie aucune voiture (§12bis.2, limite connue) : on retombe
    // sur le nom d'archive/dossier importé, en essayant d'abord d'y retrouver
    // la voiture ciblée pour rattacher le son au bon endroit.
    let generic = sub.parent_id.eq_ignore_ascii_case("sfx");
    let parent = if generic {
        guess_sound_parent(conn, source_name).unwrap_or_else(|| source_name.to_string())
    } else {
        sub.parent_id.clone()
    };
    let name = if generic {
        source_name.to_string()
    } else {
        sub.dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_name.to_string())
    };

    if overlay::sub_exists(conn, "SOUND", &parent, &name).unwrap_or(false) {
        return;
    }

    let dest = library.join("sounds").join(&parent).join(&name);
    // Fichiers annexes (§4.6) redirigés à part (GUIDs.txt reste toujours du
    // contenu, voir resources::classify — jamais confondu avec une annexe).
    let res_dir = resources::resources_dir_for(library, "sounds", &[&parent, &name]);
    let resources_extracted = match resources::file_mod(&sub.dir, &dest, &res_dir, mode, !copy, false) {
        Ok(n) => n,
        Err(err) => {
            out.push(SubImported {
                sub_type: "SOUND".into(),
                parent_id: parent,
                name,
                projected: false,
                warning: Some(format!("stockage : {err}")),
                resources_extracted: 0,
            });
            return;
        }
    };

    let id = Uuid::new_v4().to_string();
    let _ = overlay::insert_sub_mod(
        conn,
        &id,
        "SOUND",
        &parent,
        &name,
        &dest.to_string_lossy(),
        Some(source_name),
        &Local::now().to_rfc3339(),
    );
    out.push(SubImported {
        sub_type: "SOUND".into(),
        parent_id: parent,
        name,
        projected: false,
        warning: None,
        resources_extracted,
    });
}

// --- Bascule exclusive du son (§12bis.2) ------------------------------------

/// Active un mod de son : remplace réellement le `sfx/` de la voiture par les
/// fichiers du mod (bascule exclusive). Le son d'origine est **sauvegardé une
/// fois** pour pouvoir y revenir — jamais détruit irréversiblement (§12bis.2).
pub fn activate_sound(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id).map_err(|e| e.to_string())?.ok_or("son introuvable")?;
    if sub.sub_type != "SOUND" {
        return Err("ce sous-élément n'est pas un mod de son".into());
    }
    let sfx = parent_subdir(conn, cfg, &sub.parent_id, "sfx").ok_or("voiture cible inconnue")?;
    let backup = sound_backup_dir(cfg, &sub.parent_id)?;

    // Sauvegarde du son d'origine, une seule fois (préserve le vrai original).
    if !backup.exists() {
        std::fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        if sfx.is_dir() {
            archive::copy_dir(&sfx, &backup).map_err(|e| format!("sauvegarde du son d'origine : {e}"))?;
        }
    }

    replace_dir_contents(Path::new(&sub.library_path), &sfx)?;
    overlay::set_active_sound(conn, &sub.parent_id, Some(sub_id)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Restaure le son d'origine d'une voiture (désactive le mod de son actif).
pub fn restore_sound(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Result<(), String> {
    let backup = sound_backup_dir(cfg, parent_id)?;
    if backup.is_dir() {
        let sfx = parent_subdir(conn, cfg, parent_id, "sfx").ok_or("voiture cible inconnue")?;
        replace_dir_contents(&backup, &sfx)?;
    }
    overlay::set_active_sound(conn, parent_id, None).map_err(|e| e.to_string())?;
    Ok(())
}

/// Supprime proprement un sous-élément (§12bis.3) : retire la junction de
/// projection (skin) ou restaure le son d'origine (son actif), efface les
/// fichiers stockés, puis la ligne overlay. Garde-fou junction respecté.
pub fn remove_sub(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id).map_err(|e| e.to_string())?.ok_or("sous-élément introuvable")?;
    if !sub.removable {
        return Err("fourni avec le contenu initial du mod : non supprimable individuellement".into());
    }
    match sub.sub_type.as_str() {
        "SKIN" => {
            // Retire la junction de projection dans le skins/ de l'entité cible.
            if let Some(skins_dir) = parent_subdir(conn, cfg, &sub.parent_id, "skins") {
                let link = skins_dir.join(&sub.name);
                if activation::is_junction(&link) {
                    let _ = activation::remove_junction(&link);
                }
            }
        }
        "TRACK_SKIN" => {
            // Même chose, sous skins/cm_skins/ (convention CM, §4.6bis) — et
            // retiré du marqueur d'activation s'il y était.
            if let Some(skins_dir) = parent_subdir(conn, cfg, &sub.parent_id, "skins") {
                let link = skins_dir.join("cm_skins").join(&sub.name);
                if activation::is_junction(&link) {
                    let _ = activation::remove_junction(&link);
                }
            }
            let _ = set_track_skin_active(conn, cfg, &sub.parent_id, &sub.name, false);
        }
        "SOUND" => {
            // Si actif, on rétablit d'abord le son d'origine.
            if sub.is_active {
                restore_sound(conn, cfg, &sub.parent_id)?;
            }
        }
        _ => {}
    }
    // Fichiers stockés à part.
    let _ = std::fs::remove_dir_all(Path::new(&sub.library_path));
    overlay::delete_sub_mod(conn, sub_id).map_err(|e| e.to_string())
}

/// `<lib>/sounds/<parent>/__original__` : sauvegarde du son d'origine.
fn sound_backup_dir(cfg: &AppConfig, parent_id: &str) -> Result<PathBuf, String> {
    let lib = cfg.library_path.as_ref().ok_or("bibliothèque non configurée")?;
    Ok(lib.join("sounds").join(parent_id).join("__original__"))
}

/// Remplace le contenu de `dst` par celui de `src`. `dst` est toujours un vrai
/// dossier (sous-dossier `sfx/` de la voiture), jamais une junction.
fn replace_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| format!("nettoyage de {}: {e}", dst.display()))?;
    }
    archive::copy_dir(src, dst).map_err(|e| format!("copie du son : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;

    #[test]
    fn skin_pack_routed_and_stored() {
        let base = std::env::temp_dir().join(format!("pitbox-sub-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

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
        let res = import_subs(&conn, &cfg, &library, "ferrari_skins.7z", &subs, true, ExtractionMode::InfoOnly);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].sub_type, "SKIN");
        assert!(library.join("skins").join("ferrari_488").join("af_corse_51").join("preview.jpg").is_file());
        let stored = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name, "af_corse_51");

        // Idempotence : ré-import → pas de doublon.
        let res2 = import_subs(&conn, &cfg, &library, "ferrari_skins.7z", &modscan::scan_subs(&base.join("src")), true, ExtractionMode::InfoOnly);
        assert!(res2.is_empty());
        assert_eq!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().len(), 1);

        // Suppression propre : fichiers stockés + ligne overlay effacés.
        let sub_id = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap()[0].id.clone();
        remove_sub(&conn, &cfg, &sub_id).unwrap();
        assert!(!library.join("skins").join("ferrari_488").join("af_corse_51").exists());
        assert!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn track_skin_routed_by_parent_kind() {
        let base = std::env::temp_dir().join(format!("pitbox-tsk-{}", Uuid::new_v4()));
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
        import_subs(&conn, &cfg, &library, "spa_skins.7z", &subs, true, ExtractionMode::InfoOnly);

        // Classé TRACK_SKIN, stocké sous track_skins/.
        let stored = overlay::list_subs_for_parent(&conn, "spa").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].sub_type, "TRACK_SKIN");
        assert!(library.join("track_skins").join("spa").join("night").is_dir());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn track_skin_cm_skins_wrapper_routed_to_real_parent() {
        // Convention CM réelle (mod Black Cat County) : les livrées de circuit
        // vivent sous `skins/cm_skins/<skin>`, pas directement `skins/<skin>`.
        // Sans traitement dédié, `skins_are_per_car_folders` confond "cm_skins"
        // avec un dossier de voiture/circuit cible et route le pack vers un
        // parent inexistant nommé "cm_skins" au lieu du vrai circuit.
        let base = std::env::temp_dir().join(format!("pitbox-tskcm-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "ks_black_cat_county", "Track", None, Some("Black Cat County"), "h", None, &now)
            .unwrap();

        let track = base.join("src").join("assettocorsa").join("content").join("tracks").join("ks_black_cat_county");
        let skin = track.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(track.join("ext_config.ini"), b"[BASIC]\n").unwrap();
        std::fs::write(skin.join("preview.png"), b"IMG").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].parent_id, "ks_black_cat_county");

        import_subs(&conn, &cfg, &library, "black_cat_county_cf1.zip", &subs, true, ExtractionMode::InfoOnly);

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

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn track_skin_projected_under_cm_skins_and_default_composed() {
        // Convention CM (§4.6bis) : un skin de circuit se projette sous
        // skins/cm_skins/<nom>, pas skins/<nom> directement. Activation gérée
        // par Pit Box (sub_mods.is_active, pas de notion d'exclusivité comme
        // un skin voiture) et recompose skins/default/ — ce que fait
        // réellement Content Manager (vérifié par diff avant/après une
        // sélection dans son UI : il copie les fichiers, ce qui a un effet
        // réel en jeu de notre côté). On reproduit aussi le marqueur
        // cm_skins_active.json qu'il pose à côté (sans effet sur le rendu,
        // juste pour rester cohérent si CM est rouvert ensuite) — absent
        // quand aucun skin n'est actif.
        let base = std::env::temp_dir().join(format!("pitbox-trkact-{}", Uuid::new_v4()));
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("tracks").join("ks_black_cat_county")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();

        overlay::upsert_stock_mod(&conn, "ks_black_cat_county", "Track", None, Some("Black Cat County"), &now).unwrap();

        let pack = base.join("src").join("assettocorsa").join("content").join("tracks").join("ks_black_cat_county");
        let cf1 = pack.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&cf1).unwrap();
        std::fs::write(cf1.join("preview.png"), b"IMG").unwrap();
        std::fs::write(cf1.join("shared.dds"), b"CF1").unwrap();
        let other = pack.join("skins").join("cm_skins").join("Other Skin");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("other.dds"), b"OTHER-ONLY").unwrap();
        std::fs::write(other.join("shared.dds"), b"OTHER").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        import_subs(&conn, &cfg, &library, "black_cat_county_cf1.zip", &subs, true, ExtractionMode::InfoOnly);

        let projected = ac
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county")
            .join("skins")
            .join("cm_skins")
            .join("Black Cat County CF1");
        assert!(projected.is_dir(), "devrait être projeté sous skins/cm_skins/, pas skins/ directement");

        let default_dir = ac.join("content").join("tracks").join("ks_black_cat_county").join("skins").join("default");
        assert!(list_active_track_skins(&conn, "ks_black_cat_county").is_empty(), "aucun actif au départ");

        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Black Cat County CF1", true).unwrap();
        assert_eq!(
            list_active_track_skins(&conn, "ks_black_cat_county"),
            vec!["Black Cat County CF1".to_string()]
        );
        assert!(default_dir.join("preview.png").is_file(), "fichiers de CF1 composés dans default/");
        assert!(!default_dir.join("other.dds").exists(), "Other Skin pas encore actif");
        assert_eq!(
            std::fs::read_to_string(default_dir.join("cm_skins_active.json")).unwrap(),
            "[\n  \"Black Cat County CF1\"\n]",
            "marqueur cm_skins_active.json posé, même format que CM"
        );

        // Plusieurs actifs en même temps (§4.6bis, pas exclusif comme le son) —
        // le dernier activé gagne les conflits de nom de fichier.
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Other Skin", true).unwrap();
        let mut active = list_active_track_skins(&conn, "ks_black_cat_county");
        active.sort();
        assert_eq!(active, vec!["Black Cat County CF1".to_string(), "Other Skin".to_string()]);
        assert!(default_dir.join("preview.png").is_file(), "fichier propre à CF1 toujours présent");
        assert!(default_dir.join("other.dds").is_file(), "fichier propre à Other Skin ajouté");
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
        assert_eq!(list_active_track_skins(&conn, "ks_black_cat_county"), vec!["Other Skin".to_string()]);
        assert!(!default_dir.join("preview.png").exists(), "propre à CF1, doit disparaître");
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
        assert!(!default_dir.join("cm_skins_active.json").exists(), "pas de marqueur quand aucun skin actif");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bundled_track_skin_recognized_but_not_removable() {
        // Un skin fourni avec le contenu initial du mod (dézippé normalement
        // dans content/, jamais passé par import_skin_pack) doit quand même
        // être reconnu et activable, mais pas supprimable individuellement —
        // même logique que les skins voiture (§4.6bis).
        let base = std::env::temp_dir().join(format!("pitbox-bundled-{}", Uuid::new_v4()));
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
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_black_cat_county", "Track", None, Some("Black Cat County"), &now).unwrap();

        // Avant sync : pas encore reconnu.
        assert!(overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap().is_empty());

        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        let subs = overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].sub_type, "TRACK_SKIN");
        assert_eq!(subs[0].name, "Stock Livery");
        assert!(!subs[0].removable, "fourni avec le mod, pas supprimable individuellement");
        assert!(!subs[0].is_active, "pas actif par défaut");

        // Idempotent : un second sync ne duplique pas.
        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        assert_eq!(overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap().len(), 1);

        // Activable comme n'importe quel skin : recompose bien skins/default/.
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "Stock Livery", true).unwrap();
        assert_eq!(list_active_track_skins(&conn, "ks_black_cat_county"), vec!["Stock Livery".to_string()]);
        assert!(track_dir.join("skins").join("default").join("preview.png").is_file());

        // Mais jamais supprimable individuellement.
        let sub_id = subs[0].id.clone();
        let err = remove_sub(&conn, &cfg, &sub_id).unwrap_err();
        assert!(err.contains("non supprimable"), "message attendu, obtenu : {err}");
        assert!(overlay::get_sub_mod(&conn, &sub_id).unwrap().is_some(), "toujours là, pas supprimé");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sync_reconciles_activation_from_marker_written_by_cm() {
        // Si l'utilisateur sélectionne des skins directement dans Content
        // Manager (qui écrit aussi cm_skins_active.json — vérifié
        // empiriquement), l'état de Pit Box doit refléter ce changement au
        // prochain chargement de la fiche, pas rester périmé sur son propre
        // dernier état (§4.6bis).
        let base = std::env::temp_dir().join(format!("pitbox-reconcile-{}", Uuid::new_v4()));
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
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();
        overlay::upsert_stock_mod(&conn, "ks_black_cat_county", "Track", None, Some("Black Cat County"), &now).unwrap();

        // Pit Box a activé CF1 lui-même.
        sync_bundled_track_skins(&conn, &cfg, "ks_black_cat_county");
        set_track_skin_active(&conn, &cfg, "ks_black_cat_county", "CF1", true).unwrap();
        assert_eq!(list_active_track_skins(&conn, "ks_black_cat_county"), vec!["CF1".to_string()]);

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

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn track_pack_ext_config_routed_as_layer_not_skin() {
        // ext_config.ini voisin de skins/ dans un pack de skins de circuit :
        // amélioration du circuit lui-même, indépendante des skins actifs —
        // routé comme couche (extension/ext_config.ini), pas comme un skin de
        // plus (§4.6bis).
        let base = std::env::temp_dir().join(format!("pitbox-trkext-{}", Uuid::new_v4()));
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(ac.join("content").join("tracks").join("ks_black_cat_county")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();

        overlay::upsert_stock_mod(&conn, "ks_black_cat_county", "Track", None, Some("Black Cat County"), &now).unwrap();

        let pack = base.join("src").join("assettocorsa").join("content").join("tracks").join("ks_black_cat_county");
        let skin = pack.join("skins").join("cm_skins").join("Black Cat County CF1");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.png"), b"IMG").unwrap();
        std::fs::write(pack.join("ext_config.ini"), b"[BASIC]\n").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        import_subs(&conn, &cfg, &library, "black_cat_county_cf1.zip", &subs, true, ExtractionMode::InfoOnly);

        // ext_config.ini n'est pas devenu un skin de plus.
        let stored = overlay::list_subs_for_parent(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(stored.len(), 1, "un seul skin, ext_config.ini exclu");
        assert_eq!(stored[0].name, "Black Cat County CF1");

        // Routé comme couche…
        let layers = overlay::list_layers(&conn, "ks_black_cat_county").unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "ext_config");

        // …et composé à la vraie place attendue par CSP.
        let composed = ac
            .join("content")
            .join("tracks")
            .join("ks_black_cat_county")
            .join("extension")
            .join("ext_config.ini");
        assert!(composed.is_file(), "ext_config.ini devrait être composé dans extension/");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn skin_pack_multi_car_shape() {
        // Forme `skins/<voiture>/<skin>` : un pack couvrant plusieurs voitures.
        let base = std::env::temp_dir().join(format!("pitbox-subB-{}", Uuid::new_v4()));
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
        assert!(library.join("skins").join("ferrari_488").join("af_corse_51").join("preview.jpg").is_file());
        assert!(library.join("skins").join("lambo_huracan").join("team_a").join("preview.jpg").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sound_swap_and_restore() {
        let base = std::env::temp_dir().join(format!("pitbox-snd-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();

        // Voiture (mod) avec son d'origine dans sfx/.
        let carv = library.join("cars").join("snd_car").join("v1");
        let sfx = carv.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"ORIG").unwrap();
        std::fs::write(sfx.join("car.bank"), b"ORIGBANK").unwrap();
        overlay::upsert_mod(&conn, "snd_car", "Car", Some("B"), Some("Snd"), "h", None, &now).unwrap();
        overlay::insert_version(&conn, "v1", "snd_car", Some("1.0"), None, &now, &carv.to_string_lossy(), None, "sig", &[], &[], &[], &[], None).unwrap();
        overlay::set_active_version(&conn, "snd_car", "v1").unwrap();

        // Mod de son stocké à part.
        let snd = library.join("sounds").join("snd_car").join("v8");
        std::fs::create_dir_all(&snd).unwrap();
        std::fs::write(snd.join("GUIDs.txt"), b"MOD").unwrap();
        std::fs::write(snd.join("car.bank"), b"MODBANK").unwrap();
        overlay::insert_sub_mod(&conn, "s1", "SOUND", "snd_car", "v8", &snd.to_string_lossy(), None, &now).unwrap();

        // Activation : sfx remplacé, original sauvegardé, sub actif.
        activate_sound(&conn, &cfg, "s1").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "MOD");
        assert_eq!(std::fs::read_to_string(library.join("sounds").join("snd_car").join("__original__").join("GUIDs.txt")).unwrap(), "ORIG");
        assert!(overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);

        // Restauration : son d'origine revenu, sub inactif.
        restore_sound(&conn, &cfg, "snd_car").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "ORIG");
        assert!(!overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sound_parent_guessed_from_archive_name() {
        // Cas réel : dossier de son nommé comme l'archive, fichiers sous un
        // sous-dossier "sfx" (convention standard AC) — modscan ne peut pas en
        // déduire la voiture cible, seul le nom d'archive/dossier le peut.
        let base = std::env::temp_dir().join(format!("pitbox-sndguess-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "ks_lamborghini_huracan_performante", "Car", Some("Lamborghini"), Some("Huracan Performante"), "h", None, &now).unwrap();

        let archive_name = "Sound - ks_lamborghini_huracan_performante by Marti";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].parent_id, "sfx", "modscan seul retombe sur le nom générique du dossier");

        let res = import_subs(&conn, &cfg, &library, archive_name, &subs, true, ExtractionMode::InfoOnly);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].parent_id, "ks_lamborghini_huracan_performante", "voiture retrouvée dans le nom d'archive");
        assert_eq!(res[0].name, archive_name, "nom lisible, pas « sfx »");

        let _ = std::fs::remove_dir_all(&base);
    }
}
