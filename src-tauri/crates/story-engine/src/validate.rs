//! Validation (SPEC-story-mode, Principles: "always validated"): everything
//! readable but wrong in a loaded story, reported all at once, each with the
//! file it is in. Errors are what makes the story misbehave; warnings what is
//! probably a mistake but plays (a node nobody reaches, a draft left aside).
//!
//! What it cannot check yet, for lack of the data: car and driver ids
//! (`car.<id>`, `ahead_of`) - the package and the roster arrive in later lots.
//! Media files are checked only when the folder was listed (`Story::media`).

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::fmt;

use crate::expr::{Condition, ContentFact, Effect, EventFact, Literal, Var};
use crate::frame::{frames_on_entry, Frame};
use crate::load::Story;
use crate::model::{Character, ContentItem, ContentType, Event, EventType, Node, NodeKind, Placement};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: String,
    /// The flow node it is about, when there is one: the editor selects it.
    pub node: Option<String>,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{level}: {}: {}", self.file, self.message)
    }
}

/// The type of a variable, for checking what it is compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
enum VarType {
    Int,
    Bool,
    Date,
    LapTime,
    Chapter,
    Relation,
    /// The reply of that content item: one of its reply ids.
    Reply(String),
}

struct Checker<'a> {
    story: &'a Story,
    out: Vec<Diagnostic>,
    /// The node being checked, recorded on every diagnostic raised meanwhile.
    node: Option<String>,
}

pub fn validate(story: &Story) -> Vec<Diagnostic> {
    let mut c = Checker {
        story,
        out: Vec::new(),
        node: None,
    };
    c.manifest_and_world();
    c.flow();
    c.content();
    c.events();
    c.reachability();
    c.nowhere_to_show();
    c.events_used_once();
    c.unused();
    c.out.sort_by(|a, b| (a.severity, &a.file).cmp(&(b.severity, &b.file)));
    c.out
}

