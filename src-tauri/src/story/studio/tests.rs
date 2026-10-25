use std::path::Path;

use serde::Serialize;
use story_engine::frame::{Frame, FRAMES};
use story_engine::load::ROSTER;
use story_engine::model::Chapter;
use story_engine::refs::NameKind;
use story_engine::scene::SCENES;
use story_engine::validate::Diagnostic;

use super::*;
use crate::errors;
use crate::story::history;
use crate::story::holdings::all_held;
use crate::testutil::temp_dir;

fn json<T: Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serializes")
}

fn chapter_of(folder: &Path, file: &str) -> (Chapter, String) {
    let open = open(folder, &all_held).expect("opens");
    let c = open
        .chapters
        .into_iter()
        .find(|c| c.file == file)
        .expect("chapter listed");
    (c.chapter.expect("readable"), c.stamp)
}

// "New story" opens on a story with nothing to fix, and the hub lists it
// (SPEC-story-mode, Create Your Own).
#[test]
fn a_new_story_is_listed_and_clean() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-stories");
    let folder = base.join("My Story");
    create(&config, &folder, "My story", "en", Frame::None, "Chapter 1").expect("created");
    let listed = list(&config);
    assert_eq!(listed.len(), 1, "listed once");
    assert_eq!(listed[0].title.as_deref(), Some("My story"));
    assert_eq!((listed[0].errors, listed[0].warnings), (0, 0), "nothing to fix");
    assert_eq!(story_id(&folder), "my_story", "id from the folder name");
}

// The editor never mixes a story into files it did not write.
#[test]
fn a_new_story_refuses_a_folder_that_is_not_empty() {
    let config = temp_dir("studio-config");
    let folder = temp_dir("studio-full");
    std::fs::write(folder.join("notes.txt"), "mine").unwrap();
    assert_eq!(
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap_err(),
        errors::STORY_FOLDER_NOT_EMPTY
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("notes.txt")).unwrap(),
        "mine",
        "the folder's own file untouched"
    );
    assert!(list(&config).is_empty(), "not listed");
}

// Forgetting a story only drops it from the list (SPEC-story-mode,
// Create Your Own): its files stay.
#[test]
fn forgetting_a_story_keeps_its_files() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    forget(&config, &folder).unwrap();
    assert!(list(&config).is_empty(), "no longer listed");
    assert!(folder.join("story.yaml").is_file(), "story.yaml still there");
    register(&config, &folder).expect("opens again");
    assert_eq!(list(&config).len(), 1);
    assert_eq!(
        register(&config, &folder.join("flow")).unwrap_err(),
        errors::STORY_NOT_A_STORY,
        "a folder without story.yaml is not a story"
    );
}

// A save writes the chapter, which reads back; the diagnostics follow.
#[test]
fn a_saved_chapter_reads_back_and_is_checked() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    chapter.nodes[0].next = Some("nowhere".into());
    let new_stamp = save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).expect("saved");
    let (back, read_stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    assert_eq!(read_stamp, new_stamp, "the returned stamp is the file's");
    assert_eq!(back.nodes[0].next.as_deref(), Some("nowhere"));
    let d = check(&folder, &all_held).diagnostics;
    assert!(
        d.iter()
            .any(|d| d.node.as_deref() == Some("start") && d.message.contains("nowhere")),
        "the broken link is reported on its node: {d:#?}"
    );
}

// A file changed outside Pit Box since the editor read it is not
// overwritten: the editor reloads it instead.
#[test]
fn a_chapter_changed_outside_is_never_overwritten() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let (chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    let path = folder.join("flow/01-chapter.yaml");
    let by_hand = format!("# typed by hand\n{}", std::fs::read_to_string(&path).unwrap());
    std::fs::write(&path, &by_hand).unwrap();
    assert_eq!(
        save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap_err(),
        errors::STORY_FILE_CHANGED
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), by_hand, "the hand edit kept");
    assert_eq!(
        save_file(&folder, "flow/01-chapter.yaml", None, json(&chapter)).unwrap_err(),
        errors::STORY_FILE_CHANGED,
        "a new chapter never replaces an existing file"
    );
}

