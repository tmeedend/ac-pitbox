//! Matching a track mod to a Wikidata entity, by coordinates first (§4.2).
//!
//! Coordinates beat names here, and the spec gives the reason: Nordschleife is
//! an article called Nürburgring, Shutoko is the Metropolitan Expressway, and
//! an Initial D pass is a mountain nobody spells the same way twice.
//!
//! Two corrections to §4.2, both measured rather than reasoned:
//!
//! - **The search runs on Wikidata, not on a language wiki.** The English
//!   article "Nürburgring" carries no GeoData coordinates at all, and neither
//!   does Suzuka's, so `list=geosearch` against en.wikipedia can never return
//!   the very circuit the spec uses as its example. The Wikidata item does
//!   carry `P625`, and searching there yields Q-ids directly.
//! - **The coordinates come from CSP, not from `ui_track.json`.** `sun.rs`
//!   already resolves them from `data_track_params.ini` — 445 tracks, layouts
//!   included — and falls back to the mod's geotags. Reading geotags alone
//!   would fail on every Kunos track, whose `geotags` field is the literal
//!   placeholder `["lat", "lon"]`.

use super::api::{EntityDetails, Fetched, WikiClient};
use super::clean::Cleaner;
use super::ids;
use super::matching::{decide, name_similarity, Candidate, MatchOutcome, Thresholds};

/// How many neighbours to ask for.
///
/// Fifty and not ten, and this is not caution: measured five metres from the
/// Nordschleife, **twenty items share the exact same coordinate** — nineteen
/// Grand Prix editions and the circuit — and the order between equals is
/// arbitrary. With a small limit the circuit is simply not in the answer. Fifty
/// is also the ceiling of the single `details` call that follows.
const GEO_LIMIT: u32 = 50;

/// How many name hits to consider when there are no coordinates at all.
const SEARCH_LIMIT: u32 = 20;

/// What the mod says about itself.
#[derive(Debug, Clone, Default)]
pub struct TrackSubject {
    /// The mod's display name, before cleaning.
    pub name: Option<String>,
    /// Latitude and longitude, resolved by the caller — `sun::track_location`
    /// in the app, a literal in the tests.
    pub location: Option<(f64, f64)>,
}

/// Proximity as a score in `[0, 1]`, so the calibration report can put cars and
/// tracks in one table. It is **not** a car score: it says "near", not
/// "likely".
fn proximity(distance_m: f64, radius_m: u32) -> f64 {
    let radius = radius_m.max(1) as f64;
    (1.0 - distance_m / radius).clamp(0.0, 1.0)
}

/// Drops a candidate that is `part of` another candidate still in the running.
///
/// This is §4.2's "apparier au niveau du circuit, pas de la configuration",
/// done with claims already in hand: a layout item points at its circuit
/// through `P361`, so the child is the one to drop. It also spares a genuine
/// tie — circuit and configuration sit at the same coordinate, and without
/// this the pair would be rejected as ambiguous.
pub fn collapse_configurations(candidates: Vec<(EntityDetails, f64)>) -> Vec<(EntityDetails, f64)> {
    let present: Vec<String> = candidates.iter().map(|(d, _)| d.entity_id.clone()).collect();
    candidates
        .into_iter()
        .filter(|(details, _)| {
            let child_of_a_finalist = details.part_of.iter().any(|parent| present.contains(parent));
            if child_of_a_finalist {
                log::debug!("wiki/track: {} est une partie d'un autre candidat", details.entity_id);
            }
            !child_of_a_finalist
        })
        .collect()
}

/// The verdict for a geographic search: nearest wins, unless two are so close
/// together that "nearest" means nothing.
///
/// Pure — the tests drive this directly, coordinates and all.
pub fn decide_by_distance(candidates: Vec<(EntityDetails, f64)>, thresholds: &Thresholds) -> MatchOutcome {
    let accepted: Vec<(EntityDetails, f64)> = candidates
        .into_iter()
        .filter(|(details, _)| details.is_of_type(&ids::TRACK_TYPES))
        .collect();
    let mut accepted = collapse_configurations(accepted);
    accepted.sort_by(|a, b| a.1.total_cmp(&b.1));

    let to_candidate = |(details, distance): &(EntityDetails, f64)| Candidate {
        entity_id: details.entity_id.clone(),
        label: details.label.clone(),
        description: details.description.clone(),
        score: proximity(*distance, thresholds.track_radius_m),
    };

    let Some(best) = accepted.first() else {
        return MatchOutcome::NoCandidate;
    };
    let Some(second) = accepted.get(1) else {
        return MatchOutcome::Matched {
            candidate: to_candidate(best),
            runner_up: None,
            margin: None,
        };
    };

    // The tie is judged in **metres**, not in score: a hundred metres means the
    // same thing whatever the search radius, and the radius is a setting.
    let gap_m = second.1 - best.1;
    let best_c = to_candidate(best);
    let second_c = to_candidate(second);
    let margin = best_c.score - second_c.score;
    if gap_m < thresholds.track_tie_margin_m {
        return MatchOutcome::Ambiguous {
            best: best_c,
            runner_up: second_c,
            margin,
        };
    }
    MatchOutcome::Matched {
        candidate: best_c,
        runner_up: Some(second_c),
        margin: Some(margin),
    }
}

