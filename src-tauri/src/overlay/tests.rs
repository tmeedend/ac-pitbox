use super::*;
/// Rule: every column a `*_SELECT` reads must be created by `init` **or**
/// added by `migrate` — on a brand-new database as well as an old one.
///
/// Ce test vient d'un bug réel, et son mode de défaillance est ce qui le
/// rend nécessaire : une colonne lue mais jamais créée fait échouer le
/// SELECT, et les appelants de ces listes sont pour la plupart
/// *best-effort* (`let _ = …`). Le mod s'importait donc normalement mais ne
/// s'activait plus, sans un mot nulle part. Rien dans le typage ne relie la
/// liste de colonnes du SELECT à celle des `ALTER`.
#[test]
fn every_listing_runs_on_a_fresh_database() {
    let base = crate::testutil::temp_dir("fresh-db");
    let conn = open(&base.join("overlay.sqlite")).unwrap();
    list_mods(&conn).expect("mods");
    list_other_mods(&conn).expect("other_mods");
    list_apps(&conn).expect("apps");
    list_subs_by_type(&conn, "SKIN").expect("sub_mods");
    list_layers(&conn, "x", HostKind::Track).expect("layers");
    pending_answer(&conn, "x", "cars", "Wallpapers").expect("pending_answers");
    list_all_extra_links(&conn).expect("extra_links");
    list_forced_extras(&conn).expect("forced_extras");
    list_game_backups(&conn).expect("game_backups");
    list_all_layers(&conn).expect("all layers");
    list_all_subs(&conn).expect("all sub_mods");
    drop(base);
}