impl Checker<'_> {
    fn error(&mut self, file: &str, message: String) {
        self.push(Severity::Error, file, message);
    }

    fn warning(&mut self, file: &str, message: String) {
        self.push(Severity::Warning, file, message);
    }

    fn push(&mut self, severity: Severity, file: &str, message: String) {
        self.out.push(Diagnostic {
            severity,
            file: file.into(),
            node: self.node.clone(),
            message,
        });
    }

    /// Runs `check` with diagnostics recorded against node `id`.
    fn on_node(&mut self, id: &str, check: impl FnOnce(&mut Self)) {
        self.node = Some(id.into());
        check(self);
        self.node = None;
    }

    fn manifest_and_world(&mut self) {
        let story = self.story;
        let m = &story.manifest;
        if m.languages.is_empty() {
            self.error("story.yaml", "`languages` names at least the source language".into());
        }
        if story.node(&m.start.node).is_none() {
            self.error("story.yaml", format!("start node `{}` does not exist", m.start.node));
        }
        if let Some(why) = unknown_frame(&m.start.frame) {
            self.error("story.yaml", format!("start: {why}"));
        }
        let scale = &m.settings.relations;
        let mut seen = HashSet::new();
        for level in &scale.scale {
            if !seen.insert(level) {
                self.error(
                    "story.yaml",
                    format!("relation step `{level}` appears twice in the scale"),
                );
            }
        }
        if scale.scale.is_empty() {
            self.error("story.yaml", "the relation scale has no step".into());
        } else if scale.rank(&scale.start).is_none() {
            self.error(
                "story.yaml",
                format!("relation start `{}` is not on the scale", scale.start),
            );
        }
        let mut ids = HashSet::new();
        for ch in &story.characters {
            if !ids.insert(&ch.id) {
                self.error(
                    "world/characters.yaml",
                    format!("character `{}` is declared twice", ch.id),
                );
            }
            if let Some(level) = ch.relation.as_deref().filter(|l| scale.rank(l).is_none()) {
                self.error(
                    "world/characters.yaml",
                    format!("character `{}`: relation `{level}` is not on the scale", ch.id),
                );
            }
            self.portraits(ch);
        }
        let mut flags = HashSet::new();
        for flag in &story.variables.flags {
            if !flags.insert(flag) {
                self.error("world/variables.yaml", format!("flag `{flag}` is declared twice"));
            }
            if story.variables.counters.contains_key(flag) {
                self.warning("world/variables.yaml", format!("`{flag}` is both a flag and a counter"));
            }
        }
    }

    /// A character's portraits (SPEC-story-mode, In-person dialogues): a
    /// `neutral` one for the lines that name no expression, and each a file
    /// inside the story's `media/`, so that a shared story carries it.
    fn portraits(&mut self, ch: &Character) {
        const FILE: &str = "world/characters.yaml";
        if !ch.portraits.is_empty() && !ch.portraits.contains_key("neutral") {
            self.error(
                FILE,
                format!(
                    "character `{}`: portraits need a `neutral` one, for the lines that name no expression",
                    ch.id
                ),
            );
        }
        for (expression, path) in &ch.portraits {
            let at = format!("character `{}`, portrait `{expression}`", ch.id);
            if expression.is_empty() || expression.chars().any(char::is_whitespace) {
                self.error(FILE, format!("{at}: an expression is a name, without spaces"));
            }
            if let Some(why) = media_path_problem(path) {
                self.error(FILE, format!("{at}: `{path}`: {why}"));
            } else if self.story.media.as_ref().is_some_and(|m| !m.contains(path)) {
                self.error(FILE, format!("{at}: `media/{path}` does not exist"));
            }
        }
    }

    fn var_type(&self, var: &Var) -> Result<VarType, String> {
        let story = self.story;
        Ok(match var {
            Var::Date => VarType::Date,
            Var::Year | Var::Month | Var::Reputation | Var::Followers => VarType::Int,
            Var::Chapter => VarType::Chapter,
            Var::Money if story.settings().money => VarType::Int,
            Var::Money => return Err("`money` is used but the story has no economy (`settings.money`)".into()),
            Var::Flag(f) if story.variables.flags.contains(f) => VarType::Bool,
            Var::Flag(f) => return Err(format!("unknown flag `{f}` (declare it in world/variables.yaml)")),
            Var::Counter(c) if story.variables.counters.contains_key(c) => VarType::Int,
            Var::Counter(c) => return Err(format!("unknown counter `{c}` (declare it in world/variables.yaml)")),
            Var::Rel(who) if story.character(who).is_some() => VarType::Relation,
            Var::Rel(who) => return Err(format!("unknown character `{who}`")),
            Var::Car(_) => VarType::Bool,
            Var::Event(e, _) if story.event(e).is_none() => return Err(format!("unknown event `{e}`")),
            Var::Event(_, fact) => match fact {
                EventFact::Attempts | EventFact::Position => VarType::Int,
                EventFact::Succeeded | EventFact::Finished | EventFact::Dnf => VarType::Bool,
                EventFact::BestLap => VarType::LapTime,
            },
            Var::Content(c, fact) => {
                let item = story.content_item(c).ok_or_else(|| format!("unknown content `{c}`"))?;
                match fact {
                    ContentFact::Delivered | ContentFact::Seen => VarType::Bool,
                    ContentFact::Reply if !item.script.has_choices() => {
                        return Err(format!("content `{c}` has no choices to test"))
                    }
                    ContentFact::Reply => VarType::Reply(c.clone()),
                }
            }
        })
    }

    fn condition(&mut self, file: &str, at: &str, cond: &Condition) {
        if let Err(why) = self.check_condition(cond) {
            self.error(file, format!("{at}: `{cond}`: {why}"));
        }
    }

    fn check_condition(&self, cond: &Condition) -> Result<(), String> {
        let ty = self.var_type(&cond.var)?;
        let Some((op, literal)) = &cond.test else {
            return match ty {
                VarType::Bool => Ok(()),
                _ => Err("not a yes/no value: compare it (`>=`, `==`…)".into()),
            };
        };
        let scale = &self.story.settings().relations;
        let ok = match (&ty, literal) {
            (VarType::Int, Literal::Int(_))
            | (VarType::Date, Literal::Date(_))
            | (VarType::LapTime, Literal::LapTime(_)) => true,
            (VarType::Bool, Literal::Bool(_)) => !op.is_ordering(),
            (VarType::Chapter, Literal::Name(ch)) => {
                if self.story.chapter_rank(ch).is_none() {
                    return Err(format!("unknown chapter `{ch}`"));
                }
                true
            }
            (VarType::Relation, Literal::Name(level)) => {
                if scale.rank(level).is_none() {
                    return Err(format!(
                        "`{level}` is not on the relation scale ({})",
                        scale.scale.join(", ")
                    ));
                }
                true
            }
            (VarType::Reply(c), Literal::Name(reply)) => {
                let item = self.story.content_item(c).expect("checked by var_type");
                if !item.script.labels().contains(&reply.as_str()) {
                    return Err(format!("content `{c}` has no choice labelled `{reply}`"));
                }
                !op.is_ordering()
            }
            _ => false,
        };
        if ok {
            Ok(())
        } else if op.is_ordering() && matches!(ty, VarType::Bool | VarType::Reply(_)) {
            Err(format!("`{op}` needs an order; use `==` or `!=`"))
        } else {
            Err(format!("`{}` cannot be compared with `{literal}`", cond.var))
        }
    }

    fn effect(&mut self, file: &str, at: &str, effect: &Effect) {
        if let Err(why) = self.check_effect(effect) {
            self.error(file, format!("{at}: `{effect}`: {why}"));
        }
    }

    fn check_effect(&self, effect: &Effect) -> Result<(), String> {
        let (var, value) = match effect {
            Effect::Send(c) => {
                return match self.story.content_item(c).map(|i| &i.placement) {
                    None => Err(format!("unknown content `{c}`")),
                    Some(Placement::Flow) => Ok(()),
                    Some(_) => {
                        Err("`send` delivers flow content; reactive and pool content arrive by themselves".into())
                    }
                };
            }
            Effect::Set(var, value) => (var, Some(value)),
            Effect::Add(var, _) => (var, None),
        };
        let ty = self.var_type(var)?;
        let writable = matches!(
            var,
            Var::Reputation | Var::Followers | Var::Money | Var::Counter(_) | Var::Rel(_) | Var::Flag(_) | Var::Car(_)
        );
        if !writable {
            return Err(match var {
                Var::Date | Var::Year | Var::Month => "story time moves with `advance`, it is not set".into(),
                _ => format!("`{var}` is read-only: the engine keeps it"),
            });
        }
        match (ty, value) {
            (VarType::Int | VarType::Relation, None) => Ok(()),
            (VarType::Bool, None) => Err("a yes/no value is set with `= true` or `= false`".into()),
            (VarType::Int, Some(Literal::Int(_))) | (VarType::Bool, Some(Literal::Bool(_))) => Ok(()),
            (VarType::Relation, Some(Literal::Name(level))) => {
                let scale = &self.story.settings().relations;
                match scale.rank(level) {
                    Some(_) => Ok(()),
                    None => Err(format!(
                        "`{level}` is not on the relation scale ({})",
                        scale.scale.join(", ")
                    )),
                }
            }
            (_, Some(literal)) => Err(format!("`{var}` cannot take `{literal}`")),
            _ => Err(format!("`{var}` cannot be changed that way")),
        }
    }

    fn flow(&mut self) {
        let story = self.story;
        for chapter in &story.chapters {
            let file = chapter.file.as_str();
            for node in &chapter.nodes {
                self.node = Some(node.id.clone());
                let at = format!("node `{}`", node.id);
                let target = |c: &mut Self, id: &str, what: &str| {
                    if story.node(id).is_none() {
                        c.error(file, format!("{at}: {what} `{id}` does not exist"));
                    }
                };
                if let Some(next) = &node.next {
                    target(self, next, "next node");
                }
                let uses_next = !matches!(node.kind, NodeKind::Branch { .. } | NodeKind::End { .. });
                if uses_next && node.next.is_none() && story.fallthrough(&node.id).is_none() {
                    self.error(
                        file,
                        format!("{at}: leads nowhere - the flow ends without an `end` node"),
                    );
                }
                match &node.kind {
                    NodeKind::Beat(beat) => {
                        for e in &beat.effects {
                            self.effect(file, &at, e);
                        }
                        if let Some(why) = beat.frame.as_ref().and_then(unknown_frame) {
                            self.error(file, format!("{at}: {why}"));
                        }
                    }
                    NodeKind::Message {
                        content,
                        wait_reply,
                        on_reply,
                    } => {
                        self.message_node(file, &at, content, *wait_reply, on_reply);
                        for to in on_reply.values() {
                            target(self, to, "reply exit");
                        }
                    }
                    NodeKind::Event { event, on_failure } => {
                        if story.event(event).is_none() {
                            self.error(file, format!("{at}: unknown event `{event}`"));
                        }
                        if let Some(f) = on_failure {
                            target(self, f, "failure exit");
                        }
                    }
                    NodeKind::Branch { cases, otherwise } => {
                        for case in cases {
                            target(self, &case.goto, "branch target");
                            for cond in &case.when {
                                self.condition(file, &at, cond);
                            }
                        }
                        target(self, otherwise, "branch target");
                    }
                    NodeKind::SideEvents { open, close } => {
                        for e in open.iter().chain(close) {
                            if story.event(e).is_none() {
                                self.error(file, format!("{at}: unknown event `{e}`"));
                            }
                        }
                    }
                    NodeKind::Action { .. } | NodeKind::End { .. } => {}
                }
            }
        }
        self.node = None;
    }

    /// What a Message node sends, and the replies it waits for or leaves by
    /// (SPEC-story-mode, In-person dialogues). Its exits' targets are checked
    /// with the node's other links.
    fn message_node(
        &mut self,
        file: &str,
        at: &str,
        content: &str,
        wait_reply: bool,
        on_reply: &std::collections::BTreeMap<String, String>,
    ) {
        let Some(item) = self.story.content_item(content) else {
            return self.error(file, format!("{at}: unknown content `{content}`"));
        };
        if !matches!(item.placement, Placement::Flow) {
            self.error(
                file,
                format!("{at}: `{content}` is reactive or a pool variant; a message node sends flow content"),
            );
        }
        let labels = item.script.labels();
        if wait_reply && labels.is_empty() {
            self.error(file, format!("{at}: waits for a reply but `{content}` has no choices"));
        }
        for label in on_reply.keys().filter(|l| !labels.contains(&l.as_str())) {
            self.error(
                file,
                format!("{at}: reply exit `{label}`: `{content}` has no choice labelled so"),
            );
        }
    }

    fn content(&mut self) {
        let story = self.story;
        for item in &story.content {
            let file = format!("{}:{}", item.file, item.line);
            let at = format!("content `{}`", item.id);
            if let Some(from) = item.from.as_deref().filter(|f| story.character(f).is_none()) {
                self.error(&file, format!("{at}: unknown character `{from}`"));
            }
            let when = match &item.placement {
                Placement::Flow => &[][..],
                Placement::Reactive { when, .. } | Placement::Pool { when, .. } => when.as_slice(),
            };
            for cond in when {
                self.condition(&file, &at, cond);
            }
            self.script(item);
        }
    }

    /// A content item's text (SPEC-story-mode, Dialogue text; In-person
    /// dialogues): what is outside the subset, labels told apart, sentences
    /// checked like any other, speakers and portraits checked against the
    /// story. Each diagnostic names its line in the file.
    fn script(&mut self, item: &ContentItem) {
        let story = self.story;
        let at = format!("content `{}`", item.id);
        let on = |line: usize| format!("{}:{line}", item.file);
        for e in &item.script_errors {
            self.error(&on(e.line), format!("{at}: {}", e.message));
        }
        for (line, label) in item.script.duplicate_labels() {
            self.error(
                &on(line),
                format!("{at}: label `{label}` is used twice - each reply is told apart by its label"),
            );
        }
        for (line, e) in item.script.effects() {
            self.effect(&on(line), &at, e);
        }
        for (line, c) in item.script.conditions() {
            self.condition(&on(line), &at, c);
        }
        let header = format!("{}:{}", item.file, item.line);
        let dialogue = item.kind == ContentType::Dialogue;
        if !dialogue && !item.characters.is_empty() {
            self.error(&header, format!("{at}: `characters` is for a dialogue"));
        }
        if !dialogue && item.scene.is_some() {
            self.error(&header, format!("{at}: `scene` is for a dialogue"));
        }
        for who in item.characters.iter().filter(|w| story.character(w).is_none()) {
            self.error(&header, format!("{at}: unknown character `{who}` among those present"));
        }
        for line in item.script.lines() {
            let place = on(line.line);
            if !dialogue && (line.expression.is_some() || line.position.is_some()) {
                self.error(
                    &place,
                    format!("{at}: `#expression:` and `#position:` are for a dialogue"),
                );
            }
            if let Some(who) = line.speaker.as_deref().filter(|w| story.character(w).is_none()) {
                self.warning(
                    &place,
                    format!("{at}: `{who}:` reads as a speaker, but there is no such character"),
                );
                continue;
            }
            let speaker = line.speaker.as_deref().or(item.from.as_deref());
            if let Some(who) = line.speaker.as_deref() {
                if dialogue && !item.characters.is_empty() && !item.characters.iter().any(|c| c == who) {
                    self.error(
                        &place,
                        format!("{at}: `{who}` speaks but is not among the `characters` present"),
                    );
                }
            }
            if let Some(expression) = line.expression.as_deref().filter(|_| dialogue) {
                match speaker.and_then(|w| story.character(w)) {
                    None => self.error(&place, format!("{at}: `#expression:{expression}` needs a speaker")),
                    Some(ch) if !ch.portraits.contains_key(expression) => {
                        self.error(&place, format!("{at}: `{}` has no portrait `{expression}`", ch.id))
                    }
                    Some(_) => {}
                }
            }
        }
    }

    fn events(&mut self) {
        let story = self.story;
        for event in &story.events {
            let (file, at) = (event.file.as_str(), format!("event `{}`", event.id));
            for e in event.on_success.effects.iter().chain(&event.on_failure.effects) {
                self.effect(file, &at, e);
            }
            if let Some(pool) = &event.on_failure.pool {
                if story.pool_variants(pool).next().is_none() {
                    self.error(file, format!("{at}: failure pool `{pool}` has no variant"));
                }
            }
            if event.on_failure.pick == 0 {
                self.error(file, format!("{at}: `pick: 0` sends nothing; remove the pool instead"));
            }
            if event.entry_fee.is_some() && !story.settings().money {
                self.error(file, format!("{at}: an entry fee needs an economy (`settings.money`)"));
            }
            self.session(file, &at, event);
        }
    }

    /// What the launcher needs to start the session, and what only a race
    /// has. Whether the car and the track are in the player's library is the
    /// app's to tell, not the engine's: it does not know the library.
    fn session(&mut self, file: &str, at: &str, event: &Event) {
        let s = &event.session;
        if s.track.is_none() {
            self.error(file, format!("{at}: no track"));
        }
        if s.car.is_none() {
            self.error(file, format!("{at}: no car"));
        }
        if s.layout.is_some() && s.track.is_none() {
            self.error(file, format!("{at}: a layout without a track"));
        }
        let race_only = [
            ("laps", s.laps.is_some()),
            ("grid", s.grid.is_some()),
            ("ai_level", s.ai_level.is_some()),
            ("start", s.start.is_some()),
        ];
        if event.kind != EventType::Race {
            for (key, set) in race_only {
                if set {
                    self.error(file, format!("{at}: `{key}` is for a race"));
                }
            }
        }
        if s.laps == Some(0) {
            self.error(file, format!("{at}: a race of 0 laps"));
        }
        if s.grid == Some(0) {
            self.error(file, format!("{at}: a grid of 0 cars"));
        }
        if s.ai_level.is_some_and(|l| l > 100) {
            self.error(file, format!("{at}: `ai_level` goes from 0 to 100"));
        }
    }

    /// Nodes no path from the start leads to: a draft, or a link forgotten.
    fn reachability(&mut self) {
        let story = self.story;
        let start = &story.manifest.start.node;
        let mut seen: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        if story.node(start).is_some() {
            queue.push_back(start);
        }
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            let Some(node) = story.node(id) else { continue };
            queue.extend(story.exits(node));
        }
        for chapter in &story.chapters {
            for node in chapter.nodes.iter().filter(|n| !seen.contains(n.id.as_str())) {
                self.on_node(&node.id, |c| {
                    c.warning(
                        &chapter.file,
                        format!("node `{}` is never reached from the start", node.id),
                    )
                });
            }
        }
    }

    /// Content sent where no active frame can show it (SPEC-story-mode,
    /// Nowhere to show). Where the flow tells which frames can be active -
    /// a message node, a `send` in a beat, what an event sends on success
    /// or failure - every path to that node counts; reactive content can
    /// arrive at any moment, so it is flagged only when no frame the story
    /// ever puts up can show it. Warnings: at play it waits for a frame.
    fn nowhere_to_show(&mut self) {
        let story = self.story;
        let on_entry = frames_on_entry(story);
        for chapter in &story.chapters {
            for node in &chapter.nodes {
                // A node no path reaches is already reported as such.
                let Some(frames) = on_entry.get(&node.id) else { continue };
                for (content, frames) in crate::frame::sent_by(story, node, frames) {
                    self.shown_under(&chapter.file, node, content, &frames);
                }
            }
        }
        // A beat's frame reaches the node after it; a beat with no node after
        // it is already an error.
        let mut anywhere: BTreeSet<&Frame> = on_entry.values().flatten().collect();
        // The start's frame even when the start node is missing (an error
        // of its own): no warning on every item for want of a path.
        anywhere.insert(&story.manifest.start.frame);
        for item in &story.content {
            let reactive = matches!(item.placement, Placement::Reactive { .. });
            if reactive && !anywhere.iter().any(|f| f.can_show(item.kind)) {
                self.warning(
                    &format!("{}:{}", item.file, item.line),
                    format!(
                        "content `{}` is a {}, and no frame the story puts up can show it",
                        item.id,
                        item.kind.name()
                    ),
                );
            }
        }
    }

    fn shown_under(&mut self, file: &str, node: &Node, content: &str, frames: &BTreeSet<Frame>) {
        // An unknown content is reported by `flow`, `events` or `effect`.
        let Some(item) = self.story.content_item(content) else {
            return;
        };
        let blind: Vec<String> = (frames.iter())
            .filter(|f| !f.can_show(item.kind))
            .map(|f| format!("`{f}`"))
            .collect();
        if blind.is_empty() {
            return;
        }
        let (id, kind, blind_list) = (&node.id, item.kind.name(), blind.join(", "));
        let message = if blind.len() == frames.len() {
            format!("node `{id}`: `{content}` is a {kind}, and frame {blind_list} cannot show it here; it will wait for a frame that can")
        } else {
            format!("node `{id}`: `{content}` is a {kind}, which frame {blind_list} cannot show on some paths leading here; it will wait for a frame that can")
        };
        self.on_node(id, |c| c.warning(file, message));
    }

    /// Flow content no message node or `send` delivers, and pools no event
    /// draws from.
    /// An event's facts (attempts, fees paid, failures in a row) belong to
    /// its id: two flow nodes on one event share them, so the second starts
    /// with the first one's attempts and, with free retries, pays nothing.
    fn events_used_once(&mut self) {
        let story = self.story;
        let mut seen: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        for chapter in &story.chapters {
            for node in &chapter.nodes {
                if let NodeKind::Event { event, .. } = &node.kind {
                    if let Some(first) = seen.insert(event, &node.id) {
                        self.on_node(&node.id, |c| {
                            c.warning(
                                &chapter.file,
                                format!("node `{}`: event `{event}` is already driven by node `{first}` - they share attempts and fees; give the rematch its own event", node.id),
                            )
                        });
                    }
                }
            }
        }
    }

    fn unused(&mut self) {
        let story = self.story;
        let mut sent: HashSet<&str> = HashSet::new();
        let mut effects: Vec<&Effect> = Vec::new();
        for node in story.nodes() {
            match &node.kind {
                NodeKind::Message { content, .. } => {
                    sent.insert(content);
                }
                NodeKind::Beat(beat) => effects.extend(&beat.effects),
                _ => {}
            }
        }
        for event in &story.events {
            effects.extend(event.on_success.effects.iter().chain(&event.on_failure.effects));
        }
        for item in &story.content {
            effects.extend(item.script.effects().into_iter().map(|(_, e)| e));
        }
        sent.extend(effects.iter().filter_map(|e| match e {
            Effect::Send(c) => Some(c.as_str()),
            _ => None,
        }));
        let pools: HashSet<&str> = story
            .events
            .iter()
            .filter_map(|e| e.on_failure.pool.as_deref())
            .collect();
        for item in &story.content {
            let file = format!("{}:{}", item.file, item.line);
            match &item.placement {
                Placement::Flow if !sent.contains(item.id.as_str()) => self.warning(
                    &file,
                    format!("content `{}` is never sent (no message node, no `send`)", item.id),
                ),
                Placement::Pool { pool, .. } if !pools.contains(pool.as_str()) => self.warning(
                    &file,
                    format!("content `{}`: no event draws from pool `{pool}`", item.id),
                ),
                _ => {}
            }
        }
    }
}

