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

use crate::expr::Effect;
use crate::load::Story;
use crate::model::{ContentItem, ContentType, Node, NodeId, NodeKind};

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
    /// is active. A dialogue or an illustration is played on the scene,
    /// outside any frame, so it needs none. An unknown frame answers yes: it is reported once as
    /// unknown, not again for every item it would show.
    pub fn can_show(&self, kind: ContentType) -> bool {
        if kind.on_scene() {
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

/// The frames that can be active across the flow, worked out once for a
/// story: what the checks and the Content screen's preview ask of it.
pub struct FrameMap {
    /// The frames that can be active on arriving at each node.
    pub on_entry: BTreeMap<NodeId, BTreeSet<Frame>>,
    anywhere: BTreeSet<Frame>,
}

impl FrameMap {
    pub fn new(story: &Story) -> FrameMap {
        let on_entry = frames_on_entry(story);
        let mut anywhere: BTreeSet<Frame> = on_entry.values().flatten().cloned().collect();
        // The start's frame even when the start node is missing (an error of
        // its own): no warning on every item for want of a path.
        anywhere.insert(story.manifest.start.frame.clone());
        FrameMap { on_entry, anywhere }
    }

    /// The frames content `id` can arrive under: those active wherever a
    /// node sends it. Reactive content can arrive at any moment, and content
    /// nothing sends yet could be sent anywhere: every frame the story puts
    /// up, for both.
    pub fn content(&self, story: &Story, id: &str) -> BTreeSet<Frame> {
        let mut out = BTreeSet::new();
        for node in story.nodes() {
            let Some(frames) = self.on_entry.get(&node.id) else {
                continue;
            };
            for (sent, under) in sent_by(story, node, frames) {
                if sent == id {
                    out.extend(under);
                }
            }
        }
        if out.is_empty() {
            out = self.anywhere.clone();
        }
        out
    }
}

/// Why `item` has nowhere to be shown, if so: none of the frames it can
/// arrive under can show its type (SPEC-story-mode, Nowhere to show). What
/// the checks warn about, and what the preview shows instead of the item.
pub fn nowhere(item: &ContentItem, frames: &BTreeSet<Frame>) -> Option<String> {
    (!frames.iter().any(|f| f.can_show(item.kind))).then(|| {
        format!(
            "content `{}` is a {}, and no frame it can arrive in can show it",
            item.id,
            item.kind.name()
        )
    })
}

/// What a node sends to the frame, and which frames can be active then;
/// `on_entry` are those on arriving at it.
pub fn sent_by<'a>(story: &'a Story, node: &'a Node, on_entry: &BTreeSet<Frame>) -> Vec<(&'a str, BTreeSet<Frame>)> {
    let mut sent: Vec<(&str, BTreeSet<Frame>)> = Vec::new();
    match &node.kind {
        NodeKind::Beat(beat) => {
            // The beat's own frame is up before its effects run.
            let frames = beat
                .frame
                .clone()
                .map_or_else(|| on_entry.clone(), |f| BTreeSet::from([f]));
            sent.extend(sends(&beat.effects).map(|c| (c, frames.clone())));
        }
        NodeKind::Message { content, .. } => {
            sent.push((content, on_entry.clone()));
            // What its script sends, as the conversation plays here.
            if let Some(item) = story.content_item(content) {
                let effects = item.script.effects();
                sent.extend(effects.into_iter().filter_map(|(_, e)| match e {
                    Effect::Send(c) => Some((c.as_str(), on_entry.clone())),
                    _ => None,
                }));
            }
        }
        NodeKind::Event { event, .. } => {
            if let Some(event) = story.event(event) {
                for effects in [&event.on_success.effects, &event.on_failure.effects] {
                    sent.extend(sends(effects).map(|c| (c, on_entry.clone())));
                }
                if let Some(pool) = &event.on_failure.pool {
                    sent.extend(
                        story
                            .pool_variants(pool)
                            .map(|(item, _)| (item.id.as_str(), on_entry.clone())),
                    );
                }
            }
        }
        _ => {}
    }
    sent
}

/// The content `effects` send.
fn sends(effects: &[Effect]) -> impl Iterator<Item = &str> {
    effects.iter().filter_map(|e| match e {
        Effect::Send(c) => Some(c.as_str()),
        _ => None,
    })
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
        assert!(
            Frame::None.can_show(ContentType::Illustration),
            "an illustration is shown on the scene too"
        );
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

    // Where a content item can arrive: under the frames of the nodes that
    // send it; reactive content, anywhere the story puts a frame up.
    #[test]
    fn a_content_item_arrives_under_the_frames_of_whatever_sends_it() {
        let story = crate::Story::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).unwrap();
        let map = FrameMap::new(&story);
        let phone = Frame::Named("phone".into());
        assert_eq!(
            map.content(&story, "talk_01"),
            BTreeSet::from([Frame::None]),
            "after the beat removing it"
        );
        assert_eq!(map.content(&story, "invite_01"), BTreeSet::from([phone.clone()]));
        assert_eq!(
            map.content(&story, "rival_post_01"),
            BTreeSet::from([Frame::None, phone]),
            "reactive: every frame the story puts up"
        );
        let talk = story.content_item("talk_01").unwrap();
        assert_eq!(
            nowhere(talk, &BTreeSet::from([Frame::None])),
            None,
            "a dialogue needs no frame"
        );
        let invite = story.content_item("invite_01").unwrap();
        assert!(
            nowhere(invite, &BTreeSet::from([Frame::None])).is_some(),
            "a message needs one"
        );
    }

    // The preview lets the author set what a content item reads: its
    // placement's conditions and its text's, once each.
    #[test]
    fn a_content_item_reads_its_conditions_variables_once_each() {
        let story = crate::Story::from_sources(with(
            minimal(),
            "content/a.story",
            "---\nid: a\ntype: mail\nwhen: [\"reputation >= 2\", \"not flag.met\"]\n---\n{check(\"flag.met\"): Again.}\n* (x) {check(\"rel.friend >= warm\")} [X]\n",
        ))
        .unwrap();
        let reads: Vec<String> = story
            .content_item("a")
            .unwrap()
            .reads()
            .iter()
            .map(|v| v.to_string())
            .collect();
        assert_eq!(reads, ["reputation", "flag.met", "rel.friend"]);
    }
}
