//! The picture of a track the library does not hold (SPEC-play-online.md,
//! "Vignette d'un circuit non installé").
//!
//! The library comes first, and is the page's own business (`looks.rs`). For
//! the others, in this order:
//!
//! 1. ~~the server's or CUP's picture~~ - neither exists: CUP's entries carry
//!    no image, and the `loadingImageUrl` of a server's details is the poster
//!    of the community running it, shared across tracks (measured 2026-10-15:
//!    53 different images on 141 servers, 11 of them on several tracks);
//! 2. **Wikimedia Commons**, through the Wikipedia pairing of tracks: the
//!    shipped link of the track id (`wiki-links.json`), else a match by the
//!    coordinates and the name CSP's table gives the id
//!    (`sun::track_location`). The entity's image (`P18`) is kept only when
//!    its author and licence are known (`WikiClient::commons_credit`);
//! 3. the page draws the rest: the category's pictogram and the country's
//!    flag. The country comes from here too (`P17`), when Wikidata knows the
//!    track - a server's own country is not its track's.
//!
//! **Never another track's image**: nothing here is chosen by resemblance,
//! only by the track's own id, coordinates or entity.
//!
//! The answers are kept in the thumbnail cache, the picture beside them:
//! asked once, shown at every visit. A track nobody found a picture for is
//! asked again after a while - Wikipedia grows. An unreachable network is not
//! an answer, and nothing is kept from it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::wiki::api::{Fetched, WikiClient};
use crate::wiki::clean::Cleaner;
use crate::wiki::matching::{MatchOutcome, Thresholds};
use crate::wiki::matchtrack::{self, TrackSubject};

/// What the page shows of a track it does not hold.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPicture {
    /// The thumbnail, in the cache (an absolute path, for `previewSrc`).
    pub file: Option<String>,
    /// Author and licence, as Commons records them - the credit shown with
    /// the picture, wherever it is.
    pub artist: Option<String>,
    pub licence: Option<String>,
    /// The file's page on Commons, where the full credit lives.
    pub page: Option<String>,
    /// ISO 3166-1 alpha-2 of the track's country: the drawn thumbnail's flag.
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    picture: TrackPicture,
    /// Unix seconds.
    checked_at: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    tracks: BTreeMap<String, Entry>,
}

const INDEX_FILE: &str = "online_track_pictures.json";
/// A track without a picture is asked again after thirty days.
const RETRY_EMPTY_SECS: i64 = 30 * 24 * 3600;
/// A Commons thumbnail is a few hundred KiB; anything far bigger is not one.
const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
const DOWNLOAD_TIMEOUT_MS: i32 = 15_000;

/// What the matching needs besides the network.
pub struct Sources<'a> {
    pub ac_install: Option<&'a Path>,
    /// Track id (lowercase) → Q-id: the shipped links (`wiki-links.json`).
    pub curated: &'a BTreeMap<String, String>,
    pub cleaner: &'a Cleaner,
    pub thresholds: &'a Thresholds,
}

fn read_index(dir: &Path) -> Index {
    let path = dir.join(INDEX_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("online pictures: {} unreadable, started over — {e}", path.display());
            Index::default()
        }),
        Err(_) => Index::default(),
    }
}

fn write_index(dir: &Path, index: &Index) {
    let result = std::fs::create_dir_all(dir)
        .and_then(|_| serde_json::to_vec(index).map_err(std::io::Error::other))
        .and_then(|bytes| std::fs::write(dir.join(INDEX_FILE), bytes));
    if let Err(e) = result {
        log::warn!("online pictures: cache not written — {e}");
    }
}

/// Whether a cached answer still stands: a picture always does - as long as
/// its file is there -, an empty answer for a while.
fn still_good(entry: &Entry, now: i64) -> bool {
    match &entry.picture.file {
        Some(file) => Path::new(file).is_file(),
        None => now - entry.checked_at < RETRY_EMPTY_SECS,
    }
}

/// What the cache holds for a track - no network.
pub fn cached(dir: &Path, track_id: &str) -> Option<TrackPicture> {
    let index = read_index(dir);
    let entry = index.tracks.get(&track_id.to_lowercase())?;
    entry
        .picture
        .file
        .as_deref()
        .is_none_or(|f| Path::new(f).is_file())
        .then(|| entry.picture.clone())
}

