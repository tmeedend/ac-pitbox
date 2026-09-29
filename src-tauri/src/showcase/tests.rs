//! Tests of the showcase (ESPACE§): the removal, its resumption, and the way
//! back through the import. Real file system, as everywhere in the backend.

use super::*;

const ENGINE: &str = "[ENGINE_DATA]\nLIMITER=8300\n[TURBO_0]\nMAX_BOOST=1\n";
const DRIVETRAIN: &str = "[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=6\n";

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn photo(path: &Path, width: u32, height: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]))
        .save(path)
        .unwrap();
}

/// A library with one managed mod, one version, not deployed.
struct Fixture {
    base: crate::testutil::TempDir,
    conn: Connection,
    cfg: AppConfig,
    dir: PathBuf,
}

fn fixture(tag: &str, id: &str, kind: &str, fill: impl Fn(&Path)) -> Fixture {
    let base = crate::testutil::temp_dir(tag);
    let folder = if kind == "Track" { "tracks" } else { "cars" };
    std::fs::create_dir_all(base.join("ac").join("content").join(folder)).unwrap();
    let dir = base.join("lib").join(folder).join(id).join("v1");
    fill(&dir);
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    let now = chrono::Local::now().to_rfc3339();
    // Brand and name as the car's `ui_car.json` says them: a reindex reads
    // them again, and must find nothing to change.
    overlay::upsert_mod(&conn, id, kind, Some("RSS"), Some("Lanzo"), "h", None, &now).unwrap();
    overlay::insert_version(
        &conn,
        "v1",
        id,
        Some("1.4"),
        None,
        &now,
        &dir.to_string_lossy(),
        Some("mod_v1.4.7z"),
        "sig-full",
        &["lightingfx".to_string()],
        &["red".to_string()],
        &[],
        &[],
        None,
    )
    .unwrap();
    overlay::set_active_version(&conn, id, "v1").unwrap();
    let cfg = AppConfig {
        ac_install_path: Some(base.join("ac")),
        library_path: Some(base.join("lib")),
        ..Default::default()
    };
    Fixture { base, conn, cfg, dir }
}

fn car(dir: &Path) {
    write(
        dir,
        "ui/ui_car.json",
        br#"{"name":"Lanzo","brand":"RSS","specs":{"bhp":"600bhp","weight":"1200kg"}}"#,
    );
    write(dir, "ui/badge.png", b"badge");
    write(dir, "data/engine.ini", ENGINE.as_bytes());
    write(dir, "data/drivetrain.ini", DRIVETRAIN.as_bytes());
    write(dir, "lanzo.kn5", &[7; 5000]);
    write(dir, "sfx/lanzo.bank", &[1; 3000]);
    photo(&dir.join("skins/red/preview.jpg"), 1022, 575);
    write(dir, "skins/red/livery.png", b"livery");
    write(
        dir,
        "extension/ext_config.ini",
        b"[LIGHT_SERIES_1]
MESHES=light
",
    );
}

/// Where a version's files wait for the recycle bin, as `Freeing` names it.
fn staging_root(dir: &Path) -> PathBuf {
    let name = dir.file_name().unwrap().to_string_lossy();
    dir.parent().unwrap().join(format!(".pitbox-freeing-{name}"))
}

fn files_of(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"))
        .collect();
    out.sort();
    out
}

