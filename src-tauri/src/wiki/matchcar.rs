//! Matching a car mod to a Wikidata entity, by name (§4.1).
//!
//! Source: the `ui_car.json` fields the app already reads — `brand`, `name`,
//! `year`. Two requests: a name search, then one batched fetch of the
//! candidates' types and claims.
//!
//! The scoring is a pure function of what came back, so §11's requirement —
//! "rejet effectif en cas de candidats proches ; acceptation du modèle
//! générique quand la génération est absente" — is tested without a network.

use super::api::{EntityDetails, Fetched, WikiClient};
use super::clean::{Cleaner, Weights};
use super::ids;
use super::matching::{decide, name_similarity, Candidate, MatchOutcome, Thresholds};

/// How many search hits to consider. Twenty is well inside the one batched
/// `details` call that follows (fifty ids), and a name that needs more than
/// twenty is a name that will be ambiguous anyway.
const SEARCH_LIMIT: u32 = 20;

/// What the mod says about itself.
#[derive(Debug, Clone, Default)]
pub struct CarSubject {
    pub brand: Option<String>,
    pub name: Option<String>,
    pub year: Option<i64>,
}

/// The string to search for: brand and name, cleaned (§4.3).
///
/// The brand is prepended only when the cleaned name does not already start
/// with it — modders write both `Toyota AE86` and `AE86` under brand `Toyota`,
/// and searching "Toyota Toyota AE86" scores worse on every candidate.
pub fn search_query(cleaner: &Cleaner, subject: &CarSubject) -> String {
    let name = cleaner.clean(subject.name.as_deref().unwrap_or_default());
    let brand = subject.brand.as_deref().map(str::trim).unwrap_or_default();
    if brand.is_empty() {
        return name;
    }
    if name.to_lowercase().starts_with(&brand.to_lowercase()) {
        return name;
    }
    format!("{brand} {name}").trim().to_string()
}

/// The score of §4.1.4, in `[0, 1]`.
///
/// Three signals, and the third is special: **the year is a bonus when it
/// exists and never a penalty when it does not**. Measured on the real API —
/// no car item carries `P571`, and only generations carry `P580`/`P582`. A
/// missing period is therefore the normal case, and weighting it as a zero
/// would systematically punish the generic model that §4.1 explicitly wants to
/// accept. So its weight is removed from the denominator instead.
///
/// The brand is read from the candidate's **label and description** rather
/// than resolving `P176` to a label, which would cost another request per car:
/// Wikidata's own description says it in plain words ("automobile produced by
/// Toyota", measured on Q2626308).
pub fn score_car(details: &EntityDetails, subject: &CarSubject, cleaned_name: &str, weights: &Weights) -> f64 {
    let label = details.label.as_deref().unwrap_or_default();
    let name = weights.name * name_similarity(cleaned_name, label);

    let brand_text = format!("{label} {}", details.description.as_deref().unwrap_or_default());
    let brand = match subject.brand.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => weights.brand * brand_match(b, &brand_text),
        // No brand in the mod's file is not the candidate's fault: drop the
        // signal rather than scoring it zero, same reasoning as the year.
        None => 0.0,
    };
    let brand_weight = if subject.brand.as_deref().is_some_and(|b| !b.trim().is_empty()) {
        weights.brand
    } else {
        0.0
    };

    let (year, year_weight) = match (subject.year, details.start_year) {
        (Some(wanted), Some(start)) => {
            let end = details.end_year.unwrap_or(start);
            (weights.year * year_fit(wanted, start, end), weights.year)
        }
        _ => (0.0, 0.0),
    };

    let total_weight = weights.name + brand_weight + year_weight;
    if total_weight <= 0.0 {
        return 0.0;
    }
    (name + brand + year) / total_weight
}

/// Does the brand appear in the candidate's own words? Every token of the
/// brand must be there — "Alfa Romeo" must not be satisfied by "Alfa".
fn brand_match(brand: &str, haystack: &str) -> f64 {
    let haystack = haystack.to_lowercase();
    let mut any = false;
    for token in brand.split_whitespace() {
        any = true;
        if !haystack.contains(&token.to_lowercase()) {
            return 0.0;
        }
    }
    if any {
        1.0
    } else {
        0.0
    }
}

/// How well a model year sits in a production period. One year of slack on
/// each side: a mod's `year` is the model year, which routinely precedes the
/// start of production by a few months.
fn year_fit(wanted: i64, start: i64, end: i64) -> f64 {
    if wanted >= start - 1 && wanted <= end + 1 {
        return 1.0;
    }
    let gap = if wanted < start { start - wanted } else { wanted - end };
    if gap <= 5 {
        0.5
    } else {
        0.0
    }
}

