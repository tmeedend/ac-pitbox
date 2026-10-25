//! Story Mode and Create Your Own (`docs/story/SPEC-story-mode.md`).
//!
//! A long-running work in progress, built on `main` behind a switch rather
//! than on a branch (`CLAUDE.md`, « Chantiers longs derrière un drapeau »).
//! The dependency goes one way: this module may use the rest of the app, the
//! rest of the app never uses it — only `commands/story.rs` and `lib.rs` name
//! it, and `check-conventions.mjs` (`story-isolation`) keeps it that way.

pub mod backups;
pub mod frames;
pub mod history;
pub mod holdings;
pub mod media;
pub mod packages;
pub mod play;
pub mod studio;

/// Turns the Story entry on in a built installer, where it is hidden by
/// default. Passed once on the command line; the frontend then records it in
/// `ui_prefs.json`, so later launches need nothing.
const ENABLE_STUDIO: &str = "--enable-story-studio";
/// The way back, so a tester is never left with an entry they cannot remove
/// without editing `ui_prefs.json` by hand.
const DISABLE_STUDIO: &str = "--disable-story-studio";

/// What the command line asks of the Story switch: `Some(true)` to turn it
/// on, `Some(false)` to turn it off, `None` to leave it as stored. When both
/// options are given the last one wins, as with any repeated option.
pub fn studio_switch_from_args<I, S>(args: I) -> Option<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().fold(None, |state, arg| match arg.as_ref() {
        ENABLE_STUDIO => Some(true),
        DISABLE_STUDIO => Some(false),
        _ => state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Story entry stays hidden in a release build unless it is asked for
    // on the command line (CLAUDE.md, « Chantiers longs derrière un
    // drapeau »).
    #[test]
    fn studio_switch_reads_only_its_own_options() {
        assert_eq!(
            studio_switch_from_args(["pitbox.exe"]),
            None,
            "no option leaves the stored switch alone"
        );
        assert_eq!(
            studio_switch_from_args(["pitbox.exe", "--enable-story-studio"]),
            Some(true),
            "the enable option turns the entry on"
        );
        assert_eq!(
            studio_switch_from_args(["pitbox.exe", "--disable-story-studio"]),
            Some(false),
            "the disable option turns it off"
        );
        assert_eq!(
            studio_switch_from_args(["pitbox.exe", "--enable-story-studio", "--disable-story-studio"]),
            Some(false),
            "the last option given wins"
        );
        assert_eq!(
            studio_switch_from_args(["pitbox.exe", "--enable-story-studio=1", "story-studio"]),
            None,
            "only the exact options count, nothing that merely looks like them"
        );
    }
}
