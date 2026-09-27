use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::json;

use super::*;
use crate::overlay;

// --- Fixtures ------------------------------------------------------------------------

const ENGINE_2_TURBOS: &str = "[ENGINE_DATA]\nLIMITER=8300\n[TURBO_0]\nMAX_BOOST=1\n[TURBO_1]\nMAX_BOOST=1\n";
const DRIVETRAIN: &str = "[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=6\n[GEARBOX]\nSUPPORTS_SHIFTER=0\n";

/// A car folder with a `ui_car.json` and an unpacked `data/`.
fn write_car(dir: &Path, ui: &serde_json::Value, data: &[(&str, &str)]) {
    std::fs::create_dir_all(dir.join("ui")).unwrap();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("ui").join("ui_car.json"), ui.to_string()).unwrap();
    for (name, text) in data {
        std::fs::write(dir.join("data").join(name), text).unwrap();
    }
}

fn gt2_ui() -> serde_json::Value {
    json!({
        "name": "Ferrari 458 GT2", "brand": "Ferrari", "country": "Italy", "year": 2011,
        "tags": ["rwd"],
        "specs": { "bhp": "470bhp", "torque": "520Nm", "weight": "1245kg", "topspeed": "270+km/h",
                   "acceleration": "--s 0-100", "pwratio": "2.65kg/hp", "range": 195 },
        "powerCurve": [["3000", "280"], ["8300", "455"]],
        "torqueCurve": [[3000, 470], [8300, 420]]
    })
}

/// A managed car, one version, recorded as the import records it.
fn managed_car(base: &Path, id: &str, ui: &serde_json::Value, data: &[(&str, &str)]) -> (Connection, PathBuf) {
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    let dir = base.join("lib").join("cars").join(id).join("v1");
    write_car(&dir, ui, data);
    add_version(&conn, id, "v1", &dir);
    (conn, dir)
}

