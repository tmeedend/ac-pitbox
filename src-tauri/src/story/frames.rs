//! The Frame screen's own work on disk (SPEC-story-mode, Skins: how a frame
//! gets its look): bringing a file the author picked into the story, and the
//! checks only the pictures themselves can answer - a screen that goes past
//! its body, a wallpaper of another shape than its screen, an icon that is
//! not square, a 9-slice whose margins leave nothing to stretch. The engine
//! never reads a picture: it lists files, and these checks come on top of
//! its own.

use std::path::Path;

use story_engine::frame::Template;
use story_engine::skin::{self, SKINS};
use story_engine::validate::{Diagnostic, Severity};
use story_engine::Story;

/// What a story can carry, by extension: pictures, a video loop, sounds,
/// fonts. Anything else is refused at the door.
const IMPORTABLE: [&str; 12] = [
    "png", "webp", "jpg", "jpeg", "webm", "mp4", "ogg", "mp3", "wav", "ttf", "otf", "woff2",
];

/// The smallest icon the home screen draws without blurring.
const ICON_MIN: u32 = 96;

/// Copies `source`, a file the author picked anywhere, into the story at
/// `into`: a folder of `media/` (`media/frames/smartphone`) or a skin's own
/// folder (`frames/skins/chat`). Resolves to the path a story names it by:
/// inside `media/` for the first, the file's name for a skin. A file of the
/// same name and the same bytes is the one already there; another of the
/// same name gets a number, so nothing the story uses is overwritten.
pub fn import_file(folder: &Path, source: &Path, into: &str) -> Result<String, String> {
    let inside_media = into.strip_prefix("media/").or((into == "media").then_some(""));
    let skin = into
        .strip_prefix(SKINS)
        .filter(|id| !id.is_empty() && !id.contains('/'));
    let parts_fine = into.split('/').all(|p| !p.is_empty() && p != "." && p != "..") && !into.contains(['\\', ':']);
    if !parts_fine || (inside_media.is_none() && skin.is_none()) {
        return Err(format!("not a folder a story keeps its files in: {into}"));
    }
    let name =
        (source.file_name().and_then(|n| n.to_str())).ok_or_else(|| format!("no file name: {}", source.display()))?;
    let kind = story_engine::scene::extension(name).unwrap_or_default();
    if !IMPORTABLE.contains(&kind.as_str()) {
        return Err(crate::errors::STORY_FILE_KIND.into());
    }
    let bytes = std::fs::read(source).map_err(|e| format!("{}: {e}", source.display()))?;
    let dir = folder.join(into);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let mut chosen = name.to_string();
    for n in 2.. {
        let at = dir.join(&chosen);
        match std::fs::read(&at) {
            Ok(there) if there == bytes => break,
            Ok(_) => chosen = format!("{stem}-{n}.{ext}"),
            Err(_) => {
                std::fs::write(&at, &bytes).map_err(|e| format!("{}: {e}", at.display()))?;
                break;
            }
        }
    }
    Ok(match inside_media {
        Some("") => chosen,
        Some(sub) => format!("{sub}/{chosen}"),
        None => chosen,
    })
}

/// The size of a picture of the story, if it reads.
fn size_of(path: &Path) -> Option<(u32, u32)> {
    image::image_dimensions(path).ok()
}

