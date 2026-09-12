//! Wikipedia enrichment of the detail fiche (`docs/SPEC-wikipedia-fiche-detail.md`).
//!
//! What is implemented here is lot 1 of §12, the socle: the data model (§3),
//! the resolution at display time with its fallback chain (§5), and the Action
//! API client (§6). **No interface, and no automatic matching** — nothing
//! writes a `wiki_link` row yet, that is §4.
//!
//! Three things govern every decision in this module, all from §1:
//!
//! - **Precision over recall.** An ambiguity shows nothing. A wrong article
//!   costs far more than a missing one.
//! - **Absence is not an error.** No failure path here produces a message, an
//!   icon or an `Err` for the interface; the tab is simply absent.
//! - **Nothing depends on this data.** It feeds no filter, no scoring, no other
//!   feature, and may fail silently — which is exactly why every failure that
//!   is invisible to the user is written to the log file instead.
//!
//! The legal constraint of §2 shapes one thing in the code: the extract is
//! stored and served **verbatim**, as the API returned it. Nothing here
//! reformats, truncates, merges or translates it.
#![allow(dead_code)]
// The socle has no caller yet: the matching (§4) is what will create the
// appariements this module reads, and the interface (§7) is what will display
// what it returns. Lifted as soon as either lands — it is here so that a
// deliberately unreachable lot does not have to be padded with a command
// surface nobody calls just to keep clippy quiet.

pub mod api;
pub mod calibrate;
pub mod clean;
pub mod ids;
pub mod lang;
pub mod matchcar;
pub mod matching;
pub mod matchtrack;
pub mod store;

mod http;

use chrono::{Duration, Local};
use rusqlite::Connection;

use api::{Fetched, WikiClient};
use lang::{fallback_chain, wiki_lang, EntityArticles};
use store::{CachedArticle, POSITIVE_TTL_DAYS};

/// Fills in a label the entity fetch did not return, from the title the search
/// already gave us.
///
/// Not cosmetic: `wbgetentities` is asked for `en|fr` labels, and an item that
/// has neither comes back nameless — which scores 0 against every name and
/// drops out silently. Measured on Q10843475 ("Abarth 500 Assetto Corse"),
/// whose only sitelinks are Spanish and Russian. The search knew its name; the
/// details call did not.
fn borrow_labels(details: &mut [api::EntityDetails], hits: &[api::SearchHit]) {
    for entry in details.iter_mut() {
        if entry.label.is_some() {
            continue;
        }
        if let Some(hit) = hits.iter().find(|h| h.entity_id == entry.entity_id) {
            entry.label = hit.label.clone();
        }
    }
}

/// Logs what a best-effort database call lost, and carries on.
///
/// Every read in this module is best-effort by design — a decorative tab must
/// not take an interface down with it. What it must not be is *silent*: a
/// packaged build has no console, so a swallowed error leaves nothing at all to
/// diagnose from afterwards.
fn best_effort<T>(what: &str, result: rusqlite::Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(e) => {
            log::warn!("wiki: {what} — {e}");
            None
        }
    }
}

/// The entity to climb to for §5.3, fetched and **checked**.
///
/// `part of` is trusted outright: it is the relation the spec names, and it is
/// what a generation or a configuration uses to point at its whole.
///
/// `subclass of` is not, and that is a correction born of a measurement: the
/// AE86's `P279` is "sport compact", a *classification*. Climbing it would have
/// offered an article about a category of cars under the heading "Article
/// général". So a `P279` target is only accepted when it is of the same nature
/// as the entity itself — a car model's parent must be a car model.
fn parent_to_climb(net: &WikiClient, facts: &api::EntityFacts) -> Option<api::EntityFacts> {
    if let Some(parent_id) = &facts.parent {
        return net.entity(parent_id).found();
    }
    let candidate = facts.parent_fallback.as_ref()?;
    let parent = net.entity(candidate).found()?;
    let same_nature = parent.types.iter().any(|t| facts.types.contains(t));
    if !same_nature {
        log::debug!(
            "wiki: {candidate} n'est pas de la nature de {} — pas de remontée",
            facts.entity_id
        );
        return None;
    }
    Some(parent)
}

