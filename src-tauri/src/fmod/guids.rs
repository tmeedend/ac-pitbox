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

/// Looks up one exact event path.
pub fn lookup(text: &str, event_path: &str) -> Option<Guid> {
    entries(text)
        .find(|(_, path)| *path == event_path)
        .map(|(guid, _)| guid)
}

/// Finds a car's engine event, degrading rather than failing.
///
/// The chain is the one in `docs/SPEC-engine-sound-fmod.md` §6: the requested
/// view, then the other one, then **any** event under this car whose name
/// mentions the engine. A mod that names its events unusually still gets
/// something to play; only a car with no engine event at all comes back empty.
pub fn engine_event(text: &str, car_id: &str, view: EngineView) -> Option<(String, Guid)> {
    let prefix = format!("event:/cars/{car_id}/");

    for candidate in [view, view.other()] {
        let path = format!("{prefix}{}", candidate.suffix());
        if let Some(guid) = lookup(text, &path) {
            return Some((path, guid));
        }
    }

    entries(text)
        .find(|(_, path)| path.starts_with(&prefix) && path[prefix.len()..].to_ascii_lowercase().contains("engine"))
        .map(|(guid, path)| (path.to_string(), guid))
}

/// Where a car's `GUIDs.txt` may live, most specific first.
///
/// Measured on the reference install: **122 of 299 cars ship their own**
/// `sfx/GUIDs.txt` — those are the mods, and theirs is the one that describes
/// the bank sitting next to it. The Kunos cars have no such file and are all
/// listed in the single global table instead. Trying the car's own file first
/// is therefore not a nicety: a sound mod's events are simply absent from the
/// global one.
pub fn guid_files(ac_root: &Path, car_id: &str) -> Vec<PathBuf> {
    vec![
        ac_root
            .join("content")
            .join("cars")
            .join(car_id)
            .join("sfx")
            .join("GUIDs.txt"),
        ac_root.join("content").join("sfx").join("GUIDs.txt"),
    ]
}

/// Reads the candidate files in order and returns the first engine event found.
pub fn resolve_engine_event(ac_root: &Path, car_id: &str, view: EngineView) -> Option<(String, Guid)> {
    for file in guid_files(ac_root, car_id) {
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

    /// The mod's own file wins: a sound mod's events are absent from the global
    /// table, so preferring the global one would silently play the wrong bank's
    /// event — or nothing at all.
    #[test]
    fn a_cars_own_guid_file_wins_over_the_global_one() {
        let base = crate::testutil::temp_dir("fmod-guids");
        let car = base.join("content").join("cars").join("modcar").join("sfx");
        let global = base.join("content").join("sfx");
        std::fs::create_dir_all(&car).expect("create car sfx dir");
        std::fs::create_dir_all(&global).expect("create global sfx dir");

        std::fs::write(
            global.join("GUIDs.txt"),
            "{00000000-0000-0000-0000-000000000000} event:/cars/modcar/engine_ext\n",
        )
        .expect("write global table");
        std::fs::write(
            car.join("GUIDs.txt"),
            "{d33f0a36-b38e-410f-b895-4797f5f77e18} event:/cars/modcar/engine_ext\n",
        )
        .expect("write the mod's own table");

        let (_, guid) = resolve_engine_event(&base, "modcar", EngineView::Exterior).expect("resolved");
        assert_eq!(
            guid,
            Guid::parse("{d33f0a36-b38e-410f-b895-4797f5f77e18}").unwrap(),
            "the car's own GUIDs.txt must take precedence"
        );
    }

    /// Kunos cars have no file of their own, and must fall through.
    #[test]
    fn resolve_falls_through_to_the_global_table() {
        let base = crate::testutil::temp_dir("fmod-guids-global");
        let global = base.join("content").join("sfx");
        std::fs::create_dir_all(&global).expect("create global sfx dir");
        std::fs::write(global.join("GUIDs.txt"), GT40).expect("write global table");

        let (path, _) = resolve_engine_event(&base, "ks_ford_gt40", EngineView::Exterior).expect("resolved");
        assert_eq!(path, "event:/cars/ks_ford_gt40/engine_ext");
    }

    #[test]
    fn resolve_is_empty_when_no_table_exists() {
        let base = crate::testutil::temp_dir("fmod-guids-missing");
        assert!(
            resolve_engine_event(&base, "modcar", EngineView::Exterior).is_none(),
            "a missing GUIDs.txt is not an error, just nothing to play"
        );
    }
}
