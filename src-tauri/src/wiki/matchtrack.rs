//! Matching a track mod to a Wikidata entity, by coordinates first (§4.2).
//!
//! Coordinates beat names here, and the spec gives the reason: Nordschleife is
//! an article called Nürburgring, Shutoko is the Metropolitan Expressway, and
//! an Initial D pass is a mountain nobody spells the same way twice.
//!
//! Four corrections to §4.2, every one of them measured rather than reasoned —
//! the last two came out of the first calibration run over the real library:
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
//! - **Only purpose-built circuits are matched automatically.** §4.2 widens the
//!   allowlist to roads and passes, and the intent is right, but the run showed
//!   there is no signal to act on it with: a street is within reach of every
//!   coordinate on earth, and the name cannot arbitrate — the spec chose
//!   coordinates *because* "Shutoko" does not look like "Metropolitan
//!   Expressway". Roads produced four wrong articles for a handful of right
//!   ones, so §1 settles it. `decide_by_distance` carries the detail.
//! - **The radius is the API's ceiling, and it is not always enough.** Monza is
//!   listed at Milan's coordinates, eighteen kilometres away, while
//!   `list=geosearch` refuses anything over ten. What rescues those is the name
//!   fallback of §4.2.4, extended here to "no candidate found by coordinates".

use super::api::{EntityDetails, Fetched, WikiClient};
use super::clean::Cleaner;
use super::ids;
use super::matching::{decide, name_similarity, Candidate, MatchOutcome, Thresholds};

/// How many neighbours to ask for.
///
/// Two hundred, and every increase was paid for by a failure. Measured five
/// metres from the Nordschleife, **twenty items share the exact coordinate** —
/// nineteen Grand Prix editions and the circuit — and the order between equals
/// is arbitrary. Silverstone, with seventy British Grands Prix tagged at the
/// same spot, pushed its own circuit past any small limit: the calibration run
/// returned nothing for it. The candidates are fetched in batches of fifty and
/// the walk stops at the first circuit, so the usual case is still one call.
const GEO_LIMIT: u32 = 200;

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
    // **Circuits, and nothing else.** A purpose-built circuit nine hundred
    // metres away beats a street at five, because every circuit has streets at
    // its own coordinates — the calibration run rejected the Nordschleife,
    // Vallelunga and Zandvoort for tying with one.
    let circuits: Vec<(EntityDetails, f64)> = candidates
        .iter()
        .filter(|(d, _)| d.is_of_type(&ids::CIRCUIT_TYPES))
        .cloned()
        .collect();
    // **And no road at all when there is no circuit**, which the second
    // calibration run settled against the spec's own wish.
    //
    // §4.2 widens the allowlist to roads for Shutoko and the touge passes, and
    // the intent is right — but a road is *always* within reach: every set of
    // coordinates on earth has a street next to it. The run matched
    // `ks_barcelona` to "carrer de Sant Lluís", `trento-bondone` to "Via
    // Calepina", `la_canyons` to "Red Box" and the fictional
    // `ks_black_cat_county` to "Hill Street" — four wrong articles presented
    // as right, against a handful of real roads found.
    //
    // Nor can the name arbitrate: §4.2 chose coordinates precisely *because*
    // "Shutoko" does not look like "Metropolitan Expressway". So there is no
    // signal that separates the two situations, and §1 decides what to do
    // without one — nothing. Roads stay in `ids::ROUTE_TYPES` for the manual
    // correction of §7.6, which relaxes the type filter anyway, and for the
    // name fallback below where the name *did* agree.
    let mut accepted = collapse_configurations(circuits);
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
    locale: &str,
) -> MatchOutcome {
    if let Some((latitude, longitude)) = subject.location {
        let outcome = by_coordinates(net, thresholds, latitude, longitude);
        // Nothing of the right type within ten kilometres — the API's ceiling —
        // does not mean the track has no article: Monza's coordinates are
        // Milan's, eighteen kilometres from the circuit. So the name gets its
        // turn, which §4.2.4 already grants when there are no coordinates at
        // all. An **ambiguity is not retried**: it is a verdict (§1), and
        // neither is an unreachable network, which taught us nothing.
        if matches!(outcome, MatchOutcome::NoCandidate) {
            return by_name(net, cleaner, thresholds, subject, locale);
        }
        return outcome;
    }
    by_name(net, cleaner, thresholds, subject, locale)
}

