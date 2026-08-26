//! `GUIDs.txt` — the only route from an event path to an event.
//!
//! Assetto Corsa ships no per-car string bank, so `FMOD_Studio_System_GetEvent`
//! (which takes a path) is useless for cars: the path has to be turned into a
//! GUID first, and this text file is the table that does it. See
//! `docs/SPEC-engine-sound-fmod.md` §2.3.

use std::path::{Path, PathBuf};

/// `FMOD_GUID`, laid out like a Windows GUID: three little-endian integers
/// followed by eight raw bytes.
///
/// `#[repr(C)]` because it is passed straight to `GetEventByID`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    /// Parses the `{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}` form, braces
    /// optional.
    ///
    /// `FMOD_Studio_ParseID` would do this too, and the first instinct was to
    /// call it rather than risk getting the byte order wrong by hand. Doing it
    /// here instead buys two things that matter: an event can be resolved
    /// **before** any DLL is loaded — which is what lets the fallback chain in
    /// `resolve_engine_event` run on a machine with no game installed — and the
    /// whole thing is unit-testable. The byte-order risk is covered by pinning
    /// the result against what `ParseID` actually returned during lot 0; see
    /// `guid_matches_what_fmod_parse_id_returned`.
    pub fn parse(text: &str) -> Option<Guid> {
        let text = text.trim();
        let text = text.strip_prefix('{').unwrap_or(text);
        let text = text.strip_suffix('}').unwrap_or(text);

        let mut groups = text.split('-');
        let (g1, g2, g3, g4, g5) = (
            groups.next()?,
            groups.next()?,
            groups.next()?,
            groups.next()?,
            groups.next()?,
        );
        if groups.next().is_some() {
            return None;
        }
        if (g1.len(), g2.len(), g3.len(), g4.len(), g5.len()) != (8, 4, 4, 4, 12) {
            return None;
        }

        let mut data4 = [0u8; 8];
        // The last two groups are *not* integers: they are the eight bytes of
        // `data4` written in order, which is why they are read pairwise here
        // while the first three go through `from_str_radix`.
        for (slot, pair) in data4.iter_mut().zip(hex_pairs(g4).chain(hex_pairs(g5))) {
            *slot = pair?;
        }

        Some(Guid {
            data1: u32::from_str_radix(g1, 16).ok()?,
            data2: u16::from_str_radix(g2, 16).ok()?,
            data3: u16::from_str_radix(g3, 16).ok()?,
            data4,
        })
    }
}

fn hex_pairs(s: &str) -> impl Iterator<Item = Option<u8>> + '_ {
    s.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
}

/// Which of the two engine events to reach for first.
///
/// Left open on purpose: `docs/SPEC-engine-sound-fmod.md` §7 has not settled
/// whether the exterior or the interior view should be the default, and this
/// module has no business deciding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EngineView {
    /// `engine_ext` — what a bystander hears, and the most comparable between mods.
    #[default]
    Exterior,
    /// `engine_int` — what the driver hears.
    Interior,
}

impl EngineView {
    fn suffix(self) -> &'static str {
        match self {
            EngineView::Exterior => "engine_ext",
            EngineView::Interior => "engine_int",
        }
    }

    fn other(self) -> EngineView {
        match self {
            EngineView::Exterior => EngineView::Interior,
            EngineView::Interior => EngineView::Exterior,
        }
    }
}

/// One `{guid} path` line. Lines that are not events (`bus:/…`, `bank:/…`, and
/// anything malformed) are skipped, so callers never see them.
fn entries(text: &str) -> impl Iterator<Item = (Guid, &str)> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix('{')?;
        let (guid, path) = rest.split_once('}')?;
        let path = path.trim();
        if !path.starts_with("event:/") {
            return None;
        }
        Some((Guid::parse(guid)?, path))
    })
}

