//! What a version in the showcase keeps on disk (ESPACE§3), and what says so.
//!
//! A version "in the showcase" lost its heavy files — models, textures, sound
//! banks, physics — and kept a **skeleton**: exactly what the lists read (the
//! `ui/` files, one frozen image) plus a manifest of what went. Three things
//! live here and nowhere else, because the export (EXPORT§) reuses them as is:
//!
//! - the **whitelist** ([`is_kept`]), the single definition of a skeleton;
//! - the **manifest** ([`Manifest`]), which lets the disk say what is missing
//!   without the base, and lets a rehydration check it put everything back;
//! - the **guard** ([`guard`]), the one refusal every path that lays a mod
//!   in the game goes through (ESPACE R5): a skeleton deployed in `content/`
//!   is a car without a model, and both AC and Content Manager crash on it.
//!
//! Putting a mod in the showcase — the removal itself — is `showcase.rs`.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::modscan::ModKind;

/// The manifest, at the root of every freed folder (ESPACE§4.2).
pub const MANIFEST_NAME: &str = ".pitbox-vitrine.json";
/// The frozen card image (ESPACE§3.3), `.png` or `.jpg` after this stem.
const IMAGE_STEM: &str = ".pitbox-vitrine";
const IMAGE_EXTENSIONS: [&str; 2] = ["png", "jpg"];
/// Width of every image a skeleton keeps, the frozen one and the reduced
/// track previews (ESPACE§3.3): enough for the largest card, a few dozen KB.
pub const IMAGE_WIDTH: u32 = 480;
const JPEG_QUALITY: u8 = 80;
/// Version of the manifest format, bumped on an incompatible change.
const MANIFEST_FORMAT: u32 = 1;

/// A path relative to a mod folder, lower-cased with `/` separators: Windows
/// paths compare without case, and the whitelist is written once for both
/// separators.
fn normalized(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// True for the showcase's own files at the root: manifest and frozen image.
fn is_own_file(rel: &str) -> bool {
    rel == MANIFEST_NAME || IMAGE_EXTENSIONS.iter().any(|ext| rel == format!("{IMAGE_STEM}.{ext}"))
}

/// Whether `rel`, relative to a folder, is one of the showcase's own files at
/// its root — the manifest or the frozen image —, not one of the mod's.
pub fn is_showcase_file(rel: &Path) -> bool {
    is_own_file(&normalized(rel))
}

/// Whether `rel`, a file of a car or track folder, stays in its skeleton
/// (ESPACE§3.1). **The only definition of a skeleton**, for the showcase as
/// for the export.
///
/// A car keeps its `ui/*.json` (name, brand, specs, curves, tags) and its
/// badge; a track keeps, for every layout, its `ui_track.json`, outline, logo
/// and preview — the preview reduced ([`is_reduced`]). Everything else goes:
/// `data.acd` above all, encrypted by Kunos, whose content is already in the
/// base (FICHE§); skin previews, whose names are too (`versions.skins`).
///
/// Two additions to the table of ESPACE§3.1, both read by the track card
/// (`inspect::track_outline`) and both small: `outline.jpg` next to
/// `outline.png`, and a `map.png` **inside `ui/`** — not the layout's own
/// `<layout>/map.png`, which goes as the spec says.
pub fn is_kept(kind: ModKind, rel: &Path) -> bool {
    let rel = normalized(rel);
    if is_own_file(&rel) {
        return true;
    }
    let Some(inside_ui) = rel.strip_prefix("ui/") else {
        return false;
    };
    match kind {
        ModKind::Car => {
            // Directly in `ui/`: a car's `ui/` subfolders are CM's own
            // caches, never read by the lists.
            !inside_ui.contains('/')
                && (inside_ui.ends_with(".json") || inside_ui == "badge.png" || inside_ui == "badge.jpg")
        }
        ModKind::Track => {
            let name = inside_ui.rsplit('/').next().unwrap_or(inside_ui);
            matches!(
                name,
                "ui_track.json"
                    | "outline.png"
                    | "outline.jpg"
                    | "map.png"
                    | "logo.png"
                    | "logo.jpg"
                    | "preview.png"
                    | "preview.jpg"
            )
        }
    }
}

/// Whether a kept file is kept **reduced** to [`IMAGE_WIDTH`] (ESPACE§3.3):
/// a track preview is a full-size photo, where logos and outlines are already
/// small PNGs that would lose their sharpness for a few KB.
pub fn is_reduced(kind: ModKind, rel: &Path) -> bool {
    if kind != ModKind::Track {
        return false;
    }
    let rel = normalized(rel);
    rel.starts_with("ui/") && (rel.ends_with("/preview.png") || rel.ends_with("/preview.jpg"))
}

/// The frozen card image of a skeleton, when it has one.
pub fn image_of(dir: &Path) -> Option<PathBuf> {
    IMAGE_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{IMAGE_STEM}.{ext}")))
        .find(|p| p.is_file())
}

/// Writes `src` as the frozen image of `dir` (ESPACE§3.3): [`IMAGE_WIDTH`]
/// wide, PNG when it has transparency (a regenerated grid thumbnail), JPEG
/// otherwise. Replaces a previous one of either format.
pub fn freeze_image(src: &Path, dir: &Path) -> Result<PathBuf, String> {
    let (img, _) = decode(src)?;
    let img = narrowed(img);
    for old in IMAGE_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{IMAGE_STEM}.{ext}")))
    {
        if old.is_file() {
            std::fs::remove_file(&old).map_err(|e| format!("{}: {e}", old.display()))?;
        }
    }
    let transparent = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < u8::MAX);
    let dest = dir.join(format!("{IMAGE_STEM}.{}", if transparent { "png" } else { "jpg" }));
    write_image(&img, &dest, transparent)?;
    Ok(dest)
}

