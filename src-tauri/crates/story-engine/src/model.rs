//! The types of a story folder (SPEC-story-mode, File format), as read from
//! its YAML files.
//!
//! Serialized for the editor (`write.rs`, and the studio's JSON): the
//! story-wide types in full, with every default spelt out, so the editor
//! never has to know them; `write.rs` leaves the defaults out of the file.
//!
//! Every struct refuses unknown keys: a misspelt `on_falure` must be an error
//! on its line, not a silently ignored field that changes how the story
//! plays. The price is that a setting introduced by a later lot needs its
//! field here first - which is the point of `Manifest::format` and
//! `requires`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::expr::{Condition, Criterion, Effect};
use crate::frame::Frame;
use crate::script::{self, Script, ScriptError};
use crate::time::{Clock, Span, StoryDate};

pub type NodeId = String;
pub type ContentId = String;
pub type EventId = String;
pub type PoolId = String;
pub type CharacterId = String;
pub type DriverId = String;

/// The story-folder format this engine reads. A story declaring a higher one
/// was written for a newer Pit Box and is refused rather than misread.
pub const STORY_FORMAT: u32 = 1;

// ---------------------------------------------------------------- story.yaml

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitbox: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csp: Option<String>,
}

/// Where and when the story begins.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub node: NodeId,
    /// What shows the story at first, `none` for a visual novel. Required,
    /// with no default: the platform never picks a frame for the author.
    pub frame: Frame,
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
/// here; the others (background, calendar…) arrive with their lot. The frame
/// is not one: it is part of the state, set by `start` and by Beats.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateDisplay {
    Relative,
    Real,
    Seasons,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    /// Story time is frozen and the event can be driven again.
    Retry,
    /// The result stands and the story goes on along the success exit,
    /// without the success effects.
    Final,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveDifficulty {
    pub every: u32,
    pub cap: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Autosave {
    PerDay,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Protagonist {
    Fixed,
    Chosen,
}

/// The closed scale relations move along, from worst to best. A story picks
/// its own; the default has five steps. Saves keep the step's *name*, so a
/// step inserted in a later version of the story leaves running games
/// meaning what they meant.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Variables {
    /// Yes/no values, all false at the start.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    /// Whole numbers, with their starting value. Written in name order.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub counters: BTreeMap<String, i64>,
}

/// `world/characters.yaml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Characters {
    pub characters: Vec<Character>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: CharacterId,
    /// Display name in the source language; opaque to the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Starting relation, when it is not the scale's `start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    /// One picture per expression (`neutral`, `happy`…), as a path inside
    /// the story's `media/` folder; at least `neutral` when there are any.
    /// Shown by dialogues (SPEC-story-mode, In-person dialogues).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub portraits: BTreeMap<String, String>,
}

// ------------------------------------------------------------------ drivers/

/// `drivers/roster.yaml` (SPEC-story-mode, Drivers): the story's own drivers,
/// those a grid can name. Optional: without it, grids use filler names.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Roster {
    pub drivers: Vec<Driver>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Driver {
    pub id: DriverId,
    /// Display name, as the entry list shows it; opaque to the engine. A
    /// driver who is a character and has none takes the character's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The character this driver is on track: the friend one texts with and
    /// races against is one person, not two.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<CharacterId>,
    /// A country as the game names it in full (`Italy`), never a code: the
    /// entry list writes it as it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    /// Age at the start of the story.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<DriverRole>,
    /// 0 to 100: their skill wherever a grid names them without its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_level: Option<u32>,
    /// 0 to 100; none: the game's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggression: Option<u32>,
}

/// What a driver is to the story; the engine gives it no meaning yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverRole {
    Friend,
    Rival,
    Star,
    Filler,
}

// -------------------------------------------------------------------- flow/

/// `flow/<nn>-<name>.yaml`: one chapter. Chapters are ordered by file name.
/// Serialized the way it is read, for the editor that writes it back
/// (`write.rs`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    /// Path inside the story folder, for diagnostics.
    #[serde(skip)]
    pub file: String,
}

