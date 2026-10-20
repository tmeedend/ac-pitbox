//! What the player sees (SPEC-story-mode, Story Mode: player side), rebuilt
//! from a state alone: the items the frame and the scene have shown, each with
//! the lines said and the replies chosen, the choices open now, and what the
//! flow waits for. Rebuilt from the history rather than kept by the host, so
//! a game resumed from its save shows exactly what was seen before - "the
//! frame stays the record of the character's life".

use serde::Serialize;

use crate::engine::{difficulty_step, fee_due, offers, Offer, Step};
use crate::load::Story;
use crate::model::{DateDisplay, EventType, Session};
use crate::state::{Happening, State, Wait};

/// The game as the screen shows it.
#[derive(Debug, Clone, Serialize)]
pub struct PlayView {
    pub title: String,
    /// The active frame: `none`, or a frame's name.
    pub frame: String,
    /// `YYYY-MM-DD` and `HH:MM`, in the story's time.
    pub date: String,
    pub clock: String,
    pub date_display: DateDisplay,
    /// Only when the story has an economy.
    pub money: Option<i64>,
    pub reputation: i64,
    pub followers: i64,
    /// In the order they arrived. An item delivered twice shows twice: a new
    /// delivery is a new conversation.
    pub items: Vec<SeenItem>,
    pub waiting: Waiting,
    /// The event a session was launched for, until its result comes back.
    pub launched: Option<String>,
}

/// One delivery of a content item and what was said in it since.
#[derive(Debug, Clone, Serialize)]
pub struct SeenItem {
    pub content: String,
    /// Its content type, as a `.story` header names it.
    pub kind: &'static str,
    pub from: Option<String>,
    /// A dialogue's characters present.
    pub characters: Vec<String>,
    /// When it arrived; an item's own clock wins over the story's.
    pub date: String,
    pub clock: String,
    pub steps: Vec<Step>,
    /// What its conversation offers now; empty unless it waits for a reply.
    pub choices: Vec<Offer>,
    /// Its conversation is under way: it waits at a choice point.
    pub open: bool,
    /// The player has opened it (`Input::Seen`).
    pub seen: bool,
}

/// What the flow waits for, with what the screen needs to show it.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Waiting {
    /// The summary before a session: what, where, in what, for what.
    Event {
        event: String,
        name: Option<String>,
        event_type: EventType,
        /// Boxed: the other variants are a name.
        session: Box<Session>,
        /// Its success criteria, as written.
        success: Vec<String>,
        /// The attempt this launch would be, from 1.
        attempt: u32,
        fee: Option<i64>,
        difficulty_step: u32,
    },
    Reply {
        content: String,
    },
    Action {
        action: String,
    },
    Ended {
        ending: String,
    },
}

pub fn play_view(story: &Story, state: &State) -> PlayView {
    let mut items: Vec<SeenItem> = Vec::new();
    for entry in &state.history {
        match &entry.what {
            Happening::Delivered { content, .. } => {
                let item = story.content_item(content);
                items.push(SeenItem {
                    content: content.clone(),
                    kind: item.map_or("message", |i| i.kind.name()),
                    from: item.and_then(|i| i.from.clone()),
                    characters: item.map(|i| i.characters.clone()).unwrap_or_default(),
                    date: entry.date.to_string(),
                    clock: item.and_then(|i| i.clock).unwrap_or(entry.clock).to_string(),
                    steps: Vec::new(),
                    choices: Vec::new(),
                    open: false,
                    seen: false,
                });
            }
            Happening::Said { content, lines } => {
                if let Some(seen) = latest(&mut items, content) {
                    seen.steps.extend(lines.iter().cloned().map(Step::Said));
                }
            }
            Happening::Replied { content, reply, text } => {
                if let Some(seen) = latest(&mut items, content) {
                    seen.steps.push(Step::Chose(Offer {
                        label: reply.clone(),
                        text: text.clone(),
                    }));
                }
            }
            _ => {}
        }
    }
    for (content, conversation) in &state.conversations {
        let script = story.content_item(content).map(|i| &i.script);
        if let (Some(script), Some(seen)) = (script, latest(&mut items, content)) {
            seen.choices = offers(script, &conversation.at, &conversation.offered);
            seen.open = true;
        }
    }
    // `seen` is the latest delivery's: an earlier one, superseded, counts as
    // read, or it would stay unread forever. From the end, so that the first
    // met of each item is its latest.
    let mut met = std::collections::HashSet::new();
    for seen in items.iter_mut().rev() {
        let latest = met.insert(seen.content.clone());
        seen.seen = !latest || state.content_facts(&seen.content).seen;
    }
    PlayView {
        title: story.manifest.title.clone(),
        frame: state.frame.as_ref().unwrap_or(&story.manifest.start.frame).to_string(),
        date: state.date.to_string(),
        clock: state.clock.to_string(),
        date_display: story.settings().date,
        money: story.settings().money.then_some(state.money),
        reputation: state.reputation,
        followers: state.followers,
        items,
        waiting: waiting(story, state),
        launched: state.launched.clone(),
    }
}

