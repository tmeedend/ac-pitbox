//! A re-import is a duplicate only when nothing differs (IMPORT§, SPEC §4.1).
//!
//! The content signature looks at `.kn5` and `.acd` files, by path and size:
//! cheap, stored on every version, and what finds a showcase version again.
//! Equal signatures therefore only SAY "probably the same"; what the user asked
//! to import is compared with the active version before it is thrown away. Each
//! test is a way an update used to pass for a duplicate without a word, and
//! every one of them was red on the code that trusted the signature alone.

use super::bulk::analyze_bulk;
use super::*;

struct Lib {
    base: crate::testutil::TempDir,
    library: PathBuf,
    conn: Connection,
    cfg: AppConfig,
    rules: Rules,
}

fn lib() -> Lib {
    let base = crate::testutil::temp_dir("duplicates");
    let library = base.join("library");
    std::fs::create_dir_all(&library).unwrap();
    let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
    let cfg = AppConfig {
        library_path: Some(library.clone()),
        ..Default::default()
    };
    Lib {
        base,
        library,
        conn,
        cfg,
        rules: crate::rules::default_rules(),
    }
}

/// The car as a modder ships it: model, unpacked physics, skins, a few more.
/// Each file's content is its own name, so a change is a deliberate edit.
const FILES: &[(&str, &str)] = &[
    (
        "ui/ui_car.json",
        r#"{"name":"Dup Car","brand":"B","tags":[],"version":"1.0"}"#,
    ),
    ("model.kn5", "KN5-MODEL-v1"),
    ("data/engine.ini", "[ENGINE_DATA]\nLIMITER=8300\n"),
    ("data/drivetrain.ini", "[TRACTION]\nTYPE=RWD\n"),
    ("skins/red/preview.jpg", "RED"),
    ("sfx/car.bank", "BANK"),
];

impl Lib {
    /// Writes (or rewrites) the source folder `src/dup_car` from `FILES`, then
    /// lets `edit` change it.
    fn source(&self, edit: impl FnOnce(&Path)) -> PathBuf {
        let root = self.base.join("src");
        let dir = root.join("dup_car");
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, content) in FILES {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        edit(&dir);
        root
    }

    fn import(&self, root: &Path) -> String {
        let dir = root.join("dup_car");
        let ctx = ImportCtx::silent();
        let r = import_one_folder(&ctx, &self.conn, &self.cfg, &self.rules, 0, &dir, true, &[], None);
        assert!(r.error.is_none(), "{:?}", r.error);
        r.mods[0].outcome.clone()
    }

    fn versions(&self) -> usize {
        crate::overlay::get_versions(&self.conn, "dup_car").unwrap().len()
    }

    /// What the bulk analysis says of the folder `root` (one sub-folder, one mod).
    fn analysed(&self, root: &Path) -> String {
        let entries = analyze_bulk(&self.conn, &self.cfg, root).unwrap();
        entries[0].mods[0].status.clone()
    }

    /// The first import of the car as it ships.
    fn first(&self) -> PathBuf {
        let root = self.source(|_| {});
        assert_eq!(self.import(&root), "IMPORT");
        root
    }
}

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

/// What must stay true: the very same folder, imported again, is a duplicate -
/// no second version, in the import and in the bulk analysis.
#[test]
fn a_really_identical_reimport_is_still_a_duplicate() {
    let l = lib();
    let root = l.first();
    assert_eq!(l.analysed(&root), "duplicate");
    assert_eq!(l.import(&root), "DUPLICATE");
    assert_eq!(l.versions(), 1, "nothing was stored");
}

