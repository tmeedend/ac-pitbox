//! Validation (SPEC-story-mode, Principles: "always validated"): everything
//! readable but wrong in a loaded story, reported all at once, each with the
//! file it is in. Errors are what makes the story misbehave; warnings what is
//! probably a mistake but plays (a node nobody reaches, a draft left aside).
//!
//! What it cannot check yet, for lack of the data: car and driver ids
//! (`car.<id>`, `ahead_of`) - the package and the roster arrive in later lots
//! - and content types a frame cannot show (frames, L2).

use std::collections::{HashSet, VecDeque};
use std::fmt;

use crate::expr::{Condition, ContentFact, Effect, EventFact, Literal, Var};
use crate::load::Story;
use crate::model::{NodeKind, Placement};

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
                    ContentFact::Reply if item.replies.is_empty() => {
                        return Err(format!("content `{c}` has no replies to test"))
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
                if !item.replies.iter().any(|r| r.id == *reply) {
                    return Err(format!("content `{c}` has no reply `{reply}`"));
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
                    }
                    NodeKind::Message { content, wait_reply } => match story.content_item(content) {
                        None => self.error(file, format!("{at}: unknown content `{content}`")),
                        Some(item) => {
                            if !matches!(item.placement, Placement::Flow) {
                                self.error(file, format!("{at}: `{content}` is reactive or a pool variant; a message node sends flow content"));
                            }
                            if *wait_reply && item.replies.is_empty() {
                                self.error(
                                    file,
                                    format!("{at}: waits for a reply but `{content}` has no `replies`"),
                                );
                            }
                        }
                    },
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
            let mut replies = HashSet::new();
            for reply in &item.replies {
                if !replies.insert(&reply.id) {
                    self.error(&file, format!("{at}: reply `{}` appears twice", reply.id));
                }
                for e in &reply.effects {
                    self.effect(&file, &format!("{at}, reply `{}`", reply.id), e);
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
            let mut targets: Vec<&str> = Vec::new();
            match &node.kind {
                NodeKind::Branch { cases, otherwise } => {
                    targets.extend(cases.iter().map(|c| c.goto.as_str()));
                    targets.push(otherwise);
                }
                NodeKind::End { .. } => {}
                kind => {
                    targets.extend(story.next_of(node).map(|n| n.id.as_str()));
                    if let NodeKind::Event {
                        on_failure: Some(f), ..
                    } = kind
                    {
                        targets.push(f);
                    }
                }
            }
            queue.extend(targets);
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
            effects.extend(item.replies.iter().flat_map(|r| &r.effects));
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

#[cfg(test)]
mod tests {
    use super::*;
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
            "has no `replies`",
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
}
