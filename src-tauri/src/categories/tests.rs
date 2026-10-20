use super::*;
use crate::config::AppConfig;
use crate::harmonize;
use crate::rules::{default_rules, TagMerge};
use std::path::PathBuf;

// --- pure rules ---------------------------------------------------------------

/// A tag written from a name, the way the creation of a track category writes
/// it (TAXO§6.4): the same cases as `tagForName` in `trackCategoryNames.test.ts`.
#[test]
fn a_category_is_written_from_a_name_like_a_new_track_category() {
    assert_eq!(slug_tag("Côte d'Azur").as_deref(), Some("#cote_d_azur"));
    assert_eq!(slug_tag("  GT3 ").as_deref(), Some("#gt3"));
    assert_eq!(slug_tag("##Drift").as_deref(), Some("#drift"));
    assert_eq!(slug_tag("!!!"), None, "nothing to file under");
}

/// A tag the rules know keeps its own spelling: `#formula 1` is not
/// `#formula_1`, and picking it must name the category the rules compute.
#[test]
fn a_known_tag_keeps_its_spelling_and_a_new_one_is_slugged() {
    let known = vec!["#formula 1".to_string(), "#can-am".to_string()];
    assert_eq!(canonical("#Formula 1", &known).as_deref(), Some("#formula 1"));
    assert_eq!(canonical("can-am", &known).as_deref(), Some("#can-am"));
    assert_eq!(canonical("Hot Rods", &known).as_deref(), Some("#hot_rods"));
}

fn car_decision(id: &str, forced: Option<&str>) -> CategoryDecision {
    CategoryDecision {
        forced: forced.map(str::to_string),
        ..CategoryDecision::new(id)
    }
}

/// REGLES§3 level 4: forcing the category the rules compute stores nothing and
/// forgets a forced one - the car follows the rules again.
#[test]
fn forcing_the_computed_category_stores_nothing_and_forgets_a_forced_one() {
    let auto = vec!["#gt3".to_string()];
    let d = decide(
        true,
        &auto,
        car_decision("c", None),
        &CategoryOp::Force("#GT3".into()),
        &[],
    )
    .unwrap();
    assert!(d.is_empty(), "the computed value is no decision");
    let d = decide(
        true,
        &auto,
        car_decision("c", Some("#custom")),
        &CategoryOp::Force("#gt3".into()),
        &[],
    )
    .unwrap();
    assert!(d.is_empty(), "choosing it again forgets the forced one");
    let d = decide(
        true,
        &auto,
        car_decision("c", None),
        &CategoryOp::Force("Hot Rods".into()),
        &[],
    )
    .unwrap();
    assert_eq!(
        d.forced.as_deref(),
        Some("#hot_rods"),
        "an unknown category is a valid one once forced"
    );
    let d = decide(true, &auto, d, &CategoryOp::Reset, &[]).unwrap();
    assert!(d.is_empty(), "reset forgets it");
}

/// A car takes a force, a track takes deltas: the wrong gesture is refused.
#[test]
fn a_gesture_of_the_other_kind_is_refused() {
    let d = CategoryDecision::new("x");
    assert!(decide(true, &[], d.clone(), &CategoryOp::Add("#a".into()), &[]).is_err());
    assert!(decide(false, &[], d, &CategoryOp::Force("#a".into()), &[]).is_err());
}

/// Adding a computed category does nothing, removing then adding it back
/// cancels, removing an added one forgets it.
#[test]
fn track_deltas_add_remove_and_cancel() {
    let auto = vec!["#touge".to_string(), "#drift".to_string()];
    let none = CategoryDecision::new("t");
    let d = decide(false, &auto, none.clone(), &CategoryOp::Add("#drift".into()), &[]).unwrap();
    assert!(d.is_empty(), "already computed: nothing to add");

    let d = decide(false, &auto, none.clone(), &CategoryOp::Remove("#touge".into()), &[]).unwrap();
    assert_eq!(d.removed, ["#touge"]);
    let d = decide(false, &auto, d, &CategoryOp::Add("#touge".into()), &[]).unwrap();
    assert!(d.is_empty(), "taking it off then putting it back cancels");

    let d = decide(false, &auto, none, &CategoryOp::Add("#rally".into()), &[]).unwrap();
    assert_eq!(d.added, ["#rally"]);
    let d = decide(false, &auto, d, &CategoryOp::Remove("#rally".into()), &[]).unwrap();
    assert!(d.is_empty(), "removing an added one only forgets it");
}

