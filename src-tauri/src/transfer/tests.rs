//! Tests of the library transfer (EXPORT§8.4): two installations on a real
//! file system, one exported, the other imported into.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;
use crate::importer::ImportedMod;
use crate::usermeta::{set_display_name, set_note, EntityKind};

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn photo(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_fn(1022, 575, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]))
        .save(path)
        .unwrap();
}

/// A car as a mod author ships it: ui, physics, model, sound, one skin.
fn car(dir: &Path, name: &str) {
    write(
        dir,
        "ui/ui_car.json",
        format!(r#"{{"name":"{name}","brand":"RSS","specs":{{"bhp":"600bhp","weight":"1200kg"}}}}"#).as_bytes(),
    );
    write(dir, "ui/badge.png", b"badge");
    write(dir, "data/engine.ini", b"[ENGINE_DATA]\nLIMITER=8300\n");
    write(dir, "data/drivetrain.ini", b"[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=6\n");
    write(dir, "model.kn5", &[7; 5000]);
    write(dir, "data.acd", &[3; 700]);
    write(dir, "sfx/car.bank", &[1; 3000]);
    photo(&dir.join("skins/red/preview.jpg"));
}

/// One installation: a game folder, a library, a configuration folder and
/// Content Manager's presets — each in its own place, as on a real machine.
struct Install {
    root: PathBuf,
    db: Db,
    cfg: AppConfig,
    places: Places,
}

impl Install {
    /// `stock`: the game's own cars in `content/`, indexed as a first start
    /// does.
    fn new(root: &Path, name: &str, stock: &[&str]) -> Self {
        let root = root.join(name);
        let ac = root.join("ac");
        std::fs::create_dir_all(ac.join("content/cars")).unwrap();
        std::fs::create_dir_all(ac.join("content/tracks")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::create_dir_all(root.join("presets")).unwrap();
        for id in stock {
            write(
                &ac.join("content/cars").join(id),
                "ui/ui_car.json",
                format!(r#"{{"name":"{id}","brand":"Kunos"}}"#).as_bytes(),
            );
        }
        let cfg = AppConfig {
            ac_install_path: Some(ac),
            library_path: Some(root.join("lib")),
            ..Default::default()
        };
        let conn = overlay::open(&root.join("config/overlay.sqlite")).unwrap();
        crate::stock::index_stock_content(&conn, &cfg, &crate::rules::default_rules(), false).unwrap();
        let places = Places {
            config_dir: root.join("config"),
            presets_dir: Some(root.join("presets")),
            app_version: "0.8.0".into(),
        };
        Self {
            root,
            db: Db(Mutex::new(conn)),
            cfg,
            places,
        }
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.0.lock().unwrap()
    }

    fn import_folder(&self, src: &Path) -> Vec<ImportedMod> {
        let ctx = crate::import_progress::ImportCtx::silent();
        let paths = [src.to_string_lossy().into_owned()];
        crate::importer::import_folders(
            &ctx,
            &self.db,
            &self.cfg,
            &crate::rules::default_rules(),
            &paths,
            true,
            &[],
        )
        .unwrap()
        .into_iter()
        .flat_map(|r| r.mods)
        .collect()
    }

    fn library(&self) -> &Path {
        self.cfg.library_path.as_deref().unwrap()
    }
}

/// The installation everything is exported from: a managed car with a layer,
/// game content annotated (one car the other machine has, one it has not), a
/// mod installed outside Pit Box, an app and a mannequin renamed and
/// annotated, a profile, and settings of every part.
fn source(root: &Path) -> Install {
    let src = Install::new(root, "source", &["ks_mazda_mx5_cup", "ks_ferrari_f2004"]);
    car(&src.root.join("in/lanzo"), "Lanzo");
    // A second livery, plain blue, first in alphabetical order.
    let blue = src.root.join("in/lanzo/skins/blue/preview.jpg");
    std::fs::create_dir_all(blue.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(1022, 575, image::Rgb([20, 40, 230]))
        .save(&blue)
        .unwrap();
    assert_eq!(src.import_folder(&src.root.join("in"))[0].outcome, "IMPORT");
    // A layer: the car's folder without a model.
    write(&src.root.join("hd/lanzo"), "ui/ui_car.json", br#"{"name":"Lanzo HD"}"#);
    write(&src.root.join("hd/lanzo"), "texture_hd/body.dds", &[5; 2000]);
    assert_eq!(src.import_folder(&src.root.join("hd"))[0].outcome, "EXTENSION");
    // Installed by hand before Pit Box: a real folder of `content/`.
    car(
        &src.cfg
            .ac_install_path
            .as_ref()
            .unwrap()
            .join("content/cars/rss_hybrid"),
        "Hybrid",
    );
    {
        let conn = src.conn();
        crate::stock::index_stock_content(&conn, &src.cfg, &crate::rules::default_rules(), false).unwrap();
        set_display_name(&conn, EntityKind::Mod, "lanzo", Some("Lanzo V10")).unwrap();
        set_note(&conn, EntityKind::Mod, "lanzo", Some("brakes early")).unwrap();
        set_note(&conn, EntityKind::Mod, "ks_mazda_mx5_cup", Some("cup note")).unwrap();
        set_note(&conn, EntityKind::Mod, "ks_ferrari_f2004", Some("DLC note")).unwrap();

        let app = src.root.join("appsrc/apps/lua/Timer");
        write(&app, "Timer.lua", b"-- app");
        let found = crate::modscan::scan_apps(&src.root.join("appsrc"));
        crate::apps::import_apps(
            &conn,
            src.library(),
            "timer.7z",
            &found,
            true,
            crate::resources::ExtractionMode::InfoOnly,
        );
        set_display_name(&conn, EntityKind::App, "Timer", Some("Lap timer")).unwrap();
        set_note(&conn, EntityKind::App, "Timer", Some("keep it")).unwrap();

        write(&src.root.join("dolls"), "content/driver/ada.kn5", &[9; 400]);
        crate::others::import_other(
            &conn,
            src.library(),
            "Dolls.7z",
            &src.root.join("dolls"),
            true,
            crate::resources::ExtractionMode::InfoOnly,
        )
        .unwrap();
        set_note(&conn, EntityKind::Other, "Dolls", Some("the good one")).unwrap();

        overlay::create_profile(&conn, "p1", "Evening", "2026-10-09").unwrap();
        overlay::add_profile_extra_entry(&conn, "p1", "app", "Timer").unwrap();
    }
    write_settings(&src);
    src
}

/// A file of every part's settings, `config.json` naming this machine's paths.
fn write_settings(src: &Install) {
    let config = &src.places.config_dir;
    write(
        config,
        "config.json",
        serde_json::to_string(&serde_json::json!({
            "ac_install_path": src.cfg.ac_install_path,
            "library_path": src.cfg.library_path,
            "prefs": { "language": "it" }
        }))
        .unwrap()
        .as_bytes(),
    );
    // The preferred livery, as the screens store it: a JSON object serialized
    // into a string, its preview an absolute path of this machine. Not the
    // first one: `blue` comes before it, and is what the backend alone picks.
    let version = overlay::get_versions(&src.conn(), "lanzo").unwrap().remove(0);
    let red = crate::libpath::resolve(src.cfg.library_path.as_deref(), &version.library_path)
        .unwrap()
        .join("skins/red/preview.jpg");
    let preferred = serde_json::json!({ "id": "red", "name": "Red", "preview": red }).to_string();
    write(
        config,
        "ui_prefs.json",
        serde_json::json!({ "pitbox.nav.section": "tracks", "pitbox.skin.lanzo": preferred })
            .to_string()
            .as_bytes(),
    );
    write(config, "saved_grids.json", br#"{"GT3 night":{"cars":["lanzo"]}}"#);
    write(config, "taxonomy.json", br#"{"brands":{}}"#);
    write(config, "logos/rss-1234abcd.png", b"png");
    write(
        config,
        "music.json",
        format!(
            r#"{{"enabled":true,"menu_folder":"{}"}}"#,
            config.join("music").display()
        )
        .replace('\\', "\\\\")
        .as_bytes(),
    );
    // A Content Manager preset names its other presets by absolute path, at
    // the top and inside the JSON it stores as strings.
    let cm = config.join("CM/Presets/Assists/Pro.cmpreset");
    let mode = serde_json::json!({ "Laps": 7, "Grid": cm }).to_string();
    write(
        src.places.presets_dir.as_ref().unwrap(),
        "Spa dusk.cmpreset",
        serde_json::json!({ "AssistsPresetFilename": cm, "AssistsData": "{\"Abs\":1}", "ModeData": mode })
            .to_string()
            .as_bytes(),
    );
}

/// Whether `bytes` hold an absolute Windows path — `C:\`, `C:/`, at any
/// depth of escaping (`C:\\\\`): a drive letter that does not end a word,
/// then a colon and a separator. `http://` does not match, its letter ends
/// a word.
fn names_a_path(bytes: &[u8]) -> bool {
    bytes.windows(3).enumerate().any(|(i, w)| {
        w[0].is_ascii_alphabetic()
            && w[1] == b':'
            && (w[2] == b'\\' || w[2] == b'/')
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
    })
}

fn entries(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut e = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.name().to_string(), bytes)
        })
        .collect()
}

/// Rule (EXPORT§8.3): every table of the base is classified. A table added
/// without a thought for the export would leave with all its content, or be
/// lost in silence.
#[test]
fn every_table_of_the_base_is_classified() {
    let base = crate::testutil::temp_dir("transfer-gate");
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let known: Vec<&str> = tables::TABLES.iter().map(|(t, _, _)| *t).collect();
    for t in &tables {
        assert!(
            known.contains(&t.as_str()),
            "table {t} is not classified in transfer::tables::TABLES (EXPORT§8.3)"
        );
    }
    for t in &known {
        assert!(
            tables.iter().any(|x| x == t),
            "TABLES names {t}, which the base no longer has"
        );
    }
}

/// The round trip of EXPORT§8.4, done once per test: the source exported
/// whole, then imported into another installation that has the cup car and
/// not the DLC, a preset of the same name already, and its own paths.
struct Transferred {
    _root: crate::testutil::TempDir,
    dest: Install,
    file: PathBuf,
    exported: ExportReport,
    imported: ImportReport,
    sheet_before: crate::techsheet::TechSheet,
}

fn transferred(tag: &str) -> Transferred {
    let root = crate::testutil::temp_dir(tag);
    let src = source(&root);
    let sheet_before = crate::techsheet::effective(&src.conn(), "lanzo").unwrap();
    let file = root.join("library.pitbox");
    let exported = export(&src.db, &src.cfg, &src.places, &Part::ALL, &file).unwrap();

    let dest = Install::new(&root, "dest", &["ks_mazda_mx5_cup"]);
    write(dest.places.presets_dir.as_ref().unwrap(), "Spa dusk.cmpreset", b"mine");
    write(
        &dest.places.config_dir,
        "config.json",
        serde_json::to_string(&serde_json::json!({ "library_path": dest.cfg.library_path, "prefs": {} }))
            .unwrap()
            .as_bytes(),
    );
    let inspection = inspect(&dest.conn(), &dest.cfg, &dest.places, &file).unwrap();
    assert_eq!(inspection.refusal, None, "an empty installation takes it");
    let imported = import(
        &dest.db,
        &dest.cfg,
        &dest.places,
        &crate::rules::default_rules(),
        &file,
        &Part::ALL,
    )
    .unwrap();
    Transferred {
        _root: root,
        dest,
        file,
        exported,
        imported,
        sheet_before,
    }
}

/// Rule (EXPORT R1, R2): the export holds no playable file and no path of
/// the machine it was made on — the skeletons and nothing heavier.
#[test]
fn the_export_holds_no_playable_file_and_no_path_of_the_machine() {
    let t = transferred("transfer-zip");
    let counts = &t.exported.counts;
    assert_eq!(counts.cars, 2, "the managed car and the one installed by hand");
    assert_eq!(counts.unmanaged, 1);
    assert_eq!(counts.stock_with_user_data, 2);
    assert_eq!((counts.apps, counts.others, counts.layers), (1, 1, 1));
    assert_eq!((counts.sessions, counts.grids), (1, 1));

    let zipped = entries(&t.file);
    for (name, _) in &zipped {
        let lower = name.to_lowercase();
        assert!(
            ![".kn5", ".acd", ".bank", ".dds"].iter().any(|e| lower.ends_with(e)),
            "no heavy file leaves: {name}"
        );
    }
    // Any absolute path at all, at any depth of escaping: the preferred
    // livery's preview in `ui_prefs.json` is a JSON string inside a JSON
    // string, and a search for the machine's paths as such missed it on the
    // real library.
    for (name, bytes) in &zipped {
        let lower = name.to_lowercase();
        if lower.ends_with(".png") || lower.ends_with(".jpg") {
            continue;
        }
        assert!(
            !names_a_path(bytes),
            "{name} names a path of the machine: {}",
            String::from_utf8_lossy(bytes)
        );
    }
    let presets: Vec<&(String, Vec<u8>)> = zipped.iter().filter(|(n, _)| n.ends_with(".cmpreset")).collect();
    let preset: serde_json::Value = serde_json::from_slice(&presets[0].1).unwrap();
    assert_eq!(
        preset["AssistsData"], "{\"Abs\":1}",
        "what is not a path stays as it was"
    );
    assert!(
        zipped.iter().any(|(n, _)| n.ends_with("ui/ui_car.json")),
        "the skeletons leave"
    );
}

/// Rule (EXPORT§4, §8.4): what the user entered and decided comes back on the
/// other machine, every mod in the showcase with its skeleton, and the tech
/// sheet without reading a single file (FICHE§9.4).
#[test]
fn what_the_user_entered_comes_back_and_every_mod_is_in_the_showcase() {
    let t = transferred("transfer-mods");
    assert_eq!(t.imported.mods, 2);
    let conn = t.dest.conn();
    let lanzo = overlay::get_mod(&conn, "lanzo").unwrap().expect("the managed car");
    assert!(lanzo.showcase, "every mod arrives in the showcase");
    assert_eq!(lanzo.display_name_user.as_deref(), Some("Lanzo V10"));
    assert_eq!(lanzo.notes_user.as_deref(), Some("brakes early"));
    assert_eq!(
        crate::techsheet::effective(&conn, "lanzo").unwrap(),
        t.sheet_before,
        "the tech sheet comes back without reading a single file"
    );
    let version = overlay::get_versions(&conn, "lanzo").unwrap().remove(0);
    let folder = crate::libpath::resolve(t.dest.cfg.library_path.as_deref(), &version.library_path).unwrap();
    assert!(
        folder.join("ui/ui_car.json").is_file(),
        "its skeleton is in the library"
    );
    assert!(!folder.join("model.kn5").exists());
    let manifest = crate::skeleton::read_manifest(&folder).expect("it knows what it lacks");
    assert!(manifest.removed.iter().any(|f| f.path == "model.kn5"));
    let frozen = crate::skeleton::image_of(&folder).expect("with its frozen image");
    let center = image::open(&frozen).unwrap().to_rgb8();
    let px = center.get_pixel(center.width() / 2, center.height() / 2);
    assert!(
        px[0] > 80 && px[2] < 150,
        "the livery the card shows, the preferred red one, not the first blue one: {px:?}"
    );
    assert!(
        crate::skeleton::guard_mod(&conn, "lanzo").is_err(),
        "it cannot go into the game"
    );
    let layer = overlay::list_layers(&conn, "lanzo", crate::layers::HostKind::Car)
        .unwrap()
        .remove(0);
    assert!(layer.is_skeleton(), "its layer too");

    let hybrid = overlay::get_mod(&conn, "rss_hybrid")
        .unwrap()
        .expect("the car installed by hand");
    assert!(
        !hybrid.is_stock && !hybrid.is_unmanaged,
        "it arrives managed (EXPORT§4.2)"
    );
    assert!(hybrid.showcase);

    let app = overlay::get_app(&conn, "Timer").unwrap().expect("the app");
    assert!(app.is_skeleton());
    assert_eq!(app.display_name_user.as_deref(), Some("Lap timer"));
    assert_eq!(app.notes_user.as_deref(), Some("keep it"));
    let other = overlay::get_other_mod(&conn, "Dolls").unwrap().expect("the mannequin");
    assert!(other.is_skeleton());
    assert_eq!(other.notes_user.as_deref(), Some("the good one"));
    let (rows, index) = crate::others::others_base(&conn).unwrap();
    let card = crate::others::others_from_disk(&t.dest.cfg, rows, &index).remove(0);
    assert_eq!(
        card.categories,
        vec!["driver"],
        "still a mannequin, read from its manifest"
    );
    assert_eq!(
        overlay::get_profile_extra_entries(&conn, "p1").unwrap().len(),
        1,
        "the profile"
    );
}

/// Rule (EXPORT§4.2): the notes on the game's own content are laid on the
/// same content of the other machine; those on content it does not have (a
/// DLC) are named, never lost in silence.
#[test]
fn notes_on_game_content_land_where_it_exists_and_the_rest_is_named() {
    let t = transferred("transfer-stock");
    assert_eq!(t.imported.stock_applied, 1, "the note on the cup car is laid on it");
    assert_eq!(
        t.imported.stock_missing,
        vec!["ks_ferrari_f2004"],
        "the DLC note is named"
    );
    let cup = overlay::get_mod(&t.dest.conn(), "ks_mazda_mx5_cup").unwrap().unwrap();
    assert_eq!(cup.notes_user.as_deref(), Some("cup note"));
    assert!(cup.is_stock, "game content stays game content");
}

/// Rule (EXPORT§5.2, §7.3): the settings come back into this machine's
/// folders without moving its paths, a preset of the same name is never
/// overwritten, and the next start harmonizes again.
#[test]
fn the_settings_come_back_and_this_machines_paths_stay() {
    let t = transferred("transfer-settings");
    let config = &t.dest.places.config_dir;
    let merged: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config.join("config.json")).unwrap()).unwrap();
    assert_eq!(merged["prefs"]["language"], "it", "the preferences come");
    assert_eq!(
        merged["library_path"],
        serde_json::json!(t.dest.cfg.library_path),
        "this machine's paths stay"
    );
    for file in [
        "ui_prefs.json",
        "saved_grids.json",
        "taxonomy.json",
        "logos/rss-1234abcd.png",
    ] {
        assert!(config.join(file).is_file(), "{file} came");
    }
    let music: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config.join("music.json")).unwrap()).unwrap();
    assert!(
        music["menu_folder"].is_null(),
        "the music folder of the other machine stays there"
    );
    let presets = t.dest.places.presets_dir.as_ref().unwrap();
    assert_eq!(
        std::fs::read(presets.join("Spa dusk.cmpreset")).unwrap(),
        b"mine",
        "never overwritten"
    );
    assert!(presets.join("Spa dusk (2).cmpreset").is_file());
    assert_eq!(t.imported.presets_renamed, 1);
    assert_eq!(
        overlay::get_meta(&t.dest.conn(), overlay::META_ENGINE_VERSION).unwrap(),
        None,
        "the next start harmonizes again"
    );
}

