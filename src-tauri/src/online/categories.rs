//! What the Online page knows of track categories (SPEC-play-online.md,
//! "Catégorie de circuit"), from the sources the library does not cover.
//!
//! A track's category resolves in this order, on the screen
//! (`src/lib/online/categories.ts`): the user's choice, the library, the
//! catalogue, then the words of the server itself. The library half travels
//! with the list (`Looks`); this module hands the three others over — the
//! choices made on the page (`overlay.sqlite`), the catalogue's table of
//! tracks and its table of keywords (`taxonomy-catalog.json` and its overlay,
//! REGLES§2) — with the closed vocabulary they are all drawn from: the rules'
//! allowlist of track categories.
//!
//! A category outside that vocabulary is dropped and logged rather than shown:
//! a value the filter offers must be one the library can carry too.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::Serialize;

use crate::rules::Rules;
use pitbox_catalog::taxonomy::category_list;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct CategoryKnowledge {
    /// The rules' allowlist of track categories, `#`-prefixed, in its order:
    /// what a choice is made from.
    pub vocabulary: Vec<String>,
    /// The user's choices, lowercase track id → categories.
    pub chosen: BTreeMap<String, Vec<String>>,
    /// The catalogue's table, overlay applied: lowercase track id →
    /// categories.
    pub catalog: BTreeMap<String, Vec<String>>,
    /// Keyword (lowercased) → the category it announces.
    pub keywords: BTreeMap<String, String>,
}

/// The vocabulary of a set of rules: its allowlist, normalised.
fn vocabulary(rules: &Rules) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in &rules.track.category_allowlist {
        for c in category_list(c) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// The categories of `value` the vocabulary knows; the others are logged.
fn known(value: &str, vocabulary: &[String], what: &str) -> Vec<String> {
    let (kept, dropped): (Vec<String>, Vec<String>) =
        category_list(value).into_iter().partition(|c| vocabulary.contains(c));
    if !dropped.is_empty() {
        log::warn!("online categories: {what}: {dropped:?} not in the track categories, ignored");
    }
    kept
}

/// Everything the page needs, read in one go.
pub fn knowledge(conn: &Connection, rules: &Rules) -> CategoryKnowledge {
    let vocabulary = vocabulary(rules);
    let chosen = match crate::overlay::list_chosen_track_categories(conn) {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("online categories: cannot read the choices — {e}");
            Vec::new()
        }
    };
    let chosen = chosen
        .into_iter()
        .map(|(id, cats)| {
            let cats = known(&cats, &vocabulary, &format!("choice for {id}"));
            (id.to_lowercase(), cats)
        })
        .filter(|(_, cats)| !cats.is_empty())
        .collect();
    let catalog = rules
        .online_track_categories
        .iter()
        .map(|(id, cats)| {
            (
                id.to_lowercase(),
                known(cats, &vocabulary, &format!("catalogue entry {id}")),
            )
        })
        .filter(|(_, cats)| !cats.is_empty())
        .collect();
    let keywords = rules
        .online_keywords
        .iter()
        .filter_map(|(word, cat)| {
            let cat = known(cat, &vocabulary, &format!("keyword {word}")).into_iter().next()?;
            let word = word.trim().to_lowercase();
            (!word.is_empty()).then_some((word, cat))
        })
        .collect();
    CategoryKnowledge {
        vocabulary,
        chosen,
        catalog,
        keywords,
    }
}

/// Records the categories the user chose for a track — none forgets the
/// choice. Only categories of the vocabulary: the screen offers nothing else,
/// so another value is a caller's mistake, refused rather than stored.
pub fn choose(conn: &Connection, rules: &Rules, track_id: &str, categories: &[String]) -> Result<(), String> {
    let id = track_id.trim().to_lowercase();
    if id.is_empty() {
        return Err("no track id".into());
    }
    let vocabulary = vocabulary(rules);
    let cats = category_list(&categories.join(" "));
    if let Some(bad) = cats.iter().find(|c| !vocabulary.contains(c)) {
        return Err(format!("{bad} is not a track category"));
    }
    if cats.is_empty() {
        crate::overlay::clear_chosen_track_categories(conn, &id).map_err(|e| e.to_string())
    } else {
        let at = chrono::Local::now().to_rfc3339();
        crate::overlay::set_chosen_track_categories(conn, &id, &cats.join(" "), &at).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        let mut r = Rules::default();
        r.track.category_allowlist = vec!["#drift".into(), "#touge".into(), "freeroam".into()];
        r.online_track_categories
            .insert("PK_Akina".into(), "#touge, #nonsense".into());
        r.online_keywords.insert("Drift".into(), "#drift".into());
        r.online_keywords.insert("race".into(), "#circuit".into());
        r
    }

    /// Rule (SPEC-play-online.md): every source draws from the closed
    /// vocabulary of track categories; what it does not know is dropped.
    #[test]
    fn every_source_draws_from_the_vocabulary() {
        let base = crate::testutil::temp_dir("online-categories");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let k = knowledge(&conn, &rules());
        assert_eq!(
            k.vocabulary,
            vec!["#drift", "#touge", "#freeroam"],
            "normalised, in order"
        );
        assert_eq!(k.catalog["pk_akina"], vec!["#touge"], "an unknown category is dropped");
        assert_eq!(
            k.keywords.get("drift").map(String::as_str),
            Some("#drift"),
            "keys lowercased"
        );
        assert!(
            !k.keywords.contains_key("race"),
            "a keyword towards no category is dropped"
        );
    }

    /// Rule: a choice is stored by track id, replaced by the next one, and an
    /// empty choice forgets it. A category outside the vocabulary is refused.
    #[test]
    fn a_choice_is_stored_replaced_and_forgotten() {
        let base = crate::testutil::temp_dir("online-choice");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let r = rules();
        choose(&conn, &r, "LA_Canyons", &["#drift".into()]).unwrap();
        choose(&conn, &r, "la_canyons", &["touge".into(), "#freeroam".into()]).unwrap();
        assert_eq!(knowledge(&conn, &r).chosen["la_canyons"], vec!["#touge", "#freeroam"]);
        assert!(
            choose(&conn, &r, "la_canyons", &["#oval".into()]).is_err(),
            "not in the vocabulary"
        );
        choose(&conn, &r, "la_canyons", &[]).unwrap();
        assert!(knowledge(&conn, &r).chosen.is_empty(), "forgotten");
    }
}