/// The picture of a track: cached, or asked and cached. `None` when the
/// network could not answer.
pub fn picture(dir: &Path, net: &WikiClient, sources: &Sources, track_id: &str, now: i64) -> Option<TrackPicture> {
    let id = track_id.to_lowercase();
    if let Some(entry) = read_index(dir).tracks.get(&id) {
        if still_good(entry, now) {
            return Some(entry.picture.clone());
        }
    }
    let picture = match resolve(dir, net, sources, &id) {
        Fetched::Found(p) => p,
        Fetched::Absent => TrackPicture::default(),
        Fetched::Unavailable => return None,
    };
    // Read again: another track may have been written meanwhile.
    let mut index = read_index(dir);
    index.tracks.insert(
        id,
        Entry {
            picture: picture.clone(),
            checked_at: now,
        },
    );
    write_index(dir, &index);
    Some(picture)
}

/// The entity of a track: its shipped link, else a match by CSP's
/// coordinates and name. `Absent` when nothing designates one.
fn entity_of(net: &WikiClient, sources: &Sources, id: &str) -> Fetched<String> {
    if let Some(q) = sources.curated.get(id) {
        return Fetched::Found(q.clone());
    }
    // Without CSP's entry the track is a bare folder name: matching that by
    // words would find articles, not this track.
    let Some(location) = sources
        .ac_install
        .and_then(|ac| crate::sun::track_location(ac, id, None))
        .filter(|l| l.source != crate::sun::CoordSource::Geotags)
    else {
        return Fetched::Absent;
    };
    let subject = TrackSubject {
        name: location.name.clone(),
        location: Some((location.latitude, location.longitude)),
    };
    match matchtrack::match_track(net, sources.cleaner, sources.thresholds, &subject, "en") {
        MatchOutcome::Matched { candidate, .. } => Fetched::Found(candidate.entity_id),
        MatchOutcome::Unavailable => Fetched::Unavailable,
        MatchOutcome::Ambiguous { .. } | MatchOutcome::NoCandidate => Fetched::Absent,
    }
}

fn resolve(dir: &Path, net: &WikiClient, sources: &Sources, id: &str) -> Fetched<TrackPicture> {
    let entity = match entity_of(net, sources, id) {
        Fetched::Found(q) => q,
        other => return other.map_absent(),
    };
    let facts = match net.entity_picture(&entity) {
        Fetched::Found(f) => f,
        other => return other.map_absent(),
    };
    let mut out = TrackPicture::default();
    if let Some(country) = &facts.country {
        match net.entity_picture(country) {
            Fetched::Found(c) => out.country = c.iso2.map(|c| c.to_uppercase()),
            Fetched::Unavailable => return Fetched::Unavailable,
            Fetched::Absent => {}
        }
    }
    if let Some(credit) = facts.image.as_deref().and_then(|f| net.commons_credit(f)) {
        match download(dir, net, &credit.url) {
            Some(file) => {
                out.file = Some(file.to_string_lossy().into_owned());
                out.artist = Some(credit.artist);
                out.licence = Some(credit.licence);
                out.page = Some(credit.description_url).filter(|p| !p.is_empty());
            }
            // The facts stand; the picture is asked again next time.
            None => return Fetched::Unavailable,
        }
    }
    Fetched::Found(out)
}

/// The thumbnail, beside the others of the cache, named by its URL.
fn download(dir: &Path, net: &WikiClient, url: &str) -> Option<PathBuf> {
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_lowercase())
        .filter(|ext| ["jpg", "jpeg", "png", "webp", "gif"].contains(&ext.as_str()))
        .unwrap_or_else(|| "jpg".into());
    let key = format!("{:x}", Sha256::digest(url.as_bytes()));
    let path = dir.join(format!("online-{key}.{ext}"));
    if path.is_file() {
        return Some(path);
    }
    let response = crate::http::get_url(url, net.user_agent(), DOWNLOAD_TIMEOUT_MS, MAX_IMAGE_BYTES)?;
    if response.status != 200 || response.body.is_empty() {
        log::warn!("online pictures: {url} answered {}", response.status);
        return None;
    }
    let written = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, &response.body));
    match written {
        Ok(()) => Some(path),
        Err(e) => {
            log::warn!("online pictures: {} not written — {e}", path.display());
            None
        }
    }
}

trait MapAbsent<T> {
    /// A `Fetched` of another kind that did not find: its verdict carried over.
    fn map_absent(self) -> Fetched<T>;
}

