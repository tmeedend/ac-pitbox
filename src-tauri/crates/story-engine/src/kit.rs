//! The starter kit (SPEC-story-mode, Skins: how a frame gets its look): one
//! template sheet per template, showing where each slot sits, its reference
//! size, the stretch margins we suggest for it and what to keep clear - so
//! an author knows what images to make, whatever the tool. Drawn here, beside
//! the slot table (`skin::slots`), so a sheet can never miss a slot: a test
//! holds the two together. Pure: an SVG string, which the editor saves as
//! it is and as a PNG.

use std::fmt::Write;

use crate::frame::Template;
use crate::skin::{REFERENCE_WIDTH, SCENE_WIDTH};

/// The phone screen a sheet is drawn on, 1080 px wide like a skin's images.
const PHONE: (u32, u32) = (REFERENCE_WIDTH, 2340);
/// The scene, for what plays on it.
const SCENE: (u32, u32) = (SCENE_WIDTH, 1080);

/// A slot placed on its sheet, at the size it is drawn at.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub slot: &'static str,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// The stretch margins we suggest (top, right, bottom, left), what
    /// keeps a drawn edge as drawn; zeros for a picture stretched whole.
    pub margins: [u32; 4],
}

/// A part of the screen to leave clear of anything that matters.
#[derive(Debug, Clone, Copy)]
pub struct Clear {
    pub what: &'static str,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// One template's sheet.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub template: Template,
    pub width: u32,
    pub height: u32,
    pub pieces: Vec<Piece>,
    pub clear: Vec<Clear>,
}

const fn piece(slot: &'static str, x: u32, y: u32, w: u32, h: u32, m: u32) -> Piece {
    Piece {
        slot,
        x,
        y,
        w,
        h,
        margins: [m, m, m, m],
    }
}

/// A piece stretched across: margins on its sides only.
const fn band(slot: &'static str, x: u32, y: u32, w: u32, h: u32, side: u32) -> Piece {
    Piece {
        slot,
        x,
        y,
        w,
        h,
        margins: [0, side, 0, side],
    }
}

/// The sheet of `template`.
pub fn sheet(template: Template) -> Sheet {
    use Template as T;
    let phone = |pieces: Vec<Piece>| Sheet {
        template,
        width: PHONE.0,
        height: PHONE.1,
        pieces: [vec![piece("background", 0, 0, PHONE.0, PHONE.1, 0)], pieces].concat(),
        clear: vec![Clear {
            what: "status bar: drawn by Pit Box",
            x: 0,
            y: 0,
            w: PHONE.0,
            h: 110,
        }],
    };
    let header = band("header", 0, 110, PHONE.0, 180, 60);
    let scene = |pieces: Vec<Piece>, clear: Vec<Clear>| Sheet {
        template,
        width: SCENE.0,
        height: SCENE.1,
        pieces,
        clear,
    };
    let stage = || {
        scene(
            vec![
                piece("textbox", 160, 790, 1600, 250, 60),
                piece("nameplate", 200, 715, 380, 80, 30),
                piece("choice", 510, 300, 900, 100, 40),
            ],
            vec![Clear {
                what: "portraits stand here",
                x: 160,
                y: 120,
                w: 1600,
                h: 160,
            }],
        )
    };
    match template {
        T::Thread | T::Call => phone(vec![
            header,
            piece("bubble_in", 40, 360, 720, 150, 48),
            piece("bubble_out", 320, 560, 720, 150, 48),
            piece("reply", 90, 2120, 900, 140, 40),
        ]),
        T::Inbox | T::Statement | T::Agenda => phone(vec![header, band("row", 0, 290, PHONE.0, 200, 40)]),
        T::Feed => phone(vec![header, piece("post", 40, 330, 1000, 600, 40)]),
        T::Dialogue | T::Landline => stage(),
        T::Newspaper => scene(
            vec![
                piece("paper", 290, 40, 1340, 1000, 40),
                band("masthead", 330, 80, 1260, 140, 60),
                piece("photo", 330, 420, 1260, 380, 20),
            ],
            Vec::new(),
        ),
        T::Letter => scene(vec![piece("paper", 560, 60, 800, 960, 60)], Vec::new()),
        T::Tv => scene(
            vec![
                piece("set", 270, 110, 1380, 860, 90),
                band("lower_third", 330, 760, 1260, 140, 40),
            ],
            vec![Clear {
                what: "the still shows here",
                x: 360,
                y: 200,
                w: 1200,
                h: 540,
            }],
        ),
    }
}