/// Rule (EXPORT§4.2, §8.4): a mod installed outside Pit Box, exported then
/// imported, comes back managed when its archive is imported — rehydrated,
/// not a duplicate.
#[test]
fn a_mod_installed_by_hand_is_recovered_by_importing_its_archive() {
    let root = crate::testutil::temp_dir("transfer-unmanaged");
    let src = Install::new(&root, "source", &[]);
    let original = src
        .cfg
        .ac_install_path
        .as_ref()
        .unwrap()
        .join("content/cars/rss_hybrid");
    car(&original, "Hybrid");
    crate::stock::index_stock_content(&src.conn(), &src.cfg, &crate::rules::default_rules(), false).unwrap();
    let file = root.join("library.pitbox");
    export(&src.db, &src.cfg, &src.places, &[Part::Library], &file).unwrap();
    assert!(original.join("model.kn5").is_file(), "its game folder was only read");

    let dest = Install::new(&root, "dest", &[]);
    import(
        &dest.db,
        &dest.cfg,
        &dest.places,
        &crate::rules::default_rules(),
        &file,
        &[Part::Library],
    )
    .unwrap();
    car(&root.join("archive/rss_hybrid"), "Hybrid");
    let out = dest.import_folder(&root.join("archive"));
    assert_eq!(
        out[0].outcome, "REHYDRATED",
        "the archive recognises the version in the showcase"
    );
    assert!(!skeleton_of(&dest, "rss_hybrid"), "and it has its files again");
}