/// A flow node (SPEC-story-mode, Flow nodes). Without `next`, a node leads to
/// the one written after it, then to the first node of the next chapter.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(try_from = "RawNode", into = "RawNode")]
pub struct Node {
    pub id: NodeId,
    pub next: Option<NodeId>,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    /// Changes the state; can move story time on.
    Beat(Beat),
    /// Sends a flow content item to the frame, or plays a dialogue on the
    /// scene. The flow waits for the conversation to end when it has
    /// choices and `wait_reply` is set, the node has reply exits, or it is a
    /// dialogue. `on_reply`: where a reply label leads (SPEC-story-mode,
    /// In-person dialogues); a label it does not name takes `next`.
    Message {
        content: ContentId,
        wait_reply: bool,
        on_reply: BTreeMap<String, NodeId>,
    },
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Beat {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advance: Option<Span>,
    /// In-story time of day after the beat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock: Option<Clock>,
    /// The frame from this beat on, `none` to remove it; left out, the
    /// frame does not change. Applied before the beat's effects, so a
    /// `send` in the same beat goes to the new frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    #[serde(rename = "if")]
    pub when: Vec<Condition>,
    pub goto: NodeId,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SideEvents {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub open: Vec<EventId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub close: Vec<EventId>,
}

/// What a node looks like in YAML: one key names its kind (`beat:`,
/// `message:`, `event:`…), a few others only make sense beside one kind.
/// Fields in the order they are written: the id, the kind, then the way out.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    beat: Option<Beat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<ContentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wait_reply: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_reply: Option<BTreeMap<String, NodeId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<EventId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_failure: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<Vec<Case>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    otherwise: Option<NodeId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    side_events: Option<SideEvents>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<NodeId>,
}

impl From<Node> for RawNode {
    fn from(node: Node) -> Self {
        let mut raw = RawNode {
            id: node.id,
            next: node.next,
            beat: None,
            message: None,
            wait_reply: None,
            on_reply: None,
            event: None,
            on_failure: None,
            branch: None,
            otherwise: None,
            action: None,
            side_events: None,
            end: None,
        };
        match node.kind {
            NodeKind::Beat(beat) => raw.beat = Some(beat),
            NodeKind::Message {
                content,
                wait_reply,
                on_reply,
            } => {
                raw.message = Some(content);
                // `false` is the default: written only when it says something.
                raw.wait_reply = wait_reply.then_some(true);
                raw.on_reply = (!on_reply.is_empty()).then_some(on_reply);
            }
            NodeKind::Event { event, on_failure } => {
                raw.event = Some(event);
                raw.on_failure = on_failure;
            }
            NodeKind::Branch { cases, otherwise } => {
                raw.branch = Some(cases);
                raw.otherwise = Some(otherwise);
            }
            NodeKind::Action { action } => raw.action = Some(action),
            NodeKind::SideEvents { open, close } => raw.side_events = Some(SideEvents { open, close }),
            NodeKind::End { ending } => raw.end = Some(ending),
        }
        raw
    }
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
                on_reply: raw.on_reply.unwrap_or_default(),
            });
        } else if raw.wait_reply.is_some() || raw.on_reply.is_some() {
            return Err(format!(
                "node `{id}`: `wait_reply` and `on_reply` only go with `message`"
            ));
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
    /// Characters face to face, played on the scene outside any frame
    /// (SPEC-story-mode, In-person dialogues): it needs no frame.
    Dialogue,
    /// A full-screen picture with optional text, for the big moments: shown
    /// on the scene too, with or without a frame.
    Illustration,
}

impl ContentType {
    /// The name a `.story` header gives it.
    pub fn name(self) -> &'static str {
        match self {
            ContentType::Message => "message",
            ContentType::Call => "call",
            ContentType::Mail => "mail",
            ContentType::Article => "article",
            ContentType::SocialPost => "social_post",
            ContentType::Video => "video",
            ContentType::Listing => "listing",
            ContentType::Statement => "statement",
            ContentType::CalendarEntry => "calendar_entry",
            ContentType::Dialogue => "dialogue",
            ContentType::Illustration => "illustration",
        }
    }

    /// Played on the scene, outside any frame: it needs none.
    pub fn on_scene(self) -> bool {
        matches!(self, ContentType::Dialogue | ContentType::Illustration)
    }
}

