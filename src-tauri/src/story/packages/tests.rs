use super::*;
use crate::story::holdings::all_held;
use crate::testutil::temp_dir;
use std::io::{Cursor, Write};

/// The sample's files, `edit` applied to the one at `path`.
fn sample_with(path: &str, edit: impl Fn(&str) -> String) -> Vec<(String, Vec<u8>)> {
    let mut files = story_engine::sample::folder_files();
    let file = files.iter_mut().find(|(p, _)| p == path).expect("a file of the sample");
    file.1 = edit(std::str::from_utf8(&file.1).unwrap()).into_bytes();
    files
}

fn version(v: &str) -> impl Fn(&str) -> String + '_ {
    move |t| t.replace("version: \"1\"", &format!("version: \"{v}\""))
}

/// A `.pitstory` of `files`, written under `dir`.
fn packed(dir: &Path, name: &str, files: &[(String, Vec<u8>)]) -> PathBuf {
    let sources = (files.iter())
        .filter(|(p, _)| p.ends_with(".yaml") || p.ends_with(".story"))
        .map(|(p, b)| story_engine::load::SourceFile::new(p, std::str::from_utf8(b).unwrap()))
        .collect();
    let manifest = Story::from_sources(sources).expect("reads").manifest;
    let path = dir.join(name);
    std::fs::write(&path, package::pack(&manifest, files).unwrap()).unwrap();
    path
}

/// A `.pitstory` written by hand: a manifest and items as given, for
/// what `package::pack` never makes.
fn by_hand(dir: &Path, manifest: &str, items: &[(&str, &[u8])]) -> PathBuf {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(package::MANIFEST, options).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    for (name, bytes) in items {
        zip.start_file(format!("{}{name}", package::ITEMS), options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    let path = dir.join("by_hand.pitstory");
    std::fs::write(&path, zip.finish().unwrap().into_inner()).unwrap();
    path
}

fn one_item_manifest(item: &[u8]) -> String {
    format!(
        r#"{{"format":1,"id":"sample","title":"Sample story","version":"1",
            "items":[{{"kind":"story","file":"s.zip","size":{},"sha256":"{}"}}]}}"#,
        item.len(),
        package::sha256_hex(item)
    )
}

fn installed(id: &str, version: &str, previous: Option<&str>) -> Outcome {
    Outcome::Installed {
        id: id.into(),
        title: "Sample story".into(),
        version: version.into(),
        previous: previous.map(String::from),
        imports: Vec::new(),
    }
}

// One action installs the story, and Story Mode offers it beside the
// sample, from Pit Box's own data (SPEC-story-mode, Story Mode: player
// side, stories offered).
#[test]
fn a_package_installs_and_plays() {
    let base = temp_dir("story-pkg-install");
    let config = base.join("config");
    let file = packed(&base, "sample.pitstory", &story_engine::sample::folder_files());

    assert_eq!(
        install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true).unwrap(),
        installed("sample", "1", None)
    );
    let dir = config.join("story/installed/sample");
    assert!(
        dir.join("media/scenes/bedroom.webp").is_file(),
        "media unpacked with the story"
    );
    assert!(
        config.join("story/installed/sample.package.json").is_file(),
        "the manifest kept"
    );
    let leftovers: Vec<_> = (std::fs::read_dir(config.join("story/installed")).unwrap().flatten())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".staging") || n.starts_with(".old"))
        .collect();
    assert!(leftovers.is_empty(), "nothing left of the staging: {leftovers:?}");
    let cached: Vec<_> = (std::fs::read_dir(config.join("story/installed/.cache/sample"))
        .unwrap()
        .flatten())
    .map(|e| e.file_name().to_string_lossy().into_owned())
    .collect();
    assert!(
        matches!(cached.as_slice(), [one] if one.starts_with("story-")),
        "the story's item kept for an update: {cached:?}"
    );

    let source = Source::Installed { id: "sample".into() };
    let listed = play::list(&config, &all_held);
    let p = listed.iter().find(|p| p.source == source).expect("offered");
    assert_eq!((p.errors, p.version.as_deref()), (0, Some("1")));
    let game = play::new_game(&config, &source, 7).expect("starts");
    assert_eq!(
        game.folder.as_deref(),
        Some(dir.as_path()),
        "media read from the installed folder"
    );
    assert!(
        config.join("story/saves/installed/sample.json").is_file(),
        "its own save"
    );
}

