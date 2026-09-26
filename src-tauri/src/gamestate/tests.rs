//! End-to-end tests of the game folder scan (DOSSIER§9.3): one fixture game
//! folder holding at least one case of every line of the tables DOSSIER§4.1, DOSSIER§4.2 and
//! DOSSIER§4.3, laid by the engine itself wherever the engine lays it - activation,
//! game additions, backups - so that the scan is measured against what really
//! happens, not against a hand-made imitation of it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::Connection;

use super::*;
use crate::config::AppConfig;
use crate::overlay;

const NOW: &str = "2026-09-26T10:00:00+02:00";

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Writes a file dated `age` ago: the date arbitration of shared files
/// (§4.5.4) is what several cases below turn on.
fn write_aged(path: &Path, content: &str, age: Duration) {
    write(path, content);
    let f = std::fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - age).unwrap();
}

const OLD: Duration = Duration::from_secs(400 * 24 * 3600);
const RECENT: Duration = Duration::from_secs(24 * 3600);

/// A managed car with an active version in the library.
fn managed_car(conn: &Connection, library: &Path, id: &str, name: &str) -> PathBuf {
    let v = library.join("cars").join(id).join("v1");
    // The name the reindex after activation reads back.
    write(&v.join("ui").join("ui_car.json"), &format!(r#"{{"name":"{name}"}}"#));
    write(&v.join("data").join("car.ini"), "[HEADER]");
    write(&v.join("data").join("other.ini"), "[OTHER]");
    overlay::upsert_mod(conn, id, "Car", Some("RSS"), Some(name), "h", None, NOW).unwrap();
    overlay::insert_version(
        conn,
        &format!("{id}-v1"),
        id,
        Some("1.0"),
        None,
        NOW,
        &v.to_string_lossy(),
        None,
        "sig",
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    overlay::set_active_version(conn, id, &format!("{id}-v1")).unwrap();
    v
}

fn extra(library: &Path, owner: &str, rel: &str, content: &str, age: Duration) {
    write_aged(&library.join("extras").join("cars").join(owner).join(rel), content, age);
}

struct Fixture {
    _base: crate::testutil::TempDir,
    ac: PathBuf,
    library: PathBuf,
    conn: Connection,
    cfg: AppConfig,
}

fn fixture() -> Fixture {
    let base = crate::testutil::temp_dir("gamestate");
    let ac = base.join("ac");
    let library = base.join("library");
    for d in [
        "content/cars",
        "content/tracks",
        "system/shaders",
        "apps/python",
        "content/gui",
    ] {
        std::fs::create_dir_all(ac.join(d)).unwrap();
    }
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    let cfg = AppConfig {
        ac_install_path: Some(ac.clone()),
        library_path: Some(library.clone()),
        ..Default::default()
    };

    // --- Population 2 and 4: managed cars and their game additions ---------
    managed_car(&conn, &library, "rss_lanzo", "RSS GTM Lanzo V10");
    extra(
        &library,
        "rss_lanzo",
        "extension/config/cars/rss/lanzo.ini",
        "lanzo",
        RECENT,
    );
    extra(
        &library,
        "rss_lanzo",
        "extension/textures/common/rss/shared.dds",
        "new",
        RECENT,
    );
    extra(
        &library,
        "rss_lanzo",
        "extension/config/cars/rss/gone.ini",
        "gone",
        RECENT,
    );
    // An accented name, for the search (DOSSIER§7.2).
    extra(&library, "rss_lanzo", "extension/textures/Clément.dds", "c", RECENT);
    // A game file the mod replaces, after backup (§4.5.4).
    write_aged(&ac.join("system/shaders/pitbox_test.fx"), "original", OLD);
    extra(&library, "rss_lanzo", "system/shaders/pitbox_test.fx", "modded", RECENT);
    crate::activation::activate(&conn, &cfg, "rss_lanzo", None).unwrap();

    // Shares a file with the Lanzo, with an older copy: the Lanzo keeps it.
    managed_car(&conn, &library, "rss_forza", "RSS Formula Forza");
    extra(
        &library,
        "rss_forza",
        "extension/textures/common/rss/shared.dds",
        "old",
        OLD,
    );
    crate::activation::activate(&conn, &cfg, "rss_forza", None).unwrap();

    // In a CM zone, held by a newer foreign file: its copy waits.
    write_aged(&ac.join("extension/config/tracks/loaded/spa.ini"), "from CM", RECENT);
    managed_car(&conn, &library, "rss_hybrid", "RSS Formula Hybrid");
    extra(
        &library,
        "rss_hybrid",
        "extension/config/tracks/loaded/spa.ini",
        "mine",
        OLD,
    );
    crate::activation::activate(&conn, &cfg, "rss_hybrid", None).unwrap();

    // In the library, never activated.
    managed_car(&conn, &library, "rss_classic", "Formula Classic Disabled");

    // Drift after the deployment: a file rewritten, a file gone, a file added.
    let lanzo = ac.join("content/cars/rss_lanzo");
    std::fs::remove_file(lanzo.join("data/car.ini")).unwrap();
    write(&lanzo.join("data/car.ini"), "[HEADER] rewritten by another tool");
    std::fs::remove_file(lanzo.join("data/other.ini")).unwrap();
    write(&lanzo.join("preview_cm.jpg"), "jpg");
    std::fs::remove_file(ac.join("extension/config/cars/rss/gone.ini")).unwrap();

    // --- Population 1: links --------------------------------------------------
    for id in [
        "abarth500",
        "ks_ferrari_f2004",
        "bmw_1m",
        "bmw_m3_e30",
        "ks_mazda_mx5_cup",
    ] {
        write(&ac.join("content/cars").join(id).join("ui/ui_car.json"), "{}");
        overlay::upsert_stock_mod(&conn, id, "Car", None, Some(id), NOW, false).unwrap();
    }
    let store = library.join("skins/abarth500/red_custom");
    write(&store.join("livery.dds"), "dds");
    overlay::insert_sub_mod(
        &conn,
        "skin1",
        "SKIN",
        "abarth500",
        "red_custom",
        &store.to_string_lossy(),
        None,
        NOW,
    )
    .unwrap();
    std::fs::create_dir_all(ac.join("content/cars/abarth500/skins")).unwrap();
    crate::activation::create_junction(&ac.join("content/cars/abarth500/skins/red_custom"), &store).unwrap();
    // A junction whose target is gone.
    let gone = library.join("skins/abarth500/gone_skin");
    std::fs::create_dir_all(&gone).unwrap();
    overlay::insert_sub_mod(
        &conn,
        "skin2",
        "SKIN",
        "abarth500",
        "gone_skin",
        &gone.to_string_lossy(),
        None,
        NOW,
    )
    .unwrap();
    crate::activation::create_junction(&ac.join("content/cars/abarth500/skins/gone_skin"), &gone).unwrap();
    std::fs::remove_dir(&gone).unwrap();

    let app = library.join("apps/myapp");
    write(&app.join("myapp.py"), "print()");
    overlay::insert_app(&conn, "myapp", &app.to_string_lossy(), None, NOW).unwrap();
    crate::activation::create_junction(&ac.join("apps/python/myapp"), &app).unwrap();

    let hud = library.join("others/hud");
    write(&hud.join("content/gui/pitbox_hud/logo.png"), "png");
    overlay::insert_other_mod(&conn, "hud", &hud.to_string_lossy(), None, NOW).unwrap();
    let hud_link = ac.join("content/gui/pitbox_hud");
    crate::activation::create_junction(&hud_link, &hud.join("content/gui/pitbox_hud")).unwrap();
    overlay::set_other_active(&conn, "hud", true, &[hud_link.to_string_lossy().into_owned()]).unwrap();

    // --- Population 3: a replaced sound -------------------------------------
    write(&ac.join("content/cars/ks_mazda_mx5_cup/sfx/mx5.bank"), "bank");
    let sound = library.join("sounds/ks_mazda_mx5_cup/loud");
    write(&sound.join("mx5.bank"), "loud");
    overlay::insert_sub_mod(
        &conn,
        "snd1",
        "SOUND",
        "ks_mazda_mx5_cup",
        "loud",
        &sound.to_string_lossy(),
        None,
        NOW,
    )
    .unwrap();
    overlay::set_active_sound(&conn, "ks_mazda_mx5_cup", Some("snd1")).unwrap();

    // --- Populations 6 and 7, and the orphans -------------------------------
    write(&ac.join("content/cars/vrc_formula_na_2021/data.acd"), "acd");
    write(&ac.join("dwrite.dll"), "dll");
    write(&ac.join("system/cfg/assetto_corsa.ini"), "ini");
    let ghost = ac.join("content/cars/ghost_car");
    write(&ghost.join("data.acd"), "acd");
    write(
        &ghost.join(crate::deploy::MARKER_FILE),
        r#"{"mod_id":"ghost_car","kind":"Car","deployed_at":"x"}"#,
    );
    // An addition of a mod long gone, identical to what sits in the game.
    let stored = library.join("extras/cars/old_mod/content/fonts/old.ttf");
    write(&stored, "font");
    std::fs::create_dir_all(ac.join("content/fonts")).unwrap();
    std::fs::hard_link(&stored, ac.join("content/fonts/old.ttf")).unwrap();

    // A folder of 600 entries, served in slices of 500 (DOSSIER§8.1).
    for i in 0..600 {
        write(&ac.join(format!("content/texture/big/t{i:03}.dds")), "t");
    }

    Fixture {
        _base: base,
        ac,
        library,
        conn,
        cfg,
    }
}

fn scan(f: &Fixture) -> Arc<Index> {
    let store = Store::default();
    assert!(store.begin(), "the slot is free");
    let snap = Snapshot::load(&f.conn, &f.cfg, game_generation()).unwrap();
    run_scan(&store, snap, &|_| {});
    store.end(None);
    store.index().expect("the scan published an index")
}

fn class_at(idx: &Index, rel: &str) -> (Population, State, Option<Drift>) {
    let id = idx
        .tree
        .find(Path::new(rel))
        .unwrap_or_else(|| panic!("{rel} is in the tree"));
    let c = &idx.class[id as usize];
    (c.population, c.state, c.drift)
}

fn owner_at(idx: &Index, rel: &str) -> Option<String> {
    let id = idx.tree.find(Path::new(rel))?;
    idx.class[id as usize].owner.map(|o| idx.owners[o as usize].id.clone())
}

fn node(idx: &Index, rel: &str) -> NodeId {
    idx.tree.find(Path::new(rel)).unwrap()
}

/// Rule (DOSSIER§4.1 to DOSSIER§4.3): every path of the fixture gets the population,
/// the state and the kind of drift the spec gives it.
#[test]
fn every_case_of_the_tables_is_classified() {
    let f = fixture();
    let idx = scan(&f);
    use Drift::*;
    use Population as P;
    use State as S;
    let cases: &[(&str, P, S, Option<Drift>)] = &[
        // 1 - links
        ("content/cars/abarth500/skins/red_custom", P::Link, S::Posed, None),
        (
            "content/cars/abarth500/skins/gone_skin",
            P::Link,
            S::Drift,
            Some(BrokenLink),
        ),
        ("apps/python/myapp", P::Link, S::Posed, None),
        ("content/gui/pitbox_hud", P::Link, S::Posed, None),
        // 2 - deployed mod folder
        ("content/cars/rss_lanzo/ui/ui_car.json", P::ModFolder, S::Posed, None),
        (
            "content/cars/rss_lanzo/data/car.ini",
            P::ModFolder,
            S::Drift,
            Some(Modified),
        ),
        (
            "content/cars/rss_lanzo/data/other.ini",
            P::ModFolder,
            S::Drift,
            Some(Missing),
        ),
        ("content/cars/rss_lanzo/preview_cm.jpg", P::ModFolder, S::Nobody, None),
        ("content/cars/ghost_car/data.acd", P::ModFolder, S::Drift, Some(Orphan)),
        // 3 - replaced sound
        (
            "content/cars/ks_mazda_mx5_cup/sfx/mx5.bank",
            P::Sound,
            S::ReplacesGame,
            None,
        ),
        // 4 - game additions
        ("extension/config/cars/rss/lanzo.ini", P::Extra, S::Posed, None),
        ("extension/textures/common/rss/shared.dds", P::Extra, S::Posed, None),
        ("extension/config/cars/rss/gone.ini", P::Extra, S::Drift, Some(Missing)),
        ("system/shaders/pitbox_test.fx", P::Extra, S::ReplacesGame, None),
        ("extension/config/tracks/loaded/spa.ini", P::Extra, S::Waiting, None),
        // 5, 6, 7
        (
            "content/cars/ks_ferrari_f2004/ui/ui_car.json",
            P::Origin,
            S::Nobody,
            None,
        ),
        (
            "content/cars/vrc_formula_na_2021/data.acd",
            P::Unmanaged,
            S::Nobody,
            None,
        ),
        ("dwrite.dll", P::Rest, S::Nobody, None),
        ("content/fonts/old.ttf", P::Rest, S::Drift, Some(Orphan)),
    ];
    for (rel, population, state, drift) in cases {
        assert_eq!(
            class_at(&idx, rel),
            (*population, *state, *drift),
            "{rel}: population, state, drift"
        );
    }

    assert_eq!(
        owner_at(&idx, "extension/textures/common/rss/shared.dds").as_deref(),
        Some("rss_lanzo"),
        "the newer copy provides"
    );
    let shared = node(&idx, "extension/textures/common/rss/shared.dds");
    assert_eq!(
        idx.class[shared as usize].claimants, 2,
        "a shared file counts both claimants"
    );
    assert!(
        idx.class[node(&idx, "extension/config/tracks/loaded/spa.ini") as usize].cm_zone,
        "loaded/ is a CM zone"
    );
    assert!(
        !idx.tree
            .node(node(&idx, "content/cars/rss_lanzo/data/other.ini"))
            .present,
        "the missing file is not invented on the disk"
    );
    assert_eq!(
        owner_at(&idx, "content/cars/abarth500/skins/red_custom").as_deref(),
        Some("skin1")
    );
    assert_eq!(owner_at(&idx, "content/gui/pitbox_hud").as_deref(), Some("hud"));

    let summary = idx.summary();
    assert!(summary.replaces_game >= 2, "the shader and the sound replace the game");
    assert_eq!(summary.waiting, 1, "one copy waits");
    assert!(summary.drift >= 5, "modified, missing twice, broken, orphans");
}

/// Rule (DOSSIER§R1): the scan reads, it never writes - the game folder and the
/// library are identical byte for byte before and after.
#[test]
fn the_scan_leaves_the_disk_untouched() {
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, (bool, Vec<u8>, Option<SystemTime>)> {
        walkdir::WalkDir::new(root)
            .into_iter()
            .flatten()
            .map(|e| {
                let meta = std::fs::symlink_metadata(e.path()).unwrap();
                let bytes = if meta.is_file() {
                    std::fs::read(e.path()).unwrap()
                } else {
                    Vec::new()
                };
                let rel = e.path().strip_prefix(root).unwrap().to_path_buf();
                (rel, (meta.file_type().is_symlink(), bytes, meta.modified().ok()))
            })
            .collect()
    }
    let f = fixture();
    let before = (snapshot(&f.ac), snapshot(&f.library));
    let idx = scan(&f);
    assert!(idx.classified);
    let after = (snapshot(&f.ac), snapshot(&f.library));
    assert!(before.0 == after.0, "the game folder is identical after the scan");
    assert!(before.1 == after.1, "the library is identical after the scan");
}

/// Rule (DOSSIER§4.5, DOSSIER§8.1): what is entirely nobody's folds into one line per
/// folder - official cars under their own label -, an unmanaged mod keeps its
/// line, a State chip unfolds everything, and a large folder is served in
/// slices.
#[test]
fn nobody_folds_into_one_line_and_large_folders_come_in_slices() {
    let f = fixture();
    let idx = scan(&f);
    let cars = node(&idx, "content/cars");
    let page = idx.children(cars, &Filters::default(), false, 0, 500);
    let names: Vec<&str> = page.rows.iter().map(|r| r.name.as_str()).collect();
    for kept in [
        "abarth500",
        "ghost_car",
        "ks_mazda_mx5_cup",
        "rss_lanzo",
        "vrc_formula_na_2021",
    ] {
        assert!(names.contains(&kept), "{kept} keeps its line: {names:?}");
    }
    let group = page.group.expect("the intact official cars fold");
    assert_eq!(
        (group.count, group.label),
        (3, "originCars"),
        "three official cars, intact"
    );
    let members = idx.children(cars, &Filters::default(), true, 0, 500);
    assert_eq!(members.rows.len(), 3, "unfolding shows them in place");

    let nobody = Filters {
        states: vec![StateValue::Nobody],
        ..Default::default()
    };
    assert!(
        idx.children(cars, &nobody, false, 0, 500).group.is_none(),
        "a State chip unfolds"
    );

    let big = node(&idx, "content/texture/big");
    let slice = idx.children(big, &Filters::default(), true, 0, 500);
    assert_eq!((slice.rows.len(), slice.total), (500, 600), "the first slice of 500");
    let next = idx.children(big, &Filters::default(), true, 500, 500);
    assert_eq!(next.rows.len(), 100, "then the rest");
}

/// Rule (DOSSIER§6): a State chip keeps the files in that state and their
/// folders; Provenance keeps what an element claims or provides - a shared
/// file it claims without providing included.
#[test]
fn filters_prune_the_tree() {
    let f = fixture();
    let idx = scan(&f);
    let replaces = Filters {
        states: vec![StateValue::ReplacesGame],
        ..Default::default()
    };
    let root = idx.children(0, &replaces, false, 0, 500);
    let names: Vec<&str> = root.rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"system") && names.contains(&"content"),
        "folders holding a replaced file: {names:?}"
    );
    assert!(
        !names.contains(&"extension") && !names.contains(&"dwrite.dll"),
        "and nothing else: {names:?}"
    );

    let forza = Filters {
        owner: Some(OwnerRef {
            kind: OwnerKind::Car,
            id: "rss_forza".into(),
        }),
        ..Default::default()
    };
    let rss = node(&idx, "extension/textures/common/rss");
    let rows = idx.children(rss, &forza, false, 0, 500).rows;
    assert_eq!(rows.len(), 1, "its one claim");
    assert_eq!(
        rows[0].provider.as_deref(),
        Some("RSS GTM Lanzo V10"),
        "shown with the mod that provides it"
    );
    assert_eq!(rows[0].others, 1, "and the other claimant counted");

    let abarth = Filters {
        owner: Some(OwnerRef {
            kind: OwnerKind::Car,
            id: "abarth500".into(),
        }),
        ..Default::default()
    };
    let skins = node(&idx, "content/cars/abarth500/skins");
    assert_eq!(
        idx.children(skins, &abarth, false, 0, 500).rows.len(),
        2,
        "a car's provenance includes its liveries"
    );
}

/// Rule (DOSSIER§7): the search finds library elements laid or not, by name or
/// id, ignores case and accents, and a query with a separator searches paths.
#[test]
fn the_search_answers_do_i_have_this_mod() {
    let f = fixture();
    let idx = scan(&f);
    let none = Filters::default();
    let lim = SearchLimits::default();

    let lanzo = idx.search("lanzo", &none, lim);
    let hit = lanzo
        .mods
        .items
        .iter()
        .find(|m| m.owner.id == "rss_lanzo")
        .expect("found by name");
    assert_eq!(hit.presence, Presence::InGame);
    assert!(hit.drift, "in the game, with its drift");
    assert!(
        lanzo.dirs.items.iter().any(|d| d.row.name == "rss_lanzo"),
        "and its folder"
    );
    assert!(
        lanzo.files.items.iter().all(|f| f.row.name.contains("lanzo")),
        "a file is found by its own name, not by its folder's"
    );

    let classic = idx.search("classic disabled", &none, lim);
    assert_eq!(classic.mods.items.len(), 1, "every word required, in any order");
    assert_eq!(
        classic.mods.items[0].presence,
        Presence::Disabled,
        "found while disabled"
    );

    let f2004 = idx.search("F2004", &none, lim);
    assert_eq!(
        f2004.mods.items[0].presence,
        Presence::Origin,
        "official content says so"
    );

    assert_eq!(idx.search("clement", &none, lim).files.total, 1, "accents ignored");
    let path = idx.search(r"skins\red", &none, lim);
    assert_eq!(path.dirs.total, 1, "a path finds the livery");
    assert_eq!(path.files.total, 0, "not each of its files");
    assert_eq!(idx.search("x", &none, lim).mods.total, 0, "two characters minimum");
    assert_eq!(
        idx.search("zonda", &none, lim).mods.total,
        0,
        "nothing is an answer too"
    );
}

/// Rule (DOSSIER§8.2): the detail panel names the provider, the other
/// claimants and why they do not win, and what brings a replaced original back.
#[test]
fn the_detail_panel_explains_a_path() {
    let f = fixture();
    let idx = scan(&f);
    let shared = idx
        .detail(node(&idx, "extension/textures/common/rss/shared.dds"))
        .unwrap();
    assert_eq!(shared.claims.len(), 2);
    assert!(shared.claims[0].provided, "the provider first");
    assert_eq!(shared.claims[1].reason, Some("older"), "the other has an older copy");

    let shader = idx.detail(node(&idx, "system/shaders/pitbox_test.fx")).unwrap();
    assert!(
        matches!(shader.mechanism, Some(query::Mechanism::Replaced { .. })),
        "the original is kept"
    );
    assert_eq!(
        shader.revert.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(),
        ["rss_lanzo"]
    );

    let modified = idx.detail(node(&idx, "content/cars/rss_lanzo/data/car.ini")).unwrap();
    let drift = modified.drift.unwrap();
    assert_ne!(drift.expected_size, drift.actual_size, "the sizes that differ");

    let folder = idx.detail(node(&idx, "content/cars/rss_lanzo")).unwrap();
    assert_eq!(
        folder.folder.unwrap().version.as_deref(),
        Some("1.0"),
        "the deployed version"
    );
    assert_eq!(folder.contributors[0].owner.id, "rss_lanzo");

    assert_eq!(
        idx.reveal("content/cars/rss_lanzo/data").map(|c| c.len()),
        Some(4),
        "the chain down to a path"
    );
}

/// Rule (DOSSIER§5.4): a write by Pit Box makes the index stale.
#[test]
fn a_game_write_makes_the_index_stale() {
    let f = fixture();
    let store = Store::default();
    store.begin();
    run_scan(
        &store,
        Snapshot::load(&f.conn, &f.cfg, game_generation()).unwrap(),
        &|_| {},
    );
    store.end(None);
    assert!(!store.status().index.unwrap().stale);
    drop(GameWrite::begin());
    assert!(store.status().index.unwrap().stale, "an activation since the scan");
}

/// Rule (DOSSIER§9.1): the module imports no function that writes. Checked on
/// the source itself: one `use`, one inline path, would be enough to break the
/// rule without any test of the engine noticing.
#[test]
fn read_only_imports() {
    const FORBIDDEN: &[&str] = &[
        "fs::write",
        "fs::remove",
        "fs::copy",
        "fs::rename",
        "fs::create_dir",
        "fs::hard_link",
        "set_modified",
        "File::create",
        "OpenOptions",
        "deploy_tree",
        "compose_tree",
        "compose_layers_into",
        "link_or_copy",
        "remove_deployment",
        "create_junction",
        "create_file_link",
        "remove_junction",
        "gamebackup::",
        "extras::deploy(",
        "extras::undeploy(",
        "extras::force_one(",
        "extras::store(",
        "extras::sync(",
        "sync_pack(",
        "recompose(",
        "activation::activate",
        "activation::deactivate",
        "overlay::set_",
        "overlay::insert_",
        "overlay::delete_",
        "overlay::add_",
        "overlay::upsert",
        "overlay::mark_",
        "overlay::update_",
        "overlay::clear_",
        ".execute(",
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("gamestate");
    for file in ["mod.rs", "tree.rs", "snapshot.rs", "classify.rs", "query.rs"] {
        let src = std::fs::read_to_string(dir.join(file)).unwrap();
        for bad in FORBIDDEN {
            assert!(!src.contains(bad), "gamestate/{file} uses `{bad}`, which writes");
        }
    }
    let facade =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/gamestate.rs")).unwrap();
    for bad in FORBIDDEN {
        assert!(
            !facade.contains(bad),
            "commands/gamestate.rs uses `{bad}`, which writes"
        );
    }
}
