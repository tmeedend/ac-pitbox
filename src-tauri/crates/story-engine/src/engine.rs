//! Running a story (SPEC-story-mode, Story model): advance the flow as far as
//! it goes, deliver whatever reactive content has become true, and stop on
//! what the player must do - drive an event, answer, act in the frame.
//!
//! The host (Story Mode, the CLI, later the balance simulator) only ever
//! calls [`run`] once and then [`submit`] with what the player did. Both hand
//! back what happened, ending with what the flow now waits for. Every rule
//! the story's settings decide is read here from `Settings`, never assumed.

use std::fmt;

use crate::expr::{Criterion, Effect, Literal, Var};
use crate::frame::Frame;
use crate::load::Story;
use crate::model::{Beat, ContentType, FailureMode, Node, NodeKind, Placement, Repeat, RetryCost};
use crate::script::{self, Position, Stop};
use crate::state::{Conversation, Entry, EventResult, Happening, Held, Scheduled, State, Verdict, Via, Wait};
use crate::time::Span;

/// What the host must show or do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Delivered {
        content: String,
        via: Via,
    },
    TimeMoved {
        to: crate::time::StoryDate,
    },
    /// A Beat changed the frame (`none`: removed it).
    FrameChanged {
        to: Frame,
    },
    /// A Beat set the place behind the frame, or forced one of its variants.
    SceneChanged {
        place: String,
        variant: Option<String>,
    },
    /// Sent while no active frame can show it: it waits for one that can,
    /// and is `Delivered` then.
    Held {
        content: String,
    },
    /// Lines of a content item's conversation, in order, with what held.
    Lines {
        content: String,
        lines: Vec<Said>,
    },
    /// The choices a conversation offers now; the player answers with
    /// `Input::Reply` and one of these labels.
    Choices {
        content: String,
        choices: Vec<Offer>,
    },
    /// The flow waits for this event. `difficulty_step`: how many steps of
    /// skill the launcher takes off the opponents (adaptive difficulty).
    AwaitEvent {
        event: String,
        attempt: u32,
        difficulty_step: u32,
        fee: Option<i64>,
    },
    AwaitReply {
        content: String,
    },
    AwaitAction {
        action: String,
    },
    Ended {
        ending: String,
    },
}

/// A line as the frame or the scene shows it, kept in the history.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Said {
    /// A character's id; `None`: the item's sender.
    pub speaker: Option<String>,
    pub text: String,
    pub expression: Option<String>,
    pub position: Option<Position>,
    /// The speaker leaves the stage after this line. Left out of a save
    /// when false, as a save written before it existed reads it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave: bool,
}

/// A choice offered: its label and its text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Offer {
    pub label: String,
    pub text: String,
}

/// What the player did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A session starts for this event (the flow's, or an open side event):
    /// the entry fee is paid now.
    Launch { event: String },
    /// Its result came back.
    Result { event: String, result: EventResult },
    /// The player chose a reply - a choice's label - in a conversation
    /// waiting at a choice point.
    Reply { content: String, reply: String },
    /// The action the flow waits for is done.
    ActionDone { action: String },
    /// The player opened a content item.
    Seen { content: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// More steps than any story needs without waiting for the player: a
    /// cycle of beats and branches the state never leaves.
    Loop {
        node: String,
    },
    UnknownNode(String),
    /// A node with no `next` at the end of the flow.
    FallsOffFlow {
        node: String,
    },
    /// The input does not answer what the flow waits for.
    Unexpected(String),
    CannotAfford {
        fee: i64,
        money: i64,
    },
    StoryEnded,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Loop { node } => {
                write!(f, "the flow loops around `{node}` without ever waiting for the player")
            }
            EngineError::UnknownNode(n) => write!(f, "unknown node `{n}`"),
            EngineError::FallsOffFlow { node } => {
                write!(f, "node `{node}` leads nowhere: the flow ends without an `end` node")
            }
            EngineError::Unexpected(why) => f.write_str(why),
            EngineError::CannotAfford { fee, money } => write!(f, "entry fee {fee}, money {money}"),
            EngineError::StoryEnded => f.write_str("the story has ended"),
        }
    }
}

