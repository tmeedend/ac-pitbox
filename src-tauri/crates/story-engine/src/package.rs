//! A story package (SPEC-story-mode, Content: story package and
//! dependencies): what a story is cut into on its way to a player.
//!
//! **A manifest and one zip per item.** The manifest (`manifest.json`) names
//! each item with its size and SHA-256; an item is a zip of its own: the
//! story's own files, and each car and track the package carries, as its
//! folder (`<id>/...`, an archive the ordinary import reads). Cut this way, an update downloads the items whose fingerprint changed
//! (L4.4) and a new player the whole list - the requirements of the spec - and
//! every item is checked against the manifest before anything lands.
//!
//! **A `.pitstory` is that set in one file**: the manifest and the items
//! under `items/`, all stored, for a story shared by hand rather than from a
//! URL. The same installer reads both.
//!
//! **An item's zip is reproducible**: entries in path order, one fixed date,
//! the same compression. An item whose files did not change keeps its
//! fingerprint from one build to the next - which is what lets an update skip
//! it. The file name carries the fingerprint, so two versions of an item
//! never share a name on a host.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::needs::AcKind;

/// The container's format: an older Pit Box refuses a newer one rather than
/// half-reading it.
pub const FORMAT: u32 = 1;
/// The manifest, at the root of a `.pitstory`.
pub const MANIFEST: &str = "manifest.json";
/// Where the items are, in a `.pitstory`.
pub const ITEMS: &str = "items/";
/// A package file's extension.
pub const EXTENSION: &str = "pitstory";

/// What a story is made of, by its first segment: `story.yaml`, the folders
/// the engine reads, and `media/`. Anything else in the author's folder - a
/// `.git`, notes, a draft - stays out of the package, and a package carrying
/// anything else is refused.
const CARRIED: [&str; 10] = [
    "story.yaml",
    "flow",
    "content",
    "events",
    "world",
    "drivers",
    "scenes",
    "frames",
    "media",
    // The register of the cars and tracks it carries (`credits.rs`).
    "package",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    /// The story's id, as its `story.yaml` says: what an update is matched by.
    pub id: String,
    pub title: String,
    pub version: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub kind: ItemKind,
    /// A car's or a track's folder id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The item's file, beside the manifest (under `items/` in a `.pitstory`).
    pub file: String,
    pub size: u64,
    /// Lower-case hex.
    pub sha256: String,
    /// A car's or a track's folder as `folder_signature` reads it: what tells,
    /// without downloading the item, whether the player's copy is the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// The story's own files.
    Story,
    Car,
    Track,
    /// A kind this Pit Box does not know: the package is a newer one's.
    #[serde(other)]
    Unknown,
}

impl ItemKind {
    /// The game content it is, for a car or a track.
    pub fn content(self) -> Option<AcKind> {
        match self {
            ItemKind::Car => Some(AcKind::Car),
            ItemKind::Track => Some(AcKind::Track),
            ItemKind::Story | ItemKind::Unknown => None,
        }
    }
}

impl From<AcKind> for ItemKind {
    fn from(kind: AcKind) -> ItemKind {
        match kind {
            AcKind::Car => ItemKind::Car,
            AcKind::Track => ItemKind::Track,
        }
    }
}

impl Manifest {
    /// The manifest of `story`'s package, its items given.
    pub fn for_story(story: &crate::model::Manifest, items: Vec<Item>) -> Result<Manifest, String> {
        if !valid_id(&story.id) {
            return Err(format!(
                "story id `{}`: a package needs letters, digits, `_` or `-` only",
                story.id
            ));
        }
        Ok(Manifest {
            format: FORMAT,
            id: story.id.clone(),
            title: story.title.clone(),
            version: story.version.clone(),
            items,
        })
    }

    pub fn from_json(text: &str) -> Result<Manifest, String> {
        serde_json::from_str(text).map_err(|e| format!("{MANIFEST}: {e}"))
    }
}

/// An id a package may carry: it names a folder on the player's machine, so
/// letters, digits, `_` and `-` only, and never one of the names Windows
/// keeps for its devices.
pub fn valid_id(id: &str) -> bool {
    const DEVICES: [&str; 4] = ["con", "prn", "aux", "nul"];
    let lower = id.to_ascii_lowercase();
    let device = DEVICES.contains(&lower.as_str())
        || ((lower.starts_with("com") || lower.starts_with("lpt"))
            && lower.len() == 4
            && lower.as_bytes()[3].is_ascii_digit());
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !device
}