/// Rule (EXPORT§4.2): a mod the other machine already has, installed by hand
/// in its `content/`, stays what it is there — a real folder, its own row —
/// and gets the notes written on it. The export's version, layers and
/// skeleton would describe a mod in the showcase whose real folder sits in
/// the game, and recovering it would then run into that folder.
#[test]
fn a_mod_installed_by_hand_on_both_sides_keeps_its_local_row() {
    let root = crate::testutil::temp_dir("transfer-both");
    let src = Install::new(&root, "source", &[]);
    car(
        &src.cfg
            .ac_install_path
            .as_ref()
            .unwrap()
            .join("content/cars/rss_hybrid"),
        "Hybrid",
    );
    crate::stock::index_stock_content(&src.conn(), &src.cfg, &crate::rules::default_rules(), false).unwrap();
    set_note(&src.conn(), EntityKind::Mod, "rss_hybrid", Some("mine on both")).unwrap();
    // A layer on it, on the source machine: it must not land on the local one.
    write(
        &src.root.join("hd/rss_hybrid"),
        "ui/ui_car.json",
        br#"{"name":"Hybrid HD"}"#,
    );
    write(&src.root.join("hd/rss_hybrid"), "texture_hd/body.dds", &[5; 2000]);
    src.import_folder(&src.root.join("hd"));
    let file = root.join("library.pitbox");
    export(&src.db, &src.cfg, &src.places, &[Part::Library], &file).unwrap();

    let dest = Install::new(&root, "dest", &[]);
    let local = dest
        .cfg
        .ac_install_path
        .as_ref()
        .unwrap()
        .join("content/cars/rss_hybrid");
    car(&local, "Hybrid");
    crate::stock::index_stock_content(&dest.conn(), &dest.cfg, &crate::rules::default_rules(), false).unwrap();
    let report = import(
        &dest.db,
        &dest.cfg,
        &dest.places,
        &crate::rules::default_rules(),
        &file,
        &[Part::Library],
    )
    .unwrap();

    assert_eq!(report.local_kept, vec!["rss_hybrid"], "named in the report");
    assert_eq!(report.mods, 0, "nothing imported in its place");
    let conn = dest.conn();
    let hybrid = overlay::get_mod(&conn, "rss_hybrid").unwrap().unwrap();
    assert!(
        hybrid.is_stock && hybrid.is_unmanaged,
        "still the mod installed by hand here"
    );
    assert!(!hybrid.showcase, "with its files");
    assert_eq!(
        hybrid.notes_user.as_deref(),
        Some("mine on both"),
        "and the note written on it"
    );
    assert!(
        overlay::list_layers(&conn, "rss_hybrid", crate::layers::HostKind::Car)
            .unwrap()
            .is_empty(),
        "no layer laid on a mod Pit Box does not manage"
    );
    assert!(
        !dest.library().join("cars").join("rss_hybrid").exists(),
        "no skeleton written for it in the library"
    );
    assert!(local.join("model.kn5").is_file(), "its real folder untouched");
}