// An update keeps the game that resumes on it; one that would lose it
// asks first and changes nothing until told to (SPEC-story-mode,
// Content: installation and updates).
#[test]
fn an_update_keeps_the_game_or_asks_before_losing_it() {
    let base = temp_dir("story-pkg-update");
    let config = base.join("config");
    install(
        &config,
        Origin::File(&packed(&base, "v1.pitstory", &story_engine::sample::folder_files())),
        false,
        &all_held,
        &mut |_, _| true,
    )
    .unwrap();
    let source = Source::Installed { id: "sample".into() };
    play::new_game(&config, &source, 7).unwrap();

    let v2 = packed(&base, "v2.pitstory", &sample_with("story.yaml", version("2")));
    assert_eq!(
        install(&config, Origin::File(&v2), false, &all_held, &mut |_, _| true).unwrap(),
        installed("sample", "2", Some("1"))
    );
    play::resume(&config, &source).expect("the game resumes on the update");

    // The game waits on `invite`, which version 3 renames.
    let mut v3 = sample_with("story.yaml", version("3"));
    let flow = "flow/01-first-race.yaml";
    let renamed = sample_with(flow, |t| t.replace("  - id: invite\n", "  - id: invite_again\n"));
    let at = v3.iter().position(|(p, _)| p == flow).unwrap();
    v3[at] = renamed.into_iter().find(|(p, _)| p == flow).unwrap();
    let v3 = packed(&base, "v3.pitstory", &v3);
    match install(&config, Origin::File(&v3), false, &all_held, &mut |_, _| true).unwrap() {
        Outcome::LosesGame { previous, version, .. } => {
            assert_eq!((previous.as_str(), version.as_str()), ("2", "3"));
        }
        other => panic!("asked first, got {other:?}"),
    }
    let listed = play::list(&config, &all_held);
    let p = listed.iter().find(|p| p.source == source).unwrap();
    assert_eq!(p.version.as_deref(), Some("2"), "nothing changed before the answer");
    play::resume(&config, &source).expect("the game still resumes");

    assert_eq!(
        install(&config, Origin::File(&v3), true, &all_held, &mut |_, _| true).unwrap(),
        installed("sample", "3", Some("2"))
    );
    let listed = play::list(&config, &all_held);
    let p = listed.iter().find(|p| p.source == source).unwrap();
    assert_eq!(p.version.as_deref(), Some("3"));
    let game = p.game.as_ref().expect("the game is kept");
    assert_eq!(
        game.problem.as_deref(),
        Some(errors::STORY_SAVE_STALE),
        "and says why it cannot resume"
    );
}

// Files are checked after download (SPEC-story-mode, Content): a
// package whose item is not what the manifest says installs nothing.
#[test]
fn a_damaged_package_installs_nothing() {
    let base = temp_dir("story-pkg-damaged");
    let config = base.join("config");
    let item = package::story_item(&story_engine::sample::folder_files()).unwrap();
    let mut bad = item.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0xff;
    let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &bad)]);
    assert_eq!(
        install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true),
        Err(errors::STORY_PACKAGE_DAMAGED.into())
    );
    let file = by_hand(&base, &one_item_manifest(&item), &[("other.zip", &item)]);
    assert_eq!(
        install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true),
        Err(errors::STORY_PACKAGE_DAMAGED.into()),
        "item missing"
    );
    assert!(
        play::list(&config, &all_held)
            .iter()
            .all(|p| p.source == Source::Sample),
        "nothing offered"
    );

    let ok = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
    assert_eq!(
        install(&config, Origin::File(&ok), false, &all_held, &mut |_, _| true).unwrap(),
        installed("sample", "1", None),
        "the same, whole"
    );
}

// A package can only bring a story's own files: a path leaving the
// folder, or a file a story is not made of, refuses all of it.
#[test]
fn a_package_writes_nothing_outside_the_story() {
    let base = temp_dir("story-pkg-escape");
    let config = base.join("config");
    for evil in ["../../evil.txt", "notes.txt", "media/../../evil.txt"] {
        let mut files = story_engine::sample::folder_files();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        files.push((evil.into(), b"x".to_vec()));
        for (p, b) in &files {
            zip.start_file(p.as_str(), options).unwrap();
            zip.write_all(b).unwrap();
        }
        let item = zip.finish().unwrap().into_inner();
        let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
        assert_eq!(
            install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true),
            Err(errors::STORY_PACKAGE_DAMAGED.into()),
            "{evil}"
        );
        assert!(
            !config.join("story/installed/sample").exists(),
            "{evil}: nothing installed"
        );
        assert!(
            !config.join("evil.txt").exists() && !base.join("evil.txt").exists(),
            "{evil}"
        );
    }
}