/// Reduces an image to [`IMAGE_WIDTH`] where it lies, in its own format — the
/// one its bytes say, whatever its extension.
/// Returns the bytes it gave back; an image already narrow enough is left
/// untouched.
pub fn reduce_in_place(path: &Path) -> Result<u64, String> {
    let before = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    let (img, format) = decode(path)?;
    if img.width() <= IMAGE_WIDTH {
        return Ok(0);
    }
    write_image(&narrowed(img), path, format == image::ImageFormat::Png)?;
    let after = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    Ok(before.saturating_sub(after))
}

/// An image and its real format, read from its bytes and not from its name:
/// mods ship JPEGs named `preview.png`, which the game reads all the same.
fn decode(path: &Path) -> Result<(image::DynamicImage, image::ImageFormat), String> {
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let reader = image::ImageReader::open(path)
        .map_err(|e| err(&e))?
        .with_guessed_format()
        .map_err(|e| err(&e))?;
    let format = reader.format().ok_or_else(|| err(&"unknown image format"))?;
    let img = reader.decode().map_err(|e| err(&e))?;
    Ok((img, format))
}

fn narrowed(img: image::DynamicImage) -> image::DynamicImage {
    if img.width() <= IMAGE_WIDTH {
        return img;
    }
    let height = ((img.height() as u64 * IMAGE_WIDTH as u64) / img.width() as u64).max(1) as u32;
    img.resize_exact(IMAGE_WIDTH, height, image::imageops::FilterType::Triangle)
}

fn write_image(img: &image::DynamicImage, dest: &Path, png: bool) -> Result<(), String> {
    let file = std::fs::File::create(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut out = std::io::BufWriter::new(file);
    let res = if png {
        img.write_to(&mut out, image::ImageFormat::Png)
    } else {
        // JPEG has no alpha: encoding an RGBA image fails outright.
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&img.to_rgb8())
    };
    res.map_err(|e| format!("{}: {e}", dest.display()))
}

// --- The manifest ---------------------------------------------------------------

/// One file a skeleton no longer has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemovedFile {
    /// Relative to the freed folder, `/`-separated.
    pub path: String,
    pub size: u64,
}

