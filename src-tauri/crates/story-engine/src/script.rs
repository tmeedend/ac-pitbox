//! The text of a content item (SPEC-story-mode, Dialogue text): a subset of
//! Ink, read here into a tree, written back from it, and played on the
//! story's state.
//!
//! **Our own reader rather than an Ink runtime.** The spec first counted on
//! blade-ink-rs. Tried on this very subset (October 2026, `bladeink` and
//! `bladeink-compiler` 2.1.0), it fell short twice: its compiler refuses a
//! string holding an operator on a `~` line (`~ effect("x += 1")`:
//! "unterminated string literal"), which is the form every effect takes; and a
//! choice's label is not reachable from the runtime (private fields, and
//! `get_visit_count_at_path_string("yes")` does not resolve), while the label
//! is the "reply chosen" fact. A reader of our own was needed anyway: to
//! refuse what is outside the subset, for the Form view, and to check speakers
//! and tags against the story. Playing the tree it builds is a few dozen
//! lines. What is written stays valid Ink, so Inky can still be used to type
//! it.
//!
//! The subset, line by line (indentation is free, as in Ink; nesting is told
//! by the number of `*` and `-`):
//!
//! ```text
//! friend: There is a race next weekend. #expression:happy #position:left
//! {check("rel.friend >= warm"): friend: You know I count on you.}
//! * (yes) [I'm in!]
//!     ~ effect("rel.friend += 1")
//!     friend: Great.
//!     * * (sure) {check("money >= 100")} [Count me in for the next one too.]
//! * (no) [Not this time.]
//! - friend: See you.
//! -> DONE
//! ```
//!
//! A line is `speaker: text` (a character's id) or plain text, which belongs
//! to the item's sender. A choice always has a label and its text in
//! brackets. A gather `-` joins the choices above it. Anything else - knots,
//! diverts but `-> DONE` / `-> END`, Ink's own variables, comments, sticky
//! `+` choices, glue - is an error: the subset is what a later translation
//! format must be able to hold.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::expr::{Condition, Effect};

/// A content item's text, read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    pub items: Vec<Item>,
}