// A sentence that does not read is refused with the engine's words, the
// ones the author needs to fix it.
#[test]
fn a_chapter_that_does_not_read_says_why_in_the_engine_words() {
    let bad = serde_json::json!({
        "id": "ch01",
        "nodes": [{ "id": "start", "beat": { "effects": ["money +=== 1"] } }]
    });
    let why = render("flow/01-chapter.yaml", bad).unwrap_err();
    assert!(why.contains("cannot read effect `money +=== 1`"), "{why}");
    let good = serde_json::json!({ "id": "ch01", "nodes": [{ "id": "start", "end": "done" }] });
    assert!(render("flow/01-chapter.yaml", good).is_ok());
    let bad_date = serde_json::json!({ "characters": [{ "id": "a", "relation": "warm", "age": 3 }] });
    assert!(
        render("world/characters.yaml", bad_date).unwrap_err().contains("age"),
        "an unknown key named, in a world file too"
    );
}

// Only the story's own files are written, inside the story: its chapters,
// story.yaml and the two world files.
#[test]
fn only_the_story_files_are_written() {
    let base = temp_dir("studio-story");
    for bad in [
        "flow/../story.yaml",
        "flow/sub/01.yaml",
        "flow/.yaml",
        "flow/01.txt",
        "flow/..\\x.yaml",
        "events/../story.yaml",
        "events/sub/race.yaml",
        "world/../story.yaml",
        "world/economy.yaml",
        "drivers/careers.yaml",
    ] {
        assert!(!is_chapter_path(bad), "{bad} is not a chapter");
        assert!(
            save_file(&base, bad, None, serde_json::json!({})).is_err(),
            "{bad} refused"
        );
        assert!(!base.join(bad).exists(), "{bad} not written");
    }
    assert!(is_chapter_path("flow/02-chapter.yaml"));
    assert!(is_event_path("events/race_01.yaml") && !is_chapter_path("events/race_01.yaml"));
    assert!(is_content_path("content/friend/invite.story") && is_content_path("content/a.story"));
    for bad in [
        "content/../story.yaml",
        "content/.hidden.story",
        "content//a.story",
        "content/.story",
        "flow/a.story",
    ] {
        assert!(!is_content_path(bad), "{bad} is not a content file the editor writes");
    }
    assert!(
        remove_file(&base, "flow/01-chapter.yaml", "").is_err(),
        "only an event's file is removed"
    );
}

// An event is created, edited and read back like a chapter; deleting it
// leaves the node citing it for the checks, and Ctrl+Z brings it back
// (SPEC-story-mode, Create Your Own).
#[test]
fn an_event_is_saved_removed_and_restored() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let file = "events/race_01.yaml";
    let event = serde_json::json!({
        "id": "race_01", "name": "Race 01", "type": "race",
        "session": { "track": "some_track", "car": "some_car", "laps": 8 },
        "success": ["position <= 3"],
        "on_success": { "effects": [], "advance": null },
        "on_failure": { "effects": [], "pool": null, "pick": 1 },
        "entry_fee": null,
    });
    let stamp = save_file(&folder, file, None, event).expect("created");
    let opened = open(&folder, &all_held).unwrap();
    let read = opened.events[0].value.as_ref().expect("reads");
    assert_eq!((read.id.as_str(), read.session.laps), ("race_01", Some(8)));
    assert_eq!(opened.events[0].stamp, stamp, "the stamp the save returned");

    let (mut chapter, cstamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    chapter.nodes[0].kind = story_engine::model::NodeKind::Event {
        event: "race_01".into(),
        on_failure: None,
    };
    save_file(&folder, "flow/01-chapter.yaml", Some(&cstamp), json(&chapter)).unwrap();
    assert!(
        check(&folder, &all_held).diagnostics.is_empty(),
        "{:#?}",
        check(&folder, &all_held)
    );

    assert_eq!(
        remove_file(&folder, file, "0").unwrap_err(),
        errors::STORY_FILE_CHANGED,
        "never removed on an old stamp"
    );
    remove_file(&folder, file, &stamp).expect("removed");
    assert!(!folder.join(file).exists());
    assert!(
        (check(&folder, &all_held).diagnostics.iter()).any(|d| d.message.contains("unknown event `race_01`")),
        "the node citing it is reported"
    );
    history::with(&folder, |h| h.undo(&folder)).unwrap();
    assert!(folder.join(file).exists(), "Ctrl+Z brings it back");
}

