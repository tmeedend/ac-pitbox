//! The neutral sample story, carried inside Pit Box: always something to
//! play in Story Mode before stories come as packages (L4), the same story
//! the tests play, and the one an author copies into Create Your Own to see
//! how a story is made. Played from here, it has no folder: its media are
//! served from the bytes carried here (`media`), and written out by a copy.

use crate::load::SourceFile;

/// Its id, as `story.yaml` declares it.
pub const ID: &str = "sample";

// `FILES`: every file of `sample/` outside `media/`, as text; `MEDIA`: the
// files of `media/`, as bytes, with their paths inside the story folder.
// Listed by `build.rs`, so that a file added to the folder is carried.
include!(concat!(env!("OUT_DIR"), "/sample_files.rs"));

/// Every file of the sample folder, text and media, as bytes: what a copy
/// writes.
pub fn folder_files() -> Vec<(String, Vec<u8>)> {
    (FILES.iter().map(|(p, t)| (p.to_string(), t.as_bytes().to_vec())))
        .chain(MEDIA.iter().map(|(p, b)| (p.to_string(), b.to_vec())))
        .collect()
}

/// A file of the sample's `media/`, by its path inside that folder
/// (`portraits/friend_happy.png`), as Story Mode shows it.
pub fn media(path: &str) -> Option<&'static [u8]> {
    MEDIA
        .iter()
        .find(|(p, _)| p.strip_prefix("media/") == Some(path))
        .map(|(_, bytes)| *bytes)
}

/// The sample's files, as `load::read_dir` would read them from its folder.
pub fn files() -> Vec<SourceFile> {
    FILES.iter().map(|(path, text)| SourceFile::new(path, text)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::read_dir;

    // What `build.rs` carries is the folder as the engine reads it: a file
    // carried but never read (or read but not carried) would make the
    // played sample differ from the tested one.
    #[test]
    fn the_embedded_sample_is_the_sample_folder() {
        let folder = read_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).expect("reads");
        let mut on_disk: Vec<_> = folder.iter().map(|f| (f.path.clone(), f.text.clone())).collect();
        let mut embedded: Vec<_> = files().into_iter().map(|f| (f.path, f.text)).collect();
        on_disk.sort();
        embedded.sort();
        assert_eq!(embedded, on_disk, "every file of sample/, as it is on disk");
        let mut media: Vec<String> =
            crate::load::read_media(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sample"))
                .into_iter()
                .map(|p| format!("media/{p}"))
                .collect();
        media.sort();
        let carried: Vec<String> = MEDIA.iter().map(|(p, _)| p.to_string()).collect();
        assert_eq!(carried, media, "every media file too, for the copy");
        let story = crate::Story::from_sources(files()).expect("loads");
        assert_eq!(story.manifest.id, ID);
    }

    // Story Mode shows the sample's pictures by their path in `media/`, the
    // one a portrait or an illustration names - and nothing outside it.
    #[test]
    fn a_media_file_is_found_by_its_path_inside_media() {
        assert!(
            media("portraits/friend_happy.png").is_some(),
            "a portrait, as a character names it"
        );
        assert!(
            media("media/portraits/friend_happy.png").is_none(),
            "the path is inside media/"
        );
        assert!(media("story.yaml").is_none(), "a text file is not a media file");
        assert!(media("portraits/nobody.png").is_none(), "nothing for a missing file");
    }
}