/// Looks up one event path, **case-insensitively**, and returns it as written.
///
/// Not pedantry, and not a guess: four cars in the reference corpus declare
/// their events under a differently-cased car id than their own folder name —
/// `ford_mustang_boss_429_SE`, `ford_mustang_boss_SE`,
/// `traffic_aegis_daihatsu_Copen`, and `ks_ferrari_Sf15t`, which is **Kunos's
/// own content**, not a mod. Windows paths do not care about case, so the
/// authors had no reason to. An exact comparison loses all four silently, which
/// is exactly how it was found: as four unexplained gaps in the corpus survey,
/// not as a bug anyone reported.
///
/// The path is returned as the file spells it, never as we asked for it: it is
/// a diagnostic, and showing the caller its own guess back would be worthless.
pub fn lookup(text: &str, event_path: &str) -> Option<(String, Guid)> {
    let wanted = event_path.to_ascii_lowercase();
    entries(text)
        .find(|(_, path)| path.to_ascii_lowercase() == wanted)
        .map(|(guid, path)| (path.to_string(), guid))
}

/// Finds a car's engine event, degrading rather than failing.
///
/// The chain is the one in `docs/SPEC-engine-sound-fmod.md` §6: the requested
/// view, then the other one, then **any** event under this car whose name
/// mentions the engine. A mod that names its events unusually still gets
/// something to play; only a car with no engine event at all comes back empty.
pub fn engine_event(text: &str, car_id: &str, view: EngineView) -> Option<(String, Guid)> {
    let prefix = format!("event:/cars/{}/", car_id.to_ascii_lowercase());

    for candidate in [view, view.other()] {
        if let Some(found) = lookup(text, &format!("{prefix}{}", candidate.suffix())) {
            return Some(found);
        }
    }

    entries(text)
        .find(|(_, path)| {
            let lower = path.to_ascii_lowercase();
            lower.starts_with(&prefix) && lower[prefix.len()..].contains("engine")
        })
        .map(|(guid, path)| (path.to_string(), guid))
}

/// The master bus, into which every event eventually routes.
///
/// Read from the **global** table only, and that is not an oversight: a car's
/// own `GUIDs.txt` declares its `grp_*` buses but never `bus:/` itself, so
/// looking beside the bank would find nothing for any modded car.
pub fn master_bus(ac_root: &Path) -> Option<Guid> {
    let table = ac_root.join("content").join("sfx").join("GUIDs.txt");
    let text = std::fs::read_to_string(table).ok()?;
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix('{')?;
        let (guid, path) = rest.split_once('}')?;
        (path.trim() == "bus:/").then(|| Guid::parse(guid))?
    })
}

/// Finds one named event of a car (`limiter`, `horn`, `gear_ext`…) in whichever
/// table describes this bank.
///
/// Same search order as [`resolve_engine_event`], and the same reason for it:
/// a sound mod's events live in its own table, never in the game's.
pub fn resolve_event(bank_dir: &Path, ac_root: Option<&Path>, car_id: &str, event: &str) -> Option<Guid> {
    let wanted = format!("event:/cars/{car_id}/{event}");
    for file in guid_files(bank_dir, ac_root) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if let Some((_, guid)) = lookup(&text, &wanted) {
            return Some(guid);
        }
    }
    None
}

/// Where the table describing a given bank may live, most specific first.
///
/// The candidate that matters is **next to the bank**, not under the car:
/// auditioning a sound mod reads its bank straight out of the library, where
/// nothing of the game's layout applies. Measured on the reference install,
/// **122 of 299 cars ship their own `sfx/GUIDs.txt`** — those are the mods, and
/// a mod's events are simply *absent* from the global table. Kunos cars have no
/// file of their own and fall through to it.
pub fn guid_files(bank_dir: &Path, ac_root: Option<&Path>) -> Vec<PathBuf> {
    let mut files = vec![bank_dir.join("GUIDs.txt")];
    if let Some(root) = ac_root {
        files.push(root.join("content").join("sfx").join("GUIDs.txt"));
    }
    files
}

