//! Validation (SPEC-story-mode, Principles: "always validated"): everything
//! readable but wrong in a loaded story, reported all at once, each with the
//! file it is in. Errors are what makes the story misbehave; warnings what is
//! probably a mistake but plays (a node nobody reaches, a draft left aside).
//!
//! What it cannot check yet, for lack of the data: car ids (`car.<id>`) -
//! the package arrives in a later lot. Media files are checked only when the
//! folder was listed (`Story::media`).

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::fmt;

use crate::expr::{Condition, ContentFact, Criterion, Effect, EventFact, Literal, Var};
use crate::frame::{nowhere, Frame, FrameMap};
use crate::load::Story;
use crate::model::{Beat, Character, ContentItem, ContentType, Event, EventType, Node, NodeKind, Placement};

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
    c.drivers();
    c.scenes();
    c.frames();
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
        if let Some(why) = unknown_frame(story, &m.start.frame) {
            self.error("story.yaml", format!("start: {why}"));
        }
        if let Some(why) = m.start.scene.as_deref().and_then(|p| unknown_scene(story, p)) {
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

    /// The roster (SPEC-story-mode, Drivers): each driver once, the character
    /// they are on track one that exists and is no other driver, their
    /// numbers in range.
    fn drivers(&mut self) {
        const FILE: &str = crate::load::ROSTER;
        let story = self.story;
        let (mut ids, mut characters) = (HashSet::new(), HashSet::new());
        for d in &story.drivers {
            let at = format!("driver `{}`", d.id);
            if !ids.insert(&d.id) {
                self.error(FILE, format!("{at} is declared twice"));
            }
            if let Some(ch) = &d.character {
                if story.character(ch).is_none() {
                    self.error(FILE, format!("{at}: unknown character `{ch}`"));
                } else if !characters.insert(ch) {
                    self.error(FILE, format!("{at}: character `{ch}` is already another driver"));
                }
            }
            if d.name.is_none() && d.character.is_none() {
                self.warning(FILE, format!("{at} has no name: the grid would show the livery's"));
            }
            for (key, value) in [("ai_level", d.ai_level), ("aggression", d.aggression)] {
                if value.is_some_and(|v| v > 100) {
                    self.error(FILE, format!("{at}: `{key}` goes from 0 to 100"));
                }
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
            self.media_file(FILE, &at, None, path);
        }
    }

    /// The scenes (SPEC-story-mode, Scenes, frame and sound): each place
    /// with something to show, each variant once and reachable, its files
    /// inside `media/` and of a kind the scene plays, the weather and mood
    /// it asks for ones a Beat sets - unless a Beat forces it, which shows
    /// it whatever the state.
    fn scenes(&mut self) {
        use crate::scene::{AUDIO, IMAGE, SCENES as FILE, VIDEO};
        let story = self.story;
        let set_by_beats = |pick: fn(&Beat) -> &Option<String>| -> HashSet<&str> {
            (story.nodes())
                .filter_map(|n| match &n.kind {
                    NodeKind::Beat(beat) => pick(beat).as_deref().filter(|v| *v != "none"),
                    _ => None,
                })
                .collect()
        };
        let (weathers, moods) = (set_by_beats(|b| &b.weather), set_by_beats(|b| &b.mood));
        let forced = set_by_beats(|b| &b.variant);
        for place in &story.scenes {
            let at = format!("scene `{}`", place.id);
            if place.variants.is_empty() {
                self.error(FILE, format!("{at} has no variant: nothing to show"));
            }
            if let Some(path) = &place.ambient {
                self.sound(FILE, &at, path, &AUDIO);
            }
            let (mut ids, mut always) = (HashSet::new(), None);
            for v in &place.variants {
                let at = format!("{at}, variant `{}`", v.id);
                if !ids.insert(&v.id) {
                    self.error(FILE, format!("{at} is declared twice"));
                }
                let is_forced = forced.contains(v.id.as_str());
                match always {
                    Some(first) if !is_forced => {
                        self.warning(FILE, format!("{at} never shows: `{first}`, before it, always does"))
                    }
                    None if v.always() => always = Some(&v.id),
                    _ => {}
                }
                self.media_file(FILE, &at, None, &v.file);
                let kind = crate::scene::extension(&v.file).unwrap_or_default();
                if !IMAGE.contains(&kind.as_str()) && !VIDEO.contains(&kind.as_str()) {
                    self.error(
                        FILE,
                        format!(
                            "{at}: `{}` is neither an image ({}) nor a video loop ({})",
                            v.file,
                            IMAGE.join(", "),
                            VIDEO.join(", ")
                        ),
                    );
                }
                if let Some(path) = &v.ambient {
                    self.sound(FILE, &at, path, &AUDIO);
                }
                for (what, wanted, set) in [("weather", &v.weather, &weathers), ("mood", &v.mood, &moods)] {
                    for w in wanted.iter().filter(|w| !is_forced && !set.contains(w.as_str())) {
                        self.warning(
                            FILE,
                            format!("{at}: no Beat sets the {what} `{w}`, so it never shows on it"),
                        );
                    }
                }
            }
        }
    }

    /// An ambient sound: a file inside `media/`, of a kind the scene plays.
    fn sound(&mut self, file: &str, at: &str, path: &str, kinds: &[&str]) {
        self.media_file(file, at, Some("ambient"), path);
        if !kinds.contains(&crate::scene::extension(path).unwrap_or_default().as_str()) {
            self.error(
                file,
                format!("{at}: ambient `{path}` is not a sound ({})", kinds.join(", ")),
            );
        }
    }

    /// The frames a story declares (SPEC-story-mode, Skins: how a frame gets
    /// its look): each with apps, each app showing what its template can, a
    /// content type in one app of the frame only - two would leave the item
    /// nowhere to be told apart - its icon a file of `media/`, and one that
    /// appears from a Beat unlocked by one.
    fn frames(&mut self) {
        use crate::frame::{Appears, FRAMES as FILE};
        let story = self.story;
        let unlocked: HashSet<&str> = (story.nodes())
            .filter_map(|n| match &n.kind {
                NodeKind::Beat(beat) => Some(beat.unlock.iter().map(String::as_str)),
                _ => None,
            })
            .flatten()
            .collect();
        for frame in &story.frames {
            let at = format!("frame `{}`", frame.id);
            if frame.apps.is_empty() {
                self.error(FILE, format!("{at} has no app: nothing could show in it"));
            }
            let (mut ids, mut shown) = (HashSet::new(), std::collections::HashMap::new());
            for app in &frame.apps {
                let at = format!("{at}, app `{}`", app.id);
                if !ids.insert(app.id.as_str()) {
                    self.error(FILE, format!("{at} is declared twice"));
                }
                if app.shows.is_empty() {
                    self.error(FILE, format!("{at} shows no content type"));
                }
                for kind in &app.shows {
                    if !app.template.can_show(*kind) {
                        self.error(
                            FILE,
                            format!("{at}: a {} cannot show a {}", app.template.name(), kind.name()),
                        );
                    }
                    if let Some(first) = shown.insert(kind.name(), app.id.as_str()) {
                        self.error(
                            FILE,
                            format!(
                                "{at}: app `{first}` already shows {}: a type goes to one app",
                                kind.name()
                            ),
                        );
                    }
                }
                if let Some(icon) = &app.icon {
                    self.media_file(FILE, &at, Some("icon"), icon);
                }
                if app.appears == Appears::Beat && !unlocked.contains(app.id.as_str()) {
                    self.warning(
                        FILE,
                        format!("{at} appears from a Beat, and no Beat unlocks it: it shows with its first item only"),
                    );
                }
            }
        }
    }

    /// A Beat's apps to unlock: apps one of the story's frames has.
    fn beat_unlock(&mut self, file: &str, at: &str, beat: &Beat) {
        let story = self.story;
        for id in &beat.unlock {
            let known = (story.frames.iter()).any(|f| f.apps.iter().any(|a| &a.id == id))
                || (story.frame("phone").is_none() && crate::frame::phone_apps().iter().any(|a| &a.id == id));
            if !known {
                self.error(file, format!("{at}: unlock: no frame has an app `{id}`"));
            }
        }
    }

    /// A Beat's place and forced variant: ones the scenes declare.
    fn beat_scene(&mut self, file: &str, at: &str, beat: &Beat) {
        let story = self.story;
        if let Some(why) = beat.scene.as_deref().and_then(|p| unknown_scene(story, p)) {
            self.error(file, format!("{at}: {why}"));
        }
        let Some(variant) = &beat.variant else { return };
        let has = |p: &crate::scene::Place| p.variants.iter().any(|v| &v.id == variant);
        match beat.scene.as_deref().map(|p| story.scene(p)) {
            Some(Some(place)) if !has(place) => {
                self.error(file, format!("{at}: scene `{}` has no variant `{variant}`", place.id))
            }
            None if !story.scenes.iter().any(has) => {
                self.error(file, format!("{at}: no scene has a variant `{variant}`"))
            }
            _ => {}
        }
    }

    /// A file a story shows - a portrait, a picture, a scene - named by its
    /// path inside `media/`: inside that folder, so that a shared story
    /// carries it, and there. `label` says which of the item's files it is.
    fn media_file(&mut self, file: &str, at: &str, label: Option<&str>, path: &str) {
        if let Some(why) = media_path_problem(path) {
            let label = label.map(|l| format!("{l} ")).unwrap_or_default();
            self.error(file, format!("{at}: {label}`{path}`: {why}"));
        } else if self.story.media.as_ref().is_some_and(|m| !m.contains(path)) {
            self.error(file, format!("{at}: `media/{path}` does not exist"));
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
                        if let Some(why) = beat.frame.as_ref().and_then(|f| unknown_frame(self.story, f)) {
                            self.error(file, format!("{at}: {why}"));
                        }
                        self.beat_unlock(file, &at, beat);
                        self.beat_scene(file, &at, beat);
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
        if let Some(why) = item.scene.as_deref().and_then(|p| unknown_scene(story, p)) {
            self.error(&header, format!("{at}: {why}"));
        }
        self.picture(item, &header, &at);
        if item.transition.is_some() && !item.kind.on_scene() {
            self.error(
                &header,
                format!("{at}: `transition` is for a dialogue or an illustration"),
            );
        }
        for who in item.characters.iter().filter(|w| story.character(w).is_none()) {
            self.error(&header, format!("{at}: unknown character `{who}` among those present"));
        }
        for line in item.script.lines() {
            let place = on(line.line);
            if !dialogue && (line.expression.is_some() || line.position.is_some() || line.leave) {
                self.error(
                    &place,
                    format!("{at}: `#expression:`, `#position:` and `#leave` are for a dialogue"),
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

    /// An illustration's picture (SPEC-story-mode, In-person dialogues): a
    /// file inside `media/`, as a portrait is; and nothing to answer - an
    /// illustration is looked at, its text only says what it shows.
    fn picture(&mut self, item: &ContentItem, header: &str, at: &str) {
        let illustration = item.kind == ContentType::Illustration;
        match (&item.picture, illustration) {
            (Some(_), false) => self.error(header, format!("{at}: `picture` is for an illustration")),
            (None, true) => self.error(header, format!("{at}: an illustration needs a `picture` in `media/`")),
            (Some(path), true) => self.media_file(header, at, Some("picture"), path),
            (None, false) => {}
        }
        if illustration && item.script.has_choices() {
            self.error(header, format!("{at}: an illustration offers no reply"));
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
            self.opponents(file, &at, event);
            self.ahead_of(file, &at, event);
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
            ("opponents", !s.opponents.is_empty()),
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

    /// The opponents an author places on a race's grid: drivers of the roster,
    /// each once, numbers in range, and no more than the grid holds.
    fn opponents(&mut self, file: &str, at: &str, event: &Event) {
        let s = &event.session;
        let mut seen = HashSet::new();
        for (i, o) in s.opponents.iter().enumerate() {
            let at = format!("{at}, opponent {}", i + 1);
            if let Some(d) = &o.driver {
                if self.story.driver(d).is_none() {
                    self.error(file, format!("{at}: unknown driver `{d}`"));
                } else if !seen.insert(d) {
                    self.error(file, format!("{at}: driver `{d}` is already on this grid"));
                }
            }
            for (key, value) in [
                ("ai_level", o.ai_level.unwrap_or(0)),
                ("ballast", o.ballast),
                ("restrictor", o.restrictor),
            ] {
                if value > 100 {
                    self.error(file, format!("{at}: `{key}` goes from 0 to 100"));
                }
            }
        }
        let cars = s.opponents.len() + 1;
        if s.grid.is_some_and(|g| (g as usize) < cars) {
            self.error(
                file,
                format!(
                    "{at}: a grid of {} cars cannot hold {} opponents and the player",
                    s.grid.unwrap_or(0),
                    s.opponents.len()
                ),
            );
        }
    }

    /// `ahead_of <driver>` names a driver of the roster who is on the event's
    /// grid: otherwise the event could never be won.
    fn ahead_of(&mut self, file: &str, at: &str, event: &Event) {
        for c in &event.success {
            let Criterion::AheadOf(d) = c else { continue };
            if self.story.driver(d).is_none() {
                self.error(file, format!("{at}: `{c}`: unknown driver `{d}`"));
            } else if !event.session.opponents.iter().any(|o| o.driver.as_ref() == Some(d)) {
                self.error(file, format!("{at}: `{c}`: driver `{d}` is not on its grid"));
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
    /// A player action reached with no frame is flagged the same way: with
    /// no frame, a story acts through the replies of its dialogues.
    fn nowhere_to_show(&mut self) {
        let story = self.story;
        let map = FrameMap::new(story);
        for chapter in &story.chapters {
            for node in &chapter.nodes {
                // A node no path reaches is already reported as such.
                let Some(frames) = map.on_entry.get(&node.id) else {
                    continue;
                };
                for (content, frames) in crate::frame::sent_by(story, node, frames) {
                    self.shown_under(&chapter.file, node, content, &frames);
                }
                if let NodeKind::Action { action } = &node.kind {
                    self.action_under(&chapter.file, node, action, frames);
                }
            }
        }
        for item in story
            .content
            .iter()
            .filter(|i| matches!(i.placement, Placement::Reactive { .. }))
        {
            if let Some(why) = nowhere(story, item, &map.content(story, &item.id)) {
                self.warning(&format!("{}:{}", item.file, item.line), why);
            }
        }
    }

    fn shown_under(&mut self, file: &str, node: &Node, content: &str, frames: &BTreeSet<Frame>) {
        // An unknown content is reported by `flow`, `events` or `effect`.
        let Some(item) = self.story.content_item(content) else {
            return;
        };
        let blind: Vec<String> = (frames.iter())
            .filter(|f| !f.can_show(self.story, item.kind))
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

    fn action_under(&mut self, file: &str, node: &Node, action: &str, frames: &BTreeSet<Frame>) {
        if !frames.contains(&Frame::None) {
            return;
        }
        let id = &node.id;
        let message = if frames.len() == 1 {
            format!("node `{id}`: action `{action}` is reached with no frame, where there is nothing to do it in; with no frame, offer it as the replies of a dialogue")
        } else {
            format!("node `{id}`: action `{action}` is reached with no frame on some paths, where there is nothing to do it in; with no frame, offer it as the replies of a dialogue")
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
fn unknown_frame(story: &Story, frame: &Frame) -> Option<String> {
    (!frame.is_known(story)).then(|| {
        let declared: Vec<&str> = (story.frames.iter()).map(|f| f.id.as_str()).collect();
        let shipped = (crate::frame::BUILT_IN.iter())
            .map(|b| b.name)
            .filter(|b| !declared.contains(b));
        let names: Vec<&str> = declared.iter().copied().chain(shipped).collect();
        let shipped: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
        format!(
            "unknown frame `{frame}` (the frames are {}, or `none`)",
            shipped.join(", ")
        )
    })
}

/// Why a story cannot name place `id`, if it cannot.
fn unknown_scene(story: &Story, id: &str) -> Option<String> {
    story
        .scene(id)
        .is_none()
        .then(|| format!("unknown scene `{id}` (declare it in {})", crate::scene::SCENES))
}

/// Why `path` is not a file inside `media/`, if it is not: a shared story
/// carries its own folder only, so what it shows must live in it.
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
mod tests;
