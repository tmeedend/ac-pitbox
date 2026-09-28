//! The showcase (ESPACE§): putting a mod in it — its heavy files go, its row
//! and everything the user added to it stay — and taking it out again, when
//! the import of its archive brings the files back ([`rehydrate`]).
//!
//! What a skeleton keeps is `skeleton.rs`; this module is the removal, in the
//! order that makes an interruption harmless:
//!
//! 1. **deactivate** through the normal path — and stop if that fails: the
//!    source of a mod still laid in the game is never removed;
//! 2. make sure the **tech sheet** is in the base, and **freeze the card
//!    image**, both before the files they are read from go;
//! 3. write the **manifest**, before any removal: an app killed halfway
//!    leaves a manifest that says more is missing than is, never less;
//! 4. **remove** what the manifest lists — recycle bin first (ESPACE R8) —,
//!    reduce the kept track previews, prune the emptied folders;
//! 5. **mark** the version in the base;
//! 6. **journal** it in the mod's history.
//!
//! A manifest on a version still marked complete therefore means "stopped
//! between 3 and 5", and [`resume_interrupted`] finishes the job at startup.
//!
//! Only cars and tracks, and only their versions, in this first lot: layers,
//! attached skins and sounds, additions to the game and resources keep their
//! files (ESPACE§5.4 is the next step). Keeping them is safe — none of them
//! can reach the game without their host.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::{self, ModRow, VersionRow};
use crate::skeleton::{self, Manifest};

/// What putting one mod in the showcase did.
#[derive(Debug, Clone, Serialize)]
pub struct ShowcaseOutcome {
    pub mod_id: String,
    /// Bytes given back to the disk, all versions together.
    pub freed_bytes: u64,
    /// True when every removed file is in the recycle bin; false when the bin
    /// refused (a mod weighs more than its quota as a rule) and they were
    /// deleted for good. Said after the fact, as ESPACE R8 asks.
    pub recycled: bool,
    /// It was in the game and was taken out first.
    pub was_active: bool,
}

/// The preview a version shows on its own: its first skin's for a car, its
/// default layout's for a track.
fn own_preview(kind: ModKind, dir: &Path) -> Option<PathBuf> {
    match kind {
        ModKind::Car => crate::inspect::preview_path(kind, dir),
        ModKind::Track => crate::inspect::track_preview(dir),
    }
    .map(PathBuf::from)
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Puts a mod in the showcase (ESPACE§5.5). `card_image` is the image its
/// card shows right now, as the screen chose it (a preferred skin, a
/// regenerated grid thumbnail); `None` falls back on the backend's choice.
/// `keep_archive` keeps the source archive kept at import, if any, which is
/// what recovers the mod in one click and offline (ESPACE§5.2).
pub fn to_showcase(
    conn: &Connection,
    cfg: &AppConfig,
    mod_id: &str,
    card_image: Option<&Path>,
    keep_archive: bool,
) -> Result<ShowcaseOutcome, String> {
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    if m.is_stock {
        // Kunos content and mods installed outside Pit Box have no library
        // copy to free: their only files are the game's.
        return Err(crate::errors::STOCK_NOT_FREEABLE.into());
    }
    let active = m.active_version_id.clone().ok_or(crate::errors::NO_ACTIVE_VERSION)?;
    skeleton::guard(conn, &active)?;
    let kind = ModKind::from_kind(&m.kind).unwrap_or(ModKind::Car);

    // 1. Out of the game first, by the normal path: it also withdraws the
    //    additions and restores the game files they replaced (§4.5.4).
    let was_active = crate::activation::is_mod_active(cfg, kind, mod_id);
    if was_active {
        crate::activation::deactivate(conn, cfg, mod_id)?;
    }

    // 2. The tech sheet is read from `data.acd`, about to go (ESPACE§3.4).
    //    Cheap when it is already current.
    if kind == ModKind::Car {
        crate::techsheet::refresh_active(conn, cfg, mod_id, false).map_err(|e| e.to_string())?;
    }
    let card_image = card_image
        .map(Path::to_path_buf)
        .or_else(|| crate::library::preview_for(conn, cfg, &m).map(PathBuf::from));

    // **All versions or none** (ESPACE§5.4): a mod half in the showcase, an
    // old version complete next to a skeleton, is a state nobody asked for.
    // Hence two passes. The first puts every version's files aside — a move
    // on the same volume, undone as easily as it is done — and the first
    // version that resists puts them all back. Only then does the second send
    // them away, the one step that cannot be undone.
    let versions: Vec<VersionRow> = overlay::get_versions(conn, mod_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|v| !v.is_skeleton())
        .collect();
    let stamp = now();
    let mut freeing: Vec<Freeing> = Vec::with_capacity(versions.len());
    for v in &versions {
        let image = (v.id == active).then_some(card_image.as_deref()).flatten();
        let prepared = prepare(cfg, kind, &m, v, image, &stamp).and_then(|f| {
            let staged = f.stage();
            freeing.push(f);
            staged
        });
        if let Err(e) = prepared {
            for f in &freeing {
                f.roll_back();
            }
            return Err(e);
        }
    }

    let mut freed_bytes = 0;
    let mut recycled = true;
    for f in &freeing {
        let (freed, bin) = f.complete(conn, kind)?;
        freed_bytes += freed;
        recycled &= bin;
        if !keep_archive {
            recycled &= drop_kept_archive(conn, cfg, f.v)?;
        }
    }

    // 6. A lifecycle event: it belongs in the mod's timeline.
    let details = serde_json::json!({ "key": "showcased", "bytes": freed_bytes }).to_string();
    overlay::add_history(conn, mod_id, &stamp, "SHOWCASE", &details).map_err(|e| e.to_string())?;

    Ok(ShowcaseOutcome {
        mod_id: mod_id.to_string(),
        freed_bytes,
        recycled,
        was_active,
    })
}

/// One version on its way into the showcase: its folder, and the manifest
/// that lists — and alone decides — what leaves it.
struct Freeing<'a> {
    v: &'a VersionRow,
    mod_id: &'a str,
    dir: PathBuf,
    manifest: Manifest,
}