/// `.pitbox-vitrine.json` (ESPACE§4.2): what went, and what it came from.
/// Enough to tell what is missing without the base, to check a rehydration,
/// and to find the archive again if the base is lost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub freed_at: String,
    pub mod_id: String,
    pub version_label: Option<String>,
    /// The signature of the **complete** version, as stored at import.
    /// Never recomputed on a skeleton: it would hash what is left, and the
    /// original is what recognizes the archive when it comes back (ESPACE§7.3).
    pub content_signature: Option<String>,
    pub source_archive: Option<String>,
    pub source_site: Option<String>,
    pub source_file_name: Option<String>,
    pub removed: Vec<RemovedFile>,
    pub removed_bytes: u64,
}

impl Manifest {
    pub fn new(freed_at: String, mod_id: String, removed: Vec<RemovedFile>) -> Self {
        let removed_bytes = removed.iter().map(|f| f.size).sum();
        Self {
            format: MANIFEST_FORMAT,
            freed_at,
            mod_id,
            version_label: None,
            content_signature: None,
            source_archive: None,
            source_site: None,
            source_file_name: None,
            removed,
            removed_bytes,
        }
    }
}

/// The manifest of a freed folder; `None` when there is none, or one this
/// version cannot read (a newer format, a damaged file).
pub fn read_manifest(dir: &Path) -> Option<Manifest> {
    let raw = std::fs::read_to_string(dir.join(MANIFEST_NAME)).ok()?;
    serde_json::from_str::<Manifest>(&raw)
        .inspect_err(|e| log::warn!("unreadable showcase manifest in {}: {e}", dir.display()))
        .ok()
        .filter(|m| m.format <= MANIFEST_FORMAT)
}

/// Written synchronously, and **before** anything is removed (ESPACE§5.5):
/// an interrupted removal leaves a manifest that says more is missing than
/// is, never less.
pub fn write_manifest(dir: &Path, manifest: &Manifest) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(MANIFEST_NAME), json).map_err(|e| format!("{}: {e}", dir.display()))
}

/// The manifest's paths a rehydrated folder still lacks, or has at another
/// size (ESPACE§7.4). Empty when everything came back.
pub fn verify(dir: &Path, manifest: &Manifest) -> Vec<String> {
    manifest
        .removed
        .iter()
        .filter(|f| std::fs::metadata(dir.join(&f.path)).map(|m| m.len()).ok() != Some(f.size))
        .map(|f| f.path.clone())
        .collect()
}

/// What a recovered app or "other" mod still lacks, against the manifest its
/// folder held (ESPACE§7.4, ESPACE§5.6). Logged, not reported: the archive
/// imported is the one the user has, and the element is back either way.
pub fn warn_missing(id: &str, dir: &Path, manifest: &Manifest) {
    let missing = verify(dir, manifest);
    if let Some(first) = missing.first() {
        log::warn!(
            "{id} recovered from the showcase without {} expected file(s), first {first}",
            missing.len()
        );
    }
}

/// Puts a manifest back after a recovery that failed halfway (ESPACE§5.6):
/// the folder was emptied to receive the files, and the row still says it has
/// none. Without its manifest it would no longer say what it held.
pub fn restore_manifest(dir: &Path, manifest: &Manifest) {
    let restored = std::fs::create_dir_all(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))
        .and_then(|()| write_manifest(dir, manifest));
    if let Err(e) = restored {
        log::warn!("showcase manifest not put back: {e}");
    }
}

/// What a skeleton's folder held before its files went: what it kept, plus
/// what its manifest says left — in `identity::rel_files` form, so the import
/// compares an incoming archive to the complete version (ESPACE§7.5). Without
/// a manifest, only what is left.
pub fn original_files(dir: &Path) -> std::collections::BTreeSet<String> {
    let mut files: std::collections::BTreeSet<String> = crate::identity::rel_files(dir)
        .into_iter()
        .filter(|rel| !is_own_file(rel))
        .collect();
    if let Some(manifest) = read_manifest(dir) {
        files.extend(
            manifest
                .removed
                .iter()
                .map(|f| f.path.to_lowercase().replace('\\', "/")),
        );
    }
    files
}