/// The article to show for a mod, in the requested language (§5).
///
/// `net` is `None` when online enrichment is switched off (§8) — the cache
/// stays readable, no request goes out. `requested_lang` is an app locale and
/// gets truncated (`fr-CA` reads `fr.wikipedia.org`); the global preference
/// behind it belongs to the frontend, which is where §5.1 puts it.
///
/// `None` means there is nothing to show, and that is an ordinary outcome: no
/// appariement, no article, nothing cached and no network. The caller has
/// nothing to display and no error to report.
///
/// The order below is what keeps the request count at "a few dozen a month"
/// (§6.3): cache, then negative cache, then at most three requests — one for
/// the entity, one for the parent only when the entity cannot serve the
/// requested language, one for the article itself.
pub fn resolve_article(
    conn: &Connection,
    net: Option<&WikiClient>,
    mod_key: &str,
    requested_lang: &str,
) -> Option<CachedArticle> {
    let lang = wiki_lang(requested_lang);

    // No appariement, nothing to resolve. Creating one is §4's job.
    let link = best_effort("wiki_link", store::get_link(conn, mod_key))??;

    let cached = best_effort("wiki_cache", store::get_article(conn, &link.entity_id, &lang)).flatten();
    if let Some(row) = &cached {
        if store::is_fresh(&row.fetched_at, Duration::days(POSITIVE_TTL_DAYS), Local::now()) {
            return cached;
        }
    }

    // §8: enrichment off. What was fetched before stays readable, however old —
    // a stale extract is worth more than an empty tab, and nothing here depends
    // on it being current.
    let net = net?;

    // §3.3, before any request: this mod has been tried recently and led
    // nowhere. Without this check every opening of the fiche replays the whole
    // resolution for an answer that has not changed.
    if best_effort("wiki_no_match", store::has_fresh_no_match(conn, mod_key, Local::now())).unwrap_or(false) {
        return cached;
    }

    let facts = match net.entity(&link.entity_id) {
        Fetched::Found(facts) => facts,
        // The entity itself has no article anywhere: durable, worth remembering.
        Fetched::Absent => {
            best_effort("wiki_no_match write", store::note_no_match(conn, mod_key));
            return cached;
        }
        // Nothing was learned — in particular, not that there is no article.
        Fetched::Unavailable => return cached,
    };

    // The parent is fetched only when the entity cannot serve the requested
    // language, which is the only case where level 2 can win (§5.2). When it
    // can, level 1 takes it and the second request would be pure politeness
    // debt against §6.2.
    let parent = if facts.titles.contains_key(&lang) {
        None
    } else {
        parent_to_climb(net, &facts)
    };

    let entity_articles = EntityArticles::new(facts.entity_id.clone(), facts.titles.clone());
    let parent_articles = parent
        .as_ref()
        .map(|p| EntityArticles::new(p.entity_id.clone(), p.titles.clone()));
    let chain = fallback_chain(&entity_articles, parent_articles.as_ref(), &lang);

    // Tracks whether any link of the chain failed for a reason that teaches us
    // nothing. One `Unavailable` is enough to forbid writing the negative
    // cache: a train tunnel must not cost ninety days of absent tab.
    let mut could_not_ask = false;

    for attempt in chain {
        match net.article(&attempt.lang, &attempt.title) {
            Fetched::Found(text) => {
                let row = CachedArticle {
                    entity_id: link.entity_id.clone(),
                    // The language **asked for**, per §3.2 — the one actually
                    // served is readable off the URL.
                    lang: lang.clone(),
                    article_title: text.title,
                    article_url: text.url,
                    revision_id: text.revision_id,
                    extract: text.extract,
                    parent_entity: attempt.via_parent.then(|| attempt.entity_id.clone()),
                    available_langs: text.available_langs,
                    fetched_at: Local::now().to_rfc3339(),
                };
                best_effort("wiki_cache write", store::put_article(conn, &row));
                // It used to lead nowhere and now it does: the entry would
                // otherwise keep an article out of sight for up to ninety days.
                best_effort("wiki_no_match clear", store::forget_no_match(conn, mod_key));
                return Some(row);
            }
            Fetched::Absent => continue,
            Fetched::Unavailable => {
                could_not_ask = true;
                continue;
            }
        }
    }

    if !could_not_ask {
        best_effort("wiki_no_match write", store::note_no_match(conn, mod_key));
    }
    cached
}