/// The categories added to a track fall in the order of Familles › Circuits;
/// the first stays the main one, and one the list does not know goes last.
#[test]
fn added_categories_follow_the_order_of_the_allowlist() {
    let order: Vec<String> = ["#rally", "#touge", "#drift", "#oval"].map(String::from).into();
    let auto = vec!["#touge".to_string(), "#drift".to_string()];
    let d = CategoryDecision {
        added: vec!["#oval".into(), "#rally".into(), "#hot_rods".into()],
        removed: vec!["#touge".into()],
        ..CategoryDecision::new("t")
    };
    let (main, all) = shown(false, None, &auto, Some(&d), &order);
    assert_eq!(all, ["#rally", "#drift", "#oval", "#hot_rods"]);
    assert_eq!(main.as_deref(), Some("#rally"), "rank 1 is the main category");
}

/// REGLES§13.5 at its smallest: with no decision the computed result goes
/// through untouched - its order and its spelling too, not a normalised copy.
#[test]
fn nothing_decided_nothing_touched() {
    let order: Vec<String> = ["#drift", "#rally"].map(String::from).into();
    let auto = vec!["#Rally".to_string(), "#drift".to_string()];
    let (main, all) = shown(false, Some("#Rally"), &auto, None, &order);
    assert_eq!(all, auto, "same order, same spelling");
    assert_eq!(main.as_deref(), Some("#Rally"));
    let (car, none) = shown(true, Some("#GT3"), &[], None, &order);
    assert_eq!((car.as_deref(), none.len()), (Some("#GT3"), 0));
}

// --- on a base ----------------------------------------------------------------

struct Lib {
    _dir: crate::testutil::TempDir,
    lib: PathBuf,
    conn: rusqlite::Connection,
    cfg: AppConfig,
}

fn lib() -> Lib {
    let dir = crate::testutil::temp_dir("categories");
    let lib = dir.join("lib");
    let conn = overlay::open(&dir.join("overlay.sqlite")).unwrap();
    let cfg = AppConfig {
        library_path: Some(lib.clone()),
        ..Default::default()
    };
    Lib {
        _dir: dir,
        lib,
        conn,
        cfg,
    }
}

impl Lib {
    /// A version of a mod with its `ui_*.json`; the first one added is active.
    fn add(&self, kind: &str, id: &str, version: &str, tags: &[&str]) -> PathBuf {
        let dir: PathBuf = self.lib.join(kind).join(id).join(version);
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        let file = if kind == "Car" { "ui_car.json" } else { "ui_track.json" };
        let json = serde_json::json!({ "name": id, "tags": tags });
        std::fs::write(dir.join("ui").join(file), json.to_string()).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&self.conn, id, kind, None, Some(id), "h", None, &now).unwrap();
        let vid = format!("{id}_{version}");
        overlay::insert_version(
            &self.conn,
            &vid,
            id,
            Some(version),
            None,
            &now,
            &dir.to_string_lossy(),
            None,
            &vid,
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        if overlay::get_mod(&self.conn, id)
            .unwrap()
            .unwrap()
            .active_version_id
            .is_none()
        {
            overlay::set_active_version(&self.conn, id, &vid).unwrap();
        }
        dir
    }

    fn row(&self, id: &str) -> overlay::ModRow {
        overlay::get_mod(&self.conn, id).unwrap().expect("mod listed")
    }

    fn harmonize(&self, rules: &Rules) {
        harmonize::harmonize_all(&self.conn, &self.cfg, rules).unwrap();
    }

    fn op(&self, rules: &Rules, ids: &[&str], op: CategoryOp) {
        let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
        apply(&self.conn, rules, &ids, &op).unwrap();
    }
}

