//! The Action API client (`docs/SPEC-wikipedia-fiche-detail.md` §6).
//!
//! Two requests, no more (§6.1): one to Wikidata to turn a Q-id into article
//! titles, a parent entity and the list of languages, one to the wiki of the
//! chosen language for the introduction extract. The Action API (`/w/api.php`)
//! rather than the REST ones, which are being retired.
//!
//! **Nothing here returns a `Result`.** A dead network, a timeout, a 404 and a
//! quota refusal are four non-results (§1): the tab is simply absent, and there
//! is no error for a caller to propagate or an interface to show. What the
//! caller does need to tell apart is "the thing does not exist" from "I could
//! not ask" — see `Fetched` — because only the first is worth remembering.
//!
//! The parsing is deliberately split out of the fetching (`parse_entity`,
//! `parse_article`): those two are pure, and they carry the shape of the real
//! responses, measured on `Q1377219` / `Toyota AE86` rather than assumed.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use super::http;
use super::ids;

/// Wikidata's own API host — entities live here, not on a language wiki.
const WIKIDATA_HOST: &str = "www.wikidata.org";

/// §6.2: five seconds, every phase included. A decorative tab has no business
/// making anyone wait longer than that.
const TIMEOUT_MS: i32 = 5_000;

/// §6.2: exponential backoff on 429, then a silent give-up. Three tries and
/// under two seconds of waiting in total — the expected traffic is a few dozen
/// requests a month (§6.3), so a quota refusal means something is wrong, not
/// that we should wait it out.
const RETRIES: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_millis(500);

/// `wbgetentities` takes at most 50 ids per call for an anonymous client.
/// Going over silently truncates, which would look like "those candidates have
/// no type" and drop them from the filter.
pub const MAX_BATCH: usize = 50;

/// Widest radius `list=geosearch` accepts, in metres. Asking for more is not
/// truncated — the request is **refused entirely**.
pub const MAX_GEOSEARCH_RADIUS_M: u32 = 10_000;

/// What came back from an API call. Three variants, all of them non-results in
/// the sense of §1 — none is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched<T> {
    /// The API answered and the thing exists.
    Found(T),
    /// The API answered: the thing does not exist (missing page, 404, empty
    /// extract). **Durable** — the negative cache may act on it.
    Absent,
    /// We could not ask: network, timeout, quota, unreadable answer. Nothing
    /// was learned, so nothing should be remembered from it.
    Unavailable,
}

impl<T> Fetched<T> {
    /// Collapses the distinction when the caller genuinely does not need it.
    pub fn found(self) -> Option<T> {
        match self {
            Fetched::Found(value) => Some(value),
            _ => None,
        }
    }
}

/// What Wikidata knows about an entity, reduced to what §5 needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFacts {
    pub entity_id: String,
    /// Language code to article title, from the entity's sitelinks.
    pub titles: BTreeMap<String, String>,
    /// `P31` of the entity itself — needed to judge whether a `P279` parent is
    /// of the same nature (see `parent_fallback`).
    pub types: Vec<String>,
    /// The parent entity of §5.3 by `part of`, when there is exactly one.
    /// **One level only**: this is never climbed again.
    pub parent: Option<String>,
    /// The `subclass of` candidate, kept **separate and unvalidated**.
    ///
    /// Measured on the AE86: its `P279` is "sport compact", a classification,
    /// not a generic model. So this one is only usable once fetched and found
    /// to be of the same nature as the entity — which is why it does not sit
    /// in `parent`. See `ids::SUBCLASS_OF` and `wiki::parent_to_climb`.
    pub parent_fallback: Option<String>,
}

/// Everything the matching of §4 reads off one candidate entity.
///
/// Fetched in batches: `wbgetentities` takes up to 50 ids at once, so a whole
/// list of candidates costs **one** request — which is what makes §4 fit in the
/// politeness budget of §6.2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityDetails {
    pub entity_id: String,
    /// English label and short description — the description is what §7.6 will
    /// show to disambiguate by eye ("modèle d'automobile Toyota, 1983–1987").
    pub label: Option<String>,
    pub description: Option<String>,
    /// `P31`, the type filter's input (§4.1.3, §4.2.2).
    pub types: Vec<String>,
    /// `P176`, the brand half of the car score.
    pub manufacturers: Vec<String>,
    pub part_of: Vec<String>,
    pub subclass_of: Vec<String>,
    /// `P580`/`P582`, the production period when it exists — measured as the
    /// only source for it (see `ids::START_TIME`).
    pub start_year: Option<i64>,
    pub end_year: Option<i64>,
    pub titles: BTreeMap<String, String>,
}

impl EntityDetails {
    /// Is this candidate of one of the accepted types (§4.1.3, §4.2.2)?
    pub fn is_of_type(&self, allowed: &[&str]) -> bool {
        self.types.iter().any(|t| allowed.contains(&t.as_str()))
    }
}

