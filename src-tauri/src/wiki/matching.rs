//! What the two matching strategies have in common — and it is deliberately
//! little (WIKI§4).
//!
//! Cars are matched by name, tracks by coordinates; forcing one abstraction
//! over the two would mean a "search" trait whose implementations share no
//! code. What they do share is the **shape of the verdict**: a single candidate
//! that stood out, or nothing. That, and the name similarity used by the car
//! score and by the track's name fallback, is all that lives here.

use serde::Serialize;

/// One candidate, with whatever score its strategy gave it.
///
/// The score is in `[0, 1]` in both strategies, but it does not mean the same
/// thing: for a car it is the weighted name/brand/year score of WIKI§4.1, for a
/// track it is proximity. Comparable within a strategy, never across.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub entity_id: String,
    pub label: Option<String>,
    /// Wikidata's short description — "automobile produced by Toyota". What
    /// WIKI§7.6 will show to disambiguate by eye, and what makes the calibration
    /// report readable.
    pub description: Option<String>,
    pub score: f64,
}

impl Candidate {
    /// Label if there is one, id otherwise — for reports and logs.
    ///
    /// Only the calibration report calls it, and that report is reachable only
    /// from `#[ignore]` tests: dead in a lib build, alive where it matters.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.entity_id)
    }
}

/// What an attempt concluded. Three of the four are non-results (WIKI§1).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum MatchOutcome {
    /// One candidate stood out, by enough.
    Matched {
        candidate: Candidate,
        /// The second best, when there was one — kept so the calibration
        /// report can show what was nearly chosen instead.
        runner_up: Option<Candidate>,
        /// Gap to the runner-up. `None` when there was only one candidate.
        margin: Option<f64>,
    },
    /// Candidates were found and none stood out. **The heart of WIKI§1**: an
    /// ambiguity produces a non-result, not a draw.
    Ambiguous {
        best: Candidate,
        runner_up: Candidate,
        margin: f64,
    },
    /// Nothing of an accepted type came back — or nothing came back at all.
    NoCandidate,
    /// The network could not answer. Not a verdict: nothing was learned, and
    /// in particular not that this mod has no article (WIKI§3.3).
    Unavailable,
}

/// Thresholds, never literals in the code.
///
/// They come from `Prefs` (`config.rs`) precisely because they are what the
/// calibration command exists to tune: the values shipped are a starting
/// point, and WIKI§13 says so.
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    /// Below this, the best candidate is not good enough to be anybody's
    /// article.
    pub min_score: f64,
    /// Gap the best must have over the second (WIKI§4.1).
    pub min_margin: f64,
    /// Radius of the geographic search, metres (WIKI§4.2).
    pub track_radius_m: u32,
    /// Two accepted track candidates closer together than this are a tie, and
    /// a tie is an ambiguity.
    pub track_tie_margin_m: f64,
}