/// REGLES§3 level 4, the fourth trigger included: a decision wins over an
/// update of the mod (`store`), a change of active version (`harmonize_mod`),
/// and a re-application of the rules (`harmonize_all`) - the call the "reapply"
/// button, every edit of a rule and the startup pass for a new engine all make.
#[test]
fn a_decision_survives_every_recomputation() {
    let l = lib();
    let rules = default_rules();
    l.add("Car", "car", "v1", &["#gt3"]);
    l.add("Track", "track", "v1", &["drift", "touge"]);
    l.harmonize(&rules);
    assert_eq!(l.row("car").category.as_deref(), Some("#gt3"), "computed to begin with");
    assert_eq!(l.row("track").categories, ["#touge", "#drift"]);

    l.op(&rules, &["car"], CategoryOp::Force("#hot_rods".into()));
    l.op(&rules, &["track"], CategoryOp::Remove("#touge".into()));
    l.op(&rules, &["track"], CategoryOp::Add("#rally".into()));
    let car = l.row("car");
    assert_eq!(car.category.as_deref(), Some("#hot_rods"));
    assert_eq!(car.category_source, "manual");
    assert_eq!(
        car.category_auto.as_deref(),
        Some("#gt3"),
        "the computed one stays known"
    );
    assert_eq!(l.row("track").categories, ["#rally", "#drift"]);

    // An import or an update of the mod: the harmonisation stores a new result.
    let h = harmonize::compute(&rules, ModKind::Car, &["#gt4".to_string()], "car", "", None, None);
    harmonize::store(&l.conn, "car", &h, None, &rules).unwrap();
    let h = harmonize::compute(
        &rules,
        ModKind::Track,
        &["drift".to_string(), "rallycross".to_string()],
        "track",
        "",
        None,
        None,
    );
    harmonize::store(&l.conn, "track", &h, None, &rules).unwrap();
    assert_eq!(l.row("car").category.as_deref(), Some("#hot_rods"), "update");
    assert_eq!(l.row("car").category_auto.as_deref(), Some("#gt4"));
    assert_eq!(l.row("track").categories, ["#rally", "#rallycross", "#drift"], "update");

    // Another version becomes the active one: its tags go through the rules.
    l.add("Car", "car", "v2", &["#lmp1"]);
    l.add("Track", "track", "v2", &["oval"]);
    for id in ["car", "track"] {
        overlay::set_active_version(&l.conn, id, &format!("{id}_v2")).unwrap();
        harmonize::harmonize_mod(&l.conn, &l.cfg, &rules, id).unwrap();
    }
    assert_eq!(l.row("car").category.as_deref(), Some("#hot_rods"), "version change");
    assert_eq!(l.row("car").category_auto.as_deref(), Some("#lmp1"));
    assert_eq!(l.row("track").categories, ["#rally", "#oval"], "version change");

    // The rules change and are re-applied to the library.
    let mut other = rules.clone();
    other.car.tag_merge.push(TagMerge {
        id: Some("t.merge".into()),
        from: vec!["#lmp1".into()],
        to: vec!["#sportscars".into()],
    });
    l.harmonize(&other);
    assert_eq!(l.row("car").category.as_deref(), Some("#hot_rods"), "re-applied rules");
    assert_eq!(l.row("car").category_auto.as_deref(), Some("#sportscars"));
    assert_eq!(l.row("track").categories, ["#rally", "#oval"], "re-applied rules");
}

/// Choosing the category the rules compute stores nothing, and forgets the one
/// forced before: the car is followed by the rules again.
#[test]
fn choosing_the_computed_category_puts_the_car_back_under_the_rules() {
    let l = lib();
    let rules = default_rules();
    l.add("Car", "car", "v1", &["#gt3"]);
    l.harmonize(&rules);
    l.op(&rules, &["car"], CategoryOp::Force("#hot_rods".into()));
    assert!(overlay::get_category_decision(&l.conn, "car").unwrap().is_some());

    l.op(&rules, &["car"], CategoryOp::Force("#GT3".into()));
    assert!(
        overlay::get_category_decision(&l.conn, "car").unwrap().is_none(),
        "no row left"
    );
    let car = l.row("car");
    assert_eq!(
        (car.category.as_deref(), car.category_source.as_str()),
        (Some("#gt3"), "rules")
    );
}