/// Rule (ESPACE§3.1, ESPACE§5.5): an active car is taken out of the game, then
/// its folder keeps exactly the skeleton — no `data/`, no model, no skin
/// preview — and the manifest lists exactly the rest, with sizes.
#[test]
fn a_car_in_the_showcase_keeps_exactly_its_skeleton() {
    let f = fixture("showcase-car", "lanzo", "Car", car);
    crate::activation::activate(&f.conn, &f.cfg, "lanzo", None).unwrap();
    assert!(crate::activation::is_mod_active(&f.cfg, ModKind::Car, "lanzo"));
    let before = crate::inspect::dir_size_bytes(&f.dir) as i64;
    overlay::update_version_size(&f.conn, "v1", before).unwrap();

    let out = to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

    assert!(out.was_active, "it was in the game");
    assert!(
        !f.base.join("ac/content/cars/lanzo").exists(),
        "taken out of the game before anything else"
    );
    assert_eq!(
        files_of(&f.dir),
        vec![
            ".pitbox-vitrine.jpg",
            ".pitbox-vitrine.json",
            "ui/badge.png",
            "ui/ui_car.json"
        ],
        "exactly the skeleton is left"
    );
    let manifest = skeleton::read_manifest(&f.dir).expect("a manifest");
    let listed: Vec<&str> = manifest.removed.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        listed,
        vec![
            "data/drivetrain.ini",
            "data/engine.ini",
            "extension/ext_config.ini",
            "lanzo.kn5",
            "sfx/lanzo.bank",
            "skins/red/livery.png",
            "skins/red/preview.jpg"
        ],
        "the manifest lists what went"
    );
    assert_eq!(
        manifest.content_signature.as_deref(),
        Some("sig-full"),
        "the original signature"
    );
    assert_eq!(manifest.source_archive.as_deref(), Some("mod_v1.4.7z"));
    let image = skeleton::image_of(&f.dir).unwrap();
    assert!(
        std::fs::metadata(&image).unwrap().len() < 100_000,
        "the frozen image is small"
    );

    let v = overlay::get_version(&f.conn, "v1").unwrap().unwrap();
    assert!(v.is_skeleton(), "marked in the base");
    assert_eq!(out.freed_bytes, manifest.removed_bytes, "a car has nothing reduced");
    assert_eq!(v.freed_bytes, Some(out.freed_bytes as i64));
    assert_eq!(
        v.size_bytes,
        Some(before),
        "the size is the mod's own, from before its files went"
    );
    assert_eq!(v.skins, vec!["red".to_string()], "the skin names stay in the base");
    assert_eq!(v.csp_features, vec!["lightingfx".to_string()]);
    assert!(
        !f.dir.parent().unwrap().join(".pitbox-freeing-v1").exists(),
        "no staging folder left behind"
    );
    let history = overlay::get_history(&f.conn, "lanzo").unwrap();
    assert_eq!(history[0].event, "SHOWCASE", "journaled");
}

/// Rule (ESPACE R5): once in the showcase, nothing lays the mod in the
/// game again, and the card says it is in the showcase, not broken.
#[test]
fn a_showcase_mod_never_goes_back_into_the_game() {
    let f = fixture("showcase-guard", "lanzo", "Car", car);
    to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

    assert_eq!(
        crate::activation::activate(&f.conn, &f.cfg, "lanzo", None)
            .err()
            .as_deref(),
        Some(crate::errors::CONTENT_FREED),
        "activation refuses"
    );
    assert!(!f.base.join("ac/content/cars/lanzo").exists(), "and wrote nothing");
    crate::compose::recompose(&f.conn, &f.cfg, "lanzo")
        .expect("a layer action on a mod in the showcase is not an error");
    assert!(!f.base.join("ac/content/cars/lanzo").exists(), "but composes nothing");
    let ctx = crate::bulk::BulkCtx::silent();
    let report = crate::bulk::activate(&ctx, &f.conn, &f.cfg, &["lanzo".to_string()]);
    assert_eq!(report.skipped, vec!["lanzo".to_string()], "a lot counts it apart");
    assert!(report.failed.is_empty(), "never as a failure");
    let now = chrono::Local::now().to_rfc3339();
    overlay::create_profile(&f.conn, "p1", "Endurance", &now).unwrap();
    overlay::add_profile_entry(&f.conn, "p1", "lanzo", "v1").unwrap();
    let applied = crate::profiles::apply(&f.conn, &f.cfg, "p1").unwrap();
    assert_eq!(applied.skipped, vec!["lanzo".to_string()], "a profile leaves it out");
    assert!(applied.errors.is_empty(), "without an error");
    assert_eq!(
        crate::extras::deploy(&f.conn, &f.cfg, crate::extras::OwnerKind::Car, "lanzo")
            .err()
            .as_deref(),
        Some(crate::errors::CONTENT_FREED),
        "its additions to the game stay out"
    );

    let m = overlay::get_mod(&f.conn, "lanzo").unwrap().unwrap();
    assert_eq!(
        crate::maintenance::broken_reason(&f.conn, &f.cfg, &m),
        None,
        "not broken"
    );
    let card = crate::library::detail(&f.conn, &f.cfg, "lanzo").unwrap().unwrap().card;
    assert!(card.base.showcase);
    assert!(!card.broken);
    assert_eq!(
        card.preview.map(PathBuf::from),
        skeleton::image_of(&f.dir),
        "the card shows the frozen image"
    );
    assert_eq!(
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).err().as_deref(),
        Some(crate::errors::CONTENT_FREED),
        "and it cannot be freed twice"
    );
}

