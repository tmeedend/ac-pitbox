//! Skins (SPEC-story-mode, Skins: how a frame gets its look): images,
//! colours and a font placed on a template's slots. A skin is a folder,
//! `frames/skins/<id>/`, holding `skin.yaml` and its files, so that it can be
//! copied from one story to another whole; its paths are inside that folder.
//! A skin never changes what a template does, and holds no code.
//!
//! **What is missing falls back**: a slot, a colour or the font left out
//! takes Pit Box's theme, so a skin can start with one image.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::frame::Template;

/// The folder of a story's skins: `frames/skins/<id>/skin.yaml`.
pub const SKINS: &str = "frames/skins/";

/// A skin's id from the path of its `skin.yaml`, if it is one.
pub fn skin_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix(SKINS)?.strip_suffix("/skin.yaml")?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

/// `frames/skins/<id>/skin.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Skin {
    /// Its folder's name; not written in the file.
    #[serde(skip)]
    pub id: String,
    /// The template it dresses: an app of another template cannot wear it.
    pub template: Template,
    /// `#rrggbb`, by name (`COLORS`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
    /// The text's font, a file of the skin's folder (TTF, OTF, WOFF2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// By slot name (`slots`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Slot>,
}

/// An image placed on a slot.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    /// A file of the skin's folder: PNG or WebP, drawn for a screen 1080 px
    /// wide.
    pub file: String,
    /// For a slot that stretches (a bubble, a header): its fixed margins,
    /// top, right, bottom, left, in the image's pixels - the 9-slice that
    /// keeps a drawn edge as drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice: Option<[u32; 4]>,
}

/// The colours a skin can give: the screen's background, its text, the
/// quieter text, and the accent (the player's own bubbles, the buttons).
pub const COLORS: [&str; 4] = ["background", "text", "muted", "accent"];

/// The width the images of a skin are drawn for: the phone screen's, or
/// the scene's for what plays there.
pub const REFERENCE_WIDTH: u32 = 1080;
pub const SCENE_WIDTH: u32 = 1920;

/// The images a skin can be.
pub const IMAGE: [&str; 2] = ["png", "webp"];
/// The fonts a skin can carry.
pub const FONT: [&str; 3] = ["ttf", "otf", "woff2"];

/// The slots `template` offers: the screen's background and the app's
/// header, then its own pieces.
pub fn slots(template: Template) -> &'static [&'static str] {
    match template {
        Template::Thread => &["background", "header", "bubble_in", "bubble_out", "reply"],
        Template::Inbox => &["background", "header", "row"],
        Template::Feed => &["background", "header", "post"],
        Template::Statement => &["background", "header", "row"],
        Template::Agenda => &["background", "header", "row"],
        Template::Call => &["background", "header", "bubble_in", "bubble_out", "reply"],
        Template::Dialogue | Template::Landline => &["textbox", "nameplate", "choice"],
        Template::Newspaper => &["paper", "masthead", "photo"],
        Template::Letter => &["paper"],
        Template::Tv => &["set", "lower_third"],
    }
}

/// `#rrggbb`.
pub fn is_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skin_is_named_by_its_folder() {
        assert_eq!(skin_id("frames/skins/chat_blue/skin.yaml"), Some("chat_blue"));
        assert_eq!(skin_id("frames/skins/skin.yaml"), None, "no folder");
        assert_eq!(skin_id("frames/skins/a/b/skin.yaml"), None, "one level only");
        assert_eq!(skin_id("frames/frames.yaml"), None);
        assert!(is_color("#1a2B3c") && !is_color("1a2b3c") && !is_color("#12345"));
    }
}