/// One step of a weave, in playing order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    Line(Line),
    /// `~ effect("…")`.
    Effect {
        effect: Effect,
        #[serde(default)]
        line: usize,
    },
    /// A choice point: consecutive choices of one level.
    Choices {
        choices: Vec<Choice>,
    },
    /// `-`: the choices above join here.
    Gather,
    /// `-> DONE` or `-> END`: the conversation stops here.
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Shown only when all hold: `{check("…"): …}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<Condition>,
    /// The id written before `:`; `None`: the item's sender says it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub text: String,
    /// Dialogue only: the speaker's portrait for this line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// 1-based line in the `.story` file; 0 for a line built by the editor.
    #[serde(default)]
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    /// Stable: the "reply chosen" fact, and what a Message node's exits name.
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<Condition>,
    pub text: String,
    /// What follows when it is chosen.
    #[serde(default)]
    pub body: Vec<Item>,
    #[serde(default)]
    pub line: usize,
}

/// Where a speaker stands on the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Left,
    Center,
    Right,
}

impl Position {
    fn name(self) -> &'static str {
        match self {
            Position::Left => "left",
            Position::Center => "center",
            Position::Right => "right",
        }
    }
}

/// Something outside the subset, or that does not read, on a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// 1-based line in the `.story` file.
    pub line: usize,
    pub message: String,
}

impl Script {
    /// Every choice label, in the order written.
    pub fn labels(&self) -> Vec<&str> {
        fn collect<'a>(items: &'a [Item], out: &mut Vec<&'a str>) {
            for item in items {
                if let Item::Choices { choices } = item {
                    for c in choices {
                        out.push(&c.label);
                        collect(&c.body, out);
                    }
                }
            }
        }
        let mut out = Vec::new();
        collect(&self.items, &mut out);
        out
    }

    pub fn has_choices(&self) -> bool {
        !self.labels().is_empty()
    }

    /// Every line, wherever it is.
    pub fn lines(&self) -> Vec<&Line> {
        let mut out = Vec::new();
        visit(&self.items, &mut |item| {
            if let Item::Line(l) = item {
                out.push(l);
            }
        });
        out
    }

    /// Every effect, with its line: what the validator checks like any
    /// sentence, and what the frame check looks in for `send`.
    pub fn effects(&self) -> Vec<(usize, &Effect)> {
        let mut out = Vec::new();
        visit(&self.items, &mut |item| {
            if let Item::Effect { effect, line } = item {
                out.push((*line, effect));
            }
        });
        out
    }

    /// Every condition on a line or a choice, with its line.
    pub fn conditions(&self) -> Vec<(usize, &Condition)> {
        let mut out = Vec::new();
        visit(&self.items, &mut |item| match item {
            Item::Line(l) => out.extend(l.when.iter().map(|c| (l.line, c))),
            Item::Choices { choices } => {
                for c in choices {
                    out.extend(c.when.iter().map(|w| (c.line, w)));
                }
            }
            _ => {}
        });
        out
    }

    /// The labels written twice in the script, with their line: a label is
    /// unique in its content item, since it is the "reply chosen" fact.
    pub fn duplicate_labels(&self) -> Vec<(usize, &str)> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        visit(&self.items, &mut |item| {
            if let Item::Choices { choices } = item {
                for c in choices {
                    if !seen.insert(c.label.as_str()) {
                        out.push((c.line, c.label.as_str()));
                    }
                }
            }
        });
        out
    }
}

/// Calls `f` on every item, choice bodies included, in the order written.
fn visit<'a>(items: &'a [Item], f: &mut impl FnMut(&'a Item)) {
    for item in items {
        f(item);
        if let Item::Choices { choices } = item {
            for c in choices {
                visit(&c.body, f);
            }
        }
    }
}

// ------------------------------------------------------------------ reading

/// Reads `body`, whose first line is line `first_line` of its file. Whatever
/// reads is kept, so the editor can show it; every line that does not is in
/// the errors.
pub fn parse(body: &str, first_line: usize) -> (Script, Vec<ScriptError>) {
    let mut errors = Vec::new();
    // stack[0] is the script; stack[d] the body of the last choice of depth
    // d, still open.
    let mut stack: Vec<Vec<Item>> = vec![Vec::new()];
    for (i, raw) in body.lines().enumerate() {
        let line = first_line + i;
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let mut fail = |message: String| errors.push(ScriptError { line, message });
        if let Some(rest) = text.strip_prefix('*') {
            let (depth, rest) = markers(rest, '*');
            match choice(rest, line) {
                Err(why) => fail(why),
                Ok(_) if stack.len() < depth => fail(format!(
                    "a choice of depth {depth} needs a choice of depth {} above it",
                    depth - 1
                )),
                Ok(c) => {
                    close_to(&mut stack, depth);
                    let parent = stack.last_mut().expect("never empty");
                    match parent.last_mut() {
                        Some(Item::Choices { choices }) => choices.push(c),
                        _ => parent.push(Item::Choices { choices: vec![c] }),
                    }
                    stack.push(Vec::new());
                }
            }
        } else if text.starts_with('-') && !text.starts_with("->") {
            let (depth, rest) = markers(&text[1..], '-');
            if stack.len() < depth {
                fail(format!(
                    "this `-` joins choices of depth {depth}, and there are none above it"
                ));
                continue;
            }
            close_to(&mut stack, depth);
            let parent = stack.last_mut().expect("never empty");
            if !matches!(parent.last(), Some(Item::Choices { .. })) {
                fail("a `-` joins the choices above it, and there are none".into());
                continue;
            }
            parent.push(Item::Gather);
            if rest.starts_with('(') {
                fail("a gather has no label in the subset: labels go on choices".into());
            } else if !rest.is_empty() {
                match step(rest, line) {
                    Ok(item) => stack.last_mut().expect("never empty").push(item),
                    Err(why) => fail(why),
                }
            }
        } else {
            match step(text, line) {
                Ok(item) => stack.last_mut().expect("never empty").push(item),
                Err(why) => fail(why),
            }
        }
    }
    close_to(&mut stack, 1);
    let items = stack.pop().expect("the script itself");
    (Script { items }, errors)
}

/// The depth told by a run of `marker`s (the first already taken), and what
/// follows them. `- -> DONE` is a gather then a divert, not two gathers.
fn markers(mut rest: &str, marker: char) -> (usize, &str) {
    let mut depth = 1;
    loop {
        rest = rest.trim_start();
        if rest.starts_with(marker) && !rest.starts_with("->") {
            depth += 1;
            rest = &rest[1..];
        } else {
            return (depth, rest);
        }
    }
}

/// Closes the open choice bodies deeper than `depth - 1`, attaching each to
/// its choice.
fn close_to(stack: &mut Vec<Vec<Item>>, depth: usize) {
    while stack.len() > depth {
        let body = stack.pop().expect("deeper than depth");
        let parent = stack.last_mut().expect("never empty");
        if let Some(Item::Choices { choices }) = parent.last_mut() {
            choices.last_mut().expect("a choice opened this body").body = body;
        }
    }
}

/// A line that is not a choice or a gather: an effect, an end, a line.
fn step(text: &str, line: usize) -> Result<Item, String> {
    if let Some(rest) = text.strip_prefix('~') {
        let rest = rest.trim();
        let sentence = call(rest, "effect")
            .filter(|(_, after)| after.trim().is_empty())
            .map(|(s, _)| s)
            .ok_or_else(|| format!("`~ {rest}`: the subset changes the state only with `~ effect(\"…\")`"))?;
        let effect = sentence.parse::<Effect>()?;
        return Ok(Item::Effect { effect, line });
    }
    if let Some(rest) = text.strip_prefix("->") {
        return match rest.trim() {
            "DONE" | "END" => Ok(Item::End),
            other => Err(format!(
                "`-> {other}`: the only divert in the subset is `-> DONE` (or `-> END`)"
            )),
        };
    }
    if text.starts_with('+') {
        return Err("sticky choices (`+`) are not in the subset: use `*`".into());
    }
    if text.starts_with("//") || text.starts_with("/*") {
        return Err("comments are not in the subset".into());
    }
    if text.starts_with("===") || text.starts_with("==") {
        return Err("knots and stitches are not in the subset: one content item is one conversation".into());
    }
    if ["VAR ", "CONST ", "INCLUDE ", "EXTERNAL ", "LIST "]
        .iter()
        .any(|k| text.starts_with(k))
    {
        return Err("Ink's own declarations are not in the subset: the story's variables are read with `check(\"…\")` and changed with `effect(\"…\")`".into());
    }
    if let Some(rest) = text.strip_prefix('{') {
        // `{check("…") and check("…"): the line} #tags`
        let close = rest.rfind('}').ok_or("a `{` condition is never closed by `}`")?;
        let (inside, after) = (&rest[..close], rest[close + 1..].trim());
        let (when, content) = conditions(inside)?;
        let content = content
            .strip_prefix(':')
            .ok_or("a condition on a line is `{check(\"…\"): the line}`")?;
        let mut l = said(content.trim(), line)?;
        l.when = when;
        if !after.is_empty() {
            let (text, tags) = split_tags(after);
            if !text.is_empty() {
                return Err(format!("`{text}` after the condition's `}}`: only tags may follow it"));
            }
            apply_tags(&mut l, tags)?;
        }
        return Ok(Item::Line(l));
    }
    said(text, line).map(Item::Line)
}

