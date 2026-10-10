//! A brand's logo found online (TAXO§9.1): candidates from three sources, one
//! of them picked by the user, downloaded and made his own logo file (TAXO§9).
//!
//! Nothing ships with Pit Box (TAXO§13) and nothing is fetched without a
//! gesture: the user asks, sees, picks - the file then lives in Pit Box's
//! folder like one he gave, never in the game's.
//!
//! Three sources, in this order:
//! 1. **car-logos-dataset**, an open index of 387 car makers' logos taken from
//!    carlogos.org: the chrome emblems a badge looks like, transparent, at
//!    256 px. One per brand, found by name - 39 of the 47 brands of the
//!    reference library, the missing ones being confidential (Tatuus,
//!    Glickenhaus). First: it is made for exactly this.
//! 2. **Wikidata's "logo image" (P154)** of the entities the query names - for
//!    "Renault", the group's current logo and the 2009 one.
//! 3. **A file search on Commons** for "<query> logo" - the earlier logos, the
//!    variants, page by page. The widest offer and the least convincing: many
//!    wordmarks, lettering, models' logos.
//!
//! Only SVG and PNG are offered: what a logo of the user's may be (TAXO§9).
//! And no raster bigger than a logo needs: Commons hands a 500 px rendition
//! instead of a 5,000 px original, and whatever arrives is brought down to
//! [`MAX_SIDE`] before it is stored.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

use crate::http;
use crate::wiki::api::{Fetched, WikiClient};

const WIKIDATA_HOST: &str = "www.wikidata.org";
const COMMONS_HOST: &str = "commons.wikimedia.org";
/// The dataset's index: name, slug and image URLs of every maker.
const DATASET_INDEX: &str = "https://raw.githubusercontent.com/filippofilip95/car-logos-dataset/master/logos/data.json";
/// What a logo is downloaded from - and nothing else: the URL comes from the
/// page, and nothing else is ours to fetch on its word.
const DOWNLOAD_HOSTS: [&str; 3] = [
    "https://upload.wikimedia.org/",
    "https://thumb.wikimedia.org/",
    "https://raw.githubusercontent.com/filippofilip95/car-logos-dataset/",
];
/// Files asked of the Commons search per page. Some are dropped (a JPEG
/// photo of a dealership), so a page shows a little less.
const PAGE: u32 = 30;
/// Entities of the query whose logo is offered: "Renault" is the group, the
/// manufacturer, the F1 team - each with its own logo.
const ENTITIES: u32 = 5;
/// Dataset entries offered at most: the exact name, then names that contain
/// it ("Renault" also finds "Renault Samsung").
const DATASET_HITS: usize = 6;
/// Width of the rendition Commons is asked for: the preview, and what a PNG
/// downloads instead of its original. One of Commons' standard sizes, and
/// wide enough that a long wordmark (2000x296) keeps 64 px of height.
const THUMB_WIDTH: u32 = 500;
/// A stored raster logo's longest side. A logo is drawn at 32 px at most, 96
/// on a 3x screen (TAXO§10); the election stops counting at 128 (`logos.rs`).
/// Asked at use: results of more than 5,000 px had no business here.
pub const MAX_SIDE: u32 = 512;
/// A logo is a small file; anything bigger is not what was meant.
const MAX_BYTES: u64 = 5 * 1024 * 1024;
const TIMEOUT_MS: i32 = 15_000;

/// Where a candidate comes from - shown on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Carlogos,
    Wikidata,
    Commons,
}

/// One file offered.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// File name at its source, for the tooltip.
    pub name: String,
    /// What is downloaded when it is picked - a rendition rather than a huge
    /// original.
    pub url: String,
    /// A PNG preview, an SVG's included.
    pub thumb: String,
    /// The size of what is downloaded; `None` for a vector file, which has
    /// none that matters, or when the source does not say.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// `svg` or `png`.
    pub format: &'static str,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub candidates: Vec<Candidate>,
    /// The offset of the next page, `None` past the last.
    pub next: Option<u32>,
}

