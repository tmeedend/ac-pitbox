//! Pit Box Story Mode engine (docs/story/SPEC-story-mode.md, lot L1).
//!
//! Pure: no Tauri, no access to the application. Reads a story folder,
//! validates it, runs its flow on a state and saves that state; and writes
//! back the files the editor changes.

/// Serde through the text form, for the types an author writes as a short
/// sentence: dates, clock times and spans (`"1998-03-01"`, `"18:40"`,
/// `"1 month"`), conditions, effects and success criteria
/// (`"rel.friend += 1"`). Story files, saves and the editor all read and write
/// that same text, so printing a parsed value must give the author's sentence
/// back (tested in `expr.rs` and `time.rs`).
macro_rules! serde_as_text {
    ($t:ty) => {
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub mod engine;
pub mod expr;
pub mod frame;
pub mod load;
pub mod model;
pub mod refs;
pub mod sample;
pub mod save;
pub mod script;
pub mod state;
pub mod time;
pub mod validate;
pub mod view;
pub mod write;

pub use load::{LoadError, SourceFile, Story};

#[cfg(test)]
pub(crate) mod testutil {
    use crate::load::SourceFile;

    /// The smallest readable story: a manifest and one chapter whose start
    /// node ends the story.
    pub fn minimal() -> Vec<SourceFile> {
        vec![
            SourceFile::new(
                "story.yaml",
                "format: 1\nid: sample\ntitle: Sample\nversion: \"1\"\nlanguages: [en]\nstart:\n  node: start\n  frame: phone\n",
            ),
            SourceFile::new("flow/01-one.yaml", "id: ch01\nnodes:\n  - id: start\n    end: done\n"),
        ]
    }

    /// `files` with `path` replaced (or added) by `text`.
    pub fn with(mut files: Vec<SourceFile>, path: &str, text: &str) -> Vec<SourceFile> {
        files.retain(|f| f.path != path);
        files.push(SourceFile::new(path, text));
        files
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::engine::{run, submit, Input};
    use crate::state::{EventResult, State, Wait};
    use crate::validate::validate;
    use crate::Story;

    fn sample() -> Story {
        Story::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).expect("the sample story loads")
    }

    fn drive(story: &Story, state: &mut State, event: &str, position: u32) {
        submit(story, state, Input::Launch { event: event.into() }).expect("launch");
        let result = EventResult {
            finished: true,
            position: Some(position),
            ..Default::default()
        };
        submit(
            story,
            state,
            Input::Result {
                event: event.into(),
                result,
            },
        )
        .expect("result");
    }

    // L1's "done": the engine runs a sample story from start to end, the
    // way `story-tool play` does. The sample is the shipped example of the
    // format, so it must also be spotless.
    #[test]
    fn the_sample_story_is_clean_and_plays_to_its_end() {
        let story = sample();
        assert_eq!(validate(&story), [], "no diagnostic on the sample");
        let mut state = State::new(&story, 1);
        run(&story, &mut state).unwrap();
        assert_eq!(
            state.wait,
            Wait::Reply {
                content: "invite_01".into()
            }
        );
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "invite_01".into(),
                reply: "accept".into(),
            },
        )
        .unwrap();
        drive(&story, &mut state, "race_01", 5);
        assert_eq!(state.date.to_string(), "2001-03-01", "a failure costs no time");
        drive(&story, &mut state, "race_01", 2);
        assert_eq!(state.date.to_string(), "2001-04-01", "success: one month on");
        for _ in 0..2 {
            submit(
                &story,
                &mut state,
                Input::ActionDone {
                    action: "visit_garage".into(),
                },
            )
            .unwrap();
        }
        assert_eq!(state.money, 300 - 50 - 50 + 200, "paid retries, prize");
        drive(&story, &mut state, "race_02", 1);
        assert_eq!(
            state.wait,
            Wait::Reply {
                content: "talk_01".into()
            },
            "the face-to-face dialogue holds the flow until it is answered"
        );
        let reputation = state.reputation;
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "talk_01".into(),
                reply: "rest".into(),
            },
        )
        .unwrap();
        assert_eq!(state.relations["parent"], "close", "`rest` led to its own follow-up");
        assert_eq!(state.reputation, reputation, "not to the one `another` leads to");
        assert_eq!(
            state.wait,
            Wait::Ended {
                ending: "finished".into()
            }
        );
        assert_eq!(
            state.frame,
            Some(crate::frame::Frame::Named("phone".into())),
            "the phone put back after the face-to-face dialogue"
        );
        for content in [
            "sponsor_offer",
            "rival_post_01",
            "garage_remark",
            "congrats_01",
            "talk_01",
        ] {
            assert_eq!(state.content_facts(content).delivered, 1, "{content} delivered once");
        }
        assert_eq!(
            state.content_facts("invite_02").delivered,
            0,
            "the branch skipped the second ask"
        );
    }
}
