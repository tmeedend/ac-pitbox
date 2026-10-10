//! Engine sound mods (§8.3): which car a sound targets, its import, and the
//! exclusive swap of the car's `sfx/` — the original is backed up once and
//! restored, never destroyed.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::{destination, gated, host_exists, parent_subdir, record_sub, SubGate, SubImported, SubProgress};
use crate::config::AppConfig;
use crate::modscan::FoundSub;
use crate::resources::{self, ExtractionMode};
use crate::{archive, overlay};

/// Tente d'identifier la voiture ciblée par un mod de son quand la détection
/// par arborescence (modscan) retombe sur le dossier générique "sfx" (nom de
/// dossier standard AC, `content/cars/<id>/sfx`, jamais un nom de voiture) :
/// l'id d'une voiture connue apparaît-il, seul, dans le nom d'archive/dossier
/// importé (souvent explicite, ex. « Sound - <id> by <auteur> ») ?
fn guess_sound_parent(conn: &Connection, source_name: &str) -> Option<String> {
    let lower = source_name.to_lowercase();
    let cars: Vec<String> = overlay::list_mods(conn)
        .ok()?
        .into_iter()
        .filter(|m| m.kind == "Car")
        .map(|m| m.id_interne)
        .collect();

    // 1. L'id complet d'une voiture apparaît tel quel dans le nom source.
    let mut direct = cars.iter().filter(|id| lower.contains(id.to_lowercase().as_str()));
    if let Some(first) = direct.next() {
        return direct.next().is_none().then(|| first.clone());
    }

    // 2. Repli : le nom source ne reprend souvent que le modèle, pas le
    //   préfixe marque de l'id (ex. « 312T_amafmod » pour un id du genre
    //   « ferrari_312t ») — un segment (séparé par « _ ») du nom source
    //   retrouvé tel quel comme segment de l'id suffit, à condition d'être
    //   assez long pour ne pas matcher au hasard (ex. pas juste "v2"/"by").
    let source_segments: Vec<&str> = lower.split('_').filter(|s| s.len() >= 3).collect();
    let mut fuzzy = cars.iter().filter(|id| {
        let id_lower = id.to_lowercase();
        id_lower.split('_').any(|seg| source_segments.contains(&seg))
    });
    let first = fuzzy.next()?;
    fuzzy.next().is_none().then(|| first.clone())
}

/// Le dossier nomme-t-il une voiture connue de la base ?
fn is_known_car(conn: &Connection, id: &str) -> bool {
    overlay::get_mod(conn, id)
        .ok()
        .flatten()
        .is_some_and(|m| m.kind == "Car")
}

/// Le nom d'un `.bank` sans son extension, s'il y en a un dans le dossier.
///
/// **AC nomme toujours le bank d'après la voiture** — `content/cars/<id>/sfx/<id>.bank` —
/// et c'est ainsi que le moteur audio le trouve. Vérifié sur les 296 voitures
/// du corpus de référence qui en ont un : **296 sur 296**. C'est donc le
/// signal le plus sûr dont on dispose pour savoir ce qu'un mod de son vise,
/// bien plus que le nom de l'archive.
fn bank_stem(dir: &Path) -> Option<String> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("bank")))
        .and_then(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
}

/// Un identifiant qui a la forme d'un id AC : un mot composé, pas un nom
/// générique. Écarte les `car.bank` / `sound.bank` que certains packs livrent,
/// dont le nom ne désigne rien.
fn looks_like_ac_id(name: &str) -> bool {
    name.len() >= 5 && name.contains('_')
}

