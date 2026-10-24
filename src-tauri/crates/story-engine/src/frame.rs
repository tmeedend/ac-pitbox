//! Frames (SPEC-story-mode, Content types, templates and frames): what shows
//! a story's content - a phone, a desk with letters, or nothing at all (a
//! visual novel).
//!
//! The engine is frame-agnostic: a frame is only a name, its apps and the
//! content types they show. The frames Pit Box ships are a table here
//! (`BUILT_IN`), not cases in the engine, and a story declares its own in
//! `frames/frames.yaml` - several phones, from the most basic to the latest -
//! each built on a kind Pit Box ships (`phone` so far). A declared frame
//! named like a shipped one replaces it in that story. **None of them is a
//! default**: a story names its starting frame, and the phone is one frame
//! among others.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::expr::Effect;
use crate::load::Story;
use crate::model::{ContentItem, ContentType, Layout, Node, NodeId, NodeKind};

/// Where a story declares its own frames.
pub const FRAMES: &str = "frames/frames.yaml";

/// `frames/frames.yaml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Frames {
    pub frames: Vec<FrameDecl>,
}

/// A frame a story declares: a device of a kind Pit Box ships, with its
/// look and its apps. Every part of the look is optional: what is left out
/// takes Pit Box's theme.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrameDecl {
    /// What a story's start and its Beats name it by.
    pub id: String,
    pub kind: Kind,
    /// Display name in the source language; opaque to the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A picture of the device, its screen left transparent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// Behind the home screen, a path inside `media/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<String>,
    #[serde(default, skip_serializing_if = "StatusStyle::is_default")]
    pub status_bar: StatusStyle,
    #[serde(default, skip_serializing_if = "Sounds::is_empty")]
    pub sounds: Sounds,
    /// In the order the home screen shows them.
    pub apps: Vec<App>,
}

/// A device's picture and where its screen is in it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    /// A path inside `media/`: PNG or WebP, the screen transparent.
    pub file: String,
    pub screen: Screen,
}

/// The screen's rectangle in the body picture's pixels, and the radius of
/// its corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// How much the status bar shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusStyle {
    /// Clock, date, signal, Wi-Fi, battery.
    #[default]
    Full,
    /// The clock alone: an old phone.
    Simple,
    None,
}

impl StatusStyle {
    fn is_default(&self) -> bool {
        *self == StatusStyle::Full
    }
}

/// The phone's own sounds, paths inside `media/`: an app opened, a reply sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send: Option<String>,
}

impl Sounds {
    fn is_empty(&self) -> bool {
        self.tap.is_none() && self.send.is_none()
    }

    pub fn files(&self) -> impl Iterator<Item = (&'static str, &String)> {
        [("tap", &self.tap), ("send", &self.send)]
            .into_iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| (k, v)))
    }
}

/// What a Beat sets in the status bar; what it leaves out stays as it was.
/// A date label of `""` gives the story's date back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusBar {
    /// Shown instead of the story's date: "Mon 12 March".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Percent, 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery: Option<u8>,
    /// Bars, 0 to 4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub airplane: Option<bool>,
}

impl StatusBar {
    pub fn is_empty(&self) -> bool {
        *self == StatusBar::default()
    }

    /// `patch` over this one.
    pub fn apply(&mut self, patch: &StatusBar) {
        if let Some(d) = &patch.date {
            self.date = (!d.is_empty()).then(|| d.clone());
        }
        self.battery = patch.battery.or(self.battery);
        self.signal = patch.signal.or(self.signal);
        self.wifi = patch.wifi.or(self.wifi);
        self.airplane = patch.airplane.or(self.airplane);
    }
}

/// The kinds of frame Pit Box ships: what a declared frame builds on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A home screen of apps.
    Phone,
}

/// An app of a phone: a template showing some content types.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub id: String,
    /// Display name in the source language; left out, Pit Box names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its icon, a path inside `media/`; left out, its template's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub template: Template,
    /// The content types it shows: each its template can show, and each in
    /// one app of the frame only.
    pub shows: Vec<ContentType>,
    #[serde(default, skip_serializing_if = "Appears::is_default")]
    pub appears: Appears,
    /// Its look, a skin of `frames/skins/` for its template; left out, Pit
    /// Box's theme. A Beat can swap it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<String>,
}

/// The templates content is shown in (SPEC-story-mode, Content types,
/// templates and frames): those an app can be, then the ways content plays
/// on the scene, which a skin can dress too. The catalog comes with the
/// catalogs (L6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Template {
    Thread,
    Inbox,
    Feed,
    Statement,
    /// Calendar entries, by their date.
    Agenda,
    /// The call screen: a call played in the frame.
    Call,
    /// On the scene: characters face to face.
    Dialogue,
    /// On the scene: a document laid out as a newspaper's page.
    Newspaper,
    /// On the scene: a document laid out as a letter.
    Letter,
    /// On the scene: a video no app shows, as a TV report.
    Tv,
    /// On the scene: a call no app takes, on a landline.
    Landline,
}

