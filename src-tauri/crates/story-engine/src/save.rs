//! Saves (SPEC-story-mode, Story Mode: player side): the state and the full
//! history, as JSON, **versioned from the first day**. A save is a persisted
//! format written by a released Pit Box on a player's machine, so:
//!
//! - `format` is read before anything else; a save from a newer Pit Box is
//!   refused, never misread;
//! - a field added to `State` takes `#[serde(default)]` and needs no new
//!   format; a field removed, renamed or changing meaning bumps
//!   `SAVE_FORMAT` and gets a step in `migrate`, with a test reading the old
//!   form;
//! - `save_v1_still_reads` pins format 1 as written today.
//!
//! JSON rather than YAML: nobody edits a save by hand, and `serde_json` is
//! already everywhere in Pit Box.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::load::Story;
use crate::model::Placement;
use crate::state::{State, Wait};

pub const SAVE_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub format: u32,
    pub story_id: String,
    /// The story's version when the game was saved: a story updated since
    /// may have moved what the save points to (`check`).
    pub story_version: String,
    pub state: State,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    NotASave(String),
    /// Written by a newer Pit Box.
    TooNew {
        format: u32,
    },
    /// Saved from another story.
    OtherStory {
        expected: String,
        found: String,
    },
    /// Points to something the story no longer has (it was updated).
    Stale(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::NotASave(why) => write!(f, "not a story save: {why}"),
            SaveError::TooNew { format } => {
                write!(
                    f,
                    "save format {format} comes from a newer Pit Box (this one reads up to {SAVE_FORMAT})"
                )
            }
            SaveError::OtherStory { expected, found } => {
                write!(f, "this save belongs to story `{found}`, not `{expected}`")
            }
            SaveError::Stale(why) => write!(f, "the story changed since this save: {why}"),
        }
    }
}

impl Save {
    pub fn new(story: &Story, state: &State) -> Save {
        Save {
            format: SAVE_FORMAT,
            story_id: story.manifest.id.clone(),
            story_version: story.manifest.version.clone(),
            state: state.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a save serialises: plain data, string keys")
    }

    pub fn from_json(text: &str) -> Result<Save, SaveError> {
        let mut value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| SaveError::NotASave(e.to_string()))?;
        let format = value
            .get("format")
            .and_then(|f| f.as_u64())
            .ok_or_else(|| SaveError::NotASave("no `format`".into()))? as u32;
        if format > SAVE_FORMAT {
            return Err(SaveError::TooNew { format });
        }
        migrate(&mut value, format)?;
        serde_json::from_value(value).map_err(|e| SaveError::NotASave(e.to_string()))
    }

    /// Whether the save can resume `story`: the same story, and everything
    /// it points to still there. A story update that removed the node a
    /// player stands on must say so, not crash three steps later.
    pub fn check(&self, story: &Story) -> Result<(), SaveError> {
        if self.story_id != story.manifest.id {
            return Err(SaveError::OtherStory {
                expected: story.manifest.id.clone(),
                found: self.story_id.clone(),
            });
        }
        let s = &self.state;
        let stale = |why: String| Err(SaveError::Stale(why));
        if story.node(&s.node).is_none() {
            return stale(format!("node `{}` no longer exists", s.node));
        }
        match &s.wait {
            Wait::Event { event } if story.event(event).is_none() => {
                return stale(format!("event `{event}` no longer exists"))
            }
            Wait::Reply { content } if story.content_item(content).is_none() => {
                return stale(format!("content `{content}` no longer exists"))
            }
            Wait::Reply { content } if !s.conversations.contains_key(content) => {
                return stale(format!(
                    "the flow waits on a conversation in `{content}` that is not under way"
                ))
            }
            _ => {}
        }
        for event in s.launched.iter().chain(&s.side_events) {
            if story.event(event).is_none() {
                return stale(format!("event `{event}` no longer exists"));
            }
        }
        // A conversation saved at a choice point resumes there only if the
        // script still has that choice point, with the labels it offered.
        for (content, conversation) in &s.conversations {
            let choices = story
                .content_item(content)
                .and_then(|item| crate::script::choices_at(&item.script, &conversation.at));
            let kept = choices
                .is_some_and(|cs| (conversation.offered.iter()).all(|label| cs.iter().any(|c| &c.label == label)));
            if !kept {
                return stale(format!(
                    "the conversation in `{content}` no longer has the choice it stood at"
                ));
            }
        }
        for held in &s.held {
            if story.content_item(&held.content).is_none() {
                return stale(format!(
                    "content `{}`, waiting for a frame, no longer exists",
                    held.content
                ));
            }
        }
        for scheduled in &s.scheduled {
            if !matches!(
                story.content_item(&scheduled.content).map(|c| &c.placement),
                Some(Placement::Reactive { .. })
            ) {
                return stale(format!(
                    "scheduled content `{}` is no longer reactive content",
                    scheduled.content
                ));
            }
        }
        let scale = &story.settings().relations;
        for (who, level) in &s.relations {
            if scale.rank(level).is_none() {
                return stale(format!("relation `{level}` of `{who}` is no longer on the scale"));
            }
        }
        Ok(())
    }

