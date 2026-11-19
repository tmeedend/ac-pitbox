//! Content Manager's copies of the in-game thumbnail sessions (GRILLE§7.1),
//! taken out of its result list (RESULTAT§5).
//!
//! These are CM's data, not the game's files: golden rules 2 and 5 do not
//! apply as written. Their spirit does: **only what Pit Box caused, or what
//! the user confirmed, goes** — and always to the recycle bin.
//!
//! What a thumbnail session looks like, measured on the 30 such copies of the
//! dev machine: Magione, a Quick Drive practice (one car) or track day (up to
//! twelve), and not a lap begun by any car — the game is paused before a
//! wheel turns.

use std::path::Path;

use serde::Serialize;

use super::cmcopy::{self, CmCopy};

/// How long after the game closed CM's copy may still appear.
const COPY_DELAY_MS: i64 = 60_000;

/// Whether a copy has the shape of a thumbnail session (RESULTAT§5.2).
pub fn looks_like_thumbnail(c: &CmCopy) -> bool {
    c.track_id.eq_ignore_ascii_case(crate::gameshot::STUDIO_TRACK)
        && c.track_layout.is_none()
        && c.quick_drive.is_some()
        && matches!(c.mode.as_str(), "practice" | "trackday")
        && (1..=crate::gameshot::CARS_PER_SESSION).contains(&c.result.entries.len())
        && !c.result.any_lap_begun()
}

/// Whether a copy is the one of this thumbnail job: written while it ran, the
/// player's car the job's first, and every car one of the job's.
///
/// Not "the same cars": CM has been seen leaving cars out of a track day and
/// doubling others in their place (`gameshot::report_left_out`). The count
/// must match, the membership must hold.
pub fn is_job_copy(c: &CmCopy, cars: &[String], from_ms: i64, to_ms: i64) -> bool {
    looks_like_thumbnail(c)
        && c.at_ms >= from_ms
        && c.at_ms <= to_ms + COPY_DELAY_MS
        && c.result.entries.len() == cars.len()
        && cars.first().is_some_and(|first| first == c.player_car())
        && c.result.entries.iter().all(|e| cars.contains(&e.car_id))
}

/// One of CM's copies the Maintenance action proposes to take away.
#[derive(Debug, Clone, Serialize)]
pub struct ThumbnailCopy {
    pub file: String,
    pub at_ms: i64,
    pub cars: Vec<String>,
}

fn read(dir: &Path, file: &str, modified: i64) -> Option<CmCopy> {
    let bytes = std::fs::read(dir.join(file)).ok()?;
    cmcopy::parse(file, &bytes, modified).ok()
}

/// Every copy of the folder with the shape of a thumbnail session, latest
/// first. Reads only.
pub fn list_past(dir: &Path) -> Vec<ThumbnailCopy> {
    let mut out: Vec<ThumbnailCopy> = cmcopy::list(dir)
        .into_iter()
        .filter_map(|(file, modified)| read(dir, &file, modified))
        .filter(looks_like_thumbnail)
        .map(|c| ThumbnailCopy {
            cars: c.result.entries.iter().map(|e| e.car_id.clone()).collect(),
            file: c.file,
            at_ms: c.at_ms,
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.at_ms));
    out
}

/// A file of the folder by its bare name: a path, or anything that walks out
/// of it, is refused.
fn own_file(dir: &Path, file: &str) -> Option<std::path::PathBuf> {
    let plain = Path::new(file).file_name().and_then(|n| n.to_str()) == Some(file);
    (plain && file.to_ascii_lowercase().ends_with(".json")).then(|| dir.join(file))
}

/// Puts the given copies in the recycle bin, each checked again first: the
/// list comes from the screen, the decision on each file is made here.
/// Returns the files that went; every other one is logged with its reason.
pub fn trash(dir: &Path, files: &[String], bin: &dyn Fn(&Path) -> Result<(), String>) -> Vec<String> {
    let mut gone = Vec::new();
    for file in files {
        let Some(path) = own_file(dir, file) else {
            log::warn!("results: {file} is not a file of CM's session folder, left alone");
            continue;
        };
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp_millis())
            .unwrap_or(0);
        match read(dir, file, modified) {
            Some(c) if looks_like_thumbnail(&c) => match bin(&path) {
                Ok(()) => {
                    log::info!("results: CM's copy of a thumbnail session {file} sent to the recycle bin");
                    gone.push(file.clone());
                }
                Err(e) => log::warn!("results: {file} not sent to the recycle bin ({e})"),
            },
            _ => log::warn!("results: {file} does not look like a thumbnail session, left alone"),
        }
    }
    gone
}