/// Turns the candidates that survived the type filter into a verdict.
/// Pure: this is what the tests drive.
pub fn rank(
    details: &[EntityDetails],
    subject: &CarSubject,
    cleaned_name: &str,
    weights: &Weights,
    thresholds: &Thresholds,
) -> MatchOutcome {
    let candidates: Vec<Candidate> = details
        .iter()
        .filter(|d| d.is_of_type(&ids::CAR_TYPES))
        .map(|d| Candidate {
            entity_id: d.entity_id.clone(),
            label: d.label.clone(),
            description: d.description.clone(),
            score: score_car(d, subject, cleaned_name, weights),
        })
        .collect();
    decide(candidates, thresholds)
}

/// The whole §4.1 pipeline, network included.
pub fn match_car(
    net: &WikiClient,
    cleaner: &Cleaner,
    weights: &Weights,
    thresholds: &Thresholds,
    subject: &CarSubject,
) -> MatchOutcome {
    let cleaned_name = cleaner.clean(subject.name.as_deref().unwrap_or_default());
    let query = search_query(cleaner, subject);
    if query.trim().is_empty() {
        return MatchOutcome::NoCandidate;
    }

    // English, whatever the reading language will be: it has the widest
    // coverage, and the type filter that follows is language-independent. What
    // the user reads is settled later and separately, by §5.2.
    let hits = match net.search_pages("en", &query, SEARCH_LIMIT) {
        Fetched::Found(hits) => hits,
        Fetched::Absent => return MatchOutcome::NoCandidate,
        Fetched::Unavailable => return MatchOutcome::Unavailable,
    };
    let ids: Vec<String> = hits.iter().map(|h| h.entity_id.clone()).collect();

    match net.details(&ids) {
        Fetched::Found(mut details) => {
            super::borrow_labels(&mut details, &hits);
            rank(&details, subject, &cleaned_name, weights, thresholds)
        }
        Fetched::Absent => MatchOutcome::NoCandidate,
        Fetched::Unavailable => MatchOutcome::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::clean::MatchingConfig;
    use crate::wiki::matching::Thresholds;

    fn thresholds() -> Thresholds {
        Thresholds {
            min_score: 0.55,
            min_margin: 0.12,
            track_radius_m: 5_000,
            track_tie_margin_m: 150.0,
        }
    }

    fn car(id: &str, label: &str, description: &str, years: Option<(i64, i64)>) -> EntityDetails {
        EntityDetails {
            entity_id: id.into(),
            label: Some(label.into()),
            description: Some(description.into()),
            types: vec![ids::CAR_MODEL.into()],
            start_year: years.map(|(s, _)| s),
            end_year: years.map(|(_, e)| e),
            ..EntityDetails::default()
        }
    }

    fn subject(brand: &str, name: &str, year: Option<i64>) -> CarSubject {
        CarSubject {
            brand: Some(brand.into()),
            name: Some(name.into()),
            year,
        }
    }

    /// Rule (§4.1.1): the brand is not repeated when the name already carries
    /// it — "Toyota Toyota AE86" scores worse against every candidate.
    #[test]
    fn the_query_does_not_say_the_brand_twice() {
        let cleaner = Cleaner::new(&MatchingConfig::default());
        assert_eq!(
            search_query(&cleaner, &subject("Toyota", "AE86 v1.2", None)),
            "Toyota AE86"
        );
        assert_eq!(
            search_query(&cleaner, &subject("Toyota", "Toyota AE86 [4K]", None)),
            "Toyota AE86"
        );
    }

    /// Rule (§4.1, §11): **the generic model is accepted when the generation is
    /// absent.** The spec says so in as many words — "Toyota Corolla" is a
    /// valid result for an AE86 mod.
    #[test]
    fn the_generic_model_is_accepted_when_the_generation_is_absent() {
        let details = vec![
            car("Q2626308", "Toyota Corolla", "automobile produced by Toyota", None),
            car("Q1", "Nissan Skyline", "automobile produced by Nissan", None),
        ];
        let subject = subject("Toyota", "Corolla AE86", Some(1986));

        let outcome = rank(&details, &subject, "Corolla AE86", &Weights::default(), &thresholds());

        match outcome {
            MatchOutcome::Matched { candidate, .. } => assert_eq!(candidate.entity_id, "Q2626308"),
            other => panic!("le modèle générique doit être retenu, pas {other:?}"),
        }
    }

    /// Rule (§1, §4.1.5, §11): **two plausible candidates produce nothing.**
    /// Two generations of the same model, both matching the brand and the
    /// name, is the realistic ambiguity — and it must not be resolved.
    #[test]
    fn two_generations_of_the_same_model_produce_nothing() {
        let details = vec![
            car("Q1", "Toyota Supra", "automobile produced by Toyota", None),
            car("Q2", "Toyota Supra", "sports car produced by Toyota", None),
        ];
        let subject = subject("Toyota", "Supra", None);

        let outcome = rank(&details, &subject, "Supra", &Weights::default(), &thresholds());

        match outcome {
            MatchOutcome::Ambiguous { margin, .. } => assert!(margin < 0.12, "écart {margin}"),
            other => panic!("deux candidats identiques sont ambigus, pas {other:?}"),
        }
    }

    /// Rule (§4.1.3): what is not of an accepted type is not a candidate at
    /// all. A video game named after a car is the case that costs the most —
    /// it matches the name perfectly.
    #[test]
    fn something_that_is_not_a_car_model_is_not_a_candidate() {
        let mut game = car("Q9", "Toyota Supra", "video game", None);
        game.types = vec!["Q7889".into()]; // video game
        let details = vec![game];

        let outcome = rank(
            &details,
            &subject("Toyota", "Supra", None),
            "Supra",
            &Weights::default(),
            &thresholds(),
        );

        assert_eq!(
            outcome,
            MatchOutcome::NoCandidate,
            "le filtre de type passe avant le score"
        );
    }

    /// Rule (§4.1.4, measured): a missing production period is **neutral**, not
    /// a zero.
    ///
    /// No car item measured carries `P571`, and only generations carry
    /// `P580`/`P582` — so "no period" is the ordinary case, the generic model
    /// of §4.1 included. Neutral means two things: the candidate scores exactly
    /// what it would if no year had been asked for (the weight leaves the
    /// denominator), and it stays **ahead** of a candidate whose period
    /// actually contradicts the year.
    #[test]
    fn a_missing_production_period_is_neutral_not_a_zero() {
        let weights = Weights::default();
        let undated = car("Q2", "Toyota Corolla", "automobile produced by Toyota", None);

        let asked = score_car(&undated, &subject("Toyota", "Corolla", Some(1986)), "Corolla", &weights);
        let not_asked = score_car(&undated, &subject("Toyota", "Corolla", None), "Corolla", &weights);
        assert!(
            (asked - not_asked).abs() < 1e-9,
            "{asked} vs {not_asked} : demander une année ne change rien à qui n'en a pas"
        );

        let wrong_period = car(
            "Q3",
            "Toyota Corolla",
            "automobile produced by Toyota",
            Some((2015, 2020)),
        );
        let contradicted = score_car(
            &wrong_period,
            &subject("Toyota", "Corolla", Some(1986)),
            "Corolla",
            &weights,
        );
        assert!(asked > contradicted, "l'absence vaut mieux qu'une période qui dément");
    }

    /// Rule (§4.1.4): a period that exists and excludes the year does lower the
    /// score — that is the whole point of reading it.
    #[test]
    fn a_period_that_excludes_the_year_lowers_the_score() {
        let subject = subject("Toyota", "Corolla", Some(1986));
        let weights = Weights::default();

        let right = car(
            "Q1",
            "Toyota Corolla",
            "automobile produced by Toyota",
            Some((1983, 1987)),
        );
        let wrong = car(
            "Q2",
            "Toyota Corolla",
            "automobile produced by Toyota",
            Some((2015, 2020)),
        );

        assert!(
            score_car(&right, &subject, "Corolla", &weights) > score_car(&wrong, &subject, "Corolla", &weights),
            "la période cohérente doit l'emporter"
        );
    }

    /// Rule (§4.1.4): the brand must match in full. "Alfa Romeo" is not
    /// satisfied by an entity whose words only contain "Alfa".
    #[test]
    fn a_brand_matches_whole_or_not_at_all() {
        assert_eq!(brand_match("Alfa Romeo", "Alfa Romeo Giulia, automobile"), 1.0);
        assert_eq!(brand_match("Alfa Romeo", "Alfa, a kit car"), 0.0);
        assert_eq!(brand_match("Toyota", "automobile produced by Toyota"), 1.0);
    }
}
