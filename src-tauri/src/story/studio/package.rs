//! The Package section (SPEC-story-mode, Create Your Own: Package): the
//! story's permission register (`story_engine::credits`) as the editor shows
//! and edits it, and the build of its package (`story_engine::package`).
//!
//! The registers are read here rather than with the story's other files:
//! `media/` and `package/` are not folders the engine reads, and a register
//! says nothing about how the story plays. Its errors block the build, never
//! the play - an author plays a story long before asking anyone for a
//! permission.
//!
//! **The cars and tracks a package carries** are those the story races on
//! that are not the game's own, taken from the author's library (or game)
//! as they are: the version in play, its folder whole. One the author does
//! not have cannot be carried, and blocks the build.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::Serialize;
use story_engine::credits::{self, ContentCredit, Credit, Register, CONTENT_REGISTER};
use story_engine::needs::{needed, AcKind};
use story_engine::package::{self, ItemKind, Manifest};
use story_engine::validate::{validate, Diagnostic, Severity};
use story_engine::Story;

use super::{stamp, EditedFile};
use crate::errors;
use crate::story::holdings::{Holding, Lookup};

/// The registers of a story and what they say.
#[derive(Debug, Serialize)]
pub struct PackageView {
    /// Every register of files the story has, in path order; a register a
    /// file belongs in but that does not exist yet is not listed: its first
    /// line creates it.
    pub registers: Vec<EditedFile<Vec<Credit>>>,
    /// `package/manifest.yaml`, the register of the cars and tracks carried;
    /// empty, with an empty stamp, while the story has none.
    pub content_register: EditedFile<Vec<ContentCredit>>,
    /// Every car and track the story races on, as the author's machine
    /// holds it: those not the game's own are what the package carries.
    pub content: Vec<Holding>,
    pub register: Register,
}

/// The registers of the story in `folder`, read now.
pub fn package_view(folder: &Path, lookup: Lookup) -> PackageView {
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
        let (text, parsed) = read_text(folder, file, credits::parse);
        registers.push(edited(file, &text, &parsed));
        read.insert(file.clone(), parsed);
    }
    let mut register = credits::check(&paths, &read);
    let content_text =
        (paths.contains(CONTENT_REGISTER)).then(|| read_text(folder, CONTENT_REGISTER, credits::parse_content));
    let content = check_content(
        &mut register,
        Story::load(folder).ok().as_ref(),
        content_text.as_ref().map(|(_, parsed)| parsed),
        lookup,
    );
    let content_register = match &content_text {
        Some((text, parsed)) => edited(CONTENT_REGISTER, text, parsed),
        None => EditedFile {
            file: CONTENT_REGISTER.into(),
            stamp: String::new(),
            value: Some(Vec::new()),
            error: None,
        },
    };
    PackageView {
        registers,
        content_register,
        content,
        register,
    }
}

fn read_text<T>(folder: &Path, file: &str, parse: fn(&str) -> Result<T, String>) -> (String, Result<T, String>) {
    match std::fs::read_to_string(folder.join(file)) {
        Ok(text) => {
            let parsed = parse(&text);
            (text, parsed)
        }
        Err(e) => (String::new(), Err(e.to_string())),
    }
}

fn edited<T: Clone>(file: &str, text: &str, parsed: &Result<T, String>) -> EditedFile<T> {
    EditedFile {
        file: file.into(),
        stamp: stamp(text),
        value: parsed.as_ref().ok().cloned(),
        error: parsed.as_ref().err().cloned(),
    }
}