/// The checks only the pictures can answer, for every frame and skin of
/// `story` read from `folder`.
pub fn image_checks(folder: &Path, story: &Story) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut say = |severity, file: &str, message: String| {
        out.push(Diagnostic {
            severity,
            file: file.into(),
            node: None,
            message,
        })
    };
    let media = |p: &str| size_of(&folder.join("media").join(p));
    let frames_file = story_engine::frame::FRAMES;
    for frame in &story.frames {
        let at = format!("frame `{}`", frame.id);
        let screen = frame.body.as_ref().map(|b| b.screen);
        if let Some(body) = &frame.body {
            if let Some((w, h)) = media(&body.file) {
                let s = body.screen;
                if s.x + s.width > w || s.y + s.height > h {
                    say(
                        Severity::Error,
                        frames_file,
                        format!("{at}: the screen goes past the body's picture ({w} × {h})"),
                    );
                }
            }
        }
        if let (Some(wallpaper), Some(s)) = (&frame.wallpaper, screen) {
            if let Some((w, h)) = media(wallpaper) {
                let (want, got) = (s.width as f64 / s.height as f64, w as f64 / h as f64);
                if (want - got).abs() / want > 0.05 {
                    say(
                        Severity::Warning,
                        frames_file,
                        format!(
                            "{at}: the wallpaper ({w} × {h}) is not the shape of its screen ({} × {}): it is cropped",
                            s.width, s.height
                        ),
                    );
                }
                if w < s.width {
                    say(
                        Severity::Warning,
                        frames_file,
                        format!(
                            "{at}: the wallpaper ({w} px wide) is smaller than its screen ({} px): it shows blurred",
                            s.width
                        ),
                    );
                }
            }
        }
        for app in &frame.apps {
            let Some((w, h)) = app.icon.as_deref().and_then(media) else {
                continue;
            };
            let at = format!("{at}, app `{}`", app.id);
            if w != h {
                say(
                    Severity::Warning,
                    frames_file,
                    format!("{at}: its icon ({w} × {h}) is not square: it is stretched"),
                );
            } else if w < ICON_MIN {
                say(
                    Severity::Warning,
                    frames_file,
                    format!("{at}: its icon ({w} px) is smaller than {ICON_MIN} px: it shows blurred"),
                );
            }
        }
    }
    for s in &story.skins {
        let file = format!("{SKINS}{}/skin.yaml", s.id);
        let reference = if s.template.is_app() {
            skin::REFERENCE_WIDTH
        } else {
            skin::SCENE_WIDTH
        };
        for (name, slot) in &s.slots {
            let Some((w, h)) = size_of(&folder.join(SKINS).join(&s.id).join(&slot.file)) else {
                continue;
            };
            if let Some([t, r, b, l]) = slot.slice {
                if t + b >= h || l + r >= w {
                    say(
                        Severity::Error,
                        &file,
                        format!("slot `{name}`: its margins ({t}, {r}, {b}, {l}) leave nothing to stretch in a {w} × {h} picture"),
                    );
                }
            }
            let fills = matches!(name.as_str(), "background" | "paper" | "set");
            if fills && w < reference / 2 {
                let what = if s.template == Template::Tv { "scene" } else { "screen" };
                say(
                    Severity::Warning,
                    &file,
                    format!("slot `{name}`: {w} px wide, drawn for a {what} {reference} px wide: it shows blurred"),
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn png(path: &Path, w: u32, h: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbaImage::new(w, h).save(path).unwrap();
    }

    // A file picked anywhere lands in the story's folder asked for, named by
    // what a story writes; the same file twice is the same file, another of
    // the same name gets a number; nothing lands outside, nor of a kind a
    // story cannot carry.
    #[test]
    fn a_picked_file_is_copied_into_the_story_without_overwriting() {
        let base = temp_dir("story-import");
        let (folder, outside) = (base.join("s"), base.join("picked"));
        png(&outside.join("body.png"), 4, 8);
        let path = import_file(&folder, &outside.join("body.png"), "media/frames/old").expect("imported");
        assert_eq!(path, "frames/old/body.png", "a path inside media/");
        assert!(folder.join("media/frames/old/body.png").is_file());
        assert_eq!(
            import_file(&folder, &outside.join("body.png"), "media/frames/old").unwrap(),
            "frames/old/body.png",
            "the same file again: the one already there"
        );
        png(&outside.join("other/body.png"), 2, 2);
        assert_eq!(
            import_file(&folder, &outside.join("other/body.png"), "media/frames/old").unwrap(),
            "frames/old/body-2.png",
            "another file of the same name: numbered"
        );
        assert_eq!(
            import_file(&folder, &outside.join("body.png"), "frames/skins/chat").unwrap(),
            "body.png",
            "in a skin: its name"
        );
        std::fs::write(outside.join("notes.txt"), "x").unwrap();
        assert!(
            import_file(&folder, &outside.join("notes.txt"), "media").is_err(),
            "not a story's kind"
        );
        for into in ["frames", "media/../x", "frames/skins/a/b", "C:/x", "flow"] {
            assert!(
                import_file(&folder, &outside.join("body.png"), into).is_err(),
                "{into} refused"
            );
        }
    }

    // The pictures say what the engine cannot: a screen past its body, a
    // wallpaper of another shape, an icon not square, margins that leave
    // nothing to stretch, a background drawn too small.
    #[test]
    fn the_pictures_are_checked_for_size_and_shape() {
        let base = temp_dir("story-images");
        let folder = base.join("s");
        png(&folder.join("media/body.png"), 100, 200);
        png(&folder.join("media/wall.png"), 50, 50);
        png(&folder.join("media/icon.png"), 64, 32);
        png(&folder.join("frames/skins/chat/bubble.png"), 40, 40);
        png(&folder.join("frames/skins/chat/bg.png"), 300, 600);
        let files = vec![
            story_engine::load::SourceFile::new(
                "story.yaml",
                "format: 1\nid: s\ntitle: S\nversion: \"1\"\nlanguages: [en]\nstart:\n  node: start\n  frame: p\n",
            ),
            story_engine::load::SourceFile::new("flow/01.yaml", "id: c\nnodes:\n  - id: start\n    end: done\n"),
            story_engine::load::SourceFile::new(
                story_engine::frame::FRAMES,
                "frames:\n  - id: p\n    kind: phone\n    body: { file: body.png, screen: { x: 10, y: 10, width: 95, height: 100 } }\n    wallpaper: wall.png\n    apps:\n      - { id: m, icon: icon.png, template: thread, shows: [message], skin: chat }\n",
            ),
            story_engine::load::SourceFile::new(
                "frames/skins/chat/skin.yaml",
                "template: thread\nslots:\n  bubble_in: { file: bubble.png, slice: [20, 10, 20, 10] }\n  background: { file: bg.png }\n",
            ),
        ];
        let story = Story::from_sources(files).unwrap();
        let found = image_checks(&folder, &story);
        let says = |severity: Severity, needle: &str| {
            found
                .iter()
                .any(|d| d.severity == severity && d.message.contains(needle))
        };
        assert!(
            says(Severity::Error, "the screen goes past the body's picture (100 × 200)"),
            "{found:#?}"
        );
        assert!(says(Severity::Warning, "is not the shape of its screen"), "{found:#?}");
        assert!(says(Severity::Warning, "smaller than its screen"), "{found:#?}");
        assert!(
            says(Severity::Warning, "its icon (64 × 32) is not square"),
            "{found:#?}"
        );
        assert!(
            says(Severity::Error, "leave nothing to stretch in a 40 × 40 picture"),
            "{found:#?}"
        );
        assert!(says(Severity::Warning, "slot `background`: 300 px wide"), "{found:#?}");
    }
}