/// Words of a name, lowercased, punctuation dropped — **and letters split from
/// digits**.
///
/// The second half is what makes `RX-7`, `RX 7` and `RX7` the same name: all
/// three give `["rx", "7"]`. Modders write the same car all three ways, and
/// splitting only on punctuation would leave `rx7` sharing nothing with `rx`
/// and `7` — a car scoring 0.375 against its own name.
fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = false;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        let is_digit = c.is_ascii_digit();
        if !current.is_empty() && is_digit != current_is_digit {
            out.push(std::mem::take(&mut current));
        }
        current_is_digit = is_digit;
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// How much two names look like the same thing, in `[0, 1]`.
///
/// Half Jaccard, half containment. The containment half is what makes the
/// **generic model an acceptable answer** (WIKI§4.1): `Toyota Corolla` is entirely
/// contained in `Toyota Corolla Levin`, so it scores high even though it is
/// shorter — which is exactly the outcome the spec asks for, since WIKI§5.3 will
/// fall back to the generic article anyway.
pub fn name_similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (tokens(a), tokens(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.iter().filter(|t| b.contains(t)).count() as f64;
    let union = (a.len() + b.len()) as f64 - shared;
    let smallest = a.len().min(b.len()) as f64;
    let jaccard = shared / union;
    let containment = shared / smallest;
    0.5 * jaccard + 0.5 * containment
}

/// The verdict of WIKI§4.1, applied to candidates **already sorted** best first.
///
/// Two ways to end with nothing, and they are not the same: too weak (nobody
/// is plausible) and too close (several are). The second is the one the spec
/// insists on — "l'ambiguïté produit un non-résultat, pas un tirage au sort".
impl Thresholds {
    /// Reads the four settings. One place, so the calibration and the fiche
    /// can never drift apart on what "the threshold" means.
    pub fn from_prefs(prefs: &crate::config::Prefs) -> Self {
        Self {
            min_score: prefs.wiki_match_min_score,
            min_margin: prefs.wiki_match_min_margin,
            track_radius_m: prefs.wiki_track_radius_m,
            track_tie_margin_m: prefs.wiki_track_tie_margin_m,
        }
    }
}

pub fn decide(mut candidates: Vec<Candidate>, thresholds: &Thresholds) -> MatchOutcome {
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut it = candidates.into_iter();
    let Some(best) = it.next() else {
        return MatchOutcome::NoCandidate;
    };
    if best.score < thresholds.min_score {
        return MatchOutcome::NoCandidate;
    }
    let Some(runner_up) = it.next() else {
        return MatchOutcome::Matched {
            candidate: best,
            runner_up: None,
            margin: None,
        };
    };
    let margin = best.score - runner_up.score;
    if margin < thresholds.min_margin {
        return MatchOutcome::Ambiguous {
            best,
            runner_up,
            margin,
        };
    }
    MatchOutcome::Matched {
        candidate: best,
        runner_up: Some(runner_up),
        margin: Some(margin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn thresholds() -> Thresholds {
        Thresholds {
            min_score: 0.55,
            min_margin: 0.12,
            track_radius_m: 5_000,
            track_tie_margin_m: 150.0,
        }
    }

    fn candidate(id: &str, score: f64) -> Candidate {
        Candidate {
            entity_id: id.into(),
            label: Some(id.into()),
            description: None,
            score,
        }
    }

    /// Rule (WIKI§4.1): the generic model is a valid answer for a generation mod.
    /// `Toyota Corolla` must score well against `Toyota Corolla Levin`, since
    /// WIKI§5.3 falls back to the generic article anyway.
    #[test]
    fn the_generic_model_stays_close_to_its_generation() {
        let generic = name_similarity("Toyota Corolla", "Toyota Corolla Levin");
        let unrelated = name_similarity("Toyota Corolla", "Nissan Skyline");
        assert!(generic > 0.8, "le modèle générique reste proche ({generic})");
        assert_eq!(unrelated, 0.0, "rien en commun");
    }

    /// Rule (WIKI§4.3): punctuation is not meaning. The three spellings modders use
    /// for the same car must land on each other — `RX-7`, `RX 7` and `RX7`.
    #[test]
    fn punctuation_does_not_separate_a_name_from_itself() {
        assert_eq!(name_similarity("Mazda RX-7", "Mazda RX 7"), 1.0);
        assert_eq!(
            name_similarity("Mazda RX-7", "mazda rx7"),
            1.0,
            "lettres et chiffres séparés"
        );
        assert_eq!(name_similarity("Toyota AE86", "toyota ae-86"), 1.0);
    }

    /// Rule (WIKI§1, WIKI§4.1): **two close candidates produce nothing.** This is the
    /// test the whole feature turns on — a wrong article costs far more than a
    /// missing one, so a near-tie is never resolved by picking the first.
    #[test]
    fn two_close_candidates_produce_nothing() {
        let outcome = decide(
            vec![candidate("Q1", 0.82), candidate("Q2", 0.78), candidate("Q3", 0.40)],
            &thresholds(),
        );
        match outcome {
            MatchOutcome::Ambiguous {
                best,
                runner_up,
                margin,
            } => {
                assert_eq!(best.entity_id, "Q1");
                assert_eq!(runner_up.entity_id, "Q2");
                assert!(margin < 0.12, "l'écart est sous le seuil ({margin})");
            }
            other => panic!("une quasi-égalité doit être ambiguë, pas {other:?}"),
        }
    }

    /// Rule (WIKI§4.1): the same pair, once the best is clear enough, is a match
    /// — and it carries its margin so the calibration report can show it.
    #[test]
    fn a_clear_winner_is_matched_with_its_margin() {
        let outcome = decide(vec![candidate("Q1", 0.90), candidate("Q2", 0.50)], &thresholds());
        match outcome {
            MatchOutcome::Matched {
                candidate,
                runner_up,
                margin,
            } => {
                assert_eq!(candidate.entity_id, "Q1");
                assert_eq!(runner_up.expect("second").entity_id, "Q2");
                assert!((margin.expect("marge") - 0.40).abs() < 1e-9);
            }
            other => panic!("un vainqueur net doit être retenu, pas {other:?}"),
        }
    }

    /// Rule (WIKI§1): a candidate nobody would recognise is no candidate. Without
    /// the floor, a lone bad hit would win by walkover for lack of a second.
    #[test]
    fn a_lone_weak_candidate_wins_nothing() {
        let outcome = decide(vec![candidate("Q1", 0.20)], &thresholds());
        assert_eq!(outcome, MatchOutcome::NoCandidate);
        assert_eq!(decide(vec![], &thresholds()), MatchOutcome::NoCandidate, "ni le vide");
    }

    /// Rule: a single candidate above the floor is a match with no margin —
    /// there is nothing to be ambiguous with.
    #[test]
    fn a_lone_strong_candidate_has_no_margin() {
        let outcome = decide(vec![candidate("Q1", 0.90)], &thresholds());
        match outcome {
            MatchOutcome::Matched { margin, runner_up, .. } => {
                assert_eq!(margin, None);
                assert_eq!(runner_up, None);
            }
            other => panic!("un seul candidat solide est retenu, pas {other:?}"),
        }
    }
}