fn skeleton_of(install: &Install, id: &str) -> bool {
    crate::skeleton::is_showcase(&install.conn(), id).unwrap()
}

/// Rule (EXPORT R3, R4, §7.3): an import is refused, saying why, into an
/// installation that holds something, from a newer Pit Box, or from a file
/// that is not an export.
#[test]
fn an_import_is_refused_where_it_could_overwrite_or_misread() {
    let root = crate::testutil::temp_dir("transfer-refusals");
    let src = Install::new(&root, "source", &[]);
    let file = root.join("library.pitbox");
    export(&src.db, &src.cfg, &src.places, &[Part::Preferences], &file).unwrap();

    let dest = Install::new(&root, "dest", &[]);
    car(&dest.root.join("in/lanzo"), "Lanzo");
    dest.import_folder(&dest.root.join("in"));
    let refusal = inspect(&dest.conn(), &dest.cfg, &dest.places, &file).unwrap().refusal;
    assert_eq!(
        refusal.map(|r| (r.key, r.count)),
        Some(("transfer.refuseNotEmpty", Some(1)))
    );
    assert_eq!(
        import(
            &dest.db,
            &dest.cfg,
            &dest.places,
            &crate::rules::default_rules(),
            &file,
            &Part::ALL
        )
        .err()
        .as_deref(),
        Some(crate::errors::TRANSFER_NOT_EMPTY),
        "the import itself refuses too"
    );

    let older = Install::new(&root, "older", &[]);
    let places = Places {
        app_version: "0.7.9".into(),
        ..Places {
            config_dir: older.places.config_dir.clone(),
            presets_dir: None,
            app_version: String::new(),
        }
    };
    let refusal = inspect(&older.conn(), &older.cfg, &places, &file)
        .unwrap()
        .refusal
        .unwrap();
    assert_eq!(refusal.key, "transfer.refuseNewer");
    assert_eq!(
        refusal.version.as_deref(),
        Some("0.8.0"),
        "and says which version to install"
    );

    let not_one = root.join("other.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&not_one).unwrap());
    zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert_eq!(
        inspect(&older.conn(), &older.cfg, &older.places, &not_one)
            .err()
            .as_deref(),
        Some(crate::errors::TRANSFER_NOT_AN_EXPORT)
    );
}

/// Rule (EXPORT§7.3): an import that fails on the way leaves the installation
/// as it found it — no mod in the base, no file written.
#[test]
fn an_import_that_fails_on_the_way_leaves_the_installation_empty() {
    let root = crate::testutil::temp_dir("transfer-rollback");
    let src = Install::new(&root, "source", &[]);
    car(&src.root.join("in/lanzo"), "Lanzo");
    src.import_folder(&src.root.join("in"));
    write(&src.places.config_dir, "ui_prefs.json", b"{}");
    let file = root.join("library.pitbox");
    export(&src.db, &src.cfg, &src.places, &Part::ALL, &file).unwrap();

    let dest = Install::new(&root, "dest", &[]);
    // A file where the skeletons need a folder: writing them fails halfway.
    write(dest.library(), "cars", b"in the way");
    let failed = import(
        &dest.db,
        &dest.cfg,
        &dest.places,
        &crate::rules::default_rules(),
        &file,
        &Part::ALL,
    );
    assert!(failed.is_err(), "the import fails");
    let conn = dest.conn();
    assert!(
        overlay::get_mod(&conn, "lanzo").unwrap().is_none(),
        "the base is rolled back"
    );
    assert_eq!(tables::held(&conn).unwrap(), 0);
    assert!(
        !dest.places.config_dir.join("ui_prefs.json").exists(),
        "no setting left behind"
    );
}

/// Rule (EXPORT R4): an export made by an older Pit Box — a base without the
/// columns added since — imports, the migrations bringing it up to date.
#[test]
fn an_export_from_an_older_pit_box_is_brought_up_to_date() {
    let root = crate::testutil::temp_dir("transfer-older");
    let src = Install::new(&root, "source", &[]);
    car(&src.root.join("in/lanzo"), "Lanzo");
    src.import_folder(&src.root.join("in"));
    let file = root.join("library.pitbox");
    export(&src.db, &src.cfg, &src.places, &[Part::Library], &file).unwrap();

    // The same export, its base stripped of columns a later version added.
    let old = root.join("old.pitbox");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&old).unwrap());
    for (name, bytes) in entries(&file) {
        let bytes = if name == DATABASE_ENTRY {
            let tmp = root.join("old.sqlite");
            std::fs::write(&tmp, &bytes).unwrap();
            {
                let c = Connection::open(&tmp).unwrap();
                for (table, col) in [
                    ("mods", "tech_marks"),
                    ("apps", "content_state"),
                    ("layers", "notes_user"),
                ] {
                    c.execute(&format!("ALTER TABLE {table} DROP COLUMN {col}"), [])
                        .unwrap();
                }
            }
            std::fs::read(&tmp).unwrap()
        } else {
            bytes
        };
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut zip, &bytes).unwrap();
    }
    zip.finish().unwrap();

    let dest = Install::new(&root, "dest", &[]);
    import(
        &dest.db,
        &dest.cfg,
        &dest.places,
        &crate::rules::default_rules(),
        &old,
        &[Part::Library],
    )
    .unwrap();
    assert!(
        overlay::get_mod(&dest.conn(), "lanzo").unwrap().is_some(),
        "the old export imports"
    );
}