/// Whether `path`, `/` between folders, is a file a story package carries:
/// inside one of the story's parts, with no segment that could leave the
/// folder it lands in or that is hidden.
pub fn carried(path: &str) -> bool {
    let segments: Vec<&str> = path.split('/').collect();
    CARRIED.contains(&segments[0])
        && (segments.len() == 1) == (segments[0] == "story.yaml")
        // `..` starts with a dot too.
        && (segments.iter()).all(|s| !s.is_empty() && !s.starts_with('.') && !s.contains(['\\', ':']))
}

/// The files of a story folder a package carries, as paths with `/`, in
/// path order.
pub fn folder_paths(root: &Path) -> std::io::Result<Vec<String>> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                walk(&entry.path(), &format!("{path}/"), out)?;
            } else if carried(&path) {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, "", &mut out)?;
    out.sort();
    Ok(out)
}

/// The files of a story folder a package carries, with their bytes, in
/// path order.
pub fn folder_files(root: &Path) -> std::io::Result<Vec<(String, Vec<u8>)>> {
    (folder_paths(root)?.into_iter())
        .map(|p| std::fs::read(root.join(&p)).map(|bytes| (p, bytes)))
        .collect()
}

/// The SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The size and SHA-256 of what `reader` holds, read to its end.
pub fn sha256_of(mut reader: impl Read) -> std::io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let size = std::io::copy(&mut reader, &mut hasher)?;
    Ok((size, hex(&hasher.finalize())))
}

/// An item, its file named after its kind, its id and its fingerprint.
pub fn item(kind: ItemKind, id: Option<&str>, size: u64, sha256: String) -> Item {
    let name = match (kind, id) {
        (ItemKind::Car, Some(id)) => format!("car-{id}"),
        (ItemKind::Track, Some(id)) => format!("track-{id}"),
        _ => "story".into(),
    };
    Item {
        kind,
        id: id.map(String::from),
        file: format!("{name}-{}.zip", &sha256[..16]),
        size,
        sha256,
        signature: None,
    }
}

/// A mod's folder in short: the path (lower case, `/`) and the size of each
/// of its files. Two copies with the same signature are the same as far as
/// a player can tell without reading every byte - which is all the hub says
/// of a copy the player has ("another version is played").
pub fn folder_signature(root: &Path) -> std::io::Result<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel = (rel.components())
                    .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((rel, entry.metadata()?.len()));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for (rel, size) in files {
        hasher.update(format!("{rel}|{size}\n").as_bytes());
    }
    Ok(hex(&hasher.finalize()))
}

pub fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// How an item's entries are written: the same compression and one fixed
/// date, so the same files make the same bytes.
fn reproducible(method: zip::CompressionMethod, size: u64) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644)
        .large_file(size >= u64::from(u32::MAX))
}

/// The story's own files as an item's zip, reproducibly (see the module).
/// Files outside what a package carries are left out.
pub fn story_item(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let sorted: BTreeMap<&str, &[u8]> = (files.iter())
        .filter(|(p, _)| carried(p))
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in sorted {
        let options = reproducible(zip::CompressionMethod::Deflated, bytes.len() as u64);
        zip.start_file(path, options).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

/// A car's or a track's folder as an item's zip, written to `w`: every file
/// of `root`, whatever it is - nothing inside a mod's folder is sorted out
/// (CLAUDE.md, golden rule 3) - under `<id>/`, in path order, reproducibly.
pub fn content_item<W: Write + Seek>(root: &Path, id: &str, w: W) -> Result<W, String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                walk(&entry.path(), &format!("{path}/"), out)?;
            } else {
                out.push((path, entry.path()));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, &format!("{id}/"), &mut files).map_err(|e| format!("{}: {e}", root.display()))?;
    files.sort();
    let mut zip = zip::ZipWriter::new(w);
    for (name, path) in files {
        let mut file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        zip.start_file(name, reproducible(zip::CompressionMethod::Deflated, size))
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut zip).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    zip.finish().map_err(|e| e.to_string())
}

/// A `.pitstory` written to `w`: the manifest, then each of its items, in
/// the manifest's order, read from `items`.
pub fn write_package<W: Write + Seek>(manifest: &Manifest, items: Vec<Box<dyn Read + '_>>, w: W) -> Result<W, String> {
    if items.len() != manifest.items.len() {
        return Err(format!(
            "{} items for {} in the manifest",
            items.len(),
            manifest.items.len()
        ));
    }
    let mut zip = zip::ZipWriter::new(w);
    let json = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    zip.start_file(MANIFEST, reproducible(zip::CompressionMethod::Stored, 0))
        .map_err(|e| e.to_string())?;
    zip.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
    for (item, mut reader) in manifest.items.iter().zip(items) {
        let options = reproducible(zip::CompressionMethod::Stored, item.size);
        zip.start_file(format!("{ITEMS}{}", item.file), options)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut reader, &mut zip).map_err(|e| format!("{}: {e}", item.file))?;
    }
    zip.finish().map_err(|e| e.to_string())
}