fn add_version(conn: &Connection, id: &str, version: &str, dir: &Path) {
    let now = chrono::Local::now().to_rfc3339();
    overlay::upsert_mod(conn, id, "Car", None, Some(id), "h", None, &now).unwrap();
    overlay::insert_version(
        conn,
        version,
        id,
        None,
        None,
        &now,
        &dir.to_string_lossy(),
        None,
        "sig",
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    overlay::set_active_version(conn, id, version).unwrap();
    record(conn, id, version, dir, false).unwrap();
}

/// What `harmonize::store` does for a car, rules given.
fn harmonize(conn: &Connection, id: &str, h: &crate::rules::Harmonized, native_country: Option<&str>) {
    let rules = crate::rules::default_rules();
    crate::harmonize::store(conn, id, h, native_country, &rules).unwrap();
}

fn value(sheet: &TechSheet, f: &str) -> Option<(serde_json::Value, Source)> {
    sheet.values.get(f).map(|r| (r.value.clone(), r.source))
}

fn edit(f: &str, v: serde_json::Value) -> Edit {
    Edit {
        field: f.into(),
        value: v,
        revert: false,
    }
}

fn revert(f: &str) -> Edit {
    Edit {
        field: f.into(),
        value: serde_json::Value::Null,
        revert: true,
    }
}

fn row(conn: &Connection, id: &str) -> overlay::ModRow {
    overlay::get_mod(conn, id).unwrap().expect("mod listed")
}

// --- R2: the order of sources ---------------------------------------------------------

fn facts(list: &[(&str, Source, serde_json::Value)]) -> Facts {
    list.iter().map(|(f, s, v)| ((f.to_string(), *s), v.clone())).collect()
}

/// FICHE§9.4 — the user beats the physics, which beats `ui_car.json`, which
/// beats the tags; a user `NULL` forces "unknown" over all of them.
#[test]
fn the_user_beats_the_physics_which_beats_the_file_which_beats_the_tags() {
    let f = facts(&[
        ("drivetrain", Source::Physics, json!("AWD")),
        ("drivetrain", Source::Rules, json!("RWD")),
        ("year", Source::Ui, json!(2011)),
        ("year", Source::Table, json!(2010)),
        ("engine_config", Source::Rules, json!("V8")),
        ("aid.abs", Source::Physics, json!(true)),
    ]);
    let mut user = BTreeMap::new();
    let sheet = resolve(&f, &user);
    assert_eq!(
        value(&sheet, "drivetrain"),
        Some((json!("AWD"), Source::Physics)),
        "physics over tags"
    );
    assert_eq!(
        value(&sheet, "year"),
        Some((json!(2011), Source::Ui)),
        "the file over the table"
    );
    assert_eq!(
        value(&sheet, "engine_config"),
        Some((json!("V8"), Source::Rules)),
        "tags when alone"
    );

    user.insert("drivetrain".to_string(), Some(json!("FWD")));
    user.insert("aid.abs".to_string(), None);
    let sheet = resolve(&f, &user);
    assert_eq!(
        value(&sheet, "drivetrain"),
        Some((json!("FWD"), Source::User)),
        "the user over all"
    );
    assert_eq!(value(&sheet, "aid.abs"), None, "a user NULL forces unknown");
    assert_eq!(
        sheet.fallback.get("drivetrain").map(|r| (r.value.clone(), r.source)),
        Some((json!("AWD"), Source::Physics)),
        "what reverting would show"
    );
    assert_eq!(sheet.edited, vec!["aid.abs".to_string(), "drivetrain".to_string()]);
}

/// FICHE§4 — sequential, dual-clutch, automatic come from the tags even over
/// the physics; "no H-pattern" alone reads as paddles, an H-pattern as manual.
#[test]
fn the_gearbox_takes_the_tags_for_what_the_physics_cannot_tell() {
    let paddles = facts(&[("gearbox", Source::Physics, json!("PADDLES"))]);
    assert_eq!(
        value(&resolve(&paddles, &BTreeMap::new()), "gearbox").unwrap().0,
        json!("PADDLES")
    );

    let seq = facts(&[
        ("gearbox", Source::Physics, json!("PADDLES")),
        ("gearbox", Source::Rules, json!("SEQUENTIAL")),
    ]);
    assert_eq!(
        value(&resolve(&seq, &BTreeMap::new()), "gearbox"),
        Some((json!("SEQUENTIAL"), Source::Rules))
    );

    let manual = facts(&[
        ("gearbox", Source::Physics, json!("PADDLES")),
        ("gearbox", Source::Rules, json!("MANUAL")),
    ]);
    assert_eq!(
        value(&resolve(&manual, &BTreeMap::new()), "gearbox"),
        Some((json!("PADDLES"), Source::Physics)),
        "a manual the physics denies is not believed"
    );
}

/// FICHE§10 — a supercharger is no `[TURBO_n]`: "no turbo" leaves the word to
/// the tags; a turbo the physics does not have is not believed.
#[test]
fn a_supercharger_is_taken_from_the_tags_a_ghost_turbo_is_not() {
    let sc = facts(&[
        ("aspiration", Source::Physics, json!("NA")),
        ("aspiration", Source::Rules, json!("SUPERCHARGED")),
    ]);
    assert_eq!(
        value(&resolve(&sc, &BTreeMap::new()), "aspiration"),
        Some((json!("SUPERCHARGED"), Source::Rules))
    );
    // The Cayman GT4 of the reference install: tagged turbo, none in physics.
    let ghost = facts(&[
        ("aspiration", Source::Physics, json!("NA")),
        ("aspiration", Source::Rules, json!("TURBO")),
    ]);
    assert_eq!(
        value(&resolve(&ghost, &BTreeMap::new()), "aspiration"),
        Some((json!("NA"), Source::Physics))
    );
}

/// FICHE§4 — the ratio is worked out when the file does not give it, and only
/// from units that make it the same ratio.
#[test]
fn the_power_to_weight_ratio_is_computed_only_from_compatible_units() {
    let f = facts(&[
        ("power", Source::Ui, json!({"n": 500.0, "unit": "bhp"})),
        ("weight", Source::Ui, json!({"n": 1250.0, "unit": "kg"})),
    ]);
    let sheet = resolve(&f, &BTreeMap::new());
    assert_eq!(
        value(&sheet, "pwratio"),
        Some((json!({"n": 2.5, "unit": "kg/hp"}), Source::Computed))
    );
    let whp = facts(&[
        ("power", Source::Ui, json!({"n": 500.0, "unit": "whp"})),
        ("weight", Source::Ui, json!({"n": 1250.0, "unit": "kg"})),
    ]);
    assert!(
        !resolve(&whp, &BTreeMap::new()).values.contains_key("pwratio"),
        "whp is another ratio"
    );
}

// --- The files --------------------------------------------------------------------

/// FICHE§4 — what the files of a Kunos-like car give, end to end: numbers read,
/// placeholders gone, curves kept, physics words.
#[test]
fn a_car_reads_into_a_sheet() {
    let base = crate::testutil::temp_dir("techsheet-read");
    let (conn, _) = managed_car(
        &base,
        "gt2",
        &gt2_ui(),
        &[
            ("engine.ini", ENGINE_2_TURBOS),
            ("drivetrain.ini", DRIVETRAIN),
            ("electronics.ini", "[ABS]\nPRESENT=0\n[TRACTION_CONTROL]\nPRESENT=1\n"),
            ("car.ini", "[BASIC]\nTOTALMASS=1320\n[FUEL]\nMAX_FUEL=110\n"),
        ],
    );
    let sheet = effective(&conn, "gt2").unwrap();
    assert_eq!(
        value(&sheet, "power"),
        Some((json!({"n": 470.0, "unit": "bhp"}), Source::Ui))
    );
    assert_eq!(
        value(&sheet, "topspeed").unwrap().0,
        json!({"n": 270.0, "unit": "km/h", "plus": true})
    );
    assert!(!sheet.values.contains_key("acceleration"), "`--s 0-100` is absent");
    assert_eq!(value(&sheet, "range"), Some((json!(195.0), Source::Ui)));
    assert_eq!(value(&sheet, "gears"), Some((json!(6), Source::Physics)));
    assert_eq!(value(&sheet, "gearbox").unwrap().0, json!("PADDLES"));
    assert_eq!(value(&sheet, "fuel_tank"), Some((json!(110.0), Source::Physics)));
    assert_eq!(value(&sheet, "aid.abs").unwrap().0, json!(false), "no ABS is shown");
    assert_eq!(value(&sheet, "aid.tc").unwrap().0, json!(true));
    assert!(!sheet.values.contains_key("aid.drs"), "no DRS is not stored");
    assert_eq!(
        value(&sheet, "power_curve").unwrap().0,
        json!([[3000.0, 280.0], [8300.0, 455.0]])
    );
    assert!(!sheet.values.contains_key("_recorded"), "the marker is not a field");
}

/// FICHE§9.4 — two `[TURBO_n]` without another signal are a turbo, not a
/// biturbo; the name saying "twin turbo" confirms it.
#[test]
fn two_turbo_sections_make_a_biturbo_only_when_the_name_says_so() {
    let base = crate::testutil::temp_dir("techsheet-twin");
    let (conn, _) = managed_car(&base, "plain", &gt2_ui(), &[("engine.ini", ENGINE_2_TURBOS)]);
    assert_eq!(
        value(&effective(&conn, "plain").unwrap(), "aspiration").unwrap().0,
        json!("TURBO")
    );

    let mut ui = gt2_ui();
    ui["name"] = json!("Mitsubishi GTO Twin Turbo");
    let dir = base.join("lib").join("cars").join("gto").join("v1");
    write_car(&dir, &ui, &[("engine.ini", ENGINE_2_TURBOS)]);
    add_version(&conn, "gto", "gto_v1", &dir);
    assert_eq!(
        value(&effective(&conn, "gto").unwrap(), "aspiration").unwrap().0,
        json!("TWIN_TURBO")
    );
}

// --- R6: decisions survive -----------------------------------------------------------

/// FICHE§9.4 — a correction survives an update of the mod (a new version,
/// new physics) and a re-harmonisation.
#[test]
fn a_correction_survives_an_update_and_a_reharmonisation() {
    let base = crate::testutil::temp_dir("techsheet-survive");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[("drivetrain.ini", DRIVETRAIN)]);
    save_user(
        &conn,
        "car",
        vec![edit("gears", json!(7)), edit("engine_config", json!("V8"))],
    )
    .unwrap();

    // The author publishes a new version, with different physics.
    let v2 = base.join("lib").join("cars").join("car").join("v2");
    write_car(
        &v2,
        &gt2_ui(),
        &[("drivetrain.ini", &DRIVETRAIN.replace("COUNT=6", "COUNT=5"))],
    );
    add_version(&conn, "car", "v2", &v2);
    // And the rules are re-applied.
    let h = crate::rules::Harmonized {
        engine_config: Some("V12".into()),
        ..Default::default()
    };
    harmonize(&conn, "car", &h, None);

    let sheet = effective(&conn, "car").unwrap();
    assert_eq!(
        value(&sheet, "gears"),
        Some((json!(7), Source::User)),
        "the correction stands"
    );
    assert_eq!(value(&sheet, "engine_config"), Some((json!("V8"), Source::User)));

    save_user(&conn, "car", vec![revert("gears")]).unwrap();
    assert_eq!(
        value(&effective(&conn, "car").unwrap(), "gears"),
        Some((json!(5), Source::Physics)),
        "reverting goes back to the new version's physics"
    );
}

