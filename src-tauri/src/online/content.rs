//! Where to get what a server needs and this machine lacks
//! (`SPEC-play-online.md`, "Contenu manquant : prêt à rejoindre").
//!
//! Three sources, in the spec's order — the first that needs nothing from the
//! user wins:
//!
//! 1. **the archive kept at import**, for a mod in the showcase
//!    (`showcase::sources`) — offline;
//! 2. **the server's own word**: the `content` block of `/api/details`, the
//!    format of Content Manager's wrapper (`ServerEntry.Extended.cs`) — per car
//!    and for the track, a `url`, or nothing, in which case the server serves
//!    the file itself at `/content/car/<id>` and `/content/track`; `cup: true`
//!    sends to the registry, `direct: false` means "not downloadable";
//! 3. **Content Manager's registry** (CUP, `cup.rs`), when it lists the id and
//!    lets a program download it.
//!
//! Whatever the source, the download follows `cup::download_archive`: an
//! archive goes to the import, anything else to the browser. This module only
//! says where.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::cup::{compare_versions, Registry};

use super::readiness::Level;

/// The registry changes a few times a day; asking it at every server opened
/// would be one request per click for the same answer.
const REGISTRY_FRESH: Duration = Duration::from_secs(10 * 60);

static REGISTRY: Mutex<Option<(Instant, Registry)>> = Mutex::new(None);

/// What the server declares about one car or its track.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Declared {
    pub url: Option<String>,
    /// The `url` key is there, even empty. An empty one means "no link" —
    /// measured: such a server answers 404 on its own `/content/…` — while an
    /// absent one means the server serves the file itself.
    pub url_given: bool,
    pub version: Option<String>,
    pub cup: bool,
    /// `direct: false`: the server says it cannot be downloaded.
    pub not_direct: bool,
}

/// The `content` block, per car id (lowercase) and for the track.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ServerContent {
    pub cars: HashMap<String, Declared>,
    pub track: Option<Declared>,
    /// The server protects its files with its password (`"password": true`):
    /// its own downloads need a hash Pit Box does not compute.
    pub password: bool,
}

fn declared(v: &Value) -> Declared {
    Declared {
        url: v["url"].as_str().filter(|u| !u.trim().is_empty()).map(str::to_string),
        url_given: v.get("url").is_some(),
        version: v["version"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        cup: v["cup"].as_bool() == Some(true),
        not_direct: v["direct"].as_bool() == Some(false),
    }
}

/// Reads the `content` block of an `/api/details` answer; empty without one.
pub(super) fn parse_content(details: &Value) -> ServerContent {
    let content = &details["content"];
    ServerContent {
        cars: content["cars"]
            .as_object()
            .map(|cars| cars.iter().map(|(id, v)| (id.to_lowercase(), declared(v))).collect())
            .unwrap_or_default(),
        track: content["track"].is_object().then(|| declared(&content["track"])),
        password: content["password"].as_bool() == Some(true),
    }
}

/// How to get one car or track, and how its version compares.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Fetch {
    /// The source archive kept at import (a mod in the showcase).
    pub kept_archive: bool,
    /// Where the server says to download it: the author's link, or the
    /// server's own `/content/…`.
    pub server_url: Option<String>,
    /// Content Manager's registry has it and lets a program download it.
    pub cup: bool,
    /// The version the server runs, when it declares one.
    pub server_version: Option<String>,
    /// The version installed here, when there is one.
    pub installed_version: Option<String>,
    /// Joining fetches it first: missing here, or outdated, with a source.
    /// Set with the level (`settle`).
    pub needed: bool,
}

impl Fetch {
    /// There is a source to download from.
    pub fn any(&self) -> bool {
        self.kept_archive || self.server_url.is_some() || self.cup
    }

    /// The server runs a newer version than the one installed — the other
    /// cause of a kick, the integrity check (`SPEC-play-online.md`, "Versions").
    pub fn outdated(&self) -> bool {
        self.installed_version.is_some()
            && compare_versions(self.server_version.as_deref(), self.installed_version.as_deref())
                == std::cmp::Ordering::Greater
    }
}

/// A level once the sources are known (`SPEC-play-online.md`, "Contenu
/// manquant"): missing but downloadable becomes one click — "Prepare & join"
/// fetches it first; installed but older than the server's becomes one click
/// when an update can be fetched (the integrity check would kick otherwise),
/// and stays as it is when none can: the panel then shows the versions. A
/// blocked level (a DLC) stays blocked.
/// Marks `fetch.needed` when the level is one click because of a download.
pub fn settle(level: Level, fetch: &mut Fetch) -> Level {
    let settled = match level {
        Level::Download if fetch.any() => Level::OneClick,
        Level::Ready if fetch.outdated() && (fetch.server_url.is_some() || fetch.cup) => Level::OneClick,
        other => other,
    };
    fetch.needed = settled != level;
    settled
}

/// What the server and the registry offer for one car (`kind` `"car"`) or
/// the track (`"track"`, `path` `None`). `base` is the server's own address,
/// `http://ip:port`.
pub(super) fn fetch_for(
    declared: Option<&Declared>,
    server: &ServerContent,
    base: &str,
    path: &str,
    registry: &Registry,
    cup_key: (&str, &str),
) -> Fetch {
    let mut fetch = Fetch::default();
    if let Some(d) = declared {
        fetch.server_version = d.version.clone();
        fetch.server_url = match (&d.url, d.url_given, d.cup, d.not_direct) {
            (Some(url), _, _, _) => Some(url.clone()),
            (None, false, false, false) if !server.password => Some(format!("{base}{path}")),
            _ => None,
        };
        fetch.cup = d.cup;
    }
    let key = (cup_key.0.to_string(), cup_key.1.to_lowercase());
    if registry.get(&key).is_some_and(|latest| !latest.limited) {
        fetch.cup = true;
    }
    fetch
}