/// Steps without waiting before the engine calls it a loop. A real chapter
/// is a few dozen nodes; a beat-and-branch counting loop a few hundred.
const MAX_STEPS: usize = 10_000;

/// Conversations started from within another one (`effect("send …")`)
/// before the engine calls it a loop: two items sending each other.
const MAX_NESTED: usize = 32;

/// Advances the flow from where the state stands, until it waits.
pub fn run(story: &Story, state: &mut State) -> Result<Vec<Output>, EngineError> {
    let mut r = Run::new(story, state);
    r.advance()?;
    Ok(r.out)
}

/// Applies what the player did, then advances the flow. All or nothing: on
/// an error the state is left as it was, so the host can tell the player and
/// let them try again - a result refused half-way must not lose the launch.
pub fn submit(story: &Story, state: &mut State, input: Input) -> Result<Vec<Output>, EngineError> {
    if matches!(state.wait, Wait::Ended { .. }) && !matches!(input, Input::Seen { .. }) {
        return Err(EngineError::StoryEnded);
    }
    let mut next = state.clone();
    let mut r = Run::new(story, &mut next);
    r.apply(input)?;
    r.advance()?;
    let out = r.out;
    *state = next;
    Ok(out)
}

/// One step of a conversation as the player saw it, in a preview or in a
/// game: a line said, or the reply chosen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Said(Said),
    Chose(Offer),
}

/// A content item played on its own (SPEC-story-mode, Create Your Own:
/// Content, live preview).
#[derive(Debug, Clone)]
pub struct Preview {
    pub steps: Vec<Step>,
    /// What the conversation offers where it stopped; empty once it ended.
    pub choices: Vec<Offer>,
    pub ended: bool,
    /// The other content items it sent (`effect("send …")`).
    pub sent: Vec<String>,
    /// The state once `setup` applied, and once the conversation played:
    /// what changed between the two is what the conversation did.
    pub before: State,
    pub after: State,
}

/// Plays content `id` on its own, for the Content screen's preview: from the
/// story's starting state with `setup` applied (the values the author set to
/// preview with), then each of `replies` in turn, as long as it is offered -
/// the preview stays right while the text changes under it. The flow never
/// moves and no reactive content comes: only this conversation plays.
pub fn preview(story: &Story, setup: &[Effect], id: &str, replies: &[String]) -> Result<Preview, EngineError> {
    let mut state = State::new(story, 0);
    let mut r = Run::new(story, &mut state);
    // The item plays wherever it would arrive: the preview shows the checks'
    // warning beside it when no frame can show it.
    r.held_by_frame = false;
    for effect in setup {
        r.apply_effect(effect)?;
    }
    r.out.clear();
    let before = r.state.clone();
    let mut steps = Vec::new();
    let mut sent = Vec::new();
    let mut offered: Vec<Offer> = Vec::new();
    let mut collect = |out: Vec<Output>, steps: &mut Vec<Step>, offered: &mut Vec<Offer>| {
        for o in out {
            match o {
                Output::Lines { content, lines } if content == id => steps.extend(lines.into_iter().map(Step::Said)),
                Output::Choices { content, choices } if content == id => *offered = choices,
                Output::Delivered { content, .. } if content != id => sent.push(content),
                _ => {}
            }
        }
    };
    r.deliver(id, Via::Flow)?;
    collect(std::mem::take(&mut r.out), &mut steps, &mut offered);
    for label in replies {
        let Some(choice) = offered.iter().find(|c| &c.label == label).cloned() else {
            break;
        };
        if !r.state.conversations.contains_key(id) {
            break;
        }
        steps.push(Step::Chose(choice));
        offered.clear();
        r.reply(id.into(), label.clone())?;
        collect(std::mem::take(&mut r.out), &mut steps, &mut offered);
    }
    let ended = !r.state.conversations.contains_key(id);
    if ended {
        offered.clear();
    }
    Ok(Preview {
        steps,
        choices: offered,
        ended,
        sent,
        before,
        after: state,
    })
}

