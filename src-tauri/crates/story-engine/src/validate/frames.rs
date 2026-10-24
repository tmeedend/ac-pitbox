//! The checks of a story's frames and skins (SPEC-story-mode, Skins: how a
//! frame gets its look), and of what a Beat changes in the phone.

use std::collections::{HashMap, HashSet};

use super::{media_path_problem, Checker};
use crate::frame::{Appears, Template, FRAMES as FILE};
use crate::model::{Beat, NodeKind};
use crate::skin::{self, Skin, SKINS};

/// The files a phone's look can be, by what they are.
const PICTURE: [&str; 4] = ["png", "webp", "jpg", "jpeg"];
const SOUND: [&str; 3] = ["ogg", "mp3", "wav"];

fn extension(path: &str) -> String {
    crate::scene::extension(path).unwrap_or_default()
}

impl Checker<'_> {
    /// The frames a story declares: each with apps, each app showing what
    /// its template can, a content type in one app of the frame only - two
    /// would leave the item nowhere to be told apart - its icon and the
    /// frame's pictures and sounds files of `media/`, its skin one for its
    /// template, and one that appears from a Beat unlocked by one. Then the
    /// skins themselves.
    pub(super) fn frames(&mut self) {
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
            if let Some(body) = &frame.body {
                self.look_file(&at, "body", &body.file, &PICTURE[..2]);
                if body.screen.width == 0 || body.screen.height == 0 {
                    self.error(FILE, format!("{at}: the body's screen has no width or no height"));
                }
            }
            if let Some(wallpaper) = &frame.wallpaper {
                self.look_file(&at, "wallpaper", wallpaper, &PICTURE);
            }
            for (what, sound) in frame.sounds.files() {
                self.look_file(&at, what, sound, &SOUND);
            }
            let (mut ids, mut shown) = (HashSet::new(), HashMap::new());
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
                if let Some(why) = app.skin.as_deref().and_then(|s| self.skin_problem(s, app.template)) {
                    self.error(FILE, format!("{at}: {why}"));
                }
                if app.appears == Appears::Beat && !unlocked.contains(app.id.as_str()) {
                    self.warning(
                        FILE,
                        format!("{at} appears from a Beat, and no Beat unlocks it: it shows with its first item only"),
                    );
                }
            }
        }
        for s in &story.skins {
            self.skin(s);
        }
    }

    /// A picture or a sound of a frame's look: a file of `media/`, of a kind
    /// the phone can show or play.
    fn look_file(&mut self, at: &str, what: &str, path: &str, kinds: &[&str]) {
        self.media_file(FILE, at, Some(what), path);
        if !kinds.contains(&extension(path).as_str()) {
            self.error(
                FILE,
                format!("{at}: {what} `{path}` is not one of {}", kinds.join(", ")),
            );
        }
    }

    /// Why an app of `template` cannot wear skin `id`, if it cannot.
    fn skin_problem(&self, id: &str, template: Template) -> Option<String> {
        match self.story.skin(id) {
            None => Some(format!(
                "unknown skin `{id}` (a skin is a folder {SKINS}{id}/ with its skin.yaml)"
            )),
            Some(s) if s.template != template => Some(format!(
                "skin `{id}` dresses a {}, not a {}",
                s.template.name(),
                template.name()
            )),
            Some(_) => None,
        }
    }

    /// A skin: its colours among those a skin gives, written `#rrggbb`; its
    /// slots among its template's; its files in its own folder, of a kind
    /// a skin can carry.
    fn skin(&mut self, s: &Skin) {
        let file = format!("{SKINS}{}/skin.yaml", s.id);
        for (name, value) in &s.colors {
            if !skin::COLORS.contains(&name.as_str()) {
                self.error(
                    &file,
                    format!("no colour `{name}` (the colours are {})", skin::COLORS.join(", ")),
                );
            } else if !skin::is_color(value) {
                self.error(&file, format!("colour `{name}`: `{value}` is not written #rrggbb"));
            }
        }
        if let Some(font) = &s.font {
            self.skin_file(&file, s, "font", font, &skin::FONT);
        }
        let slots = skin::slots(s.template);
        for (name, slot) in &s.slots {
            if !slots.contains(&name.as_str()) {
                self.error(
                    &file,
                    format!(
                        "a {} has no slot `{name}` (its slots are {})",
                        s.template.name(),
                        slots.join(", ")
                    ),
                );
            }
            self.skin_file(&file, s, &format!("slot `{name}`"), &slot.file, &skin::IMAGE);
        }
    }

    /// A file a skin names: beside its `skin.yaml`, there, and of `kinds`.
    fn skin_file(&mut self, file: &str, s: &Skin, what: &str, path: &str, kinds: &[&str]) {
        let inside = format!("{SKINS}{}/{path}", s.id);
        if path.contains('/') || media_path_problem(path).is_some() {
            self.error(
                file,
                format!("{what}: `{path}` is a file of the skin's own folder, by its name"),
            );
        } else if self.story.skin_files.as_ref().is_some_and(|f| !f.contains(&inside)) {
            self.error(file, format!("{what}: `{inside}` does not exist"));
        } else if !kinds.contains(&extension(path).as_str()) {
            self.error(file, format!("{what}: `{path}` is not one of {}", kinds.join(", ")));
        }
    }

    /// What a Beat changes in the phone: apps to unlock and to dress, that
    /// one of the story's frames has, in a skin for that app's template;
    /// the status bar's battery and signal within their range.
    pub(super) fn beat_phone(&mut self, file: &str, at: &str, beat: &Beat) {
        let story = self.story;
        // The apps by id, with their templates, in every frame that can be up.
        let mut apps: Vec<(String, Template)> = (story.frames.iter())
            .flat_map(|f| f.apps.iter().map(|a| (a.id.clone(), a.template)))
            .collect();
        if story.frame("phone").is_none() {
            apps.extend(crate::frame::phone_apps().into_iter().map(|a| (a.id, a.template)));
        }
        let template = |id: &str| apps.iter().find(|(a, _)| a == id).map(|(_, t)| *t);
        for id in &beat.unlock {
            if template(id).is_none() {
                self.error(file, format!("{at}: unlock: no frame has an app `{id}`"));
            }
        }
        for (app, skin) in &beat.skins {
            match template(app) {
                None => self.error(file, format!("{at}: skins: no frame has an app `{app}`")),
                Some(t) => {
                    if let Some(why) = self.skin_problem(skin, t) {
                        self.error(file, format!("{at}: skins: app `{app}`: {why}"));
                    }
                }
            }
        }
        if let Some(status) = &beat.status {
            if status.battery.is_some_and(|b| b > 100) {
                self.error(file, format!("{at}: status: the battery is a percentage, 0 to 100"));
            }
            if status.signal.is_some_and(|s| s > 4) {
                self.error(file, format!("{at}: status: the signal is 0 to 4 bars"));
            }
        }
    }
}
