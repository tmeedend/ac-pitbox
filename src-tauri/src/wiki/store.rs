//! The three tables of §3, in `overlay.sqlite`.
//!
//! `wiki_link` is metadata the app produced and the user may correct; it
//! belongs with the rest of the overlay, and it is the row that will travel
//! between installations one day (§10). The two others are caches of what the
//! network said, and they live in the same file for one practical reason: the
//! purge of §8 is then two `DELETE`s under the lock the app already holds,
//! rather than a second database with its own migrations.
//!
//! The schema itself is declared in `overlay::init`, like every other table —
//! keeping one place where the whole schema can be read is what gives
//! `overlay::tests::every_listing_runs_on_a_fresh_database` its meaning.

use chrono::{DateTime, Duration, Local};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Where an appariement comes from, and who may overwrite whom (§3.1).
///
/// **The declaration order is the precedence**: the derived `Ord` gives
/// `Auto < Import < Manual`, so `set_link` only has to compare. Spelled this
/// way rather than as a `rank()` method because a new variant would then have
/// to be inserted in the right place to be correct, which is exactly where it
/// is easiest to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkSource {
    /// Matched by the app (§4).
    Auto,
    /// Received from a shared set of appariements (§10, not implemented).
    Import,
    /// Corrected by hand (§7.6). Never overwritten automatically.
    Manual,
}

impl LinkSource {
    fn as_str(self) -> &'static str {
        match self {
            LinkSource::Auto => "auto",
            LinkSource::Import => "import",
            LinkSource::Manual => "manual",
        }
    }

    /// Anything unknown reads as `Auto`, the weakest rank: a value this app
    /// does not understand must never be able to block a correction.
    fn parse(value: &str) -> Self {
        match value {
            "manual" => LinkSource::Manual,
            "import" => LinkSource::Import,
            _ => LinkSource::Auto,
        }
    }
}

/// §3.1 — one mod, one entity.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiLink {
    /// The mod's folder name (`ks_toyota_ae86`), which is `mods.id_interne`.
    /// Chosen by the spec for being stable across installations.
    pub mod_key: String,
    pub entity_id: String,
    pub source: LinkSource,
    pub resolved_at: String,
}

/// §3.2 — what is shown for one (entity, requested language) pair.
///
/// `lang` is the language that was **asked for**, not necessarily the one the
/// text is in: the chain of §5.2 may have landed on English. The language
/// actually displayed is readable from `article_url`, which is why that URL is
/// stored rather than rebuilt (see `article_lang`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedArticle {
    pub entity_id: String,
    pub lang: String,
    pub article_title: String,
    pub article_url: String,
    pub revision_id: Option<i64>,
    pub extract: String,
    /// Set when the text comes from the parent entity (§5.3), which the
    /// interface must say (§7.3).
    pub parent_entity: Option<String>,
    pub available_langs: Vec<String>,
    pub fetched_at: String,
    /// L'article rendu par MediaWiki. Vide = le rendu n'a pas pu être obtenu,
    /// et le texte brut d'`extract` porte l'onglet à lui seul.
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub sections: Vec<super::api::Section>,
    /// Les seules images affichables : Commons, licence et auteur connus (§9).
    #[serde(default)]
    pub images: Vec<super::api::ImageCredit>,
}

impl CachedArticle {
    /// The language the stored text is actually in, read off its URL.
    ///
    /// `https://fr.wikipedia.org/wiki/...` gives `fr`. The alternative was a
    /// tenth column duplicating what the URL already says — and the URL is the
    /// one field guaranteed to be right, since it comes from the API itself.
    pub fn article_lang(&self) -> Option<&str> {
        let host = self.article_url.strip_prefix("https://")?;
        let (code, rest) = host.split_once('.')?;
        rest.starts_with("wikipedia.org/").then_some(code)
    }
}

/// Version of what a cached row *contains*, as opposed to how old it is.
///
/// Same need, and same remedy, as `harmonize::ENGINE_VERSION`: the rows cached
/// before this bumped hold an introduction only, and nothing in a row says so —
/// its `fetched_at` is recent, so the TTL would happily serve a truncated
/// article for thirty days. Bump this whenever the stored text changes meaning,
/// and `purge_outdated` empties the cache once.
///
/// 2 — the whole article rather than its introduction (§7.3).
/// 3 — the rendered article, its sections and its image credits alongside.
pub const CONTENT_VERSION: u32 = 3;