/// A category the rules do not know is a category once forced: what the
/// pickers offer, so the Category filter and the grid pool find it.
#[test]
fn a_forced_unknown_category_is_offered_and_shown() {
    let l = lib();
    let rules = default_rules();
    l.add("Car", "car", "v1", &["#gt3"]);
    l.add("Track", "track", "v1", &["drift"]);
    l.harmonize(&rules);
    l.op(&rules, &["car"], CategoryOp::Force("Hot Rods".into()));
    l.op(&rules, &["track"], CategoryOp::Add("Hill Cars".into()));

    let cars = vocabulary(&l.conn, &rules, ModKind::Car).unwrap();
    assert!(cars.contains(&"#hot_rods".to_string()), "decided on another mod");
    assert!(cars.contains(&"#gt3".to_string()), "what the rules produce");
    let tracks = vocabulary(&l.conn, &rules, ModKind::Track).unwrap();
    assert_eq!(tracks[0], "#rally", "the allowlist first, in its order");
    assert_eq!(tracks.last().map(String::as_str), Some("#hill_cars"));
    assert_eq!(
        l.row("car").category.as_deref(),
        Some("#hot_rods"),
        "what the filter and the grid read"
    );
}

/// A bulk gesture reaches every mod of the selection and is all or nothing;
/// reset clears the decisions.
#[test]
fn a_bulk_gesture_reaches_every_track_and_can_be_reset() {
    let l = lib();
    let rules = default_rules();
    l.add("Track", "a", "v1", &["drift"]);
    l.add("Track", "b", "v1", &["touge"]);
    l.harmonize(&rules);

    l.op(&rules, &["a", "b"], CategoryOp::Add("#rally".into()));
    assert_eq!(l.row("a").categories, ["#rally", "#drift"]);
    assert_eq!(l.row("b").categories, ["#rally", "#touge"]);
    l.op(&rules, &["a", "b"], CategoryOp::Remove("#drift".into()));
    assert_eq!(l.row("a").categories, ["#rally"]);
    assert_eq!(l.row("b").categories, ["#rally", "#touge"], "nothing to take off there");

    let bad = ["a".to_string(), "ghost".to_string()];
    assert!(apply(&l.conn, &rules, &bad, &CategoryOp::Reset).is_err());
    assert_eq!(
        l.row("a").categories,
        ["#rally"],
        "a failure leaves the first one as it was"
    );

    l.op(&rules, &["a", "b"], CategoryOp::Reset);
    assert_eq!(l.row("a").categories, ["#drift"]);
    assert_eq!(l.row("b").categories, ["#touge"]);
    assert!(overlay::list_category_decisions(&l.conn).unwrap().is_empty());
}

/// REGLES§13.5: a library with no decision is classified exactly as before -
/// what is shown IS what the rules compute, mod by mod.
#[test]
fn a_library_without_decisions_is_classified_exactly_as_before() {
    let l = lib();
    let rules = default_rules();
    l.add("Car", "c1", "v1", &["#gt3"]);
    l.add("Car", "c2", "v1", &["vintage"]);
    l.add("Track", "t1", "v1", &["drift", "touge"]);
    l.add("Track", "t2", "v1", &[]);
    l.harmonize(&rules);
    for m in overlay::list_mods(&l.conn).unwrap() {
        let (_, h) = harmonize::read_and_compute(&l.conn, &l.cfg, &rules, &m).expect("files");
        assert_eq!(m.category, h.category, "{}", m.id_interne);
        assert_eq!(m.categories, h.categories, "{}", m.id_interne);
        assert_eq!(m.category, m.category_auto, "{}", m.id_interne);
        assert_eq!(m.category_source, "rules", "{}", m.id_interne);
    }
}

/// REGLES§8.3: a rule whose result a decision hides is counted apart, so an
/// overridden rule does not look useless.
#[test]
fn an_overridden_rule_counts_apart_the_mods_where_it_is_masked() {
    let l = lib();
    let mut rules = default_rules();
    rules.car.tag_merge.push(TagMerge {
        id: Some("t.merge".into()),
        from: vec!["gtthree".into()],
        to: vec!["#gt3".into()],
    });
    l.add("Car", "forced", "v1", &["gtthree"]);
    l.add("Car", "followed", "v1", &["gtthree"]);
    l.harmonize(&rules);
    l.op(&rules, &["forced"], CategoryOp::Force("#hot_rods".into()));

    let fx = harmonize::effects(&l.conn, &l.cfg, &rules).unwrap();
    assert_eq!(fx.get("t.merge").copied(), Some(2), "it acts on both");
    assert_eq!(fx.masked("t.merge"), 1, "and one of them has it hidden");

    // Forcing the very category the rule produces hides nothing.
    l.op(&rules, &["forced"], CategoryOp::Force("#gt3".into()));
    let fx = harmonize::effects(&l.conn, &l.cfg, &rules).unwrap();
    assert_eq!(fx.masked("t.merge"), 0);
}

