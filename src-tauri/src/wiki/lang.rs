//! Language resolution for the Wikipedia tab (`docs/SPEC-wikipedia-fiche-detail.md` WIKI§5).
//!
//! Everything in this file is **pure**: no network, no database, no clock. That
//! is the whole point of the split. The fallback chain is the one piece whose
//! order is easy to get wrong and impossible to notice afterwards — a French
//! article silently replaced by an English one reads as "this mod has no French
//! article", never as a bug — so WIKI§11 asks for it to be testable without
//! Wikipedia being up, and it is.

use std::collections::BTreeMap;

/// Language every level of the chain falls back to (WIKI§5.2, levels 3 and 4).
pub const FALLBACK_LANG: &str = "en";

/// Wiki language code for an app locale: `"pt-BR"` becomes `"pt"`.
///
/// Same truncation as the frontend's `i18n/index.svelte.ts`, and for the same
/// reason: the app ships one dictionary per two-letter code, and Wikipedia's
/// language editions are addressed the same way (`fr.wikipedia.org`). Variants
/// that do have their own wiki (`zh-yue`, `be-tarask`) are never *requested* by
/// the app — they can only turn up in `available_langs`, where they are kept
/// verbatim for the language selector (WIKI§5.4).
pub fn wiki_lang(locale: &str) -> String {
    let code = locale.split(['-', '_']).next().unwrap_or(locale);
    code.trim().to_ascii_lowercase()
}

/// Which wikis to search a name in, in order: the reader's own language, then
/// English.
///
/// Not an optimisation — a correctness fix, measured. The English Wikipedia's
/// `Abarth 500` is a **disambiguation page** (Q4167410), so a full-text search
/// there for "Abarth 500 Assetto Corse" returns `Fiat 500 (2007)` and the type
/// filter rightly throws it away. The French Wikipedia has a real `Abarth 500`
/// article, and its search returns it first. European cars are routinely
/// better covered at home than in English, so home comes first.
///
/// English still follows, because it carries the widest net for everything
/// else — and the second search only ever runs when the first found nothing.
pub fn search_order(locale: &str) -> Vec<String> {
    let home = wiki_lang(locale);
    if home == FALLBACK_LANG {
        return vec![home];
    }
    vec![home, FALLBACK_LANG.to_string()]
}

/// The articles one Wikidata entity has, one title per language.
///
/// Built from Wikidata sitelinks (`api::EntityFacts`), or from what the cache
/// already knows. The title matters as much as the language: a wiki is queried
/// by title, and the title changes from one language to the next — the AE86 is
/// `Toyota AE86` in English and `Toyota Sprinter Trueno` in French (measured on
/// the real API, not assumed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityArticles {
    pub entity_id: String,
    pub titles: BTreeMap<String, String>,
}

impl EntityArticles {
    pub fn new(entity_id: impl Into<String>, titles: BTreeMap<String, String>) -> Self {
        Self {
            entity_id: entity_id.into(),
            titles,
        }
    }
}

/// One link of the fallback chain (WIKI§5.2): an article that is known to exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub entity_id: String,
    pub lang: String,
    pub title: String,
    /// True at levels 2 and 4: the article is the **parent** entity's, which
    /// the interface has to say out loud (WIKI§7.3, "Article général : {titre}").
    pub via_parent: bool,
}

