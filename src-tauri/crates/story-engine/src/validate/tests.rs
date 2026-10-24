use super::*;
use crate::load::draft;
use crate::testutil::{minimal, with};

fn diagnostics(extra: &[(&str, &str)]) -> Vec<Diagnostic> {
    let mut files = minimal();
    for (path, text) in extra {
        files = with(files, path, text);
    }
    validate(&crate::Story::from_sources(files).expect("loads"))
}

fn has(diags: &[Diagnostic], severity: Severity, needle: &str) -> bool {
    diags
        .iter()
        .any(|d| d.severity == severity && d.message.contains(needle))
}

#[test]
fn the_minimal_story_is_clean() {
    assert_eq!(diagnostics(&[]), [], "nothing to say about a story that only ends");
}

// "Always validated": an unknown variable is an error, whatever the
// kind - flag, counter, character, event, content.
#[test]
fn unknown_variables_are_errors_where_they_are_written() {
    let flow = "id: ch01\nnodes:\n  - id: start\n    branch:\n      - if: [\"flag.ghost\", \"counter.nope >= 1\", \"rel.nobody >= warm\", \"event.none.attempts > 0\"]\n        goto: end\n    otherwise: end\n  - id: end\n    beat: { effects: [\"money += 5\"] }\n    next: start\n";
    let d = diagnostics(&[("flow/01-one.yaml", flow)]);
    for needle in [
        "unknown flag `ghost`",
        "unknown counter `nope`",
        "unknown character `nobody`",
        "unknown event `none`",
        "no economy",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
    assert!(
        d.iter().all(|x| x.file == "flow/01-one.yaml"),
        "reported on the chapter file"
    );
}

// The editor selects the node a diagnostic is about (Create Your Own,
// Validation): every diagnostic raised on a flow node names it, the
// others name none.
#[test]
fn a_diagnostic_on_a_flow_node_names_that_node() {
    let flow = "id: ch01
nodes:
  - id: start
    message: missing
  - id: end
    end: done
  - id: lost
    end: done
";
    let d = diagnostics(&[("flow/01-one.yaml", flow)]);
    let on = |needle: &str| d.iter().find(|x| x.message.contains(needle)).map(|x| x.node.clone());
    assert_eq!(
        on("unknown content"),
        Some(Some("start".into())),
        "an error on a node: {d:#?}"
    );
    assert_eq!(
        on("never reached"),
        Some(Some("lost".into())),
        "a warning on a node: {d:#?}"
    );
    let manifest = diagnostics(&[(
        "story.yaml",
        "format: 1
id: s
title: S
version: \"1\"
languages: []
start:
  node: start
  frame: phone
",
    )]);
    assert!(
        manifest.iter().all(|x| x.node.is_none()),
        "nothing about a node: {manifest:#?}"
    );
}

#[test]
fn values_must_fit_their_variable() {
    let vars = "flags: [met]\ncounters: { visits: 0 }\n";
    let chars = "characters:\n  - id: friend\n";
    let flow = "id: ch01\nnodes:\n  - id: start\n    beat: { effects: [\"flag.met += 1\", \"rel.friend = best\", \"date = 2000-01-01\", \"counter.visits = true\"] }\n  - id: b\n    branch:\n      - if: [\"flag.met >= true\", \"reputation\", \"rel.friend >= adored\", \"chapter >= ch09\"]\n        goto: e\n    otherwise: e\n  - id: e\n    end: done\n";
    let d = diagnostics(&[
        ("world/variables.yaml", vars),
        ("world/characters.yaml", chars),
        ("flow/01-one.yaml", flow),
    ]);
    for needle in [
        "`= true` or `= false`",
        "`best` is not on the relation scale",
        "story time moves with `advance`",
        "cannot take `true`",
        "needs an order",
        "not a yes/no value",
        "`adored` is not on the relation scale",
        "unknown chapter `ch09`",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
}

#[test]
fn broken_links_and_a_flow_falling_off_its_end_are_errors() {
    let flow = "id: ch01\nnodes:\n  - id: start\n    event: missing_event\n    on_failure: nowhere\n  - id: talk\n    message: missing_content\n";
    let d = diagnostics(&[("flow/01-one.yaml", flow)]);
    for needle in [
        "unknown event `missing_event`",
        "failure exit `nowhere` does not exist",
        "unknown content `missing_content`",
        "leads nowhere",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
}

#[test]
fn unreachable_nodes_and_unsent_content_are_warnings() {
    let flow = "id: ch01\nnodes:\n  - id: start\n    end: done\n  - id: draft\n    end: other\n";
    let content = "---\nid: never\ntype: message\n---\nx\n---\nid: orphan\ntype: message\npool: no_event\n---\ny\n";
    let d = diagnostics(&[("flow/01-one.yaml", flow), ("content/x.story", content)]);
    assert!(has(&d, Severity::Warning, "node `draft` is never reached"), "{d:#?}");
    assert!(has(&d, Severity::Warning, "content `never` is never sent"), "{d:#?}");
    assert!(
        has(&d, Severity::Warning, "no event draws from pool `no_event`"),
        "{d:#?}"
    );
    assert!(
        d.iter().all(|x| x.severity == Severity::Warning),
        "nothing blocking: {d:#?}"
    );
    let line = d.iter().find(|x| x.message.contains("orphan")).map(|x| x.file.as_str());
    assert_eq!(line, Some("content/x.story:6"), "content reported with its line");
}

#[test]
fn an_event_driven_by_two_nodes_is_a_warning() {
    let flow = "id: ch01
nodes:
  - id: start
    event: e
  - id: again
    event: e
  - id: end
    end: done
";
    let d = diagnostics(&[
        ("flow/01-one.yaml", flow),
        (
            "events/e.yaml",
            "id: e
type: race
",
        ),
    ]);
    assert!(has(&d, Severity::Warning, "already driven by node `start`"), "{d:#?}");
}

#[test]
fn content_placement_and_replies_are_checked() {
    let content = "---\nid: reactive\ntype: mail\nwhen: [\"reputation > 1\"]\n---\nx\n---\nid: plain\ntype: message\nfrom: stranger\n---\ny\n";
    let flow = "id: ch01\nnodes:\n  - id: start\n    message: reactive\n  - id: b\n    message: plain\n    wait_reply: true\n  - id: c\n    beat: { effects: [\"send reactive\"] }\n  - id: e\n    end: done\n";
    let d = diagnostics(&[("content/x.story", content), ("flow/01-one.yaml", flow)]);
    for needle in [
        "a message node sends flow content",
        "has no choices",
        "unknown character `stranger`",
        "`send` delivers flow content",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
}

#[test]
fn the_relation_scale_and_event_settings_are_checked() {
    let mut files = minimal();
    files[0]
        .text
        .push_str("settings:\n  relations: { scale: [a, b, a], start: z }\n");
    files = with(
        files,
        "events/e.yaml",
        "id: e\ntype: race\nentry_fee: 10\non_failure: { pool: empty, pick: 0 }\n",
    );
    let d = validate(&crate::Story::from_sources(files).unwrap());
    for needle in [
        "`a` appears twice",
        "start `z` is not on the scale",
        "needs an economy",
        "pool `empty` has no variant",
        "`pick: 0`",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
}

// An event names its track and its car, and what only a race has
// (laps, grid, opponents' skill, start) is refused on another type
// (SPEC-story-mode, Events, retries and failure).
#[test]
fn an_event_session_is_checked() {
    let flow = "id: ch01
nodes:
  - id: start
    event: lap
  - id: end
    end: done
";
    let event = "id: lap
type: hot_lap
session: { layout: gp, laps: 5, ai_level: 120 }
";
    let d = diagnostics(&[("flow/01-one.yaml", flow), ("events/lap.yaml", event)]);
    for expected in [
        "event `lap`: no track",
        "event `lap`: no car",
        "event `lap`: a layout without a track",
        "event `lap`: `laps` is for a race",
        "event `lap`: `ai_level` is for a race",
        "event `lap`: `ai_level` goes from 0 to 100",
    ] {
        assert!(has(&d, Severity::Error, expected), "{expected}: {d:#?}");
    }
    let fine = "id: lap
type: race
session: { track: t, car: c, laps: 5, grid: 10, ai_level: 90, start: first }
";
    let d = diagnostics(&[("flow/01-one.yaml", flow), ("events/lap.yaml", fine)]);
    assert!(d.is_empty(), "a full race session is clean: {d:#?}");
}

// The roster (SPEC-story-mode, Drivers): each driver once, the character
// they are one that exists and no other driver's, numbers in range; a
// driver nobody named would race under the livery's name.
#[test]
fn the_roster_is_checked() {
    let world = "characters:\n  - id: friend\n";
    let roster = "drivers:
  - { id: a, character: friend }
  - { id: a, name: Twice }
  - { id: b, character: ghost, name: B }
  - { id: c, character: friend, name: C, ai_level: 101, aggression: 150 }
  - { id: d }
";
    let d = diagnostics(&[("world/characters.yaml", world), ("drivers/roster.yaml", roster)]);
    for expected in [
        "driver `a` is declared twice",
        "driver `b`: unknown character `ghost`",
        "driver `c`: character `friend` is already another driver",
        "driver `c`: `ai_level` goes from 0 to 100",
        "driver `c`: `aggression` goes from 0 to 100",
    ] {
        assert!(has(&d, Severity::Error, expected), "{expected}: {d:#?}");
    }
    assert!(has(&d, Severity::Warning, "driver `d` has no name"), "{d:#?}");
    assert!(
        d.iter().all(|x| x.file == "drivers/roster.yaml"),
        "all on the roster: {d:#?}"
    );
}

// A race's grid names drivers of the roster, each once, and holds no
// more opponents than its size; only a race has one.
#[test]
fn an_event_grid_is_checked() {
    let flow = "id: ch01\nnodes:\n  - id: start\n    event: r\n  - id: end\n    end: done\n";
    let roster = "drivers:\n  - { id: rival, name: R }\n";
    let event = "id: r
type: race
session:
  track: t
  car: c
  grid: 3
  opponents:
    - { driver: rival }
    - { driver: rival, ballast: 120 }
    - { driver: ghost, ai_level: 101 }
";
    let d = diagnostics(&[
        ("flow/01-one.yaml", flow),
        ("drivers/roster.yaml", roster),
        ("events/r.yaml", event),
    ]);
    for expected in [
        "event `r`, opponent 2: driver `rival` is already on this grid",
        "event `r`, opponent 2: `ballast` goes from 0 to 100",
        "event `r`, opponent 3: unknown driver `ghost`",
        "event `r`, opponent 3: `ai_level` goes from 0 to 100",
        "event `r`: a grid of 3 cars cannot hold 3 opponents and the player",
    ] {
        assert!(has(&d, Severity::Error, expected), "{expected}: {d:#?}");
    }
    let lap = "id: r\ntype: hot_lap\nsession:\n  track: t\n  car: c\n  opponents: [{ driver: rival }]\n";
    let d = diagnostics(&[
        ("flow/01-one.yaml", flow),
        ("drivers/roster.yaml", roster),
        ("events/r.yaml", lap),
    ]);
    assert!(has(&d, Severity::Error, "`opponents` is for a race"), "{d:#?}");
    let fine = "id: r\ntype: race\nsession:\n  track: t\n  car: c\n  opponents: [{ driver: rival }, {}]\n";
    let d = diagnostics(&[
        ("flow/01-one.yaml", flow),
        ("drivers/roster.yaml", roster),
        ("events/r.yaml", fine),
    ]);
    assert!(d.is_empty(), "a grid without `grid` is the listed field: {d:#?}");
}

// `ahead_of` names a driver who races in the event: one unknown, or not
// on the grid, would make the event impossible to win.
#[test]
fn ahead_of_names_a_driver_on_the_grid() {
    let flow = "id: ch01\nnodes:\n  - id: start\n    event: r\n  - id: end\n    end: done\n";
    let roster = "drivers:\n  - { id: rival, name: R }\n  - { id: star, name: S }\n";
    let event = "id: r
type: race
session: { track: t, car: c, opponents: [{ driver: rival }] }
success: [\"ahead_of rival\", \"ahead_of star\", \"ahead_of ghost\"]
";
    let d = diagnostics(&[
        ("flow/01-one.yaml", flow),
        ("drivers/roster.yaml", roster),
        ("events/r.yaml", event),
    ]);
    assert!(
        has(&d, Severity::Error, "`ahead_of star`: driver `star` is not on its grid"),
        "{d:#?}"
    );
    assert!(
        has(&d, Severity::Error, "`ahead_of ghost`: unknown driver `ghost`"),
        "{d:#?}"
    );
    assert!(!d.iter().any(|x| x.message.contains("ahead_of rival")), "{d:#?}");
}

// Frames: `none` or a frame Pit Box ships, at the start and in a Beat;
// the frames a story declares itself come with `frames/` (later lot).
#[test]
fn an_unknown_frame_is_an_error_where_it_is_written() {
    let mut files = with(
        minimal(),
        "flow/01-one.yaml",
        "id: ch01\nnodes:\n  - id: start\n    beat: { frame: pager }\n  - id: end\n    end: done\n",
    );
    files[0].text = files[0].text.replace("frame: phone", "frame: desk");
    let d = validate(&crate::Story::from_sources(files).unwrap());
    let unknown = |file: &str, frame: &str| {
        (d.iter()).any(|x| x.file == file && x.message.contains(&format!("unknown frame `{frame}`")))
    };
    assert!(unknown("story.yaml", "desk"), "the start's frame: {d:#?}");
    assert!(unknown("flow/01-one.yaml", "pager"), "a beat's frame: {d:#?}");
}

// Nowhere to show (SPEC-story-mode, Content types, templates and frames):
// content sent while no active frame can show it is a warning, said
// "here" when every path arrives without one, "on some paths" when only
// some do. A dialogue is played on the scene: it needs no frame.
#[test]
fn content_with_no_frame_to_show_it_is_flagged_where_the_flow_tells() {
    let flow = "id: ch01
nodes:
  - id: start
    branch:
      - if: [\"reputation >= 1\"]
        goto: hide
    otherwise: join
  - id: hide
    beat: { frame: none }
  - id: join
    message: maybe_shown
  - id: hide_again
    beat: { frame: none, effects: [\"send sent_in_beat\"] }
  - id: sure
    message: never_shown
  - id: talk
    message: face_to_face
  - id: end
    end: done
";
    let content = "---\nid: maybe_shown\ntype: message\n---\nx\n---\nid: sent_in_beat\ntype: mail\n---\nx\n---\nid: never_shown\ntype: message\n---\nx\n---\nid: face_to_face\ntype: dialogue\n---\nx\n";
    let d = diagnostics(&[("flow/01-one.yaml", flow), ("content/a.story", content)]);
    let about = |content: &str| {
        (d.iter())
            .filter(|x| x.message.contains(&format!("`{content}`")))
            .collect::<Vec<_>>()
    };
    let maybe = about("maybe_shown");
    assert!(
        maybe.len() == 1 && maybe[0].message.contains("on some paths") && maybe[0].node.as_deref() == Some("join"),
        "one path removed the frame: {d:#?}"
    );
    assert_eq!(maybe[0].severity, Severity::Warning, "it plays: it waits for a frame");
    let sure = about("never_shown");
    assert!(
        sure.len() == 1 && sure[0].message.contains("cannot show it here"),
        "every path: {d:#?}"
    );
    assert!(
        about("sent_in_beat").iter().any(|x| x.message.contains("mail")),
        "the beat's own frame is up before its `send`: {d:#?}"
    );
    assert!(about("face_to_face").is_empty(), "a dialogue needs no frame: {d:#?}");
    assert!(about("start").is_empty() && d.len() == 3, "nothing else: {d:#?}");
}

// L3.4: with no frame, there is nothing to do a player action in - a
// story with no frame acts through the replies of its dialogues. A
// warning: at play, a Done button stands in.
#[test]
fn an_action_reached_with_no_frame_is_flagged() {
    let flow = "id: ch01
nodes:
  - id: start
    branch:
      - if: [\"reputation >= 1\"]
        goto: hide
    otherwise: maybe
  - id: hide
    beat: { frame: none }
  - id: maybe
    action: buy_car
  - id: gone
    beat: { frame: none }
  - id: sure
    action: sell_car
  - id: back
    beat: { frame: phone }
  - id: fine
    action: wash_car
  - id: end
    end: done
";
    let d = diagnostics(&[("flow/01-one.yaml", flow)]);
    let about = |action: &str| {
        (d.iter())
            .filter(|x| x.message.contains(&format!("action `{action}`")))
            .collect::<Vec<_>>()
    };
    let maybe = about("buy_car");
    assert!(
        maybe.len() == 1 && maybe[0].message.contains("on some paths") && maybe[0].node.as_deref() == Some("maybe"),
        "one path removed the frame: {d:#?}"
    );
    assert_eq!(
        maybe[0].severity,
        Severity::Warning,
        "it plays: a Done button stands in"
    );
    let sure = about("sell_car");
    assert!(
        sure.len() == 1 && !sure[0].message.contains("on some paths"),
        "every path: {d:#?}"
    );
    assert!(about("wash_car").is_empty(), "the phone is back: {d:#?}");
}

// Reactive content can arrive whatever the frame: only a story that never
// puts up a frame able to show it is flagged.
#[test]
fn reactive_content_is_flagged_only_if_no_frame_of_the_story_shows_it() {
    let reactive = "---\nid: news\ntype: article\nwhen: [\"reputation >= 1\"]\n---\nx\n";
    let d = diagnostics(&[("content/a.story", reactive)]);
    assert!(d.is_empty(), "the phone shows an article: {d:#?}");
    let mut lost = with(minimal(), "content/a.story", reactive);
    lost[0].text = lost[0].text.replace("node: start", "node: nowhere");
    let d = validate(&crate::Story::from_sources(lost).unwrap());
    assert!(
        !has(&d, Severity::Warning, "no frame it can arrive in"),
        "a missing start node is its own error, not a frame warning per item: {d:#?}"
    );
    let mut files = with(minimal(), "content/a.story", reactive);
    files[0].text = files[0].text.replace("frame: phone", "frame: none");
    let d = validate(&crate::Story::from_sources(files).unwrap());
    assert!(
        has(&d, Severity::Warning, "no frame it can arrive in can show it"),
        "a visual novel has nowhere to show an article: {d:#?}"
    );
}

// Portraits (SPEC-story-mode, In-person dialogues): a `neutral` one, and
// each a file inside `media/` - checked against the folder when it was
// listed, never outside it.
#[test]
fn portraits_need_a_neutral_one_and_a_file_inside_media() {
    let chars = "characters:
  - id: friend
    portraits:
      happy: portraits/happy.png
      sad: ../outside.png
      angry: C:/pictures/angry.png
      tired: portraits\\tired.png
  - id: rival
    portraits:
      neutral: portraits/rival.png
";
    let mut d = draft(with(minimal(), "world/characters.yaml", chars));
    d.media = Some(BTreeSet::from(["portraits/happy.png".to_string()]));
    let diags = validate(&crate::Story::from_draft(d).unwrap());
    for needle in [
        "character `friend`: portraits need a `neutral` one",
        "portrait `sad`: `../outside.png`: a path inside `media/` cannot leave it",
        "portrait `angry`: `C:/pictures/angry.png`: a path inside `media/`, not an absolute one",
        "portrait `tired`: `portraits\\tired.png`: folders are separated by `/`",
        "portrait `neutral`: `media/portraits/rival.png` does not exist",
    ] {
        assert!(has(&diags, Severity::Error, needle), "{needle} in {diags:#?}");
    }
    assert_eq!(diags.len(), 5, "the listed `happy` file is fine: {diags:#?}");

    let unlisted = diagnostics(&[("world/characters.yaml", chars)]);
    assert!(
        !has(&unlisted, Severity::Error, "does not exist"),
        "a story built in memory has no media list to check against: {unlisted:#?}"
    );
}

// A content item's text (SPEC-story-mode, Dialogue text): what is
// outside the subset is an error on its line of the file, a label is
// used once, and the sentences inside are checked like any other.
#[test]
fn a_script_is_checked_line_by_line() {
    let content = "---\nid: talk\ntype: message\nfrom: friend\n---\nfriend: Hi.\n-> elsewhere\n* (yes) [Yes.]\n    ~ effect(\"flag.ghost = true\")\n* (yes) [Again.]\n";
    let chars = "characters:\n  - id: friend\n";
    let d = diagnostics(&[("content/a.story", content), ("world/characters.yaml", chars)]);
    let at = |line: usize, needle: &str| {
        (d.iter()).any(|x| x.file == format!("content/a.story:{line}") && x.message.contains(needle))
    };
    assert!(at(7, "only divert"), "the divert, on line 7: {d:#?}");
    assert!(at(10, "label `yes` is used twice"), "{d:#?}");
    assert!(at(9, "unknown flag `ghost`"), "an effect inside the script: {d:#?}");
}

// In-person dialogues: the characters present are known, whoever speaks
// is among them, an expression is one of the speaker's portraits, and a
// position or an expression only goes with a dialogue.
#[test]
fn dialogue_speakers_and_portraits_are_checked() {
    let chars = "characters:\n  - id: friend\n    portraits: { neutral: f.png }\n  - id: rival\n";
    let content = "---\nid: scene\ntype: dialogue\ncharacters: [friend, ghost]\n---\nfriend: Hi. #expression:angry\nrival: Me too.\nstranger: Who?\n---\nid: text\ntype: message\nfrom: friend\nscene: garage\n---\nfriend: Hi. #position:left\n";
    let d = diagnostics(&[("content/a.story", content), ("world/characters.yaml", chars)]);
    for needle in [
        "unknown character `ghost` among those present",
        "`friend` has no portrait `angry`",
        "`rival` speaks but is not among the `characters` present",
        "`scene` is for a dialogue",
        "`#expression:`, `#position:` and `#leave` are for a dialogue",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
    assert!(
        has(
            &d,
            Severity::Warning,
            "`stranger:` reads as a speaker, but there is no such character"
        ),
        "{d:#?}"
    );
}

// Reply exits: a label the content does not have, a target that does
// not exist; and a condition on a reply names one of its labels.
#[test]
fn reply_exits_and_reply_conditions_name_real_labels() {
    let content = "---\nid: ask\ntype: message\n---\n* (yes) [Yes.]\n";
    let flow = "id: ch01\nnodes:\n  - id: start\n    message: ask\n    on_reply: { maybe: end, yes: nowhere }\n  - id: check\n    branch:\n      - if: [\"content.ask.reply == perhaps\"]\n        goto: end\n    otherwise: end\n  - id: end\n    end: done\n";
    let d = diagnostics(&[("content/a.story", content), ("flow/01-one.yaml", flow)]);
    for needle in [
        "reply exit `maybe`: `ask` has no choice labelled so",
        "reply exit `nowhere` does not exist",
        "content `ask` has no choice labelled `perhaps`",
    ] {
        assert!(has(&d, Severity::Error, needle), "{needle} in {d:#?}");
    }
}

// L3b.3, illustrations: a picture inside `media/`, required for an
// illustration and for it only, and nothing to answer.
#[test]
fn an_illustration_needs_a_picture_in_media_and_offers_no_reply() {
    let content = "---
id: good
type: illustration
picture: big/moment.png
---
The finish.
---
id: missing
type: illustration
picture: big/gone.png
---
---
id: without
type: illustration
---
---
id: outside
type: illustration
picture: ../moment.png
---
---
id: asking
type: illustration
picture: big/moment.png
---
* (yes) [Yes]
---
id: message
type: message
picture: big/moment.png
---
Hello.
";
    let mut d = draft(with(minimal(), "content/a.story", content));
    d.media = Some(BTreeSet::from(["big/moment.png".to_string()]));
    let diags = validate(&crate::Story::from_draft(d).unwrap());
    for needle in [
        "content `missing`: `media/big/gone.png` does not exist",
        "content `without`: an illustration needs a `picture`",
        "content `outside`: picture `../moment.png`: a path inside `media/` cannot leave it",
        "content `asking`: an illustration offers no reply",
        "content `message`: `picture` is for an illustration",
    ] {
        assert!(has(&diags, Severity::Error, needle), "{needle} in {diags:#?}");
    }
    assert!(
        !diags
            .iter()
            .any(|x| x.message.contains("content `good`") && x.severity == Severity::Error),
        "the good one is fine: {diags:#?}"
    );
}

// L3b.4: `#leave` belongs to a dialogue, as the other stage tags do, and
// `transition` to what plays on the scene.
#[test]
fn stage_tags_and_transitions_belong_to_the_scene() {
    let content = "---
id: talk
type: dialogue
transition: cut
---
friend: Bye. #leave
---
id: note
type: message
transition: fade
---
friend: Bye. #leave
";
    let chars = "characters:
  - id: friend
";
    let d = diagnostics(&[("content/a.story", content), ("world/characters.yaml", chars)]);
    assert!(
        has(
            &d,
            Severity::Error,
            "content `note`: `transition` is for a dialogue or an illustration"
        ),
        "{d:#?}"
    );
    assert!(
        has(
            &d,
            Severity::Error,
            "content `note`: `#expression:`, `#position:` and `#leave` are for a dialogue"
        ),
        "{d:#?}"
    );
    assert!(
        !d.iter()
            .any(|x| x.message.contains("content `talk`") && x.severity == Severity::Error),
        "the dialogue is fine: {d:#?}"
    );
}

const SCENES: &str = "scenes:
  - id: garage
    ambient: garage.wav
    variants:
      - { id: day, file: day.webp }
      - { id: night, time: [night], file: night.webm }
      - { id: day, file: day.gif, ambient: hum.txt }
      - { id: rain, weather: [rain], mood: [calm], file: gone.webp }
      - { id: flashback, mood: [lost], file: day.webp }
  - id: empty
    variants: []
";

// The scenes (SPEC-story-mode, Scenes, frame and sound): each place has
// something to show, each variant once; a variant after one that always
// holds never shows; files are inside `media/` and of a kind the scene
// plays; a weather or mood no Beat sets is never met.
#[test]
fn scenes_have_reachable_variants_with_files_the_scene_can_play() {
    // `flashback` comes after an always-holding variant and asks for a mood
    // no Beat sets, but a Beat forces it: it shows, nothing to say.
    let flow = "id: ch01
nodes:
  - id: start
    beat: { scene: garage, variant: flashback }
  - id: end
    end: done
";
    let mut d = draft(with(
        with(minimal(), crate::scene::SCENES, SCENES),
        "flow/01-one.yaml",
        flow,
    ));
    d.media = Some(BTreeSet::from(
        ["day.webp", "night.webm", "day.gif", "garage.wav", "hum.txt"].map(String::from),
    ));
    let diags = validate(&crate::Story::from_draft(d).unwrap());
    for needle in [
        "scene `empty` has no variant",
        "variant `day` is declared twice",
        "`day.gif` is neither an image",
        "ambient `hum.txt` is not a sound",
        "variant `rain`: `media/gone.webp` does not exist",
    ] {
        assert!(has(&diags, Severity::Error, needle), "{needle} in {diags:#?}");
    }
    for needle in [
        "variant `night` never shows: `day`, before it, always does",
        "no Beat sets the weather `rain`",
        "no Beat sets the mood `calm`",
    ] {
        assert!(has(&diags, Severity::Warning, needle), "{needle} in {diags:#?}");
    }
    assert!(
        !has(&diags, Severity::Error, "garage.wav"),
        "a listed sound is fine: {diags:#?}"
    );
    assert!(
        !diags.iter().any(|d| d.message.contains("flashback")),
        "a forced variant shows: {diags:#?}"
    );
}

// A place is named where it is used - the start, a Beat, a dialogue - and
// must be declared; a forced variant must be one of the place's, or of
// some place when the Beat names none.
#[test]
fn a_place_or_variant_named_must_be_declared() {
    let manifest = minimal()[0]
        .text
        .replace("frame: phone", "frame: phone\n  scene: attic");
    let flow = "id: ch01\nnodes:
  - id: start
    beat: { scene: cellar }
  - id: forced
    beat: { scene: garage, variant: dusk }
  - id: anywhere
    beat: { variant: noon, mood: calm }
  - id: fine
    beat: { scene: garage, variant: day }
  - id: end
    end: done
";
    let talk = "---\nid: talk\ntype: dialogue\nscene: roof\n---\nHello.\n";
    let diags = diagnostics(&[
        ("story.yaml", &manifest),
        (
            crate::scene::SCENES,
            "scenes:\n  - id: garage\n    variants:\n      - { id: day, file: day.webp }\n",
        ),
        ("flow/01-one.yaml", flow),
        ("content/talk.story", talk),
    ]);
    for needle in [
        "start: unknown scene `attic`",
        "unknown scene `cellar` (declare it in scenes/scenes.yaml)",
        "scene `garage` has no variant `dusk`",
        "no scene has a variant `noon`",
        "unknown scene `roof`",
    ] {
        assert!(has(&diags, Severity::Error, needle), "{needle} in {diags:#?}");
    }
    assert!(
        !diags.iter().any(|d| d.node.as_deref() == Some("fine")),
        "a declared place and variant: {diags:#?}"
    );
}

// A story's frames (SPEC-story-mode, Skins: how a frame gets its look): each
// with apps, each app showing what its template can, a type in one app of the
// frame only, its icon a file of `media/`; a Beat unlocks only an app some
// frame has, and an app waiting for a Beat no Beat unlocks is said so. A
// frame the story declares can be put up.
#[test]
fn frames_have_apps_that_show_what_their_template_can() {
    let frames = "frames:
  - id: empty
    kind: phone
    apps: []
  - id: basic
    kind: phone
    apps:
      - { id: sms, template: thread, shows: [message, mail], icon: icons/sms.png }
      - { id: sms, template: inbox, shows: [mail], icon: icons/gone.png }
      - { id: later, template: feed, shows: [article], appears: beat }
      - { id: nothing, template: statement, shows: [] }
";
    let flow = "id: ch01
nodes:
  - id: start
    beat: { frame: basic, unlock: [ghost] }
  - id: end
    end: done
";
    let mut d = draft(with(
        with(minimal(), crate::frame::FRAMES, frames),
        "flow/01-one.yaml",
        flow,
    ));
    d.media = Some(BTreeSet::from(["icons/sms.png".to_string()]));
    let diags = validate(&crate::Story::from_draft(d).unwrap());
    for needle in [
        "frame `empty` has no app",
        "app `sms` is declared twice",
        "a thread cannot show a mail",
        "app `sms` already shows mail",
        "app `nothing` shows no content type",
        "`media/icons/gone.png` does not exist",
        "unlock: no frame has an app `ghost`",
    ] {
        assert!(has(&diags, Severity::Error, needle), "{needle} in {diags:#?}");
    }
    assert!(
        has(
            &diags,
            Severity::Warning,
            "app `later` appears from a Beat, and no Beat unlocks it"
        ),
        "{diags:#?}"
    );
    assert!(
        !diags.iter().any(|d| d.message.contains("unknown frame")),
        "a declared frame can be put up: {diags:#?}"
    );
    let typo = "id: ch01\nnodes:\n  - id: start\n    beat: { frame: basci }\n  - id: end\n    end: done\n";
    let d = diagnostics(&[(crate::frame::FRAMES, frames), ("flow/01-one.yaml", typo)]);
    assert!(
        has(
            &d,
            Severity::Error,
            "unknown frame `basci` (the frames are `empty`, `basic`, `phone`, or `none`)"
        ),
        "the story's frames named first: {d:#?}"
    );
}
