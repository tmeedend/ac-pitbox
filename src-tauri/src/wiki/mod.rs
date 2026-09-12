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
pub mod curated;
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

/// What the fiche needs, decided **without touching the network**.
///
/// The three-step split — `plan`, `fetch`, `commit` — exists for one reason
/// and it is not elegance: `plan` and `commit` run under the SQLite lock,
/// `fetch` does not. A resolution can spend fifteen seconds on the network in
/// the worst case (three timeouts), and holding the overlay's mutex for that
/// long would freeze every other command in the app — the exact shape of the
/// blocked-library bug `commands/ui_prefs.rs` documents. A decorative tab must
/// never be able to do that.
pub enum Step {
    /// Nothing to ask anyone: here is what to display, if anything.
    Settled(Option<CachedArticle>),
    /// The network is needed. Everything it takes is in here, already read.
    Ask(Ask),
}

/// What `fetch` needs, gathered under the lock so it needs no database.
pub struct Ask {
    pub mod_key: String,
    pub lang: String,
    /// The appariement, when there is one. `None` means the matching of §4 has
    /// to run first — which is how a fiche opened for the first time works.
    pub entity_id: Option<String>,
    pub subject: Subject,
    /// What to fall back on when the network leads nowhere: a stale cache entry
    /// beats an empty tab, and §1 says nothing here is worth an error.
    pub stale: Option<CachedArticle>,
}

/// Cars and tracks share the resolution and **not** the search strategy (§4).
pub enum Subject {
    Car(matchcar::CarSubject),
    Track(matchtrack::TrackSubject),
}

/// What `fetch` learnt, ready to be written under the lock.
pub struct Resolved {
    pub mod_key: String,
    /// An appariement found just now, to store as `auto`.
    pub matched: Option<String>,
    pub article: Option<CachedArticle>,
    /// True only when the absence is **durable** — never after a network
    /// failure, which teaches nothing (§3.3).
    pub no_match: bool,
    pub stale: Option<CachedArticle>,
}

/// Reads everything the resolution needs from the overlay (§5).
///
/// `online` is §8's switch: off, the cache stays readable and not one request
/// goes out. `requested_lang` is an app locale and gets truncated — `fr-CA`
/// reads `fr.wikipedia.org`.
pub fn plan(
    conn: &Connection,
    ac_install: Option<&std::path::Path>,
    mod_key: &str,
    requested_lang: &str,
    online: bool,
) -> Step {
    let lang = wiki_lang(requested_lang);
    let link = best_effort("wiki_link", store::get_link(conn, mod_key)).flatten();

    let cached = link
        .as_ref()
        .and_then(|l| best_effort("wiki_cache", store::get_article(conn, &l.entity_id, &lang)).flatten());
    if let Some(row) = &cached {
        if store::is_fresh(&row.fetched_at, Duration::days(POSITIVE_TTL_DAYS), Local::now()) {
            return Step::Settled(cached);
        }
    }

    // §8: enrichment off. What was fetched before stays readable, however old —
    // a stale extract is worth more than an empty tab, and nothing depends on
    // it being current.
    if !online {
        return Step::Settled(cached);
    }

    // §3.3, before anything else: this mod was tried recently and led nowhere.
    // Without this check, every opening of the fiche replays a full resolution
    // for an answer that has not changed.
    if best_effort("wiki_no_match", store::has_fresh_no_match(conn, mod_key, Local::now())).unwrap_or(false) {
        return Step::Settled(cached);
    }

    let Some(subject) = subject_of(conn, ac_install, mod_key) else {
        return Step::Settled(cached);
    };
    Step::Ask(Ask {
        mod_key: mod_key.to_string(),
        lang,
        entity_id: link.map(|l| l.entity_id),
        subject,
        stale: cached,
    })
}