/// À quelle voiture rattacher un mod de son.
///
/// `modscan` ne remonte jamais au-delà du dossier qui contient directement les
/// `.bank`/`GUIDs.txt` : son `parent_id` vaut donc presque toujours "sfx"
/// (convention AC) ou le nom du dossier importé, et **aucun des deux n'est un
/// identifiant**. Quatre sources, de la plus sûre à la plus faible :
///
/// 1. le dossier lui-même nomme une voiture connue — un pack de la forme
///    `<id>/` sans sous-dossier `sfx` ;
/// 2. le **dossier parent** en nomme une : c'est la forme canonique
///    `content/cars/<id>/sfx/`, où l'identifiant est écrit dans le chemin et
///    où il n'y a rien à deviner ;
/// 3. le **nom du bank**, quand il nomme une voiture connue (296/296 dans le
///    corpus, voir [`bank_stem`]) ;
/// 4. le nom de l'archive, par [`guess_sound_parent`].
///
/// Faute de tout cela, le nom du bank s'il a la forme d'un id AC — il désigne
/// la voiture que le mod vise, même si elle n'est pas installée, ce qui range
/// le mod au bon endroit pour le jour où elle le sera — et sinon le nom de
/// l'archive, faute de mieux.
///
/// **Bug réel** que les règles 2 et 3 corrigent : un mod livré en
/// `SCIBSOUND_Ford_GT40_1.1/content/cars/ks_ford_gt40/sfx/` atterrissait sous
/// « SCIBSOUND_Ford_GT40_1.1 ». La devinette sur le nom d'archive ne pouvait
/// pas aboutir — le nom ne contient pas `ks_ford_gt40`, seulement `ford`, qui
/// désigne tout autant les cinq Mustang de la bibliothèque, donc ambiguïté et
/// abandon. L'identifiant était pourtant écrit deux fois dans l'archive.
fn resolve_sound_parent(conn: &Connection, sub: &FoundSub, source_name: &str) -> String {
    if is_known_car(conn, &sub.parent_id) {
        return sub.parent_id.clone();
    }
    let parent_dir = sub
        .dir
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    if let Some(name) = parent_dir.filter(|n| is_known_car(conn, n)) {
        return name;
    }
    let stem = bank_stem(&sub.dir);
    if let Some(name) = stem.clone().filter(|n| is_known_car(conn, n)) {
        return name;
    }
    if let Some(guessed) = guess_sound_parent(conn, source_name) {
        return guessed;
    }
    stem.filter(|n| looks_like_ac_id(n))
        .unwrap_or_else(|| source_name.to_string())
}

/// Range en ressources les fichiers posés à côté du dossier de son.
///
/// Renvoie le nombre de fichiers rangés. Best-effort et journalisé : un fichier
/// annexe qu'on n'arrive pas à copier ne doit pas faire échouer l'import du son
/// lui-même, mais il ne doit pas non plus disparaître en silence.
///
/// En mode « Aucun », rien n'est copié — c'est la définition du mode (§11) : les
/// annexes restent dans la source.
fn sweep_pack_annexes(sub: &FoundSub, res_dir: &Path, mode: ExtractionMode) -> usize {
    if mode == ExtractionMode::None {
        return 0;
    }
    let Some(root) = sub.extra_root.as_deref() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut done = 0;
    for entry in entries.flatten() {
        let src = entry.path();
        if src.is_dir() {
            continue;
        }
        let Some(name) = src.file_name() else { continue };
        if std::fs::create_dir_all(res_dir).is_err() {
            continue;
        }
        match std::fs::copy(&src, res_dir.join(name)) {
            Ok(_) => done += 1,
            Err(e) => log::warn!("annexe de mod de son non rangée ({}): {e}", src.display()),
        }
    }
    done
}

