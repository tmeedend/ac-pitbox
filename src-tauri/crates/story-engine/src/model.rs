//! The types of a story folder (SPEC-story-mode, File format), as read from
//! its YAML files.
//!
//! Every struct refuses unknown keys: a misspelt `on_falure` must be an error
//! on its line, not a silently ignored field that changes how the story
//! plays. The price is that a setting introduced by a later lot needs its
//! field here first - which is the point of `Manifest::format` and
//! `requires`.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::expr::{Condition, Criterion, Effect};
use crate::time::{Clock, Span, StoryDate};

pub type NodeId = String;
pub type ContentId = String;
pub type EventId = String;
pub type PoolId = String;
pub type CharacterId = String;

/// The story-folder format this engine reads. A story declaring a higher one
/// was written for a newer Pit Box and is refused rather than misread.
pub const STORY_FORMAT: u32 = 1;

// ---------------------------------------------------------------- story.yaml

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub version: String,
    /// The first one is the source language; the others are translations.
    pub languages: Vec<String>,
    #[serde(default)]
    pub requires: Requires,
    pub start: Start,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    pub pitbox: Option<String>,
    pub csp: Option<String>,
}

/// Where and when the story begins.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub node: NodeId,
    #[serde(default = "relative_start")]
    pub date: StoryDate,
    #[serde(default)]
    pub clock: Clock,
    #[serde(default)]
    pub reputation: i64,
    #[serde(default)]
    pub followers: i64,
    #[serde(default)]
    pub money: i64,
    #[serde(default)]
    pub cars: Vec<String>,
}

fn relative_start() -> StoryDate {
    StoryDate::RELATIVE_START
}

