//! Recognising an event's parameters without hardcoding anyone's names.
//!
//! Parameter names belong to whoever authored the mod: `rpms`, `rpm`,
//! `throttle`, `load`… The event is asked what it has, and the answer is sorted
//! out here. See `docs/SPEC-engine-sound-fmod.md` §2.4 and §2bis.

/// `FMOD_STUDIO_PARAMETER_TYPE::GAME_CONTROLLED`.
///
/// Every other value is a parameter FMOD computes itself from the event's 3D
/// attributes (`Distance`, `Event Cone Angle`, …). Writing to those is
/// meaningless, so they are not candidates for anything.
pub const KIND_GAME_CONTROLLED: i32 = 0;

/// A rev parameter has to reach at least this. `Distance` (0–500) and
/// `Event Cone Angle` (0–180) sit below it, which is a useful second line of
/// defence behind the type filter.
const REV_CEILING: f32 = 1000.0;

/// A throttle parameter is a ratio, not a count. The GT40's is 0–1; the bound
/// is loose enough for an author who scaled it to a percentage.
const THROTTLE_CEILING: f32 = 100.0;

/// One entry of `FMOD_STUDIO_PARAMETER_DESCRIPTION`, owned and free of the FFI.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub name: String,
    pub index: i32,
    pub min: f32,
    pub max: f32,
    pub kind: i32,
}

impl ParamInfo {
    /// Whether this parameter is ours to set at all.
    pub fn is_drivable(&self) -> bool {
        self.kind == KIND_GAME_CONTROLLED
    }
}

/// The parameters that mean something to us, out of everything the event has.
///
/// Both fields are optional by design: an event whose parameters are entirely
/// unrecognised still plays, at its default values (§2.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roles {
    /// Engine speed. Drives pitch, and is what the rev slider of §4.4 moves.
    pub rev: Option<ParamInfo>,
    /// Throttle. Blends the on-load layers against the off-throttle ones —
    /// measured at lot 0 as a 4× drop in level when it goes from 1.0 to 0.0.
    pub throttle: Option<ParamInfo>,
}

fn name_mentions(param: &ParamInfo, needles: &[&str]) -> bool {
    let name = param.name.to_ascii_lowercase();
    needles.iter().any(|needle| name.contains(needle))
}

/// Picks the rev and throttle parameters out of an event's list.
///
/// The type filter runs **first**, and that ordering is the point rather than a
/// detail: without it the "widest range wins" fallback below would eventually
/// settle on an automatic parameter — `Distance` is a plausible-looking 0–500
/// float that FMOD overwrites on every update, so driving it would look correct
/// and do nothing.
pub fn classify(params: &[ParamInfo]) -> Roles {
    let drivable: Vec<&ParamInfo> = params.iter().filter(|p| p.is_drivable()).collect();

    let rev = drivable
        .iter()
        .find(|p| name_mentions(p, &["rpm", "rev"]) && p.max >= REV_CEILING)
        // No recognisable name: the one parameter with a range wide enough to
        // hold an engine speed is the only sensible guess left.
        .or_else(|| {
            drivable
                .iter()
                .filter(|p| p.max >= REV_CEILING)
                .max_by(|a, b| a.max.total_cmp(&b.max))
        })
        .map(|p| (*p).clone());

    let throttle = drivable
        .iter()
        .find(|p| {
            name_mentions(p, &["throttle", "gas", "load", "accel"])
                && p.max <= THROTTLE_CEILING
                // A parameter cannot be both; the rev one wins, being the one
                // we can identify with more confidence.
                && rev.as_ref().is_none_or(|rev| rev.index != p.index)
        })
        .map(|p| (*p).clone());

    Roles { rev, throttle }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(name: &str, index: i32, min: f32, max: f32, kind: i32) -> ParamInfo {
        ParamInfo {
            name: name.to_string(),
            index,
            min,
            max,
            kind,
        }
    }

    /// The GT40's four parameters, exactly as `GetParameterByIndex` reported
    /// them at lot 0 (`docs/SPEC-engine-sound-fmod.md` §2bis).
    fn gt40() -> Vec<ParamInfo> {
        vec![
            param("throttle", 0, 0.0, 1.0, 0),
            param("rpms", 1, 0.0, 20000.0, 0),
            param("Event Cone Angle", 2, 0.0, 180.0, 2),
            param("Distance", 3, 0.0, 500.0, 1),
        ]
    }

    #[test]
    fn gt40_parameters_are_recognised() {
        let roles = classify(&gt40());
        assert_eq!(
            roles.rev.as_ref().map(|p| p.name.as_str()),
            Some("rpms"),
            "rev found by name and range"
        );
        assert_eq!(
            roles.throttle.as_ref().map(|p| p.name.as_str()),
            Some("throttle"),
            "throttle found by name"
        );
    }

    /// The type filter has to come first. An automatic parameter named like a
    /// rev one must still be refused: FMOD overwrites it on every update, so
    /// driving it would fail silently.
    #[test]
    fn automatic_parameters_are_never_picked() {
        let params = vec![
            param("rpm", 0, 0.0, 20000.0, 1), // AUTOMATIC_DISTANCE, misleadingly named
            param("Distance", 1, 0.0, 500.0, 1),
        ];
        let roles = classify(&params);
        assert!(
            roles.rev.is_none(),
            "an automatic parameter is not ours to set, whatever it is called"
        );
    }

    /// Names vary by author; a range wide enough for an engine speed is the
    /// last usable signal.
    #[test]
    fn an_unnamed_rev_parameter_is_found_by_its_range() {
        let params = vec![param("p1", 0, 0.0, 1.0, 0), param("p2", 1, 0.0, 9000.0, 0)];
        let roles = classify(&params);
        assert_eq!(
            roles.rev.as_ref().map(|p| p.index),
            Some(1),
            "only p2 can hold an engine speed"
        );
        assert!(
            roles.throttle.is_none(),
            "p1 is not named like a throttle, so it stays unclaimed"
        );
    }

    /// A car whose event exposes nothing recognisable still plays — it just
    /// plays at the defaults (§2.4).
    #[test]
    fn nothing_recognisable_yields_no_roles() {
        let params = vec![param("Distance", 0, 0.0, 500.0, 1), param("mystery", 1, 0.0, 3.0, 0)];
        let roles = classify(&params);
        assert_eq!(roles, Roles::default(), "no rev, no throttle, and no panic");
    }

    #[test]
    fn one_parameter_is_never_given_both_roles() {
        // Contrived but cheap to guard: a single wide-ranged parameter named
        // like a throttle must not end up as rev *and* throttle.
        let params = vec![param("engine_load_rpm", 0, 0.0, 8000.0, 0)];
        let roles = classify(&params);
        assert_eq!(
            roles.rev.as_ref().map(|p| p.index),
            Some(0),
            "the wide range makes it the rev parameter"
        );
        assert!(roles.throttle.is_none(), "and it cannot also be the throttle");
    }

    #[test]
    fn throttle_must_be_a_ratio_not_a_count() {
        let params = vec![param("rpms", 0, 0.0, 20000.0, 0), param("load_hint", 1, 0.0, 5000.0, 0)];
        let roles = classify(&params);
        assert!(
            roles.throttle.is_none(),
            "a 0–5000 range is not a throttle however it is named"
        );
    }
}