/// Rule (ESPACE R2, ESPACE§3.4): nothing that costs nothing is lost. The user's
/// entries and the tech sheet read from `data/` are identical after, and
/// a reindex of the skeleton does not wipe what its files no longer say.
#[test]
fn nothing_the_user_or_the_sheet_holds_is_lost() {
    let f = fixture("showcase-keep", "lanzo", "Car", car);
    let c = &f.conn;
    overlay::set_favorite(c, "lanzo", true).unwrap();
    overlay::set_manual_tags(c, "lanzo", &["endurance".to_string()]).unwrap();
    overlay::set_mod_field(c, "lanzo", "display_name_user", Some("My Lanzo")).unwrap();
    overlay::set_mod_field(c, "lanzo", "description_user", Some("Notes")).unwrap();
    crate::techsheet::record(c, "lanzo", "v1", &f.dir, false).unwrap();
    overlay::update_version_size(c, "v1", 551_000_000).unwrap();
    overlay::record_decision(c, Some("lanzo"), "mod_v1.4.7z", "extra", "extension/", None);
    c.execute(
        "INSERT INTO media_links (file_path, entity_id, kind) VALUES ('shot.jpg', 'lanzo', 'SCREENSHOT')",
        [],
    )
    .unwrap();
    let rows = |table: &str, column: &str| -> i64 {
        c.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {column} = 'lanzo'"),
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    let mod_before = serde_json::to_value(overlay::get_mod(c, "lanzo").unwrap()).unwrap();
    let sheet_before = serde_json::to_value(crate::techsheet::effective(c, "lanzo").unwrap()).unwrap();
    assert!(
        sheet_before.to_string().contains("RWD"),
        "the fixture's physics reached the sheet — otherwise this test proves nothing"
    );

    to_showcase(c, &f.cfg, "lanzo", None, true).unwrap();
    crate::maintenance::reindex_all(c, &f.cfg, true).unwrap();

    let mut mod_after = serde_json::to_value(overlay::get_mod(c, "lanzo").unwrap()).unwrap();
    // The only field allowed to move: the state itself. The size stays
    // the mod's own — a reindex that measures it included.
    assert_eq!(mod_after["showcase"], serde_json::json!(true));
    mod_after["showcase"] = mod_before["showcase"].clone();
    assert_eq!(mod_after, mod_before, "every entry of the mod is intact");
    assert_eq!(
        serde_json::to_value(crate::techsheet::effective(c, "lanzo").unwrap()).unwrap(),
        sheet_before,
        "the tech sheet is the same, reindex included"
    );
    assert_eq!(rows("import_decisions", "mod_id"), 1, "the import journal is kept");
    assert_eq!(rows("media_links", "entity_id"), 1, "and the media attached by hand");
    let v = overlay::get_version(c, "v1").unwrap().unwrap();
    assert_eq!(v.skins, vec!["red".to_string()], "the reindex kept the skin names");
    assert_eq!(v.csp_features, vec!["lightingfx".to_string()], "and the CSP features");
}

/// Rule (ESPACE§3.3): the image frozen is the one the screen shows — a
/// preferred skin, a regenerated thumbnail —, not the backend's guess.
#[test]
fn the_frozen_image_is_the_one_the_card_shows() {
    let f = fixture("showcase-image", "lanzo", "Car", car);
    let thumb = f.base.join("cache/grid.png");
    std::fs::create_dir_all(thumb.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(800, 450, image::Rgba([10, 20, 30, 0]))
        .save(&thumb)
        .unwrap();
    to_showcase(&f.conn, &f.cfg, "lanzo", Some(&thumb), true).unwrap();
    let frozen = skeleton::image_of(&f.dir).unwrap();
    assert!(
        frozen.to_string_lossy().ends_with(".png"),
        "the transparent thumbnail was frozen, not the skin preview"
    );
}

/// Rule (ESPACE§3.1, ESPACE§3.3): a track keeps every layout's `ui/`, its
/// previews reduced in place; the layout map and the models go.
#[test]
fn a_track_keeps_its_layouts_with_reduced_previews() {
    let f = fixture("showcase-track", "shannon", "Track", |dir| {
        write(dir, "ui/gp/ui_track.json", br#"{"name":"Shannonville GP"}"#);
        write(dir, "ui/gp/outline.png", b"outline");
        photo(&dir.join("ui/gp/preview.png"), 1920, 1080);
        write(dir, "gp/map.png", b"map");
        write(dir, "shannon.kn5", &[3; 4000]);
    });
    let out = to_showcase(&f.conn, &f.cfg, "shannon", None, true).unwrap();
    assert_eq!(
        files_of(&f.dir),
        vec![
            ".pitbox-vitrine.jpg",
            ".pitbox-vitrine.json",
            "ui/gp/outline.png",
            "ui/gp/preview.png",
            "ui/gp/ui_track.json"
        ]
    );
    let preview = image::open(f.dir.join("ui/gp/preview.png")).unwrap();
    assert_eq!(preview.width(), skeleton::IMAGE_WIDTH, "the preview was reduced");
    assert_eq!(
        std::fs::read(f.dir.join("ui/gp/outline.png")).unwrap(),
        b"outline",
        "the outline untouched"
    );
    let manifest = skeleton::read_manifest(&f.dir).unwrap();
    assert!(
        out.freed_bytes > manifest.removed_bytes,
        "what the reduction gave back is counted too"
    );
}

/// Rule (ESPACE§5.5): a file that cannot be moved stops the removal and
/// puts back what was moved — the version is left complete, without a
/// manifest a resume would act on.
#[test]
fn a_locked_file_leaves_the_version_complete() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = fixture("showcase-locked", "lanzo", "Car", car);
    let before = files_of(&f.dir);
    // No sharing at all: the file cannot be renamed while this is open.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.dir.join("sfx/lanzo.bank"))
        .unwrap();
    assert!(
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).is_err(),
        "the removal stops"
    );
    drop(lock);

    let mut after = files_of(&f.dir);
    after.retain(|p| !p.starts_with(".pitbox-vitrine."));
    assert_eq!(after, before, "every moved file was put back");
    assert!(
        skeleton::read_manifest(&f.dir).is_none(),
        "no manifest left for a resume to act on"
    );
    assert!(!overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
    assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "and nothing to resume");
}

/// Rule (ESPACE§5.5): a rollback during a resume puts back what the
/// interrupted run had already moved too — it never deletes a file.
/// Bug caught in review: the staging folder, holding the model moved by
/// the first run, was wiped along with the rollback.
#[test]
fn a_rollback_during_a_resume_puts_back_what_the_first_run_moved() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = fixture("showcase-resume-rollback", "lanzo", "Car", car);
    let before = files_of(&f.dir);
    let mut manifest = Manifest::new(
        "2026-09-27T10:00:00Z".into(),
        "lanzo".into(),
        skeleton::removable_files(ModKind::Car, &f.dir),
    );
    manifest.content_signature = Some("sig-full".into());
    skeleton::write_manifest(&f.dir, &manifest).unwrap();
    let staging = staging_root(&f.dir).join("lanzo");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::rename(f.dir.join("lanzo.kn5"), staging.join("lanzo.kn5")).unwrap();

    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.dir.join("sfx/lanzo.bank"))
        .unwrap();
    assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "the resume stops");
    drop(lock);

    let mut after = files_of(&f.dir);
    after.retain(|p| !p.starts_with(".pitbox-vitrine."));
    assert_eq!(after, before, "the model moved by the first run is back as well");
    assert!(!staging_root(&f.dir).exists(), "nothing left in staging");
    assert!(!overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
}