/// The choices a story makes (SPEC-story-mode, Story settings): every one is
/// a setting of the story, never a rule of the engine, and each default suits
/// a story with no custom work. Only the settings the engine interprets live
/// here; the others (background, frame, calendar…) arrive with their lot.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// How the frame shows the date. The engine counts the same way for all.
    pub date: DateDisplay,
    /// What a failed event does when its node has no failure exit of its own.
    pub failure: FailureMode,
    /// With `failure: retry`, how many attempts before the result becomes
    /// final and the story carries on. `None`: retry forever.
    pub max_attempts: Option<u32>,
    pub retry_cost: RetryCost,
    pub adaptive_difficulty: Option<AdaptiveDifficulty>,
    pub saves: Saves,
    /// Whether the story has an economy. Without it, `money` does not exist.
    pub money: bool,
    pub protagonist: Protagonist,
    pub relations: RelationScale,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            date: DateDisplay::Relative,
            failure: FailureMode::Retry,
            max_attempts: None,
            retry_cost: RetryCost::Free,
            adaptive_difficulty: None,
            saves: Saves::default(),
            money: false,
            protagonist: Protagonist::Fixed,
            relations: RelationScale::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateDisplay {
    Relative,
    Real,
    Seasons,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    /// Story time is frozen and the event can be driven again.
    Retry,
    /// The result stands and the story goes on along the success exit,
    /// without the success effects.
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryCost {
    /// Only the first attempt pays the entry fee.
    Free,
    /// Every attempt pays it again.
    Paid,
}

/// Help after repeated failures: the opponents lose a step of skill every
/// `every` consecutive failures, up to `cap` steps. The engine computes the
/// step; the launcher applies it to the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveDifficulty {
    pub every: u32,
    pub cap: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Saves {
    pub autosave: Autosave,
    pub reload: bool,
}

impl Default for Saves {
    fn default() -> Self {
        Saves {
            autosave: Autosave::PerDay,
            reload: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Autosave {
    PerDay,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protagonist {
    Fixed,
    Chosen,
}

/// The closed scale relations move along, from worst to best. A story picks
/// its own; the default has five steps. Saves keep the step's *name*, so a
/// step inserted in a later version of the story leaves running games
/// meaning what they meant.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RelationScale {
    pub scale: Vec<String>,
    /// Where every character starts unless their sheet says otherwise.
    pub start: String,
}

impl Default for RelationScale {
    fn default() -> Self {
        RelationScale {
            scale: ["enemy", "cold", "neutral", "warm", "close"].map(String::from).to_vec(),
            start: "neutral".into(),
        }
    }
}

impl RelationScale {
    pub fn rank(&self, level: &str) -> Option<usize> {
        self.scale.iter().position(|l| l == level)
    }

    /// `level` moved by `steps`, clamped to the ends of the scale.
    pub fn shift(&self, level: &str, steps: i64) -> Option<&str> {
        let rank = self.rank(level)? as i64 + steps;
        let rank = rank.clamp(0, self.scale.len() as i64 - 1) as usize;
        Some(&self.scale[rank])
    }
}

// ------------------------------------------------------------------- world/

/// `world/variables.yaml`: the story's own variables.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Variables {
    /// Yes/no values, all false at the start.
    pub flags: Vec<String>,
    /// Whole numbers, with their starting value.
    pub counters: BTreeMap<String, i64>,
}

/// `world/characters.yaml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Characters {
    pub characters: Vec<Character>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: CharacterId,
    /// Display name in the source language; opaque to the engine.
    pub name: Option<String>,
    /// Starting relation, when it is not the scale's `start`.
    pub relation: Option<String>,
}

// -------------------------------------------------------------------- flow/

/// `flow/<nn>-<name>.yaml`: one chapter. Chapters are ordered by file name.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub id: String,
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    /// Path inside the story folder, for diagnostics.
    #[serde(skip)]
    pub file: String,
}

/// A flow node (SPEC-story-mode, Flow nodes). Without `next`, a node leads to
/// the one written after it, then to the first node of the next chapter.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawNode")]
pub struct Node {
    pub id: NodeId,
    pub next: Option<NodeId>,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    /// Changes the state; can move story time on.
    Beat(Beat),
    /// Sends a flow content item to the frame. With `wait_reply`, the flow
    /// waits for the player's reply before going on.
    Message { content: ContentId, wait_reply: bool },
    /// Launches a session. `next` is the success exit; `on_failure` the
    /// failure exit, and without one the story's `failure` setting decides.
    Event { event: EventId, on_failure: Option<NodeId> },
    /// Takes the first case whose conditions all hold. No UI.
    Branch { cases: Vec<Case>, otherwise: NodeId },
    /// Waits for the player to do something in the frame. What the action
    /// does to the state is the engine's business, not the frame's: the
    /// catalogues that will compute a purchase arrive in L6.
    Action { action: String },
    /// Opens or closes repeatable events that do not advance the story.
    SideEvents { open: Vec<EventId>, close: Vec<EventId> },
    /// Ends the story.
    End { ending: String },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Beat {
    pub effects: Vec<Effect>,
    pub advance: Option<Span>,
    /// In-story time of day after the beat.
    pub clock: Option<Clock>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    #[serde(rename = "if")]
    pub when: Vec<Condition>,
    pub goto: NodeId,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SideEvents {
    pub open: Vec<EventId>,
    pub close: Vec<EventId>,
}

/// What a node looks like in YAML: one key names its kind (`beat:`,
/// `message:`, `event:`…), a few others only make sense beside one kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    id: NodeId,
    next: Option<NodeId>,
    beat: Option<Beat>,
    message: Option<ContentId>,
    wait_reply: Option<bool>,
    event: Option<EventId>,
    on_failure: Option<NodeId>,
    branch: Option<Vec<Case>>,
    otherwise: Option<NodeId>,
    action: Option<String>,
    side_events: Option<SideEvents>,
    end: Option<String>,
}

impl TryFrom<RawNode> for Node {
    type Error = String;
    fn try_from(raw: RawNode) -> Result<Self, Self::Error> {
        let id = raw.id;
        let mut kinds = Vec::new();
        if let Some(beat) = raw.beat {
            kinds.push(NodeKind::Beat(beat));
        }
        if let Some(content) = raw.message {
            kinds.push(NodeKind::Message {
                content,
                wait_reply: raw.wait_reply.unwrap_or(false),
            });
        } else if raw.wait_reply.is_some() {
            return Err(format!("node `{id}`: `wait_reply` only goes with `message`"));
        }
        if let Some(event) = raw.event {
            kinds.push(NodeKind::Event {
                event,
                on_failure: raw.on_failure,
            });
        } else if raw.on_failure.is_some() {
            return Err(format!("node `{id}`: `on_failure` only goes with `event`"));
        }
        match (raw.branch, raw.otherwise) {
            (Some(cases), Some(otherwise)) => kinds.push(NodeKind::Branch { cases, otherwise }),
            (Some(_), None) => return Err(format!("node `{id}`: a `branch` needs an `otherwise`")),
            (None, Some(_)) => return Err(format!("node `{id}`: `otherwise` only goes with `branch`")),
            (None, None) => {}
        }
        if let Some(action) = raw.action {
            kinds.push(NodeKind::Action { action });
        }
        if let Some(side) = raw.side_events {
            kinds.push(NodeKind::SideEvents {
                open: side.open,
                close: side.close,
            });
        }
        if let Some(ending) = raw.end {
            kinds.push(NodeKind::End { ending });
        }
        if kinds.len() != 1 {
            return Err(format!(
                "node `{id}` must be exactly one of beat, message, event, branch, action, side_events, end ({} found)",
                kinds.len()
            ));
        }
        let kind = kinds.remove(0);
        if raw.next.is_some() && matches!(kind, NodeKind::Branch { .. } | NodeKind::End { .. }) {
            return Err(format!("node `{id}`: a branch or an end has no `next`"));
        }
        Ok(Node {
            id,
            next: raw.next,
            kind,
        })
    }
}

// ----------------------------------------------------------------- content/

/// Content types (SPEC-story-mode, Content types): written by type, never by
/// app; the frame decides how each one is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    Message,
    Call,
    Mail,
    Article,
    SocialPost,
    Video,
    Listing,
    Statement,
    CalendarEntry,
}

/// One content item of a `.story` file: its header, and its text.
#[derive(Debug, Clone)]
pub struct ContentItem {
    pub id: ContentId,
    pub kind: ContentType,
    pub from: Option<CharacterId>,
    /// In-story time of day shown on the item; the current clock otherwise.
    pub clock: Option<Clock>,
    pub placement: Placement,
    pub replies: Vec<Reply>,
    /// The text. **Opaque**: Ink or a format with line ids is still to be
    /// chosen (SPEC-story-mode, Dialogue text), and it belongs to the editor.
    pub body: String,
    pub file: String,
    /// 1-based line of the item's header in `file`.
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Placement {
    /// Sent by a Message node or a `send` effect.
    Flow,
    /// Appears by itself when its conditions hold.
    Reactive {
        when: Vec<Condition>,
        delay: Option<Span>,
        priority: i32,
        repeat: Repeat,
    },
    /// A variant of a failure pool, eligible when its conditions hold.
    Pool { pool: PoolId, when: Vec<Condition> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    /// Delivered at most once.
    #[default]
    Once,
    /// Delivered again each time its conditions become true again.
    EachTime,
}

/// A reply the player can choose, and what choosing it changes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub id: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// A content header as written. Its placement is told by which keys are
/// present: `pool` → a pool variant, `when` → reactive, neither → flow.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawContentHeader {
    id: ContentId,
    #[serde(rename = "type")]
    kind: ContentType,
    from: Option<CharacterId>,
    clock: Option<Clock>,
    pool: Option<PoolId>,
    when: Option<Vec<Condition>>,
    delay: Option<Span>,
    priority: Option<i32>,
    repeat: Option<Repeat>,
    #[serde(default)]
    replies: Vec<Reply>,
}

impl RawContentHeader {
    pub(crate) fn into_item(self, body: String, file: String, line: usize) -> Result<ContentItem, String> {
        let id = self.id;
        let reactive_only = self.delay.is_some() || self.priority.is_some() || self.repeat.is_some();
        let placement = match (self.pool, self.when) {
            (Some(pool), when) => {
                if reactive_only {
                    return Err(format!(
                        "content `{id}`: `delay`, `priority` and `repeat` are for reactive content, not a pool variant"
                    ));
                }
                Placement::Pool {
                    pool,
                    when: when.unwrap_or_default(),
                }
            }
            (None, Some(when)) => Placement::Reactive {
                when,
                delay: self.delay,
                priority: self.priority.unwrap_or(0),
                repeat: self.repeat.unwrap_or_default(),
            },
            (None, None) if reactive_only => {
                return Err(format!(
                    "content `{id}`: `delay`, `priority` and `repeat` need `when` (reactive content)"
                ))
            }
            (None, None) => Placement::Flow,
        };
        Ok(ContentItem {
            id,
            kind: self.kind,
            from: self.from,
            clock: self.clock,
            placement,
            replies: self.replies,
            body,
            file,
            line,
        })
    }
}

// ------------------------------------------------------------------ events/

/// `events/<id>.yaml` (SPEC-story-mode, Events, retries and failure).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: EventId,
    #[serde(rename = "type")]
    pub kind: EventType,
    /// Track, car, laps, grid… Opaque here: the launcher types it (L3),
    /// against the session format of `launch.rs`.
    #[serde(default)]
    pub session: Option<serde_json::Value>,
    /// All must hold. Empty: finishing is enough.
    #[serde(default)]
    pub success: Vec<Criterion>,
    #[serde(default)]
    pub on_success: OnSuccess,
    #[serde(default)]
    pub on_failure: OnFailure,
    /// Paid at launch when the story has an economy.
    pub entry_fee: Option<i64>,
    #[serde(skip)]
    pub file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Race,
    HotLap,
    TimeAttack,
    TestDrive,
    TrackDay,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnSuccess {
    pub effects: Vec<Effect>,
    pub advance: Option<Span>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnFailure {
    pub effects: Vec<Effect>,
    pub pool: Option<PoolId>,
    /// How many variants of the pool to send ("one or two messages").
    pub pick: u8,
}

impl Default for OnFailure {
    fn default() -> Self {
        OnFailure {
            effects: Vec::new(),
            pool: None,
            pick: 1,
        }
    }
}