/// `name("sentence")` at the start of `text`, and what follows it.
fn call<'a>(text: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let rest = text.strip_prefix(name)?.trim_start().strip_prefix('(')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let after = rest[end + 1..].trim_start().strip_prefix(')')?;
    Some((&rest[..end], after))
}

/// One or more `check("…")` joined by `and`, then what follows them.
fn conditions(text: &str) -> Result<(Vec<Condition>, &str), String> {
    let mut when = Vec::new();
    let mut rest = text.trim_start();
    loop {
        let (sentence, after) =
            call(rest, "check").ok_or_else(|| format!("`{{{text}`: a condition is `check(\"…\")`, joined by `and`"))?;
        when.push(sentence.parse::<Condition>()?);
        rest = after.trim_start();
        match rest.strip_prefix("and ") {
            Some(next) => rest = next.trim_start(),
            None => return Ok((when, rest)),
        }
    }
}

/// `* (label) {check("…")} [text]`, the markers already taken.
fn choice(text: &str, line: usize) -> Result<Choice, String> {
    let rest = text
        .strip_prefix('(')
        .ok_or("a choice needs a label: `* (label) [text]`")?;
    let close = rest.find(')').ok_or("a choice's label is never closed by `)`")?;
    let label = rest[..close].trim();
    if !crate::refs::is_name(label) {
        return Err(format!(
            "`{label}` is not a label: a letter, then letters, digits and `_`"
        ));
    }
    let mut rest = rest[close + 1..].trim_start();
    let mut when = Vec::new();
    if let Some(inner) = rest.strip_prefix('{') {
        let close = inner
            .find('}')
            .ok_or("a choice's `{` condition is never closed by `}`")?;
        let (conds, left) = conditions(&inner[..close])?;
        if !left.trim().is_empty() {
            return Err("a choice's condition is `{check(\"…\")}`, nothing else".into());
        }
        when = conds;
        rest = inner[close + 1..].trim_start();
    }
    let inner = rest
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or("a choice's text is in brackets, and nothing follows them: `* (label) [text]`")?;
    let text = inner.trim();
    if text.is_empty() {
        return Err("a choice with no text".into());
    }
    forbidden(text, "a choice's text")?;
    if text.contains(['[', ']', '#']) {
        return Err("a choice's text holds no `[`, `]` or `#`".into());
    }
    Ok(Choice {
        label: label.into(),
        when,
        text: text.into(),
        body: Vec::new(),
        line,
    })
}