/// Rule (ESPACE§3.3): the card often shows a livery from an attached
/// skin pack, reached through a projection junction. It is frozen before
/// the projection goes — and the stored skin itself is left alone.
#[test]
fn a_livery_from_an_attached_skin_pack_is_frozen() {
    let f = fixture("showcase-projected", "lanzo", "Car", car);
    let store = f.base.join("lib/skins/lanzo/pack_livery");
    photo(&store.join("preview.jpg"), 800, 450);
    crate::activation::create_junction(&f.dir.join("skins/pack_livery"), &store).unwrap();

    let shown = f.dir.join("skins/pack_livery/preview.jpg");
    to_showcase(&f.conn, &f.cfg, "lanzo", Some(&shown), true).unwrap();

    let frozen = skeleton::image_of(&f.dir).expect("the livery shown on the card was frozen");
    let img = image::open(frozen).unwrap();
    assert_eq!(
        img.height(),
        270,
        "from the 800×450 livery, not the 1022×575 skin preview"
    );
    assert!(store.join("preview.jpg").is_file(), "the stored skin is untouched");
    assert!(
        !f.dir.join("skins").exists(),
        "and the projection is gone from the skeleton"
    );
}

/// A mod with an older version next to the active one, both complete.
fn with_older_version(f: &Fixture) -> PathBuf {
    let old = f.base.join("lib/cars/lanzo/v0");
    car(&old);
    overlay::insert_version(
        &f.conn,
        "v0",
        "lanzo",
        Some("1.0"),
        None,
        "2020-01-01T00:00:00+00:00",
        &old.to_string_lossy(),
        None,
        "sig-old",
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    old
}

/// Rule (ESPACE§5.4): every version of the mod goes into the showcase,
/// not only the active one.
#[test]
fn every_version_goes_into_the_showcase() {
    let f = fixture("showcase-versions", "lanzo", "Car", car);
    let old = with_older_version(&f);
    to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();
    for id in ["v0", "v1"] {
        assert!(
            overlay::get_version(&f.conn, id).unwrap().unwrap().is_skeleton(),
            "{id} is a skeleton"
        );
    }
    assert!(!old.join("lanzo.kn5").exists(), "the old version lost its files too");
}

/// Rule (ESPACE§5.4): all versions or none. One version that resists —
/// here the older one, handled after the active one — puts every version
/// back as it was: no mod half in the showcase, no journal entry.
#[test]
fn one_version_that_resists_leaves_every_version_complete() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = fixture("showcase-older", "lanzo", "Car", car);
    let old = with_older_version(&f);
    let (active_before, old_before) = (files_of(&f.dir), files_of(&old));
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(old.join("sfx/lanzo.bank"))
        .unwrap();
    assert!(
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).is_err(),
        "the lot reports the mod as failed"
    );
    drop(lock);

    assert_eq!(
        files_of(&f.dir),
        active_before,
        "the active version got every file back"
    );
    assert_eq!(files_of(&old), old_before, "the older one too");
    assert!(!skeleton::is_showcase(&f.conn, "lanzo").unwrap(), "not in the showcase");
    for id in ["v0", "v1"] {
        assert!(!overlay::get_version(&f.conn, id).unwrap().unwrap().is_skeleton());
    }
    assert!(
        overlay::get_history(&f.conn, "lanzo").unwrap().is_empty(),
        "nothing journaled"
    );
    assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "and nothing left for a resume");
}