/// Rule (EXPORT§3): unchecking a part takes nothing of it, even indirectly —
/// and Profiles goes nowhere without the library.
#[test]
fn an_unchecked_part_takes_nothing_of_it() {
    let root = crate::testutil::temp_dir("transfer-parts");
    let src = source(&root);
    let file = root.join("prefs.pitbox");
    export(
        &src.db,
        &src.cfg,
        &src.places,
        &[Part::Preferences, Part::Profiles],
        &file,
    )
    .unwrap();
    let names: Vec<String> = entries(&file).into_iter().map(|(n, _)| n).collect();
    assert!(
        names
            .iter()
            .all(|n| n == MANIFEST_ENTRY || n.starts_with("preferences/")),
        "{names:?}"
    );
    let manifest = read_manifest(&mut open_archive(&file).unwrap()).unwrap();
    assert_eq!(
        manifest.parts,
        vec![Part::Preferences],
        "no profile without the library"
    );
}

/// Rule: every table whose foreign key names a mod is filtered at import
/// (`NAME_A_MOD`) — one forgotten makes a single note on a DLC car fail the
/// whole import, which is how the list was found.
#[test]
fn every_table_naming_a_mod_is_filtered_at_import() {
    let base = crate::testutil::temp_dir("transfer-fk");
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    for (table, _, _) in tables::TABLES {
        let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list({table})")).unwrap();
        let parents: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(2))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        if parents.iter().any(|p| p == "mods") {
            assert!(
                tables::NAME_A_MOD.contains(table),
                "{table} names a mod and is not filtered"
            );
        }
    }
}

