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
pub mod scene;
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

    use crate::engine::{run, submit, Input, Output};
    use crate::frame::Frame;
    use crate::save::Save;
    use crate::state::{EventResult, Happening, State, Wait};
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

    /// The sample played up to chapter 3's dialogue: the first two
    /// chapters with the phone, the second race won. Chapter 3 removes the
    /// phone, and the friend's message sent as it starts has nothing to
    /// show it.
    fn up_to_chapter_3(story: &Story) -> State {
        let mut state = State::new(story, 1);
        run(story, &mut state).unwrap();
        assert_eq!(
            state.wait,
            Wait::Reply {
                content: "invite_01".into()
            }
        );
        submit(
            story,
            &mut state,
            Input::Reply {
                content: "invite_01".into(),
                reply: "accept".into(),
            },
        )
        .unwrap();
        drive(story, &mut state, "race_01", 5);
        assert_eq!(state.date.to_string(), "2001-03-01", "a failure costs no time");
        drive(story, &mut state, "race_01", 2);
        assert_eq!(state.date.to_string(), "2001-04-01", "success: one month on");
        for _ in 0..2 {
            submit(
                story,
                &mut state,
                Input::ActionDone {
                    action: "visit_garage".into(),
                },
            )
            .unwrap();
        }
        assert_eq!(state.money, 300 - 50 - 50 + 200, "paid retries, prize");
        submit(
            story,
            &mut state,
            Input::Launch {
                event: "race_02".into(),
            },
        )
        .unwrap();
        let out = submit(
            story,
            &mut state,
            Input::Result {
                event: "race_02".into(),
                result: EventResult {
                    finished: true,
                    position: Some(1),
                    ..Default::default()
                },
            },
        )
        .unwrap();
        assert_eq!(state.frame, Some(Frame::None), "chapter 3 removed the phone");
        assert!(
            out.contains(&Output::Held {
                content: "results_01".into()
            }),
            "the message waits for a frame: {out:?}"
        );
        assert_eq!(
            state.content_facts("results_01").delivered,
            0,
            "waiting, it is not delivered"
        );
        assert_eq!(
            state.wait,
            Wait::Reply {
                content: "talk_01".into()
            },
            "the face-to-face dialogue holds the flow until it is answered"
        );
        state
    }

    /// Answers chapter 3's dialogue: the flow goes on to the race launched
    /// from the scene.
    fn answer_the_dialogue(story: &Story, state: &mut State) {
        let reputation = state.reputation;
        submit(
            story,
            state,
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
            Wait::Event {
                event: "race_03".into()
            },
            "a race with no frame"
        );
    }

    // L1's "done": the engine runs a sample story from start to end, the
    // way `story-tool play` does. The sample is the shipped example of the
    // format, so it must also be spotless. Its chapter 3 has no frame
    // (L3.4): a message sent then waits, undelivered, until the phone is
    // back, and shows dated then.
    #[test]
    fn the_sample_story_is_clean_and_plays_to_its_end() {
        let story = sample();
        assert_eq!(validate(&story), [], "no diagnostic on the sample");
        let mut state = up_to_chapter_3(&story);
        answer_the_dialogue(&story, &mut state);
        drive(&story, &mut state, "race_03", 4);
        assert_eq!(
            state.wait,
            Wait::Ended {
                ending: "finished".into()
            }
        );
        assert_eq!(
            state.frame,
            Some(Frame::Named("phone".into())),
            "the phone put back after the face-to-face chapter"
        );
        for content in [
            "sponsor_offer",
            "rival_post_01",
            "garage_remark",
            "congrats_01",
            "talk_01",
            "results_01",
            "finish_line",
        ] {
            assert_eq!(state.content_facts(content).delivered, 1, "{content} delivered once");
        }
        assert!(state.held.is_empty(), "nothing left waiting");
        let shown = (state.history.iter())
            .find(|e| matches!(&e.what, Happening::Delivered { content, .. } if content == "results_01"))
            .expect("delivered");
        assert_eq!(
            shown.date.to_string(),
            "2001-06-08",
            "dated when the phone showed it, a week after the race"
        );
        assert_eq!(
            state.content_facts("invite_02").delivered,
            0,
            "the branch skipped the second ask"
        );
    }

    // The sample shows its places (SPEC-story-mode, Scenes, frame and
    // sound): the bedroom at the start, the garage once visited; in chapter
    // 3 the dialogue plays in its own place, the garage, at night since the
    // Beat before it set the clock to the evening, while the current place
    // stays the bedroom.
    #[test]
    fn the_sample_shows_its_places_and_their_variants() {
        let story = sample();
        let mut state = State::new(&story, 1);
        assert_eq!(state.scene.as_deref(), Some("bedroom"), "the starting place");
        state = up_to_chapter_3(&story);
        let view = crate::view::play_view(&story, &state);
        let current = view.scene.expect("a place");
        assert_eq!(
            (current.place.as_str(), current.variant.as_str()),
            ("bedroom", "default")
        );
        let talk = (view.items.iter()).find(|i| i.content == "talk_01").unwrap();
        let own = talk.scene.as_ref().expect("its own place");
        assert_eq!(
            (own.place.as_str(), own.variant.as_str()),
            ("garage", "night"),
            "21:30 is the evening"
        );
        assert_eq!(own.file, "scenes/garage_night.webp");
        answer_the_dialogue(&story, &mut state);
        drive(&story, &mut state, "race_03", 4);
        assert_eq!(state.scene.as_deref(), Some("bedroom"), "back home with the phone");
        assert_eq!(
            state.variant, None,
            "the forced daylight ended with the place set again"
        );
    }

    // A game saved in the middle of the chapter with no frame resumes with
    // what waits for the phone, which comes out when the phone is back.
    #[test]
    fn a_save_in_the_chapter_with_no_frame_keeps_what_waits_for_a_frame() {
        let story = sample();
        let mut state = up_to_chapter_3(&story);
        answer_the_dialogue(&story, &mut state);
        let mut back = Save::from_json(&Save::new(&story, &state).to_json())
            .and_then(|s| s.resume(&story))
            .expect("the save resumes");
        assert_eq!(back, state, "the same state, the held message included");
        assert_eq!(back.held.len(), 1, "the message still waits");
        drive(&story, &mut back, "race_03", 1);
        assert_eq!(
            back.content_facts("results_01").delivered,
            1,
            "out once the phone is back"
        );
        assert!(back.held.is_empty());
    }
}