/// Adds to `register` the cars and tracks the package carries - those
/// `story` races on that are not the game's own - and returns how the
/// author's machine holds each it races on. One the author does not have
/// cannot be carried: an error.
fn check_content(
    register: &mut Register,
    story: Option<&Story>,
    read: Option<&Result<Vec<ContentCredit>, String>>,
    lookup: Lookup,
) -> Vec<Holding> {
    let held = story.map(|s| lookup(&needed(s))).unwrap_or_default();
    let carried: BTreeSet<(AcKind, String)> = (held.iter())
        .filter(|h| !h.official)
        .map(|h| (h.kind, h.id.clone()))
        .collect();
    credits::check_content(register, &carried, read);
    for h in held.iter().filter(|h| !h.official && h.folder.is_none()) {
        register.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: CONTENT_REGISTER.into(),
            node: None,
            message: format!(
                "{} {}: not in your library, the package cannot carry it",
                h.kind.folder(),
                h.id
            ),
        });
    }
    held
}

/// Builds the story's package into `out`, and says its size. Refused while
/// the story or its register has an error: what the Package section lists.
pub fn build_package(folder: &Path, out: &Path, lookup: Lookup) -> Result<u64, String> {
    let story = Story::load(folder).map_err(|_| errors::STORY_BUILD_BLOCKED.to_string())?;
    let files = package::folder_files(folder).map_err(|e| e.to_string())?;
    // The register of the very files packed, read once.
    let mut register = credits::check_files(&files);
    let content_read = (files.iter()).find(|(p, _)| p == CONTENT_REGISTER).map(|(_, b)| {
        std::str::from_utf8(b)
            .map_err(|e| e.to_string())
            .and_then(credits::parse_content)
    });
    let held = check_content(&mut register, Some(&story), content_read.as_ref(), lookup);
    if validate(&story).iter().any(|d| d.severity == Severity::Error) || register.blocks() {
        return Err(errors::STORY_BUILD_BLOCKED.into());
    }

    // Each car and track zipped beside the package first: its size and
    // fingerprint go in the manifest, which comes before the items.
    let parts = Parts::new(out)?;
    let story_zip = package::story_item(&files)?;
    let mut items = vec![package::item(
        ItemKind::Story,
        None,
        story_zip.len() as u64,
        package::sha256_hex(&story_zip),
    )];
    let mut readers: Vec<Box<dyn std::io::Read>> = vec![Box::new(Cursor::new(story_zip))];
    for (n, h) in held.iter().filter(|h| !h.official).enumerate() {
        let source = h.folder.as_ref().ok_or(errors::STORY_BUILD_BLOCKED)?;
        let part = parts.0.join(format!("{n}.zip"));
        let file = File::create(&part).map_err(|e| format!("writing {}: {e}", part.display()))?;
        package::content_item(source, &h.id, file)?;
        let (size, sha256) = (File::open(&part).and_then(package::sha256_of)).map_err(|e| e.to_string())?;
        items.push(package::item(h.kind.into(), Some(&h.id), size, sha256));
        readers.push(Box::new(File::open(&part).map_err(|e| e.to_string())?));
    }
    let manifest = Manifest::for_story(&story.manifest, items)?;
    // Through a temporary file: a build stopped half-way never leaves half a
    // package under the name a player would be given.
    let tmp = out.with_extension("pitstory.tmp");
    let file = File::create(&tmp).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    package::write_package(&manifest, readers, file)?;
    std::fs::rename(&tmp, out).map_err(|e| format!("writing {}: {e}", out.display()))?;
    std::fs::metadata(out).map(|m| m.len()).map_err(|e| e.to_string())
}

/// The folder the cars and tracks are zipped in during a build, beside the
/// package, removed when the build ends, whichever way.
struct Parts(PathBuf);

impl Parts {
    fn new(out: &Path) -> Result<Parts, String> {
        let dir = out.with_extension("pitstory.parts");
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Parts(dir))
    }
}