// Renaming an event moves its file and follows its node; Ctrl+Z puts
// both back at once.
#[test]
fn renaming_an_event_moves_its_file_and_undoes_whole() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    std::fs::create_dir_all(folder.join("events")).unwrap();
    std::fs::write(
        folder.join("events/race_01.yaml"),
        "id: race_01\ntype: race\nsession: { track: t, car: c }\n",
    )
    .unwrap();
    let (mut chapter, cstamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    chapter.nodes[0].kind = story_engine::model::NodeKind::Event {
        event: "race_01".into(),
        on_failure: None,
    };
    save_file(&folder, "flow/01-chapter.yaml", Some(&cstamp), json(&chapter)).unwrap();

    rename(&folder, NameKind::Event, "race_01", "opener").expect("renamed");
    assert!(!folder.join("events/race_01.yaml").exists(), "the old file is gone");
    assert!(folder.join("events/opener.yaml").exists(), "under its new name");
    assert!(
        check(&folder, &all_held).diagnostics.is_empty(),
        "{:#?}",
        check(&folder, &all_held)
    );

    history::with(&folder, |h| h.undo(&folder)).unwrap();
    assert!(folder.join("events/race_01.yaml").exists(), "back under its old name");
    assert!(!folder.join("events/opener.yaml").exists(), "the new one undone");
    assert!(
        check(&folder, &all_held).diagnostics.is_empty(),
        "the node follows back"
    );
}

// story.yaml and the world files are saved like a chapter: canonical,
// read back, guarded by their stamp; a world file the story did not have
// yet is created by its first save.
#[test]
fn story_yaml_and_world_files_save_and_read_back() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let opened = open(&folder, &all_held).unwrap();
    assert_eq!(opened.characters.stamp, "", "no characters file yet");
    assert!(opened.characters.value.unwrap().characters.is_empty(), "read as empty");
    let mut manifest = opened.manifest.value.expect("story.yaml reads");
    manifest.title = "Renamed".into();
    manifest.settings.failure = story_engine::model::FailureMode::Final;
    save_file(&folder, MANIFEST, Some(&opened.manifest.stamp), json(&manifest)).expect("saved");
    let characters = serde_json::json!({ "characters": [{ "id": "friend", "name": "A friend" }] });
    save_file(&folder, CHARACTERS, None, characters.clone()).expect("created");
    assert_eq!(
        save_file(&folder, CHARACTERS, None, characters).unwrap_err(),
        errors::STORY_FILE_CHANGED,
        "a second \"first save\" never replaces it"
    );
    let opened = open(&folder, &all_held).unwrap();
    let m = opened.manifest.value.unwrap();
    assert_eq!(m.title, "Renamed");
    assert_eq!(m.settings.failure, story_engine::model::FailureMode::Final);
    assert_eq!(opened.characters.value.unwrap().characters[0].id, "friend");
    assert_ne!(opened.characters.stamp, "", "now on disk");
    assert_eq!(list(&config)[0].title.as_deref(), Some("Renamed"), "the hub follows");
    assert!(check(&folder, &all_held).diagnostics.is_empty(), "still clean");
}

// The sample copied into Create Your Own opens clean, portraits
// included, and refuses a folder that is not empty like a new story.
#[test]
fn the_sample_is_copied_whole_into_an_empty_folder() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("sample");
    copy_sample(&config, &folder).expect("copied");
    let opened = open(&folder, &all_held).unwrap();
    assert!(opened.diagnostics.is_empty(), "clean: {:?}", opened.diagnostics);
    assert!(
        opened.media.iter().any(|m| m == "portraits/friend_neutral.png"),
        "with its portraits: {:?}",
        opened.media
    );
    assert_eq!(list(&config).len(), 1, "listed in Create Your Own");
    assert_eq!(
        copy_sample(&config, &folder).unwrap_err(),
        errors::STORY_FOLDER_NOT_EMPTY,
        "never over an existing story"
    );
}