/// The ignition event, when the car has one — CSP builds do, Kunos cars do not.
///
/// `ign_ext` wins over `ign_int` when both exist, for the same reason
/// `engine_ext` is the default view: the audition is heard from outside the
/// car. No car in the reference library ships an `ign_ext` today — every one
/// found has only the interior event — so in practice this falls to `ign_int`,
/// and the preference is there for the day a bank does carry the exterior one.
///
/// `None` is the ordinary answer, not a failure: a car with no ignition event
/// simply starts already running (§6sexies).
pub fn resolve_ignition_event(bank_dir: &Path, ac_root: Option<&Path>, car_id: &str) -> Option<(String, Guid)> {
    for file in guid_files(bank_dir, ac_root) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for suffix in ["ign_ext", "ign_int"] {
            let path = format!("event:/cars/{car_id}/{suffix}");
            if let Some((path, guid)) = lookup(&text, &path) {
                return Some((path, guid));
            }
        }
    }
    None
}

/// Reads the candidate files in order and returns the first engine event found.
pub fn resolve_engine_event(
    bank_dir: &Path,
    ac_root: Option<&Path>,
    car_id: &str,
    view: EngineView,
) -> Option<(String, Guid)> {
    for file in guid_files(bank_dir, ac_root) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if let Some(found) = engine_event(&text, car_id, view) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exterior ignition event wins when a bank carries both — the
    /// audition is heard from outside the car.
    #[test]
    fn ignition_prefers_the_exterior_event() {
        let dir = crate::testutil::temp_dir("guids-ign");
        std::fs::write(
            dir.join("GUIDs.txt"),
            "{11111111-1111-1111-1111-111111111111} event:/cars/a_car/ign_int
             {22222222-2222-2222-2222-222222222222} event:/cars/a_car/ign_ext
",
        )
        .expect("write the table");
        let (path, _) = resolve_ignition_event(&dir, None, "a_car").expect("an ignition event");
        assert_eq!(path, "event:/cars/a_car/ign_ext", "the exterior one is preferred");
    }

    /// And the interior one is taken when it is the only one — which is every
    /// car measured so far.
    #[test]
    fn ignition_falls_back_to_the_interior_event() {
        let dir = crate::testutil::temp_dir("guids-ign-int");
        std::fs::write(
            dir.join("GUIDs.txt"),
            "{11111111-1111-1111-1111-111111111111} event:/cars/a_car/ign_int
",
        )
        .expect("write the table");
        let (path, _) = resolve_ignition_event(&dir, None, "a_car").expect("an ignition event");
        assert_eq!(path, "event:/cars/a_car/ign_int", "better than nothing");
    }

    /// A Kunos car has none, and that is not an error.
    #[test]
    fn a_car_without_an_ignition_event_says_so() {
        let dir = crate::testutil::temp_dir("guids-no-ign");
        std::fs::write(
            dir.join("GUIDs.txt"),
            "{11111111-1111-1111-1111-111111111111} event:/cars/a_car/engine_ext
",
        )
        .expect("write the table");
        assert!(
            resolve_ignition_event(&dir, None, "a_car").is_none(),
            "nothing to start"
        );
    }

    /// The byte order of the three leading integers is the one thing this
    /// parser could get wrong silently, so it is pinned to a measured oracle:
    /// the GT40's `engine_ext` GUID as `FMOD_Studio_ParseID` itself decoded it
    /// during lot 0 (`docs/SPEC-engine-sound-fmod.md` §2bis).
    #[test]
    fn guid_matches_what_fmod_parse_id_returned() {
        let parsed = Guid::parse("{d33f0a36-b38e-410f-b895-4797f5f77e18}").expect("well-formed guid parses");
        assert_eq!(parsed.data1, 0xd33f_0a36, "data1 is one little-endian u32");
        assert_eq!(parsed.data2, 0xb38e, "data2 is one little-endian u16");
        assert_eq!(parsed.data3, 0x410f, "data3 is one little-endian u16");
        assert_eq!(
            parsed.data4,
            [0xb8, 0x95, 0x47, 0x97, 0xf5, 0xf7, 0x7e, 0x18],
            "the last two groups are raw bytes, not integers"
        );
    }

    #[test]
    fn guid_accepts_braces_or_not() {
        let with = Guid::parse("{d33f0a36-b38e-410f-b895-4797f5f77e18}");
        let without = Guid::parse("d33f0a36-b38e-410f-b895-4797f5f77e18");
        assert_eq!(with, without, "braces are optional, not significant");
    }

    #[test]
    fn guid_rejects_malformed_text() {
        for bad in [
            "",
            "not-a-guid",
            "d33f0a36-b38e-410f-b895",                    // too few groups
            "d33f0a36-b38e-410f-b895-4797f5f77e18-extra", // too many
            "d33f0a3-b38e-410f-b895-4797f5f77e18",        // short first group
            "zzzzzzzz-b38e-410f-b895-4797f5f77e18",       // not hex
        ] {
            assert!(Guid::parse(bad).is_none(), "{bad:?} must not parse");
        }
    }

    const GT40: &str = "\
{3714df3b-bd5f-4b32-b037-e82ee43dd78c} bank:/ks_ford_gt40
{aaa1be0b-1e4d-4b25-96cc-58be71fd3c5c} event:/cars/ks_ford_gt40/backfire_ext
{d33f0a36-b38e-410f-b895-4797f5f77e18} event:/cars/ks_ford_gt40/engine_ext
{6855af70-8f4e-4851-a5b0-237bc434d2c1} event:/cars/ks_ford_gt40/engine_int
{86461679-185b-4a58-809e-1ef0281f4836} event:/cars/ks_ford_gt40/limiter
{5206e42b-ae8f-49f6-b059-d613b2947b49} bus:/grp_engine_ext
";

    /// `bus:/` and `bank:/` lines share the file with the events, and a lookup
    /// that matched on the GUID alone would happily return one.
    #[test]
    fn only_event_lines_are_considered() {
        let paths: Vec<_> = entries(GT40).map(|(_, p)| p).collect();
        assert!(
            paths.iter().all(|p| p.starts_with("event:/")),
            "got a non-event line: {paths:?}"
        );
        assert_eq!(paths.len(), 4, "the four events, neither bus nor bank");
    }

    #[test]
    fn engine_event_honours_the_requested_view() {
        let (path, guid) = engine_event(GT40, "ks_ford_gt40", EngineView::Exterior).expect("exterior found");
        assert_eq!(path, "event:/cars/ks_ford_gt40/engine_ext");
        assert_eq!(guid, Guid::parse("{d33f0a36-b38e-410f-b895-4797f5f77e18}").unwrap());

        let (path, _) = engine_event(GT40, "ks_ford_gt40", EngineView::Interior).expect("interior found");
        assert_eq!(path, "event:/cars/ks_ford_gt40/engine_int");
    }

    /// Regression, from four real cars found by the corpus survey rather than
    /// by anyone reporting them: the folder is `ks_ferrari_sf15t`, the events
    /// say `ks_ferrari_Sf15t`, and Kunos shipped it that way.
    #[test]
    fn a_car_id_cased_differently_from_its_folder_still_resolves() {
        let real = "{e3496e07-0e50-4c55-9347-44fcf272476d} bank:/ks_ferrari_Sf15T
{6d1f8671-b868-4b50-8a86-b7a3e4ecf447} event:/cars/ks_ferrari_Sf15t/engine_ext
{245bc852-5d43-4d05-a7d7-fb13018d5919} event:/cars/ks_ferrari_Sf15t/engine_int
";
        let (path, guid) = engine_event(real, "ks_ferrari_sf15t", EngineView::Exterior)
            .expect("the folder name must find the differently-cased event");
        assert_eq!(
            path, "event:/cars/ks_ferrari_Sf15t/engine_ext",
            "the path comes back as the file spells it, not as we asked"
        );
        assert_eq!(guid, Guid::parse("{6d1f8671-b868-4b50-8a86-b7a3e4ecf447}").unwrap());
    }

    /// Case-insensitivity must not become "any car will do".
    #[test]
    fn case_insensitivity_does_not_blur_two_different_cars() {
        assert!(
            engine_event(GT40, "ks_ford_gt40_s3", EngineView::Exterior).is_none(),
            "a longer id is a different car, not a casing variant"
        );
    }

    /// §6: a mod with no `engine_ext` must still play something.
    #[test]
    fn engine_event_falls_back_to_the_other_view_then_to_any_engine() {
        let int_only = "{6855af70-8f4e-4851-a5b0-237bc434d2c1} event:/cars/modcar/engine_int\n";
        let (path, _) = engine_event(int_only, "modcar", EngineView::Exterior).expect("falls back to interior");
        assert_eq!(
            path, "event:/cars/modcar/engine_int",
            "requested view missing, the other one serves"
        );

        let odd = "{6855af70-8f4e-4851-a5b0-237bc434d2c1} event:/cars/modcar/Engine_V8_Loop\n";
        let (path, _) = engine_event(odd, "modcar", EngineView::Exterior).expect("falls back to any engine event");
        assert_eq!(
            path, "event:/cars/modcar/Engine_V8_Loop",
            "match is case-insensitive on the name"
        );
    }

    #[test]
    fn engine_event_is_empty_when_the_car_has_none() {
        let no_engine = "{aaa1be0b-1e4d-4b25-96cc-58be71fd3c5c} event:/cars/modcar/horn\n";
        assert!(
            engine_event(no_engine, "modcar", EngineView::Exterior).is_none(),
            "a horn is not an engine"
        );
    }

    /// Another car's engine event must never be mistaken for this one's — the
    /// global table lists every Kunos car at once.
    #[test]
    fn engine_event_never_crosses_to_another_car() {
        assert!(
            engine_event(GT40, "ks_mazda_mx5", EngineView::Exterior).is_none(),
            "the GT40's events belong to the GT40"
        );
    }

    /// The table sitting next to the bank wins. A sound mod is auditioned from
    /// the library, and its events are absent from the game's global table —
    /// preferring the global one would resolve to the *stock* bank's event, or
    /// to nothing at all.
    #[test]
    fn the_table_next_to_the_bank_wins_over_the_global_one() {
        let base = crate::testutil::temp_dir("fmod-guids");
        let bank_dir = base.join("library").join("some_sound_mod");
        let ac = base.join("ac");
        std::fs::create_dir_all(&bank_dir).expect("create bank dir");
        std::fs::create_dir_all(ac.join("content").join("sfx")).expect("create global sfx dir");

        std::fs::write(
            ac.join("content").join("sfx").join("GUIDs.txt"),
            "{00000000-0000-0000-0000-000000000000} event:/cars/modcar/engine_ext
",
        )
        .expect("write global table");
        std::fs::write(
            bank_dir.join("GUIDs.txt"),
            "{d33f0a36-b38e-410f-b895-4797f5f77e18} event:/cars/modcar/engine_ext
",
        )
        .expect("write the mod's own table");

        let (_, guid) = resolve_engine_event(&bank_dir, Some(&ac), "modcar", EngineView::Exterior).expect("resolved");
        assert_eq!(
            guid,
            Guid::parse("{d33f0a36-b38e-410f-b895-4797f5f77e18}").unwrap(),
            "the table beside the bank must take precedence"
        );
    }

    /// Kunos cars ship no table of their own, and must fall through.
    #[test]
    fn resolve_falls_through_to_the_global_table() {
        let base = crate::testutil::temp_dir("fmod-guids-global");
        let bank_dir = base
            .join("ac")
            .join("content")
            .join("cars")
            .join("ks_ford_gt40")
            .join("sfx");
        let ac = base.join("ac");
        std::fs::create_dir_all(&bank_dir).expect("create car sfx dir");
        std::fs::create_dir_all(ac.join("content").join("sfx")).expect("create global sfx dir");
        std::fs::write(ac.join("content").join("sfx").join("GUIDs.txt"), GT40).expect("write global table");

        let (path, _) =
            resolve_engine_event(&bank_dir, Some(&ac), "ks_ford_gt40", EngineView::Exterior).expect("resolved");
        assert_eq!(path, "event:/cars/ks_ford_gt40/engine_ext");
    }

    #[test]
    fn resolve_is_empty_when_no_table_exists() {
        let base = crate::testutil::temp_dir("fmod-guids-missing");
        assert!(
            resolve_engine_event(&base, None, "modcar", EngineView::Exterior).is_none(),
            "a missing GUIDs.txt is not an error, just nothing to play"
        );
    }
}