/// The choices a conversation stopped at offers, label and text, in the
/// order the script reached them: what the frame shows, now or on reload.
pub fn offers(script: &script::Script, at: &[usize], offered: &[String]) -> Vec<Offer> {
    let written = script::choices_at(script, at).unwrap_or_default();
    (offered.iter())
        .filter_map(|label| written.iter().find(|c| &c.label == label))
        .map(|c| Offer {
            label: c.label.clone(),
            text: c.text.clone(),
        })
        .collect()
}

/// Whether a result satisfies an event's success criteria: all of them, and
/// finishing when there are none.
pub fn succeeded(criteria: &[Criterion], result: &EventResult) -> bool {
    if let Some(verdict) = result.forced {
        return verdict == Verdict::Success;
    }
    if !result.finished {
        return false;
    }
    criteria.iter().all(|c| match c {
        Criterion::Finished => true,
        Criterion::PositionAtMost(n) => result.position.is_some_and(|p| p <= *n),
        Criterion::AheadOf(driver) => result.ahead_of.iter().any(|d| d == driver),
        Criterion::LapUnder(ms) => result.best_lap_ms.is_some_and(|t| t < *ms),
    })
}

/// Adaptive difficulty (SPEC-story-mode, Hard events): a step every `every`
/// consecutive failures, up to `cap`.
pub fn difficulty_step(story: &Story, state: &State, event: &str) -> u32 {
    match story.settings().adaptive_difficulty {
        Some(a) if a.every > 0 => (state.event(event).consecutive_failures / a.every).min(a.cap),
        _ => 0,
    }
}

/// The fee a launch of `event` would cost now, if any.
pub fn fee_due(story: &Story, state: &State, event: &str) -> Option<i64> {
    let fee = story.event(event)?.entry_fee.filter(|_| story.settings().money)?;
    let side = state.side_events.contains(event) && !matches!(&state.wait, Wait::Event { event: e } if e == event);
    let pays = side || story.settings().retry_cost == RetryCost::Paid || state.event(event).paid == 0;
    pays.then_some(fee)
}

fn unexpected(why: String) -> Result<(), EngineError> {
    Err(EngineError::Unexpected(why))
}

struct Run<'a> {
    story: &'a Story,
    state: &'a mut State,
    out: Vec<Output>,
    /// How deep `deliver` is in conversations started by conversations.
    nested: usize,
    /// Content no active frame can show waits for one (`State::held`).
    held_by_frame: bool,
}