/// FICHE§6.1's way: a decision has no foreign key, and only a complete
/// deletion of the mod removes it.
#[test]
fn a_complete_deletion_removes_the_decision() {
    let l = lib();
    let rules = default_rules();
    l.add("Car", "car", "v1", &["#gt3"]);
    l.harmonize(&rules);
    l.op(&rules, &["car"], CategoryOp::Force("#hot_rods".into()));
    overlay::delete_mod(&l.conn, "car").unwrap();
    assert!(overlay::get_category_decision(&l.conn, "car").unwrap().is_none());
}

/// A base written before the decisions: the table appears, the computed
/// columns are copied from `category` once, and the rows read as before.
#[test]
fn an_older_base_gains_the_table_and_the_computed_columns() {
    let dir = crate::testutil::temp_dir("categories-old");
    let path = dir.join("overlay.sqlite");
    let now = chrono::Local::now().to_rfc3339();
    {
        let conn = overlay::open(&path).unwrap();
        overlay::upsert_mod(&conn, "car", "Car", None, Some("car"), "h", None, &now).unwrap();
        overlay::upsert_mod(&conn, "track", "Track", None, Some("track"), "h", None, &now).unwrap();
        overlay::update_harmonization(&conn, "car", None, None, Some("#gt3"), &[], None, &[]).unwrap();
        overlay::update_harmonization(
            &conn,
            "track",
            None,
            None,
            Some("#drift"),
            &["#drift".to_string()],
            None,
            &[],
        )
        .unwrap();
        conn.execute("DROP TABLE category_user", []).unwrap();
        conn.execute("ALTER TABLE mods DROP COLUMN category_auto", []).unwrap();
        conn.execute("ALTER TABLE mods DROP COLUMN categories_auto", [])
            .unwrap();
    }
    let conn = overlay::open(&path).expect("opening an older base");
    let car = overlay::get_mod(&conn, "car").unwrap().unwrap();
    assert_eq!(
        car.category_auto.as_deref(),
        Some("#gt3"),
        "copied from what was computed"
    );
    let track = overlay::get_mod(&conn, "track").unwrap().unwrap();
    assert_eq!(track.categories_auto, ["#drift"]);
    assert_eq!(car.category_source, "rules");
    assert!(overlay::list_category_decisions(&conn).unwrap().is_empty());
}

/// An imported decision is laid on its mod at once, also when the
/// re-application that follows cannot read that mod (no files here): and the
/// category it hid is still the one "back to computed" returns to.
#[test]
fn an_imported_decision_is_laid_even_where_the_reapplication_skips() {
    let l = lib();
    let rules = default_rules();
    let now = chrono::Local::now().to_rfc3339();
    overlay::upsert_mod(&l.conn, "ghost", "Car", None, Some("ghost"), "h", None, &now).unwrap();
    overlay::update_harmonization(&l.conn, "ghost", None, None, Some("#gt3"), &[], None, &[]).unwrap();
    let incoming = vec![
        CategoryDecision {
            forced: Some(" #Hot_Rods ".into()),
            ..CategoryDecision::new("ghost")
        },
        CategoryDecision::new("empty"),
    ];
    assert_eq!(
        merge_decisions(&l.conn, &rules, &incoming).unwrap(),
        (1, 0),
        "an empty one is nothing"
    );
    let m = l.row("ghost");
    assert_eq!(
        m.category.as_deref(),
        Some("#hot_rods"),
        "tidied and laid on what is shown"
    );
    assert_eq!(
        m.category_auto.as_deref(),
        Some("#gt3"),
        "what it hid is the computed one"
    );

    l.op(&rules, &["ghost"], CategoryOp::Force("#gt3".into()));
    assert!(overlay::get_category_decision(&l.conn, "ghost").unwrap().is_none());
    assert_eq!(
        l.row("ghost").category.as_deref(),
        Some("#gt3"),
        "back to computed clears it"
    );
}