/// FICHE§9.4 — after a correction, the library's column reads it, with its
/// sign; a deduced value carries the tags' sign.
#[test]
fn the_library_columns_follow_the_sheet() {
    let base = crate::testutil::temp_dir("techsheet-cache");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[("drivetrain.ini", DRIVETRAIN)]);
    let h = crate::rules::Harmonized {
        drivetrain: Some("FWD".into()),
        engine_config: Some("V8".into()),
        ..Default::default()
    };
    harmonize(&conn, "car", &h, None);
    let m = row(&conn, "car");
    assert_eq!(m.drivetrain.as_deref(), Some("RWD"), "the physics, not the tag");
    assert_eq!(m.engine_config.as_deref(), Some("V8"));
    assert_eq!(m.tech_marks.get("engine_config").map(String::as_str), Some("rules"));
    assert!(!m.tech_marks.contains_key("drivetrain"), "read in the files: no sign");

    save_user(&conn, "car", vec![edit("drivetrain", json!("AWD"))]).unwrap();
    let m = row(&conn, "car");
    assert_eq!(m.drivetrain.as_deref(), Some("AWD"), "the filter finds the correction");
    assert_eq!(m.tech_marks.get("drivetrain").map(String::as_str), Some("user"));
}

/// FICHE§8 — a country chosen by the user survives a re-harmonisation and a
/// new alias, and the library files the car under it.
#[test]
fn a_chosen_country_survives_the_harmonisation() {
    let base = crate::testutil::temp_dir("techsheet-country");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[]);
    harmonize(&conn, "car", &Default::default(), Some("Italy"));
    let sheet = effective(&conn, "car").unwrap();
    assert_eq!(value(&sheet, "country"), Some((json!("Italy"), Source::Ui)));

    save_user(&conn, "car", vec![edit("country", json!("Japan"))]).unwrap();
    let mut rules = crate::rules::default_rules();
    rules.country_aliases.map.insert("italy".into(), "Germany".into());
    crate::harmonize::store(&conn, "car", &Default::default(), Some("Italy"), &rules).unwrap();

    assert_eq!(
        value(&effective(&conn, "car").unwrap(), "country").unwrap().0,
        json!("Japan")
    );
    assert_eq!(
        row(&conn, "car").country.as_deref(),
        Some("Japan"),
        "the index files it under the choice"
    );
}