/// The last delivery of `content`: lines and replies belong to the
/// conversation under way, never to an earlier one.
fn latest<'a>(items: &'a mut [SeenItem], content: &str) -> Option<&'a mut SeenItem> {
    items.iter_mut().rev().find(|i| i.content == content)
}

fn waiting(story: &Story, state: &State) -> Waiting {
    match &state.wait {
        Wait::Event { event } => {
            let e = story.event(event);
            Waiting::Event {
                event: event.clone(),
                name: e.and_then(|e| e.name.clone()),
                event_type: e.map_or(EventType::Race, |e| e.kind),
                session: Box::new(e.map(|e| e.session.clone()).unwrap_or_default()),
                success: e
                    .map(|e| e.success.iter().map(ToString::to_string).collect())
                    .unwrap_or_default(),
                attempt: state.event(event).attempts + 1,
                fee: fee_due(story, state, event),
                difficulty_step: difficulty_step(story, state, event),
            }
        }
        Wait::Reply { content } => Waiting::Reply {
            content: content.clone(),
        },
        Wait::Action { action } => Waiting::Action { action: action.clone() },
        Wait::Ended { ending } => Waiting::Ended { ending: ending.clone() },
        // Never handed back by the engine: a state between two steps.
        Wait::Running => Waiting::Ended { ending: String::new() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{run, submit, Input};
    use crate::save::Save;

    fn sample() -> Story {
        Story::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).expect("the sample loads")
    }

    fn said(steps: &[Step]) -> Vec<String> {
        steps
            .iter()
            .map(|s| match s {
                Step::Said(l) => l.text.clone(),
                Step::Chose(c) => format!("> {}", c.text),
            })
            .collect()
    }

    // The frame is the record of what was seen: a game resumed from its
    // save shows the same lines, the same reply and the same open choices
    // as before it was saved (SPEC-story-mode, Story Mode: player side).
    #[test]
    fn a_resumed_game_shows_what_was_seen_before() {
        let story = sample();
        let mut state = State::new(&story, 7);
        run(&story, &mut state).expect("runs");
        let before = play_view(&story, &state);
        let invite = before
            .items
            .iter()
            .find(|i| i.content == "invite_01")
            .expect("the invite arrived");
        assert!(invite.open, "the invite waits for an answer");
        assert!(!invite.choices.is_empty(), "with its replies");
        assert!(matches!(&before.waiting, Waiting::Reply { content } if content == "invite_01"));

        let label = invite.choices[0].label.clone();
        let text = invite.choices[0].text.clone();
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "invite_01".into(),
                reply: label,
            },
        )
        .expect("answered");
        let resumed = Save::from_json(&Save::new(&story, &state).to_json())
            .and_then(|s| s.resume(&story))
            .expect("the save resumes");
        let after = play_view(&story, &resumed);
        assert_eq!(
            format!("{:?}", after.items),
            format!("{:?}", play_view(&story, &state).items),
            "the same record before and after the save"
        );
        let invite = after.items.iter().find(|i| i.content == "invite_01").unwrap();
        assert!(
            said(&invite.steps).contains(&format!("> {text}")),
            "the reply as the player read it"
        );
        assert!(
            !invite.open && invite.choices.is_empty(),
            "answered, the conversation is closed"
        );
    }

    // A new delivery is a new item, unseen: having opened the last one
    // says nothing about this one.
    #[test]
    fn an_item_delivered_again_is_unseen_again() {
        let files = crate::testutil::with(
            crate::testutil::minimal(),
            "flow/01-one.yaml",
            "id: ch01\nnodes:\n  - id: start\n    message: hello\n  - id: wait\n    action: go\n  - id: again\n    message: hello\n  - id: end\n    end: done\n",
        );
        let files = crate::testutil::with(files, "content/a.story", "---\nid: hello\ntype: message\n---\nHello.\n");
        let story = Story::from_sources(files).expect("loads");
        let mut state = State::new(&story, 1);
        run(&story, &mut state).unwrap();
        submit(
            &story,
            &mut state,
            Input::Seen {
                content: "hello".into(),
            },
        )
        .unwrap();
        assert!(play_view(&story, &state).items[0].seen, "opened");
        submit(&story, &mut state, Input::ActionDone { action: "go".into() }).unwrap();
        let view = play_view(&story, &state);
        assert_eq!(view.items.len(), 2, "two deliveries, two items");
        assert!(!view.items[1].seen, "the new one is unseen");
        assert!(view.items[0].seen, "the old one stays read");
    }

    // A flow waiting on an event shows its summary: what success takes and
    // which attempt this is.
    #[test]
    fn an_event_waits_with_its_summary() {
        let story = sample();
        let mut state = State::new(&story, 7);
        run(&story, &mut state).unwrap();
        let accept = play_view(&story, &state).items[0].choices[0].label.clone();
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "invite_01".into(),
                reply: accept,
            },
        )
        .unwrap();
        let Waiting::Event {
            event,
            success,
            attempt,
            fee,
            ..
        } = play_view(&story, &state).waiting
        else {
            panic!("the race waits");
        };
        assert_eq!(
            (event.as_str(), success, attempt, fee),
            ("race_01", vec!["position <= 3".to_string()], 1, Some(50))
        );
    }
}