#[test]
fn a_package_from_a_newer_pit_box_says_so() {
    let base = temp_dir("story-pkg-newer");
    let config = base.join("config");
    let newer = r#"{"format":2,"id":"sample","something":"new"}"#;
    assert_eq!(
        install(
            &config,
            Origin::File(&by_hand(&base, newer, &[])),
            false,
            &all_held,
            &mut |_, _| true
        ),
        Err(errors::STORY_PACKAGE_TOO_NEW.into())
    );
    let item = package::story_item(&story_engine::sample::folder_files()).unwrap();
    let unknown_kind = one_item_manifest(&item).replace(r#""kind":"story""#, r#""kind":"livery""#);
    assert_eq!(
        install(
            &config,
            Origin::File(&by_hand(&base, &unknown_kind, &[("s.zip", &item)])),
            false,
            &all_held,
            &mut |_, _| true
        ),
        Err(errors::STORY_PACKAGE_TOO_NEW.into()),
        "an item kind unknown here"
    );
    let not_a_package = base.join("a.pitstory");
    std::fs::write(&not_a_package, b"hello").unwrap();
    assert_eq!(
        install(&config, Origin::File(&not_a_package), false, &all_held, &mut |_, _| {
            true
        }),
        Err(errors::STORY_PACKAGE_UNREADABLE.into())
    );
}

// A story the checks find errors in would stop half-way: it is not
// installed (SPEC-story-mode, Create Your Own: errors block the build).
#[test]
fn a_story_with_errors_is_not_installed() {
    let base = temp_dir("story-pkg-errors");
    let config = base.join("config");
    let files = sample_with("story.yaml", |t| t.replace("node: intro", "node: nowhere"));
    let item = package::story_item(&files).unwrap();
    let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
    assert_eq!(
        install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true),
        Err(errors::STORY_PACKAGE_HAS_ERRORS.into())
    );
    assert!(!config.join("story/installed/sample").exists());
}

// Uninstalling leaves the game: installed again, the story resumes it.
#[test]
fn uninstalling_keeps_the_game_for_a_new_install() {
    let base = temp_dir("story-pkg-uninstall");
    let config = base.join("config");
    let file = packed(&base, "sample.pitstory", &story_engine::sample::folder_files());
    install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true).unwrap();
    let source = Source::Installed { id: "sample".into() };
    play::new_game(&config, &source, 7).unwrap();

    uninstall(&config, "sample").unwrap();
    assert!(!config.join("story/installed/sample").exists(), "the story is gone");
    assert!(
        !config.join("story/installed/sample.package.json").exists(),
        "its manifest too"
    );
    assert!(
        play::list(&config, &all_held).iter().all(|p| p.source != source),
        "no longer offered"
    );
    assert!(uninstall(&config, "../config").is_err(), "an id is never a path");

    install(&config, Origin::File(&file), false, &all_held, &mut |_, _| true).unwrap();
    play::resume(&config, &source).expect("the game comes back with the story");
}

// A crash between moving the old version aside and putting the new one
// in place leaves the story out of its folder: it comes back.
#[test]
fn a_swap_stopped_half_way_is_put_right() {
    let base = temp_dir("story-pkg-recover");
    let config = base.join("config");
    install(
        &config,
        Origin::File(&packed(&base, "s.pitstory", &story_engine::sample::folder_files())),
        false,
        &all_held,
        &mut |_, _| true,
    )
    .unwrap();
    let root = config.join("story/installed");
    std::fs::rename(root.join("sample"), root.join(".old-sample")).unwrap();
    std::fs::create_dir_all(root.join(".staging-sample")).unwrap();

    assert_eq!(ids(&config), vec!["sample".to_string()], "the old version is back");
    assert!(!root.join(".old-sample").exists() && !root.join(".staging-sample").exists());
}