impl Template {
    /// The content types the template can show.
    pub fn can_show(self, kind: ContentType) -> bool {
        use ContentType as C;
        match self {
            Template::Thread => kind == C::Message,
            Template::Inbox => kind == C::Mail,
            Template::Feed => matches!(kind, C::Article | C::SocialPost),
            Template::Statement => kind == C::Statement,
            Template::Agenda => kind == C::CalendarEntry,
            Template::Call => kind == C::Call,
            Template::Dialogue | Template::Newspaper | Template::Letter | Template::Tv | Template::Landline => false,
        }
    }

    /// An app can be it: it is shown in a frame, not on the scene.
    pub fn is_app(self) -> bool {
        matches!(
            self,
            Template::Thread
                | Template::Inbox
                | Template::Feed
                | Template::Statement
                | Template::Agenda
                | Template::Call
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Template::Thread => "thread",
            Template::Inbox => "inbox",
            Template::Feed => "feed",
            Template::Statement => "statement",
            Template::Agenda => "agenda",
            Template::Call => "call",
            Template::Dialogue => "dialogue",
            Template::Newspaper => "newspaper",
            Template::Letter => "letter",
            Template::Tv => "tv",
            Template::Landline => "landline",
        }
    }
}

/// When an app shows on the home screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appears {
    /// With its first item.
    #[default]
    FirstItem,
    Always,
    /// From the Beat that unlocks it - or with its first item, if that
    /// comes first: an item never lands in an app the player cannot open.
    Beat,
}

impl Appears {
    fn is_default(&self) -> bool {
        *self == Appears::FirstItem
    }
}

/// How content of `kind` (laid out as `layout`, for a document) plays on the
/// scene while `frame` is up: the template a skin for it dresses; `None`
/// when it shows in the frame instead.
pub fn on_scene_as(story: &Story, frame: &Frame, kind: ContentType, layout: Option<Layout>) -> Option<Template> {
    let in_app = || frame.apps(story).iter().any(|a| a.shows.contains(&kind));
    match kind {
        ContentType::Dialogue => Some(Template::Dialogue),
        ContentType::Document => Some(match layout.unwrap_or_default() {
            Layout::Newspaper => Template::Newspaper,
            Layout::Letter => Template::Letter,
        }),
        ContentType::Video if !in_app() => Some(Template::Tv),
        ContentType::Call if !in_app() => Some(Template::Landline),
        _ => None,
    }
}

/// The apps of the phone Pit Box ships, for a story that declares none.
pub fn phone_apps() -> Vec<App> {
    use ContentType as C;
    let app = |id: &str, template, shows: Vec<C>, appears| App {
        id: id.into(),
        name: None,
        icon: None,
        template,
        shows,
        appears,
        skin: None,
    };
    vec![
        app("messages", Template::Thread, vec![C::Message], Appears::Always),
        app("mail", Template::Inbox, vec![C::Mail], Appears::FirstItem),
        app("news", Template::Feed, vec![C::Article], Appears::FirstItem),
        app("social", Template::Feed, vec![C::SocialPost], Appears::FirstItem),
        app("bank", Template::Statement, vec![C::Statement], Appears::FirstItem),
        app("calls", Template::Call, vec![C::Call], Appears::FirstItem),
        app("calendar", Template::Agenda, vec![C::CalendarEntry], Appears::FirstItem),
    ]
}

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

/// A frame Pit Box ships, the content types it can show, and its apps.
pub struct BuiltIn {
    pub name: &'static str,
    pub shows: fn(ContentType) -> bool,
    pub apps: fn() -> Vec<App>,
}

/// The frames Pit Box ships. The phone shows every type a frame can show:
/// those its apps have no template for are listed by name in its messages.
pub const BUILT_IN: &[BuiltIn] = &[BuiltIn {
    name: "phone",
    shows: |_| true,
    apps: phone_apps,
}];

fn built_in(name: &str) -> Option<&'static BuiltIn> {
    BUILT_IN.iter().find(|b| b.name == name)
}

impl Frame {
    /// Whether the story can use this frame: `none`, one it declares, or one
    /// Pit Box ships.
    pub fn is_known(&self, story: &Story) -> bool {
        match self {
            Frame::None => true,
            Frame::Named(name) => story.frame(name).is_some() || built_in(name).is_some(),
        }
    }