/// Builds what §4 needs to search with, from what the overlay already knows
/// about the mod.
fn subject_of(conn: &Connection, ac_install: Option<&std::path::Path>, mod_key: &str) -> Option<Subject> {
    let row = best_effort("mods", crate::overlay::get_mod(conn, mod_key)).flatten()?;
    let name = row.display_name.clone().unwrap_or_else(|| row.id_interne.clone());
    if row.kind == "Track" {
        // CSP's table first, the mod's geotags second — `sun.rs` already knows
        // how, and knows that Kunos geotags are a placeholder.
        let location = ac_install
            .and_then(|ac| crate::sun::track_location(ac, mod_key, None))
            .map(|l| (l.latitude, l.longitude));
        return Some(Subject::Track(matchtrack::TrackSubject {
            name: Some(name),
            location,
        }));
    }
    Some(Subject::Car(matchcar::CarSubject {
        brand: row.brand.clone(),
        name: Some(name),
        year: row.year,
        category: row.category.clone(),
    }))
}

/// The network half (§4 then §5), with **no database at all**.
pub fn fetch(
    net: &WikiClient,
    cleaner: &clean::Cleaner,
    weights: &clean::Weights,
    thresholds: &matching::Thresholds,
    ask: Ask,
) -> Resolved {
    let mut out = Resolved {
        mod_key: ask.mod_key,
        matched: None,
        article: None,
        no_match: false,
        stale: ask.stale,
    };

    let lang = ask.lang;

    // The appariement first, when the mod has none. An ambiguity is a verdict
    // (§1): it produces no match and no negative-cache entry either, because
    // the candidates are real and a better threshold may accept one later.
    let entity_id = match ask.entity_id {
        Some(id) => id,
        None => match match_subject(net, cleaner, weights, thresholds, &ask.subject, &lang) {
            matching::MatchOutcome::Matched { candidate, .. } => {
                out.matched = Some(candidate.entity_id.clone());
                candidate.entity_id
            }
            matching::MatchOutcome::Ambiguous { .. } => return out,
            matching::MatchOutcome::NoCandidate => {
                out.no_match = true;
                return out;
            }
            matching::MatchOutcome::Unavailable => return out,
        },
    };

    let facts = match net.entity(&entity_id) {
        Fetched::Found(facts) => facts,
        // The entity itself has no article anywhere: durable, worth remembering.
        Fetched::Absent => {
            out.no_match = true;
            return out;
        }
        // Nothing was learned — in particular, not that there is no article.
        Fetched::Unavailable => return out,
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
                out.article = Some(CachedArticle {
                    entity_id: entity_id.clone(),
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
                });
                return out;
            }
            Fetched::Absent => continue,
            Fetched::Unavailable => {
                could_not_ask = true;
                continue;
            }
        }
    }

    // One `Unavailable` anywhere in the chain forbids the negative cache: a
    // train tunnel must not cost ninety days of absent tab.
    out.no_match = !could_not_ask;
    out
}

/// Runs the right strategy for the subject. Cars and tracks share this line and
/// nothing else — §4 gives them two different searches on purpose.
///
/// `locale` decides which wiki is searched **first** (`lang::search_order`,
/// English second): the English "Abarth 500" is a disambiguation page while the
/// French one is a real article, so a French reader finds what an English
/// search throws away.
fn match_subject(
    net: &WikiClient,
    cleaner: &clean::Cleaner,
    weights: &clean::Weights,
    thresholds: &matching::Thresholds,
    subject: &Subject,
    locale: &str,
) -> matching::MatchOutcome {
    match subject {
        Subject::Car(car) => matchcar::match_car(net, cleaner, weights, thresholds, car, locale),
        Subject::Track(track) => matchtrack::match_track(net, cleaner, thresholds, track, locale),
    }
}

/// Writes what `fetch` learnt, back under the lock (§3).
pub fn commit(conn: &Connection, resolved: Resolved) -> Option<CachedArticle> {
    if let Some(entity_id) = &resolved.matched {
        // `auto`: the precedence of §3.1 means this never overwrites a
        // correction or a shipped entry.
        best_effort(
            "wiki_link write",
            store::set_link(conn, &resolved.mod_key, entity_id, store::LinkSource::Auto),
        );
    }
    if let Some(row) = &resolved.article {
        best_effort("wiki_cache write", store::put_article(conn, row));
        // It used to lead nowhere and now it does: the entry would otherwise
        // keep an article out of sight for up to ninety days.
        best_effort("wiki_no_match clear", store::forget_no_match(conn, &resolved.mod_key));
        return resolved.article;
    }
    if resolved.no_match {
        best_effort("wiki_no_match write", store::note_no_match(conn, &resolved.mod_key));
    }
    resolved.stale
}