/// A year typed on the sheet is the year the library shows and sorts on.
#[test]
fn a_corrected_year_reaches_the_library() {
    let base = crate::testutil::temp_dir("techsheet-year");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[]);
    save_user(&conn, "car", vec![edit("year", json!(1998))]).unwrap();
    assert_eq!(row(&conn, "car").year, Some(1998));
    // An emptied year is a decision too: the list follows the sheet.
    save_user(&conn, "car", vec![edit("year", json!(null))]).unwrap();
    assert_eq!(
        row(&conn, "car").year,
        None,
        "forced unknown, in the list as on the sheet"
    );
    save_user(&conn, "car", vec![revert("year")]).unwrap();
}

/// FICHE§8 — the history tells which fields changed, never the values.
#[test]
fn saving_notes_the_fields_in_the_history() {
    let base = crate::testutil::temp_dir("techsheet-history");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[]);
    save_user(
        &conn,
        "car",
        vec![edit("gearbox", json!("DCT")), edit("gears", json!(7))],
    )
    .unwrap();
    let h = overlay::get_history(&conn, "car").unwrap();
    let last = h.iter().find(|h| h.event == "TECH_EDITED").expect("noted");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&last.details).unwrap(),
        json!({"key": "techEdited", "fields": ["gearbox", "gears"]})
    );
}