/// Steps 2 and 3 for one version: frozen image, projections out, manifest
/// written. Nothing has left the folder yet.
fn prepare<'a>(
    cfg: &AppConfig,
    kind: ModKind,
    m: &'a ModRow,
    v: &'a VersionRow,
    card_image: Option<&Path>,
    stamp: &str,
) -> Result<Freeing<'a>, String> {
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    // A version whose folder vanished still becomes a skeleton: R3 wants the
    // folder there, and the manifest has to live somewhere.
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    // 2. The frozen image, read from files about to go. Not fatal: a card
    //    without an image is better than a removal that stops halfway.
    //    Before the projections are removed: the card very often shows a
    //    livery from an attached skin pack, reached through one of them.
    let image = card_image.map(Path::to_path_buf).or_else(|| own_preview(kind, &dir));
    if let Some(src) = image {
        if let Err(e) = skeleton::freeze_image(&src, &dir) {
            log::warn!("showcase {}: card image not frozen: {e}", m.id_interne);
        }
    }
    remove_projections(&dir);

    // 3. The manifest, before anything goes.
    let mut manifest = Manifest::new(
        stamp.to_string(),
        m.id_interne.clone(),
        skeleton::removable_files(kind, &dir),
    );
    manifest.version_label = v.version_label.clone();
    manifest.content_signature = v.content_signature.clone();
    manifest.source_archive = v.source_archive.clone();
    manifest.source_site = v.source_site.clone();
    manifest.source_file_name = v.source_file_name.clone();
    let freeing = Freeing {
        v,
        mod_id: &m.id_interne,
        dir,
        manifest,
    };
    if let Err(e) = skeleton::write_manifest(&freeing.dir, &freeing.manifest) {
        freeing.roll_back();
        return Err(e);
    }
    Ok(freeing)
}

impl Freeing<'_> {
    /// Where this version's files wait for the recycle bin: next to its
    /// folder (same volume, so a move is a rename), named after it so a
    /// resume finds it again.
    fn root(&self) -> PathBuf {
        let name = self.dir.file_name().unwrap_or_default().to_string_lossy();
        self.dir
            .parent()
            .unwrap_or(&self.dir)
            .join(format!(".pitbox-freeing-{name}"))
    }

    /// The folder the recycle bin receives, named after the mod: that is the
    /// name the bin shows.
    fn staging(&self) -> PathBuf {
        self.root().join(self.mod_id)
    }

    /// Step 4, first half: moves every listed file into the staging folder.
    /// A file already there was moved by the interrupted run this one
    /// resumes. Undone by [`Freeing::roll_back`] — the caller decides, since
    /// a failure elsewhere must put this version back too.
    fn stage(&self) -> Result<(), String> {
        let staging = self.staging();
        std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
        for f in &self.manifest.removed {
            let src = self.dir.join(&f.path);
            if !src.exists() {
                continue;
            }
            let dst = staging.join(&f.path);
            dst.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::rename(&src, &dst))
                .map_err(|e| {
                    log::warn!("showcase {}: {} not moved ({e})", self.mod_id, src.display());
                    format!("{}: {e}", src.display())
                })?;
        }
        Ok(())
    }

    /// Puts back **everything** the staging folder holds — this run's moves
    /// and, on a resume, those of the run that was interrupted — so that a
    /// rollback never deletes a file.
    ///
    /// Only when all of it is back does the version read as complete again:
    /// manifest and frozen image removed, staging folder with them, and no
    /// resume will try to free it behind the user's back. If anything could
    /// not be put back, the manifest **and** the staging folder stay: the
    /// version is then in the state "stopped between 3 and 5", which the next
    /// start finishes — the one way out of it that loses nothing.
    fn roll_back(&self) {
        let (root, staging, mod_id) = (self.root(), self.staging(), self.mod_id);
        let mut all_back = true;
        for entry in walkdir::WalkDir::new(&staging).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(&staging) else {
                continue;
            };
            let back = self.dir.join(rel);
            let res = back
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::rename(entry.path(), &back));
            if let Err(e) = res {
                log::warn!("showcase {mod_id}: {} not put back: {e}", back.display());
                all_back = false;
            }
        }
        if !all_back {
            log::warn!(
                "showcase {mod_id}: rollback incomplete, {} kept for the next start to finish",
                root.display()
            );
            return;
        }
        let own = [self.dir.join(skeleton::MANIFEST_NAME)]
            .into_iter()
            .chain(skeleton::image_of(&self.dir))
            .filter(|p| p.exists());
        for file in own {
            let _ = std::fs::remove_file(&file)
                .inspect_err(|e| log::warn!("showcase {mod_id}: {} not removed: {e}", file.display()));
        }
        if root.exists() {
            let _ = std::fs::remove_dir_all(&root)
                .inspect_err(|e| log::warn!("showcase {mod_id}: {} not removed: {e}", root.display()));
        }
    }

    /// Step 4, second half, and step 5: the staging folder goes to the
    /// recycle bin in one piece — one entry named after the mod rather than
    /// thousands of loose files —, the kept track previews are reduced, the
    /// emptied folders pruned, and the base marked. Returns what was freed
    /// and whether the bin took it.
    ///
    /// The kept `ui/` files are **copied** into the staging folder first, so
    /// that what lands in the recycle bin is a complete mod folder: restored
    /// and dropped into Pit Box, it rehydrates the mod like its archive would.
    fn complete(&self, conn: &Connection, kind: ModKind) -> Result<(u64, bool), String> {
        let (dir, mod_id) = (&self.dir, self.mod_id);
        let staging = self.staging();
        copy_kept(dir, &staging);
        let recycled = crate::maintenance::trash_or_delete(&staging)?;
        let root = self.root();
        if root.exists() {
            std::fs::remove_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        }

        let mut reduced = 0;
        for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
            let Ok(rel) = entry.path().strip_prefix(dir) else {
                continue;
            };
            if entry.file_type().is_file() && skeleton::is_reduced(kind, rel) {
                match skeleton::reduce_in_place(entry.path()) {
                    Ok(gain) => reduced += gain,
                    Err(e) => log::warn!("showcase {mod_id}: {} not reduced: {e}", entry.path().display()),
                }
            }
        }
        // Emptied folders go, the version's own stays (ESPACE R3): everything
        // that resolves the mod's folder must keep finding it.
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                crate::extras::prune_empty_dirs(&entry.path());
            }
        }
        let freed = self.manifest.removed_bytes + reduced;
        overlay::mark_version_freed(conn, &self.v.id, &self.manifest.freed_at, freed as i64)
            .map_err(|e| e.to_string())?;
        Ok((freed, recycled))
    }
}