impl<'a> Run<'a> {
    fn new(story: &'a Story, state: &'a mut State) -> Run<'a> {
        Run {
            story,
            state,
            out: Vec::new(),
            nested: 0,
            held_by_frame: true,
        }
    }

    fn log(&mut self, what: Happening) {
        self.state.history.push(Entry {
            date: self.state.date,
            clock: self.state.clock,
            what,
        });
    }

    fn node(&self, id: &str) -> Result<&'a Node, EngineError> {
        let story: &'a Story = self.story;
        story.node(id).ok_or_else(|| EngineError::UnknownNode(id.into()))
    }

    /// Moves the cursor along a node's normal exit.
    fn go_next(&mut self, from: &str) -> Result<(), EngineError> {
        let node = self.node(from)?;
        let next = self
            .story
            .next_of(node)
            .ok_or_else(|| EngineError::FallsOffFlow { node: from.into() })?;
        self.go_to(next.id.clone());
        Ok(())
    }

    fn go_to(&mut self, node: String) {
        self.state.node = node;
        self.state.wait = Wait::Running;
    }

    fn advance(&mut self) -> Result<(), EngineError> {
        self.check_reactive()?;
        let mut steps = 0;
        while self.state.wait == Wait::Running {
            steps += 1;
            if steps > MAX_STEPS {
                return Err(EngineError::Loop {
                    node: self.state.node.clone(),
                });
            }
            let id = self.state.node.clone();
            let node = self.node(&id)?;
            match &node.kind {
                NodeKind::Beat(beat) => {
                    self.beat(beat)?;
                    self.go_next(&id)?;
                }
                NodeKind::Message {
                    content,
                    wait_reply,
                    on_reply,
                } => {
                    let content = content.clone();
                    self.deliver(&content, Via::Flow)?;
                    let dialogue = self.story.content_item(&content).map(|c| c.kind) == Some(ContentType::Dialogue);
                    let holds = *wait_reply || !on_reply.is_empty() || dialogue;
                    if holds && self.state.conversations.contains_key(&content) {
                        self.state.wait = Wait::Reply { content };
                    } else {
                        self.leave_message(&id, &content)?;
                    }
                }
                NodeKind::Event { event, .. } => self.state.wait = Wait::Event { event: event.clone() },
                NodeKind::Branch { cases, otherwise } => {
                    let target = cases
                        .iter()
                        .find(|c| self.state.all_hold(self.story, &c.when))
                        .map_or(otherwise, |c| &c.goto);
                    self.go_to(target.clone());
                }
                NodeKind::Action { action } => self.state.wait = Wait::Action { action: action.clone() },
                NodeKind::SideEvents { open, close } => {
                    self.state.side_events.extend(open.iter().cloned());
                    for e in close {
                        self.state.side_events.remove(e);
                    }
                    self.go_next(&id)?;
                }
                NodeKind::End { ending } => {
                    let ending = ending.clone();
                    self.log(Happening::Ended { ending: ending.clone() });
                    self.state.wait = Wait::Ended { ending };
                }
            }
            self.check_reactive()?;
        }
        let awaiting = match &self.state.wait {
            Wait::Event { event } => Output::AwaitEvent {
                event: event.clone(),
                attempt: self.state.event(event).attempts + 1,
                difficulty_step: difficulty_step(self.story, self.state, event),
                fee: fee_due(self.story, self.state, event),
            },
            Wait::Reply { content } => Output::AwaitReply {
                content: content.clone(),
            },
            Wait::Action { action } => Output::AwaitAction { action: action.clone() },
            Wait::Ended { ending } => Output::Ended { ending: ending.clone() },
            Wait::Running => unreachable!("the loop above only exits once the flow waits"),
        };
        self.out.push(awaiting);
        Ok(())
    }

    fn beat(&mut self, beat: &Beat) -> Result<(), EngineError> {
        // Before the effects: a `send` in this beat goes to the frame the
        // beat puts up.
        if let Some(frame) = beat.frame.as_ref().filter(|f| self.state.frame.as_ref() != Some(*f)) {
            self.state.frame = Some(frame.clone());
            self.out.push(Output::FrameChanged { to: frame.clone() });
            self.release_held()?;
        }
        self.set_scene(beat);
        self.state.unlocked.extend(beat.unlock.iter().cloned());
        if let Some(status) = &beat.status {
            self.state.status.apply(status);
        }
        (self.state.skins).extend(beat.skins.iter().map(|(a, s)| (a.clone(), s.clone())));
        for effect in &beat.effects {
            self.apply_effect(effect)?;
        }
        if let Some(clock) = beat.clock {
            self.state.clock = clock;
        }
        if let Some(span) = beat.advance {
            self.move_time(span);
        }
        Ok(())
    }

    fn move_time(&mut self, span: Span) {
        let from = self.state.date;
        self.state.date = from.advance(span);
        self.log(Happening::TimeMoved { from });
        self.out.push(Output::TimeMoved { to: self.state.date });
    }

    /// The place, its forced variant, the weather and the mood a Beat sets
    /// (SPEC-story-mode, Scenes, frame and sound). A place set again lets
    /// the state pick its variant again; `none` clears the weather or mood.
    fn set_scene(&mut self, beat: &Beat) {
        let state = &mut *self.state;
        let before = (state.scene.clone(), state.variant.clone());
        if let Some(place) = &beat.scene {
            state.scene = Some(place.clone());
            state.variant = None;
        }
        if let Some(variant) = &beat.variant {
            state.variant = Some(variant.clone());
        }
        for (value, into) in [(&beat.weather, &mut state.weather), (&beat.mood, &mut state.mood)] {
            match value.as_deref() {
                Some("none") => *into = None,
                Some(v) => *into = Some(v.to_string()),
                None => {}
            }
        }
        let after = (state.scene.clone(), state.variant.clone());
        if after != before {
            if let (Some(place), variant) = after {
                self.out.push(Output::SceneChanged { place, variant });
            }
        }
    }

    /// The frame up now.
    fn frame(&self) -> &Frame {
        self.state.frame.as_ref().unwrap_or(&self.story.manifest.start.frame)
    }

    /// Whether the frame up now can show `content`. An unknown item can:
    /// there is nothing to hold.
    fn can_show(&self, content: &str) -> bool {
        (self.story.content_item(content)).is_none_or(|item| self.frame().can_show(self.story, item.kind))
    }

    /// Delivers, in the order they were sent, the held items the frame now
    /// up can show; dated now, when they show. The others keep waiting.
    fn release_held(&mut self) -> Result<(), EngineError> {
        let (now, still): (Vec<Held>, Vec<Held>) = std::mem::take(&mut self.state.held)
            .into_iter()
            .partition(|h| self.can_show(&h.content));
        self.state.held = still;
        for held in now {
            self.deliver(&held.content, held.via)?;
        }
        Ok(())
    }

    /// Sends a content item, and plays its conversation from the start: a
    /// new delivery is a new conversation, so the replies of the last one
    /// are forgotten, and so is having seen it - the player has not seen
    /// this one yet. Content the frame up now cannot show waits for one
    /// that can, undelivered (SPEC-story-mode, Nowhere to show).
    fn deliver(&mut self, content: &str, via: Via) -> Result<(), EngineError> {
        if self.held_by_frame && !self.can_show(content) {
            self.state.held.push(Held {
                content: content.into(),
                via,
            });
            self.out.push(Output::Held {
                content: content.into(),
            });
            return Ok(());
        }
        let facts = self.state.content.entry(content.to_string()).or_default();
        facts.delivered += 1;
        facts.seen = false;
        facts.reply = None;
        facts.chosen.clear();
        self.state.conversations.remove(content);
        self.log(Happening::Delivered {
            content: content.into(),
            via: via.clone(),
        });
        self.out.push(Output::Delivered {
            content: content.into(),
            via,
        });
        self.nested += 1;
        if self.nested > MAX_NESTED {
            return Err(EngineError::Loop {
                node: self.state.node.clone(),
            });
        }
        let played = self.converse(content, vec![0]);
        self.nested -= 1;
        played
    }

    /// Plays `content`'s script from `at` until a choice point or its end
    /// (SPEC-story-mode, Dialogue text). At the end, a flow waiting on this
    /// conversation goes on along the node's reply exit.
    fn converse(&mut self, content: &str, at: script::At) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let Some(item) = story.content_item(content) else {
            return Ok(());
        };
        let mut host = Speaking {
            run: self,
            said: Vec::new(),
        };
        let stop = script::play(&item.script, at, &mut host)?;
        let said = host.said;
        if !said.is_empty() {
            self.log(Happening::Said {
                content: content.into(),
                lines: said.clone(),
            });
            self.out.push(Output::Lines {
                content: content.into(),
                lines: said,
            });
        }
        match stop {
            Stop::Choices { at, offered } => {
                let choices = offers(&item.script, &at, &offered);
                self.state
                    .conversations
                    .insert(content.into(), Conversation { at, offered });
                self.out.push(Output::Choices {
                    content: content.into(),
                    choices,
                });
            }
            Stop::Ended => {
                self.state.conversations.remove(content);
                if matches!(&self.state.wait, Wait::Reply { content: c } if c == content) {
                    let node = self.state.node.clone();
                    self.leave_message(&node, content)?;
                }
            }
        }
        Ok(())
    }

    /// Leaves a Message node: along the exit of the most recent reply that
    /// has one, else along `next`.
    fn leave_message(&mut self, node: &str, content: &str) -> Result<(), EngineError> {
        let exit = match &self.node(node)?.kind {
            NodeKind::Message { on_reply, .. } => (self.state.content_facts(content).chosen.iter().rev())
                .find_map(|label| on_reply.get(label))
                .cloned(),
            _ => None,
        };
        match exit {
            Some(target) => {
                self.go_to(target);
                Ok(())
            }
            None => self.go_next(node),
        }
    }

    fn apply_effect(&mut self, effect: &Effect) -> Result<(), EngineError> {
        if let Effect::Send(content) = effect {
            return self.deliver(content, Via::Effect);
        }
        let s = &mut *self.state;
        let scale = &self.story.settings().relations;
        match effect {
            Effect::Send(_) => {}
            Effect::Add(var, n) => match var {
                Var::Reputation => s.reputation += n,
                Var::Followers => s.followers += n,
                Var::Money => s.money += n,
                Var::Counter(c) => *s.counters.entry(c.clone()).or_default() += n,
                Var::Rel(who) => {
                    if let Some(level) = s.relations.get(who).and_then(|l| scale.shift(l, *n)) {
                        s.relations.insert(who.clone(), level.to_string());
                    }
                }
                _ => {}
            },
            Effect::Set(var, value) => match (var, value) {
                (Var::Reputation, Literal::Int(n)) => s.reputation = *n,
                (Var::Followers, Literal::Int(n)) => s.followers = *n,
                (Var::Money, Literal::Int(n)) => s.money = *n,
                (Var::Counter(c), Literal::Int(n)) => {
                    s.counters.insert(c.clone(), *n);
                }
                (Var::Flag(f), Literal::Bool(b)) | (Var::Car(f), Literal::Bool(b)) => {
                    let set = if matches!(var, Var::Flag(_)) {
                        &mut s.flags
                    } else {
                        &mut s.cars
                    };
                    if *b {
                        set.insert(f.clone());
                    } else {
                        set.remove(f);
                    }
                }
                (Var::Rel(who), Literal::Name(level)) if scale.rank(level).is_some() => {
                    s.relations.insert(who.clone(), level.clone());
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// Delivers the reactive content that has become due (SPEC-story-mode,
    /// Reactive content). Runs after every change of state; repeats while a
    /// delivery makes something else true, since a condition can read
    /// `content.<id>.delivered`.
    fn check_reactive(&mut self) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let mut reactive: Vec<_> = story
            .content
            .iter()
            .filter_map(|c| match &c.placement {
                Placement::Reactive {
                    when,
                    delay,
                    priority,
                    repeat,
                } => Some((c.id.as_str(), when, *delay, *priority, *repeat)),
                _ => None,
            })
            .collect();
        // Higher priority first; equal ones keep the order of the files.
        reactive.sort_by_key(|&(_, _, _, priority, _)| std::cmp::Reverse(priority));
        for _ in 0..MAX_STEPS {
            let mut changed = false;
            // Delays first: an item whose conditions no longer hold when its
            // delay runs out is dropped, and may be scheduled again later.
            let date = self.state.date;
            let (due, waiting): (Vec<Scheduled>, Vec<Scheduled>) = std::mem::take(&mut self.state.scheduled)
                .into_iter()
                .partition(|s| s.due <= date);
            self.state.scheduled = waiting;
            for s in due {
                if let Some(Placement::Reactive { when, .. }) =
                    self.story.content_item(&s.content).map(|c| &c.placement)
                {
                    if self.state.all_hold(self.story, when) {
                        self.deliver(&s.content, Via::Reactive)?;
                        changed = true;
                    }
                }
            }
            for &(id, when, delay, _, repeat) in &reactive {
                let holds = self.state.all_hold(self.story, when);
                let fire = match repeat {
                    Repeat::Once => holds && self.state.content_facts(id).delivered == 0,
                    Repeat::EachTime => {
                        let was = self.state.armed.contains(id);
                        if holds != was {
                            if holds {
                                self.state.armed.insert(id.to_string());
                            } else {
                                self.state.armed.remove(id);
                            }
                        }
                        holds && !was
                    }
                };
                // Already on its way: waiting for its delay, or for a frame.
                let pending = self.state.scheduled.iter().any(|s| s.content == id)
                    || self.state.held.iter().any(|h| h.content == id);
                if !fire || pending {
                    continue;
                }
                match delay {
                    Some(span) => self.state.scheduled.push(Scheduled {
                        content: id.into(),
                        due: date.advance(span),
                    }),
                    None => {
                        self.deliver(id, Via::Reactive)?;
                        changed = true;
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
        Err(EngineError::Loop {
            node: self.state.node.clone(),
        })
    }

    fn apply(&mut self, input: Input) -> Result<(), EngineError> {
        match input {
            Input::Launch { event } => self.launch(event)?,
            Input::Result { event, result } => {
                if self.state.launched.as_deref() != Some(event.as_str()) {
                    return unexpected(format!("no session was launched for `{event}`"));
                }
                self.state.launched = None;
                self.event_result(event, result)?;
            }
            Input::Reply { content, reply } => self.reply(content, reply)?,
            Input::ActionDone { action } => {
                if !matches!(&self.state.wait, Wait::Action { action: a } if *a == action) {
                    return unexpected(format!("the flow is not waiting for action `{action}`"));
                }
                self.log(Happening::ActionDone { action });
                let node = self.state.node.clone();
                self.go_next(&node)?;
            }
            Input::Seen { content } => {
                if self.state.content_facts(&content).delivered == 0 {
                    return unexpected(format!("`{content}` was never delivered"));
                }
                self.state.content.entry(content).or_default().seen = true;
            }
        }
        Ok(())
    }

    /// A session starts: the flow's event or an open side event. The entry
    /// fee is paid here, before the player drives, so that a player who
    /// cannot afford it is told before the game starts.
    fn launch(&mut self, event: String) -> Result<(), EngineError> {
        let flow_event = matches!(&self.state.wait, Wait::Event { event: e } if *e == event);
        if !flow_event && !self.state.side_events.contains(&event) {
            return unexpected(format!(
                "event `{event}` is neither the one the flow waits for nor an open side event"
            ));
        }
        if self.state.launched.as_deref() == Some(event.as_str()) {
            // Launched again before a result (the game was closed early):
            // the fee was already paid for this attempt.
            return Ok(());
        }
        if let Some(running) = &self.state.launched {
            // Its fee is paid: replacing it would lose the attempt. The host
            // closes it with a result first (a DNF when the game crashed).
            return unexpected(format!("`{running}` is still running: submit its result first"));
        }
        let fee = fee_due(self.story, self.state, &event);
        if let Some(fee) = fee {
            if self.state.money < fee {
                return Err(EngineError::CannotAfford {
                    fee,
                    money: self.state.money,
                });
            }
            self.state.money -= fee;
            self.state.events.entry(event.clone()).or_default().paid += 1;
        }
        self.state.launched = Some(event.clone());
        self.log(Happening::Launched { event, fee });
        Ok(())
    }

    /// A reply in any conversation waiting at a choice point - the one the
    /// flow waits on, or a reactive message answered later. The script goes
    /// on under the chosen label, and the flow with it once it ends.
    fn reply(&mut self, content: String, reply: String) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let Some(item) = story.content_item(&content) else {
            return unexpected(format!("unknown content `{content}`"));
        };
        let Some(conversation) = self.state.conversations.get(&content) else {
            return unexpected(format!("`{content}` waits for no reply"));
        };
        if !conversation.offered.contains(&reply) {
            return unexpected(format!(
                "`{content}` offers no reply `{reply}` (offered: {})",
                conversation.offered.join(", ")
            ));
        }
        let Some(next) = script::after_choice(&item.script, &conversation.at, &reply) else {
            return unexpected(format!("`{content}` has no choice `{reply}` any more"));
        };
        let text = (offers(&item.script, &conversation.at, std::slice::from_ref(&reply)).pop())
            .map(|o| o.text)
            .unwrap_or_default();
        let facts = self.state.content.entry(content.clone()).or_default();
        facts.reply = Some(reply.clone());
        facts.chosen.push(reply.clone());
        self.state.conversations.remove(&content);
        self.log(Happening::Replied {
            content: content.clone(),
            reply,
            text,
        });
        self.converse(&content, next)
    }

    /// SPEC-story-mode, Events, retries and failure. The result is recorded
    /// first, so that a failure pool's conditions can read it ("so close"
    /// after a P2).
    fn event_result(&mut self, event_id: String, result: EventResult) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let Some(event) = story.event(&event_id) else {
            return Err(EngineError::Unexpected(format!("unknown event `{event_id}`")));
        };
        let success = succeeded(&event.success, &result);
        let facts = self.state.events.entry(event_id.clone()).or_default();
        facts.attempts += 1;
        let attempt = facts.attempts;
        if success {
            facts.successes += 1;
            facts.consecutive_failures = 0;
        } else {
            facts.consecutive_failures += 1;
        }
        facts.last = Some(result.clone());
        self.log(Happening::Result {
            event: event_id.clone(),
            attempt,
            result,
            success,
        });

        let flow_node = match &self.state.wait {
            Wait::Event { event } if *event == event_id => Some(self.state.node.clone()),
            _ => None,
        };
        if success {
            for effect in &event.on_success.effects {
                self.apply_effect(effect)?;
            }
            // A side event never moves the story on.
            if let Some(node) = flow_node {
                if let Some(span) = event.on_success.advance {
                    self.move_time(span);
                }
                self.go_next(&node)?;
            }
            return Ok(());
        }

        for effect in &event.on_failure.effects {
            self.apply_effect(effect)?;
        }
        if let Some(pool) = &event.on_failure.pool {
            self.draw_pool(pool, event.on_failure.pick, &event_id)?;
        }
        let Some(node) = flow_node else { return Ok(()) };
        let NodeKind::Event { on_failure, .. } = &self.node(&node)?.kind else {
            return Ok(());
        };
        // The author's failure exit wins; without one, the story's setting.
        if let Some(target) = on_failure {
            self.go_to(target.clone());
            return Ok(());
        }
        let settings = self.story.settings();
        let retry = match settings.failure {
            FailureMode::Retry => settings.max_attempts.is_none_or(|max| attempt < max),
            FailureMode::Final => false,
        };
        if !retry {
            self.go_next(&node)?;
        }
        Ok(())
    }

    /// Sends `pick` variants of a failure pool, among those whose conditions
    /// hold, never one already sent until every eligible one has been.
    fn draw_pool(&mut self, pool: &str, pick: u8, event: &str) -> Result<(), EngineError> {
        let eligible: Vec<String> = self
            .story
            .pool_variants(pool)
            .filter(|(_, when)| self.state.all_hold(self.story, when))
            .map(|(item, _)| item.id.clone())
            .collect();
        let used = self.state.pools.entry(pool.to_string()).or_default();
        let mut fresh: Vec<String> = eligible.iter().filter(|id| !used.contains(*id)).cloned().collect();
        if fresh.is_empty() {
            used.clear();
            fresh = eligible;
        }
        for _ in 0..usize::from(pick).min(fresh.len()) {
            let id = fresh.remove(self.state.rng.below(fresh.len()));
            self.state.pools.entry(pool.to_string()).or_default().insert(id.clone());
            self.deliver(&id, Via::Pool { event: event.into() })?;
        }
        Ok(())
    }
}

/// The story's state as a script sees it while it plays.
struct Speaking<'r, 'a> {
    run: &'r mut Run<'a>,
    said: Vec<Said>,
}

impl script::Host for Speaking<'_, '_> {
    type Error = EngineError;

    fn holds(&mut self, when: &[crate::expr::Condition]) -> bool {
        self.run.state.all_hold(self.run.story, when)
    }

    fn effect(&mut self, effect: &Effect) -> Result<(), EngineError> {
        self.run.apply_effect(effect)
    }

    fn say(&mut self, line: &script::Line) {
        self.said.push(Said {
            speaker: line.speaker.clone(),
            text: line.text.clone(),
            expression: line.expression.clone(),
            position: line.position,
            leave: line.leave,
        });
    }
}

#[cfg(test)]
mod tests;