/// A bad value leaves the sheet as it was, rather than half saved.
#[test]
fn a_bad_value_saves_nothing() {
    let base = crate::testutil::temp_dir("techsheet-bad");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[]);
    let err = save_user(
        &conn,
        "car",
        vec![edit("gears", json!(7)), edit("power", json!("lots"))],
    );
    assert!(err.is_err(), "a text is no power");
    assert!(effective(&conn, "car").unwrap().edited.is_empty(), "nothing written");
    assert!(
        save_user(&conn, "car", vec![edit("wheelbase", json!(2))]).is_err(),
        "not a field"
    );
}

// --- Housekeeping --------------------------------------------------------------------

/// FICHE§9.3 — the backfill reads what was never read, and then nothing.
#[test]
fn the_backfill_is_idempotent() {
    let base = crate::testutil::temp_dir("techsheet-backfill");
    let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
    let dir = base.join("lib").join("cars").join("old").join("v1");
    write_car(&dir, &gt2_ui(), &[]);
    let now = chrono::Local::now().to_rfc3339();
    overlay::upsert_mod(&conn, "old", "Car", None, Some("old"), "h", None, &now).unwrap();
    overlay::insert_version(
        &conn,
        "old_v1",
        "old",
        None,
        None,
        &now,
        &dir.to_string_lossy(),
        None,
        "sig",
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    overlay::set_active_version(&conn, "old", "old_v1").unwrap();
    let cfg = crate::config::AppConfig::default();

    let pending = pending_cars(&conn, &cfg).unwrap();
    assert_eq!(pending.len(), 1, "a car imported before the sheet existed");
    let p = &pending[0];
    store_files(&conn, &p.mod_id, &p.version, &read_files(&p.dir, &p.mod_id, p.stock)).unwrap();
    assert!(
        pending_cars(&conn, &cfg).unwrap().is_empty(),
        "nothing left the second time"
    );
}

/// Deleting a version takes its facts; a complete deletion takes the
/// decisions too — explicitly, since they have no foreign key.
#[test]
fn deletions_take_what_belongs_to_them() {
    let base = crate::testutil::temp_dir("techsheet-delete");
    let (conn, _) = managed_car(&base, "car", &gt2_ui(), &[("drivetrain.ini", DRIVETRAIN)]);
    save_user(&conn, "car", vec![edit("gears", json!(7))]).unwrap();
    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };

    overlay::delete_version(&conn, "v1").unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM tech_facts WHERE version_id = 'v1'"), 0);
    overlay::delete_mod(&conn, "car").unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM tech_facts"), 0, "facts cascade");
    assert_eq!(
        count("SELECT COUNT(*) FROM tech_user"),
        0,
        "decisions removed explicitly"
    );
}

/// A base written before the sheet existed opens, gains the tables, and
/// lists its mods — the mods listing reads `tech_user` for year and country.
#[test]
fn a_base_without_the_sheet_tables_gains_them() {
    let base = crate::testutil::temp_dir("techsheet-migrate");
    let path = base.join("overlay.sqlite");
    {
        let conn = overlay::open(&path).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "car", "Car", None, Some("car"), "h", Some(2004), &now).unwrap();
        conn.execute_batch("DROP TABLE tech_user; DROP TABLE tech_facts;")
            .unwrap();
        assert!(overlay::list_mods(&conn).is_err(), "the guard of the guard");
    }
    let conn = overlay::open(&path).unwrap();
    let mods = overlay::list_mods(&conn).expect("listed");
    assert_eq!(mods[0].year, Some(2004), "the year read before is still read");
    assert!(mods[0].tech_marks.is_empty(), "no sign on a car never read");
}

/// SESSION§3 — the session screen asks the sheet, a correction included.
#[test]
fn the_session_screen_reads_the_aids_from_the_sheet() {
    let base = crate::testutil::temp_dir("techsheet-session");
    let (conn, _) = managed_car(
        &base,
        "car",
        &gt2_ui(),
        &[("electronics.ini", "[ABS]\nPRESENT=0\n[TRACTION_CONTROL]\nPRESENT=1\n")],
    );
    let found = crate::electronics::read(&conn, "car").expect("the physics says");
    assert!(!found.abs && found.traction_control);
    save_user(&conn, "car", vec![edit("aid.abs", json!(true))]).unwrap();
    assert!(
        crate::electronics::read(&conn, "car").unwrap().abs,
        "the correction holds there too"
    );
    assert!(
        crate::electronics::read(&conn, "nothing").is_none(),
        "a car that does not say"
    );
}
