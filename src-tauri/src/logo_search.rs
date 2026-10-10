//! A brand's logo found online (TAXO§9.1): candidates from Wikimedia, one of
//! them picked by the user, downloaded and made his own logo file (TAXO§9).
//!
//! Nothing ships with Pit Box (TAXO§13) and nothing is fetched without a
//! gesture: the user asks, sees, picks - the file then lives in Pit Box's
//! folder like one he gave, never in the game's.
//!
//! Two sources, in this order, both through the Wikimedia client of the
//! Wikipedia tab (same User-Agent, same back-off):
//! 1. **Wikidata's "logo image" (P154)** of the entities the query names -
//!    for "Renault", the group's current logo and the 2009 one. Precise, so
//!    first, marked as such.
//! 2. **A file search on Commons** for "<query> logo" - the earlier logos, the
//!    variants, page by page.
//!
//! Only SVG and PNG are offered: what a logo of the user's may be (TAXO§9). On
//! the reference brands most results are SVG, transparent by construction.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::http;
use crate::wiki::api::{Fetched, WikiClient};

const WIKIDATA_HOST: &str = "www.wikidata.org";
const COMMONS_HOST: &str = "commons.wikimedia.org";
/// Where Commons serves its files: the only host a logo is downloaded from.
const FILES_URL: &str = "https://upload.wikimedia.org/";
/// Files asked of the Commons search per page. Some are dropped (a JPEG
/// photo of a dealership), so a page shows a little less.
const PAGE: u32 = 30;
/// Entities of the query whose logo is offered first: "Renault" is the group,
/// the manufacturer, the F1 team - each with its own logo.
const ENTITIES: u32 = 5;
/// Width asked of the thumbnailer for a preview. Commons rounds it to one of
/// its standard sizes, which is fine.
const THUMB_WIDTH: u32 = 160;
/// A logo is a small file; anything bigger is not what was meant.
const MAX_BYTES: u64 = 5 * 1024 * 1024;
const TIMEOUT_MS: i32 = 15_000;

/// One file offered.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// File name on Commons, without `File:`.
    pub name: String,
    /// The original file - what is downloaded.
    pub url: String,
    /// A PNG preview, an SVG's included.
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    /// `svg` or `png`.
    pub format: &'static str,
    /// Wikidata names it the logo of an entity the query found.
    pub official: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub candidates: Vec<Candidate>,
    /// The offset of the next page, `None` past the last.
    pub next: Option<u32>,
}

/// One page of candidates for `query`. The first page starts with Wikidata's
/// logos; the following ones are the Commons search alone.
pub fn search(query: &str, offset: u32) -> Result<Page, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Page {
            candidates: Vec::new(),
            next: None,
        });
    }
    let client = WikiClient::new();
    // Best-effort: without them the search still offers plenty.
    let mut candidates = if offset == 0 {
        official(&client, query)
    } else {
        Vec::new()
    };
    let path = format!(
        "/w/api.php?action=query&format=json&formatversion=2&generator=search&gsrnamespace=6\
         &gsrsearch={}&gsrlimit={PAGE}&gsroffset={offset}&prop=imageinfo&iiprop=url%7Csize%7Cmime\
         &iiurlwidth={THUMB_WIDTH}",
        http::encode_query_value(&format!("{query} logo")),
    );
    let (found, next) = match client.get_json(COMMONS_HOST, &path) {
        Fetched::Found(root) => parse_files(&root, false),
        Fetched::Absent => (Vec::new(), None),
        Fetched::Unavailable => return Err(crate::errors::LOGO_SEARCH_UNAVAILABLE.into()),
    };
    for c in found {
        if !candidates.iter().any(|o| o.name == c.name) {
            candidates.push(c);
        }
    }
    Ok(Page { candidates, next })
}

/// The logos Wikidata gives the entities `query` names (P154), the preferred
/// one of each first.
fn official(client: &WikiClient, query: &str) -> Vec<Candidate> {
    let path = format!(
        "/w/api.php?action=wbsearchentities&format=json&language=en&type=item&limit={ENTITIES}&search={}",
        http::encode_query_value(query),
    );
    let Fetched::Found(root) = client.get_json(WIKIDATA_HOST, &path) else {
        return Vec::new();
    };
    let ids = parse_entity_ids(&root);
    if ids.is_empty() {
        return Vec::new();
    }
    let path = format!(
        "/w/api.php?action=wbgetentities&format=json&formatversion=2&props=claims&ids={}",
        http::encode_query_value(&ids.join("|")),
    );
    let Fetched::Found(root) = client.get_json(WIKIDATA_HOST, &path) else {
        return Vec::new();
    };
    let names = parse_logo_claims(&root, &ids);
    if names.is_empty() {
        return Vec::new();
    }
    let titles: Vec<String> = names.iter().map(|n| format!("File:{n}")).collect();
    let path = format!(
        "/w/api.php?action=query&format=json&formatversion=2&prop=imageinfo&iiprop=url%7Csize%7Cmime\
         &iiurlwidth={THUMB_WIDTH}&titles={}",
        http::encode_query_value(&titles.join("|")),
    );
    let Fetched::Found(root) = client.get_json(COMMONS_HOST, &path) else {
        return Vec::new();
    };
    let (mut found, _) = parse_files(&root, true);
    // `titles=` answers in its own order: Wikidata's is the one that means
    // something (the entities by relevance, the preferred logo first).
    found.sort_by_key(|c| names.iter().position(|n| n == &c.name).unwrap_or(usize::MAX));
    found
}