/// The name fallback of §4.2.4, used when the track has no coordinates.
///
/// Nothing but name similarity: a track has no brand and no production period,
/// so the car score's other two signals have no input here. The type allowlist
/// is the track one.
pub fn rank_by_name(details: &[EntityDetails], cleaned_name: &str, thresholds: &Thresholds) -> MatchOutcome {
    let candidates: Vec<Candidate> = details
        .iter()
        .filter(|d| d.is_of_type(&ids::TRACK_TYPES))
        .map(|d| Candidate {
            entity_id: d.entity_id.clone(),
            label: d.label.clone(),
            description: d.description.clone(),
            score: name_similarity(cleaned_name, d.label.as_deref().unwrap_or_default()),
        })
        .collect();
    decide(candidates, thresholds)
}

/// The whole §4.2 pipeline, network included.
pub fn match_track(
    net: &WikiClient,
    cleaner: &Cleaner,
    thresholds: &Thresholds,
    subject: &TrackSubject,
) -> MatchOutcome {
    if let Some((latitude, longitude)) = subject.location {
        return by_coordinates(net, thresholds, latitude, longitude);
    }
    by_name(net, cleaner, thresholds, subject)
}

fn by_coordinates(net: &WikiClient, thresholds: &Thresholds, latitude: f64, longitude: f64) -> MatchOutcome {
    let hits = match net.geosearch(latitude, longitude, thresholds.track_radius_m, GEO_LIMIT) {
        Fetched::Found(hits) => hits,
        Fetched::Absent => return MatchOutcome::NoCandidate,
        Fetched::Unavailable => return MatchOutcome::Unavailable,
    };
    let ids: Vec<String> = hits.iter().map(|h| h.entity_id.clone()).collect();
    let details = match net.details(&ids) {
        Fetched::Found(details) => details,
        Fetched::Absent => return MatchOutcome::NoCandidate,
        Fetched::Unavailable => return MatchOutcome::Unavailable,
    };
    // `details` comes back keyed by id, in no particular order; the distance
    // lives in the geosearch answer, so the two are joined here.
    let paired: Vec<(EntityDetails, f64)> = details
        .into_iter()
        .filter_map(|d| {
            let hit = hits.iter().find(|h| h.entity_id == d.entity_id)?;
            Some((d, hit.distance_m))
        })
        .collect();
    decide_by_distance(paired, thresholds)
}