/// Skin projections inside a version folder are junctions onto skins stored
/// apart (§8.3), not files of the version: removing the link leaves the skin
/// where it is stored, and keeps `skins/` from outliving the skeleton.
fn remove_projections(dir: &Path) {
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if entry.path_is_symlink() {
            if let Err(e) = crate::activation::remove_junction(entry.path()) {
                log::warn!("showcase: projection {} not removed: {e}", entry.path().display());
            }
        }
    }
}

/// Copies what the skeleton keeps into the staging folder, the showcase's own
/// files excepted. Best-effort: the recycled folder is a convenience, the
/// skeleton is what matters.
fn copy_kept(dir: &Path, staging: &Path) {
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        let Ok(rel) = entry.path().strip_prefix(dir) else {
            continue;
        };
        let name = rel.to_string_lossy();
        if !entry.file_type().is_file() || name.starts_with(".pitbox-vitrine") {
            continue;
        }
        let dst = staging.join(rel);
        let res = dst
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::copy(entry.path(), &dst).map(|_| ()));
        if let Err(e) = res {
            log::warn!(
                "showcase: {} not copied to the recycled folder: {e}",
                entry.path().display()
            );
        }
    }
}

/// Sends the kept source archive of a version to the recycle bin and forgets
/// it. Its folder is `<lib>/_source_archives/<uuid>/`, one per version.
fn drop_kept_archive(conn: &Connection, cfg: &AppConfig, v: &VersionRow) -> Result<bool, String> {
    let Some(kept) = v
        .kept_archive_path
        .as_deref()
        .and_then(|p| crate::libpath::resolve(cfg.library_path.as_deref(), p))
        .and_then(|p| p.parent().map(Path::to_path_buf))
    else {
        return Ok(true);
    };
    let recycled = crate::maintenance::trash_or_delete(&kept)?;
    overlay::clear_kept_archive(conn, &v.id).map_err(|e| e.to_string())?;
    Ok(recycled)
}

/// The startup net of ESPACE§5.5: a version still marked complete whose
/// folder holds a manifest was stopped between writing it and marking the
/// base. The removal is finished, from the manifest, and journaled. Returns
/// how many versions it finished.
///
/// A manifest is only trusted when it names this mod and this version's
/// signature: a folder imported from an archive that happened to contain a
/// skeleton's manifest must never be freed on the strength of it.
pub fn resume_interrupted(conn: &Connection, cfg: &AppConfig) -> usize {
    let mut done = 0;
    let Ok(mods) = overlay::list_mods(conn) else {
        return 0;
    };
    for m in mods.iter().filter(|m| !m.is_stock) {
        let kind = ModKind::from_kind(&m.kind).unwrap_or(ModKind::Car);
        for v in overlay::get_versions(conn, &m.id_interne).unwrap_or_default() {
            if v.is_skeleton() {
                continue;
            }
            let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path) else {
                continue;
            };
            let Some(manifest) = skeleton::read_manifest(&dir) else {
                continue;
            };
            if manifest.mod_id != m.id_interne || manifest.content_signature != v.content_signature {
                log::warn!(
                    "showcase manifest in {} does not describe this version, ignored",
                    dir.display()
                );
                continue;
            }
            if crate::activation::is_mod_active(cfg, kind, &m.id_interne) {
                if let Err(e) = crate::activation::deactivate(conn, cfg, &m.id_interne) {
                    log::warn!("showcase resume {}: not deactivated, left as is: {e}", m.id_interne);
                    continue;
                }
            }
            let freeing = Freeing {
                v: &v,
                mod_id: &m.id_interne,
                dir,
                manifest,
            };
            let finished = match freeing.stage() {
                Ok(()) => freeing.complete(conn, kind),
                Err(e) => {
                    freeing.roll_back();
                    Err(e)
                }
            };
            match finished {
                Ok((freed, _)) => {
                    let details = serde_json::json!({ "key": "showcased", "bytes": freed }).to_string();
                    if let Err(e) =
                        overlay::add_history(conn, &m.id_interne, &freeing.manifest.freed_at, "SHOWCASE", &details)
                    {
                        log::warn!("showcase resume {}: history not written: {e}", m.id_interne);
                    }
                    done += 1;
                }
                Err(e) => log::warn!("showcase resume {}: {e}", m.id_interne),
            }
        }
    }
    done
}

// --- Taking a mod out of the showcase -------------------------------------------

/// What bringing a version's files back did (ESPACE§7.3, ESPACE§7.4).
#[derive(Debug, Clone, Default)]
pub struct Rehydrated {
    /// Paths of the manifest the archive did not bring back, or at another
    /// size. The version is complete all the same (ESPACE§7.4): if what is
    /// missing matters, `broken_reason` says so.
    pub missing: Vec<String>,
    /// Annexes filed into the mod's resources, as a plain import does.
    pub resources_extracted: usize,
}