impl<U, T> MapAbsent<T> for Fetched<U> {
    fn map_absent(self) -> Fetched<T> {
        match self {
            Fetched::Unavailable => Fetched::Unavailable,
            _ => Fetched::Absent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(file: Option<&Path>, checked_at: i64) -> Entry {
        Entry {
            picture: TrackPicture {
                file: file.map(|f| f.to_string_lossy().into_owned()),
                ..Default::default()
            },
            checked_at,
        }
    }

    // SPEC-play-online, "Vignette d'un circuit non installé": a picture is
    // asked once; an empty answer is asked again after a while; a picture
    // whose file vanished from the cache is asked again at once.
    #[test]
    fn a_cached_answer_stands_until_it_should_be_asked_again() {
        let dir = crate::testutil::temp_dir("onlinepic");
        let file = dir.join("online-x.jpg");
        std::fs::write(&file, b"IMG").unwrap();
        let now = 100 * 24 * 3600;
        assert!(still_good(&entry(Some(&file), 0), now), "a picture stays");
        assert!(still_good(&entry(None, now - 3600), now), "an empty answer, recent");
        assert!(
            !still_good(&entry(None, now - RETRY_EMPTY_SECS - 1), now),
            "an empty answer, old"
        );
        assert!(
            !still_good(&entry(Some(&dir.join("gone.jpg")), now), now),
            "a picture whose file is gone"
        );
    }

    #[test]
    fn the_cache_reads_back_what_it_wrote_by_lowercase_id() {
        let dir = crate::testutil::temp_dir("onlinepic2");
        let mut index = Index::default();
        let picture = TrackPicture {
            country: Some("IT".into()),
            ..Default::default()
        };
        index.tracks.insert(
            "monza".into(),
            Entry {
                picture: picture.clone(),
                checked_at: 1,
            },
        );
        write_index(&dir, &index);
        assert_eq!(
            cached(&dir, "Monza"),
            Some(picture),
            "found whatever the case of the id"
        );
        assert_eq!(cached(&dir, "spa"), None, "a track never asked");
    }

    // Never another track's image: a track CSP does not know, with no shipped
    // link, is matched by nothing - not by its folder name.
    #[test]
    fn a_track_nobody_designates_has_no_entity() {
        let dir = crate::testutil::temp_dir("onlinepic3");
        let ac = dir.join("ac");
        std::fs::create_dir_all(ac.join("extension").join("config")).unwrap();
        let curated = BTreeMap::new();
        let cleaner = Cleaner::new(&Default::default());
        let thresholds = crate::wiki::matching::Thresholds::from_prefs(&Default::default());
        let sources = Sources {
            ac_install: Some(&ac),
            curated: &curated,
            cleaner: &cleaner,
            thresholds: &thresholds,
        };
        let net = WikiClient::new();
        assert!(matches!(entity_of(&net, &sources, "nohesi_415"), Fetched::Absent));
        let curated = BTreeMap::from([("nohesi_415".to_string(), "Q1".to_string())]);
        let sources = Sources {
            curated: &curated,
            ..sources
        };
        assert!(
            matches!(entity_of(&net, &sources, "nohesi_415"), Fetched::Found(q) if q == "Q1"),
            "a shipped link is taken as is"
        );
    }

    /// What a few lobby tracks get, for real: run by hand with
    /// `PITBOX_AC=<install> cargo test --lib online::picture -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the AC install and the network; diagnostic, not a check"]
    fn what_lobby_tracks_get() {
        let Ok(ac) = std::env::var("PITBOX_AC") else {
            eprintln!("PITBOX_AC not set");
            return;
        };
        let dir = crate::testutil::temp_dir("onlinepic-net");
        let curated = crate::wiki::curated::shipped()
            .links
            .into_iter()
            .map(|(id, q)| (id.to_lowercase(), q))
            .collect();
        let cleaner = Cleaner::new(&Default::default());
        let thresholds = crate::wiki::matching::Thresholds::from_prefs(&Default::default());
        let ac = PathBuf::from(ac);
        let sources = Sources {
            ac_install: Some(&ac),
            curated: &curated,
            cleaner: &cleaner,
            thresholds: &thresholds,
        };
        let net = WikiClient::new();
        for id in [
            "valencia",
            "salzburgring",
            "monza",
            "shuto_revival_project_beta",
            "pk_akina",
            "nohesi_415",
        ] {
            eprintln!("{id}: {:?}", picture(&dir, &net, &sources, id, 0));
        }
    }
}
