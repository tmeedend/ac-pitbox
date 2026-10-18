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
            _ => {}
        }
        for event in s.launched.iter().chain(&s.side_events) {
            if story.event(event).is_none() {
                return stale(format!("event `{event}` no longer exists"));
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
    pub fn resume(self, story: &Story) -> Result<State, SaveError> {
        self.check(story)?;
        let mut state = self.state;
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
}