/// A line of text: `speaker: text #tags`, or plain text.
fn said(text: &str, line: usize) -> Result<Line, String> {
    let (text, tags) = split_tags(text);
    let (speaker, text) = match speaker_prefix(text) {
        Some((who, rest)) => (Some(who.to_string()), rest),
        None => (None, text),
    };
    if text.is_empty() {
        return Err("a line with nothing to say".into());
    }
    forbidden(text, "a line")?;
    let mut l = Line {
        when: Vec::new(),
        speaker,
        text: text.into(),
        expression: None,
        position: None,
        line,
    };
    apply_tags(&mut l, tags)?;
    Ok(l)
}

/// `who: rest` when the line starts with a name and a colon.
pub(crate) fn speaker_prefix(text: &str) -> Option<(&str, &str)> {
    let colon = text.find(':')?;
    let who = &text[..colon];
    let rest = &text[colon + 1..];
    let separated = rest.is_empty() || rest.starts_with([' ', '\t']);
    (crate::refs::is_name(who) && separated).then(|| (who, rest.trim_start()))
}

/// Ink constructs that would read as something else than text.
fn forbidden(text: &str, what: &str) -> Result<(), String> {
    for (token, name) in [
        ("{", "`{` (a condition opens a line)"),
        ("}", "`}`"),
        ("->", "a divert `->`"),
        ("<>", "glue `<>`"),
        ("//", "a comment `//`"),
        ("\\", "an escape `\\`"),
    ] {
        if text.contains(token) {
            return Err(format!("{what} holds {name}, which is not text in Ink"));
        }
    }
    Ok(())
}