/// One hit of `wbsearchentities`, before anything is known about its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub entity_id: String,
    pub label: Option<String>,
    pub description: Option<String>,
}

/// One hit of Wikidata's geographic search, with the distance the API itself
/// computed — in metres, nearest first.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoHit {
    pub entity_id: String,
    pub distance_m: f64,
}

/// One article's introduction, as the API gives it — never reworded, never
/// summarised, never passed through a model (§2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleText {
    pub title: String,
    /// Canonical URL as served by the API. Stored with the text and never
    /// rebuilt afterwards (§3.2).
    pub url: String,
    pub revision_id: Option<i64>,
    pub extract: String,
    /// Languages this article exists in, for the selector (§5.4). Includes the
    /// language it was fetched in, which the API's interlanguage links leave
    /// out.
    pub available_langs: Vec<String>,
}

/// Holds the User-Agent and nothing else: WinHTTP opens and closes its handles
/// per request, so there is no connection to keep alive.
pub struct WikiClient {
    user_agent: String,
}

impl Default for WikiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WikiClient {
    /// §6.2: the User-Agent is mandatory and must identify the application —
    /// Wikimedia blocks requests without one. Name, version and a contact URL,
    /// which is the form their policy asks for.
    pub fn new() -> Self {
        Self {
            user_agent: format!(
                "PitBox/{} (+https://github.com/tmeedend/ac-pitbox)",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }

    /// Sitelinks and parent entity for a Q-id, in one `wbgetentities` call.
    ///
    /// This single request pays for three things at once: the titles the
    /// fallback chain needs (§5.2), the parent of §5.3, and the language list
    /// of §5.4.
    pub fn entity(&self, entity_id: &str) -> Fetched<EntityFacts> {
        // Validated before it reaches a URL. The format check is cheap here and
        // mandatory the day entries arrive by import (§10).
        if !is_entity_id(entity_id) {
            log::warn!("wiki/api: {entity_id:?} is not a Wikidata id");
            return Fetched::Absent;
        }
        let path = format!(
            "/w/api.php?action=wbgetentities&ids={}&props=sitelinks%7Cclaims&format=json&formatversion=2",
            http::encode_query_value(entity_id)
        );
        match self.get_json(WIKIDATA_HOST, &path) {
            Fetched::Found(root) => parse_entity(&root, entity_id),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// The introduction extract of one article, in one `action=query` call
    /// (§6.1): plain text, canonical URL, revision number and interlanguage
    /// links together.
    pub fn article(&self, lang: &str, title: &str) -> Fetched<ArticleText> {
        let path = format!(
            "/w/api.php?action=query&format=json&formatversion=2\
             &prop=extracts%7Cinfo%7Clanglinks&inprop=url&exintro=1&explaintext=1&lllimit=max&redirects=1&titles={}",
            http::encode_query_value(title)
        );
        match self.get_json(&format!("{lang}.wikipedia.org"), &path) {
            Fetched::Found(root) => parse_article(&root, lang),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// Free-text entity search (§4.1.1, and the manual correction of §7.6).
    ///
    /// `wbsearchentities` is the API's own name lookup: it returns ids, labels
    /// and short descriptions, but **no types** — the filtering of §4.1.3 needs
    /// `details` afterwards. Two requests per name, which is the floor.
    pub fn search(&self, query: &str, lang: &str, limit: u32) -> Fetched<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            // Searching for nothing is not a network question (§4.3: a name
            // made only of noise cleans down to empty).
            return Fetched::Absent;
        }
        let path = format!(
            "/w/api.php?action=wbsearchentities&search={}&language={}&uselang={}&type=item&limit={limit}\
             &format=json&formatversion=2",
            http::encode_query_value(query),
            http::encode_query_value(lang),
            http::encode_query_value(lang),
        );
        match self.get_json(WIKIDATA_HOST, &path) {
            Fetched::Found(root) => parse_search(&root),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// Candidate entities from **Wikipedia's full-text search** (§4.1.1).
    ///
    /// Not `wbsearchentities`, and this is measured rather than preferred: that
    /// endpoint matches labels and aliases from the **start of the string**, so
    /// `"BMW M3 E30"` returns *nothing at all* — no item is labelled that, the
    /// item is called `BMW M3`. Every mod name carrying a generation, a step or
    /// a year therefore found zero candidates, which was most of the library.
    ///
    /// A full-text search returns what a person gets in their browser: for
    /// `"BMW M3 E30"`, `BMW M3` first; for `"Abarth 500 Assetto Corse"` — a
    /// variant with no article of its own — the `Abarth 500` page that
    /// describes it in a section. Which is exactly what §4.1 wants, since §5.3
    /// accepts the generic model anyway.
    ///
    /// One request: `generator=search` carries `pageprops` along, so the Q-ids
    /// come back with the titles.
    pub fn search_pages(&self, lang: &str, query: &str, limit: u32) -> Fetched<Vec<SearchHit>> {
        let query = query.trim();
        if query.is_empty() {
            return Fetched::Absent;
        }
        let path = format!(
            "/w/api.php?action=query&format=json&formatversion=2&generator=search\
             &gsrsearch={}&gsrlimit={limit}&gsrnamespace=0&prop=pageprops&ppprop=wikibase_item",
            http::encode_query_value(query),
        );
        match self.get_json(&format!("{lang}.wikipedia.org"), &path) {
            Fetched::Found(root) => parse_search_pages(&root),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// Full details for up to `MAX_BATCH` entities in one request.
    pub fn details(&self, ids: &[String]) -> Fetched<Vec<EntityDetails>> {
        if ids.is_empty() {
            return Fetched::Absent;
        }
        let ids: Vec<&str> = ids
            .iter()
            .take(MAX_BATCH)
            .map(String::as_str)
            .filter(|id| is_entity_id(id))
            .collect();
        if ids.is_empty() {
            log::warn!("wiki/api: aucun id valide dans le lot demandé");
            return Fetched::Absent;
        }
        let path = format!(
            "/w/api.php?action=wbgetentities&ids={}&props=labels%7Cdescriptions%7Cclaims%7Csitelinks\
             &languages=en%7Cfr&format=json&formatversion=2",
            http::encode_query_value(&ids.join("|")),
        );
        match self.get_json(WIKIDATA_HOST, &path) {
            Fetched::Found(root) => parse_details(&root),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// Entities within `radius_m` of a point, nearest first (§4.2.1).
    ///
    /// **On Wikidata, not on a language wiki**, and that is a correction to the
    /// spec rather than a shortcut: the English article "Nürburgring" carries
    /// no GeoData coordinates at all (nor does Suzuka's), so a `geosearch`
    /// against en.wikipedia can never return the very circuit §4.2 uses as its
    /// example. The Wikidata item does carry `P625`, and searching there
    /// returns Q-ids directly — one request less, and it works.
    pub fn geosearch(&self, latitude: f64, longitude: f64, radius_m: u32, limit: u32) -> Fetched<Vec<GeoHit>> {
        // **The API caps this at ten kilometres**, and says so by refusing the
        // whole request: `"The value \"25000\" for parameter \"gsradius\" must
        // be between 10 and 10,000."` A wider setting used to turn every single
        // track into "network unavailable" — twenty-four of them, all at once,
        // which is what a systematic failure looks like next to a real outage.
        // Clamped here rather than validated in the settings: this is the
        // API's limit, and this is the only place that knows it.
        let radius_m = radius_m.clamp(10, MAX_GEOSEARCH_RADIUS_M);
        let path = format!(
            "/w/api.php?action=query&format=json&formatversion=2&list=geosearch\
             &gscoord={latitude}%7C{longitude}&gsradius={radius_m}&gslimit={limit}",
        );
        match self.get_json(WIKIDATA_HOST, &path) {
            Fetched::Found(root) => parse_geosearch(&root),
            Fetched::Absent => Fetched::Absent,
            Fetched::Unavailable => Fetched::Unavailable,
        }
    }

    /// One GET, decoded as JSON, with the backoff of §6.2.
    ///
    /// A transport failure is **not** retried: it already cost a five-second
    /// timeout, and trying again would only make the fiche wait longer for
    /// something it does not need. Only an answer that explicitly says "later"
    /// (429) or "not now" (5xx) is worth a second try.
    fn get_json(&self, host: &str, path: &str) -> Fetched<Value> {
        for attempt in 0..RETRIES {
            let Some(response) = http::get(host, path, &self.user_agent, TIMEOUT_MS) else {
                return Fetched::Unavailable;
            };
            match response.status {
                200 => {
                    return match serde_json::from_slice::<Value>(&response.body) {
                        Ok(value) => Fetched::Found(value),
                        Err(e) => {
                            log::warn!("wiki/api: {host} answered something that is not JSON — {e}");
                            Fetched::Unavailable
                        }
                    }
                }
                404 => return Fetched::Absent,
                429 | 500..=599 => {
                    if attempt + 1 < RETRIES {
                        std::thread::sleep(BACKOFF_BASE * 2u32.pow(attempt));
                        continue;
                    }
                    log::warn!(
                        "wiki/api: {host} still answering {} after {RETRIES} tries",
                        response.status
                    );
                    return Fetched::Unavailable;
                }
                other => {
                    log::warn!("wiki/api: {host} answered {other}");
                    return Fetched::Unavailable;
                }
            }
        }
        Fetched::Unavailable
    }
}

/// A Wikidata item id: `Q` and digits, nothing else.
///
/// Kept strict on purpose — this string is concatenated into a URL, and §10
/// will hand the same check arbitrary third-party content at import time.
pub fn is_entity_id(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('Q') else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Language code behind a sitelink key: `"frwiki"` gives `"fr"`.
///
/// Sitelinks cover the whole Wikimedia estate, not just Wikipedia: the AE86's
/// entity carries `commonswiki` beside `enwiki`, `frwiki` and `jawiki`
/// (measured). Keys for the other projects end in something else
/// (`frwikiquote`, `enwikisource`), so requiring a `wiki` suffix filters them
/// out; what it does not filter is the projects whose *name* ends in wiki, and
/// those are listed below.
fn site_to_lang(site: &str) -> Option<String> {
    /// Wikimedia sites whose key looks exactly like a language one. None of
    /// them is a language edition of Wikipedia.
    const NOT_LANGUAGES: [&str; 9] = [
        "commons",
        "species",
        "meta",
        "wikidata",
        "mediawiki",
        "incubator",
        "outreach",
        "foundation",
        "sources",
    ];
    let code = site.strip_suffix("wiki")?;
    if code.is_empty() || NOT_LANGUAGES.contains(&code) {
        return None;
    }
    // Sitelink keys spell a variant with an underscore where the hostname uses
    // a dash: `be_x_old`, `zh_yue`.
    Some(code.replace('_', "-"))
}

/// The parent entity of §5.3, or nothing.
///
/// "part of" first, "subclass of" as a fallback. **Exactly one** candidate or
/// none: several parents is an ambiguity, and §1 settles ambiguity by showing
/// nothing rather than drawing lots. Deprecated statements are skipped — that
/// rank exists precisely to mean "this was wrong".
/// Every distinct entity id a property points at, deprecated statements and
/// valueless snaks skipped. Measured shape:
/// `claims.<P>[i].mainsnak.datavalue.value.id`.
fn claim_ids(claims: &Value, property: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for statement in claims.get(property).and_then(Value::as_array).into_iter().flatten() {
        // That rank exists precisely to mean "this was wrong".
        if statement.get("rank").and_then(Value::as_str) == Some("deprecated") {
            continue;
        }
        let snak = &statement["mainsnak"];
        // "novalue" and "somevalue" snaks carry no id at all.
        if snak.get("snaktype").and_then(Value::as_str) != Some("value") {
            continue;
        }
        let Some(id) = snak["datavalue"]["value"]["id"].as_str() else {
            continue;
        };
        if !found.iter().any(|seen| seen == id) {
            found.push(id.to_string());
        }
    }
    found
}

/// The year of a time-valued property. Wikidata writes `+1986-00-00T00:00:00Z`
/// — a leading sign, and zeroed month and day when only the year is known
/// (measured on Q1377219), so this cannot go through a date parser.
fn claim_year(claims: &Value, property: &str) -> Option<i64> {
    let statement = claims.get(property).and_then(Value::as_array)?.first()?;
    let time = statement["mainsnak"]["datavalue"]["value"]["time"].as_str()?;
    time.trim_start_matches(['+', '-']).get(..4)?.parse().ok()
}

/// **Exactly one** value or nothing: several parents is an ambiguity, and §1
/// settles ambiguity by showing nothing rather than drawing lots.
fn single(ids: Vec<String>, property: &str) -> Option<String> {
    match ids.len() {
        1 => ids.into_iter().next(),
        0 => None,
        n => {
            log::debug!("wiki/api: {property} has {n} values, no parent climb");
            None
        }
    }
}

fn sitelink_titles(entity: &Value) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    if let Some(sitelinks) = entity.get("sitelinks").and_then(Value::as_object) {
        for (site, link) in sitelinks {
            let (Some(lang), Some(title)) = (site_to_lang(site), link["title"].as_str()) else {
                continue;
            };
            titles.insert(lang, title.to_string());
        }
    }
    titles
}

/// Everything about one entity, from the `entities.<id>` object.
fn details_of(entity: &Value, entity_id: &str) -> EntityDetails {
    let claims = &entity["claims"];
    EntityDetails {
        entity_id: entity_id.to_string(),
        label: entity["labels"]["en"]["value"].as_str().map(str::to_string),
        description: entity["descriptions"]["en"]["value"].as_str().map(str::to_string),
        types: claim_ids(claims, ids::INSTANCE_OF),
        manufacturers: claim_ids(claims, ids::MANUFACTURER),
        part_of: claim_ids(claims, ids::PART_OF),
        subclass_of: claim_ids(claims, ids::SUBCLASS_OF),
        start_year: claim_year(claims, ids::START_TIME),
        end_year: claim_year(claims, ids::END_TIME),
        titles: sitelink_titles(entity),
    }
}

/// Reads a `wbgetentities` answer for one entity. Pure — the tests feed it the
/// real shape.
fn parse_entity(root: &Value, entity_id: &str) -> Fetched<EntityFacts> {
    let entity = &root["entities"][entity_id];
    if entity.is_null() || entity.get("missing").is_some() {
        return Fetched::Absent;
    }
    let details = details_of(entity, entity_id);
    if details.titles.is_empty() {
        // The entity exists but has no article anywhere: nothing to show, and
        // that is a durable fact rather than a failure.
        return Fetched::Absent;
    }
    Fetched::Found(EntityFacts {
        entity_id: entity_id.to_string(),
        titles: details.titles,
        types: details.types,
        parent: single(details.part_of, ids::PART_OF),
        parent_fallback: single(details.subclass_of, ids::SUBCLASS_OF),
    })
}

/// Reads a batched `wbgetentities` answer. Entities that came back missing are
/// dropped rather than failing the batch: one dead id among fifty candidates
/// is a normal thing, not a reason to lose the other forty-nine.
fn parse_details(root: &Value) -> Fetched<Vec<EntityDetails>> {
    let Some(entities) = root["entities"].as_object() else {
        log::warn!("wiki/api: a batch answer without an entities object");
        return Fetched::Unavailable;
    };
    let found: Vec<EntityDetails> = entities
        .iter()
        .filter(|(_, entity)| entity.get("missing").is_none())
        .map(|(id, entity)| details_of(entity, id))
        .collect();
    if found.is_empty() {
        Fetched::Absent
    } else {
        Fetched::Found(found)
    }
}

/// Reads a `wbsearchentities` answer.
fn parse_search(root: &Value) -> Fetched<Vec<SearchHit>> {
    let Some(hits) = root["search"].as_array() else {
        log::warn!("wiki/api: a search answer without a search array");
        return Fetched::Unavailable;
    };
    let found: Vec<SearchHit> = hits
        .iter()
        .filter_map(|hit| {
            let id = hit["id"].as_str()?;
            // Properties and lexemes can surface on a search; only items can
            // be an appariement.
            is_entity_id(id).then(|| SearchHit {
                entity_id: id.to_string(),
                label: hit["label"].as_str().map(str::to_string),
                description: hit["description"].as_str().map(str::to_string),
            })
        })
        .collect();
    if found.is_empty() {
        Fetched::Absent
    } else {
        Fetched::Found(found)
    }
}

/// Reads a `generator=search` answer: article titles with their Wikidata ids,
/// kept in the search engine's own relevance order (`index`).
///
/// A page without a `wikibase_item` is dropped — it cannot be an appariement,
/// and there is nothing to score it against.
fn parse_search_pages(root: &Value) -> Fetched<Vec<SearchHit>> {
    let Some(pages) = root["query"]["pages"].as_array() else {
        // No `query` at all is how this API says "no result", not a failure.
        return Fetched::Absent;
    };
    let mut ranked: Vec<(i64, SearchHit)> = pages
        .iter()
        .filter_map(|page| {
            let id = page["pageprops"]["wikibase_item"].as_str()?;
            is_entity_id(id).then(|| {
                (
                    page["index"].as_i64().unwrap_or(i64::MAX),
                    SearchHit {
                        entity_id: id.to_string(),
                        label: page["title"].as_str().map(str::to_string),
                        description: None,
                    },
                )
            })
        })
        .collect();
    if ranked.is_empty() {
        return Fetched::Absent;
    }
    ranked.sort_by_key(|(index, _)| *index);
    Fetched::Found(ranked.into_iter().map(|(_, hit)| hit).collect())
}

/// Reads a `list=geosearch` answer. On Wikidata the `title` of a result **is**
/// the Q-id, which is the whole reason the search runs there (measured).
fn parse_geosearch(root: &Value) -> Fetched<Vec<GeoHit>> {
    let Some(hits) = root["query"]["geosearch"].as_array() else {
        log::warn!("wiki/api: a geosearch answer without query.geosearch");
        return Fetched::Unavailable;
    };
    let found: Vec<GeoHit> = hits
        .iter()
        .filter_map(|hit| {
            let id = hit["title"].as_str()?;
            is_entity_id(id).then(|| GeoHit {
                entity_id: id.to_string(),
                distance_m: hit["dist"].as_f64().unwrap_or(f64::MAX),
            })
        })
        .collect();
    if found.is_empty() {
        Fetched::Absent
    } else {
        Fetched::Found(found)
    }
}

/// Reads an `action=query` answer. Pure — same reason as `parse_entity`.
fn parse_article(root: &Value, lang: &str) -> Fetched<ArticleText> {
    // `formatversion=2` makes `pages` an array (measured); the first entry is
    // the only one, since a single title is asked for.
    let Some(page) = root["query"]["pages"].as_array().and_then(|pages| pages.first()) else {
        log::warn!("wiki/api: an answer without query.pages");
        return Fetched::Unavailable;
    };
    if page.get("missing").is_some() {
        return Fetched::Absent;
    }
    let (Some(title), Some(url)) = (page["title"].as_str(), page["fullurl"].as_str()) else {
        // Both are asked for explicitly; missing means the API changed shape,
        // which is not something to store as "this article does not exist".
        log::warn!("wiki/api: a page without a title or a url");
        return Fetched::Unavailable;
    };
    let extract = page["extract"].as_str().unwrap_or_default().trim();
    if extract.is_empty() {
        // A page with no introduction (a disambiguation page, a redirect that
        // lost its target) has nothing to show: precision over recall (§1).
        return Fetched::Absent;
    }

    let mut available_langs = vec![lang.to_string()];
    for link in page["langlinks"].as_array().into_iter().flatten() {
        if let Some(code) = link["lang"].as_str() {
            if !available_langs.iter().any(|seen| seen == code) {
                available_langs.push(code.to_string());
            }
        }
    }

    Fetched::Found(ArticleText {
        title: title.to_string(),
        url: url.to_string(),
        revision_id: page["lastrevid"].as_i64(),
        extract: extract.to_string(),
        available_langs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Rule (§3.1, §10): a Q-id is `Q` and digits. Everything else — a URL
    /// above all — is refused before it can reach a request.
    #[test]
    fn only_a_q_id_passes_for_an_entity() {
        assert!(is_entity_id("Q1377219"));
        assert!(!is_entity_id("Q"), "no digits");
        assert!(!is_entity_id("q1377219"), "lowercase is not an id");
        assert!(!is_entity_id("P361"), "a property is not an item");
        assert!(
            !is_entity_id("https://fr.wikipedia.org/wiki/Toyota_AE86"),
            "a URL is never stored as an id (§3.1)"
        );
        assert!(!is_entity_id("Q123&action=delete"), "nothing that changes a query");
    }

    /// Rule (§5.4): sitelinks describe the whole Wikimedia estate; only the
    /// Wikipedia language editions are languages. `commonswiki` sits next to
    /// `frwiki` on the real entity, which is how this was found.
    #[test]
    fn only_wikipedia_language_editions_count_as_languages() {
        assert_eq!(site_to_lang("frwiki").as_deref(), Some("fr"));
        assert_eq!(site_to_lang("jawiki").as_deref(), Some("ja"));
        assert_eq!(site_to_lang("commonswiki"), None, "Commons is not a language");
        assert_eq!(site_to_lang("specieswiki"), None);
        assert_eq!(site_to_lang("frwikiquote"), None, "another project entirely");
        assert_eq!(site_to_lang("enwikisource"), None);
        assert_eq!(
            site_to_lang("zh_yuewiki").as_deref(),
            Some("zh-yue"),
            "variant keeps its dash"
        );
    }

    fn ae86_entity() -> Value {
        // The shape measured on Q1377219, trimmed to what is read.
        json!({
            "entities": {
                "Q1377219": {
                    "sitelinks": {
                        "enwiki": { "site": "enwiki", "title": "Toyota AE86" },
                        "frwiki": { "site": "frwiki", "title": "Toyota Sprinter Trueno" },
                        "jawiki": { "site": "jawiki", "title": "トヨタ・AE86" },
                        "commonswiki": { "site": "commonswiki", "title": "Category:Toyota AE86" }
                    },
                    "claims": {
                        "P31": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q3231690" } } }, "rank": "normal" }],
                        "P279": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q5333841" } } }, "rank": "normal" }],
                        "P361": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q2626308" } } }, "rank": "normal" }]
                    }
                }
            }
        })
    }

    /// Rule (§5.2, §5.3): one entity answer yields the titles the chain needs
    /// and the single parent to climb to.
    #[test]
    fn an_entity_answer_yields_titles_and_one_parent() {
        let Fetched::Found(facts) = parse_entity(&ae86_entity(), "Q1377219") else {
            panic!("the entity exists");
        };
        assert_eq!(
            facts.titles.get("fr").map(String::as_str),
            Some("Toyota Sprinter Trueno")
        );
        assert_eq!(facts.titles.get("en").map(String::as_str), Some("Toyota AE86"));
        assert!(!facts.titles.contains_key("commons"), "Commons is not an article");
        assert_eq!(facts.parent.as_deref(), Some("Q2626308"), "part of is the parent");
        assert_eq!(facts.types, vec!["Q3231690"], "its own type, to judge a P279 parent");
    }

    /// Rule (§5.3, measured): **a `subclass of` target is not a parent until it
    /// has been checked.**
    ///
    /// The AE86's P279 is `Q5333841` = "sport compact", a classification. It
    /// therefore lands in `parent_fallback`, never in `parent` — climbing it
    /// blindly would offer an article about a car *category* as the mod's
    /// "article général".
    #[test]
    fn a_classification_never_lands_in_parent() {
        let mut value = ae86_entity();
        value["entities"]["Q1377219"]["claims"]["P361"] = json!([]);

        let Fetched::Found(facts) = parse_entity(&value, "Q1377219") else {
            panic!("the entity exists");
        };
        assert_eq!(facts.parent, None, "no part of, so no confirmed parent");
        assert_eq!(
            facts.parent_fallback.as_deref(),
            Some("Q5333841"),
            "kept as a candidate the caller must type-check"
        );
    }

    /// Rule (§1, §5.3): two parents is an ambiguity, and an ambiguity shows
    /// nothing rather than picking one.
    #[test]
    fn two_parents_produce_no_parent_at_all() {
        let mut value = ae86_entity();
        value["entities"]["Q1377219"]["claims"]["P361"] = json!([
            { "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q2626308" } } }, "rank": "normal" },
            { "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q999999" } } }, "rank": "normal" }
        ]);

        let Fetched::Found(facts) = parse_entity(&value, "Q1377219") else {
            panic!("the entity exists");
        };
        assert_eq!(facts.parent, None, "no drawing of lots");
    }

    /// Rule (§1): an unknown entity is `Absent`, never an error — and never
    /// confused with a network failure.
    #[test]
    fn an_unknown_entity_is_absent() {
        let value = json!({ "entities": { "Q404": { "id": "Q404", "missing": "" } } });
        assert_eq!(parse_entity(&value, "Q404"), Fetched::Absent);
    }

    /// Rule (§6.1): one article answer carries everything the cache row needs —
    /// text, url, revision and the languages for the selector.
    #[test]
    fn an_article_answer_fills_a_cache_row() {
        // The shape measured on en.wikipedia.org for "Toyota AE86".
        let value = json!({
            "query": {
                "pages": [{
                    "title": "Toyota AE86",
                    "fullurl": "https://en.wikipedia.org/wiki/Toyota_AE86",
                    "lastrevid": 1373212308i64,
                    "extract": "The AE86 series of the Toyota Corolla Levin are small, front-engine cars.",
                    "langlinks": [
                        { "lang": "fr", "title": "Toyota Sprinter Trueno" },
                        { "lang": "ja", "title": "トヨタ・AE86" }
                    ]
                }]
            }
        });

        let Fetched::Found(article) = parse_article(&value, "en") else {
            panic!("the article exists");
        };
        assert_eq!(article.title, "Toyota AE86");
        assert_eq!(
            article.url, "https://en.wikipedia.org/wiki/Toyota_AE86",
            "url stored, never rebuilt"
        );
        assert_eq!(article.revision_id, Some(1373212308));
        assert!(
            article.extract.starts_with("The AE86 series"),
            "text kept verbatim (§2)"
        );
        assert_eq!(
            article.available_langs,
            vec!["en", "fr", "ja"],
            "the fetched language is not in the interlanguage links, and must be added"
        );
    }

    /// Rule (§1): a missing page and a page with no introduction are the same
    /// non-result — there is nothing to show either way.
    #[test]
    fn a_missing_page_and_an_empty_extract_are_both_absent() {
        let missing = json!({ "query": { "pages": [{ "title": "Nope", "missing": true }] } });
        assert_eq!(parse_article(&missing, "fr"), Fetched::Absent);

        let empty = json!({
            "query": { "pages": [{ "title": "Nope", "fullurl": "https://fr.wikipedia.org/wiki/Nope", "extract": "  " }] }
        });
        assert_eq!(parse_article(&empty, "fr"), Fetched::Absent, "nothing to display");
    }

    /// Rule (§4.1.4, measured): the production period is read off `P580`/`P582`,
    /// whose time strings carry a sign and zeroed month and day.
    #[test]
    fn a_production_period_is_read_from_its_time_strings() {
        let value = json!({
            "entities": { "Q1377219": { "claims": {
                "P580": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "time": "+1986-00-00T00:00:00Z" } } } }],
                "P582": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "time": "+1989-00-00T00:00:00Z" } } } }]
            }}}
        });
        let Fetched::Found(details) = parse_details(&value) else {
            panic!("one entity came back");
        };
        assert_eq!(details[0].start_year, Some(1986));
        assert_eq!(details[0].end_year, Some(1989));
    }

    /// Rule (§4): a batch drops what came back missing and keeps the rest —
    /// one dead id among the candidates is normal, not a failed batch.
    #[test]
    fn a_batch_drops_the_missing_and_keeps_the_rest() {
        let value = json!({
            "entities": {
                "Q1": { "claims": { "P31": [{ "mainsnak": { "snaktype": "value", "datavalue": { "value": { "id": "Q3231690" } } } }] } },
                "Q404": { "id": "Q404", "missing": "" }
            }
        });
        let Fetched::Found(details) = parse_details(&value) else {
            panic!("Q1 came back");
        };
        assert_eq!(details.len(), 1, "only the living one");
        assert!(
            details[0].is_of_type(&crate::wiki::ids::CAR_TYPES),
            "and it is a car model"
        );
    }

    /// Rule (§4.1.1, measured): the candidate search is Wikipedia's full-text
    /// one, and it answers a name carrying a generation.
    ///
    /// `wbsearchentities("BMW M3 E30")` comes back **empty** — it matches
    /// labels from the start of the string, and no item is called that. The
    /// full-text search returns `BMW M3` first, which is the answer §4.1 wants.
    #[test]
    fn a_full_text_search_keeps_the_engines_own_order() {
        let value = json!({
            "query": { "pages": [
                { "index": 3, "title": "BMW 3 Series", "pageprops": { "wikibase_item": "Q466066" } },
                { "index": 1, "title": "BMW M3", "pageprops": { "wikibase_item": "Q796579" } },
                { "index": 2, "title": "BMW 3 Series (E30)", "pageprops": { "wikibase_item": "Q838837" } },
                { "index": 4, "title": "Une page sans entité" }
            ]}
        });
        let Fetched::Found(hits) = parse_search_pages(&value) else {
            panic!("three pages carry an entity");
        };
        assert_eq!(hits.len(), 3, "une page sans Q-id n'est pas un candidat");
        assert_eq!(hits[0].entity_id, "Q796579", "l'ordre de pertinence est rétabli");
        assert_eq!(hits[0].label.as_deref(), Some("BMW M3"));
    }

    /// Rule (§1): a search that finds nothing is `Absent` — a non-result, never
    /// an error.
    #[test]
    fn a_search_without_results_is_absent() {
        assert_eq!(parse_search_pages(&json!({ "batchcomplete": true })), Fetched::Absent);
    }

    /// Rule (§4.2.1, measured): on Wikidata a geosearch result's `title` is the
    /// Q-id itself, and `dist` is in metres, nearest first.
    #[test]
    fn a_geosearch_result_is_a_q_id_and_a_distance() {
        let value = json!({
            "query": { "geosearch": [
                { "title": "Q152207", "dist": 4.9, "lat": 50.33, "lon": 6.94 },
                { "title": "Q8069", "dist": 120.0, "lat": 50.33, "lon": 6.94 },
                { "title": "Property:P625", "dist": 1.0 }
            ]}
        });
        let Fetched::Found(hits) = parse_geosearch(&value) else {
            panic!("two items came back");
        };
        assert_eq!(hits.len(), 2, "what is not an item is not a candidate");
        assert_eq!(hits[0].entity_id, "Q152207");
        assert_eq!(hits[0].distance_m, 4.9, "metres, as the API computed them");
    }

    /// Rule (§1): a shape we do not recognise is `Unavailable`, not `Absent` —
    /// an API change must not be written into the negative cache as "this mod
    /// has no article" for ninety days.
    #[test]
    fn an_unrecognisable_answer_is_unavailable_not_absent() {
        let value = json!({ "batchcomplete": true });
        assert_eq!(parse_article(&value, "fr"), Fetched::Unavailable);
    }

    /// **Smoke test, run by hand.** Everything above proves the parsing; this
    /// is the only thing that proves the WinHTTP client underneath it — TLS,
    /// the User-Agent, the query string, the read loop. Ignored so that no test
    /// ever depends on Wikipedia being up (§11), and kept because FFI that has
    /// never run is FFI nobody has checked.
    ///
    /// ```text
    /// cargo test --lib wiki -- --ignored --nocapture talks_to_wikipedia
    /// ```
    ///
    /// Expected: the French title of Q1377219 is `Toyota Sprinter Trueno`, its
    /// parent is Q2626308, and the article comes back with an extract and a
    /// revision number.
    #[test]
    #[ignore = "hits Wikipedia for real; a smoke test, not a check"]
    fn talks_to_wikipedia_for_real() {
        let client = WikiClient::new();

        let Fetched::Found(facts) = client.entity("Q1377219") else {
            panic!("Wikidata unreachable or shape changed");
        };
        eprintln!("parent   {:?}", facts.parent);
        eprintln!("langs    {}", facts.titles.len());
        eprintln!("fr       {:?}", facts.titles.get("fr"));

        let title = facts.titles.get("fr").expect("a French article");
        let Fetched::Found(article) = client.article("fr", title) else {
            panic!("fr.wikipedia.org unreachable or shape changed");
        };
        eprintln!("title    {}", article.title);
        eprintln!("url      {}", article.url);
        eprintln!("revision {:?}", article.revision_id);
        eprintln!("langs    {}", article.available_langs.len());
        eprintln!("extract  {}…", article.extract.chars().take(120).collect::<String>());

        assert!(!article.extract.is_empty(), "an extract came back");
        assert!(
            article.url.starts_with("https://fr.wikipedia.org/"),
            "the French wiki answered"
        );
    }
}