fn by_coordinates(net: &WikiClient, thresholds: &Thresholds, latitude: f64, longitude: f64) -> MatchOutcome {
    let hits = match net.geosearch(latitude, longitude, thresholds.track_radius_m, GEO_LIMIT) {
        Fetched::Found(hits) => hits,
        Fetched::Absent => return MatchOutcome::NoCandidate,
        Fetched::Unavailable => return MatchOutcome::Unavailable,
    };
    // Fetched fifty at a time, nearest first, and **stopped at the first
    // circuit**: the hits are distance-ordered, so anything in a later batch is
    // farther away and cannot win. A track with a circuit next to it therefore
    // still costs one call; only the ones with none walk the whole list.
    let mut paired: Vec<(EntityDetails, f64)> = Vec::new();
    for batch in hits.chunks(super::api::MAX_BATCH) {
        let ids: Vec<String> = batch.iter().map(|h| h.entity_id.clone()).collect();
        match net.details(&ids) {
            Fetched::Found(details) => {
                // `details` comes back keyed by id, in no particular order; the
                // distance lives in the geosearch answer, so the two are joined
                // here.
                paired.extend(details.into_iter().filter_map(|d| {
                    let hit = hits.iter().find(|h| h.entity_id == d.entity_id)?;
                    Some((d, hit.distance_m))
                }));
            }
            Fetched::Absent => continue,
            // Half an answer is still an answer: what was already gathered is
            // nearer than what is missing.
            Fetched::Unavailable if paired.is_empty() => return MatchOutcome::Unavailable,
            Fetched::Unavailable => break,
        }
        if paired.iter().any(|(d, _)| d.is_of_type(&ids::CIRCUIT_TYPES)) {
            break;
        }
    }
    decide_by_distance(paired, thresholds)
}

fn by_name(
    net: &WikiClient,
    cleaner: &Cleaner,
    thresholds: &Thresholds,
    subject: &TrackSubject,
    locale: &str,
) -> MatchOutcome {
    let cleaned = cleaner.clean(subject.name.as_deref().unwrap_or_default());
    if cleaned.trim().is_empty() {
        return MatchOutcome::NoCandidate;
    }
    // Home wiki first, English second — same reason as the cars.
    for wiki in super::lang::search_order(locale) {
        let hits = match net.search_pages(&wiki, &cleaned, SEARCH_LIMIT) {
            Fetched::Found(hits) => hits,
            Fetched::Absent => continue,
            Fetched::Unavailable => return MatchOutcome::Unavailable,
        };
        let ids: Vec<String> = hits.iter().map(|h| h.entity_id.clone()).collect();
        let outcome = match net.details(&ids) {
            Fetched::Found(mut details) => {
                super::borrow_labels(&mut details, &hits);
                rank_by_name(&details, &cleaned, thresholds)
            }
            Fetched::Absent => MatchOutcome::NoCandidate,
            Fetched::Unavailable => return MatchOutcome::Unavailable,
        };
        if !matches!(outcome, MatchOutcome::NoCandidate) {
            return outcome;
        }
    }
    MatchOutcome::NoCandidate
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

    /// Rule (§4.2.2, calibrated): **a circuit beats a nearer street.**
    ///
    /// Straight from the calibration run: the Nordschleife tied with
    /// `Kurt-Bosch-Straße` and was rejected, Vallelunga with two Roman streets,
    /// Zandvoort with `Duinweg`. Every circuit has streets at its own
    /// coordinates, so distance alone can never separate them — the type does.
    #[test]
    fn a_circuit_beats_a_nearer_street() {
        let candidates = vec![
            (item("Q127041698", "Kurt-Bosch-Straße", ids::STREET), 5.0),
            (item("Q152207", "Nürburgring", ids::MOTORSPORT_RACING_TRACK), 900.0),
        ];

        match decide_by_distance(candidates, &thresholds()) {
            MatchOutcome::Matched {
                candidate, runner_up, ..
            } => {
                assert_eq!(candidate.entity_id, "Q152207", "le circuit, même plus loin");
                assert_eq!(runner_up, None, "la rue n'est plus en lice du tout");
            }
            other => panic!("le circuit doit l'emporter, pas {other:?}"),
        }
    }

    /// Rule (§1, calibrated): **a road alone matches nothing**, however close.
    ///
    /// This one goes against §4.2's wish, and the run is why: a street is
    /// within reach of every coordinate, so `ks_barcelona` came back as "carrer
    /// de Sant Lluís" and the fictional `ks_black_cat_county` as "Hill Street".
    /// Shutoko and the touge passes lose their automatic match here — they are
    /// what the manual correction of §7.6 is for — but nobody is told a wrong
    /// article is the right one.
    #[test]
    fn a_road_alone_matches_nothing() {
        let expressway = vec![(item("Q1369525", "Shuto Expressway", ids::URBAN_MOTORWAY), 40.0)];
        assert_eq!(
            decide_by_distance(expressway, &thresholds()),
            MatchOutcome::NoCandidate,
            "même à quarante mètres"
        );

        let street = vec![(item("Q16975733", "Hill Street", ids::STREET), 5.0)];
        assert_eq!(
            decide_by_distance(street, &thresholds()),
            MatchOutcome::NoCandidate,
            "surtout une rue de centre-ville"
        );
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