/// Only `ui_car.json` changed (tags, specs, description): no `.kn5` or `.acd`
/// moved, so the signature did not either.
#[test]
fn an_update_that_only_changes_the_ui_json_is_an_update() {
    let l = lib();
    l.first();
    let root = l.source(|d| {
        write(
            d,
            "ui/ui_car.json",
            r#"{"name":"Dup Car","brand":"B","tags":["gt3"],"version":"1.1"}"#,
        )
    });
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// The unpacked physics tuned by a few characters - same length, same name.
#[test]
fn a_same_length_edit_in_an_ini_is_an_update() {
    let l = lib();
    l.first();
    let root = l.source(|d| write(d, "data/engine.ini", "[ENGINE_DATA]\nLIMITER=8400\n"));
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// The model rebuilt with the same byte count.
#[test]
fn a_kn5_of_the_same_size_and_other_content_is_an_update() {
    let l = lib();
    l.first();
    let root = l.source(|d| write(d, "model.kn5", "KN5-MODEL-v2"));
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// A skin added next to the unchanged model.
#[test]
fn an_added_file_is_an_update() {
    let l = lib();
    l.first();
    let root = l.source(|d| write(d, "skins/blue/preview.jpg", "BLUE"));
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// A file the new release no longer ships.
#[test]
fn a_removed_file_is_an_update() {
    let l = lib();
    l.first();
    let root = l.source(|d| std::fs::remove_file(d.join("sfx/car.bank")).unwrap());
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// The active version's folder cannot be read (library disk unplugged, folder
/// moved): there is nothing to compare, and the answer stays what the signature
/// said - a duplicate, as before. Reading nothing must not turn every re-import
/// into an update.
#[test]
fn a_missing_library_folder_keeps_the_signatures_answer() {
    let l = lib();
    let root = l.first();
    let active = crate::overlay::active_library_path(&l.conn, "dup_car")
        .unwrap()
        .unwrap();
    let folder = crate::libpath::resolve(l.cfg.library_path.as_deref(), &active).unwrap();
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(l.analysed(&root), "duplicate");
    assert_eq!(l.import(&root), "DUPLICATE");
    assert_eq!(l.versions(), 1);
    assert!(l.library.is_dir());
}

/// Switches a sound mod on for the car, as the user does: the car's `sfx/` in
/// the library then holds the mod's files and the original is kept apart.
fn switch_a_sound_on(l: &mut Lib) {
    l.cfg.ac_install_path = Some(l.base.join("ac"));
    let sound = l.library.join("sounds").join("dup_car").join("v8");
    write(&sound, "car.bank", "MODBANK, NOT THE SAME LENGTH");
    write(&sound, "GUIDs.txt", "MOD");
    let now = chrono::Local::now().to_rfc3339();
    crate::overlay::insert_sub_mod(
        &l.conn,
        "s1",
        "SOUND",
        "dup_car",
        "v8",
        &sound.to_string_lossy(),
        None,
        &now,
    )
    .unwrap();
    crate::submods::activate_sound(&l.conn, &l.cfg, "s1").unwrap();
    let active = crate::overlay::active_library_path(&l.conn, "dup_car")
        .unwrap()
        .unwrap();
    let folder = crate::libpath::resolve(l.cfg.library_path.as_deref(), &active).unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("sfx/car.bank")).unwrap(),
        "MODBANK, NOT THE SAME LENGTH",
        "the sound really is switched"
    );
}

/// Real regression of the byte-by-byte rule: the switch rewrites the car's
/// `sfx/` in the library, so the same archive no longer looked like what it
/// had been imported as, and came back as a second version - with the stock
/// sound, while the base still said the mod was on. What it was imported with
/// is the original sound, kept apart: that is what it is compared with.
#[test]
fn a_reimport_after_a_sound_switch_is_still_a_duplicate() {
    let mut l = lib();
    let root = l.first();
    switch_a_sound_on(&mut l);
    assert_eq!(l.analysed(&root), "duplicate");
    assert_eq!(l.import(&root), "DUPLICATE");
    assert_eq!(l.versions(), 1, "nothing was stored");
}

/// The sound being on hides nothing else: the archive's own `sfx/` is compared
/// with the original one, so a release that ships another sound is an update.
#[test]
fn a_new_sound_in_the_archive_is_an_update_even_while_a_sound_is_on() {
    let mut l = lib();
    l.first();
    switch_a_sound_on(&mut l);
    let root = l.source(|d| write(d, "sfx/car.bank", "A NEW ORIGINAL SOUND"));
    assert_eq!(l.analysed(&root), "update");
    assert_eq!(l.import(&root), "UPDATE_REPLACE");
    assert_eq!(l.versions(), 2);
}

/// Back to the stock sound, `sfx/` is the car's own again and the plain
/// comparison applies - nothing of the sound's detour is left in the rule.
#[test]
fn a_reimport_after_the_stock_sound_is_back_is_still_a_duplicate() {
    let mut l = lib();
    let root = l.first();
    switch_a_sound_on(&mut l);
    crate::submods::restore_sound(&l.conn, &l.cfg, "dup_car").unwrap();
    assert_eq!(l.analysed(&root), "duplicate");
    assert_eq!(l.import(&root), "DUPLICATE");
    assert_eq!(l.versions(), 1);
}

/// The original sound is gone from the library: nothing to compare the `sfx/`
/// with, and the answer stays the signature's - a duplicate, as everywhere the
/// files cannot be read.
#[test]
fn a_missing_original_sound_keeps_the_signatures_answer() {
    let mut l = lib();
    let root = l.first();
    switch_a_sound_on(&mut l);
    std::fs::remove_dir_all(crate::submods::sound_backup_dir(&l.cfg, "dup_car").unwrap()).unwrap();
    assert_eq!(l.analysed(&root), "duplicate");
    assert_eq!(l.import(&root), "DUPLICATE");
    assert_eq!(l.versions(), 1);
}