/// The file name of `template`'s sheet, without its extension.
pub fn file_name(template: Template) -> String {
    format!("{}-sheet", template.name())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `sheet` as an SVG drawing at its reference size: each slot with its name,
/// its size and its suggested margins (dashed), what to keep clear hatched,
/// and a title saying what the sheet is for.
pub fn svg(sheet: &Sheet) -> String {
    let (w, h) = (sheet.width, sheet.height);
    // The title and the key in a band above the drawing, which keeps the
    // sheet's own coordinates: a slot at (x, y) on the screen is at (x, y)
    // in the group below the band.
    let band = 150;
    let full = h + band;
    let name = sheet.template.name();
    let mut out = String::new();
    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{full}" viewBox="0 0 {w} {full}" font-family="sans-serif">
<rect width="{w}" height="{band}" fill="#eef0f5"/>
<text x="24" y="52" font-size="30" font-weight="bold" fill="#1d2433">Pit Box starter kit - {name} ({w} × {h} px)</text>
<text x="24" y="92" font-size="22" fill="#6a6a78">Draw each slot at the size shown, as PNG or WebP.</text>
<text x="24" y="124" font-size="22" fill="#6a6a78">Dashed lines: the stretch margins we suggest (top right bottom left).</text>
<g transform="translate(0 {band})">
<defs><pattern id="hatch" width="24" height="24" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="24" height="24" fill="#f4f4f6"/><line x1="0" y1="0" x2="0" y2="24" stroke="#c9c9d1" stroke-width="10"/></pattern></defs>
<rect width="{w}" height="{h}" fill="#ffffff"/>"##
    );
    for c in &sheet.clear {
        let _ = writeln!(
            out,
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#hatch)"/><text x="{}" y="{}" font-size="26" fill="#6a6a78">Keep clear - {}</text>"##,
            c.x,
            c.y,
            c.w,
            c.h,
            c.x + 16,
            c.y + c.h / 2 + 9,
            escape(c.what)
        );
    }
    for (i, p) in sheet.pieces.iter().enumerate() {
        out.push_str(&piece_svg(sheet, i, p));
    }
    let _ = writeln!(
        out,
        r##"</g>
</svg>"##
    );
    out
}

/// One slot on its sheet: its box, its suggested margins dashed, and its
/// name, size and margins written in it.
fn piece_svg(sheet: &Sheet, i: usize, p: &Piece) -> String {
    let mut out = String::new();
    // The background is the whole sheet: drawn as its outline, the rest over
    // it in a colour of their own.
    let whole = p.w == sheet.width && p.h == sheet.height;
    let (fill, stroke) = if whole {
        ("none", "#9a9aa8")
    } else {
        (["#dce8ff", "#ffe2d6", "#dff3df", "#f1e2fb"][i % 4], "#3b4a6b")
    };
    let _ = writeln!(
        out,
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" stroke="{stroke}" stroke-width="3"/>"##,
        p.x, p.y, p.w, p.h
    );
    let [t, r, b, l] = p.margins;
    let (x0, y0, x1, y1) = (p.x, p.y, p.x + p.w, p.y + p.h);
    // Each margin as a line across the piece, where it is not zero.
    let lines = [
        (t, (x0, y0 + t, x1, y0 + t)),
        (b, (x0, y1 - b, x1, y1 - b)),
        (l, (x0 + l, y0, x0 + l, y1)),
        (r, (x1 - r, y0, x1 - r, y1)),
    ];
    for (_, (ax, ay, bx, by)) in lines.iter().filter(|(m, _)| *m > 0) {
        let _ = writeln!(
            out,
            r##"<line x1="{ax}" y1="{ay}" x2="{bx}" y2="{by}" stroke="#d0533a" stroke-width="2" stroke-dasharray="10 8"/>"##
        );
    }
    // Named at its top, or at its bottom when it holds other pieces - the
    // paper, the TV set, the background - so names never overlap.
    let holds = (sheet.pieces.iter())
        .any(|o| !std::ptr::eq(o, p) && o.x >= x0 && o.y >= y0 && o.x + o.w <= x1 && o.y + o.h <= y1);
    let (tx, ty) = if holds || whole {
        (x0 + 14, y1 - 46)
    } else {
        (x0 + 14, y0 + 34)
    };
    let size = format!("{} × {} px", p.w, p.h);
    let margins = if p.margins == [0; 4] {
        "stretched whole".to_string()
    } else {
        format!("margins {t} {r} {b} {l}")
    };
    let _ = writeln!(
        out,
        r##"<text x="{tx}" y="{ty}" font-size="28" font-weight="bold" fill="#1d2433">{}</text><text x="{tx}" y="{}" font-size="22" fill="#3b4a6b">{size} - {margins}</text>"##,
        p.slot,
        ty + 28
    );
    out
}

/// Every sheet: what the Frame screen saves.
pub fn sheets() -> Vec<(String, String)> {
    (Template::ALL.into_iter())
        .map(|t| (file_name(t), svg(&sheet(t))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin;

    // A sheet shows every slot its template has, nothing else, inside the
    // sheet, each with margins that leave something to stretch - so the kit
    // can never fall behind the slot table, and an image drawn to it passes
    // the checks.
    #[test]
    fn every_sheet_shows_every_slot_of_its_template_and_fits() {
        for t in Template::ALL {
            let s = sheet(t);
            let mut drawn: Vec<&str> = s.pieces.iter().map(|p| p.slot).collect();
            drawn.sort();
            let mut wanted = skin::slots(t).to_vec();
            wanted.sort();
            assert_eq!(drawn, wanted, "the slots of a {}", t.name());
            let width = if t.is_app() { REFERENCE_WIDTH } else { SCENE_WIDTH };
            assert_eq!(s.width, width, "a {} is drawn at its reference width", t.name());
            for p in &s.pieces {
                assert!(
                    p.x + p.w <= s.width && p.y + p.h <= s.height,
                    "{} inside its sheet",
                    p.slot
                );
                let [top, r, b, l] = p.margins;
                assert!(top + b < p.h && l + r < p.w, "{}: something left to stretch", p.slot);
            }
            let svg = svg(&s);
            assert!(
                svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"),
                "an SVG drawing"
            );
            for p in &s.pieces {
                assert!(svg.contains(&format!(">{}<", p.slot)), "{} named on the sheet", p.slot);
            }
        }
        assert_eq!(sheets().len(), Template::ALL.len(), "one sheet per template");
        assert_eq!(file_name(Template::Thread), "thread-sheet");
    }
}
