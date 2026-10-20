//! The neutral sample story, carried inside Pit Box: always something to
//! play in Story Mode before stories come as packages (L4), and the same
//! story the tests play. Its text files only - its media (portraits) stay in
//! the repository, so it plays with initials where a picture would be.

use crate::load::SourceFile;

/// Its id, as `story.yaml` declares it.
pub const ID: &str = "sample";

const FILES: &[(&str, &str)] = &[
    (
        "content/friend/invite.story",
        include_str!("../sample/content/friend/invite.story"),
    ),
    (
        "content/friend/talk.story",
        include_str!("../sample/content/friend/talk.story"),
    ),
    (
        "content/parent/broke.story",
        include_str!("../sample/content/parent/broke.story"),
    ),
    (
        "content/pools/fail.story",
        include_str!("../sample/content/pools/fail.story"),
    ),
    (
        "content/rival/posts.story",
        include_str!("../sample/content/rival/posts.story"),
    ),
    (
        "content/sponsor/offer.story",
        include_str!("../sample/content/sponsor/offer.story"),
    ),
    ("drivers/roster.yaml", include_str!("../sample/drivers/roster.yaml")),
    ("events/race_01.yaml", include_str!("../sample/events/race_01.yaml")),
    ("events/race_02.yaml", include_str!("../sample/events/race_02.yaml")),
    (
        "events/track_day_01.yaml",
        include_str!("../sample/events/track_day_01.yaml"),
    ),
    (
        "flow/01-first-race.yaml",
        include_str!("../sample/flow/01-first-race.yaml"),
    ),
    (
        "flow/02-second-race.yaml",
        include_str!("../sample/flow/02-second-race.yaml"),
    ),
    ("story.yaml", include_str!("../sample/story.yaml")),
    ("world/characters.yaml", include_str!("../sample/world/characters.yaml")),
    ("world/variables.yaml", include_str!("../sample/world/variables.yaml")),
];

/// The sample's files, as `load::read_dir` would read them from its folder.
pub fn files() -> Vec<SourceFile> {
    FILES.iter().map(|(path, text)| SourceFile::new(path, text)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::read_dir;

    // A file added to the sample folder and forgotten here would make the
    // played sample differ from the tested one.
    #[test]
    fn the_embedded_sample_is_the_sample_folder() {
        let folder = read_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).expect("reads");
        let mut on_disk: Vec<_> = folder.iter().map(|f| (f.path.clone(), f.text.clone())).collect();
        let mut embedded: Vec<_> = files().into_iter().map(|f| (f.path, f.text)).collect();
        on_disk.sort();
        embedded.sort();
        assert_eq!(embedded, on_disk, "every file of sample/, as it is on disk");
        let story = crate::Story::from_sources(files()).expect("loads");
        assert_eq!(story.manifest.id, ID);
    }
}