/// Empties the cache when it holds rows a previous version of the code wrote.
///
/// Only `wiki_cache` — the appariements and the negative cache say nothing
/// about the text and have no reason to go.
pub fn purge_outdated(conn: &Connection) -> rusqlite::Result<bool> {
    const KEY: &str = "wiki_content_version";
    let stored: Option<u32> = crate::overlay::get_meta(conn, KEY)?.and_then(|v| v.parse().ok());
    if stored == Some(CONTENT_VERSION) {
        return Ok(false);
    }
    conn.execute("DELETE FROM wiki_cache", [])?;
    crate::overlay::set_meta(conn, KEY, &CONTENT_VERSION.to_string())?;
    Ok(stored.is_some())
}

/// §13: a cached article is refetched after thirty days. Long enough that a
/// browsed library costs nothing, short enough that a rewritten introduction
/// catches up within a season.
pub const POSITIVE_TTL_DAYS: i64 = 30;

/// §3.3: a mod with no match is left alone for ninety days. An article can be
/// created in the meantime — this is the only reason the negative cache
/// expires at all.
pub const NEGATIVE_TTL_DAYS: i64 = 90;

fn now_stamp() -> String {
    Local::now().to_rfc3339()
}

/// Is an RFC3339 stamp still within `ttl` of `now`?
///
/// `now` is a parameter and not a call to the clock: that is what lets the TTL
/// test of §11 assert on an expired entry without waiting ninety days. An
/// unparseable stamp reads as stale — refetching costs one request, trusting a
/// corrupted row costs a wrong article forever.
pub fn is_fresh(stamp: &str, ttl: Duration, now: DateTime<Local>) -> bool {
    match DateTime::parse_from_rfc3339(stamp) {
        Ok(written) => now.signed_duration_since(written) < ttl,
        Err(_) => false,
    }
}

pub fn get_link(conn: &Connection, mod_key: &str) -> rusqlite::Result<Option<WikiLink>> {
    conn.query_row(
        "SELECT mod_key, entity_id, source, resolved_at FROM wiki_link WHERE mod_key = ?1",
        [mod_key],
        |row| {
            Ok(WikiLink {
                mod_key: row.get(0)?,
                entity_id: row.get(1)?,
                source: LinkSource::parse(&row.get::<_, String>(2)?),
                resolved_at: row.get(3)?,
            })
        },
    )
    .optional()
}