/// A `.pitstory` of a story whose files are `files`, in memory, with the
/// story as its one item: what `story-tool` builds, having no library to take
/// cars and tracks from. The caller checks the story first: a story with
/// errors is never packed.
pub fn pack(manifest: &crate::model::Manifest, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let story = story_item(files)?;
    let item = item(ItemKind::Story, None, story.len() as u64, sha256_hex(&story));
    let built = Manifest::for_story(manifest, vec![item])?;
    let out = write_package(&built, vec![Box::new(story.as_slice())], Cursor::new(Vec::new()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Story;

    fn sample_manifest() -> crate::model::Manifest {
        Story::from_sources(crate::sample::files())
            .expect("the sample reads")
            .manifest
    }

    // An update downloads only the items whose fingerprint changed: an item
    // built twice from the same files must be the same bytes (module doc).
    #[test]
    fn an_item_built_twice_from_the_same_files_is_the_same() {
        let mut files = crate::sample::folder_files();
        let first = story_item(&files).unwrap();
        files.reverse();
        assert_eq!(first, story_item(&files).unwrap(), "order of the files does not matter");
        files[0].1.push(b'\n');
        assert_ne!(first, story_item(&files).unwrap(), "a changed file changes the item");
    }

    // Only what a story is made of travels; a path that could leave the
    // folder it lands in never does (SPEC-story-mode, Content).
    #[test]
    fn a_package_carries_the_story_and_nothing_else() {
        for ok in [
            "story.yaml",
            "flow/01.yaml",
            "media/scenes/a b.webp",
            "frames/skins/x/Inter.ttf",
        ] {
            assert!(carried(ok), "{ok} is carried");
        }
        for not in [
            ".git/config",
            "notes.txt",
            "story.yaml/x",
            "media",
            "media/../story.yaml",
            "media/.hidden.png",
            "media//a.png",
            "media/a\\..\\b.png",
            "media/c:x.png",
            "/media/a.png",
        ] {
            assert!(!carried(not), "{not} is not carried");
        }
    }

    #[test]
    fn a_package_id_names_a_folder_safely() {
        for ok in ["sample", "race_01", "my-story", "Com10"] {
            assert!(valid_id(ok), "{ok} accepted");
        }
        for not in ["", "a b", "../x", "é", "con", "NUL", "com1", "lpt9", &"x".repeat(65)] {
            assert!(!valid_id(not), "{not} refused");
        }
    }

    // The manifest names its item with the fingerprint the item has.
    #[test]
    fn a_pitstory_holds_its_manifest_and_its_checked_item() {
        let files = crate::sample::folder_files();
        let bytes = pack(&sample_manifest(), &files).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let manifest = {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(MANIFEST).unwrap(), &mut text).unwrap();
            Manifest::from_json(&text).unwrap()
        };
        assert_eq!((manifest.format, manifest.id.as_str()), (FORMAT, "sample"));
        let [item] = manifest.items.as_slice() else {
            panic!("one item: {:?}", manifest.items)
        };
        let mut story = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name(&format!("{ITEMS}{}", item.file)).unwrap(), &mut story).unwrap();
        assert_eq!(story.len() as u64, item.size, "size as declared");
        assert_eq!(sha256_hex(&story), item.sha256, "fingerprint as declared");
        assert!(
            item.file.contains(&item.sha256[..16]),
            "the name carries the fingerprint"
        );
    }

    // A package from a newer Pit Box is told apart from a damaged one.
    #[test]
    fn an_unknown_item_kind_still_reads() {
        let text = r#"{"format":1,"id":"a","title":"A","version":"1",
            "items":[{"kind":"livery","file":"x.zip","size":1,"sha256":"00"}]}"#;
        assert_eq!(Manifest::from_json(text).unwrap().items[0].kind, ItemKind::Unknown);
        assert!(
            Manifest::from_json(r#"{"format":1}"#).is_err(),
            "an incomplete manifest does not read"
        );
    }

    #[test]
    fn an_item_is_named_after_its_kind_its_id_and_its_fingerprint() {
        let named = item(ItemKind::Car, Some("my_car"), 3, "ab".repeat(32));
        assert_eq!(named.file, format!("car-my_car-{}.zip", "ab".repeat(8)));
        assert_eq!(
            item(ItemKind::Story, None, 3, "cd".repeat(32)).file,
            format!("story-{}.zip", "cd".repeat(8))
        );
    }

    #[test]
    fn an_id_unfit_for_a_folder_is_not_packed() {
        let mut manifest = sample_manifest();
        manifest.id = "my story".into();
        assert!(pack(&manifest, &crate::sample::folder_files()).is_err());
    }
}