/// Rule (ESPACE§5.5): a manifest written, half the files moved, the base
/// still saying "complete" — the next start finishes the job, from the
/// manifest. A manifest that does not describe the version is ignored.
#[test]
fn an_interrupted_removal_is_finished_at_startup() {
    let f = fixture("showcase-resume", "lanzo", "Car", car);
    let mut manifest = Manifest::new(
        "2026-09-27T10:00:00Z".into(),
        "lanzo".into(),
        skeleton::removable_files(ModKind::Car, &f.dir),
    );
    manifest.content_signature = Some("another-archive".into());
    skeleton::write_manifest(&f.dir, &manifest).unwrap();
    assert_eq!(
        resume_interrupted(&f.conn, &f.cfg),
        0,
        "a foreign manifest is not trusted"
    );
    assert!(f.dir.join("lanzo.kn5").exists());

    manifest.content_signature = Some("sig-full".into());
    skeleton::write_manifest(&f.dir, &manifest).unwrap();
    // Half done: the model is already in the staging folder.
    let staging = staging_root(&f.dir).join("lanzo");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::rename(f.dir.join("lanzo.kn5"), staging.join("lanzo.kn5")).unwrap();

    assert_eq!(resume_interrupted(&f.conn, &f.cfg), 1, "one version finished");
    assert_eq!(
        files_of(&f.dir),
        vec![".pitbox-vitrine.json", "ui/badge.png", "ui/ui_car.json"],
        "the rest went too"
    );
    assert!(!staging_root(&f.dir).exists(), "the staging folder too");
    assert!(overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
    assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "idempotent");
}