#[allow(clippy::too_many_arguments)]
pub(super) fn import_sound(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    mode: ExtractionMode,
    gate: &SubGate,
    out: &mut Vec<SubImported>,
    progress: &mut SubProgress,
) {
    progress.step(&sub.parent_id);
    let parent = resolve_sound_parent(conn, sub, source_name);
    // Le **nom affiché** du mod, qui est une autre question que son parent : le
    // nom du dossier ne vaut que s'il dit quelque chose. "sfx" ne dit rien, et
    // un dossier qui porte l'id de la voiture non plus — deux mods de son pour
    // la même voiture s'appelleraient alors pareil. Dans ces deux cas le nom de
    // l'archive est le seul libellé lisible.
    let dir_name = sub
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = if dir_name.is_empty() || dir_name.eq_ignore_ascii_case("sfx") || dir_name == parent {
        source_name.to_string()
    } else {
        dir_name
    };

    // Déjà là et complet : rien à faire. En vitrine : il revient dans sa
    // propre ligne (ESPACE§7.5).
    let new_dest = library.join("sounds").join(&parent).join(&name);
    let Some((dest, freed)) = destination(conn, library, "SOUND", &parent, &name, new_dest) else {
        return;
    };

    // Voiture absente et pas encore tranché (§4.3bis) : rien n'est écrit, on
    // demande — même règle que pour une livrée et pour une couche sans sa base.
    if gated(conn, gate, &parent, &name) {
        out.push(SubImported {
            sub_type: "SOUND".into(),
            parent_id: parent,
            name,
            projected: false,
            warning: None,
            parent_known: false,
            awaiting_decision: true,
            resources_extracted: 0,
        });
        return;
    }

    // Fichiers annexes (§4.5.2) redirigés à part (GUIDs.txt reste toujours du
    // contenu, voir resources::classify — jamais confondu avec une annexe).
    let res_dir = resources::resources_dir_for(library, "sounds", &[&parent, &name]);
    let resources_extracted =
        match resources::file_mod(&sub.dir, &dest, &res_dir, mode, !copy, resources::Source::ModFolder) {
            Ok(n) => n,
            Err(err) => {
                out.push(SubImported {
                    sub_type: "SOUND".into(),
                    parent_known: host_exists(conn, &parent),
                    awaiting_decision: false,
                    parent_id: parent,
                    name,
                    projected: false,
                    warning: Some(format!("stockage : {err}")),
                    resources_extracted: 0,
                });
                return;
            }
        };

    // Ce qui était livré **à côté** du dossier de son lui appartient : notice,
    // changelog, capture. À ce niveau — un dossier qui ne contient qu'un `sfx/`
    // — un fichier isolé est de la documentation, AC n'y lit rien ; il part donc
    // en ressource sans tri par extension. Une ressource n'est jamais déployée,
    // donc rien ne peut atterrir dans le jeu par ce chemin.
    let resources_extracted = resources_extracted + sweep_pack_annexes(sub, &res_dir, mode);

    let stored = crate::libpath::to_relative(Some(library), &dest);
    record_sub(conn, freed.as_deref(), "SOUND", &parent, &name, &stored, source_name);
    out.push(SubImported {
        sub_type: "SOUND".into(),
        parent_known: host_exists(conn, &parent),
        awaiting_decision: false,
        parent_id: parent,
        name,
        projected: false,
        warning: None,
        resources_extracted,
    });
}

// --- Bascule exclusive du son (§8.3) ------------------------------------

