//! What reading the cars' files changed in the library (FICHE§9.3).
//!
//! The first start of a version with the tech sheet — or of one whose readers
//! were fixed (`READER_VERSION`) — reads every car in the background, and the
//! library's spec columns change under the user: 111 cars gained "turbo" on
//! the reference install, 19 changed drivetrain. That is the point of the
//! sheet, and it must be **said, in numbers**, not discovered in a filter.
//!
//! Kept in a file of its own, `techsheet-report.json`, until the user closes
//! it: the backfill may end before the screen listens, or while it looks
//! elsewhere. Same shape as the catalogue's report (`catalog_update.rs`),
//! which it sits next to without being one — no rule changed here.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "techsheet-report.json";

/// How one spec column moved, in cars.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldChanges {
    /// Empty before, a value now.
    pub gained: usize,
    /// One value before, another now.
    pub changed: usize,
    /// A value before, empty now.
    pub lost: usize,
}

/// The report of one backfill.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Cars read.
    pub cars: usize,
    /// Per spec column (`drivetrain`, `aspiration`…), in the cache's order.
    pub fields: BTreeMap<String, FieldChanges>,
    #[serde(default)]
    pub dismissed: bool,
}

impl Report {
    /// Counts one car: its five cached columns before and after the reading.
    pub fn count(&mut self, before: &[Option<String>], after: &[Option<String>]) {
        self.cars += 1;
        for ((f, b), a) in super::CACHED.iter().zip(before).zip(after) {
            if b == a {
                continue;
            }
            let c = self.fields.entry(f.to_string()).or_default();
            match (b, a) {
                (None, _) => c.gained += 1,
                (_, None) => c.lost += 1,
                _ => c.changed += 1,
            }
        }
    }

    /// Nothing moved: nothing to tell.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// The report still to show, if any.
pub fn load(dir: &Path) -> Option<Report> {
    let path = dir.join(FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<Report>(&text) {
        Ok(r) => (!r.dismissed).then_some(r),
        Err(e) => {
            log::warn!("{} unreadable, no tech sheet report — {e}", path.display());
            None
        }
    }
}

/// Written synchronously (CLAUDE.md rule 6): the report must survive a close
/// of the app right after the backfill.
pub fn save(dir: &Path, report: &Report) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(FILE), json).map_err(|e| e.to_string())
}

/// "Close": the report stays on disk, marked, so a restart does not show it
/// again.
pub fn dismiss(dir: &Path) -> Result<(), String> {
    let Some(mut report) = load(dir) else {
        return Ok(());
    };
    report.dismissed = true;
    save(dir, &report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(v: [Option<&str>; 5]) -> Vec<Option<String>> {
        v.iter().map(|x| x.map(str::to_string)).collect()
    }

    /// FICHE§9.3 — a column that gains a value, changes or loses one is
    /// counted as such; one that stays the same is not counted at all.
    #[test]
    fn each_column_is_counted_by_how_it_moved() {
        let mut r = Report::default();
        // drivetrain, aspiration, gearbox, engine_config, engine_pos
        r.count(
            &cols([Some("RWD"), None, Some("MANUAL"), Some("V8"), None]),
            &cols([Some("AWD"), Some("TURBO"), Some("MANUAL"), None, None]),
        );
        r.count(
            &cols([Some("RWD"), None, None, None, None]),
            &cols([Some("RWD"), Some("NA"), None, None, None]),
        );
        assert_eq!(r.cars, 2);
        assert_eq!(
            r.fields["drivetrain"],
            FieldChanges {
                gained: 0,
                changed: 1,
                lost: 0
            }
        );
        assert_eq!(
            r.fields["aspiration"],
            FieldChanges {
                gained: 2,
                changed: 0,
                lost: 0
            }
        );
        assert_eq!(
            r.fields["engine_config"],
            FieldChanges {
                gained: 0,
                changed: 0,
                lost: 1
            }
        );
        assert!(!r.fields.contains_key("gearbox"), "unchanged: not in the report");
    }

    /// A report is shown until closed, then never again — a restart included.
    #[test]
    fn a_report_is_kept_until_closed() {
        let dir = crate::testutil::temp_dir("techsheet-report");
        assert!(load(&dir).is_none(), "nothing written yet");
        let mut r = Report::default();
        r.count(&cols([None; 5]), &cols([Some("RWD"), None, None, None, None]));
        save(&dir, &r).unwrap();
        assert_eq!(load(&dir), Some(r.clone()), "read back as written");
        dismiss(&dir).unwrap();
        assert!(load(&dir).is_none(), "closed");
    }
}