/// Rule (ESPACE§7.1): "reinstall from the kept archive" is a way back
/// from the showcase — the version is complete again, not a skeleton
/// holding complete files.
#[test]
fn reinstalling_from_the_kept_archive_takes_the_mod_out_of_the_showcase() {
    let f = fixture("showcase-reinstall", "lanzo", "Car", car);
    // A kept *folder*: no 7-Zip needed to take it back.
    let kept = f.base.join("lib/_source_archives/u1/src");
    car(&kept.join("lanzo"));
    overlay::set_kept_archive(&f.conn, "v1", &kept.to_string_lossy()).unwrap();
    to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

    crate::maintenance::reinstall_from_archive(&f.conn, &f.cfg, "lanzo").unwrap();

    assert!(!skeleton::is_showcase(&f.conn, "lanzo").unwrap(), "out of the showcase");
    assert!(f.dir.join("lanzo.kn5").is_file(), "the files are back");
    assert!(skeleton::read_manifest(&f.dir).is_none(), "and the manifest gone");
    crate::activation::activate(&f.conn, &f.cfg, "lanzo", None).expect("it can go into the game");
}

/// Rule (ESPACE§5.2): the confirmation is told what the deletion does
/// before anything is done — in the game or not, how many versions, what
/// they weigh, where the archive came from — and afterwards, that the
/// mod is in the showcase.
#[test]
fn the_plan_says_what_a_deletion_does() {
    let f = fixture("showcase-plan", "lanzo", "Car", car);
    crate::activation::activate(&f.conn, &f.cfg, "lanzo", None).unwrap();
    let size = crate::inspect::dir_size_bytes(&f.dir);

    let p = &plan(&f.conn, &f.cfg, &["lanzo".to_string()]).unwrap()[0];
    assert!(p.active && !p.showcase);
    assert_eq!(p.versions, 1);
    assert_eq!(p.size_bytes, size, "a size never recorded is measured");
    assert_eq!(
        p.source_file_name.as_deref(),
        Some("mod_v1.4.7z"),
        "the archive name, as a fallback"
    );
    assert!(!p.kept_archive);

    to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();
    let p = &plan(&f.conn, &f.cfg, &["lanzo".to_string()]).unwrap()[0];
    assert!(p.showcase && !p.active, "only a complete deletion is left");
    assert_eq!(p.versions, 0, "nothing left to free");
}

/// Rule (ESPACE§7.1): the author's page is read from the `ui_*.json` the
/// skeleton kept — and only an address a browser opens is offered.
#[test]
fn the_sources_keep_only_web_addresses() {
    let f = fixture("showcase-sources", "lanzo", "Car", car);
    write(
        &f.dir,
        "ui/ui_car.json",
        br#"{"name":"Lanzo","url":"https://rss.example/lanzo"}"#,
    );
    to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();
    let s = sources(&f.conn, &f.cfg, "lanzo").unwrap();
    assert_eq!(s.author_url.as_deref(), Some("https://rss.example/lanzo"));
    assert_eq!(s.file_name.as_deref(), Some("mod_v1.4.7z"));

    write(
        &f.dir,
        "ui/ui_car.json",
        br#"{"name":"Lanzo","url":"file:///C:/Windows"}"#,
    );
    assert_eq!(
        sources(&f.conn, &f.cfg, "lanzo").unwrap().author_url,
        None,
        "never a local file"
    );
}

/// Rule (ESPACE R3, ESPACE§5.1): a broken mod whose library folder vanished can
/// still go into the showcase — that is what keeps its name, notes and
/// tags when the Maintenance screen offers to delete it. The folder comes
/// back as a skeleton holding its manifest, and the mod is no longer
/// listed as broken.
#[test]
fn a_broken_mod_whose_folder_vanished_goes_into_the_showcase() {
    let f = fixture("showcase-broken", "lanzo", "Car", car);
    std::fs::remove_dir_all(&f.dir).unwrap();
    let m = overlay::get_mod(&f.conn, "lanzo").unwrap().unwrap();
    assert!(
        crate::maintenance::broken_reason(&f.conn, &f.cfg, &m).is_some(),
        "broken to begin with — otherwise this test proves nothing"
    );

    let out = to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

    assert_eq!(out.freed_bytes, 0, "nothing was left to free");
    assert!(
        skeleton::read_manifest(&f.dir).is_some(),
        "the folder is back, with its manifest"
    );
    let m = overlay::get_mod(&f.conn, "lanzo").unwrap().unwrap();
    assert!(m.showcase);
    assert_eq!(
        crate::maintenance::broken_reason(&f.conn, &f.cfg, &m),
        None,
        "no longer broken"
    );
}

