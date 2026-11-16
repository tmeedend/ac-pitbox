use super::cmcopy::tests::sample;
use super::*;

fn base() -> (crate::testutil::TempDir, Connection) {
    let dir = crate::testutil::temp_dir("sessionlog");
    let conn = overlay::open(&dir.join("overlay.sqlite")).expect("base");
    (dir, conn)
}

fn launch(conn: &Connection, kind: LaunchKind, car: &str, track: &str, at: i64) -> i64 {
    let setup = serde_json::json!({
        "car_id": car, "car_skin": null, "track_id": track, "track_layout": null, "session_type": "practice",
    });
    let parsed: crate::launch::RaceSetup = serde_json::from_value(setup.clone()).unwrap();
    let mut l = session_launch(kind, at, &setup, &parsed, None);
    if kind == LaunchKind::Gameshot {
        l.cars = vec![car.into()];
    }
    note_launch(conn, l).expect("noted")
}

fn count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM session_results", [], |r| r.get(0))
        .unwrap()
}

fn write_result(dir: &Path, sample: &[u8]) -> std::path::PathBuf {
    let path = dir.join("race_out.json");
    std::fs::write(&path, sample).unwrap();
    path
}

const LAPS: &[u8] = include_bytes!("../raceresult/samples/cm_practice_laps.json");
const TRACKDAY: &[u8] = include_bytes!("../raceresult/samples/cm_thumbnail_trackday.json");

// RESULTAT§3.1: when the game closes after a session of Pit Box, the game's
// file is read and the session recorded with its launch, whose setup can
// start it again.
#[test]
fn a_pitbox_session_is_recorded_when_the_game_closes() {
    let (dir, conn) = base();
    let copy = sample("laps");
    let id = launch(
        &conn,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        now_ms() - 60_000,
    );
    let file = write_result(&dir, LAPS);
    let closed = game_closed(&conn, Some(&file), now_ms(), None)
        .unwrap()
        .expect("a launch was open");
    assert_eq!(closed.launch.id, id);
    let session = closed.session_id.expect("recorded");
    let record = overlay::session_of_launch(&conn, id).unwrap().unwrap();
    assert_eq!(record.id, session);
    assert_eq!((record.origin.as_str(), record.mode.as_str()), ("pitbox", "practice"));
    assert_eq!(record.best_lap_ms, Some(161_626));
    assert!(overlay::open_launch(&conn).unwrap().is_none(), "the launch is closed");
    assert!(copy.result.any_lap_begun());
}

// RESULTAT§3.3: CM's copy of a Pit Box session is tied to it, never recorded a
// second time — whichever of the two is read first.
#[test]
fn the_copy_of_a_pitbox_session_is_attached_not_added() {
    let (dir, conn) = base();
    let copy = sample("laps");
    let id = launch(
        &conn,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        copy.at_ms - 600_000,
    );
    let file = write_result(&dir, LAPS);
    // The game's file first: the closing happened just before CM's copy.
    game_closed(&conn, Some(&file), copy.at_ms - 1_000, None).unwrap();
    assert_eq!(count(&conn), 1);
    assert_eq!(take_copy(&conn, &copy).unwrap(), "attached");
    assert_eq!(count(&conn), 1, "no second line");
    let record = overlay::session_of_launch(&conn, id).unwrap().unwrap();
    assert_eq!(record.cm_file.as_deref(), Some("260912-184107.json"));

    // The copy alone: Pit Box was closed while the game ran.
    let (_dir2, conn2) = base();
    let id2 = launch(
        &conn2,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        copy.at_ms - 600_000,
    );
    assert_eq!(take_copy(&conn2, &copy).unwrap(), "recorded");
    let record = overlay::session_of_launch(&conn2, id2)
        .unwrap()
        .expect("tied to its launch");
    assert_eq!(record.origin, "pitbox");
}

/// The game's file is read only when newer than the launch.
fn filetime_now(path: &Path) {
    let bytes = std::fs::read(path).unwrap();
    std::fs::write(path, bytes).unwrap();
}

// RESULTAT§3.4: a thumbnail session and a Story Mode session are never
// recorded, from either source.
#[test]
fn thumbnails_and_story_are_never_recorded() {
    let (dir, conn) = base();
    let shot = sample("trackday");
    launch(
        &conn,
        LaunchKind::Gameshot,
        shot.player_car(),
        "magione",
        shot.at_ms - 60_000,
    );
    let file = write_result(&dir, TRACKDAY);
    let closed = game_closed(&conn, Some(&file), shot.at_ms - 1_000, None)
        .unwrap()
        .unwrap();
    assert_eq!(
        closed.session_id, None,
        "the game's file is not read for a thumbnail session"
    );
    assert_eq!(take_copy(&conn, &shot).unwrap(), "thumbnail");

    let laps = sample("laps");
    launch(
        &conn,
        LaunchKind::Story,
        "ks_lamborghini_huracan_performante",
        "monza",
        laps.at_ms - 600_000,
    );
    assert_eq!(take_copy(&conn, &laps).unwrap(), "story", "driven, but a story's");
    assert_eq!(count(&conn), 0);
}

// RESULTAT§3.4: a session where no car began a lap is not recorded, even one
// CM started.
#[test]
fn a_session_without_a_lap_begun_is_not_recorded() {
    let (_dir, conn) = base();
    assert_eq!(take_copy(&conn, &sample("shot")).unwrap(), "no_lap");
    assert_eq!(count(&conn), 0);
}