/// Active un mod de son : remplace réellement le `sfx/` de la voiture par les
/// fichiers du mod (bascule exclusive). Le son d'origine est **sauvegardé une
/// fois** pour pouvoir y revenir — jamais détruit irréversiblement (§8.3).
pub fn activate_sound(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SOUND_NOT_FOUND)?;
    if sub.sub_type != "SOUND" {
        return Err(crate::errors::NOT_A_SOUND_MOD.into());
    }
    // A sound in the showcase has no bank left to lay (ESPACE§5.4).
    if sub.is_skeleton() {
        return Err(crate::errors::CONTENT_FREED.into());
    }
    // The car's own `sfx/` is gone with its files (ESPACE R5): nothing to
    // back up, nothing to switch.
    crate::skeleton::guard_mod(conn, &sub.parent_id)?;
    let sfx = parent_subdir(conn, cfg, &sub.parent_id, "sfx").ok_or(crate::errors::TARGET_CAR_UNKNOWN)?;
    let backup = sound_backup_dir(cfg, &sub.parent_id)?;

    // Sauvegarde du son d'origine, une seule fois (préserve le vrai original).
    if !backup.exists() {
        std::fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        if sfx.is_dir() {
            archive::copy_dir(&sfx, &backup).map_err(|e| format!("sauvegarde du son d'origine : {e}"))?;
        }
    }

    let sound_dir = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    replace_dir_contents(&sound_dir, &sfx)?;
    overlay::set_active_sound(conn, &sub.parent_id, Some(sub_id)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Restaure le son d'origine d'une voiture (désactive le mod de son actif).
pub fn restore_sound(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Result<(), String> {
    let backup = sound_backup_dir(cfg, parent_id)?;
    if backup.is_dir() {
        let sfx = parent_subdir(conn, cfg, parent_id, "sfx").ok_or(crate::errors::TARGET_CAR_UNKNOWN)?;
        replace_dir_contents(&backup, &sfx)?;
    }
    overlay::set_active_sound(conn, parent_id, None).map_err(|e| e.to_string())?;
    Ok(())
}

/// `<lib>/sounds/<parent>/__original__` : sauvegarde du son d'origine.
/// `pub(crate)` pour `enginesound` : écouter « Origine » doit lire le vrai
/// original, qui est ici dès qu'un mod a été activé une fois — et non le `sfx/`
/// du jeu, qui contient alors le mod.
pub(crate) fn sound_backup_dir(cfg: &AppConfig, parent_id: &str) -> Result<PathBuf, String> {
    let lib = cfg.library_path.as_ref().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
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
    use crate::submods::import_subs;
    use chrono::Local;
    #[test]
    fn sound_swap_and_restore() {
        let base = crate::testutil::temp_dir("snd");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        // Voiture (mod) avec son d'origine dans sfx/.
        let carv = library.join("cars").join("snd_car").join("v1");
        let sfx = carv.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"ORIG").unwrap();
        std::fs::write(sfx.join("car.bank"), b"ORIGBANK").unwrap();
        overlay::upsert_mod(&conn, "snd_car", "Car", Some("B"), Some("Snd"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            "snd_car",
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
        overlay::set_active_version(&conn, "snd_car", "v1").unwrap();

        // Mod de son stocké à part.
        let snd = library.join("sounds").join("snd_car").join("v8");
        std::fs::create_dir_all(&snd).unwrap();
        std::fs::write(snd.join("GUIDs.txt"), b"MOD").unwrap();
        std::fs::write(snd.join("car.bank"), b"MODBANK").unwrap();
        overlay::insert_sub_mod(
            &conn,
            "s1",
            "SOUND",
            "snd_car",
            "v8",
            &snd.to_string_lossy(),
            None,
            &now,
        )
        .unwrap();

        // Activation : sfx remplacé, original sauvegardé, sub actif.
        activate_sound(&conn, &cfg, "s1").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "MOD");
        assert_eq!(
            std::fs::read_to_string(
                library
                    .join("sounds")
                    .join("snd_car")
                    .join("__original__")
                    .join("GUIDs.txt")
            )
            .unwrap(),
            "ORIG"
        );
        assert!(overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);

        // Restauration : son d'origine revenu, sub inactif.
        restore_sound(&conn, &cfg, "snd_car").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "ORIG");
        assert!(!overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);
    }

    /// Fabrique un décor : bibliothèque, base, et les voitures données.
    fn sound_fixture(tag: &str, cars: &[&str]) -> (crate::testutil::TempDir, PathBuf, Connection, AppConfig) {
        let base = crate::testutil::temp_dir(tag);
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();
        for car in cars {
            overlay::upsert_mod(&conn, car, "Car", None, Some(car), "h", None, &now).unwrap();
        }
        (base, library, conn, cfg)
    }

    /// Les cinq Ford de la bibliothèque de l'utilisateur : c'est leur présence
    /// qui rendait la devinette par nom d'archive ambiguë, donc inopérante.
    const FORDS: &[&str] = &[
        "ks_ford_gt40",
        "ford_mustang_boss_302",
        "ford_mustang_boss_429",
        "ks_ford_escort_mk1",
        "ks_ford_mustang_2015",
    ];

    /// Bug réel (`SCIBSOUND_Ford_GT40_1.1`) : l'archive livre l'arborescence de
    /// jeu complète, donc l'identifiant est **écrit dans le chemin**, et le mod
    /// atterrissait quand même sous le nom de l'archive. La devinette sur ce nom
    /// ne pouvait pas trancher — « Ford » désigne cinq voitures ici.
    #[test]
    fn sound_parent_read_from_the_game_path() {
        let (base, library, conn, cfg) = sound_fixture("sndpath", FORDS);
        let archive_name = "SCIBSOUND_Ford_GT40_1.1";
        let sfx = base
            .join("src")
            .join(archive_name)
            .join("content")
            .join("cars")
            .join("ks_ford_gt40")
            .join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&base.join("src").join(archive_name));
        assert_eq!(subs.len(), 1, "un seul son détecté");
        assert_eq!(subs[0].parent_id, "sfx", "modscan seul ne voit que le nom du dossier");

        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, "ks_ford_gt40",
            "la voiture est lue dans le chemin, pas devinée"
        );
        assert_eq!(res[0].name, archive_name, "nom lisible, pas « sfx »");
    }

    /// Bug réel (`FordGT40_SoundmodV097_AmplifiedNL`) : rien dans le chemin, un
    /// simple `sfx/` à la racine du pack. Mais AC nomme toujours le bank d'après
    /// la voiture — 296 sur 296 dans le corpus — et c'est ce qui tranche.
    #[test]
    fn sound_parent_read_from_the_bank_filename() {
        let (base, library, conn, cfg) = sound_fixture("sndbank", FORDS);
        let archive_name = "FordGT40_SoundmodV097_AmplifiedNL";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, "ks_ford_gt40",
            "la voiture est lue dans le nom du bank"
        );
    }

    /// Un bank au nom générique ne doit **pas** l'emporter : « car » ne désigne
    /// rien, et la devinette sur le nom d'archive reste alors le bon recours.
    /// C'est exactement la forme du test `sound_parent_guessed_from_archive_name`
    /// juste au-dessus, qui ne doit pas régresser.
    #[test]
    fn generic_bank_name_never_wins_over_the_archive_name() {
        let (base, library, conn, cfg) = sound_fixture("sndgeneric", &["ks_lamborghini_huracan_performante"]);
        let archive_name = "Sound - ks_lamborghini_huracan_performante by Marti";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(
            res[0].parent_id, "ks_lamborghini_huracan_performante",
            "« car » ne nomme rien, le nom d'archive reprend la main"
        );
    }

    /// Une voiture que l'utilisateur n'a pas installée : le mod se range quand
    /// même sous l'identifiant que le bank vise, prêt pour le jour où elle le
    /// sera, plutôt que sous un nom d'archive qui ne se rattachera jamais.
    #[test]
    fn sound_for_an_absent_car_is_filed_under_the_id_its_bank_targets() {
        let (base, library, conn, cfg) = sound_fixture("sndabsent", &[]);
        let archive_name = "SomeSoundPack v3";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res[0].parent_id, "ks_ford_gt40", "l'id visé par le bank");
    }

    /// Bug réel : le `ReadMe.txt` livré à côté du `sfx/` n'avait aucun
    /// propriétaire possible et devenait un « autre mod » à lui tout seul, dont
    /// l'unique fichier partait ensuite en ressources. Il appartient au son.
    #[test]
    fn a_file_beside_the_sound_folder_becomes_its_resource() {
        let (base, library, conn, cfg) = sound_fixture("sndannexe", FORDS);
        let archive_name = "FordGT40_SoundmodV097_AmplifiedNL";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();
        std::fs::write(src.join("ReadMe.txt"), b"notice").unwrap();

        let subs = modscan::scan_subs(&src);
        assert_eq!(
            subs[0].extra_root.as_deref(),
            Some(src.as_path()),
            "le dossier du pack est déclaré, donc consommé : plus d'« autre mod »"
        );

        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res[0].resources_extracted, 1, "la notice est comptée");
        let stored =
            resources::resources_dir_for(&library, "sounds", &["ks_ford_gt40", archive_name]).join("ReadMe.txt");
        assert!(stored.is_file(), "la notice est rangée dans les ressources du son");
    }

    /// Le garde-fou : sous `content/cars/<id>/`, les voisins du `sfx/` sont le
    /// contenu de la voiture. Les avaler comme annexes du son sortirait des
    /// fichiers du mod — très exactement la règle d'or n°3.
    #[test]
    fn car_content_beside_sfx_is_never_taken_for_a_sound_annexe() {
        let (base, _library, _conn, _cfg) = sound_fixture("sndguard", FORDS);
        let car = base
            .join("src")
            .join("SCIBSOUND_Ford_GT40_1.1")
            .join("content")
            .join("cars")
            .join("ks_ford_gt40");
        let sfx = car.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();
        std::fs::write(car.join("data.acd"), b"contenu voiture").unwrap();

        let subs = modscan::scan_subs(&base.join("src").join("SCIBSOUND_Ford_GT40_1.1"));
        assert_eq!(subs.len(), 1, "un seul son");
        assert!(
            subs[0].extra_root.is_none(),
            "sous content/cars/<id>/, rien n'est une annexe du son"
        );
    }

    /// Un dossier qui livre autre chose à côté du son n'est pas son emballage :
    /// rien ne dit alors que ce qui l'entoure revient au son.
    #[test]
    fn a_pack_with_another_folder_is_not_a_sound_wrapper() {
        let (base, _library, _conn, _cfg) = sound_fixture("sndmixed", FORDS);
        let src = base.join("src").join("MixedPack");
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("ks_ford_gt40.bank"), b"Y").unwrap();
        std::fs::create_dir_all(src.join("autre_chose")).unwrap();

        let subs = modscan::scan_subs(&src);
        let sound = subs.iter().find(|s| matches!(s.kind, modscan::SubKind::Sound)).unwrap();
        assert!(sound.extra_root.is_none(), "un second dossier annule l'emballage");
    }

    #[test]
    fn sound_parent_guessed_from_archive_name() {
        // Cas réel : dossier de son nommé comme l'archive, fichiers sous un
        // sous-dossier "sfx" (convention standard AC) — modscan ne peut pas en
        // déduire la voiture cible, seul le nom d'archive/dossier le peut.
        let base = crate::testutil::temp_dir("sndguess");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(
            &conn,
            "ks_lamborghini_huracan_performante",
            "Car",
            Some("Lamborghini"),
            Some("Huracan Performante"),
            "h",
            None,
            &now,
        )
        .unwrap();

        let archive_name = "Sound - ks_lamborghini_huracan_performante by Marti";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        assert_eq!(subs.len(), 1);
        assert_eq!(
            subs[0].parent_id, "sfx",
            "modscan seul retombe sur le nom générique du dossier"
        );

        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, "ks_lamborghini_huracan_performante",
            "voiture retrouvée dans le nom d'archive"
        );
        assert_eq!(res[0].name, archive_name, "nom lisible, pas « sfx »");
    }

    #[test]
    fn sound_parent_guessed_when_bank_files_sit_directly_in_the_dropped_folder() {
        // Bug réel : certains packs posent GUIDs.txt/.bank directement dans le
        // dossier importé, sans sous-dossier sfx/ intermédiaire. `modscan`
        // retombe alors sur le nom de CE dossier comme `parent_id` (ici
        // « 312T_amafmod ») — aussi peu identifiant que "sfx", mais différent
        // de "sfx" au sens strict, donc pas rattrapé par un simple test
        // littéral sur "sfx". Doit quand même déclencher la devinette.
        let base = crate::testutil::temp_dir("sndguess-nosfx");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(
            &conn,
            "ferrari_312t",
            "Car",
            Some("Ferrari"),
            Some("312T"),
            "h",
            None,
            &now,
        )
        .unwrap();

        let archive_name = "312T_amafmod";
        let src = base.join("src").join(archive_name);
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(src.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        assert_eq!(subs.len(), 1);
        assert_eq!(
            subs[0].parent_id, archive_name,
            "modscan retombe sur le nom du dossier lui-même, pas « sfx »"
        );

        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, "ferrari_312t",
            "voiture retrouvée malgré un parent_id qui n'est ni « sfx » ni un id connu"
        );
    }

    #[test]
    fn sound_parent_guessed_from_model_segment_without_brand_prefix() {
        // Cas réel rapporté : le dossier de son ne reprend que le modèle
        // (« 312T_amafmod »), pas le préfixe marque de l'id de la voiture
        // (« rss_formula_1970s_312t ») — la correspondance directe (id complet
        // inclus tel quel dans le nom) échoue, il faut comparer par segment.
        let base = crate::testutil::temp_dir("sndguess-segment");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(
            &conn,
            "rss_formula_1970s_312t",
            "Car",
            Some("RSS"),
            Some("Formula 1970s 312T"),
            "h",
            None,
            &now,
        )
        .unwrap();

        let archive_name = "312T_amafmod";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, "rss_formula_1970s_312t",
            "voiture retrouvée par segment de modèle, sans le préfixe marque"
        );
    }

    #[test]
    fn sound_parent_not_guessed_when_segment_ambiguous_between_two_cars() {
        // Deux voitures partagent un segment de modèle (ex. deux livrées/eras
        // du même châssis importées séparément) : mieux vaut ne rien deviner
        // que de rattacher au hasard à l'une des deux.
        let base = crate::testutil::temp_dir("sndguess-ambiguous");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let now = Local::now().to_rfc3339();

        overlay::upsert_mod(&conn, "team_a_312t", "Car", None, Some("A 312T"), "h", None, &now).unwrap();
        overlay::upsert_mod(&conn, "team_b_312t", "Car", None, Some("B 312T"), "h", None, &now).unwrap();

        let archive_name = "312T_amafmod";
        let src = base.join("src").join(archive_name);
        let sfx = src.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"X").unwrap();
        std::fs::write(sfx.join("car.bank"), b"Y").unwrap();

        let subs = modscan::scan_subs(&src);
        let res = import_subs(
            &conn,
            &cfg,
            &library,
            archive_name,
            &subs,
            true,
            ExtractionMode::InfoOnly,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].parent_id, archive_name,
            "ambigu : nom d'archive brut, pas de choix au hasard"
        );
    }
}