/// One page of candidates for `query`. The first page starts with the
/// dataset and Wikidata; the following ones are the Commons search alone.
pub fn search(query: &str, offset: u32) -> Result<Page, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Page {
            candidates: Vec::new(),
            next: None,
        });
    }
    let client = WikiClient::new();
    // Both best-effort: without them the search still offers plenty.
    let mut candidates = if offset == 0 {
        let mut first = dataset(&client, query);
        first.extend(official(&client, query));
        first
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
        Fetched::Found(root) => parse_files(&root, Source::Commons),
        Fetched::Absent => (Vec::new(), None),
        Fetched::Unavailable if !candidates.is_empty() => (Vec::new(), None),
        Fetched::Unavailable => return Err(crate::errors::LOGO_SEARCH_UNAVAILABLE.into()),
    };
    for c in found {
        if !candidates.iter().any(|o| o.name == c.name) {
            candidates.push(c);
        }
    }
    Ok(Page { candidates, next })
}

/// One maker of the dataset.
#[derive(Debug, Clone, PartialEq)]
struct DatasetEntry {
    name: String,
    slug: String,
    thumb: String,
}

/// The dataset's index, read once per session - 387 entries, and the same
/// for every brand searched. Kept only when it was read.
static DATASET: Mutex<Option<Vec<DatasetEntry>>> = Mutex::new(None);

/// The dataset's logos for `query`.
fn dataset(client: &WikiClient, query: &str) -> Vec<Candidate> {
    let cached = DATASET.lock().ok().and_then(|d| d.clone());
    let entries = match cached {
        Some(entries) => entries,
        None => {
            let Some(response) = http::get_url(DATASET_INDEX, client.user_agent(), TIMEOUT_MS, MAX_BYTES as usize)
            else {
                return Vec::new();
            };
            let parsed = (response.status == 200)
                .then(|| serde_json::from_slice::<Value>(&response.body).ok())
                .flatten()
                .map(|root| parse_dataset(&root))
                .unwrap_or_default();
            if parsed.is_empty() {
                log::warn!("logo_search: dataset index unreadable (status {})", response.status);
                return Vec::new();
            }
            if let Ok(mut d) = DATASET.lock() {
                *d = Some(parsed.clone());
            }
            parsed
        }
    };
    match_dataset(&entries, query)
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
    let (mut found, _) = parse_files(&root, Source::Wikidata);
    // `titles=` answers in its own order: Wikidata's is the one that means
    // something (the entities by relevance, the preferred logo first).
    found.sort_by_key(|c| names.iter().position(|n| n == &c.name).unwrap_or(usize::MAX));
    found
}

