//! Scenes (SPEC-story-mode, Scenes, frame and sound): the place behind the
//! frame and the dialogues - a bedroom, a garage - declared in
//! `scenes/scenes.yaml`, each with its variants, of which the state picks one.
//!
//! **The variant is the first whose criteria all hold**, in the order the
//! file lists them; a criterion left out holds always. When none holds, the
//! last one shows: it is the place's default, which is why an author writes
//! the general case last. A Beat can force a variant until a Beat sets a place
//! again.
//!
//! Season and time of day come from story time; weather and mood are the
//! story's own, set by Beats and kept until a Beat changes them - the story
//! has no weather of its own otherwise, and an event's weather is the
//! session's, not the scene's.

use serde::{Deserialize, Serialize};

use crate::load::Story;
use crate::time::{Clock, StoryDate};

/// Where a story's scenes are declared.
pub const SCENES: &str = "scenes/scenes.yaml";

pub type SceneId = String;

/// `scenes/scenes.yaml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenes {
    pub scenes: Vec<Place>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    pub id: SceneId,
    /// Display name in the source language; opaque to the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Blurred behind the frame, the default; a variant can say otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<bool>,
    /// A sound looped while the place shows, a path inside `media/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<String>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// What a Beat forcing it names.
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub season: Vec<Season>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time: Vec<Period>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weather: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mood: Vec<String>,
    /// A still image or a seamless video loop, a path inside `media/`.
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<bool>,
    /// Its own sound, instead of the place's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<String>,
}

impl Variant {
    /// Holds whatever the state: the variants after it never show.
    pub fn always(&self) -> bool {
        self.season.is_empty() && self.time.is_empty() && self.weather.is_empty() && self.mood.is_empty()
    }

    fn holds(&self, m: &Moment) -> bool {
        let fits = |wanted: &[String], current: Option<&str>| {
            wanted.is_empty() || current.is_some_and(|c| wanted.iter().any(|w| w == c))
        };
        (self.season.is_empty() || self.season.contains(&m.season))
            && (self.time.is_empty() || self.time.contains(&m.period))
            && fits(&self.weather, m.weather)
            && fits(&self.mood, m.mood)
    }
}

/// The seasons of the northern hemisphere, by month: December to February is
/// winter. A story set in the south is a setting to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

impl Season {
    pub fn of(date: StoryDate) -> Season {
        match date.month {
            3..=5 => Season::Spring,
            6..=8 => Season::Summer,
            9..=11 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// The times of day: morning from 05:00, afternoon from 12:00, evening from
/// 18:00, night from 22:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Morning,
    Afternoon,
    Evening,
    Night,
}

impl Period {
    pub fn of(clock: Clock) -> Period {
        match clock.hour {
            5..=11 => Period::Morning,
            12..=17 => Period::Afternoon,
            18..=21 => Period::Evening,
            _ => Period::Night,
        }
    }
}

/// What picks a variant, in story time.
pub struct Now<'a> {
    pub date: StoryDate,
    pub clock: Clock,
    pub weather: Option<&'a str>,
    pub mood: Option<&'a str>,
    /// A variant a Beat forced.
    pub forced: Option<&'a str>,
}

impl<'a> Now<'a> {
    pub fn moment(&self) -> Moment<'a> {
        Moment {
            season: Season::of(self.date),
            period: Period::of(self.clock),
            weather: self.weather,
            mood: self.mood,
            forced: self.forced,
        }
    }
}

/// What picks a variant, as its criteria name it: the editor's preview sets
/// these directly, without a date or a clock.
pub struct Moment<'a> {
    pub season: Season,
    pub period: Period,
    pub weather: Option<&'a str>,
    pub mood: Option<&'a str>,
    /// A variant a Beat forced.
    pub forced: Option<&'a str>,
}

/// A place as it shows now: what the scene layer needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shown {
    pub place: SceneId,
    pub variant: String,
    /// A path inside `media/`.
    pub file: String,
    /// A video loop rather than a still image.
    pub video: bool,
    pub blur: bool,
    pub ambient: Option<String>,
}

/// Whether a file of a scene is a video loop, by its extension.
pub fn is_video(path: &str) -> bool {
    extension(path).is_some_and(|e| VIDEO.contains(&e.as_str()))
}

/// The files a variant can show.
pub const IMAGE: [&str; 4] = ["png", "webp", "jpg", "jpeg"];
pub const VIDEO: [&str; 2] = ["webm", "mp4"];
/// The files an ambient sound can be.
pub const AUDIO: [&str; 3] = ["ogg", "mp3", "wav"];

pub fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())
}

/// Place `id` as it shows `now`; `None` for a place the story does not
/// declare, or one with no variant - the checks report both.
pub fn shown(story: &Story, id: &str, now: &Now) -> Option<Shown> {
    pick(story.scene(id)?, &now.moment())
}

