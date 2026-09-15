//! Choosing the article by hand (WIKI§7.6).
//!
//! The spec put this in a context menu of the tab, reachable only once an
//! article was already showing. Decided with the user and changed: **the tab is
//! permanent and carries this**, because the case where choosing helps most is
//! precisely the one where nothing was found — and an absent tab offered
//! nowhere to go. An empty tab that proposes something is not the "encart
//! grisé" WIKI§1 forbids; it is an offer, and it carries no alarm.
//!
//! Two things are deliberately looser here than in the automatic matching:
//!
//! - **No type filter.** WIKI§7.6 says so outright: if someone wants to tie their
//!   mod to an entity outside the taxonomy, the taxonomy is likelier to be
//!   wrong than they are.
//! - **No score, no threshold.** A person is reading the list; the Wikidata
//!   short description ("automobile produced by Toyota") settles at a glance
//!   what a number never could.

use serde::Serialize;

use super::api::{Fetched, WikiClient};

/// One line of the search results.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub entity_id: String,
    pub label: String,
    /// Wikidata's own short description — the thing that tells a Corolla from a
    /// Corolla in one glance.
    pub description: Option<String>,
}

/// Free search for the correction panel: what a person would get in a search
/// box, minus the type filter.
pub fn search(
    net: &WikiClient,
    cleaner: &super::clean::Cleaner,
    query: &str,
    locale: &str,
    limit: u32,
) -> Vec<Suggestion> {
    // The raw text is searched as typed. It is *not* put through the cleaning
    // rules: the user is correcting precisely because the automatic guess was
    // wrong, and silently rewriting what they typed would hide why.
    let _ = cleaner;
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    for wiki in super::lang::search_order(locale) {
        let hits = match net.search_pages(&wiki, query, limit) {
            Fetched::Found(hits) => hits,
            Fetched::Absent => continue,
            Fetched::Unavailable => return Vec::new(),
        };
        let ids: Vec<String> = hits.iter().map(|h| h.entity_id.clone()).collect();
        let Fetched::Found(mut details) = net.details(&ids) else {
            continue;
        };
        super::borrow_labels(&mut details, &hits);
        // The search engine's own order is kept: it ranked by relevance to what
        // the user typed, which is exactly the question being asked.
        let mut out: Vec<Suggestion> = Vec::new();
        for hit in &hits {
            let Some(d) = details.iter().find(|d| d.entity_id == hit.entity_id) else {
                continue;
            };
            out.push(Suggestion {
                entity_id: d.entity_id.clone(),
                label: d
                    .label
                    .clone()
                    .or_else(|| hit.label.clone())
                    .unwrap_or_else(|| d.entity_id.clone()),
                description: d.description.clone(),
            });
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

/// Language and title of a Wikipedia article URL, or nothing.
///
/// **The host check is an allowlist and it is ASCII-only**, which is what WIKI§10
/// asks for: `wikipedıa.org` — with a dotless Turkish ı — looks identical in a
/// proof-reading and is a different domain entirely. Refusing everything whose
/// host is not plain ASCII `<lang>.wikipedia.org` closes that door without
/// having to enumerate lookalikes.
///
/// Pure, so WIKI§11's "rejet des domaines non-Wikipédia, y compris homographes" is
/// testable without a network.
pub fn parse_wikipedia_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    // No credentials, no port, no punycode: anything unusual is refused rather
    // than interpreted.
    if !host.is_ascii() || host.contains('@') || host.contains(':') {
        return None;
    }
    let lang = host.strip_suffix(".wikipedia.org")?;
    if lang.is_empty() || !lang.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') {
        return None;
    }
    let title = path.strip_prefix("wiki/")?;
    // A query string or a fragment is not part of the title.
    let title = title.split(['?', '#']).next().unwrap_or_default();
    if title.is_empty() {
        return None;
    }
    Some((lang.to_string(), percent_decode(title).replace('_', " ")))
}

/// Undoes the percent-encoding of a URL path segment. Titles arrive encoded
/// when pasted from a browser (`Toyota_Sprinter_Trueno`, `%C3%9C`).
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The entity behind a pasted Wikipedia URL (WIKI§7.6).
///
/// The URL itself is **never stored** (WIKI§3.1): it is resolved to a Q-id and
/// thrown away, because the Q-id is language-independent and survives an
/// article being renamed.
pub fn entity_from_url(net: &WikiClient, url: &str) -> Option<String> {
    let (lang, title) = parse_wikipedia_url(url)?;
    net.entity_of_page(&lang, &title)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (WIKI§7.6): a URL copied from the browser gives its language and title.
    #[test]
    fn a_pasted_article_url_yields_its_language_and_title() {
        assert_eq!(
            parse_wikipedia_url("https://fr.wikipedia.org/wiki/Toyota_Sprinter_Trueno"),
            Some(("fr".into(), "Toyota Sprinter Trueno".into()))
        );
        assert_eq!(
            parse_wikipedia_url("https://en.wikipedia.org/wiki/Mazda_MX-5?action=history"),
            Some(("en".into(), "Mazda MX-5".into())),
            "une requête ne fait pas partie du titre"
        );
        assert_eq!(
            parse_wikipedia_url("https://de.wikipedia.org/wiki/N%C3%BCrburgring"),
            Some(("de".into(), "Nürburgring".into())),
            "le titre est décodé"
        );
    }

    /// Rule (WIKI§10, WIKI§11): **everything that is not a Wikipedia URL is refused,
    /// homographs included.**
    ///
    /// The dotless ı of `wikipedıa.org` is invisible in a proof-reading and
    /// points at a domain anyone can register. Refusing every non-ASCII host
    /// closes that door without enumerating lookalikes — and this is a
    /// decorative feature, so there is nothing to gain by being clever.
    #[test]
    fn a_lookalike_domain_is_refused() {
        assert_eq!(
            parse_wikipedia_url("https://fr.wikipedıa.org/wiki/Abarth_500"),
            None,
            "ı sans point : ce n'est pas wikipedia.org"
        );
        assert_eq!(parse_wikipedia_url("https://wikipedia.org.evil.com/wiki/X"), None);
        assert_eq!(parse_wikipedia_url("https://fr.wikipedia.org.evil.com/wiki/X"), None);
        assert_eq!(parse_wikipedia_url("https://evil.com/wiki/X"), None);
        assert_eq!(parse_wikipedia_url("https://fr.wikipedia.org@evil.com/wiki/X"), None);
        assert_eq!(parse_wikipedia_url("ftp://fr.wikipedia.org/wiki/X"), None);
    }

    /// Rule: a Wikimedia URL that is not an article is not an article.
    #[test]
    fn only_article_paths_count() {
        assert_eq!(
            parse_wikipedia_url("https://fr.wikipedia.org/w/index.php?title=X"),
            None
        );
        assert_eq!(parse_wikipedia_url("https://fr.wikipedia.org/wiki/"), None);
        assert_eq!(
            parse_wikipedia_url("https://commons.wikimedia.org/wiki/Main_Page"),
            None,
            "Commons n'est pas Wikipédia"
        );
    }
}