/// Every file of a folder that keeps no skeleton, with its size: a skin's or a
/// sound's (ESPACE§5.4), an app's or an "other" mod's (ESPACE§5.6). The
/// showcase's own files excepted.
pub fn all_files(dir: &Path) -> Vec<RemovedFile> {
    files_but(dir, |rel, _| is_own_file(&normalized(rel)))
}

/// Every file of `dir` the skeleton does not keep, with its size, sorted.
pub fn removable_files(kind: ModKind, dir: &Path) -> Vec<RemovedFile> {
    files_but(dir, |rel, _| is_kept(kind, rel))
}

/// Largest text file a mod in the showcase keeps among its resources.
const KEPT_NOTE_BYTES: u64 = 64 * 1024;

/// Whether a file of a mod's resources stays when the mod goes into the
/// showcase (ESPACE§5.4): a short text — a notice often says **where to
/// download** the mod, and weighs a few KB.
pub fn is_kept_resource(rel: &Path, size: u64) -> bool {
    let ext = rel
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    size < KEPT_NOTE_BYTES && matches!(ext.as_str(), "txt" | "md" | "nfo" | "url")
}

/// The files of a mod's resources that leave with its files.
pub fn resources_to_free(dir: &Path) -> Vec<RemovedFile> {
    files_but(dir, is_kept_resource)
}