/// The text before the first `#`, and the tags after it.
fn split_tags(text: &str) -> (&str, Vec<&str>) {
    match text.find('#') {
        None => (text.trim(), Vec::new()),
        Some(i) => (
            text[..i].trim(),
            text[i..].split('#').map(str::trim).filter(|t| !t.is_empty()).collect(),
        ),
    }
}

fn apply_tags(l: &mut Line, tags: Vec<&str>) -> Result<(), String> {
    for tag in tags {
        if let Some(name) = tag.strip_prefix("expression:") {
            let name = name.trim();
            if l.expression.replace(name.to_string()).is_some() {
                return Err("two `#expression:` tags on one line".into());
            }
            if name.is_empty() {
                return Err("`#expression:` names an expression".into());
            }
        } else if let Some(value) = tag.strip_prefix("position:") {
            let position = match value.trim() {
                "left" => Position::Left,
                "center" => Position::Center,
                "right" => Position::Right,
                other => return Err(format!("`#position:{other}`: a position is left, center or right")),
            };
            if l.position.replace(position).is_some() {
                return Err("two `#position:` tags on one line".into());
            }
        } else {
            return Err(format!(
                "`#{tag}`: the subset's tags are `#expression:` and `#position:`"
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ writing

/// The script in canonical form: what the Form view writes. Choice bodies
/// are indented four spaces a level, which Ink ignores and a reader needs.
pub fn print(script: &Script) -> String {
    let mut out = String::new();
    print_items(&script.items, 0, &mut out);
    out
}

fn print_items(items: &[Item], depth: usize, out: &mut String) {
    let indent = "    ".repeat(depth);
    let gather = format!("{indent}{}", "- ".repeat(depth + 1).trim_end());
    for (i, item) in items.iter().enumerate() {
        // Whatever follows a choice point without a gather - another line,
        // another choice point - would read back inside its last choice, or
        // as more of its choices: the gather an edited tree may lack is
        // written here.
        let after_choices = i > 0 && matches!(items[i - 1], Item::Choices { .. });
        if after_choices && !matches!(item, Item::Gather) {
            let _ = writeln!(out, "{gather}");
        }
        match item {
            Item::Line(l) => {
                let _ = writeln!(out, "{indent}{}", print_line(l));
            }
            Item::Effect { effect, .. } => {
                let _ = writeln!(out, "{indent}~ effect(\"{effect}\")");
            }
            Item::End => {
                let _ = writeln!(out, "{indent}-> DONE");
            }
            Item::Gather => {
                let _ = writeln!(out, "{gather}");
            }
            Item::Choices { choices } => {
                for c in choices {
                    let mut head = format!("{indent}{}({})", "* ".repeat(depth + 1), c.label);
                    if !c.when.is_empty() {
                        let _ = write!(head, " {{{}}}", checks(&c.when));
                    }
                    let _ = writeln!(out, "{head} [{}]", c.text);
                    print_items(&c.body, depth + 1, out);
                }
            }
        }
    }
}

fn print_line(l: &Line) -> String {
    let mut text = match &l.speaker {
        Some(who) => format!("{who}: {}", l.text),
        None => l.text.clone(),
    };
    if !l.when.is_empty() {
        text = format!("{{{}: {text}}}", checks(&l.when));
    }
    if let Some(e) = &l.expression {
        let _ = write!(text, " #expression:{e}");
    }
    if let Some(p) = l.position {
        let _ = write!(text, " #position:{}", p.name());
    }
    text
}

fn checks(when: &[Condition]) -> String {
    let parts: Vec<String> = when.iter().map(|c| format!("check(\"{c}\")")).collect();
    parts.join(" and ")
}

// ------------------------------------------------------------------ playing

/// Where a conversation stands: the path to the choice point it waits at,
/// as item and choice indices alternating (`[3]`: item 3 of the script;
/// `[3, 1, 0]`: item 0 of the body of choice 1 of item 3).
pub type At = Vec<usize>;

/// What playing a script needs from the story's state.
pub trait Host {
    type Error;
    fn holds(&mut self, when: &[Condition]) -> bool;
    fn effect(&mut self, effect: &Effect) -> Result<(), Self::Error>;
    fn say(&mut self, line: &Line);
}

/// Why playing stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    Ended,
    /// A choice point, and the labels it offers (their conditions held).
    Choices {
        at: At,
        offered: Vec<String>,
    },
}

/// The items at the weave `path` leads to (`path` of even length: pairs of
/// item and choice indices).
fn weave<'a>(script: &'a Script, path: &[usize]) -> Option<&'a [Item]> {
    let mut items = script.items.as_slice();
    for pair in path.chunks(2) {
        let [item, choice] = pair else { return None };
        match items.get(*item)? {
            Item::Choices { choices } => items = &choices.get(*choice)?.body,
            _ => return None,
        }
    }
    Some(items)
}

/// The choices at `at`, if a choice point is there: a save resumed on an
/// updated story checks its conversation still points to one.
pub fn choices_at<'a>(script: &'a Script, at: &[usize]) -> Option<&'a [Choice]> {
    let (last, path) = at.split_last()?;
    match weave(script, path)?.get(*last)? {
        Item::Choices { choices } => Some(choices),
        _ => None,
    }
}

