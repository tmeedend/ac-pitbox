//! The Package section (SPEC-story-mode, Create Your Own: Package): the
//! story's permission register (`story_engine::credits`) as the editor shows
//! and edits it, and the build of its package (`story_engine::package`).
//!
//! The registers are read here rather than with the story's other files:
//! `media/` is not a folder the engine reads, and a register says nothing
//! about how the story plays. Its errors block the build, never the play -
//! an author plays a story long before asking anyone for a permission.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use story_engine::credits::{self, Credit, Register};
use story_engine::package;
use story_engine::validate::{validate, Severity};
use story_engine::Story;

use super::{stamp, EditedFile};
use crate::errors;

/// The registers of a story and what they say.
#[derive(Debug, Serialize)]
pub struct PackageView {
    /// Every register the story has, in path order; a register a file
    /// belongs in but that does not exist yet is not listed: its first line
    /// creates it.
    pub registers: Vec<EditedFile<Vec<Credit>>>,
    pub register: Register,
}

/// The registers of the story in `folder`, read now.
pub fn package_view(folder: &Path) -> PackageView {
    let paths: BTreeSet<String> = match package::folder_paths(folder) {
        Ok(paths) => paths.into_iter().collect(),
        Err(e) => {
            log::warn!("story package: {} not listed: {e}", folder.display());
            BTreeSet::new()
        }
    };
    let mut read = BTreeMap::new();
    let mut registers = Vec::new();
    for file in paths.iter().filter(|p| credits::is_register(p)) {
        let (text, parsed) = match std::fs::read_to_string(folder.join(file)) {
            Ok(text) => {
                let parsed = credits::parse(&text);
                (text, parsed)
            }
            Err(e) => (String::new(), Err(e.to_string())),
        };
        registers.push(EditedFile {
            file: file.clone(),
            stamp: stamp(&text),
            value: parsed.as_ref().ok().cloned(),
            error: parsed.as_ref().err().cloned(),
        });
        read.insert(file.clone(), parsed);
    }
    PackageView {
        register: credits::check(&paths, &read),
        registers,
    }
}

/// Builds the story's package into `out`, and says its size. Refused while
/// the story or its register has an error: what the Package section lists.
pub fn build_package(folder: &Path, out: &Path) -> Result<u64, String> {
    let story = Story::load(folder).map_err(|_| errors::STORY_BUILD_BLOCKED.to_string())?;
    let files = package::folder_files(folder).map_err(|e| e.to_string())?;
    // The register of the very files packed, read once.
    if validate(&story).iter().any(|d| d.severity == Severity::Error) || credits::check_files(&files).blocks() {
        return Err(errors::STORY_BUILD_BLOCKED.into());
    }
    let bytes = package::pack(&story.manifest, &files)?;
    // Through a temporary file: a build stopped half-way never leaves half a
    // package under the name a player would be given.
    let tmp = out.with_extension("pitstory.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, out).map_err(|e| format!("writing {}: {e}", out.display()))?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn sample_in(dir: &Path) {
        for (file, bytes) in story_engine::sample::folder_files() {
            let path = dir.join(&file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    // The sample's register is complete: it builds, and the package
    // installs (SPEC-story-mode, Content: permission register).
    #[test]
    fn a_story_whose_register_is_complete_builds_and_installs() {
        let base = temp_dir("story-build");
        let story = base.join("story");
        sample_in(&story);
        std::fs::write(story.join("notes.txt"), "private").unwrap();
        let view = package_view(&story);
        assert_eq!(view.registers.len(), 3, "media and two skins");
        assert!(view.register.diagnostics.is_empty(), "{:#?}", view.register.diagnostics);

        let out = base.join("sample-1.pitstory");
        assert!(build_package(&story, &out).unwrap() > 0);
        let config = base.join("config");
        crate::story::packages::install(&config, &out, false).expect("the build installs");
        assert!(
            !config.join("story/installed/sample/notes.txt").exists(),
            "notes stay with the author"
        );
        assert!(
            config.join("story/installed/sample/media/credits.yaml").is_file(),
            "credits travel"
        );
    }

    // An item without permission blocks the build, never the play.
    #[test]
    fn a_file_without_permission_blocks_the_build() {
        let base = temp_dir("story-build-blocked");
        let story = base.join("story");
        sample_in(&story);
        std::fs::write(story.join("media/scenes/extra.webp"), b"x").unwrap();
        let view = package_view(&story);
        assert!(view.register.blocks());
        let extra = (view.register.coverage.iter()).find(|c| c.file == "media/scenes/extra.webp");
        assert_eq!(extra.and_then(|c| c.line), None, "listed without a credit");

        let out = base.join("x.pitstory");
        assert_eq!(build_package(&story, &out), Err(errors::STORY_BUILD_BLOCKED.into()));
        assert!(!out.exists(), "nothing written");
        assert!(Story::load(&story).is_ok(), "the story still reads and plays");
    }
}