fn files_but(dir: &Path, kept: impl Fn(&Path, u64) -> bool) -> Vec<RemovedFile> {
    let mut out: Vec<RemovedFile> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(dir).ok()?;
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            (!kept(rel, size)).then(|| RemovedFile {
                path: rel.to_string_lossy().replace('\\', "/"),
                size,
            })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

// --- The guard ------------------------------------------------------------------

/// Whether this version lost its files.
pub fn is_skeleton(conn: &Connection, version_id: &str) -> rusqlite::Result<bool> {
    Ok(crate::overlay::version_content_state(conn, version_id)?.as_deref() == Some(crate::overlay::CONTENT_SKELETON))
}

/// Whether a mod is **in the showcase**: its active version is a skeleton
/// (ESPACE§4.1). Derived, never stored — an older version can be a skeleton
/// while the active one is complete.
pub fn is_showcase(conn: &Connection, mod_id: &str) -> rusqlite::Result<bool> {
    match crate::overlay::active_version_id(conn, mod_id)? {
        Some(vid) => is_skeleton(conn, &vid),
        None => Ok(false),
    }
}

/// **The guard of ESPACE R5**: `Err(CONTENT_FREED)` for a skeleton. Called at
/// the head of every path that lays a mod in the game or needs its heavy
/// files; an unknown version passes, its caller reports that better.
pub fn guard(conn: &Connection, version_id: &str) -> Result<(), String> {
    if is_skeleton(conn, version_id).map_err(|e| e.to_string())? {
        return Err(crate::errors::CONTENT_FREED.into());
    }
    Ok(())
}

/// [`guard`] on the active version of a mod.
pub fn guard_mod(conn: &Connection, mod_id: &str) -> Result<(), String> {
    if is_showcase(conn, mod_id).map_err(|e| e.to_string())? {
        return Err(crate::errors::CONTENT_FREED.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn kept(kind: ModKind, rel: &str) -> bool {
        is_kept(kind, Path::new(rel))
    }

    /// Rule (ESPACE§3.1): a car keeps its `ui/*.json` and its badge, nothing
    /// else — no `data.acd`, no skin preview, no model.
    #[test]
    fn a_car_skeleton_keeps_its_ui_json_and_badge_only() {
        for rel in [
            "ui/ui_car.json",
            "ui/badge.png",
            "ui/cm_tags.json",
            "UI\\UI_CAR.JSON",
            MANIFEST_NAME,
        ] {
            assert!(kept(ModKind::Car, rel), "{rel} stays");
        }
        for rel in [
            "data.acd",
            "data/engine.ini",
            "rss_gtm_lanzo_v10.kn5",
            "sfx/rss_gtm_lanzo_v10.bank",
            "skins/red/preview.jpg",
            "skins/red/livery.png",
            "ui/cm_cache/something.json",
            "ui/upgrade.png",
            "extension/ext_config.ini",
            "ui_car.json",
        ] {
            assert!(!kept(ModKind::Car, rel), "{rel} goes");
        }
    }

    /// Rule (ESPACE§3.1): every layout of a track keeps its name, outline,
    /// logo and preview; the layout's own map and every model go.
    #[test]
    fn a_track_skeleton_keeps_every_layouts_ui() {
        for rel in [
            "ui/ui_track.json",
            "ui/outline.png",
            "ui/preview.png",
            "ui/gp/ui_track.json",
            "ui/gp/outline.png",
            "ui/gp/logo.png",
            "ui/gp/preview.png",
        ] {
            assert!(kept(ModKind::Track, rel), "{rel} stays");
        }
        for rel in [
            "shannonville.kn5",
            "gp/map.png",
            "gp/data/surfaces.ini",
            "ui/gp/sponsors.dds",
            "skins/x.dds",
        ] {
            assert!(!kept(ModKind::Track, rel), "{rel} goes");
        }
        assert!(
            is_reduced(ModKind::Track, Path::new("ui/gp/preview.png")),
            "a track preview is reduced"
        );
        assert!(
            !is_reduced(ModKind::Track, Path::new("ui/gp/outline.png")),
            "an outline keeps its sharpness"
        );
        assert!(!is_reduced(ModKind::Car, Path::new("ui/badge.png")));
    }

    /// Rule (ESPACE§4.2): the manifest lists exactly what the whitelist
    /// removes, with sizes, and a round trip reads the same list back.
    #[test]
    fn the_manifest_lists_exactly_what_goes() {
        let base = crate::testutil::temp_dir("skeleton-manifest");
        write(&base, "ui/ui_car.json", b"{}");
        write(&base, "car.kn5", b"12345");
        write(&base, "skins/red/preview.jpg", b"123");
        let removed = removable_files(ModKind::Car, &base);
        assert_eq!(
            removed,
            vec![
                RemovedFile {
                    path: "car.kn5".into(),
                    size: 5
                },
                RemovedFile {
                    path: "skins/red/preview.jpg".into(),
                    size: 3
                },
            ],
            "the model and the skin preview go, the ui stays"
        );
        let manifest = Manifest::new("2026-09-27T10:00:00Z".into(), "car".into(), removed);
        assert_eq!(manifest.removed_bytes, 8, "the total is the sum of the sizes");
        write_manifest(&base, &manifest).unwrap();
        let back = read_manifest(&base).expect("the manifest reads back");
        assert_eq!(back.removed, manifest.removed);
        assert!(
            !removable_files(ModKind::Car, &base)
                .iter()
                .any(|f| f.path == MANIFEST_NAME),
            "the manifest never lists itself"
        );
    }

    /// Rule (ESPACE§7.4): after a rehydration, a file missing or of another
    /// size is reported; a complete folder reports nothing.
    #[test]
    fn verify_reports_what_did_not_come_back() {
        let base = crate::testutil::temp_dir("skeleton-verify");
        let manifest = Manifest::new(
            String::new(),
            "car".into(),
            vec![
                RemovedFile {
                    path: "car.kn5".into(),
                    size: 5,
                },
                RemovedFile {
                    path: "sfx/car.bank".into(),
                    size: 2,
                },
            ],
        );
        write(&base, "car.kn5", b"12345");
        write(&base, "sfx/car.bank", b"1");
        assert_eq!(verify(&base, &manifest), vec!["sfx/car.bank".to_string()], "wrong size");
        write(&base, "sfx/car.bank", b"12");
        assert!(verify(&base, &manifest).is_empty(), "everything is back");
    }

    /// Rule (ESPACE§3.3): the frozen image is narrow, a PNG when it has
    /// transparency (a regenerated thumbnail), a JPEG otherwise — and
    /// freezing again replaces it instead of leaving two.
    /// Rule: an image is read for what it is, not for what its name says. Bug
    /// met on a real library (`Ph_highway`): a JPEG named `preview.png`, which
    /// the game reads all the same, failed to decode as a PNG and left its
    /// 13 MB in the skeleton and in the export.
    #[test]
    fn a_jpeg_named_png_is_reduced_all_the_same() {
        let base = crate::testutil::temp_dir("skeleton-misnamed");
        let preview = base.join("preview.png");
        let jpeg = base.join("photo.jpg");
        image::RgbImage::from_pixel(1920, 1080, image::Rgb([30, 90, 200]))
            .save(&jpeg)
            .unwrap();
        std::fs::rename(&jpeg, &preview).unwrap();

        assert!(reduce_in_place(&preview).unwrap() > 0, "it gave bytes back");
        let reduced = image::ImageReader::open(&preview)
            .unwrap()
            .with_guessed_format()
            .unwrap();
        assert_eq!(
            reduced.format(),
            Some(image::ImageFormat::Jpeg),
            "still a JPEG, as the game read it"
        );
        assert_eq!(reduced.decode().unwrap().width(), IMAGE_WIDTH);
        assert!(freeze_image(&preview, &base).is_ok(), "and it can be frozen");
    }

    #[test]
    fn the_frozen_image_is_narrow_and_keeps_its_transparency() {
        let base = crate::testutil::temp_dir("skeleton-image");
        let photo = base.join("photo.png");
        image::RgbImage::from_pixel(1920, 1080, image::Rgb([200, 30, 30]))
            .save(&photo)
            .unwrap();
        let cutout = base.join("cutout.png");
        image::RgbaImage::from_pixel(1920, 1080, image::Rgba([200, 30, 30, 0]))
            .save(&cutout)
            .unwrap();
        let dir = base.join("v1");
        std::fs::create_dir_all(&dir).unwrap();

        let frozen = freeze_image(&photo, &dir).unwrap();
        assert!(
            frozen.to_string_lossy().ends_with(".jpg"),
            "an opaque photo becomes a JPEG"
        );
        let img = image::open(&frozen).unwrap();
        assert_eq!((img.width(), img.height()), (IMAGE_WIDTH, 270), "narrowed, ratio kept");
        assert!(std::fs::metadata(&frozen).unwrap().len() < 100_000, "well under 100 KB");

        let frozen = freeze_image(&cutout, &dir).unwrap();
        assert!(frozen.to_string_lossy().ends_with(".png"), "transparency needs a PNG");
        assert_eq!(image_of(&dir), Some(frozen), "one image, the last one");
        assert!(!dir.join(format!("{IMAGE_STEM}.jpg")).exists(), "the JPEG was replaced");
        assert!(
            is_kept(ModKind::Car, Path::new(".pitbox-vitrine.png")),
            "and it is part of the skeleton"
        );
    }

    /// Rule (ESPACE R5): the guard refuses a skeleton, lets a complete
    /// version through, and says so with the i18n key.
    #[test]
    fn the_guard_refuses_a_skeleton_only() {
        let base = crate::testutil::temp_dir("skeleton-guard");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_mod(&conn, "car", "Car", None, Some("Car"), "h", None, &now).unwrap();
        crate::overlay::insert_version(
            &conn,
            "v1",
            "car",
            None,
            None,
            &now,
            "cars/car/v1",
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "car", "v1").unwrap();
        assert!(guard(&conn, "v1").is_ok(), "a complete version passes");
        assert!(
            guard(&conn, "unknown").is_ok(),
            "an unknown one is its caller's business"
        );

        crate::overlay::mark_version_freed(&conn, "v1", &now, 10).unwrap();
        assert_eq!(guard(&conn, "v1").err().as_deref(), Some(crate::errors::CONTENT_FREED));
        assert_eq!(
            guard_mod(&conn, "car").err().as_deref(),
            Some(crate::errors::CONTENT_FREED)
        );
        assert!(is_showcase(&conn, "car").unwrap(), "the mod is in the showcase");
    }
}