/// Brings the files of an archive back into the skeleton version `v`, the
/// one whose signature it matches (ESPACE§7.3). **Same version id, same
/// folder**: the timeline, the profiles pinning it and every row keyed on
/// it stay as they are. Nothing the user entered is touched, and the mod is
/// not activated — the import report offers that.
///
/// The files are laid in a fresh folder next to the skeleton, then the two
/// swap. The skeleton — manifest included — therefore goes **before** the
/// base says "complete": an interruption leaves a complete folder still
/// marked skeleton, which the next import of the same archive rehydrates
/// again, never a complete-marked folder with a manifest that
/// [`resume_interrupted`] would free a second time.
#[allow(clippy::too_many_arguments)]
pub fn rehydrate(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    kind: ModKind,
    v: &VersionRow,
    incoming: &Path,
    move_source: bool,
    res_mode: crate::resources::ExtractionMode,
    kept_archive: Option<&str>,
    on_progress: &dyn Fn(f64),
) -> Result<Rehydrated, String> {
    let dir = crate::libpath::resolve(Some(library), &v.library_path).ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let manifest = skeleton::read_manifest(&dir);
    let sibling = |suffix: &str| {
        let mut name = dir.as_os_str().to_owned();
        name.push(suffix);
        crate::importer::unique_dir(Path::new(&name))
    };

    let fresh = sibling(".rehydrating");
    let total = crate::inspect::dir_size_bytes(incoming).max(1);
    let copied = std::cell::Cell::new(0u64);
    let resources_extracted = crate::resources::file_mod_reported(
        incoming,
        &fresh,
        &crate::resources::resources_dir(library, kind, &v.mod_id),
        res_mode,
        move_source,
        crate::resources::Source::ModFolder,
        &|bytes| {
            copied.set(copied.get() + bytes);
            on_progress((copied.get() as f64 / total as f64).min(1.0));
        },
    )?;
    let missing = manifest
        .as_ref()
        .map(|m| skeleton::verify(&fresh, m))
        .unwrap_or_default();

    // The swap. A skeleton folder that vanished (removed by hand) leaves
    // nothing to set aside.
    let old = sibling(".skeleton");
    if dir.exists() {
        std::fs::rename(&dir, &old).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    if let Err(e) = std::fs::rename(&fresh, &dir) {
        if old.exists() {
            let _ = std::fs::rename(&old, &dir)
                .inspect_err(|e| log::warn!("rehydrate {}: skeleton not put back: {e}", v.mod_id));
        }
        if move_source {
            // The files were *moved* here: this folder is now their only
            // copy, and deleting it would destroy what the user imported.
            log::warn!("rehydrate {}: files left in {}", v.mod_id, fresh.display());
        } else {
            let _ = std::fs::remove_dir_all(&fresh)
                .inspect_err(|e| log::warn!("rehydrate {}: {} not removed: {e}", v.mod_id, fresh.display()));
        }
        return Err(format!("{}: {e}", dir.display()));
    }
    if old.exists() {
        let _ = std::fs::remove_dir_all(&old)
            .inspect_err(|e| log::warn!("rehydrate {}: {} not removed: {e}", v.mod_id, old.display()));
    }

    let size = crate::inspect::dir_size_bytes(&dir) as i64;
    overlay::mark_version_full(conn, &v.id, size).map_err(|e| e.to_string())?;
    // A kept archive the version still has wins: replacing it would leave the
    // old copy on disk with nothing pointing at it, while the new one, left
    // unreferenced, is cleaned up by the import (`kept_archive_in_use`).
    let still_kept = v
        .kept_archive_path
        .as_deref()
        .and_then(|p| crate::libpath::resolve(Some(library), p))
        .is_some_and(|p| p.exists());
    if let (Some(kept), false) = (kept_archive, still_kept) {
        overlay::set_kept_archive(conn, &v.id, kept).map_err(|e| e.to_string())?;
    }
    if !missing.is_empty() {
        log::warn!("rehydrate {}: {} expected file(s) missing", v.mod_id, missing.len());
    }
    let details = serde_json::json!({ "key": "rehydrated", "missing": missing.len() }).to_string();
    overlay::add_history(conn, &v.mod_id, &now(), "REHYDRATED", &details).map_err(|e| e.to_string())?;
    // What the files say — skins, CSP, the tech sheet — read again as a
    // reindex does: only the fields read from files, never an entry.
    crate::maintenance::reindex_mod(conn, cfg, &v.mod_id, false)?;
    Ok(Rehydrated {
        missing,
        resources_extracted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGINE: &str = "[ENGINE_DATA]\nLIMITER=8300\n[TURBO_0]\nMAX_BOOST=1\n";
    const DRIVETRAIN: &str = "[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=6\n";

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn photo(path: &Path, width: u32, height: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]))
            .save(path)
            .unwrap();
    }

    /// A library with one managed mod, one version, not deployed.
    struct Fixture {
        base: crate::testutil::TempDir,
        conn: Connection,
        cfg: AppConfig,
        dir: PathBuf,
    }

    fn fixture(tag: &str, id: &str, kind: &str, fill: impl Fn(&Path)) -> Fixture {
        let base = crate::testutil::temp_dir(tag);
        let folder = if kind == "Track" { "tracks" } else { "cars" };
        std::fs::create_dir_all(base.join("ac").join("content").join(folder)).unwrap();
        let dir = base.join("lib").join(folder).join(id).join("v1");
        fill(&dir);
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        // Brand and name as the car's `ui_car.json` says them: a reindex reads
        // them again, and must find nothing to change.
        overlay::upsert_mod(&conn, id, kind, Some("RSS"), Some("Lanzo"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn,
            "v1",
            id,
            Some("1.4"),
            None,
            &now,
            &dir.to_string_lossy(),
            Some("mod_v1.4.7z"),
            "sig-full",
            &["lightingfx".to_string()],
            &["red".to_string()],
            &[],
            &[],
            None,
        )
        .unwrap();
        overlay::set_active_version(&conn, id, "v1").unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(base.join("ac")),
            library_path: Some(base.join("lib")),
            ..Default::default()
        };
        Fixture { base, conn, cfg, dir }
    }

    fn car(dir: &Path) {
        write(
            dir,
            "ui/ui_car.json",
            br#"{"name":"Lanzo","brand":"RSS","specs":{"bhp":"600bhp","weight":"1200kg"}}"#,
        );
        write(dir, "ui/badge.png", b"badge");
        write(dir, "data/engine.ini", ENGINE.as_bytes());
        write(dir, "data/drivetrain.ini", DRIVETRAIN.as_bytes());
        write(dir, "lanzo.kn5", &[7; 5000]);
        write(dir, "sfx/lanzo.bank", &[1; 3000]);
        photo(&dir.join("skins/red/preview.jpg"), 1022, 575);
        write(dir, "skins/red/livery.png", b"livery");
        write(
            dir,
            "extension/ext_config.ini",
            b"[LIGHT_SERIES_1]
MESHES=light
",
        );
    }

    /// Where a version's files wait for the recycle bin, as `Freeing` names it.
    fn staging_root(dir: &Path) -> PathBuf {
        let name = dir.file_name().unwrap().to_string_lossy();
        dir.parent().unwrap().join(format!(".pitbox-freeing-{name}"))
    }

    fn files_of(dir: &Path) -> Vec<String> {
        let mut out: Vec<String> = walkdir::WalkDir::new(dir)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        out.sort();
        out
    }

    /// Rule (ESPACE§3.1, ESPACE§5.5): an active car is taken out of the game, then
    /// its folder keeps exactly the skeleton — no `data/`, no model, no skin
    /// preview — and the manifest lists exactly the rest, with sizes.
    #[test]
    fn a_car_in_the_showcase_keeps_exactly_its_skeleton() {
        let f = fixture("showcase-car", "lanzo", "Car", car);
        crate::activation::activate(&f.conn, &f.cfg, "lanzo", None).unwrap();
        assert!(crate::activation::is_mod_active(&f.cfg, ModKind::Car, "lanzo"));
        let before = crate::inspect::dir_size_bytes(&f.dir) as i64;
        overlay::update_version_size(&f.conn, "v1", before).unwrap();

        let out = to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

        assert!(out.was_active, "it was in the game");
        assert!(
            !f.base.join("ac/content/cars/lanzo").exists(),
            "taken out of the game before anything else"
        );
        assert_eq!(
            files_of(&f.dir),
            vec![
                ".pitbox-vitrine.jpg",
                ".pitbox-vitrine.json",
                "ui/badge.png",
                "ui/ui_car.json"
            ],
            "exactly the skeleton is left"
        );
        let manifest = skeleton::read_manifest(&f.dir).expect("a manifest");
        let listed: Vec<&str> = manifest.removed.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            listed,
            vec![
                "data/drivetrain.ini",
                "data/engine.ini",
                "extension/ext_config.ini",
                "lanzo.kn5",
                "sfx/lanzo.bank",
                "skins/red/livery.png",
                "skins/red/preview.jpg"
            ],
            "the manifest lists what went"
        );
        assert_eq!(
            manifest.content_signature.as_deref(),
            Some("sig-full"),
            "the original signature"
        );
        assert_eq!(manifest.source_archive.as_deref(), Some("mod_v1.4.7z"));
        let image = skeleton::image_of(&f.dir).unwrap();
        assert!(
            std::fs::metadata(&image).unwrap().len() < 100_000,
            "the frozen image is small"
        );

        let v = overlay::get_version(&f.conn, "v1").unwrap().unwrap();
        assert!(v.is_skeleton(), "marked in the base");
        assert_eq!(out.freed_bytes, manifest.removed_bytes, "a car has nothing reduced");
        assert_eq!(v.freed_bytes, Some(out.freed_bytes as i64));
        assert_eq!(
            v.size_bytes,
            Some(before),
            "the size is the mod's own, from before its files went"
        );
        assert_eq!(v.skins, vec!["red".to_string()], "the skin names stay in the base");
        assert_eq!(v.csp_features, vec!["lightingfx".to_string()]);
        assert!(
            !f.dir.parent().unwrap().join(".pitbox-freeing-v1").exists(),
            "no staging folder left behind"
        );
        let history = overlay::get_history(&f.conn, "lanzo").unwrap();
        assert_eq!(history[0].event, "SHOWCASE", "journaled");
    }

    /// Rule (ESPACE R5): once in the showcase, nothing lays the mod in the
    /// game again, and the card says it is in the showcase, not broken.
    #[test]
    fn a_showcase_mod_never_goes_back_into_the_game() {
        let f = fixture("showcase-guard", "lanzo", "Car", car);
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

        assert_eq!(
            crate::activation::activate(&f.conn, &f.cfg, "lanzo", None)
                .err()
                .as_deref(),
            Some(crate::errors::CONTENT_FREED),
            "activation refuses"
        );
        assert!(!f.base.join("ac/content/cars/lanzo").exists(), "and wrote nothing");
        crate::compose::recompose(&f.conn, &f.cfg, "lanzo")
            .expect("a layer action on a mod in the showcase is not an error");
        assert!(!f.base.join("ac/content/cars/lanzo").exists(), "but composes nothing");
        let ctx = crate::bulk::BulkCtx::silent();
        let report = crate::bulk::activate(&ctx, &f.conn, &f.cfg, &["lanzo".to_string()]);
        assert_eq!(report.skipped, vec!["lanzo".to_string()], "a lot counts it apart");
        assert!(report.failed.is_empty(), "never as a failure");
        let now = chrono::Local::now().to_rfc3339();
        overlay::create_profile(&f.conn, "p1", "Endurance", &now).unwrap();
        overlay::add_profile_entry(&f.conn, "p1", "lanzo", "v1").unwrap();
        let applied = crate::profiles::apply(&f.conn, &f.cfg, "p1").unwrap();
        assert_eq!(applied.skipped, vec!["lanzo".to_string()], "a profile leaves it out");
        assert!(applied.errors.is_empty(), "without an error");
        assert_eq!(
            crate::extras::deploy(&f.conn, &f.cfg, crate::extras::OwnerKind::Car, "lanzo")
                .err()
                .as_deref(),
            Some(crate::errors::CONTENT_FREED),
            "its additions to the game stay out"
        );

        let m = overlay::get_mod(&f.conn, "lanzo").unwrap().unwrap();
        assert_eq!(
            crate::maintenance::broken_reason(&f.conn, &f.cfg, &m),
            None,
            "not broken"
        );
        let card = crate::library::detail(&f.conn, &f.cfg, "lanzo").unwrap().unwrap().card;
        assert!(card.base.showcase);
        assert!(!card.broken);
        assert_eq!(
            card.preview.map(PathBuf::from),
            skeleton::image_of(&f.dir),
            "the card shows the frozen image"
        );
        assert_eq!(
            to_showcase(&f.conn, &f.cfg, "lanzo", None, true).err().as_deref(),
            Some(crate::errors::CONTENT_FREED),
            "and it cannot be freed twice"
        );
    }

    /// Rule (ESPACE R2, ESPACE§3.4): nothing that costs nothing is lost. The user's
    /// entries and the tech sheet read from `data/` are identical after, and
    /// a reindex of the skeleton does not wipe what its files no longer say.
    #[test]
    fn nothing_the_user_or_the_sheet_holds_is_lost() {
        let f = fixture("showcase-keep", "lanzo", "Car", car);
        let c = &f.conn;
        overlay::set_favorite(c, "lanzo", true).unwrap();
        overlay::set_manual_tags(c, "lanzo", &["endurance".to_string()]).unwrap();
        overlay::set_mod_field(c, "lanzo", "display_name_user", Some("My Lanzo")).unwrap();
        overlay::set_mod_field(c, "lanzo", "description_user", Some("Notes")).unwrap();
        crate::techsheet::record(c, "lanzo", "v1", &f.dir, false).unwrap();
        overlay::update_version_size(c, "v1", 551_000_000).unwrap();
        overlay::record_decision(c, Some("lanzo"), "mod_v1.4.7z", "extra", "extension/", None);
        c.execute(
            "INSERT INTO media_links (file_path, entity_id, kind) VALUES ('shot.jpg', 'lanzo', 'SCREENSHOT')",
            [],
        )
        .unwrap();
        let rows = |table: &str, column: &str| -> i64 {
            c.query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column} = 'lanzo'"),
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        let mod_before = serde_json::to_value(overlay::get_mod(c, "lanzo").unwrap()).unwrap();
        let sheet_before = serde_json::to_value(crate::techsheet::effective(c, "lanzo").unwrap()).unwrap();
        assert!(
            sheet_before.to_string().contains("RWD"),
            "the fixture's physics reached the sheet — otherwise this test proves nothing"
        );

        to_showcase(c, &f.cfg, "lanzo", None, true).unwrap();
        crate::maintenance::reindex_all(c, &f.cfg, true).unwrap();

        let mut mod_after = serde_json::to_value(overlay::get_mod(c, "lanzo").unwrap()).unwrap();
        // The only field allowed to move: the state itself. The size stays
        // the mod's own — a reindex that measures it included.
        assert_eq!(mod_after["showcase"], serde_json::json!(true));
        mod_after["showcase"] = mod_before["showcase"].clone();
        assert_eq!(mod_after, mod_before, "every entry of the mod is intact");
        assert_eq!(
            serde_json::to_value(crate::techsheet::effective(c, "lanzo").unwrap()).unwrap(),
            sheet_before,
            "the tech sheet is the same, reindex included"
        );
        assert_eq!(rows("import_decisions", "mod_id"), 1, "the import journal is kept");
        assert_eq!(rows("media_links", "entity_id"), 1, "and the media attached by hand");
        let v = overlay::get_version(c, "v1").unwrap().unwrap();
        assert_eq!(v.skins, vec!["red".to_string()], "the reindex kept the skin names");
        assert_eq!(v.csp_features, vec!["lightingfx".to_string()], "and the CSP features");
    }

    /// Rule (ESPACE§3.3): the image frozen is the one the screen shows — a
    /// preferred skin, a regenerated thumbnail —, not the backend's guess.
    #[test]
    fn the_frozen_image_is_the_one_the_card_shows() {
        let f = fixture("showcase-image", "lanzo", "Car", car);
        let thumb = f.base.join("cache/grid.png");
        std::fs::create_dir_all(thumb.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(800, 450, image::Rgba([10, 20, 30, 0]))
            .save(&thumb)
            .unwrap();
        to_showcase(&f.conn, &f.cfg, "lanzo", Some(&thumb), true).unwrap();
        let frozen = skeleton::image_of(&f.dir).unwrap();
        assert!(
            frozen.to_string_lossy().ends_with(".png"),
            "the transparent thumbnail was frozen, not the skin preview"
        );
    }

    /// Rule (ESPACE§3.1, ESPACE§3.3): a track keeps every layout's `ui/`, its
    /// previews reduced in place; the layout map and the models go.
    #[test]
    fn a_track_keeps_its_layouts_with_reduced_previews() {
        let f = fixture("showcase-track", "shannon", "Track", |dir| {
            write(dir, "ui/gp/ui_track.json", br#"{"name":"Shannonville GP"}"#);
            write(dir, "ui/gp/outline.png", b"outline");
            photo(&dir.join("ui/gp/preview.png"), 1920, 1080);
            write(dir, "gp/map.png", b"map");
            write(dir, "shannon.kn5", &[3; 4000]);
        });
        let out = to_showcase(&f.conn, &f.cfg, "shannon", None, true).unwrap();
        assert_eq!(
            files_of(&f.dir),
            vec![
                ".pitbox-vitrine.jpg",
                ".pitbox-vitrine.json",
                "ui/gp/outline.png",
                "ui/gp/preview.png",
                "ui/gp/ui_track.json"
            ]
        );
        let preview = image::open(f.dir.join("ui/gp/preview.png")).unwrap();
        assert_eq!(preview.width(), skeleton::IMAGE_WIDTH, "the preview was reduced");
        assert_eq!(
            std::fs::read(f.dir.join("ui/gp/outline.png")).unwrap(),
            b"outline",
            "the outline untouched"
        );
        let manifest = skeleton::read_manifest(&f.dir).unwrap();
        assert!(
            out.freed_bytes > manifest.removed_bytes,
            "what the reduction gave back is counted too"
        );
    }

    /// Rule (ESPACE§5.5): a file that cannot be moved stops the removal and
    /// puts back what was moved — the version is left complete, without a
    /// manifest a resume would act on.
    #[test]
    fn a_locked_file_leaves_the_version_complete() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = fixture("showcase-locked", "lanzo", "Car", car);
        let before = files_of(&f.dir);
        // No sharing at all: the file cannot be renamed while this is open.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(f.dir.join("sfx/lanzo.bank"))
            .unwrap();
        assert!(
            to_showcase(&f.conn, &f.cfg, "lanzo", None, true).is_err(),
            "the removal stops"
        );
        drop(lock);

        let mut after = files_of(&f.dir);
        after.retain(|p| !p.starts_with(".pitbox-vitrine."));
        assert_eq!(after, before, "every moved file was put back");
        assert!(
            skeleton::read_manifest(&f.dir).is_none(),
            "no manifest left for a resume to act on"
        );
        assert!(!overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
        assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "and nothing to resume");
    }

    /// Rule (ESPACE§5.5): a rollback during a resume puts back what the
    /// interrupted run had already moved too — it never deletes a file.
    /// Bug caught in review: the staging folder, holding the model moved by
    /// the first run, was wiped along with the rollback.
    #[test]
    fn a_rollback_during_a_resume_puts_back_what_the_first_run_moved() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = fixture("showcase-resume-rollback", "lanzo", "Car", car);
        let before = files_of(&f.dir);
        let mut manifest = Manifest::new(
            "2026-09-27T10:00:00Z".into(),
            "lanzo".into(),
            skeleton::removable_files(ModKind::Car, &f.dir),
        );
        manifest.content_signature = Some("sig-full".into());
        skeleton::write_manifest(&f.dir, &manifest).unwrap();
        let staging = staging_root(&f.dir).join("lanzo");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::rename(f.dir.join("lanzo.kn5"), staging.join("lanzo.kn5")).unwrap();

        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(f.dir.join("sfx/lanzo.bank"))
            .unwrap();
        assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "the resume stops");
        drop(lock);

        let mut after = files_of(&f.dir);
        after.retain(|p| !p.starts_with(".pitbox-vitrine."));
        assert_eq!(after, before, "the model moved by the first run is back as well");
        assert!(!staging_root(&f.dir).exists(), "nothing left in staging");
        assert!(!overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
    }

    /// Rule (ESPACE§3.3): the card often shows a livery from an attached
    /// skin pack, reached through a projection junction. It is frozen before
    /// the projection goes — and the stored skin itself is left alone.
    #[test]
    fn a_livery_from_an_attached_skin_pack_is_frozen() {
        let f = fixture("showcase-projected", "lanzo", "Car", car);
        let store = f.base.join("lib/skins/lanzo/pack_livery");
        photo(&store.join("preview.jpg"), 800, 450);
        crate::activation::create_junction(&f.dir.join("skins/pack_livery"), &store).unwrap();

        let shown = f.dir.join("skins/pack_livery/preview.jpg");
        to_showcase(&f.conn, &f.cfg, "lanzo", Some(&shown), true).unwrap();

        let frozen = skeleton::image_of(&f.dir).expect("the livery shown on the card was frozen");
        let img = image::open(frozen).unwrap();
        assert_eq!(
            img.height(),
            270,
            "from the 800×450 livery, not the 1022×575 skin preview"
        );
        assert!(store.join("preview.jpg").is_file(), "the stored skin is untouched");
        assert!(
            !f.dir.join("skins").exists(),
            "and the projection is gone from the skeleton"
        );
    }

    /// A mod with an older version next to the active one, both complete.
    fn with_older_version(f: &Fixture) -> PathBuf {
        let old = f.base.join("lib/cars/lanzo/v0");
        car(&old);
        overlay::insert_version(
            &f.conn,
            "v0",
            "lanzo",
            Some("1.0"),
            None,
            "2020-01-01T00:00:00+00:00",
            &old.to_string_lossy(),
            None,
            "sig-old",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        old
    }

    /// Rule (ESPACE§5.4): every version of the mod goes into the showcase,
    /// not only the active one.
    #[test]
    fn every_version_goes_into_the_showcase() {
        let f = fixture("showcase-versions", "lanzo", "Car", car);
        let old = with_older_version(&f);
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();
        for id in ["v0", "v1"] {
            assert!(
                overlay::get_version(&f.conn, id).unwrap().unwrap().is_skeleton(),
                "{id} is a skeleton"
            );
        }
        assert!(!old.join("lanzo.kn5").exists(), "the old version lost its files too");
    }

    /// Rule (ESPACE§5.4): all versions or none. One version that resists —
    /// here the older one, handled after the active one — puts every version
    /// back as it was: no mod half in the showcase, no journal entry.
    #[test]
    fn one_version_that_resists_leaves_every_version_complete() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = fixture("showcase-older", "lanzo", "Car", car);
        let old = with_older_version(&f);
        let (active_before, old_before) = (files_of(&f.dir), files_of(&old));
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(old.join("sfx/lanzo.bank"))
            .unwrap();
        assert!(
            to_showcase(&f.conn, &f.cfg, "lanzo", None, true).is_err(),
            "the lot reports the mod as failed"
        );
        drop(lock);

        assert_eq!(
            files_of(&f.dir),
            active_before,
            "the active version got every file back"
        );
        assert_eq!(files_of(&old), old_before, "the older one too");
        assert!(!skeleton::is_showcase(&f.conn, "lanzo").unwrap(), "not in the showcase");
        for id in ["v0", "v1"] {
            assert!(!overlay::get_version(&f.conn, id).unwrap().unwrap().is_skeleton());
        }
        assert!(
            overlay::get_history(&f.conn, "lanzo").unwrap().is_empty(),
            "nothing journaled"
        );
        assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "and nothing left for a resume");
    }

    /// Rule (ESPACE§5.5): a manifest written, half the files moved, the base
    /// still saying "complete" — the next start finishes the job, from the
    /// manifest. A manifest that does not describe the version is ignored.
    #[test]
    fn an_interrupted_removal_is_finished_at_startup() {
        let f = fixture("showcase-resume", "lanzo", "Car", car);
        let mut manifest = Manifest::new(
            "2026-09-27T10:00:00Z".into(),
            "lanzo".into(),
            skeleton::removable_files(ModKind::Car, &f.dir),
        );
        manifest.content_signature = Some("another-archive".into());
        skeleton::write_manifest(&f.dir, &manifest).unwrap();
        assert_eq!(
            resume_interrupted(&f.conn, &f.cfg),
            0,
            "a foreign manifest is not trusted"
        );
        assert!(f.dir.join("lanzo.kn5").exists());

        manifest.content_signature = Some("sig-full".into());
        skeleton::write_manifest(&f.dir, &manifest).unwrap();
        // Half done: the model is already in the staging folder.
        let staging = staging_root(&f.dir).join("lanzo");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::rename(f.dir.join("lanzo.kn5"), staging.join("lanzo.kn5")).unwrap();

        assert_eq!(resume_interrupted(&f.conn, &f.cfg), 1, "one version finished");
        assert_eq!(
            files_of(&f.dir),
            vec![".pitbox-vitrine.json", "ui/badge.png", "ui/ui_car.json"],
            "the rest went too"
        );
        assert!(!staging_root(&f.dir).exists(), "the staging folder too");
        assert!(overlay::get_version(&f.conn, "v1").unwrap().unwrap().is_skeleton());
        assert_eq!(resume_interrupted(&f.conn, &f.cfg), 0, "idempotent");
    }

    /// Rule (ESPACE§7.1): "reinstall from the kept archive" is a way back
    /// from the showcase — the version is complete again, not a skeleton
    /// holding complete files.
    #[test]
    fn reinstalling_from_the_kept_archive_takes_the_mod_out_of_the_showcase() {
        let f = fixture("showcase-reinstall", "lanzo", "Car", car);
        // A kept *folder*: no 7-Zip needed to take it back.
        let kept = f.base.join("lib/_source_archives/u1/src");
        car(&kept.join("lanzo"));
        overlay::set_kept_archive(&f.conn, "v1", &kept.to_string_lossy()).unwrap();
        to_showcase(&f.conn, &f.cfg, "lanzo", None, true).unwrap();

        crate::maintenance::reinstall_from_archive(&f.conn, &f.cfg, "lanzo").unwrap();

        assert!(!skeleton::is_showcase(&f.conn, "lanzo").unwrap(), "out of the showcase");
        assert!(f.dir.join("lanzo.kn5").is_file(), "the files are back");
        assert!(skeleton::read_manifest(&f.dir).is_none(), "and the manifest gone");
        crate::activation::activate(&f.conn, &f.cfg, "lanzo", None).expect("it can go into the game");
    }

    /// Rule (ESPACE§5.2): the kept source archive goes only when asked, and
    /// the base forgets it with it.
    #[test]
    fn the_kept_archive_goes_only_when_asked() {
        let f = fixture("showcase-archive", "lanzo", "Car", car);
        let kept = f.base.join("lib/_source_archives/u1/mod_v1.4.7z");
        write(&f.base, "lib/_source_archives/u1/mod_v1.4.7z", b"7z");
        overlay::set_kept_archive(&f.conn, "v1", &kept.to_string_lossy()).unwrap();

        to_showcase(&f.conn, &f.cfg, "lanzo", None, false).unwrap();
        assert!(!kept.exists(), "removed on request");
        assert_eq!(
            overlay::get_version(&f.conn, "v1").unwrap().unwrap().kept_archive_path,
            None
        );
    }

    /// Imports the folders of `src` as the app does, through the public path.
    fn import(db: &overlay::Db, cfg: &AppConfig, src: &Path) -> Vec<crate::importer::ImportedMod> {
        let ctx = crate::import_progress::ImportCtx::silent();
        let rules = crate::rules::default_rules();
        let paths = [src.to_string_lossy().into_owned()];
        crate::importer::import_folders(&ctx, db, cfg, &rules, &paths, true, &[])
            .unwrap()
            .into_iter()
            .flat_map(|r| r.mods)
            .collect()
    }

    /// A library and an AC install, and a car folder to import from `src`.
    fn round_trip_setup(tag: &str) -> (crate::testutil::TempDir, overlay::Db, AppConfig, PathBuf) {
        let base = crate::testutil::temp_dir(tag);
        std::fs::create_dir_all(base.join("ac/content/cars")).unwrap();
        std::fs::create_dir_all(base.join("lib")).unwrap();
        let db = overlay::Db(std::sync::Mutex::new(
            overlay::open(&base.join("overlay.sqlite")).unwrap(),
        ));
        let cfg = AppConfig {
            ac_install_path: Some(base.join("ac")),
            library_path: Some(base.join("lib")),
            ..Default::default()
        };
        let src = base.join("src");
        car(&src.join("lanzo"));
        (base, db, cfg, src)
    }

    /// Rule (ESPACE§7.3, ESPACE§9.4): import, showcase, import the same archive
    /// again — the files come back into the **same version**, byte for byte,
    /// the user's entries untouched, and the mod is not laid in the game.
    #[test]
    fn the_same_archive_brings_the_files_back_into_the_same_version() {
        let (_base, db, cfg, src) = round_trip_setup("showcase-roundtrip");
        assert_eq!(import(&db, &cfg, &src)[0].outcome, "IMPORT");
        {
            let conn = db.0.lock().unwrap();
            overlay::set_mod_field(&conn, "lanzo", "display_name_user", Some("My Lanzo")).unwrap();
            overlay::set_favorite(&conn, "lanzo", true).unwrap();
            to_showcase(&conn, &cfg, "lanzo", None, true).unwrap();
        }

        let back = import(&db, &cfg, &src);
        assert_eq!(back[0].outcome, "REHYDRATED", "recognized, not a duplicate");
        assert_eq!(back[0].missing_files, 0, "everything came back");

        let conn = db.0.lock().unwrap();
        let versions = overlay::get_versions(&conn, "lanzo").unwrap();
        assert_eq!(versions.len(), 1, "the same version, not a new one");
        let v = &versions[0];
        assert!(!v.is_skeleton(), "complete again");
        assert_eq!(v.freed_at, None);
        let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path).unwrap();
        assert_eq!(files_of(&dir), files_of(&src.join("lanzo")), "the same files");
        for rel in files_of(&dir) {
            assert_eq!(
                std::fs::read(dir.join(&rel)).unwrap(),
                std::fs::read(src.join("lanzo").join(&rel)).unwrap(),
                "{rel} identical byte for byte"
            );
        }
        let m = overlay::get_mod(&conn, "lanzo").unwrap().unwrap();
        assert_eq!(m.display_name_user.as_deref(), Some("My Lanzo"), "entries untouched");
        assert!(m.is_favorite);
        assert!(!skeleton::is_showcase(&conn, "lanzo").unwrap());
        assert!(
            !crate::activation::is_mod_active(&cfg, ModKind::Car, "lanzo"),
            "not laid in the game on its own"
        );
        assert_eq!(overlay::get_history(&conn, "lanzo").unwrap()[0].event, "REHYDRATED");
        assert!(
            !dir.parent()
                .unwrap()
                .read_dir()
                .unwrap()
                .flatten()
                .any(|e| e.path() != dir),
            "no working folder left next to the version"
        );
        crate::activation::activate(&conn, &cfg, "lanzo", None).expect("and it can be activated again");
    }

    /// Rule (ESPACE§7.3): another version of a mod in the showcase is an
    /// update — never a layer on the skeleton —, the skeleton staying in the
    /// timeline.
    #[test]
    fn another_version_updates_and_the_skeleton_stays_in_the_timeline() {
        let (_base, db, cfg, src) = round_trip_setup("showcase-update");
        import(&db, &cfg, &src);
        to_showcase(&db.0.lock().unwrap(), &cfg, "lanzo", None, true).unwrap();

        write(&src.join("lanzo"), "lanzo.kn5", &[9; 6000]);
        let out = import(&db, &cfg, &src);
        assert_eq!(out[0].outcome, "UPDATE_REPLACE", "an update");

        let conn = db.0.lock().unwrap();
        let versions = overlay::get_versions(&conn, "lanzo").unwrap();
        assert_eq!(versions.len(), 2, "the skeleton stays in the timeline");
        assert_eq!(versions.iter().filter(|v| v.is_skeleton()).count(), 1);
        assert!(
            !skeleton::is_showcase(&conn, "lanzo").unwrap(),
            "the new version is the active one"
        );
        assert!(
            overlay::list_layers(&conn, "lanzo", crate::layers::HostKind::Car)
                .unwrap()
                .is_empty(),
            "no layer on the skeleton"
        );
    }
}