impl Drop for Parts {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            log::warn!("story package: {} not removed: {e}", self.0.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::holdings::{all_held, held_as};
    use crate::story::packages::{self, Outcome};
    use crate::testutil::temp_dir;

    fn sample_in(dir: &Path) {
        for (file, bytes) in story_engine::sample::folder_files() {
            let path = dir.join(&file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    /// The game's own content, as the sample races on: carried by no package.
    fn official(wanted: &BTreeSet<(AcKind, String)>) -> Vec<Holding> {
        let mut held = all_held(wanted);
        for h in &mut held {
            h.official = true;
        }
        held
    }

    // The sample's register is complete: it builds, and the package
    // installs (SPEC-story-mode, Content: permission register).
    #[test]
    fn a_story_whose_register_is_complete_builds_and_installs() {
        let base = temp_dir("story-build");
        let story = base.join("story");
        sample_in(&story);
        std::fs::write(story.join("notes.txt"), "private").unwrap();
        let view = package_view(&story, &official);
        assert_eq!(view.registers.len(), 3, "media and two skins");
        assert!(view.register.diagnostics.is_empty(), "{:#?}", view.register.diagnostics);
        assert_eq!(view.content.len(), 8, "every car and track the sample races on");

        let out = base.join("sample-1.pitstory");
        assert!(build_package(&story, &out, &official).unwrap() > 0);
        assert!(
            !base.join("sample-1.pitstory.parts").exists(),
            "nothing left beside the package"
        );
        let config = base.join("config");
        packages::install(&config, &out, false, &all_held).expect("the build installs");
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
        let view = package_view(&story, &official);
        assert!(view.register.blocks());
        let extra = (view.register.coverage.iter()).find(|c| c.file == "media/scenes/extra.webp");
        assert_eq!(extra.and_then(|c| c.line), None, "listed without a credit");

        let out = base.join("x.pitstory");
        assert_eq!(
            build_package(&story, &out, &official),
            Err(errors::STORY_BUILD_BLOCKED.into())
        );
        assert!(!out.exists(), "nothing written");
        assert!(Story::load(&story).is_ok(), "the story still reads and plays");
    }

    /// A mod's folder, made up: a `data/` and a picture at its root.
    fn fake_mod(dir: &Path, id: &str, engine: &str) -> PathBuf {
        let folder = dir.join(id);
        std::fs::create_dir_all(folder.join("data")).unwrap();
        std::fs::write(folder.join("data/engine.ini"), engine).unwrap();
        std::fs::write(folder.join("body_shadow.png"), b"png").unwrap();
        folder
    }

    /// The sample with its first race moved onto `mod_car`, credited as
    /// `permission`.
    fn racing_on_a_mod(story: &Path, permission: &str) {
        sample_in(story);
        let event = story.join("events/race_01.yaml");
        let text = std::fs::read_to_string(&event).unwrap();
        std::fs::write(&event, text.replace("car: abarth500", "car: mod_car")).unwrap();
        std::fs::create_dir_all(story.join("package")).unwrap();
        std::fs::write(
            story.join(CONTENT_REGISTER),
            format!("- kind: car\n  id: mod_car\n  author: A modder\n  licence: Shared\n  permission: {permission}\n"),
        )
        .unwrap();
    }

    /// The author's machine: `mod_car` in `car`, the rest the game's own.
    fn author_with(car: PathBuf) -> impl Fn(&BTreeSet<(AcKind, String)>) -> Vec<Holding> {
        move |w| {
            let mut held = held_as(w, |(_, id)| (id == "mod_car").then(|| car.clone()));
            for h in held.iter_mut().filter(|h| h.id != "mod_car") {
                (h.official, h.folder) = (true, Some(PathBuf::from("game")));
            }
            held
        }
    }

    // A package carries the cars and tracks that are not the game's own, as
    // the author has them; each needs its credit like a file
    // (SPEC-story-mode, Content).
    #[test]
    fn a_mod_the_story_races_on_travels_with_its_credit() {
        let base = temp_dir("story-build-mod");
        let story = base.join("story");
        racing_on_a_mod(&story, "requested");
        let lookup = author_with(fake_mod(&base.join("library"), "mod_car", "[ENGINE]"));

        let view = package_view(&story, &lookup);
        assert_eq!(
            (view.register.content.iter().map(|c| (c.id.as_str(), c.line))).collect::<Vec<_>>(),
            vec![("mod_car", Some(0))]
        );
        assert!(view.register.blocks(), "its permission is only requested");
        let out = base.join("p.pitstory");
        assert_eq!(
            build_package(&story, &out, &lookup),
            Err(errors::STORY_BUILD_BLOCKED.into())
        );

        racing_on_a_mod(&story, "granted");
        build_package(&story, &out, &lookup).expect("builds once granted");
        let mut zip = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let manifest = {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut zip.by_name(package::MANIFEST).unwrap(), &mut text).unwrap();
            Manifest::from_json(&text).unwrap()
        };
        let kinds: Vec<_> = manifest.items.iter().map(|i| (i.kind, i.id.clone())).collect();
        assert_eq!(
            kinds,
            vec![(ItemKind::Story, None), (ItemKind::Car, Some("mod_car".into()))]
        );

        // A mod the author does not have cannot travel.
        let without = |w: &BTreeSet<(AcKind, String)>| {
            let mut held = lookup(w);
            held.iter_mut()
                .filter(|h| h.id == "mod_car")
                .for_each(|h| h.folder = None);
            held
        };
        let view = package_view(&story, &without);
        assert!(
            (view.register.diagnostics.iter()).any(|d| d.message.contains("mod_car: not in your library")),
            "{:#?}",
            view.register.diagnostics
        );
    }

    // On the player's side: a mod they lack is set aside for the ordinary
    // import; one they have is never replaced, and a different version of
    // it is recorded (SPEC-story-mode, Content: a car the player already has).
    #[test]
    fn an_install_imports_what_is_missing_and_never_replaces_what_is_there() {
        let base = temp_dir("story-install-mod");
        let story = base.join("story");
        racing_on_a_mod(&story, "granted");
        let car = fake_mod(&base.join("author"), "mod_car", "[ENGINE]");
        let out = base.join("p.pitstory");
        build_package(&story, &out, &author_with(car.clone())).unwrap();
        let installed = |config: &Path, lookup: Lookup| match packages::install(config, &out, false, lookup).unwrap() {
            Outcome::Installed { imports, .. } => imports,
            other => panic!("installed, got {other:?}"),
        };

        // Lacking it: one archive to import, inside Pit Box's data.
        let config = base.join("lacking");
        let lacking =
            |w: &BTreeSet<(AcKind, String)>| held_as(w, |(_, id)| (id != "mod_car").then(|| PathBuf::from("x")));
        let imports = installed(&config, &lacking);
        let [archive] = imports.as_slice() else {
            panic!("one import: {imports:?}")
        };
        assert!(archive.is_file() && archive.starts_with(config.join("story/installed/.imports")));
        packages::imported(&config, archive).unwrap();
        assert!(!archive.exists(), "removed once imported");
        assert!(
            packages::imported(&config, &car.join("data/engine.ini")).is_err(),
            "only what was set aside"
        );

        // Having it, as the package has it: nothing to import, nothing said.
        let config = base.join("same");
        let theirs = fake_mod(&base.join("player_same"), "mod_car", "[ENGINE]");
        assert!(installed(&config, &|w| held_as(w, |_| Some(theirs.clone()))).is_empty());
        assert!(packages::other_versions(&config, "sample").is_empty());

        // Having another version: kept as it is, and said.
        let config = base.join("other");
        let theirs = fake_mod(&base.join("player_other"), "mod_car", "[ENGINE]\nPOWER=999");
        assert!(installed(&config, &|w| held_as(w, |_| Some(theirs.clone()))).is_empty());
        assert_eq!(
            packages::other_versions(&config, "sample"),
            vec![(AcKind::Car, "mod_car".to_string())]
        );
        assert_eq!(
            std::fs::read_to_string(theirs.join("data/engine.ini")).unwrap(),
            "[ENGINE]\nPOWER=999",
            "the player's version untouched"
        );
    }
}