fn by_name(net: &WikiClient, cleaner: &Cleaner, thresholds: &Thresholds, subject: &TrackSubject) -> MatchOutcome {
    let cleaned = cleaner.clean(subject.name.as_deref().unwrap_or_default());
    if cleaned.trim().is_empty() {
        return MatchOutcome::NoCandidate;
    }
    let hits = match net.search(&cleaned, "en", SEARCH_LIMIT) {
        Fetched::Found(hits) => hits,
        Fetched::Absent => return MatchOutcome::NoCandidate,
        Fetched::Unavailable => return MatchOutcome::Unavailable,
    };
    let ids: Vec<String> = hits.into_iter().map(|h| h.entity_id).collect();
    match net.details(&ids) {
        Fetched::Found(details) => rank_by_name(&details, &cleaned, thresholds),
        Fetched::Absent => MatchOutcome::NoCandidate,
        Fetched::Unavailable => MatchOutcome::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thresholds() -> Thresholds {
        Thresholds {
            min_score: 0.55,
            min_margin: 0.12,
            track_radius_m: 5_000,
            track_tie_margin_m: 150.0,
        }
    }

    fn item(id: &str, label: &str, kind: &str) -> EntityDetails {
        EntityDetails {
            entity_id: id.into(),
            label: Some(label.into()),
            types: vec![kind.into()],
            ..EntityDetails::default()
        }
    }

    /// Rule (§4.2.2, measured): **the type filter carries the whole strategy.**
    ///
    /// This is the Nordschleife, as the API really answers it: nineteen Grand
    /// Prix editions at 4.9 m, the circuit further down the list. Without the
    /// filter, "the nearest" matches the track to a race that took place on it.
    #[test]
    fn the_nearest_thing_to_a_circuit_is_not_a_circuit() {
        let candidates = vec![
            (item("Q8069", "Luxembourg Grand Prix", "Q18608583"), 4.9),
            (item("Q164885", "1000 km Nürburgring", "Q18608583"), 4.9),
            (item("Q628095", "Nürburg", "Q486972"), 809.0),
            (item("Q152207", "Nürburgring", ids::MOTORSPORT_RACING_TRACK), 4.9),
        ];

        let outcome = decide_by_distance(candidates, &thresholds());

        match outcome {
            MatchOutcome::Matched { candidate, .. } => {
                assert_eq!(candidate.entity_id, "Q152207", "le circuit, pas le Grand Prix");
            }
            other => panic!("le circuit doit être retenu, pas {other:?}"),
        }
    }

    /// Rule (§4.2): "apparier au niveau du circuit, pas de la configuration".
    /// A layout item points at its circuit through `part of`, and sits at the
    /// same coordinate — so without the collapse the pair would be rejected as
    /// a tie, and the mod would get nothing.
    #[test]
    fn a_configuration_gives_way_to_its_circuit() {
        let mut layout = item("Q999", "Nürburgring Nordschleife", ids::MOTORSPORT_RACING_TRACK);
        layout.part_of = vec!["Q152207".into()];
        let candidates = vec![
            (layout, 4.9),
            (item("Q152207", "Nürburgring", ids::MOTORSPORT_RACING_TRACK), 5.0),
        ];

        let outcome = decide_by_distance(candidates, &thresholds());

        match outcome {
            MatchOutcome::Matched {
                candidate, runner_up, ..
            } => {
                assert_eq!(candidate.entity_id, "Q152207", "le circuit porte l'appariement");
                assert_eq!(runner_up, None, "la configuration n'est plus en lice");
            }
            other => panic!("la configuration doit s'effacer, pas {other:?}"),
        }
    }

    /// Rule (§1): two unrelated circuits at the same place is an ambiguity, and
    /// an ambiguity produces nothing — even here, where the spec only says
    /// "le plus proche".
    #[test]
    fn two_unrelated_circuits_at_the_same_spot_produce_nothing() {
        let candidates = vec![
            (item("Q1", "Circuit A", ids::MOTORSPORT_RACING_TRACK), 40.0),
            (item("Q2", "Circuit B", ids::RACE_TRACK), 60.0),
        ];

        let outcome = decide_by_distance(candidates, &thresholds());

        match outcome {
            MatchOutcome::Ambiguous { best, runner_up, .. } => {
                assert_eq!(best.entity_id, "Q1");
                assert_eq!(runner_up.entity_id, "Q2");
            }
            other => panic!("vingt mètres d'écart ne départagent rien, pas {other:?}"),
        }
    }

    /// Rule (§4.2.3): once they are far enough apart, the nearest wins.
    #[test]
    fn far_enough_apart_the_nearest_wins() {
        let candidates = vec![
            (item("Q1", "Circuit A", ids::MOTORSPORT_RACING_TRACK), 40.0),
            (item("Q2", "Circuit B", ids::RACE_TRACK), 900.0),
        ];

        match decide_by_distance(candidates, &thresholds()) {
            MatchOutcome::Matched { candidate, margin, .. } => {
                assert_eq!(candidate.entity_id, "Q1");
                assert!(margin.expect("marge") > 0.0);
            }
            other => panic!("860 m d'écart départagent, pas {other:?}"),
        }
    }

    /// Rule (§4.2.2): nothing of an accepted type nearby is a non-result, not
    /// a fallback onto the village next door.
    #[test]
    fn a_neighbourhood_without_a_track_matches_nothing() {
        let candidates = vec![
            (item("Q628095", "Nürburg", "Q486972"), 12.0),
            (item("Q182382", "Nürburg Castle", "Q23413"), 20.0),
        ];
        assert_eq!(decide_by_distance(candidates, &thresholds()), MatchOutcome::NoCandidate);
    }

    /// Rule (§4.2.1): proximity is reported as a score so one table can hold
    /// both strategies — nearest is 1, the edge of the radius is 0.
    #[test]
    fn proximity_spans_the_radius() {
        assert!((proximity(0.0, 5_000) - 1.0).abs() < 1e-9);
        assert!((proximity(5_000.0, 5_000)).abs() < 1e-9);
        assert!((proximity(2_500.0, 5_000) - 0.5).abs() < 1e-9);
    }
}