/// Writes an appariement **if the precedence allows it** (§3.1).
///
/// Returns whether the row was written. A refusal is not an error: re-running
/// the automatic matching over a library where the user corrected three entries
/// is supposed to leave those three alone, silently.
///
/// An equal rank does overwrite, deliberately: a second automatic pass refreshes
/// its own verdict, and a second manual correction replaces the first.
pub fn set_link(conn: &Connection, mod_key: &str, entity_id: &str, source: LinkSource) -> rusqlite::Result<bool> {
    if let Some(existing) = get_link(conn, mod_key)? {
        if existing.source > source {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO wiki_link(mod_key, entity_id, source, resolved_at) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(mod_key) DO UPDATE SET
            entity_id = excluded.entity_id, source = excluded.source, resolved_at = excluded.resolved_at",
        params![mod_key, entity_id, source.as_str(), now_stamp()],
    )?;
    Ok(true)
}

/// Drops an appariement. Used when a correction says "none of these" — the mod
/// then has no entity at all, which is a valid answer (§1).
pub fn clear_link(conn: &Connection, mod_key: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM wiki_link WHERE mod_key = ?1", [mod_key])?;
    Ok(())
}

pub fn get_article(conn: &Connection, entity_id: &str, lang: &str) -> rusqlite::Result<Option<CachedArticle>> {
    conn.query_row(
        "SELECT entity_id, lang, article_title, article_url, revision_id, extract, parent_entity,
                available_langs, fetched_at, html, sections, images
           FROM wiki_cache WHERE entity_id = ?1 AND lang = ?2",
        params![entity_id, lang],
        |row| {
            let langs: String = row.get(7)?;
            Ok(CachedArticle {
                entity_id: row.get(0)?,
                lang: row.get(1)?,
                article_title: row.get(2)?,
                article_url: row.get(3)?,
                revision_id: row.get(4)?,
                extract: row.get(5)?,
                parent_entity: row.get(6)?,
                // A corrupted list costs the language selector, not the text.
                available_langs: serde_json::from_str(&langs).unwrap_or_default(),
                fetched_at: row.get(8)?,
                html: row.get(9)?,
                // Same stance for these two: a table of contents or a credit
                // list that cannot be read costs its own feature, never the
                // article.
                sections: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
                images: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
            })
        },
    )
    .optional()
}

pub fn put_article(conn: &Connection, article: &CachedArticle) -> rusqlite::Result<()> {
    let langs = serde_json::to_string(&article.available_langs).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO wiki_cache(entity_id, lang, article_title, article_url, revision_id, extract,
                                parent_entity, available_langs, fetched_at, html, sections, images)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(entity_id, lang) DO UPDATE SET
            article_title = excluded.article_title, article_url = excluded.article_url,
            revision_id = excluded.revision_id, extract = excluded.extract,
            parent_entity = excluded.parent_entity, available_langs = excluded.available_langs,
            fetched_at = excluded.fetched_at, html = excluded.html,
            sections = excluded.sections, images = excluded.images",
        params![
            article.entity_id,
            article.lang,
            article.article_title,
            article.article_url,
            article.revision_id,
            article.extract,
            article.parent_entity,
            langs,
            article.fetched_at,
            article.html,
            serde_json::to_string(&article.sections).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&article.images).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

/// Records that this mod led nowhere (§3.3).
///
/// Only ever called on a **durable** non-result — an entity or an article that
/// does not exist. A network failure must never come through here: it would
/// turn one offline session into ninety days without a tab.
pub fn note_no_match(conn: &Connection, mod_key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO wiki_no_match(mod_key, attempted_at) VALUES(?1, ?2)
         ON CONFLICT(mod_key) DO UPDATE SET attempted_at = excluded.attempted_at",
        params![mod_key, now_stamp()],
    )?;
    Ok(())
}

pub fn forget_no_match(conn: &Connection, mod_key: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM wiki_no_match WHERE mod_key = ?1", [mod_key])?;
    Ok(())
}

/// Has this mod been tried recently enough that it should not be tried again?
///
/// This is the whole point of the table (§3.3): without it, every opening of a
/// fiche with no match replays a full resolution — two requests each time, for
/// an answer that will not have changed.
pub fn has_fresh_no_match(conn: &Connection, mod_key: &str, now: DateTime<Local>) -> rusqlite::Result<bool> {
    let stamp: Option<String> = conn
        .query_row(
            "SELECT attempted_at FROM wiki_no_match WHERE mod_key = ?1",
            [mod_key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stamp.is_some_and(|s| is_fresh(&s, Duration::days(NEGATIVE_TTL_DAYS), now)))
}

/// §8 — empties both caches and **keeps the appariements**. Corrections made by
/// hand are not cache, and re-matching a whole library to recover them would be
/// both slow and lossy.
pub fn purge_cache(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM wiki_cache", [])?;
    conn.execute("DELETE FROM wiki_no_match", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(tag: &str) -> (crate::testutil::TempDir, Connection) {
        let base = crate::testutil::temp_dir(tag);
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).expect("overlay");
        (base, conn)
    }

    fn article(entity: &str, lang: &str) -> CachedArticle {
        CachedArticle {
            entity_id: entity.into(),
            lang: lang.into(),
            article_title: "Toyota AE86".into(),
            article_url: "https://en.wikipedia.org/wiki/Toyota_AE86".into(),
            revision_id: Some(1373212308),
            extract: "The AE86 series...".into(),
            parent_entity: None,
            available_langs: vec!["en".into(), "fr".into(), "ja".into()],
            fetched_at: now_stamp(),
            html: "<p>The AE86 series...</p>".into(),
            sections: Vec::new(),
            images: Vec::new(),
        }
    }

    /// Rule (§3.1): `manual` is never overwritten automatically — not by the
    /// matching engine, not by a shared set of appariements.
    #[test]
    fn a_manual_link_survives_auto_and_import() {
        let (_base, conn) = db("wiki-precedence");

        assert!(set_link(&conn, "ks_toyota_ae86", "Q1377219", LinkSource::Manual).unwrap());
        assert!(
            !set_link(&conn, "ks_toyota_ae86", "Q999", LinkSource::Auto).unwrap(),
            "the automatic matching must give way"
        );
        assert!(
            !set_link(&conn, "ks_toyota_ae86", "Q888", LinkSource::Import).unwrap(),
            "a shared appariement must give way too (§10)"
        );

        let link = get_link(&conn, "ks_toyota_ae86").unwrap().expect("link");
        assert_eq!(link.entity_id, "Q1377219", "the hand-made correction stands");
        assert_eq!(link.source, LinkSource::Manual);
    }

    /// Rule (§3.1): the precedence runs the other way round without resistance,
    /// and an equal rank refreshes its own verdict.
    #[test]
    fn a_stronger_source_replaces_a_weaker_one() {
        let (_base, conn) = db("wiki-precedence-up");

        assert!(set_link(&conn, "mod", "Q1", LinkSource::Auto).unwrap());
        assert!(
            set_link(&conn, "mod", "Q2", LinkSource::Import).unwrap(),
            "import beats auto"
        );
        assert!(
            set_link(&conn, "mod", "Q3", LinkSource::Manual).unwrap(),
            "manual beats import"
        );
        assert!(
            set_link(&conn, "mod", "Q4", LinkSource::Manual).unwrap(),
            "a second correction replaces the first"
        );

        assert_eq!(get_link(&conn, "mod").unwrap().unwrap().entity_id, "Q4");
    }

    /// Rule (§3.2): a cache row survives the round trip whole, list of
    /// languages and parent fallback included.
    #[test]
    fn a_cache_row_round_trips() {
        let (_base, conn) = db("wiki-cache");
        let mut row = article("Q1377219", "fr");
        row.parent_entity = Some("Q2626308".into());

        put_article(&conn, &row).unwrap();
        let back = get_article(&conn, "Q1377219", "fr").unwrap().expect("cached");

        assert_eq!(back, row, "nothing lost between the two");
        assert_eq!(
            back.parent_entity.as_deref(),
            Some("Q2626308"),
            "the parent fallback must stay visible to the interface (§7.3)"
        );
    }

    /// Rule (§3.2): the same entity in two languages is two rows, not one
    /// overwriting the other — the primary key is composite.
    #[test]
    fn two_languages_of_one_entity_coexist() {
        let (_base, conn) = db("wiki-cache-langs");

        put_article(&conn, &article("Q1377219", "fr")).unwrap();
        put_article(&conn, &article("Q1377219", "en")).unwrap();

        assert!(get_article(&conn, "Q1377219", "fr").unwrap().is_some());
        assert!(get_article(&conn, "Q1377219", "en").unwrap().is_some());
    }

    /// Rule (§3.3, §11): a mod that led nowhere is not tried again before the
    /// TTL expires — and is tried again after.
    #[test]
    fn the_negative_cache_holds_for_its_ttl_and_not_longer() {
        let (_base, conn) = db("wiki-no-match");
        let now = Local::now();

        assert!(
            !has_fresh_no_match(&conn, "ks_unknown", now).unwrap(),
            "nothing recorded yet: the resolution may run"
        );

        note_no_match(&conn, "ks_unknown").unwrap();
        assert!(
            has_fresh_no_match(&conn, "ks_unknown", now).unwrap(),
            "just recorded: no new request (§3.3)"
        );

        let later = now + Duration::days(NEGATIVE_TTL_DAYS) + Duration::hours(1);
        assert!(
            !has_fresh_no_match(&conn, "ks_unknown", later).unwrap(),
            "after ninety days the article may have been written"
        );
    }

    /// Rule (§8): the purge takes the caches and leaves the appariements. A
    /// correction made by hand is not cache.
    #[test]
    fn the_purge_keeps_what_the_user_decided() {
        let (_base, conn) = db("wiki-purge");
        set_link(&conn, "mod", "Q1377219", LinkSource::Manual).unwrap();
        put_article(&conn, &article("Q1377219", "fr")).unwrap();
        note_no_match(&conn, "other_mod").unwrap();

        purge_cache(&conn).unwrap();

        assert!(get_article(&conn, "Q1377219", "fr").unwrap().is_none(), "cache emptied");
        assert!(
            !has_fresh_no_match(&conn, "other_mod", Local::now()).unwrap(),
            "negative cache emptied"
        );
        assert!(get_link(&conn, "mod").unwrap().is_some(), "the appariement stays (§8)");
    }

    /// Rule (§3.2/§5.2): the displayed language is readable off the stored URL,
    /// which is what makes a tenth column unnecessary — a French request served
    /// by the English article must not look French.
    #[test]
    fn the_displayed_language_comes_from_the_url() {
        let mut row = article("Q1377219", "fr");
        assert_eq!(row.article_lang(), Some("en"), "asked in French, served in English");

        row.article_url = "https://ja.wikipedia.org/wiki/%E3%83%88%E3%83%A8%E3%82%BF".into();
        assert_eq!(row.article_lang(), Some("ja"));

        row.article_url = "https://example.com/wiki/Toyota".into();
        assert_eq!(row.article_lang(), None, "not a Wikipedia URL, no claim made");
    }

    /// Rule (§13): freshness is decided against a clock the caller passes, and
    /// a stamp that cannot be read counts as stale.
    #[test]
    fn freshness_is_decided_against_the_given_clock() {
        let now = Local::now();
        let ttl = Duration::days(POSITIVE_TTL_DAYS);

        let fresh = (now - Duration::days(POSITIVE_TTL_DAYS - 1)).to_rfc3339();
        assert!(is_fresh(&fresh, ttl, now), "one day short of the ttl");

        let stale = (now - Duration::days(POSITIVE_TTL_DAYS + 1)).to_rfc3339();
        assert!(!is_fresh(&stale, ttl, now), "one day past it");

        assert!(
            !is_fresh("not a date", ttl, now),
            "unreadable reads as stale, never as fresh"
        );
    }
}