/// The four levels of WIKI§5.2, in order, reduced to those that actually exist.
///
/// An empty vector is level 5 — "rien" — and that is a normal outcome, not a
/// failure (WIKI§1).
///
/// Returns the **whole chain** rather than just its head for two reasons: the
/// tests read the order (WIKI§11), and the caller moves on to the next link when a
/// fetch comes back `Unavailable` — one wiki being unreachable is not a reason
/// to give up on a level another wiki could serve.
///
/// Level 2 (parent, requested language) sits before level 3 (entity, English)
/// deliberately: on Japanese and American models the French Wikipedia often
/// covers the generic model without splitting it by generation, so climbing to
/// the parent buys more coverage in French than switching to English does.
pub fn fallback_chain(entity: &EntityArticles, parent: Option<&EntityArticles>, requested: &str) -> Vec<Attempt> {
    let requested = wiki_lang(requested);
    let mut out: Vec<Attempt> = Vec::new();

    // Levels are pushed in order; `push` drops one that repeats a level already
    // taken. That is what collapses four levels to two when the requested
    // language *is* English, instead of offering the same article twice.
    let push = |source: Option<&EntityArticles>, lang: &str, via_parent: bool, out: &mut Vec<Attempt>| {
        let Some(source) = source else { return };
        let Some(title) = source.titles.get(lang) else { return };
        if out.iter().any(|a| a.entity_id == source.entity_id && a.lang == lang) {
            return;
        }
        out.push(Attempt {
            entity_id: source.entity_id.clone(),
            lang: lang.to_string(),
            title: title.clone(),
            via_parent,
        });
    };

    push(Some(entity), &requested, false, &mut out); // 1. entity, requested language
    push(parent, &requested, true, &mut out); // 2. parent, requested language
    push(Some(entity), FALLBACK_LANG, false, &mut out); // 3. entity, English
    push(parent, FALLBACK_LANG, true, &mut out); // 4. parent, English

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn articles(id: &str, pairs: &[(&str, &str)]) -> EntityArticles {
        let titles = pairs
            .iter()
            .map(|(lang, title)| ((*lang).to_string(), (*title).to_string()))
            .collect();
        EntityArticles::new(id, titles)
    }

    /// Rule (WIKI§5.1): an app locale is truncated to its wiki language code.
    #[test]
    fn locale_truncates_to_its_language_code() {
        assert_eq!(wiki_lang("pt-BR"), "pt", "regional variant falls back to its language");
        assert_eq!(wiki_lang("fr"), "fr", "plain code untouched");
        assert_eq!(wiki_lang("FR"), "fr", "case must not travel into a hostname");
        assert_eq!(wiki_lang("zh_CN"), "zh", "underscore separates too");
    }

    /// Rule (WIKI§5.2): the four levels come out in the order the spec fixes.
    #[test]
    fn the_chain_walks_the_four_levels_in_order() {
        let entity = articles("Q1", &[("fr", "AE86 fr"), ("en", "AE86 en")]);
        let parent = articles("Q2", &[("fr", "Corolla fr"), ("en", "Corolla en")]);

        let chain = fallback_chain(&entity, Some(&parent), "fr");

        let seen: Vec<(&str, &str, bool)> = chain
            .iter()
            .map(|a| (a.entity_id.as_str(), a.lang.as_str(), a.via_parent))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("Q1", "fr", false),
                ("Q2", "fr", true),
                ("Q1", "en", false),
                ("Q2", "en", true)
            ],
            "entity/requested, parent/requested, entity/en, parent/en"
        );
    }

    /// Rule (WIKI§5.2): the case the spec singles out — only the **parent** has an
    /// article in the requested language. It must win over the entity's own
    /// English article, which is only level 3.
    #[test]
    fn the_parent_in_the_requested_language_beats_the_entity_in_english() {
        let entity = articles("Q1", &[("en", "AE86 en")]);
        let parent = articles("Q2", &[("fr", "Corolla fr"), ("en", "Corolla en")]);

        let chain = fallback_chain(&entity, Some(&parent), "fr");

        assert_eq!(chain[0].entity_id, "Q2", "the parent's French article comes first");
        assert_eq!(chain[0].lang, "fr");
        assert!(chain[0].via_parent, "and the interface must be told whose it is");
        assert_eq!(chain[1].entity_id, "Q1", "the entity's English article is level 3");
    }

    /// Rule (WIKI§5.2): asking for English collapses the chain to two links rather
    /// than offering the same article twice.
    #[test]
    fn asking_for_english_does_not_duplicate_levels() {
        let entity = articles("Q1", &[("en", "AE86 en")]);
        let parent = articles("Q2", &[("en", "Corolla en")]);

        let chain = fallback_chain(&entity, Some(&parent), "en");

        assert_eq!(chain.len(), 2, "levels 3 and 4 repeat levels 1 and 2");
        assert_eq!(chain[0].entity_id, "Q1");
        assert_eq!(chain[1].entity_id, "Q2");
    }

    /// Rule (WIKI§5.3): no parent means the chain skips levels 2 and 4 — it never
    /// climbs any higher looking for one.
    #[test]
    fn without_a_parent_only_the_entity_levels_remain() {
        let entity = articles("Q1", &[("fr", "AE86 fr"), ("en", "AE86 en")]);

        let chain = fallback_chain(&entity, None, "fr");

        assert_eq!(chain.len(), 2, "levels 2 and 4 have no source");
        assert!(
            chain.iter().all(|a| !a.via_parent),
            "nothing claims to be a parent article"
        );
    }

    /// Rule (WIKI§5.2, level 5): nothing anywhere is an empty chain, not an error.
    #[test]
    fn nothing_in_either_language_is_an_empty_chain() {
        let entity = articles("Q1", &[("ja", "AE86 ja")]);
        let parent = articles("Q2", &[("ja", "Corolla ja")]);

        assert!(fallback_chain(&entity, Some(&parent), "fr").is_empty(), "level 5: rien");
    }

    /// Rule (WIKI§5.1): the requested language goes through the same truncation as
    /// the app locale — `fr-CA` must not miss the French article.
    #[test]
    fn a_regional_locale_still_finds_its_language() {
        let entity = articles("Q1", &[("fr", "AE86 fr")]);

        let chain = fallback_chain(&entity, None, "fr-CA");

        assert_eq!(chain.len(), 1, "fr-CA is served by fr.wikipedia.org");
        assert_eq!(chain[0].lang, "fr");
    }
}