/// Downloads a candidate and makes it the brand's logo, as a file of his
/// would be (TAXO§9): copied into `logos/`, his plate setting kept.
pub fn adopt(dir: &Path, brand: &str, url: &str) -> Result<(), String> {
    // Only Commons' file host: the URL comes from the page, and nothing else
    // is ours to fetch on its word.
    if !url.starts_with(FILES_URL) {
        log::warn!("logo_search: refused to download {url}");
        return Err(crate::errors::LOGO_DOWNLOAD_FAILED.into());
    }
    let ext = extension_of(url).ok_or(crate::errors::LOGO_FORMAT)?;
    let tmp = std::env::temp_dir().join(format!("pitbox-logo-{}.{ext}", uuid::Uuid::new_v4()));
    let downloaded = http::download(url, &tmp, WikiClient::new().user_agent(), TIMEOUT_MS, &mut |done, _| {
        done <= MAX_BYTES
    });
    let result = match downloaded {
        Ok(_) => crate::logos::adopt_file(dir, brand, &tmp),
        Err(e) => {
            log::warn!("logo_search: download of {url} failed: {e:?}");
            Err(crate::errors::LOGO_DOWNLOAD_FAILED.into())
        }
    };
    if tmp.exists() {
        if let Err(e) = std::fs::remove_file(&tmp) {
            log::warn!("logo_search: {} not removed: {e}", tmp.display());
        }
    }
    result
}

/// `svg` or `png`, from the URL's path - its query string set aside.
fn extension_of(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next()?.to_ascii_lowercase();
    if path.ends_with(".svg") {
        Some("svg")
    } else if path.ends_with(".png") {
        Some("png")
    } else {
        None
    }
}

// --- Parsing, pure: the shapes measured on "Renault", 2026-10-10 ------------------

/// `wbsearchentities`: the ids, by relevance.
fn parse_entity_ids(root: &Value) -> Vec<String> {
    root["search"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["id"].as_str())
        .filter(|id| crate::wiki::api::is_entity_id(id))
        .map(str::to_string)
        .collect()
}