/// Rule: what a `ui_*.json` stored before its reading dropped the invisible
/// characters is cleaned once - names, brands, lists - and nothing typed by
/// the user is touched; the pass then never runs again.
#[test]
fn stored_texts_lose_their_invisible_characters_once() {
    let base = crate::testutil::temp_dir("strip-invisible");
    let conn = open(&base.join("overlay.sqlite")).unwrap();
    upsert_mod(
        &conn,
        "nohesi_traffic_bmw_x5",
        "Car",
        Some("\u{1d17a}BMW"),
        Some("\u{1d17a}BMW X5 | No Hesi Traffic"),
        "h",
        None,
        "2026-10-10",
    )
    .unwrap();
    upsert_mod(
        &conn,
        "clean_car",
        "Car",
        Some("Ford"),
        Some("Ford GT"),
        "h2",
        None,
        "2026-10-10",
    )
    .unwrap();
    conn.execute(
        "UPDATE mods SET display_name_user = ?1 WHERE id_interne = 'clean_car'",
        ["\u{200b}typed by the user"],
    )
    .unwrap();

    assert_eq!(strip_invisible_from_stored_texts(&conn).unwrap(), 2, "brand and name");
    let m = get_mod(&conn, "nohesi_traffic_bmw_x5").unwrap().unwrap();
    assert_eq!(m.brand.as_deref(), Some("BMW"));
    assert_eq!(m.display_name.as_deref(), Some("BMW X5 | No Hesi Traffic"));
    let typed: String = conn
        .query_row(
            "SELECT display_name_user FROM mods WHERE id_interne = 'clean_car'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(typed, "\u{200b}typed by the user", "the user's input is left alone");

    conn.execute(
        "UPDATE mods SET display_name = ?1 WHERE id_interne = 'clean_car'",
        ["\u{1d17a}Ford GT"],
    )
    .unwrap();
    assert_eq!(strip_invisible_from_stored_texts(&conn).unwrap(), 0, "once per base");
}

/// Rule (§4.6ter): a database written before answers were remembered
/// opens, gains the table, and reads "no answer" for every folder.
#[test]
fn a_database_without_remembered_answers_gains_them_on_open() {
    let base = crate::testutil::temp_dir("pending-answers-migrate");
    let path = base.join("overlay.sqlite");
    {
        let conn = open(&path).unwrap();
        // The previous format: everything but this table.
        conn.execute("DROP TABLE pending_answers", []).unwrap();
    }
    let conn = open(&path).expect("an older database still opens");
    assert_eq!(
        pending_answer(&conn, "vrc_car", "cars", "Wallpapers").unwrap(),
        None,
        "nothing remembered yet"
    );
    remember_pending_answer(&conn, "vrc_car", "cars", "Wallpapers", "resources").unwrap();
    assert_eq!(
        pending_answer(&conn, "vrc_car", "cars", "wallpapers")
            .unwrap()
            .as_deref(),
        Some("resources"),
        "found again whatever the case of the folder name"
    );
    assert_eq!(
        pending_answer(&conn, "vrc_car", "tracks", "Wallpapers").unwrap(),
        None,
        "a track and a car sharing an id do not share answers"
    );
}

/// Règle (§4.4) : les couches d'une app et celles d'un mod ne se mélangent
/// pas, **même à id identique**.
///
/// Une voiture et un circuit ne peuvent pas porter le même id : ils vivent
/// dans `mods`, dont `id_interne` est la clé primaire. Une app, elle, vit
/// dans `apps` avec sa propre clé — rien n'empêche donc un circuit et une
/// app de s'appeler pareil. `parent_id` seul a cessé d'être une clé unique
/// le jour où les apps ont reçu des couches (§8.4) ; sans le filtre sur
/// l'espace de noms, `recompose` composerait les couches de l'app dans le
/// dossier du circuit.
#[test]
fn an_app_and_a_track_sharing_an_id_do_not_share_their_layers() {
    let base = crate::testutil::temp_dir("layer-namespace");
    let conn = open(&base.join("overlay.sqlite")).unwrap();
    let now = chrono::Local::now().to_rfc3339();

    // Le même id des deux côtés : c'est tout le sujet.
    upsert_mod(&conn, "shuto", "Track", None, Some("Shuto"), "h", None, &now).unwrap();
    insert_app(&conn, "shuto", "apps/shuto", None, &now).unwrap();

    insert_layer(
        &conn,
        "L_t",
        "shuto",
        "Track",
        "pour le circuit",
        "layers/tracks/shuto/a",
        None,
        1,
        0,
        0,
        &now,
    )
    .unwrap();
    insert_layer(
        &conn,
        "L_a",
        "shuto",
        "App",
        "pour l'app",
        "layers/apps/shuto/b",
        None,
        1,
        0,
        0,
        &now,
    )
    .unwrap();

    let t = list_layers(&conn, "shuto", HostKind::Track).unwrap();
    assert_eq!(t.len(), 1, "le circuit ne voit que la sienne");
    assert_eq!(t[0].id, "L_t");

    let a = list_layers(&conn, "shuto", HostKind::App).unwrap();
    assert_eq!(a.len(), 1, "l'app ne voit que la sienne");
    assert_eq!(a[0].id, "L_a");

    // Une voiture partage l'espace de noms des circuits : la distinction ne
    // porte que sur app / pas-app, et c'est suffisant par construction.
    assert_eq!(
        list_layers(&conn, "shuto", HostKind::Car).unwrap().len(),
        1,
        "Car et Track désignent le même espace de noms"
    );

    // Les priorités ne se marchent pas dessus non plus.
    assert_eq!(next_layer_priority(&conn, "shuto", HostKind::Track).unwrap(), 1);
    assert_eq!(next_layer_priority(&conn, "shuto", HostKind::App).unwrap(), 1);
}

/// Columns `migrate` adds, each paired with a listing that reads it. Used
/// by the migration test below to build an "old" database out of the
/// current one.
const ADDED_LATER: [(&str, &str); 10] = [
    ("mods", "is_unmanaged"),
    ("mods", "tech_marks"),
    ("layers", "notes_user"),
    ("sub_mods", "author"),
    ("other_mods", "attachment_user"),
    ("versions", "content_state"),
    ("layers", "content_state"),
    ("sub_mods", "content_state"),
    ("apps", "content_state"),
    ("other_mods", "content_state"),
];

/// Rule: `open` is safe to call on a database it has already migrated.
///
/// It runs `init` **and** `migrate` every single time — there is no version
/// marker — so idempotence is not a nicety, it is the whole design. Each
/// `ALTER` swallows its "duplicate column" error with `let _ =`, which is
/// exactly the shape that hides a real failure: if one of them ever started
/// failing for another reason, nothing would say so, and the damage would
/// surface somewhere else entirely.
#[test]
fn reopening_a_database_migrates_it_again_without_losing_anything() {
    let base = crate::testutil::temp_dir("db-reopen");
    let path = base.join("overlay.sqlite");
    let now = chrono::Local::now().to_rfc3339();

    {
        let conn = open(&path).unwrap();
        upsert_mod(
            &conn,
            "ks_mazda_mx5",
            "Car",
            Some("Mazda"),
            Some("MX-5"),
            "h",
            Some(2015),
            &now,
        )
        .unwrap();
    }

    // Second open: `init` re-runs against existing tables, `migrate` against
    // columns that are all already there.
    let conn = open(&path).expect("reopening an already-migrated database");
    let rows = list_mods(&conn).expect("the listing still runs");
    assert_eq!(rows.len(), 1, "the row survived the second migration");
    assert_eq!(
        rows[0].display_name.as_deref(),
        Some("MX-5"),
        "and so did its values — `init` must never recreate a table over one that holds data"
    );
}

/// Rule: a column added to `migrate` after the fact reaches a database
/// written before it existed.
///
/// This is the other half of `every_listing_runs_on_a_fresh_database`, and
/// the half that was never covered: `init` already creates every column
/// `migrate` adds, so on a fresh database the `ALTER`s all fail harmlessly
/// and **the migration path is never exercised at all**. Only an older
/// database runs it — which is precisely the case nobody has on hand.
///
/// The old shape is derived from the current one by dropping the columns,
/// never by pasting an old `CREATE TABLE`: a copy of the schema would rot
/// at the next change and quietly stop testing anything.
///
/// The failure mode is what makes this worth a test. A listing whose SELECT
/// names a missing column errors, and most callers of these listings are
/// best-effort (`let _ = …`), so the symptom lands miles from the cause —
/// a mod that imports normally and then never activates, without a word.
#[test]
fn a_column_added_after_the_fact_reaches_a_database_that_predates_it() {
    let base = crate::testutil::temp_dir("db-old");
    let path = base.join("overlay.sqlite");
    let now = chrono::Local::now().to_rfc3339();

    {
        let conn = open(&path).unwrap();
        upsert_mod(
            &conn,
            "ks_mazda_mx5",
            "Car",
            Some("Mazda"),
            Some("MX-5"),
            "h",
            Some(2015),
            &now,
        )
        .unwrap();
        for (table, col) in ADDED_LATER {
            conn.execute(&format!("ALTER TABLE {table} DROP COLUMN {col}"), [])
                .unwrap_or_else(|e| panic!("{table}.{col} should be droppable: {e}"));
        }
        // Guard the guard: if the listing still worked here, the test would
        // pass for the wrong reason and prove nothing about `migrate`.
        assert!(
            list_mods(&conn).is_err(),
            "a listing must break on the missing column — otherwise this test proves nothing"
        );
    }

    let conn = open(&path).expect("opening an older database");
    assert_eq!(
        list_mods(&conn).expect("mods").len(),
        1,
        "the migration put the column back, and the row is still there"
    );
    list_other_mods(&conn).expect("other_mods");
    list_apps(&conn).expect("apps");
    list_subs_by_type(&conn, "SKIN").expect("sub_mods");
    list_layers(&conn, "x", HostKind::Track).expect("layers");
}

/// Rule (ESPACE§5.6): an app and an "other" mod an older base holds have
/// their files. The state arrives with its default, and what the user typed on
/// them is still there.
#[test]
fn an_app_and_an_other_mod_written_before_read_as_complete() {
    let base = crate::testutil::temp_dir("db-addon-state");
    let path = base.join("overlay.sqlite");
    let now = chrono::Local::now().to_rfc3339();
    {
        let conn = open(&path).unwrap();
        insert_app(&conn, "MyApp", "apps/MyApp", Some("myapp.7z"), &now).unwrap();
        insert_other_mod(&conn, "Hud", "others/Hud", Some("hud.7z"), &now).unwrap();
        conn.execute("UPDATE apps SET notes_user = 'app note'", []).unwrap();
        conn.execute("UPDATE other_mods SET notes_user = 'mod note', is_priority = 1", [])
            .unwrap();
        for table in ["apps", "other_mods"] {
            for col in ["content_state", "freed_at"] {
                conn.execute(&format!("ALTER TABLE {table} DROP COLUMN {col}"), [])
                    .unwrap_or_else(|e| panic!("{table}.{col} should be droppable: {e}"));
            }
        }
    }
    let conn = open(&path).expect("an older database still opens");
    let app = get_app(&conn, "MyApp").unwrap().expect("the app is still there");
    assert_eq!(app.content_state, CONTENT_FULL, "an existing app keeps its files");
    assert_eq!(app.notes_user.as_deref(), Some("app note"));
    let other = get_other_mod(&conn, "Hud").unwrap().expect("the mod is still there");
    assert!(!other.is_skeleton(), "an existing mod keeps its files");
    assert_eq!(other.notes_user.as_deref(), Some("mod note"));
    assert!(other.is_priority);
}

/// Rule (ESPACE§5.3): the complete deletion takes what belonged to the
/// mod — journal, attached media, history —, and keeps its three
/// deliberate exceptions, for a reimport under the same id to find them:
/// usage, attached skins and sounds, the Wikipedia pairing.
#[test]
fn a_complete_deletion_takes_the_journal_and_keeps_its_three_exceptions() {
    let base = crate::testutil::temp_dir("db-delete-mod");
    let conn = open(&base.join("overlay.sqlite")).unwrap();
    let now = chrono::Local::now().to_rfc3339();
    upsert_mod(&conn, "car", "Car", None, Some("Car"), "h", None, &now).unwrap();
    add_history(&conn, "car", &now, "IMPORT", "").unwrap();
    record_decision(&conn, Some("car"), "car.7z", "extra", "extension/", None);
    record_decision(&conn, Some("other"), "other.7z", "extra", "extension/", None);
    let sql = [
        "INSERT INTO media_links (file_path, entity_id, kind) VALUES ('a.jpg', 'car', 'SCREENSHOT')",
        "INSERT INTO usage (mod_id, launched, launch_count) VALUES ('car', 1, 3)",
        "INSERT INTO wiki_link (mod_key, entity_id, source, resolved_at) VALUES ('car', 'Q1', 'manual', '')",
        "INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, imported_at)
         VALUES ('s1', 'SKIN', 'car', 'red', 'skins/car/red', '')",
    ];
    for s in sql {
        conn.execute(s, []).unwrap();
    }
    let count = |table: &str, column: &str, id: &str| -> i64 {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1"),
            [id],
            |r| r.get(0),
        )
        .unwrap()
    };

    delete_mod(&conn, "car").unwrap();

    assert_eq!(count("import_decisions", "mod_id", "car"), 0, "the import journal goes");
    assert_eq!(count("import_decisions", "mod_id", "other"), 1, "another mod's stays");
    assert_eq!(count("media_links", "entity_id", "car"), 0, "the attached media go");
    assert_eq!(count("history", "mod_id", "car"), 0, "the history goes");
    assert_eq!(count("usage", "mod_id", "car"), 1, "usage stays");
    assert_eq!(count("wiki_link", "mod_key", "car"), 1, "the Wikipedia pairing stays");
    assert_eq!(count("sub_mods", "parent_id", "car"), 1, "attached skins stay");
}