    /// The state to play on, once `check` passed. A character or a counter
    /// the story gained since the save gets its starting value: without
    /// it, every condition on it would stay false and its effects do nothing.
    /// A save from before frames existed takes the story's starting frame.
    pub fn resume(self, story: &Story) -> Result<State, SaveError> {
        self.check(story)?;
        let mut state = self.state;
        state.frame.get_or_insert_with(|| story.manifest.start.frame.clone());
        let start = &story.settings().relations.start;
        for ch in &story.characters {
            (state.relations.entry(ch.id.clone()))
                .or_insert_with(|| ch.relation.clone().unwrap_or_else(|| start.clone()));
        }
        for (counter, value) in &story.variables.counters {
            state.counters.entry(counter.clone()).or_insert(*value);
        }
        Ok(state)
    }
}

/// Brings an older save up to `SAVE_FORMAT`, one format at a time. Empty
/// while there is only format 1: the first change of meaning adds
/// `1 => { …; *value["format"] = 2 }` here, and a test reading a format-1 save.
fn migrate(_value: &mut serde_json::Value, format: u32) -> Result<(), SaveError> {
    match format {
        SAVE_FORMAT => Ok(()),
        other => Err(SaveError::NotASave(format!("unknown save format {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{run, submit, Input};
    use crate::state::EventResult;
    use crate::testutil::{minimal, with};

    fn story() -> Story {
        let files = with(
            with(
                with(
                    minimal(),
                    "flow/01-one.yaml",
                    "id: ch01\nnodes:\n  - id: start\n    event: race\n  - id: end\n    end: done\n",
                ),
                "events/race.yaml",
                "id: race\ntype: race\non_failure: { pool: p }\n",
            ),
            "content/p.story",
            "---\nid: p1\ntype: message\npool: p\n---\nx\n---\nid: p2\ntype: message\npool: p\n---\ny\n",
        );
        Story::from_sources(files).unwrap()
    }

    // A save holds everything: written then read back, the game resumes
    // exactly where it was - including the random draws still to come.
    #[test]
    fn a_save_reads_back_into_the_same_game() {
        let story = story();
        let mut state = State::new(&story, 42);
        run(&story, &mut state).unwrap();
        submit(&story, &mut state, Input::Launch { event: "race".into() }).unwrap();
        submit(
            &story,
            &mut state,
            Input::Result {
                event: "race".into(),
                result: EventResult::default(),
            },
        )
        .unwrap();

        let text = Save::new(&story, &state).to_json();
        let back = Save::from_json(&text).expect("reads");
        assert_eq!(back.state, state, "same state, history included");
        back.check(&story).expect("resumable");

        let (mut a, mut b) = (state.clone(), back.state.clone());
        for s in [&mut a, &mut b] {
            submit(&story, s, Input::Launch { event: "race".into() }).unwrap();
            submit(
                &story,
                s,
                Input::Result {
                    event: "race".into(),
                    result: EventResult::default(),
                },
            )
            .unwrap();
        }
        assert_eq!(
            a, b,
            "the reloaded game draws the same pool variant: no reroll by reloading"
        );
    }

    #[test]
    fn a_save_from_a_newer_pit_box_or_another_story_is_refused() {
        let story = story();
        let mut save = Save::new(&story, &State::new(&story, 0));
        let newer = Save::from_json(&save.to_json().replace("\"format\": 1", "\"format\": 2"));
        assert_eq!(newer, Err(SaveError::TooNew { format: 2 }));
        assert!(matches!(
            Save::from_json("{\"story_id\": \"x\"}"),
            Err(SaveError::NotASave(_))
        ));
        save.story_id = "other".into();
        assert!(matches!(save.check(&story), Err(SaveError::OtherStory { .. })));
    }

    #[test]
    fn a_save_pointing_to_a_removed_node_says_so() {
        let story = story();
        let mut save = Save::new(&story, &State::new(&story, 0));
        save.state.node = "gone".into();
        assert_eq!(
            save.check(&story),
            Err(SaveError::Stale("node `gone` no longer exists".into()))
        );
    }

    // Content waiting for a frame (L3.4) is part of what a save stands on:
    // a story that lost it refuses the save, rather than deliver nothing.
    #[test]
    fn a_save_holding_removed_content_says_so() {
        let story = story();
        let mut save = Save::new(&story, &State::new(&story, 0));
        save.state.held.push(crate::state::Held {
            content: "gone".into(),
            via: crate::state::Via::Effect,
        });
        assert_eq!(
            save.check(&story),
            Err(SaveError::Stale(
                "content `gone`, waiting for a frame, no longer exists".into()
            ))
        );
    }

    // A story update that adds a character or a counter must not leave old
    // saves without it: conditions on it would never hold.
    #[test]
    fn resuming_after_a_story_update_fills_what_the_story_gained() {
        let story = story();
        let save = Save::new(&story, &State::new(&story, 0));
        let mut files = with(
            minimal(),
            "flow/01-one.yaml",
            "id: ch01\nnodes:\n  - id: start\n    event: race\n  - id: end\n    end: done\n",
        );
        files = with(files, "events/race.yaml", "id: race\ntype: race\n");
        files = with(
            files,
            "world/characters.yaml",
            "characters:\n  - id: newcomer\n    relation: warm\n",
        );
        files = with(files, "world/variables.yaml", "counters: { visits: 3 }\n");
        let updated = Story::from_sources(files).unwrap();
        let state = save.resume(&updated).expect("resumes");
        assert_eq!(
            state.relations["newcomer"], "warm",
            "new character at its starting relation"
        );
        assert_eq!(state.counters["visits"], 3, "new counter at its starting value");
    }

    // A save written before frames existed has no `frame`: it reads, and
    // resumes on its story's starting frame - never on one the platform
    // would pick (here `none`, to tell the two apart).
    #[test]
    fn a_save_from_before_frames_resumes_on_the_starting_frame() {
        let mut files = minimal();
        files[0].text = files[0].text.replace("frame: phone", "frame: none");
        let story = Story::from_sources(files).unwrap();
        let mut save = Save::new(&story, &State::new(&story, 0));
        save.state.frame = Some(crate::frame::Frame::Named("phone".into()));
        let mut json: serde_json::Value = serde_json::from_str(&save.to_json()).unwrap();
        json["state"].as_object_mut().unwrap().remove("frame");
        let old = Save::from_json(&json.to_string()).expect("a save without a frame reads");
        assert_eq!(old.state.frame, None, "nothing invented on reading");
        let state = old.resume(&story).expect("resumes");
        assert_eq!(
            state.frame,
            Some(crate::frame::Frame::None),
            "the story's starting frame"
        );

        let back = Save::from_json(&Save::new(&story, &state).to_json()).unwrap();
        assert_eq!(
            back.state.frame,
            Some(crate::frame::Frame::None),
            "`none` saved as such"
        );
    }

    // The place, its forced variant, the weather and the mood are saved; a
    // save written before scenes existed has none of them, reads, and
    // resumes with no place - a plain background until a Beat sets one,
    // rather than a starting place that may belong to another chapter.
    #[test]
    fn the_scene_is_saved_and_a_save_from_before_scenes_has_none() {
        let mut files = minimal();
        files[0].text = files[0].text.replace(
            "frame: phone",
            "frame: phone
  scene: garage",
        );
        files.push(crate::load::SourceFile::new(
            crate::scene::SCENES,
            "scenes:
  - id: garage
    variants:
      - { id: day, file: day.webp }
",
        ));
        let story = Story::from_sources(files).unwrap();
        let mut state = State::new(&story, 0);
        assert_eq!(
            state.scene.as_deref(),
            Some("garage"),
            "a new game starts on the starting place"
        );
        (state.variant, state.weather, state.mood) = (Some("day".into()), Some("rain".into()), Some("calm".into()));
        let back = Save::from_json(&Save::new(&story, &state).to_json()).unwrap();
        assert_eq!(
            (
                back.state.scene,
                back.state.variant,
                back.state.weather,
                back.state.mood
            ),
            (
                state.scene.clone(),
                state.variant.clone(),
                state.weather.clone(),
                state.mood.clone()
            ),
            "all four saved"
        );

        state.unlocked.insert("news".into());
        let mut json: serde_json::Value = serde_json::from_str(&Save::new(&story, &state).to_json()).unwrap();
        assert!(json["state"]["unlocked"].is_array(), "the apps unlocked are saved");
        for key in ["scene", "variant", "weather", "mood", "unlocked"] {
            json["state"].as_object_mut().unwrap().remove(key);
        }
        let old = Save::from_json(&json.to_string()).expect("a save without a scene reads");
        let resumed = old.resume(&story).expect("resumes");
        assert_eq!(resumed.scene, None, "no place invented");
        assert!(
            resumed.unlocked.is_empty(),
            "a save from before frames: no app unlocked"
        );
    }

    // A game saved at a choice point resumes there; a story update that
    // removed that choice point says so instead of resuming somewhere else.
    #[test]
    fn a_save_in_the_middle_of_a_conversation_resumes_at_its_choice() {
        let content = "---\nid: ask\ntype: message\n---\nCome?\n* (yes) [Yes.]\n    * * (sure) [Sure.]\n* (no) [No.]\n";
        let flow =
            "id: ch01\nnodes:\n  - id: start\n    message: ask\n    wait_reply: true\n  - id: end\n    end: done\n";
        let files = with(with(minimal(), "flow/01-one.yaml", flow), "content/ask.story", content);
        let story = Story::from_sources(files.clone()).unwrap();
        let mut state = State::new(&story, 0);
        run(&story, &mut state).unwrap();
        let reply = |label: &str| Input::Reply {
            content: "ask".into(),
            reply: label.into(),
        };
        submit(&story, &mut state, reply("yes")).unwrap();

        let back = Save::from_json(&Save::new(&story, &state).to_json()).unwrap();
        let mut resumed = back.clone().resume(&story).expect("resumes");
        assert_eq!(resumed.conversations["ask"].offered, ["sure"], "at the nested choice");
        submit(&story, &mut resumed, reply("sure")).unwrap();
        assert_eq!(resumed.wait, Wait::Ended { ending: "done".into() });

        let shorter = Story::from_sources(with(
            files,
            "content/ask.story",
            "---\nid: ask\ntype: message\n---\nCome?\n* (yes) [Yes.]\n* (no) [No.]\n",
        ))
        .unwrap();
        assert!(
            matches!(back.check(&shorter), Err(SaveError::Stale(why)) if why.contains("no longer has the choice")),
            "the nested choice point is gone"
        );
    }

    // A reply recorded before labels were kept in a list still answers
    // `content.<id>.reply == <label>`.
    #[test]
    fn a_reply_saved_before_the_list_of_labels_still_counts() {
        let content = "---\nid: ask\ntype: message\n---\n* (yes) [Yes.]\n";
        let story = Story::from_sources(with(minimal(), "content/ask.story", content)).unwrap();
        let mut state = State::new(&story, 0);
        state.content.insert(
            "ask".into(),
            crate::state::ContentFacts {
                delivered: 1,
                seen: false,
                reply: Some("yes".into()),
                chosen: Vec::new(),
            },
        );
        let cond: crate::expr::Condition = "content.ask.reply == yes".parse().unwrap();
        assert!(state.holds(&story, &cond), "the last reply alone answers");
    }

    // Format 1 as written on the first day. If this test has to change, the
    // format changed: bump SAVE_FORMAT and migrate instead.
    #[test]
    fn save_v1_still_reads() {
        let v1 = r#"{
  "format": 1,
  "story_id": "sample",
  "story_version": "1",
  "state": {
    "node": "start",
    "wait": { "kind": "event", "event": "race" },
    "date": "0001-01-01",
    "clock": "00:00",
    "reputation": 3,
    "followers": 0,
    "money": 0,
    "flags": [],
    "counters": {},
    "relations": {},
    "cars": [],
    "events": {
      "race": { "attempts": 1, "successes": 0, "consecutive_failures": 1, "paid": 0,
                "last": { "finished": true, "position": 5, "best_lap_ms": null, "ahead_of": [], "forced": null } }
    },
    "content": { "p1": { "delivered": 1, "seen": false, "reply": null } },
    "scheduled": [],
    "armed": [],
    "pools": { "p": ["p1"] },
    "side_events": [],
    "launched": null,
    "rng": 11400714819323198527,
    "history": [
      { "date": "0001-01-01", "clock": "00:00", "kind": "launched", "event": "race", "fee": null },
      { "date": "0001-01-01", "clock": "00:00", "kind": "result", "event": "race", "attempt": 1,
        "result": { "finished": true, "position": 5, "best_lap_ms": null, "ahead_of": [], "forced": null }, "success": false },
      { "date": "0001-01-01", "clock": "00:00", "kind": "delivered", "content": "p1", "via": { "kind": "pool", "event": "race" } }
    ]
  }
}"#;
        let save = Save::from_json(v1).expect("format 1 reads");
        save.check(&story()).expect("and resumes");
        assert_eq!(save.state.reputation, 3);
        assert_eq!(save.state.event("race").last.and_then(|r| r.position), Some(5));
        assert_eq!(save.state.history.len(), 3);
        assert!(save.state.pools["p"].contains("p1"), "pool memory kept");
    }

    // A reply recorded before its text was kept still reads, with no text:
    // the history only gained fields (SPEC-story-mode, Story Mode: player
    // side), so format 1 stands.
    #[test]
    fn a_reply_saved_without_its_text_still_reads() {
        let entry: crate::state::Entry = serde_json::from_str(
            r#"{ "date": "0001-01-01", "clock": "00:00", "kind": "replied", "content": "c", "reply": "yes" }"#,
        )
        .expect("reads");
        assert!(
            matches!(entry.what, crate::state::Happening::Replied { ref text, .. } if text.is_empty()),
            "{entry:?}"
        );
    }
}