/// Rule (ESPACE§5.2): the kept source archive goes only when asked, and
/// the base forgets it with it.
#[test]
fn the_kept_archive_goes_only_when_asked() {
    let f = fixture("showcase-archive", "lanzo", "Car", car);
    let kept = f.base.join("lib/_source_archives/u1/mod_v1.4.7z");
    write(&f.base, "lib/_source_archives/u1/mod_v1.4.7z", b"7z");
    overlay::set_kept_archive(&f.conn, "v1", &kept.to_string_lossy()).unwrap();

    to_showcase(&f.conn, &f.cfg, "lanzo", None, false).unwrap();
    assert!(!kept.exists(), "removed on request");
    assert_eq!(
        overlay::get_version(&f.conn, "v1").unwrap().unwrap().kept_archive_path,
        None
    );
}

/// Imports the folders of `src` as the app does, through the public path.
fn import(db: &overlay::Db, cfg: &AppConfig, src: &Path) -> Vec<crate::importer::ImportedMod> {
    let ctx = crate::import_progress::ImportCtx::silent();
    let rules = crate::rules::default_rules();
    let paths = [src.to_string_lossy().into_owned()];
    crate::importer::import_folders(&ctx, db, cfg, &rules, &paths, true, &[])
        .unwrap()
        .into_iter()
        .flat_map(|r| r.mods)
        .collect()
}

/// A library and an AC install, and a car folder to import from `src`.
fn round_trip_setup(tag: &str) -> (crate::testutil::TempDir, overlay::Db, AppConfig, PathBuf) {
    let base = crate::testutil::temp_dir(tag);
    std::fs::create_dir_all(base.join("ac/content/cars")).unwrap();
    std::fs::create_dir_all(base.join("lib")).unwrap();
    let db = overlay::Db(std::sync::Mutex::new(
        overlay::open(&base.join("overlay.sqlite")).unwrap(),
    ));
    let cfg = AppConfig {
        ac_install_path: Some(base.join("ac")),
        library_path: Some(base.join("lib")),
        ..Default::default()
    };
    let src = base.join("src");
    car(&src.join("lanzo"));
    (base, db, cfg, src)
}

/// Rule (ESPACE§7.3, ESPACE§9.4): import, showcase, import the same archive
/// again — the files come back into the **same version**, byte for byte,
/// the user's entries untouched, and the mod is not laid in the game.
#[test]
fn the_same_archive_brings_the_files_back_into_the_same_version() {
    let (_base, db, cfg, src) = round_trip_setup("showcase-roundtrip");
    assert_eq!(import(&db, &cfg, &src)[0].outcome, "IMPORT");
    {
        let conn = db.0.lock().unwrap();
        overlay::set_mod_field(&conn, "lanzo", "display_name_user", Some("My Lanzo")).unwrap();
        overlay::set_favorite(&conn, "lanzo", true).unwrap();
        to_showcase(&conn, &cfg, "lanzo", None, true).unwrap();
    }

    let back = import(&db, &cfg, &src);
    assert_eq!(back[0].outcome, "REHYDRATED", "recognized, not a duplicate");
    assert_eq!(back[0].missing_files, 0, "everything came back");

    let conn = db.0.lock().unwrap();
    let versions = overlay::get_versions(&conn, "lanzo").unwrap();
    assert_eq!(versions.len(), 1, "the same version, not a new one");
    let v = &versions[0];
    assert!(!v.is_skeleton(), "complete again");
    assert_eq!(v.freed_at, None);
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path).unwrap();
    assert_eq!(files_of(&dir), files_of(&src.join("lanzo")), "the same files");
    for rel in files_of(&dir) {
        assert_eq!(
            std::fs::read(dir.join(&rel)).unwrap(),
            std::fs::read(src.join("lanzo").join(&rel)).unwrap(),
            "{rel} identical byte for byte"
        );
    }
    let m = overlay::get_mod(&conn, "lanzo").unwrap().unwrap();
    assert_eq!(m.display_name_user.as_deref(), Some("My Lanzo"), "entries untouched");
    assert!(m.is_favorite);
    assert!(!skeleton::is_showcase(&conn, "lanzo").unwrap());
    assert!(
        !crate::activation::is_mod_active(&cfg, ModKind::Car, "lanzo"),
        "not laid in the game on its own"
    );
    assert_eq!(overlay::get_history(&conn, "lanzo").unwrap()[0].event, "REHYDRATED");
    assert!(
        !dir.parent()
            .unwrap()
            .read_dir()
            .unwrap()
            .flatten()
            .any(|e| e.path() != dir),
        "no working folder left next to the version"
    );
    crate::activation::activate(&conn, &cfg, "lanzo", None).expect("and it can be activated again");
}