/// Plays from `at` (`vec![0]`: the start) until a choice point offering
/// something, or the end. A choice point whose choices all fail their
/// conditions is passed, as if it were not there. When a choice's body ends,
/// the conversation goes on after its choice point - to the gather that
/// joins it, or further out.
pub fn play<H: Host>(script: &Script, mut at: At, host: &mut H) -> Result<Stop, H::Error> {
    loop {
        let (&i, path) = at.split_last().expect("never empty");
        let Some(items) = weave(script, path) else {
            return Ok(Stop::Ended);
        };
        let Some(item) = items.get(i) else {
            if at.len() == 1 {
                return Ok(Stop::Ended);
            }
            // The end of a choice's body: on after its choice point.
            at.truncate(at.len() - 2);
            *at.last_mut().expect("never empty") += 1;
            continue;
        };
        match item {
            Item::Line(l) => {
                if host.holds(&l.when) {
                    host.say(l);
                }
            }
            Item::Effect { effect, .. } => host.effect(effect)?,
            Item::Gather => {}
            Item::End => return Ok(Stop::Ended),
            Item::Choices { choices } => {
                let offered: Vec<String> = (choices.iter())
                    .filter(|c| host.holds(&c.when))
                    .map(|c| c.label.clone())
                    .collect();
                if !offered.is_empty() {
                    return Ok(Stop::Choices { at, offered });
                }
            }
        }
        *at.last_mut().expect("never empty") += 1;
    }
}

