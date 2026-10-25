//! Updates of the stories installed from a URL (SPEC-story-mode, Content:
//! installation and updates). The manifest is read again where the story
//! came from; a version or an item that changed is an update, and its size
//! is what it would download: the story's item unless the cache holds it,
//! and the cars and tracks the player lacks. Installing it is an ordinary
//! install from the same URL.

use std::path::Path;

use serde::Serialize;
use story_engine::package::{Item, ItemKind};

use super::{cache_dir, content, fetch, installed_manifest, root, source_url};
use crate::story::holdings::Lookup;

/// An update a story installed from a URL can take.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Update {
    pub id: String,
    pub version: String,
    /// What it would download, in bytes.
    pub size: u64,
}

/// The updates of the installed stories, one request each. A host that does
/// not answer leaves its story without an update, and the log says why.
pub fn updates(config_dir: &Path, lookup: Lookup) -> Vec<Update> {
    (super::ids(config_dir).iter())
        .filter_map(|id| update_of(config_dir, id, lookup))
        .collect()
}

/// The URL the installed story `id` updates from, if it came from one.
pub fn update_url(config_dir: &Path, id: &str) -> Option<String> {
    source_url(&root(config_dir), id)
}

fn update_of(config_dir: &Path, id: &str, lookup: Lookup) -> Option<Update> {
    let root = root(config_dir);
    let url = source_url(&root, id)?;
    let latest = match fetch::manifest_at(&url) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("story package: no update read for {id} at {url}: {e}");
            return None;
        }
    };
    if latest.id != id {
        log::warn!("story package: {url} now holds the story {}, not {id}", latest.id);
        return None;
    }
    let installed = installed_manifest(&root, id);
    if installed.is_some_and(|m| m.version == latest.version && m.items == latest.items) {
        return None;
    }
    let (stories, cars): (Vec<&Item>, Vec<&Item>) = latest.items.iter().partition(|i| i.kind == ItemKind::Story);
    let story_size: u64 = (stories.iter())
        .filter(|i| !cache_dir(&root, id).join(&i.file).is_file())
        .map(|i| i.size)
        .sum();
    let plan = content::plan(&cars, lookup);
    let content_size: u64 = (plan.fetch.iter().chain(&plan.layers).chain(&plan.versions))
        .map(|i| i.size)
        .sum();
    Some(Update {
        id: id.to_string(),
        version: latest.version,
        size: story_size + content_size,
    })
}
