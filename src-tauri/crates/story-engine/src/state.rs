//! The player's state in a story: where the flow is, what it waits for, every
//! variable, and the history of what happened. This is what a save holds
//! (`save.rs`), so every field is a persisted format: a field added later
//! takes `#[serde(default)]`, and nothing here is renamed in place.
//!
//! Maps are `BTreeMap`/`BTreeSet` so that a save is written in the same order
//! every time: two saves of the same state are the same file.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::expr::{Condition, ContentFact, EventFact, Literal, Var};
use crate::frame::Frame;
use crate::load::Story;
use crate::model::{ContentId, EventId, NodeId, PoolId};
use crate::time::{Clock, StoryDate};

/// What the flow is waiting for. `Running` only exists between two steps of
/// the engine: a state handed back to the host always waits for something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Wait {
    Running,
    Event { event: EventId },
    Reply { content: ContentId },
    Action { action: String },
    Ended { ending: String },
}

/// The result of a session, as the engine needs it. Deliberately its own
/// small type: the launcher's race result (`raceresult` in the app, L0) is
/// converted into it by the bridge (L3), so the engine never depends on the
/// app's session format.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventResult {
    pub finished: bool,
    pub position: Option<u32>,
    pub best_lap_ms: Option<u32>,
    /// Named drivers the player finished ahead of.
    #[serde(default)]
    pub ahead_of: Vec<String>,
    /// Overrides the success criteria: a playtest typing "failed", or an
    /// event without a measurable goal.
    #[serde(default)]
    pub forced: Option<Verdict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Success,
    Failure,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventFacts {
    pub attempts: u32,
    pub successes: u32,
    /// Reset by a success; drives adaptive difficulty.
    pub consecutive_failures: u32,
    /// Entry fees paid so far.
    pub paid: u32,
    pub last: Option<EventResult>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentFacts {
    /// Times it was sent to the frame (more than once for `repeat: each_time`).
    pub delivered: u32,
    /// The player opened its latest delivery.
    pub seen: bool,
    /// The last reply chosen in its conversation.
    pub reply: Option<String>,
    /// Every reply label chosen in its current conversation, in order: what
    /// `content.<id>.reply == <label>` tests, and what a Message node's reply
    /// exits read. Reset when the item is delivered again.
    #[serde(default)]
    pub chosen: Vec<String>,
}

/// A conversation waiting at a choice point (SPEC-story-mode, Dialogue
/// text): where it stands in the script, and the labels offered there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub at: crate::script::At,
    pub offered: Vec<String>,
}

/// Reactive content whose conditions held, waiting for its delay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduled {
    pub content: ContentId,
    pub due: StoryDate,
}

/// How a content item reached the frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Via {
    /// A Message node.
    Flow,
    /// By itself, its conditions being true.
    Reactive,
    /// Drawn from a failure pool after a failed event.
    Pool { event: EventId },
    /// A `send` effect.
    Effect,
}