/// Where playing resumes once `label` is chosen at the choice point `at`.
pub fn after_choice(script: &Script, at: &[usize], label: &str) -> Option<At> {
    let index = choices_at(script, at)?.iter().position(|c| c.label == label)?;
    let mut next = at.to_vec();
    next.extend([index, 0]);
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(body: &str) -> Script {
        let (script, errors) = parse(body, 1);
        assert!(errors.is_empty(), "{errors:?}");
        script
    }

    fn error(body: &str) -> String {
        let (_, errors) = parse(body, 10);
        assert_eq!(errors.len(), 1, "one error expected: {errors:?}");
        assert_eq!(errors[0].line, 10, "on the line written");
        errors[0].message.clone()
    }

    /// A host that records lines and effects and answers every condition
    /// with `holds`.
    struct Recorder {
        holds: bool,
        said: Vec<String>,
        effects: Vec<String>,
    }

    impl Host for Recorder {
        type Error = ();
        fn holds(&mut self, when: &[Condition]) -> bool {
            when.is_empty() || self.holds
        }
        fn effect(&mut self, effect: &Effect) -> Result<(), ()> {
            self.effects.push(effect.to_string());
            Ok(())
        }
        fn say(&mut self, line: &Line) {
            self.said.push(match &line.speaker {
                Some(who) => format!("{who}: {}", line.text),
                None => line.text.clone(),
            });
        }
    }

    fn recorder(holds: bool) -> Recorder {
        Recorder {
            holds,
            said: Vec::new(),
            effects: Vec::new(),
        }
    }

    const SAMPLE: &str = "friend: There is a race. #expression:happy #position:left
{check(\"rel.friend >= warm\"): friend: You know I count on you.}
* (yes) [I'm in!]
    ~ effect(\"rel.friend += 1\")
    friend: Great.
    * * (sure) {check(\"money >= 100\")} [Next one too.]
        friend: Deal.
    * * (maybe) [We'll see.]
* (no) [Not this time.]
    friend: Your loss.
- friend: See you.
-> DONE
";

    // The subset (SPEC-story-mode, Dialogue text): lines with a speaker and
    // tags, conditions, labelled choices nested by their stars, effects, a
    // gather and the end - read into the tree the Form view edits.
    #[test]
    fn the_subset_reads_into_a_tree() {
        let script = read(SAMPLE);
        assert_eq!(
            script.labels(),
            ["yes", "sure", "maybe", "no"],
            "labels in written order"
        );
        let Item::Line(first) = &script.items[0] else {
            panic!("a line first")
        };
        assert_eq!(first.speaker.as_deref(), Some("friend"));
        assert_eq!(first.text, "There is a race.");
        assert_eq!(first.expression.as_deref(), Some("happy"));
        assert_eq!(first.position, Some(Position::Left));
        let Item::Line(second) = &script.items[1] else {
            panic!("a conditional line")
        };
        assert_eq!(second.when.len(), 1, "its condition");
        let Item::Choices { choices } = &script.items[2] else {
            panic!("the choice point")
        };
        assert_eq!(choices.len(), 2, "yes and no form one choice point");
        let Item::Choices { choices: nested } = &choices[0].body[2] else {
            panic!("nested choices")
        };
        assert_eq!(nested.len(), 2, "sure and maybe, under yes");
        assert_eq!(nested[0].when.len(), 1, "a condition on a choice");
        assert!(matches!(script.items[3], Item::Gather), "the gather joins them");
        assert!(
            matches!(&script.items[4], Item::Line(l) if l.text == "See you."),
            "its line"
        );
        assert!(matches!(script.items[5], Item::End));
    }

    // What the Form view writes reads back as the same tree, and writing it
    // again changes nothing.
    #[test]
    fn the_canonical_form_reads_back_the_same() {
        let script = read(SAMPLE);
        let text = print(&script);
        let back = read(&text);
        assert_eq!(print(&back), text, "written again: the same text");
        assert_eq!(back.labels(), script.labels());
        assert!(
            text.contains("    * * (sure) {check(\"money >= 100\")} [Next one too.]"),
            "{text}"
        );
        assert!(
            text.contains("{check(\"rel.friend >= warm\"): friend: You know I count on you.}"),
            "{text}"
        );
    }

    // Anything outside the subset is an error on its line: the subset is
    // what a later translation format must be able to hold.
    #[test]
    fn what_is_not_in_the_subset_is_an_error_on_its_line() {
        for (body, needle) in [
            ("* [No label]", "needs a label"),
            ("* (two words) [x]", "is not a label"),
            ("* (a) Text without brackets", "in brackets"),
            ("+ (a) [sticky]", "sticky"),
            ("-> elsewhere", "only divert"),
            ("~ x = 1", "only with `~ effect"),
            ("VAR money = 0", "declarations"),
            ("=== knot ===", "knots"),
            ("// a comment", "comments"),
            ("friend: hello -> there", "divert"),
            ("friend: hi #mood:happy", "tags are"),
            ("friend: hi #position:top", "left, center or right"),
            ("- friend: joined, but nothing to join", "none"),
            ("~ effect(\"money +== 5\")", "cannot read effect"),
        ] {
            assert!(error(body).contains(needle), "{body}: {}", error(body));
        }
    }

    // A tree edited in the Form view can put a line right after a choice
    // point: written with the gather that keeps it there, not inside the
    // last choice.
    #[test]
    fn a_line_after_choices_is_written_with_its_gather() {
        let mut script = read("* (a) [A]\n* (b) [B]\n- After.\n");
        script.items.retain(|i| !matches!(i, Item::Gather));
        let text = print(&script);
        let back = read(&text);
        assert!(
            matches!(
                back.items.as_slice(),
                [Item::Choices { .. }, Item::Gather, Item::Line(_)]
            ),
            "{text}"
        );
        let Item::Choices { choices } = &back.items[0] else {
            unreachable!()
        };
        assert!(choices[1].body.is_empty(), "the line did not slip into `b`");
    }

    // `- -> DONE` is a gather then the end, not two gathers.
    #[test]
    fn a_gather_can_carry_the_end() {
        let script = read("* (a) [A]\n* (b) [B]\n- -> DONE\n");
        assert!(matches!(
            script.items.as_slice(),
            [Item::Choices { .. }, Item::Gather, Item::End]
        ));
    }

    // Playing: lines and effects in order, a stop at each choice point with
    // what it offers, and after a choice's body, on to the gather.
    #[test]
    fn playing_stops_at_each_choice_point_and_joins_at_the_gather() {
        let script = read(SAMPLE);
        let mut host = recorder(true);
        let Stop::Choices { at, offered } = play(&script, vec![0], &mut host).unwrap() else {
            panic!("waits for the first choice")
        };
        assert_eq!(offered, ["yes", "no"]);
        assert_eq!(host.said.len(), 2, "the conditional line held");
        let next = after_choice(&script, &at, "yes").unwrap();
        let Stop::Choices { at, offered } = play(&script, next, &mut host).unwrap() else {
            panic!("waits for the nested choice")
        };
        assert_eq!(offered, ["sure", "maybe"]);
        assert_eq!(host.effects, ["rel.friend += 1"], "the effect under the choice");
        let next = after_choice(&script, &at, "maybe").unwrap();
        assert_eq!(play(&script, next, &mut host).unwrap(), Stop::Ended);
        assert_eq!(
            host.said.last().unwrap(),
            "friend: See you.",
            "the gather's line, after the nested branch"
        );
    }

    // A condition that fails hides its line or its choice; a choice point
    // with nothing left to offer is passed.
    #[test]
    fn conditions_hide_lines_and_choices() {
        let script = read(SAMPLE);
        let mut host = recorder(false);
        let Stop::Choices { at, .. } = play(&script, vec![0], &mut host).unwrap() else {
            panic!("waits")
        };
        assert_eq!(
            host.said,
            ["friend: There is a race."],
            "the conditional line is skipped"
        );
        let next = after_choice(&script, &at, "yes").unwrap();
        let Stop::Choices { offered, .. } = play(&script, next, &mut host).unwrap() else {
            panic!("waits")
        };
        assert_eq!(offered, ["maybe"], "`sure` fails its condition");

        let only_conditional = read("* (a) {check(\"money >= 1\")} [A]\n- After.\n");
        let mut host = recorder(false);
        assert_eq!(play(&only_conditional, vec![0], &mut host).unwrap(), Stop::Ended);
        assert_eq!(host.said, ["After."], "nothing to offer: passed");
    }

    #[test]
    fn a_speaker_is_a_name_then_a_colon() {
        assert_eq!(speaker_prefix("friend: Hi"), Some(("friend", "Hi")));
        assert_eq!(speaker_prefix("Price:300"), None, "no blank after the colon");
        assert_eq!(speaker_prefix("Two words: x"), None);
        assert_eq!(speaker_prefix("Just text"), None);
    }
}
