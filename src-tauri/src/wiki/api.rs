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
    /// The parent entity of §5.3, when there is exactly one. **One level
    /// only**: this is never climbed again.
    pub parent: Option<String>,
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
fn single_parent(claims: &Value) -> Option<String> {
    for property in [ids::PART_OF, ids::SUBCLASS_OF] {
        let mut found: Vec<String> = Vec::new();
        for statement in claims.get(property).and_then(Value::as_array).into_iter().flatten() {
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
        match found.len() {
            0 => continue,
            1 => return found.into_iter().next(),
            n => {
                log::debug!("wiki/api: {property} has {n} values, no parent climb");
                return None;
            }
        }
    }
    None
}

/// Reads a `wbgetentities` answer. Pure — the tests feed it the real shape.
fn parse_entity(root: &Value, entity_id: &str) -> Fetched<EntityFacts> {
    let entity = &root["entities"][entity_id];
    if entity.is_null() || entity.get("missing").is_some() {
        return Fetched::Absent;
    }
    let mut titles = BTreeMap::new();
    if let Some(sitelinks) = entity.get("sitelinks").and_then(Value::as_object) {
        for (site, link) in sitelinks {
            let (Some(lang), Some(title)) = (site_to_lang(site), link["title"].as_str()) else {
                continue;
            };
            titles.insert(lang, title.to_string());
        }
    }
    if titles.is_empty() {
        // The entity exists but has no article anywhere: nothing to show, and
        // that is a durable fact rather than a failure.
        return Fetched::Absent;
    }
    Fetched::Found(EntityFacts {
        entity_id: entity_id.to_string(),
        titles,
        parent: single_parent(&entity["claims"]),
    })
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
        assert_eq!(
            facts.parent.as_deref(),
            Some("Q2626308"),
            "part of wins over subclass of"
        );
    }

    /// Rule (§5.3): "subclass of" is read only when "part of" says nothing —
    /// car generations are modelled both ways depending on the item.
    #[test]
    fn subclass_of_serves_when_part_of_is_absent() {
        let mut value = ae86_entity();
        value["entities"]["Q1377219"]["claims"]["P361"] = json!([]);

        let Fetched::Found(facts) = parse_entity(&value, "Q1377219") else {
            panic!("the entity exists");
        };
        assert_eq!(facts.parent.as_deref(), Some("Q5333841"), "falls back to P279");
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