/// Rule (ESPACE§8.2): the origin of an archive fills what is unknown and
/// never replaces what is known — the copy a mod is recovered from later
/// must not erase where it was first downloaded.
#[test]
fn a_known_origin_is_never_replaced() {
    let base = crate::testutil::temp_dir("db-origin");
    let conn = open(&base.join("overlay.sqlite")).unwrap();
    let now = chrono::Local::now().to_rfc3339();
    upsert_mod(&conn, "car", "Car", None, Some("Car"), "h", None, &now).unwrap();
    insert_version(
        &conn,
        "v1",
        "car",
        None,
        None,
        &now,
        "cars/car/v1",
        None,
        "sig",
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
    let first = crate::archive::Origin {
        site: Some("https://www.overtake.gg/".into()),
        file_name: None,
    };
    set_version_origin(&conn, "v1", &first).unwrap();
    let later = crate::archive::Origin {
        site: Some("https://mirror.example/".into()),
        file_name: Some("car_v1.7z".into()),
    };
    set_version_origin(&conn, "v1", &later).unwrap();
    let v = get_version(&conn, "v1").unwrap().unwrap();
    assert_eq!(
        v.source_site.as_deref(),
        Some("https://www.overtake.gg/"),
        "the first site stays"
    );
    assert_eq!(
        v.source_file_name.as_deref(),
        Some("car_v1.7z"),
        "what was unknown is filled"
    );
}

/// Rule (ESPACE§4.1): every version an older base holds is complete. The
/// showcase columns arrive with their default, never a guess, and a
/// version read back is neither freed nor from a known site.
#[test]
fn a_version_written_before_the_showcase_reads_as_complete() {
    let base = crate::testutil::temp_dir("db-showcase");
    let path = base.join("overlay.sqlite");
    let now = chrono::Local::now().to_rfc3339();
    {
        let conn = open(&path).unwrap();
        upsert_mod(&conn, "car", "Car", None, Some("Car"), "h", None, &now).unwrap();
        insert_version(
            &conn,
            "v1",
            "car",
            Some("1.0"),
            None,
            &now,
            "cars/car/v1",
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        for col in [
            "content_state",
            "freed_at",
            "freed_bytes",
            "source_site",
            "source_file_name",
        ] {
            conn.execute(&format!("ALTER TABLE versions DROP COLUMN {col}"), [])
                .unwrap_or_else(|e| panic!("versions.{col} should be droppable: {e}"));
        }
    }
    let conn = open(&path).expect("an older database still opens");
    let v = get_version(&conn, "v1").unwrap().expect("the version is still there");
    assert_eq!(v.content_state, CONTENT_FULL, "an existing version keeps its files");
    assert!(!v.is_skeleton());
    assert_eq!(v.freed_at, None, "never freed");
    assert_eq!(v.freed_bytes, None);
    assert_eq!(v.source_site, None, "no origin invented");
    assert_eq!(v.source_file_name, None);
}
