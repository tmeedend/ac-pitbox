//! The stories an author works on: the list of their folders in
//! `app_config_dir/story/studio.json`, a new story, a copy of the sample.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use story_engine::frame::Frame;
use story_engine::load::draft_dir;
use story_engine::validate::Severity;

use super::*;
use crate::errors;

/// The list of story folders, beside Pit Box's other settings.
const STUDIO_FILE: &str = "story/studio.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct StudioFile {
    /// In the order they were added: the hub shows them that way.
    stories: Vec<PathBuf>,
}

fn read_studio(config_dir: &Path) -> StudioFile {
    let path = config_dir.join(STUDIO_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            // Unreadable list: start empty rather than block the hub, but say
            // so - the next write would replace it.
            log::warn!("story studio: {} unreadable, starting empty: {e}", path.display());
            StudioFile::default()
        }),
        Err(_) => StudioFile::default(),
    }
}

fn write_studio(config_dir: &Path, studio: &StudioFile) -> Result<(), String> {
    let path = config_dir.join(STUDIO_FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(studio).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// The author's story folders, in the order they were added: what Story
/// Mode offers besides the sample (`play.rs`).
pub(in crate::story) fn folders(config_dir: &Path) -> Vec<PathBuf> {
    read_studio(config_dir).stories
}

fn add_to_studio(config_dir: &Path, folder: &Path) -> Result<(), String> {
    let mut studio = read_studio(config_dir);
    if !studio.stories.iter().any(|p| p == folder) {
        studio.stories.push(folder.to_path_buf());
        write_studio(config_dir, &studio)?;
    }
    Ok(())
}

/// A story of the hub's list, as far as its folder can be read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySummary {
    pub path: PathBuf,
    /// `None` when `story.yaml` does not read.
    pub title: Option<String>,
    pub chapters: usize,
    pub errors: usize,
    pub warnings: usize,
    /// The folder is gone (moved, renamed, an unplugged drive).
    pub missing: bool,
}

pub fn list(config_dir: &Path) -> Vec<StorySummary> {
    read_studio(config_dir)
        .stories
        .into_iter()
        .map(|path| {
            if !path.join("story.yaml").is_file() {
                return StorySummary {
                    path,
                    title: None,
                    chapters: 0,
                    errors: 0,
                    warnings: 0,
                    missing: true,
                };
            }
            let (title, chapters, diagnostics) = match draft_dir(&path) {
                Ok((_, d)) => {
                    let (title, chapters) = (d.manifest.as_ref().map(|m| m.title.clone()), d.chapters.len());
                    (title, chapters, verdict(&path, d))
                }
                Err(e) => (None, 0, vec![load_error(e)]),
            };
            let count = |s: Severity| diagnostics.iter().filter(|d| d.severity == s).count();
            StorySummary {
                title,
                chapters,
                errors: count(Severity::Error),
                warnings: count(Severity::Warning),
                missing: false,
                path,
            }
        })
        .collect()
}

/// Adds an existing story folder to the list.
pub fn register(config_dir: &Path, folder: &Path) -> Result<(), String> {
    if !folder.join("story.yaml").is_file() {
        return Err(errors::STORY_NOT_A_STORY.into());
    }
    add_to_studio(config_dir, folder)
}

/// Drops a story from the list. Its files are left as they are.
pub fn forget(config_dir: &Path, folder: &Path) -> Result<(), String> {
    let mut studio = read_studio(config_dir);
    studio.stories.retain(|p| p != folder);
    write_studio(config_dir, &studio)
}

/// Starts a new story in `folder`, which must be empty (or not exist yet):
/// the editor never mixes a story into files it did not write.
pub fn create(
    config_dir: &Path,
    folder: &Path,
    title: &str,
    language: &str,
    frame: Frame,
    chapter_title: &str,
) -> Result<(), String> {
    let files = story_engine::write::new_story(&story_id(folder), title, language, frame, chapter_title)?;
    let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|f| (f.path, f.text.into_bytes())).collect();
    write_new_story(config_dir, folder, &files)
}

/// Copies the sample story Pit Box carries into `folder`, which must be
/// empty, and adds it to the list: an author's own copy to read and change,
/// the way one learns how a story is made. The copy is the author's; the
/// sample Story Mode plays stays the one Pit Box carries.
pub fn copy_sample(config_dir: &Path, folder: &Path) -> Result<(), String> {
    write_new_story(config_dir, folder, &story_engine::sample::folder_files())
}

/// Writes a story's first files into `folder`, which must be empty (or not
/// exist yet), and adds it to the list.
fn write_new_story(config_dir: &Path, folder: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    if folder.exists() {
        let mut entries = std::fs::read_dir(folder).map_err(|e| e.to_string())?;
        if entries.next().is_some() {
            return Err(errors::STORY_FOLDER_NOT_EMPTY.into());
        }
    }
    for (file, bytes) in files {
        let path = folder.join(file);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    add_to_studio(config_dir, folder)
}

/// A story's id from its folder name: lower case, letters, digits and `_`.
pub(super) fn story_id(folder: &Path) -> String {
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let id: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let id = id.trim_matches('_').to_string();
    if id.is_empty() {
        "story".into()
    } else {
        id
    }
}