// The roster goes through the editor like a world file: read as empty
// until its first save, then checked with the rest - here a grid naming
// a driver it does not have (SPEC-story-mode, Drivers).
#[test]
fn the_roster_is_created_read_back_and_checked() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let opened = open(&folder, &all_held).unwrap();
    assert_eq!(opened.drivers.stamp, "", "no roster yet");
    assert!(opened.drivers.value.unwrap().drivers.is_empty(), "read as empty");
    let event = serde_json::json!({
        "id": "r", "type": "race",
        "session": { "track": "t", "car": "c", "opponents": [{ "driver": "rival" }] },
    });
    save_file(&folder, "events/r.yaml", None, event).expect("event created");
    let unknown = |folder: &Path| {
        (check(folder, &all_held).diagnostics.iter()).any(|d| d.message.contains("unknown driver `rival`"))
    };
    assert!(unknown(&folder), "the grid names a driver the story lacks");
    let roster = serde_json::json!({ "drivers": [{ "id": "rival", "name": "R", "ai_level": 95 }] });
    save_file(&folder, ROSTER, None, roster).expect("roster created");
    let opened = open(&folder, &all_held).unwrap();
    let drivers = opened.drivers.value.expect("reads").drivers;
    assert_eq!((drivers[0].id.as_str(), drivers[0].ai_level), ("rival", Some(95)));
    assert!(!unknown(&folder), "the driver is known now");
    assert!(
        (opened.usages.iter()).any(|u| u.kind == NameKind::Driver && u.id == "rival" && u.usages.len() == 1),
        "the grid cites the driver"
    );
}

// A `.story` file goes through the editor like an event: created by its
// first save, read back as items, guarded by its stamp, and removed when
// its last item goes (SPEC-story-mode, Create Your Own).
#[test]
fn a_content_file_is_created_read_back_and_removed() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::Named("phone".into()), "C").unwrap();
    let file = "content/friend/ask.story";
    let doc = serde_json::json!({ "items": [{
        "header": { "id": "ask", "type": "message", "when": ["reputation >= 1"] },
        "body": "Coming?\n* (yes) [Yes.]\n* (no) [No.]\n"
    }] });
    let stamp = save_file(&folder, file, None, doc).expect("created");
    let opened = open(&folder, &all_held).unwrap();
    let entry = opened.content_files.iter().find(|f| f.file == file).expect("listed");
    let items = &entry.value.as_ref().expect("reads").items;
    assert_eq!(items[0].header.id, "ask");
    assert!(items[0].body.contains("* (yes) [Yes.]"), "the text as written");
    assert_eq!(opened.content[0].labels, ["yes", "no"], "its labels, for the flow");
    assert!(
        check(&folder, &all_held).diagnostics.is_empty(),
        "{:#?}",
        check(&folder, &all_held).diagnostics
    );
    remove_file(&folder, file, &stamp).expect("removed");
    assert!(!folder.join(file).exists());
}

