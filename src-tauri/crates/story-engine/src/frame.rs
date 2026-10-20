//! Frames (SPEC-story-mode, Content types, templates and frames): what shows
//! a story's content - a phone, a desk with letters, or nothing at all (a
//! visual novel).
//!
//! The engine is frame-agnostic: a frame is only a name and the content types
//! it can show. The frames Pit Box ships are a table here (`BUILT_IN`), not
//! cases in the engine, so a frame a story declares itself (`frames/`, a later
//! lot) slots in beside them. **None of them is a default**: a story names its
//! starting frame, and the phone is one frame among others.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;

use crate::load::Story;
use crate::model::{ContentType, NodeId, NodeKind};

/// The active frame, as a story writes it: `none`, or a frame's name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Frame {
    /// No frame: content is played on the scene only (dialogues).
    None,
    Named(String),
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Frame::None => f.write_str("none"),
            Frame::Named(name) => f.write_str(name),
        }
    }
}

impl FromStr for Frame {
    type Err = String;
    fn from_str(s: &str) -> Result<Frame, String> {
        match s.trim() {
            "" => Err("a frame is `none` or a frame's name".into()),
            "none" => Ok(Frame::None),
            name if name.chars().any(char::is_whitespace) => Err(format!("`{name}` is not a frame name")),
            name => Ok(Frame::Named(name.into())),
        }
    }
}

serde_as_text!(Frame);

/// A frame Pit Box ships, and the content types it can show.
pub struct BuiltIn {
    pub name: &'static str,
    pub shows: fn(ContentType) -> bool,
}

/// The frames Pit Box ships. The phone shows every type a frame can show:
/// it has an app for each.
pub const BUILT_IN: &[BuiltIn] = &[BuiltIn {
    name: "phone",
    shows: |_| true,
}];

fn built_in(name: &str) -> Option<&'static BuiltIn> {
    BUILT_IN.iter().find(|b| b.name == name)
}

impl Frame {
    /// Whether the story can use this frame: `none`, or a frame that exists.
    pub fn is_known(&self) -> bool {
        match self {
            Frame::None => true,
            Frame::Named(name) => built_in(name).is_some(),
        }
    }

    /// Whether content of `kind` has somewhere to be shown while this frame
    /// is active. A dialogue is played on the scene, outside any frame, so
    /// it needs none. An unknown frame answers yes: it is reported once as
    /// unknown, not again for every item it would show.
    pub fn can_show(&self, kind: ContentType) -> bool {
        if kind == ContentType::Dialogue {
            return true;
        }
        match self {
            Frame::None => false,
            Frame::Named(name) => built_in(name).is_none_or(|b| (b.shows)(kind)),
        }
    }
}

/// The frames that can be active on arriving at each node, along every path
/// from the start. Branch conditions are not evaluated: a path the state
/// would never take still counts. A node no path reaches has no entry.
pub fn frames_on_entry(story: &Story) -> BTreeMap<NodeId, BTreeSet<Frame>> {
    let mut on_entry: BTreeMap<NodeId, BTreeSet<Frame>> = BTreeMap::new();
    let start = &story.manifest.start;
    let mut queue: VecDeque<(&str, Frame)> = VecDeque::from([(start.node.as_str(), start.frame.clone())]);
    while let Some((id, frame)) = queue.pop_front() {
        let Some(node) = story.node(id) else { continue };
        if !on_entry.entry(id.to_string()).or_default().insert(frame.clone()) {
            continue;
        }
        let after = match &node.kind {
            NodeKind::Beat(beat) => beat.frame.clone().unwrap_or(frame),
            _ => frame,
        };
        queue.extend(story.exits(node).into_iter().map(|next| (next, after.clone())));
    }
    on_entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{minimal, with};

    #[test]
    fn a_frame_reads_and_prints_as_the_author_wrote_it() {
        for text in ["none", "phone", "desk"] {
            let frame: Frame = text.parse().expect(text);
            assert_eq!(frame.to_string(), text, "round trip");
        }
        assert_eq!(
            "none".parse::<Frame>(),
            Ok(Frame::None),
            "`none` is no frame, not a name"
        );
        assert!("".parse::<Frame>().is_err(), "empty is not a frame");
        assert!("my phone".parse::<Frame>().is_err(), "a name has no space");
    }

    // A dialogue needs no frame; everything else needs one able to show it.
    #[test]
    fn a_dialogue_needs_no_frame_and_the_rest_needs_one() {
        let phone = Frame::Named("phone".into());
        assert!(Frame::None.can_show(ContentType::Dialogue));
        assert!(!Frame::None.can_show(ContentType::Message));
        assert!(phone.can_show(ContentType::Message) && phone.can_show(ContentType::Dialogue));
        assert!(phone.is_known() && Frame::None.is_known());
        assert!(!Frame::Named("desk".into()).is_known(), "not shipped, not declared");
    }

    // The frame follows the flow: a Beat changes it for every node after
    // it, and a node reached by two paths can have either.
    #[test]
    fn the_frame_follows_every_path_of_the_flow() {
        let story = crate::Story::from_sources(with(
            minimal(),
            "flow/01-one.yaml",
            "id: ch01\nnodes:\n  - id: start\n    branch:\n      - if: [\"reputation >= 1\"]\n        goto: hide\n    otherwise: join\n  - id: hide\n    beat: { frame: none }\n  - id: join\n    beat: {}\n  - id: back\n    beat: { frame: phone }\n  - id: end\n    end: done\n",
        ))
        .unwrap();
        let on_entry = frames_on_entry(&story);
        let phone = Frame::Named("phone".into());
        assert_eq!(on_entry["hide"], BTreeSet::from([phone.clone()]), "before the beat");
        assert_eq!(
            on_entry["join"],
            BTreeSet::from([Frame::None, phone.clone()]),
            "one path removed the frame, the other did not"
        );
        assert_eq!(on_entry["end"], BTreeSet::from([phone]), "put back on every path");
    }
}