/// `wbgetentities` with `props=claims`: the P154 file names, entity by entity
/// in `ids`' order, the preferred rank first within each, no deprecated one.
fn parse_logo_claims(root: &Value, ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let mut claims: Vec<(&str, &str)> = root["entities"][id]["claims"]["P154"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| Some((c["rank"].as_str()?, c["mainsnak"]["datavalue"]["value"].as_str()?)))
            .filter(|(rank, _)| *rank != "deprecated")
            .collect();
        claims.sort_by_key(|(rank, _)| *rank != "preferred");
        for (_, name) in claims {
            if !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// `prop=imageinfo` pages - from a search (`index` gives the order) or from
/// titles - as candidates, SVG and PNG only; and the next search offset.
fn parse_files(root: &Value, official: bool) -> (Vec<Candidate>, Option<u32>) {
    let mut pages: Vec<&Value> = root["query"]["pages"].as_array().into_iter().flatten().collect();
    pages.sort_by_key(|p| p["index"].as_u64().unwrap_or(u64::MAX));
    let candidates = pages
        .into_iter()
        .filter_map(|p| {
            let info = p["imageinfo"].as_array()?.first()?;
            let format = match info["mime"].as_str()? {
                "image/svg+xml" => "svg",
                "image/png" => "png",
                _ => return None,
            };
            Some(Candidate {
                name: p["title"].as_str()?.trim_start_matches("File:").to_string(),
                url: info["url"].as_str()?.to_string(),
                thumb: info["thumburl"].as_str()?.to_string(),
                width: info["width"].as_u64().unwrap_or(0) as u32,
                height: info["height"].as_u64().unwrap_or(0) as u32,
                format,
                official,
            })
        })
        .collect();
    let next = root["continue"]["gsroffset"].as_u64().map(|n| n as u32);
    (candidates, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// TAXO§9.1: Wikidata's logos come entity by entity, by relevance, the
    /// preferred one first and a deprecated one never - the shape measured
    /// on "Renault".
    #[test]
    fn wikidata_logos_come_by_entity_preferred_first() {
        let search = json!({"search": [{"id": "Q6686"}, {"id": "Q98584518"}, {"id": "not an id"}]});
        let ids = parse_entity_ids(&search);
        assert_eq!(ids, vec!["Q6686", "Q98584518"], "only real ids, in order");
        let entities = json!({"entities": {
            "Q98584518": {"claims": {"P154": [
                {"rank": "normal", "mainsnak": {"datavalue": {"value": "Renault 2009 logo.svg"}}}
            ]}},
            "Q6686": {"claims": {"P154": [
                {"rank": "deprecated", "mainsnak": {"datavalue": {"value": "Old.svg"}}},
                {"rank": "normal", "mainsnak": {"datavalue": {"value": "Renault 2015 logo.svg"}}},
                {"rank": "preferred", "mainsnak": {"datavalue": {"value": "2021 Renault Group logo.svg"}}}
            ]}}
        }});
        assert_eq!(
            parse_logo_claims(&entities, &ids),
            vec![
                "2021 Renault Group logo.svg",
                "Renault 2015 logo.svg",
                "Renault 2009 logo.svg"
            ]
        );
    }

    /// TAXO§9.1: a Commons search reads in its `index` order, keeps SVG and
    /// PNG only - what a logo of the user's may be - and says where the next
    /// page starts.
    #[test]
    fn a_commons_search_keeps_svg_and_png_in_order() {
        let info = |mime: &str| {
            json!([{"url": "https://upload.wikimedia.org/x", "thumburl": "https://thumb.wikimedia.org/x.png",
                    "width": 512, "height": 353, "mime": mime}])
        };
        let root = json!({
            "continue": {"gsroffset": 30},
            "query": {"pages": [
                {"index": 3, "title": "File:Renault logo.svg", "imageinfo": info("image/svg+xml")},
                {"index": 2, "title": "File:Dealer.jpg", "imageinfo": info("image/jpeg")},
                {"index": 1, "title": "File:Renault F1 logo.png", "imageinfo": info("image/png")}
            ]}
        });
        let (found, next) = parse_files(&root, false);
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Renault F1 logo.png", "Renault logo.svg"],
            "index order, no JPEG"
        );
        assert_eq!(found[1].format, "svg");
        assert_eq!(next, Some(30));
        assert_eq!(
            parse_files(&json!({"query": {"pages": []}}), false).1,
            None,
            "last page"
        );
    }

    /// The real thing, over the network - a probe, not a check (`--ignored
    /// --nocapture` to read): Renault's candidates, then the first one
    /// downloaded and adopted into a scratch folder.
    #[test]
    #[ignore = "network; probe, not a check"]
    fn probe_renault_online() {
        let page = search("Renault", 0).expect("Wikimedia reachable");
        for c in &page.candidates {
            println!(
                "{} {} {}x{} {}",
                if c.official { "*" } else { " " },
                c.format,
                c.width,
                c.height,
                c.name
            );
        }
        println!("next: {:?}", page.next);
        let dir = crate::testutil::temp_dir("logo-probe");
        let first = page.candidates.first().expect("a candidate");
        adopt(&dir, "Renault", &first.url).expect("adopted");
        let pref = crate::logos::load_prefs(&dir).remove("Renault").expect("a choice");
        let file = dir.join(crate::logos::LOGOS_DIR).join(pref.custom.expect("his file"));
        println!(
            "adopted {} ({} bytes)",
            file.display(),
            std::fs::metadata(&file).unwrap().len()
        );
    }

    /// Only Commons' file host is downloaded from, and only a logo format.
    #[test]
    fn only_a_commons_svg_or_png_is_downloaded() {
        let dir = crate::testutil::temp_dir("logo-adopt");
        assert_eq!(
            adopt(&dir, "Renault", "https://example.com/renault.svg").unwrap_err(),
            crate::errors::LOGO_DOWNLOAD_FAILED,
            "another host is refused before any request"
        );
        assert_eq!(
            adopt(&dir, "Renault", "https://upload.wikimedia.org/a/b/Renault.jpg").unwrap_err(),
            crate::errors::LOGO_FORMAT
        );
        assert_eq!(
            extension_of("https://upload.wikimedia.org/0/09/Logo.SVG?utm_source=x"),
            Some("svg"),
            "the query string is no part of the name"
        );
    }
}