// The Scenes screen (SPEC-story-mode, Create Your Own: Scenes): the
// story's places come with the story, and its preview asks the engine
// which variant shows at the moment the author set, on the place as the
// screen holds it.
#[test]
fn the_scenes_come_with_the_story_and_the_preview_picks_their_variant() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/story-engine/sample");
    let story = open(&sample, &all_held).expect("opens");
    let places = story.scenes.value.expect("the sample has scenes");
    let garage = places.scenes.iter().find(|p| p.id == "garage").expect("a garage");
    let garage = serde_json::to_value(garage).unwrap();
    let at = |period: &str, mood: Option<&str>| {
        let m: SceneMoment = serde_json::from_value(serde_json::json!({
            "season": "spring", "period": period, "weather": null, "mood": mood
        }))
        .unwrap();
        scene_pick(garage.clone(), &m).unwrap().map(|s| s.variant)
    };
    assert_eq!(at("morning", None).as_deref(), Some("day"), "the default, last");
    assert_eq!(at("night", Some("")).as_deref(), Some("night"), "an empty mood is none");
    assert_eq!(
        at("morning", Some("tense")).as_deref(),
        Some("tense"),
        "the first that holds"
    );
    let empty = serde_json::json!({ "id": "x", "variants": [] });
    let m: SceneMoment = serde_json::from_value(serde_json::json!({ "season": "winter", "period": "night" })).unwrap();
    assert_eq!(
        scene_pick(empty, &m).unwrap(),
        None,
        "a place with no variant shows nothing"
    );
    assert!(
        scene_pick(serde_json::json!({ "id": "x" }), &m).is_err(),
        "a place that does not read says why"
    );

    // The Scenes screen saves the file: a story without one gets it.
    let config = temp_dir("studio-scenes-config");
    let base = temp_dir("studio-scenes");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let places =
        serde_json::json!({ "scenes": [{ "id": "attic", "variants": [{ "id": "default", "file": "a.webp" }] }] });
    save_file(&folder, SCENES, None, places).expect("the editor writes scenes/scenes.yaml");
    let opened = open(&folder, &all_held).unwrap();
    assert_eq!(opened.scenes.value.unwrap().scenes[0].id, "attic", "read back");
}

// The Frame screen (SPEC-story-mode, Skins: how a frame gets its look)
// saves `frames/frames.yaml` and a skin in its own folder, removes a
// skin, and the sample's pictures pass the checks on their size.
#[test]
fn the_frame_screen_saves_frames_and_skins() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/story-engine/sample");
    let shown = open(&sample, &all_held).unwrap();
    assert_eq!(shown.diagnostics, [], "the sample, its pictures included");
    assert!(shown
        .skins
        .iter()
        .any(|s| s.file == "frames/skins/chat_blue/skin.yaml" && s.value.is_some()));
    assert!(shown
        .skin_files
        .contains(&"frames/skins/chat_blue/bubble_in.png".to_string()));

    let config = temp_dir("studio-frames-config");
    let base = temp_dir("studio-frames");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let frames = serde_json::json!({ "frames": [{ "id": "old", "kind": "phone", "apps": [{ "id": "sms", "template": "thread", "shows": ["message"], "skin": "green" }] }] });
    save_file(&folder, FRAMES, None, frames).expect("frames.yaml written");
    let skin = serde_json::json!({ "template": "thread", "colors": { "accent": "#00aa00" } });
    let path = "frames/skins/green/skin.yaml";
    let stamp = save_file(&folder, path, None, skin).expect("a skin written in its folder");
    let opened = open(&folder, &all_held).unwrap();
    assert_eq!(opened.frames.value.unwrap().frames[0].id, "old");
    assert_eq!(opened.diagnostics, [], "the app wears a skin that exists");
    remove_file(&folder, path, &stamp).expect("removed");
    assert!(!folder.join(path).exists());
    assert!(
        save_file(&folder, "frames/other.yaml", None, serde_json::json!({})).is_err(),
        "not a file the editor writes"
    );
}

