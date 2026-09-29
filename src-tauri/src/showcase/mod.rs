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

// --- What the screens are told ---------------------------------------------------

/// One mod as the delete confirmation presents it (ESPACE§5.2): what it
/// weighs, what goes with it, and where it could come back from.
#[derive(Debug, Clone, Serialize)]
pub struct PlanEntry {
    pub id: String,
    pub name: String,
    /// Already in the showcase: a complete deletion is all there is left.
    pub showcase: bool,
    /// In the game now: it will be taken out first.
    pub active: bool,
    /// Complete versions, which all lose their files together (ESPACE§5.4).
    pub versions: usize,
    /// What those versions weigh — what the showcase gives back.
    pub size_bytes: u64,
    /// A source archive kept at import is still there: the mod recovers in
    /// one click, offline, if it is kept (ESPACE§5.2).
    pub kept_archive: bool,
    /// The archive's original name, and the site it came from, when known.
    pub source_file_name: Option<String>,
    pub source_site: Option<String>,
}

fn kept_archive_exists(cfg: &AppConfig, v: &VersionRow) -> bool {
    v.kept_archive_path
        .as_deref()
        .and_then(|p| crate::libpath::resolve(cfg.library_path.as_deref(), p))
        .is_some_and(|p| p.exists())
}

/// What the confirmation says about each mod, read before anything is done.
/// A version whose size was never recorded (imported before sizes were) is
/// measured now: "550 MB freed" must not read "0 B".
pub fn plan(conn: &Connection, cfg: &AppConfig, ids: &[String]) -> Result<Vec<PlanEntry>, String> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else {
            continue;
        };
        let kind = ModKind::from_kind(&m.kind).unwrap_or(ModKind::Car);
        let versions = overlay::get_versions(conn, id).map_err(|e| e.to_string())?;
        let full: Vec<&VersionRow> = versions.iter().filter(|v| !v.is_skeleton()).collect();
        let size_bytes = full
            .iter()
            .map(|v| match v.size_bytes {
                Some(n) => n.max(0) as u64,
                None => crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path)
                    .map_or(0, |d| crate::inspect::dir_size_bytes(&d)),
            })
            .sum();
        let active = versions.iter().find(|v| Some(&v.id) == m.active_version_id.as_ref());
        out.push(PlanEntry {
            id: id.clone(),
            name: m.display_name.clone().unwrap_or_else(|| id.clone()),
            showcase: m.showcase,
            active: crate::activation::is_mod_active(cfg, kind, id),
            versions: full.len(),
            size_bytes,
            kept_archive: versions.iter().any(|v| kept_archive_exists(cfg, v)),
            source_file_name: active.and_then(|v| v.source_file_name.clone().or_else(|| v.source_archive.clone())),
            source_site: active.and_then(|v| v.source_site.clone()),
        });
    }
    Ok(out)
}

/// Where the files of a mod in the showcase could come back from (ESPACE§7.1).
/// The registry of Content Manager is asked by the screen itself, through the
/// update check it already has.
#[derive(Debug, Clone, Serialize)]
pub struct Sources {
    /// The source archive kept at import is still in the library.
    pub kept_archive: bool,
    /// The address of the pack the mod came in (`mods.source_url`).
    pub page_url: Option<String>,
    /// The author's page, from the `ui_*.json` the skeleton kept.
    pub author_url: Option<String>,
    /// The site the archive was downloaded from (ESPACE§8).
    pub source_site: Option<String>,
    /// The archive's original name — the best search key there is.
    pub file_name: Option<String>,
}

/// Only an address a browser can open, never a `file:` or a script: the
/// `url` of a `ui_*.json` is whatever the author typed.
fn web_address(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string())
        .filter(|s| s.starts_with("https://") || s.starts_with("http://"))
}

pub fn sources(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<Sources, String> {
    let m = overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = ModKind::from_kind(&m.kind).unwrap_or(ModKind::Car);
    let versions = overlay::get_versions(conn, id).map_err(|e| e.to_string())?;
    let active = versions.iter().find(|v| Some(&v.id) == m.active_version_id.as_ref());
    let author_url = active
        .and_then(|v| crate::libpath::resolve(cfg.library_path.as_deref(), &v.library_path))
        .and_then(|dir| match kind {
            ModKind::Car => crate::uijson::read_car(&dir),
            ModKind::Track => crate::uijson::read_track(&dir),
        })
        .and_then(|ui| web_address(ui.url));
    Ok(Sources {
        kept_archive: active.is_some_and(|v| kept_archive_exists(cfg, v)),
        page_url: web_address(m.source_url.clone()),
        author_url,
        source_site: active.and_then(|v| v.source_site.clone()),
        file_name: active.and_then(|v| v.source_file_name.clone().or_else(|| v.source_archive.clone())),
    })
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
    source: crate::importer::ArchiveSource<'_>,
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
    if let (Some(kept), false) = (source.kept, still_kept) {
        overlay::set_kept_archive(conn, &v.id, kept).map_err(|e| e.to_string())?;
    }
    // Where it came back from, when the version did not know where it came
    // from in the first place (ESPACE§8); a known origin is never replaced.
    if let Some(origin) = source.origin {
        overlay::set_version_origin(conn, &v.id, origin).map_err(|e| e.to_string())?;
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
mod tests;