/// The registry, from the cache when fresh. An unreachable registry is no
/// source rather than an error: the server's own links still work.
pub(super) fn registry() -> Registry {
    let mut cache = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, registry)) = cache.as_ref() {
        if at.elapsed() < REGISTRY_FRESH {
            return registry.clone();
        }
    }
    match crate::cup::fetch_registry() {
        Ok(registry) => {
            *cache = Some((Instant::now(), registry.clone()));
            registry
        }
        Err(e) => {
            log::warn!("online: no registry ({e}), servers' own links only");
            Registry::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cup::Latest;

    fn content() -> ServerContent {
        parse_content(&serde_json::json!({ "content": {
            "cars": {
                "RSS_GTM_Lanzo_V8": { "url": "https://example.com/lanzo.7z", "version": "1.4" },
                "self_hosted": { "version": "2.0" },
                "from_cup": { "cup": true },
                "not_offered": { "direct": false },
                "empty_link": { "url": "" }
            },
            "track": { "version": "1.1" }
        }}))
    }

    fn fetch(id: &str, registry: &Registry) -> Fetch {
        let c = content();
        fetch_for(
            c.cars.get(id),
            &c,
            "http://1.2.3.4:8081",
            &format!("/content/car/{id}"),
            registry,
            ("car", id),
        )
    }

    /// Rule (CM's `InstallMissingContentTasks`): a declared url is the source;
    /// without one the server serves the file itself; `cup` sends to the
    /// registry; `direct: false` offers nothing.
    #[test]
    fn the_server_says_where_each_car_is() {
        let none = Registry::new();
        assert_eq!(
            fetch("rss_gtm_lanzo_v8", &none).server_url.as_deref(),
            Some("https://example.com/lanzo.7z"),
            "the author's link, id read without case"
        );
        assert_eq!(
            fetch("self_hosted", &none).server_url.as_deref(),
            Some("http://1.2.3.4:8081/content/car/self_hosted"),
            "served by the server"
        );
        let cup = fetch("from_cup", &none);
        assert!(cup.cup && cup.server_url.is_none(), "the registry, not the server");
        assert!(!fetch("not_offered", &none).any(), "nothing offered");
        assert!(
            !fetch("empty_link", &none).any(),
            "an empty url is no link, not the server's own file (real server: 404)"
        );
    }

    /// Rule: the registry is a source for anything it lists, unless limited
    /// (paid, sign-in) — what the server says or not.
    #[test]
    fn the_registry_is_the_last_source() {
        let mut registry = Registry::new();
        registry.insert(
            ("car".into(), "listed".into()),
            Latest {
                version: "1.0".into(),
                limited: false,
            },
        );
        registry.insert(
            ("car".into(), "patreon".into()),
            Latest {
                version: "1.0".into(),
                limited: true,
            },
        );
        assert!(fetch("listed", &registry).cup);
        assert!(!fetch("patreon", &registry).any(), "limited: no program download");
    }

    /// Rule: a protected server's own files need its password hash; they are
    /// not offered, the author's links still are.
    #[test]
    fn a_password_hides_the_server_own_files() {
        let mut c = content();
        c.password = true;
        let own = fetch_for(
            c.cars.get("self_hosted"),
            &c,
            "http://x",
            "/content/car/self_hosted",
            &Registry::new(),
            ("car", "self_hosted"),
        );
        assert!(own.server_url.is_none());
    }

    /// Rule (SPEC-play-online, "Contenu manquant"): a source turns missing
    /// content into one click; an update with a source too; a DLC stays
    /// blocked whatever is offered.
    #[test]
    fn sources_settle_the_level() {
        let mut source = Fetch {
            cup: true,
            ..Default::default()
        };
        assert_eq!(settle(Level::Download, &mut source), Level::OneClick);
        assert!(source.needed, "fetched before joining");
        assert_eq!(
            settle(Level::Download, &mut Fetch::default()),
            Level::Download,
            "no source"
        );
        assert_eq!(
            settle(Level::Blocked, &mut source),
            Level::Blocked,
            "a DLC is not downloaded"
        );
        assert!(!source.needed, "nothing fetched for a blocked join");
        let mut outdated = Fetch {
            cup: true,
            server_version: Some("2.0".into()),
            installed_version: Some("1.0".into()),
            ..Default::default()
        };
        assert_eq!(
            settle(Level::Ready, &mut outdated),
            Level::OneClick,
            "the update is fetched first"
        );
        let mut stale_without_source = Fetch {
            server_version: Some("2.0".into()),
            installed_version: Some("1.0".into()),
            ..Default::default()
        };
        assert_eq!(
            settle(Level::Ready, &mut stale_without_source),
            Level::Ready,
            "nothing to fetch it with"
        );
    }

    /// Rule (SPEC-play-online, "Versions"): a server on a newer version than
    /// the one installed makes the content outdated; an older or equal one
    /// does not.
    #[test]
    fn a_newer_server_version_is_outdated() {
        let mut f = fetch("rss_gtm_lanzo_v8", &Registry::new());
        f.installed_version = Some("1.2".into());
        assert!(f.outdated(), "1.4 on the server, 1.2 here");
        f.installed_version = Some("1.4".into());
        assert!(!f.outdated());
        f.installed_version = None;
        assert!(!f.outdated(), "nothing installed is missing, not outdated");
    }
}