/// Rule (ESPACE§7.3): another version of a mod in the showcase is an
/// update — never a layer on the skeleton —, the skeleton staying in the
/// timeline.
#[test]
fn another_version_updates_and_the_skeleton_stays_in_the_timeline() {
    let (_base, db, cfg, src) = round_trip_setup("showcase-update");
    import(&db, &cfg, &src);
    to_showcase(&db.0.lock().unwrap(), &cfg, "lanzo", None, true).unwrap();

    write(&src.join("lanzo"), "lanzo.kn5", &[9; 6000]);
    let out = import(&db, &cfg, &src);
    assert_eq!(out[0].outcome, "UPDATE_REPLACE", "an update");

    let conn = db.0.lock().unwrap();
    let versions = overlay::get_versions(&conn, "lanzo").unwrap();
    assert_eq!(versions.len(), 2, "the skeleton stays in the timeline");
    assert_eq!(versions.iter().filter(|v| v.is_skeleton()).count(), 1);
    assert!(
        !skeleton::is_showcase(&conn, "lanzo").unwrap(),
        "the new version is the active one"
    );
    assert!(
        overlay::list_layers(&conn, "lanzo", crate::layers::HostKind::Car)
            .unwrap()
            .is_empty(),
        "no layer on the skeleton"
    );
}

/// Rule (ESPACE§7.5): a layer imported onto a mod in the showcase stays a
/// layer. It is compared with what the version held before its files went —
/// its manifest —, not with its skeleton, next to which everything looks new
/// and would have made a new version out of a single added layout.
#[test]
fn a_layer_onto_a_showcase_track_stays_a_layer() {
    let base = crate::testutil::temp_dir("showcase-layer-import");
    std::fs::create_dir_all(base.join("ac/content/tracks")).unwrap();
    std::fs::create_dir_all(base.join("lib")).unwrap();
    let db = overlay::Db(std::sync::Mutex::new(
        overlay::open(&base.join("overlay.sqlite")).unwrap(),
    ));
    let cfg = AppConfig {
        ac_install_path: Some(base.join("ac")),
        library_path: Some(base.join("lib")),
        ..Default::default()
    };
    let track = base.join("src/shannon");
    write(&track, "ui/gp/ui_track.json", br#"{"name":"Shannonville GP"}"#);
    write(&track, "gp/models.ini", b"[MODEL_0]");
    write(&track, "gp/data/surfaces.ini", b"[SURFACE_0]");
    write(&track, "shannon.kn5", &[3; 4000]);
    assert_eq!(import(&db, &cfg, &base.join("src"))[0].outcome, "IMPORT");
    to_showcase(&db.0.lock().unwrap(), &cfg, "shannon", None, true).unwrap();

    // A new layout: its model, its ini, its ui — nothing the track had.
    let layer = base.join("layer/shannon");
    write(&layer, "club/models.ini", b"[MODEL_0]");
    write(&layer, "club.kn5", &[4; 3000]);
    write(&layer, "ui/club/ui_track.json", br#"{"name":"Shannonville Club"}"#);
    let out = import(&db, &cfg, &base.join("layer"));

    assert_eq!(out[0].outcome, "EXTENSION", "a layer, not an update");
    let conn = db.0.lock().unwrap();
    assert_eq!(
        overlay::get_versions(&conn, "shannon").unwrap().len(),
        1,
        "no new version"
    );
    assert!(
        skeleton::is_showcase(&conn, "shannon").unwrap(),
        "still in the showcase"
    );
}