// The preview (SPEC-story-mode, Create Your Own: Content): a dialogue on
// the scene, a message in its frame, and the checks' warning for an item
// no frame can show; the values it reads can be set, and what the
// conversation changes is listed.
#[test]
fn the_preview_picks_its_view_and_lists_what_can_be_set_and_what_changed() {
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/story-engine/sample");
    let talk = preview(&sample, "talk_01", &[], &["rest".into()]).expect("previews");
    assert_eq!(
        (talk.view, talk.characters.as_slice()),
        ("scene", &["friend".to_string()][..])
    );
    assert!(talk.ended, "`rest` ends it");
    let place = talk.scene.as_ref().expect("its own place");
    assert_eq!(
        (place.place.as_str(), place.variant.as_str()),
        ("garage", "day"),
        "the garage at the story's start, 09:00"
    );
    assert_eq!(
        talk.changes,
        [Change {
            var: "rel.friend".into(),
            from: "neutral".into(),
            to: "cold".into()
        }]
    );
    let invite = preview(&sample, "invite_01", &[], &[]).unwrap();
    assert_eq!((invite.view, invite.frame.as_deref()), ("frame", Some("basic_phone")));
    let app = invite.app.expect("the app showing it");
    assert_eq!(
        (app.id.as_str(), app.name.as_deref()),
        ("messages", Some("Messages")),
        "the basic phone's own"
    );
    assert!(invite.scene.is_none(), "a message has no place of its own");
    let offer = preview(&sample, "sponsor_offer", &["flag.sponsor_met = true".into()], &[]).unwrap();
    let knobs: Vec<(&str, &str)> = offer.knobs.iter().map(|k| (k.var.as_str(), k.kind)).collect();
    assert_eq!(knobs, [("reputation", "number"), ("flag.sponsor_met", "flag")]);
    assert_eq!(offer.knobs[1].start, "false", "the story's own start, whatever was set");

    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let doc =
        serde_json::json!({ "items": [{ "header": { "id": "note", "type": "mail", "when": [] }, "body": "Hi.\n" }] });
    save_file(&folder, "content/note.story", None, doc).unwrap();
    let note = preview(&folder, "note", &[], &[]).unwrap();
    assert_eq!(note.view, "nowhere", "a visual novel has nowhere to show a mail");
    assert!(
        note.nowhere.unwrap().contains("no frame it can arrive in"),
        "the checks' warning"
    );
    assert!(
        preview(&folder, "note", &["money +== 1".into()], &[]).is_err(),
        "a setting that does not read"
    );
}

// The Form view edits the tree the engine reads, and gets back the text
// the engine writes: the same conversation, in canonical form.
#[test]
fn a_script_goes_through_the_form_view_and_back() {
    let text = "friend: Hi.\n* (yes) [Yes.]\n    ~ effect(\"reputation += 1\")\n* (no) [No.]\n";
    let read = read_script(text);
    assert!(read.errors.is_empty(), "{:?}", read.errors);
    let tree = serde_json::to_value(&read.script).unwrap();
    assert_eq!(write_script(tree).unwrap(), text, "the canonical form of what was read");
    let broken = read_script("friend: Hi.\n-> away\n");
    assert_eq!(broken.errors[0].line, 2, "by line of the text");
    let bad = serde_json::json!({ "items": [{ "kind": "effect", "effect": "money +== 1" }] });
    assert!(
        write_script(bad).unwrap_err().contains("cannot read effect"),
        "in the engine's words"
    );
}

// A portrait is a file of the story's `media/`: the studio lists them for
// the picker, and its checks see one that points to nothing - including a
// picture added while the editor is open.
#[test]
fn the_studio_lists_media_and_checks_portraits_against_them() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let characters = serde_json::json!({ "characters": [{ "id": "friend", "portraits": {
        "neutral": "portraits/friend.png", "happy": "portraits/happy.png" } }] });
    save_file(&folder, CHARACTERS, None, characters).expect("saved");
    std::fs::create_dir_all(folder.join("media/portraits")).unwrap();
    std::fs::write(folder.join("media/portraits/friend.png"), b"png").unwrap();

    let opened = open(&folder, &all_held).unwrap();
    assert_eq!(opened.media, ["portraits/friend.png"], "listed inside media/");
    let missing =
        |d: &[Diagnostic]| (d.iter()).any(|x| x.message.contains("`media/portraits/happy.png` does not exist"));
    assert!(missing(&opened.diagnostics), "{:#?}", opened.diagnostics);

    std::fs::write(folder.join("media/portraits/happy.png"), b"png").unwrap();
    let checked = check(&folder, &all_held);
    assert_eq!(checked.media.len(), 2, "re-listed at the next check");
    assert!(checked.diagnostics.is_empty(), "{:#?}", checked.diagnostics);
}

// A rename reaches every file citing the name, and keeps what was typed
// by hand around it (SPEC-story-mode, Create Your Own).
#[test]
fn a_rename_follows_into_every_file_and_keeps_hand_edits() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::Named("phone".into()), "C").unwrap();
    save_file(
        &folder,
        VARIABLES,
        None,
        serde_json::json!({ "counters": { "visits": 0 } }),
    )
    .unwrap();
    let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    chapter.nodes[0].kind = story_engine::model::NodeKind::Beat(
        serde_json::from_value(serde_json::json!({ "effects": ["counter.visits += 1"] })).unwrap(),
    );
    save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap();
    let content = "---