/// Downloads a candidate and makes it the brand's logo, as a file of his
/// would be (TAXO§9): copied into `logos/`, his plate setting kept. A raster
/// bigger than [`MAX_SIDE`] is brought down first.
pub fn adopt(dir: &Path, brand: &str, url: &str) -> Result<(), String> {
    if !DOWNLOAD_HOSTS.iter().any(|h| url.starts_with(h)) {
        log::warn!("logo_search: refused to download {url}");
        return Err(crate::errors::LOGO_DOWNLOAD_FAILED.into());
    }
    let ext = extension_of(url).ok_or(crate::errors::LOGO_FORMAT)?;
    let tmp = std::env::temp_dir().join(format!("pitbox-logo-{}.{ext}", uuid::Uuid::new_v4()));
    let downloaded = http::download(url, &tmp, WikiClient::new().user_agent(), TIMEOUT_MS, &mut |done, _| {
        done <= MAX_BYTES
    });
    let result = match downloaded {
        Ok(_) => {
            if ext == "png" {
                shrink(&tmp);
            }
            crate::logos::adopt_file(dir, brand, &tmp)
        }
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

/// Brings a PNG down to [`MAX_SIDE`] on its longest side, in place - never
/// below the 64 px a logo of the user's needs on its shortest (TAXO§9), which
/// a long wordmark would otherwise lose. Best-effort: left as it came, the
/// file is still a valid logo, only heavier.
fn shrink(path: &Path) {
    let img = match image::open(path) {
        Ok(img) => img,
        Err(e) => {
            log::warn!("logo_search: {} not decoded, kept as is: {e}", path.display());
            return;
        }
    };
    let Some((w, h)) = shrunk_size(img.width(), img.height()) else {
        return;
    };
    let small = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    if let Err(e) = small.save_with_format(path, image::ImageFormat::Png) {
        log::warn!("logo_search: {} not shrunk: {e}", path.display());
    }
}

/// The size to bring a raster to, `None` when it already fits.
fn shrunk_size(width: u32, height: u32) -> Option<(u32, u32)> {
    let (long, short) = (width.max(height), width.min(height));
    if long <= MAX_SIDE || short == 0 {
        return None;
    }
    let scale = (f64::from(MAX_SIDE) / f64::from(long)).max((64.0 / f64::from(short)).min(1.0));
    if scale >= 1.0 {
        return None;
    }
    let size = |v: u32| ((f64::from(v) * scale).round() as u32).max(1);
    Some((size(width), size(height)))
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

// --- Parsing and matching, pure: the shapes measured on 2026-10-10 ----------------

/// The dataset's index: `[{name, slug, image: {thumb, …}}]`. The thumbnail is
/// what is used, preview and download alike: 256 px on its shortest side,
/// where `optimized` and `original` run to 2,048.
fn parse_dataset(root: &Value) -> Vec<DatasetEntry> {
    root.as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(DatasetEntry {
                name: e["name"].as_str()?.to_string(),
                slug: e["slug"].as_str()?.to_string(),
                thumb: e["image"]["thumb"].as_str()?.to_string(),
            })
        })
        .collect()
}

/// Letters and digits only, folded: "Mercedes-Benz", "mercedes benz" and
/// "Mercédès Benz" are one key.
fn key(s: &str) -> String {
    crate::brands::fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The entries `query` names: the exact name or slug first, then names that
/// contain it or that it contains ("Alfa Romeo Giulia" finds Alfa Romeo) -
/// three characters at least, or "MG" would match half the list.
fn match_dataset(entries: &[DatasetEntry], query: &str) -> Vec<Candidate> {
    let q = key(query);
    if q.is_empty() {
        return Vec::new();
    }
    let exact = |e: &DatasetEntry| key(&e.name) == q || key(&e.slug) == q;
    let near = |e: &DatasetEntry| {
        let k = key(&e.name);
        k.len() >= 3 && q.len() >= 3 && (k.contains(&q) || q.contains(&k))
    };
    entries
        .iter()
        .filter(|e| exact(e))
        .chain(entries.iter().filter(|e| !exact(e) && near(e)))
        .take(DATASET_HITS)
        .map(|e| Candidate {
            name: e.name.clone(),
            url: e.thumb.clone(),
            thumb: e.thumb.clone(),
            width: None,
            height: None,
            format: "png",
            source: Source::Carlogos,
        })
        .collect()
}

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
///
/// An SVG downloads its original: a vector file has no size that matters. A
/// PNG downloads the rendition, which Commons gives at [`THUMB_WIDTH`] -
/// measured: a 5336x6669 original comes back at 500x625 - and points at the
/// original itself when that is smaller, while still reporting the asked
/// width: so the smaller of the two sizes is the one downloaded.
fn parse_files(root: &Value, source: Source) -> (Vec<Candidate>, Option<u32>) {
    let mut pages: Vec<&Value> = root["query"]["pages"].as_array().into_iter().flatten().collect();
    pages.sort_by_key(|p| p["index"].as_u64().unwrap_or(u64::MAX));
    let candidates = pages
        .into_iter()
        .filter_map(|p| {
            let info = p["imageinfo"].as_array()?.first()?;
            let thumb = info["thumburl"].as_str()?.to_string();
            let dim = |k: &str| info[k].as_u64().map(|v| v as u32);
            let (format, url, width, height) = match info["mime"].as_str()? {
                "image/svg+xml" => ("svg", info["url"].as_str()?.to_string(), None, None),
                "image/png" => {
                    let (w, h) = match (dim("width"), dim("height"), dim("thumbwidth"), dim("thumbheight")) {
                        (Some(w), Some(_), Some(tw), Some(th)) if tw < w => (tw, th),
                        (Some(w), Some(h), _, _) => (w, h),
                        _ => return None,
                    };
                    ("png", thumb.clone(), Some(w), Some(h))
                }
                _ => return None,
            };
            Some(Candidate {
                name: p["title"].as_str()?.trim_start_matches("File:").to_string(),
                url,
                thumb,
                width,
                height,
                format,
                source,
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

    /// TAXO§9.1: a Commons search reads in its `index` order and keeps SVG
    /// and PNG only. An SVG downloads its original and has no size to show;
    /// a big PNG downloads the 500 px rendition, a small one its original -
    /// the shapes measured on "Logo Renault F1.png" and "Logo Renault Sport
    /// F1.png".
    #[test]
    fn a_commons_search_keeps_svg_and_png_and_never_a_huge_original() {
        let original = "https://upload.wikimedia.org/x.png";
        let rendition = "https://thumb.wikimedia.org/500px-x.png";
        let root = json!({
            "continue": {"gsroffset": 30},
            "query": {"pages": [
                {"index": 4, "title": "File:Renault logo.svg", "imageinfo": [{
                    "url": "https://upload.wikimedia.org/r.svg", "thumburl": "https://thumb.wikimedia.org/r.png",
                    "width": 3780, "height": 5232, "thumbwidth": 500, "thumbheight": 692, "mime": "image/svg+xml"}]},
                {"index": 3, "title": "File:Dealer.jpg", "imageinfo": [{
                    "url": original, "thumburl": rendition, "width": 640, "height": 480, "mime": "image/jpeg"}]},
                {"index": 2, "title": "File:Logo Renault Sport F1.png", "imageinfo": [{
                    "url": original, "thumburl": original,
                    "width": 250, "height": 66, "thumbwidth": 500, "thumbheight": 132, "mime": "image/png"}]},
                {"index": 1, "title": "File:Logo Renault F1.png", "imageinfo": [{
                    "url": original, "thumburl": rendition,
                    "width": 5336, "height": 6669, "thumbwidth": 500, "thumbheight": 625, "mime": "image/png"}]}
            ]}
        });
        let (found, next) = parse_files(&root, Source::Commons);
        let names: Vec<&str> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Logo Renault F1.png", "Logo Renault Sport F1.png", "Renault logo.svg"],
            "index order, no JPEG"
        );
        assert_eq!(found[0].url, rendition, "a huge PNG downloads the rendition");
        assert_eq!((found[0].width, found[0].height), (Some(500), Some(625)));
        assert_eq!(
            (found[1].width, found[1].height),
            (Some(250), Some(66)),
            "a small one, its own size"
        );
        assert_eq!(found[2].format, "svg");
        assert_eq!(
            found[2].url, "https://upload.wikimedia.org/r.svg",
            "a vector file downloads itself"
        );
        assert_eq!(found[2].width, None, "and has no size to show");
        assert_eq!(next, Some(30));
        assert_eq!(
            parse_files(&json!({"query": {"pages": []}}), Source::Commons).1,
            None,
            "last page"
        );
    }

    /// TAXO§9.1: the dataset is matched on letters and digits, the exact name
    /// or slug first, then names one contains in the other - never on two
    /// letters, which would match half the makers.
    #[test]
    fn the_dataset_finds_a_maker_by_its_name() {
        let root = json!([
            {"name": "Renault Samsung", "slug": "renault-samsung", "image": {"thumb": "https://raw.githubusercontent.com/filippofilip95/car-logos-dataset/master/logos/thumb/renault-samsung.png"}},
            {"name": "Renault", "slug": "renault", "image": {"thumb": "https://raw.githubusercontent.com/filippofilip95/car-logos-dataset/master/logos/thumb/renault.png"}},
            {"name": "Mercedes-Benz", "slug": "mercedes-benz", "image": {"thumb": "t"}},
            {"name": "MG", "slug": "mg", "image": {"thumb": "t"}},
            {"name": "Broken"}
        ]);
        let entries = parse_dataset(&root);
        assert_eq!(entries.len(), 4, "an entry without an image is skipped");
        let names = |q: &str| {
            match_dataset(&entries, q)
                .into_iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names("renault"), vec!["Renault", "Renault Samsung"], "exact first");
        assert_eq!(names("Mercedes Benz"), vec!["Mercedes-Benz"], "punctuation aside");
        assert_eq!(names("Mercedes-Benz AMG"), vec!["Mercedes-Benz"], "a longer query");
        assert_eq!(names("MG"), vec!["MG"], "two letters: the exact name only");
        assert!(names("Tatuus").is_empty());
    }

    /// A downloaded raster is brought down to 512 px on its longest side,
    /// never under 64 on its shortest - the 5,000 px results asked at use.
    #[test]
    fn a_big_raster_is_brought_down_to_a_logo_size() {
        assert_eq!(shrunk_size(500, 625), Some((410, 512)));
        assert_eq!(shrunk_size(2048, 2048), Some((512, 512)));
        assert_eq!(shrunk_size(256, 256), None, "small enough already");
        assert_eq!(
            shrunk_size(2000, 140),
            Some((914, 64)),
            "a long wordmark keeps 64 px of height"
        );
        assert_eq!(shrunk_size(600, 50), None, "never enlarged");

        let dir = crate::testutil::temp_dir("logo-shrink");
        let path = dir.join("big.png");
        image::RgbaImage::new(1500, 900).save(&path).unwrap();
        shrink(&path);
        let img = image::open(&path).unwrap();
        assert_eq!((img.width(), img.height()), (512, 307), "shrunk in place");
    }

    /// The real thing, over the network - a probe, not a check (`--ignored
    /// --nocapture` to read): Renault's candidates, then the first one
    /// downloaded and adopted into a scratch folder.
    #[test]
    #[ignore = "network; probe, not a check"]
    fn probe_renault_online() {
        let page = search("Renault", 0).expect("reachable");
        for c in &page.candidates {
            println!("{:?} {} {:?}x{:?} {}", c.source, c.format, c.width, c.height, c.name);
        }
        println!("next: {:?}", page.next);
        let dir = crate::testutil::temp_dir("logo-probe");
        let first = page.candidates.first().expect("a candidate");
        adopt(&dir, "Renault", &first.url).expect("adopted");
        let pref = crate::logos::load_prefs(&dir).remove("Renault").expect("a choice");
        let file = dir.join(crate::logos::LOGOS_DIR).join(pref.custom.expect("his file"));
        let size = image::open(&file).map(|i| (i.width(), i.height())).ok();
        println!(
            "adopted {} ({} bytes, {size:?})",
            file.display(),
            std::fs::metadata(&file).unwrap().len()
        );
    }

    /// Only the sources' file hosts are downloaded from, and only a logo
    /// format.
    #[test]
    fn only_a_known_host_svg_or_png_is_downloaded() {
        let dir = crate::testutil::temp_dir("logo-adopt");
        assert_eq!(
            adopt(&dir, "Renault", "https://example.com/renault.svg").unwrap_err(),
            crate::errors::LOGO_DOWNLOAD_FAILED,
            "another host is refused before any request"
        );
        assert_eq!(
            adopt(
                &dir,
                "Renault",
                "https://raw.githubusercontent.com/someone/else/renault.png"
            )
            .unwrap_err(),
            crate::errors::LOGO_DOWNLOAD_FAILED,
            "on GitHub, the dataset only"
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