    /// Whether content of `kind` has somewhere to be shown while this frame
    /// is active. A dialogue, an illustration or a document is played on
    /// the scene, outside any frame, so it needs none, and a video or a call
    /// no app takes plays there too; a declared frame shows what its apps
    /// show. An unknown frame answers yes: it is reported once as unknown,
    /// not again for every item it would show.
    pub fn can_show(&self, story: &Story, kind: ContentType) -> bool {
        if kind.on_scene() || kind.falls_to_scene() {
            return true;
        }
        match self {
            Frame::None => false,
            Frame::Named(name) => match story.frame(name) {
                Some(declared) => declared.apps.iter().any(|a| a.shows.contains(&kind)),
                None => built_in(name).is_none_or(|b| (b.shows)(kind)),
            },
        }
    }

    /// Its apps, in the order the home screen shows them; none for `none`
    /// or an unknown frame.
    pub fn apps(&self, story: &Story) -> Vec<App> {
        match self {
            Frame::None => Vec::new(),
            Frame::Named(name) => match story.frame(name) {
                Some(declared) => declared.apps.clone(),
                None => built_in(name).map_or_else(Vec::new, |b| (b.apps)()),
            },
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
pub fn nowhere(story: &Story, item: &ContentItem, frames: &BTreeSet<Frame>) -> Option<String> {
    (!frames.iter().any(|f| f.can_show(story, item.kind))).then(|| {
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
        let story = crate::Story::from_sources(minimal()).unwrap();
        let phone = Frame::Named("phone".into());
        assert!(Frame::None.can_show(&story, ContentType::Dialogue));
        assert!(
            Frame::None.can_show(&story, ContentType::Illustration),
            "an illustration is shown on the scene too"
        );
        assert!(!Frame::None.can_show(&story, ContentType::Message));
        assert!(phone.can_show(&story, ContentType::Message) && phone.can_show(&story, ContentType::Dialogue));
        assert!(phone.is_known(&story) && Frame::None.is_known(&story));
        assert!(
            !Frame::Named("desk".into()).is_known(&story),
            "not shipped, not declared"
        );
        let apps: Vec<String> = phone.apps(&story).into_iter().map(|a| a.id).collect();
        assert_eq!(
            apps,
            ["messages", "mail", "news", "social", "bank", "calls", "calendar"],
            "the shipped phone's apps"
        );
    }

    // A story's own phone shows what its apps show, nothing more; one named
    // like the shipped phone replaces it in that story.
    #[test]
    fn a_declared_frame_shows_what_its_apps_show() {
        let frames = "frames:\n  - id: basic\n    kind: phone\n    apps:\n      - { id: sms, template: thread, shows: [message], appears: always }\n  - id: phone\n    kind: phone\n    apps:\n      - { id: inbox, template: inbox, shows: [mail] }\n";
        let story = crate::Story::from_sources(with(minimal(), FRAMES, frames)).unwrap();
        let basic = Frame::Named("basic".into());
        assert!(basic.is_known(&story), "declared");
        assert!(basic.can_show(&story, ContentType::Message), "its app's type");
        assert!(!basic.can_show(&story, ContentType::Mail), "no app for it");
        assert!(
            basic.can_show(&story, ContentType::Dialogue),
            "on the scene, whatever the frame"
        );
        let phone = Frame::Named("phone".into());
        assert!(
            !phone.can_show(&story, ContentType::Message),
            "the story's own `phone` replaces Pit Box's"
        );
        assert_eq!(phone.apps(&story)[0].id, "inbox");
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
        let (basic, smart) = (Frame::Named("basic_phone".into()), Frame::Named("smartphone".into()));
        assert_eq!(
            map.content(&story, "talk_01"),
            BTreeSet::from([Frame::None]),
            "after the beat removing it"
        );
        assert_eq!(map.content(&story, "invite_01"), BTreeSet::from([basic.clone()]));
        assert_eq!(
            map.content(&story, "rival_post_01"),
            BTreeSet::from([Frame::None, basic.clone(), smart.clone()]),
            "reactive: every frame the story puts up"
        );
        let talk = story.content_item("talk_01").unwrap();
        assert_eq!(
            nowhere(&story, talk, &BTreeSet::from([Frame::None])),
            None,
            "a dialogue needs no frame"
        );
        let invite = story.content_item("invite_01").unwrap();
        assert!(
            nowhere(&story, invite, &BTreeSet::from([Frame::None])).is_some(),
            "a message needs one"
        );
        let post = story.content_item("rival_post_01").unwrap();
        assert!(
            nowhere(&story, post, &BTreeSet::from([basic])).is_some(),
            "the basic phone has no social network"
        );
        assert_eq!(
            nowhere(&story, post, &BTreeSet::from([smart])),
            None,
            "the smartphone has"
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