/// How a dialogue or an illustration comes on the scene, and how a
/// dialogue's portraits come, go and change face (SPEC-story-mode,
/// In-person dialogues: transitions are options, never story settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    /// A short fade: what an item does when nothing is said.
    #[default]
    Fade,
    /// At once.
    Cut,
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
    /// Dialogue only: the characters present, shown as portraits.
    pub characters: Vec<CharacterId>,
    /// Dialogue only: where it is played; the current scene otherwise. A
    /// name kept as written until the story's scenes exist.
    pub scene: Option<String>,
    /// Illustration only: its picture, a path inside the story's `media/`.
    pub picture: Option<String>,
    /// Dialogue or illustration only: how it comes on the scene; a fade
    /// when not said.
    pub transition: Option<Transition>,
    /// The text as written, in Ink (SPEC-story-mode, Dialogue text).
    pub body: String,
    /// The text, read (`script.rs`); what did not read is in
    /// `script_errors`, for the validator to report.
    pub script: Script,
    pub script_errors: Vec<ScriptError>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    /// Delivered at most once.
    #[default]
    Once,
    /// Delivered again each time its conditions become true again.
    EachTime,
}

/// A content header as written, which is also how the editor edits it. Its
/// placement is told by which keys are present: `pool` → a pool variant,
/// `when` → reactive, neither → flow. The fields are in the order the
/// editor writes them: who and what, then the dialogue's own, then the
/// placement.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentHeader {
    pub id: ContentId,
    #[serde(rename = "type")]
    pub kind: ContentType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<CharacterId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<Clock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characters: Option<Vec<CharacterId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<Transition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<PoolId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Vec<Condition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
}

/// A content item as the editor edits it: its header, and its text as
/// written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentDoc {
    pub header: ContentHeader,
    pub body: String,
}

/// A `.story` file as the editor edits it: its items, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentFile {
    pub items: Vec<ContentDoc>,
}

impl ContentItem {
    /// The variables its conditions read - its placement's and its text's -
    /// once each, in the order written: what the Content screen's preview
    /// lets the author set.
    pub fn reads(&self) -> Vec<&crate::expr::Var> {
        let placed = match &self.placement {
            Placement::Flow => &[][..],
            Placement::Reactive { when, .. } | Placement::Pool { when, .. } => when.as_slice(),
        };
        let mut out: Vec<&crate::expr::Var> = Vec::new();
        for var in placed
            .iter()
            .map(|c| &c.var)
            .chain(self.script.conditions().into_iter().map(|(_, c)| &c.var))
        {
            if !out.contains(&var) {
                out.push(var);
            }
        }
        out
    }

    /// The item as the editor edits it. Defaults are left out, as an author
    /// would type them: a priority of 0, `repeat: once`, no characters.
    pub fn doc(&self) -> ContentDoc {
        let mut header = ContentHeader {
            id: self.id.clone(),
            kind: self.kind,
            from: self.from.clone(),
            clock: self.clock,
            characters: (!self.characters.is_empty()).then(|| self.characters.clone()),
            scene: self.scene.clone(),
            picture: self.picture.clone(),
            transition: self.transition,
            pool: None,
            when: None,
            delay: None,
            priority: None,
            repeat: None,
        };
        match &self.placement {
            Placement::Flow => {}
            Placement::Reactive {
                when,
                delay,
                priority,
                repeat,
            } => {
                // `when: []` is what makes an item reactive: always written.
                header.when = Some(when.clone());
                header.delay = *delay;
                header.priority = (*priority != 0).then_some(*priority);
                header.repeat = (*repeat != Repeat::Once).then_some(*repeat);
            }
            Placement::Pool { pool, when } => {
                header.pool = Some(pool.clone());
                header.when = (!when.is_empty()).then(|| when.clone());
            }
        }
        ContentDoc {
            header,
            body: self.body.clone(),
        }
    }
}