/// One line of the history (SPEC-story-mode, Story Mode: player side: "a save
/// holds the state and the full history of what was seen").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub date: StoryDate,
    pub clock: Clock,
    #[serde(flatten)]
    pub what: Happening,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Happening {
    Delivered {
        content: ContentId,
        via: Via,
    },
    /// Lines of a conversation, as the player saw them: the frame is the
    /// record of the character's life, rebuilt from these on reload.
    Said {
        content: ContentId,
        lines: Vec<crate::engine::Said>,
    },
    Replied {
        content: ContentId,
        reply: String,
        /// The reply as the player read it. Empty in a save written before
        /// it was kept.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        text: String,
    },
    Launched {
        event: EventId,
        fee: Option<i64>,
    },
    Result {
        event: EventId,
        attempt: u32,
        result: EventResult,
        success: bool,
    },
    TimeMoved {
        from: StoryDate,
    },
    ActionDone {
        action: String,
    },
    Ended {
        ending: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub node: NodeId,
    pub wait: Wait,
    pub date: StoryDate,
    pub clock: Clock,
    /// The active frame, `none` included. Missing only from a save written
    /// before frames existed: `Save::resume` gives it the story's starting
    /// frame, never a frame of the platform's choosing.
    #[serde(default)]
    pub frame: Option<Frame>,
    pub reputation: i64,
    pub followers: i64,
    pub money: i64,
    /// Flags that are true; the others are false.
    pub flags: BTreeSet<String>,
    pub counters: BTreeMap<String, i64>,
    /// Relation per character, by the *name* of its step on the scale.
    pub relations: BTreeMap<String, String>,
    pub cars: BTreeSet<String>,
    pub events: BTreeMap<EventId, EventFacts>,
    pub content: BTreeMap<ContentId, ContentFacts>,
    pub scheduled: Vec<Scheduled>,
    /// `each_time` items whose conditions held at the last check: they fire
    /// again only after their conditions have been false in between.
    pub armed: BTreeSet<ContentId>,
    /// Failure-pool variants already used since the pool was last exhausted.
    pub pools: BTreeMap<PoolId, BTreeSet<ContentId>>,
    /// Side events currently open.
    pub side_events: BTreeSet<EventId>,
    /// The event a session was launched for, until its result comes back.
    pub launched: Option<EventId>,
    /// Conversations waiting for the player's choice, by content item.
    #[serde(default)]
    pub conversations: BTreeMap<ContentId, Conversation>,
    pub rng: Rng,
    pub history: Vec<Entry>,
}

impl State {
    /// The state at the start of a story. `seed` makes every draw of the run
    /// reproducible; it is stored in the state, so reloading a save replays
    /// the same draws instead of offering a new one.
    pub fn new(story: &Story, seed: u64) -> State {
        let start = &story.manifest.start;
        let scale = &story.settings().relations;
        State {
            node: start.node.clone(),
            wait: Wait::Running,
            date: start.date,
            clock: start.clock,
            frame: Some(start.frame.clone()),
            reputation: start.reputation,
            followers: start.followers,
            money: start.money,
            flags: BTreeSet::new(),
            counters: story.variables.counters.clone(),
            relations: story
                .characters
                .iter()
                .map(|c| (c.id.clone(), c.relation.clone().unwrap_or_else(|| scale.start.clone())))
                .collect(),
            cars: start.cars.iter().cloned().collect(),
            events: BTreeMap::new(),
            content: BTreeMap::new(),
            scheduled: Vec::new(),
            armed: BTreeSet::new(),
            pools: BTreeMap::new(),
            side_events: BTreeSet::new(),
            launched: None,
            conversations: BTreeMap::new(),
            rng: Rng::new(seed),
            history: Vec::new(),
        }
    }

    pub fn event(&self, id: &str) -> EventFacts {
        self.events.get(id).cloned().unwrap_or_default()
    }

    pub fn content_facts(&self, id: &str) -> ContentFacts {
        self.content.get(id).cloned().unwrap_or_default()
    }

    /// What a variable is worth now. `None` when it has no value yet - the
    /// position in an event never driven - which makes any test on it false,
    /// and so its `not` true: `not event.race.finished` holds before the race.
    pub fn value(&self, story: &Story, var: &Var) -> Option<Value> {
        Some(match var {
            Var::Date => Value::Date(self.date),
            Var::Year => Value::Int(i64::from(self.date.year)),
            Var::Month => Value::Int(i64::from(self.date.month)),
            Var::Chapter => Value::Rank(story.chapter_of(&self.node)?),
            Var::Reputation => Value::Int(self.reputation),
            Var::Followers => Value::Int(self.followers),
            Var::Money => Value::Int(self.money),
            Var::Flag(f) => Value::Bool(self.flags.contains(f)),
            Var::Counter(c) => Value::Int(*self.counters.get(c)?),
            Var::Rel(who) => Value::Rank(story.settings().relations.rank(self.relations.get(who)?)?),
            Var::Car(car) => Value::Bool(self.cars.contains(car)),
            Var::Event(ev, fact) => {
                let facts = self.events.get(ev);
                let last = facts.and_then(|f| f.last.as_ref());
                match fact {
                    EventFact::Attempts => Value::Int(i64::from(facts.map_or(0, |f| f.attempts))),
                    EventFact::Succeeded => Value::Bool(facts.is_some_and(|f| f.successes > 0)),
                    EventFact::Finished => Value::Bool(last?.finished),
                    EventFact::Dnf => Value::Bool(!last?.finished),
                    EventFact::Position => Value::Int(i64::from(last?.position?)),
                    EventFact::BestLap => Value::LapTime(last?.best_lap_ms?),
                }
            }
            Var::Content(c, fact) => {
                let facts = self.content.get(c);
                match fact {
                    ContentFact::Delivered => Value::Bool(facts.is_some_and(|f| f.delivered > 0)),
                    ContentFact::Seen => Value::Bool(facts.is_some_and(|f| f.seen)),
                    ContentFact::Reply => Value::Name(facts?.reply.clone()?),
                }
            }
        })
    }

    /// Whether a condition holds. Types were checked by the validator; a
    /// mismatch left here (an unvalidated story) is simply false.
    pub fn holds(&self, story: &Story, cond: &Condition) -> bool {
        // A reply is tested against every label chosen in the conversation,
        // not only the last one (SPEC-story-mode, In-person dialogues).
        if let (Var::Content(c, ContentFact::Reply), Some((op, Literal::Name(label)))) = (&cond.var, &cond.test) {
            let Some(facts) = self.content.get(c).filter(|f| f.reply.is_some()) else {
                return cond.negated;
            };
            let chosen = facts.chosen.iter().any(|l| l == label) || facts.reply.as_deref() == Some(label);
            let result = match op {
                crate::expr::CmpOp::Eq => chosen,
                crate::expr::CmpOp::Ne => !chosen,
                _ => false,
            };
            return result != cond.negated;
        }
        let result = match (self.value(story, &cond.var), &cond.test) {
            (None, _) => false,
            (Some(Value::Bool(b)), None) => b,
            (Some(_), None) => false,
            (Some(value), Some((op, literal))) => match (value, as_value(story, &cond.var, literal)) {
                (Value::Int(a), Some(Value::Int(b))) => op.holds(a, b),
                (Value::Date(a), Some(Value::Date(b))) => op.holds(a, b),
                (Value::Rank(a), Some(Value::Rank(b))) => op.holds(a, b),
                (Value::LapTime(a), Some(Value::LapTime(b))) => op.holds(a, b),
                (Value::Bool(a), Some(Value::Bool(b))) if !op.is_ordering() => op.holds(a, b),
                (Value::Name(a), Some(Value::Name(b))) if !op.is_ordering() => op.holds(a, b),
                _ => false,
            },
        };
        result != cond.negated
    }

    pub fn all_hold(&self, story: &Story, conds: &[Condition]) -> bool {
        conds.iter().all(|c| self.holds(story, c))
    }
}

/// A variable's value, in the form comparisons need: relations and chapters
/// compare by their rank on the scale / in the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Date(StoryDate),
    Rank(usize),
    LapTime(u32),
    Name(String),
}

/// A literal read as the type of the variable it is compared with: `warm`
/// next to `rel.friend` is a rank on the scale, `ch02` next to `chapter` a
/// rank in the flow. `None` when it cannot be (the validator's error).
pub fn as_value(story: &Story, var: &Var, literal: &Literal) -> Option<Value> {
    Some(match (var, literal) {
        (Var::Rel(_), Literal::Name(level)) => Value::Rank(story.settings().relations.rank(level)?),
        (Var::Chapter, Literal::Name(chapter)) => Value::Rank(story.chapter_rank(chapter)?),
        (Var::Content(_, ContentFact::Reply), Literal::Name(reply)) => Value::Name(reply.clone()),
        (_, Literal::Int(n)) => Value::Int(*n),
        (_, Literal::Bool(b)) => Value::Bool(*b),
        (_, Literal::Date(d)) => Value::Date(*d),
        (_, Literal::LapTime(ms)) => Value::LapTime(*ms),
        (_, Literal::Name(_)) => return None,
    })
}

/// splitmix64: tiny, fast, good enough to pick which written line a
/// character sends, and its whole state is one number that fits in a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, `n > 0`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}