// RESULTAT§3.2: a session started from CM, with no launch of Pit Box around
// it, is recorded with CM's preset; an online one too, as online.
#[test]
fn a_session_started_elsewhere_is_recorded_as_cm() {
    let (_dir, conn) = base();
    assert_eq!(take_copy(&conn, &sample("laps")).unwrap(), "recorded");
    assert_eq!(take_copy(&conn, &sample("online")).unwrap(), "recorded");
    let modes: Vec<(String, Option<String>, bool)> = conn
        .prepare(
            "SELECT mode, track_layout, quick_drive IS NOT NULL FROM session_results WHERE origin = 'cm' ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        modes,
        vec![
            ("practice".into(), None, true),
            ("online".into(), Some("akina_downhill".into()), false)
        ]
    );
}

// RESULTAT§3.3: a copy on another track than the launch around it is not that
// launch's: a session started in CM meanwhile.
#[test]
fn a_copy_on_another_track_is_not_the_launch() {
    let (_dir, conn) = base();
    let copy = sample("laps");
    launch(&conn, LaunchKind::Session, "some_car", "spa", copy.at_ms - 600_000);
    assert_eq!(take_copy(&conn, &copy).unwrap(), "recorded");
    let origin: String = conn
        .query_row("SELECT origin FROM session_results", [], |r| r.get(0))
        .unwrap();
    assert_eq!(origin, "cm");
}

// The scan reads a copy once: a second pass leaves it alone.
#[test]
fn the_scan_reads_each_copy_once() {
    let (dir, conn) = base();
    let sessions = dir.join("Sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("260912-184107.json"), LAPS).unwrap();
    std::fs::write(sessions.join("261115-210327.json"), TRACKDAY).unwrap();
    std::fs::write(sessions.join("broken.json"), b"not json").unwrap();
    let lock = |f: &mut dyn FnMut(&Connection)| f(&conn);
    assert_eq!(scan(&sessions, &lock), 3);
    assert_eq!(count(&conn), 1, "the practice with laps only");
    assert_eq!(scan(&sessions, &lock), 0, "nothing read twice");
}

// RESULTAT§4: the record before this session, and the badge only when it is
// beaten — a first time on the combo has none.
#[test]
fn a_personal_best_is_beaten_only_when_one_existed() {
    let (dir, conn) = base();
    let file = write_result(&dir, LAPS);
    let first = launch(
        &conn,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        now_ms() - 60_000,
    );
    game_closed(&conn, Some(&file), now_ms(), None).unwrap();
    let v = return_view(&conn, first, Some(&file), false).unwrap().unwrap();
    assert_eq!(
        (v.best_lap_ms, v.previous_best_ms, v.new_best),
        (Some(161_626), None, false)
    );
    assert!(v.can_again, "a session of Pit Box starts again");

    conn.execute("UPDATE session_results SET best_lap_ms = 170000", [])
        .unwrap();
    let second = launch(
        &conn,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        now_ms() - 30_000,
    );
    filetime_now(&file);
    game_closed(&conn, Some(&file), now_ms(), None).unwrap();
    let v = return_view(&conn, second, Some(&file), false).unwrap().unwrap();
    assert_eq!((v.previous_best_ms, v.new_best), (Some(170_000), true));
}

// RESULTAT§2: a launch whose game never came (CM refused it) does not take
// the next session, started from CM by the user: it closes, and nothing is
// recorded or shown under it.
#[test]
fn a_launch_without_its_game_does_not_take_a_later_session() {
    let (dir, conn) = base();
    let file = write_result(&dir, LAPS);
    let started = now_ms() - 3_600_000;
    launch(
        &conn,
        LaunchKind::Session,
        "ks_lamborghini_huracan_performante",
        "monza",
        started,
    );
    // The game seen starting an hour after the launch.
    let closed = game_closed(&conn, Some(&file), now_ms(), Some(now_ms() - 60_000)).unwrap();
    assert!(closed.is_none(), "not this launch's game");
    assert_eq!(count(&conn), 0);
    assert!(
        overlay::open_launch(&conn).unwrap().is_none(),
        "the stale launch is closed"
    );
}

// RESULTAT§3.1: the game's file of another car than the one sent is not the
// launch's session.
#[test]
fn a_result_of_another_car_is_not_the_launch_session() {
    let (dir, conn) = base();
    let file = write_result(&dir, LAPS);
    launch(&conn, LaunchKind::Session, "some_other_car", "monza", now_ms() - 60_000);
    let closed = game_closed(&conn, Some(&file), now_ms(), None).unwrap().unwrap();
    assert_eq!(closed.session_id, None);
    assert_eq!(count(&conn), 0);
}

// RESULTAT§3.4: a story may drive its own version of the car under another
// id; its copy is still the story's.
#[test]
fn a_story_copy_with_another_car_is_still_the_story() {
    let (_dir, conn) = base();
    let laps = sample("laps");
    launch(
        &conn,
        LaunchKind::Story,
        "story_version_of_the_car",
        "monza",
        laps.at_ms - 600_000,
    );
    assert_eq!(take_copy(&conn, &laps).unwrap(), "story");
    assert_eq!(count(&conn), 0);
}