impl ContentHeader {
    /// `body_line`: the 1-based line of the text's first line in `file`.
    pub(crate) fn into_item(
        self,
        body: String,
        file: String,
        line: usize,
        body_line: usize,
    ) -> Result<ContentItem, String> {
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
        let (script, script_errors) = script::parse(&body, body_line);
        Ok(ContentItem {
            id,
            kind: self.kind,
            from: self.from,
            clock: self.clock,
            placement,
            characters: self.characters.unwrap_or_default(),
            scene: self.scene,
            picture: self.picture,
            transition: self.transition,
            body,
            script,
            script_errors,
            file,
            line,
        })
    }
}

// ------------------------------------------------------------------ events/

/// `events/<id>.yaml` (SPEC-story-mode, Events, retries and failure).
/// Serialized in full for the editor, every default spelt out; `write.rs`
/// leaves the defaults out of the file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: EventId,
    /// Display name in the source language ("Race 01"); opaque to the
    /// engine, like a character's.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: EventType,
    #[serde(default)]
    pub session: Session,
    /// All must hold. Empty: finishing is enough.
    #[serde(default)]
    pub success: Vec<Criterion>,
    #[serde(default)]
    pub on_success: OnSuccess,
    #[serde(default)]
    pub on_failure: OnFailure,
    /// Paid at launch when the story has an economy.
    #[serde(default)]
    pub entry_fee: Option<i64>,
    #[serde(skip)]
    pub file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Race,
    HotLap,
    TimeAttack,
    TestDrive,
    TrackDay,
}

/// What the author sets of the session an event launches. The engine plays
/// none of it: it is the launcher's (L3), which builds the app's own session
/// (`launch.rs`, `RaceSetup`) from it, field by field. Its own type rather
/// than the app's, so the pure engine never depends on the app, and a story
/// file names what an author thinks about, not how Content Manager wants it.
///
/// Everything left out is Pit Box's default for that session, and what the
/// author does not set at all (assists, damage, tyre wear) stays the
/// player's. The race-only fields are refused on another type by the
/// validator, not here: a type changed in the editor must not make the file
/// unreadable. A key left out is not serialized either, in the file as in
/// the editor's JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Session {
    /// The track's folder id (`content/tracks/<id>`), from the library.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// Its layout, for a track that has several.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// The car's folder id: the player's, and that of every opponent that
    /// names no car of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car: Option<String>,
    /// Race only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub laps: Option<u32>,
    /// Race only: cars on the grid, the player's included. With `opponents`
    /// listed, the slots beyond them drive the event's car under filler
    /// names; without `grid`, the listed opponents are the whole field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<u32>,
    /// Race only: the opponents' skill, 0 to 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_level: Option<u32>,
    /// Race only: where the player starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<GridStart>,
    /// Time of day in the game.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Clock>,
    /// Weather, as Pit Box names it (`clear`, `rain`…), not a weather
    /// folder: the folders differ from one installation to the next.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    /// Race only: the opponents the author places on the grid, in order
    /// (SPEC-story-mode, Drivers). Last, being the longest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub opponents: Vec<Opponent>,
}

/// One opponent an author places on a race's grid: a car, a livery, a driver
/// of the story's roster, their skill. Every key is optional; what is left
/// out is the event's (`car`, `ai_level`) or the driver's, and otherwise the
/// launcher's to pick (a livery, a name: the livery's own).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Opponent {
    /// A driver of `drivers/roster.yaml`; none: an unnamed driver, called
    /// what the livery declares.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<DriverId>,
    /// The car's folder id; none: the event's car.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car: Option<String>,
    /// The livery's folder id (`skins/<id>`); none: the launcher picks one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<String>,
    /// 0 to 100; none: the driver's, else the event's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_level: Option<u32>,
    /// Balance of performance, 0 to 100 like the session screen's: kilograms
    /// of ballast, percent of restrictor. 0 is none, and is left out.
    #[serde(skip_serializing_if = "is_zero")]
    pub ballast: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub restrictor: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// The player's place on the grid, the choices of Pit Box's session screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GridStart {
    Random,
    First,
    Second,
    Last,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OnSuccess {
    pub effects: Vec<Effect>,
    pub advance: Option<Span>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