/// Why a story cannot use `frame`, if it cannot.
fn unknown_frame(frame: &Frame) -> Option<String> {
    (!frame.is_known()).then(|| {
        let shipped: Vec<String> = (crate::frame::BUILT_IN.iter())
            .map(|b| format!("`{}`", b.name))
            .collect();
        format!(
            "unknown frame `{frame}` (the frames are {}, or `none`)",
            shipped.join(", ")
        )
    })
}

/// Why `path` is not a file inside `media/`, if it is not: a shared story
/// carries its own folder only, so a portrait must live in it.
fn media_path_problem(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        Some("no file")
    } else if path.contains('\\') {
        Some("folders are separated by `/`")
    } else if path.starts_with('/') || path.contains(':') {
        Some("a path inside `media/`, not an absolute one")
    } else if path.split('/').any(|part| matches!(part, "" | "." | "..")) {
        Some("a path inside `media/` cannot leave it")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
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
            !has(&d, Severity::Warning, "no frame the story puts up"),
            "a missing start node is its own error, not a frame warning per item: {d:#?}"
        );
        let mut files = with(minimal(), "content/a.story", reactive);
        files[0].text = files[0].text.replace("frame: phone", "frame: none");
        let d = validate(&crate::Story::from_sources(files).unwrap());
        assert!(
            has(&d, Severity::Warning, "no frame the story puts up can show it"),
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
            "`#expression:` and `#position:` are for a dialogue",
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
}