/// After a thumbnail session: the copies of the folder that could be CM's copy
/// of it (RESULTAT§5.1). Only one is certain; the caller decides.
pub fn job_copies(dir: &Path, cars: &[String], from_ms: i64, to_ms: i64) -> Vec<String> {
    cmcopy::list(dir)
        .into_iter()
        .filter(|(_, modified)| *modified >= from_ms - COPY_DELAY_MS)
        .filter_map(|(file, modified)| read(dir, &file, modified))
        .filter(|c| is_job_copy(c, cars, from_ms, to_ms))
        .map(|c| c.file)
        .collect()
}

/// The copy of the session, waiting for CM to write it: it does so a second
/// or so after the game, and nothing waits for it when Content Manager was
/// not Pit Box's own (`cmwatch`). Several candidates, or none in time, and
/// nothing is touched — the log says so.
pub fn wait_job_copy(
    dir: &Path,
    cars: &[String],
    from_ms: i64,
    to_ms: i64,
    tries: usize,
    pause: std::time::Duration,
) -> Option<String> {
    for attempt in 0..tries {
        match job_copies(dir, cars, from_ms, to_ms).as_slice() {
            [one] => return Some(one.clone()),
            [] if attempt + 1 < tries => std::thread::sleep(pause),
            [] => {}
            many => {
                log::warn!(
                    "results: {} CM copies could be the thumbnail session of {} ({}), none removed",
                    many.len(),
                    cars.join(", "),
                    many.join(", ")
                );
                return None;
            }
        }
    }
    log::info!(
        "results: no CM copy found for the thumbnail session of {}",
        cars.join(", ")
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessionlog::cmcopy::tests::sample;

    fn cars(c: &CmCopy) -> Vec<String> {
        c.result.entries.iter().map(|e| e.car_id.clone()).collect()
    }

    // RESULTAT§5.2: the two real thumbnail copies, a track day of twelve and
    // a practice of one, have the shape; a practice with laps and an online
    // session do not.
    #[test]
    fn a_thumbnail_session_is_recognised_by_its_shape() {
        assert!(
            looks_like_thumbnail(&sample("trackday")),
            "twelve cars at Magione, no lap"
        );
        assert!(looks_like_thumbnail(&sample("shot")), "one car at Magione, no lap");
        assert!(!looks_like_thumbnail(&sample("laps")), "laps were driven, elsewhere");
        assert!(!looks_like_thumbnail(&sample("online")));
    }

    // RESULTAT§5.1: the copy of a job is the one written while it ran, with
    // its cars — not the copy of another job, nor one written before.
    #[test]
    fn the_copy_of_a_job_is_told_by_its_time_and_cars() {
        let c = sample("trackday");
        let job = cars(&c);
        assert!(
            is_job_copy(&c, &job, c.at_ms - 30_000, c.at_ms - 1_000),
            "written just after the game"
        );
        assert!(
            !is_job_copy(&c, &job, c.at_ms + 1_000, c.at_ms + 60_000),
            "written before the job"
        );
        // A car of the session the job did not ask for.
        let other: Vec<String> = job
            .iter()
            .map(|car| {
                if car == "art_skyline_r32_gtr" {
                    "some_other_car".into()
                } else {
                    car.clone()
                }
            })
            .collect();
        assert!(!is_job_copy(&c, &other, c.at_ms - 30_000, c.at_ms), "another job");
        let mut first = job.clone();
        first.swap(0, 1);
        assert!(
            !is_job_copy(&c, &first, c.at_ms - 30_000, c.at_ms),
            "the player is the job's first car"
        );
    }

    // A copy is trashed only after being checked again, and only by its bare
    // name in the folder.
    #[test]
    fn trash_takes_only_thumbnail_copies_of_the_folder() {
        let dir = crate::testutil::temp_dir("thumbcopies");
        std::fs::write(
            dir.join("261115-210327.json"),
            include_bytes!("../raceresult/samples/cm_thumbnail_trackday.json"),
        )
        .unwrap();
        std::fs::write(
            dir.join("260912-184107.json"),
            include_bytes!("../raceresult/samples/cm_practice_laps.json"),
        )
        .unwrap();
        let listed = list_past(&dir);
        assert_eq!(listed.len(), 1, "the practice with laps is not proposed");
        assert_eq!(listed[0].cars.len(), 12);

        let binned = std::cell::RefCell::new(Vec::new());
        let bin = |p: &Path| {
            binned.borrow_mut().push(p.to_path_buf());
            Ok(())
        };
        let gone = trash(
            &dir,
            &[
                "261115-210327.json".into(),
                "260912-184107.json".into(),
                "..\\261115-210327.json".into(),
            ],
            &bin,
        );
        assert_eq!(gone, vec!["261115-210327.json".to_string()], "only the thumbnail copy");
        assert_eq!(binned.borrow().len(), 1, "nothing else reached the bin");
    }
}