id: remark
type: message
when: [\"counter.visits >= 2\"]
---

A line, indented by hand.
";
    std::fs::create_dir_all(folder.join("content")).unwrap();
    std::fs::write(folder.join("content/remark.story"), content).unwrap();
    let cited = |id: &str| {
        let u = check(&folder, &all_held).usages;
        u.into_iter().find(|n| n.id == id).map(|n| n.usages.len())
    };
    assert_eq!(cited("visits"), Some(2), "the beat and the reactive message");

    rename(&folder, NameKind::Counter, "visits", "trips").expect("renamed");
    assert_eq!(cited("trips"), Some(2), "both citations followed");
    assert_eq!(cited("visits"), None, "the old name is gone");
    assert_eq!(
        std::fs::read_to_string(folder.join("content/remark.story")).unwrap(),
        content.replace("counter.visits", "counter.trips"),
        "only the cited word changed in the hand-typed file"
    );
    assert!(
        check(&folder, &all_held).diagnostics.is_empty(),
        "clean: {:#?}",
        check(&folder, &all_held)
    );
    assert!(
        rename(&folder, NameKind::Counter, "trips", "9lives").is_err(),
        "an invalid name is refused"
    );
}

// Saves and renames land in one undo history, undone last first; a
// rename that touched several files comes back whole (SPEC-story-mode,
// Create Your Own).
#[test]
fn saves_and_renames_are_undone_last_first() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    let undo =
        |n: usize| (0..n).for_each(|_| assert!(!history::with(&folder, |h| h.undo(&folder)).unwrap().is_empty()));
    save_file(
        &folder,
        VARIABLES,
        None,
        serde_json::json!({ "counters": { "visits": 0 } }),
    )
    .unwrap();
    let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
    let original = std::fs::read_to_string(folder.join("flow/01-chapter.yaml")).unwrap();
    chapter.nodes[0].kind = story_engine::model::NodeKind::Beat(
        serde_json::from_value(serde_json::json!({ "effects": ["counter.visits += 1"] })).unwrap(),
    );
    save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap();
    rename(&folder, NameKind::Counter, "visits", "trips").unwrap();
    assert!(
        check(&folder, &all_held).history.can_undo,
        "the editor can offer Ctrl+Z"
    );

    undo(1);
    assert_eq!(
        check(&folder, &all_held).usages[0].id,
        "visits",
        "the rename undone: declaration"
    );
    assert!(
        std::fs::read_to_string(folder.join("flow/01-chapter.yaml"))
            .unwrap()
            .contains("counter.visits"),
        "and the sentence citing it"
    );
    undo(2);
    assert_eq!(
        std::fs::read_to_string(folder.join("flow/01-chapter.yaml")).unwrap(),
        original,
        "the chapter as created"
    );
    assert!(
        !folder.join(VARIABLES).exists(),
        "the world file the first save created is gone"
    );
    assert!(check(&folder, &all_held).history.can_redo && !check(&folder, &all_held).history.can_undo);
}

// One chapter that does not read hides neither the others nor itself:
// it is listed with its error.
#[test]
fn an_unreadable_chapter_is_listed_with_its_error() {
    let config = temp_dir("studio-config");
    let base = temp_dir("studio-story");
    let folder = base.join("s");
    create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
    std::fs::write(folder.join("flow/02-broken.yaml"), "id: ch02\nnodes: [oops\n").unwrap();
    let open = open(&folder, &all_held).unwrap();
    assert_eq!(open.chapters.len(), 2, "both chapters listed");
    assert!(open.chapters[0].chapter.is_some(), "the first still reads");
    assert!(open.chapters[1].error.is_some(), "the second carries its error");
    assert!(
        open.diagnostics.iter().any(|d| d.file == "flow/02-broken.yaml"),
        "and the check names it"
    );
}