/// Rule (R2): a stored path in the library is recognised whatever its case —
/// Windows does not tell `D:\AC-Library` from `d:\ac-library`.
#[test]
fn a_library_path_is_recognised_whatever_its_case() {
    let lib = Path::new(r"D:\AC-Library");
    assert_eq!(
        tables::relative(lib, r"d:\ac-library\cars\lanzo\v1").as_deref(),
        Some("cars/lanzo/v1")
    );
    assert_eq!(
        tables::relative(lib, r"D:\AC-Library\cars\lanzo").as_deref(),
        Some("cars/lanzo")
    );
    assert_eq!(
        tables::relative(lib, r"D:\AC-Library2\cars").as_deref(),
        None,
        "a neighbour is not inside"
    );
    assert_eq!(tables::relative(lib, r"C:\Games\content\cars\x").as_deref(), None);
    assert_eq!(tables::relative(lib, "cars/lanzo/v1").as_deref(), Some("cars/lanzo/v1"));
}

#[test]
fn a_version_reads_as_newer_only_when_it_is() {
    assert!(is_newer("0.10.0", "0.9.3"));
    assert!(!is_newer("0.8.0", "0.8.0"));
    assert!(!is_newer("0.7.12", "0.8.0"));
    assert!(is_newer("garbage", "0.8.0"), "what cannot be read is refused");
}