/// `place` as it shows at moment `m`; `None` when it has no variant.
pub fn pick(place: &Place, m: &Moment) -> Option<Shown> {
    let forced = m.forced.and_then(|f| place.variants.iter().find(|v| v.id == f));
    let variant = forced
        .or_else(|| place.variants.iter().find(|v| v.holds(m)))
        .or(place.variants.last())?;
    Some(Shown {
        place: place.id.clone(),
        variant: variant.id.clone(),
        file: variant.file.clone(),
        video: is_video(&variant.file),
        blur: variant.blur.or(place.blur).unwrap_or(true),
        ambient: variant.ambient.clone().or_else(|| place.ambient.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::SourceFile;

    fn story(scenes: &str) -> Story {
        let mut files = crate::sample::files();
        files.retain(|f| f.path != SCENES);
        files.push(SourceFile::new(SCENES, scenes));
        Story::from_sources(files).expect("loads")
    }

    fn now(date: &str, clock: &str) -> Now<'static> {
        Now {
            date: date.parse().unwrap(),
            clock: clock.parse().unwrap(),
            weather: None,
            mood: None,
            forced: None,
        }
    }

    const GARAGE: &str = "scenes:
  - id: garage
    ambient: hum.ogg
    variants:
      - { id: rain, weather: [rain], file: rain.webp, blur: false }
      - { id: winter_night, season: [winter], time: [night, evening], file: wn.webm, ambient: wind.ogg }
      - { id: day, file: day.webp }
";

    // The first variant whose criteria all hold shows, a criterion left out
    // holding always; with none, the last one, the place's default
    // (SPEC-story-mode, Scenes, frame and sound).
    #[test]
    fn the_first_variant_that_holds_shows_else_the_last() {
        let s = story(GARAGE);
        let pick = |n: &Now| shown(&s, "garage", n).map(|v| v.variant);
        assert_eq!(pick(&now("2026-01-10", "23:00")).as_deref(), Some("winter_night"));
        assert_eq!(
            pick(&now("2026-01-10", "19:30")).as_deref(),
            Some("winter_night"),
            "evening counts"
        );
        assert_eq!(
            pick(&now("2026-07-10", "23:00")).as_deref(),
            Some("day"),
            "not winter: the default"
        );
        assert_eq!(
            pick(&now("2026-01-10", "10:00")).as_deref(),
            Some("day"),
            "a winter morning"
        );
        let rain = Now {
            weather: Some("rain"),
            ..now("2026-01-10", "23:00")
        };
        assert_eq!(pick(&rain).as_deref(), Some("rain"), "written first, it wins");
        assert_eq!(shown(&s, "nowhere", &rain), None, "an undeclared place shows nothing");
    }

    // A Beat's forced variant wins over the state; one the place does not
    // have is ignored (the checks report it).
    #[test]
    fn a_forced_variant_wins_over_the_state() {
        let s = story(GARAGE);
        let forced = |f| Now {
            forced: Some(f),
            ..now("2026-07-10", "12:00")
        };
        assert_eq!(shown(&s, "garage", &forced("rain")).unwrap().variant, "rain");
        assert_eq!(shown(&s, "garage", &forced("nope")).unwrap().variant, "day");
    }

    // What the scene layer needs comes with the variant: a video told from
    // a still by its extension, the blur and the sound the variant's, else
    // the place's, a blur by default.
    #[test]
    fn a_variant_carries_its_file_blur_and_sound_else_the_places() {
        let s = story(GARAGE);
        let night = shown(&s, "garage", &now("2026-12-31", "22:00")).unwrap();
        assert!(night.video && night.blur, "a webm loop, blurred by default");
        assert_eq!(night.ambient.as_deref(), Some("wind.ogg"), "its own sound");
        let rain = shown(
            &s,
            "garage",
            &Now {
                weather: Some("rain"),
                ..now("2026-07-01", "12:00")
            },
        )
        .unwrap();
        assert!(!rain.video && !rain.blur, "a still, not blurred");
        assert_eq!(rain.ambient.as_deref(), Some("hum.ogg"), "the place's sound");
    }

    // Season and time of day are read from story time, at the boundaries
    // the spec names.
    #[test]
    fn seasons_and_times_of_day_have_fixed_boundaries() {
        let d = |s: &str| Season::of(s.parse().unwrap());
        assert_eq!(d("2026-02-28"), Season::Winter);
        assert_eq!(d("2026-03-01"), Season::Spring);
        assert_eq!(d("2026-08-31"), Season::Summer);
        assert_eq!(d("2026-11-30"), Season::Autumn);
        assert_eq!(d("2026-12-01"), Season::Winter);
        let t = |s: &str| Period::of(s.parse().unwrap());
        assert_eq!(t("04:59"), Period::Night);
        assert_eq!(t("05:00"), Period::Morning);
        assert_eq!(t("12:00"), Period::Afternoon);
        assert_eq!(t("18:00"), Period::Evening);
        assert_eq!(t("22:00"), Period::Night);
    }
}
