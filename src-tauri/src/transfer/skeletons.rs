//! The skeletons an export carries (EXPORT§5.1): for every folder the library
//! holds, what the showcase keeps of it (ESPACE§3.1) and the manifest of what
//! it does not — written for a complete mod too, so that at arrival every one
//! knows exactly what it lacks. The library itself is only read.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::files::{files_in, zip_name, Entry, LIBRARY_PREFIX};
use super::Part;
use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::skeleton::{self, Manifest};

/// The skeletons of a library, read from the export's copy of the base
/// **before** its rows are forced into the showcase: a row's state says
/// whether its folder is a skeleton already or must be made one.
pub(super) struct Skeletons<'a> {
    pub conn: &'a Connection,
    pub cfg: &'a AppConfig,
    pub library: &'a Path,
    pub scratch: &'a Path,
    pub stamp: &'a str,
    /// The previews the cards show ([`super::settings::preferred_previews`]), by mod id.
    pub preferred: &'a std::collections::HashMap<String, PathBuf>,
}

/// One folder of the library on its way into the zip.
struct Folder {
    /// Relative to the library, `/`-separated: the zip's name for it.
    rel: String,
    dir: PathBuf,
    /// Already a skeleton: it leaves as it is.
    freed: bool,
}

impl Skeletons<'_> {
    /// Every folder the library holds: versions of managed mods, layers,
    /// attached skins and sounds, apps and "other" mods.
    pub fn of_library(&self) -> Result<Vec<Entry>, String> {
        let mut out = Vec::new();
        self.versions(&mut out).map_err(|e| e.to_string())?;
        self.layers(&mut out).map_err(|e| e.to_string())?;
        self.manifest_only(
            "SELECT id, parent_id, library_path, content_state, source_archive
             FROM sub_mods WHERE removable = 1",
            &mut out,
        )
        .map_err(|e| e.to_string())?;
        for table in ["apps", "other_mods"] {
            self.manifest_only(
                &format!("SELECT id, id, library_path, content_state, source_archive FROM {table}"),
                &mut out,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(out)
    }

    fn folder(&self, stored: &str, state: &str) -> Option<Folder> {
        let rel = super::tables::relative(self.library, stored)?;
        Some(Folder {
            dir: self.library.join(&rel),
            rel,
            freed: state == crate::overlay::CONTENT_SKELETON,
        })
    }

    fn versions(&self, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT v.id, v.mod_id, v.library_path, v.content_state, v.version_label,
                    v.content_signature, v.source_archive, v.source_site, v.source_file_name,
                    m.kind, COALESCE(m.active_version_id = v.id, 0)
             FROM versions v JOIN mods m ON m.id_interne = v.mod_id
             WHERE m.is_stock = 0",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                Manifest {
                    version_label: r.get(4)?,
                    content_signature: r.get(5)?,
                    source_archive: r.get(6)?,
                    source_site: r.get(7)?,
                    source_file_name: r.get(8)?,
                    ..Manifest::new(self.stamp.to_string(), String::new(), Vec::new())
                },
                r.get::<_, String>(9)?,
                r.get::<_, bool>(10)?,
            ))
        })?;
        for row in rows {
            let (mod_id, stored, state, labels, kind, active) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            let kind = ModKind::from_column(&kind);
            let image = if active {
                // The image the card shows (ESPACE§3.3): the preferred livery
                // or layout the screens chose, the backend's own pick else.
                match self.preferred.get(&mod_id) {
                    Some(preview) => Some(preview.clone()),
                    None => crate::overlay::get_mod(self.conn, &mod_id)?
                        .and_then(|m| crate::library::preview_for(self.conn, self.cfg, &m))
                        .map(PathBuf::from),
                }
            } else {
                crate::showcase::own_preview(kind, &folder.dir)
            };
            let manifest = Manifest { mod_id, ..labels };
            self.skeleton_of(&folder, Some(kind), manifest, image, out);
        }
        Ok(())
    }

    fn layers(&self, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT parent_id, parent_kind, library_path, content_state, source_archive FROM layers")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (parent, parent_kind, stored, state, archive) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            // A layer keeps its host's skeleton (ESPACE§5.4); an app's layer
            // keeps nothing.
            let kind = match parent_kind.as_str() {
                "Car" => Some(ModKind::Car),
                "Track" => Some(ModKind::Track),
                _ => None,
            };
            let mut manifest = Manifest::new(self.stamp.to_string(), parent, Vec::new());
            manifest.source_archive = archive;
            self.skeleton_of(&folder, kind, manifest, None, out);
        }
        Ok(())
    }

    /// Rows that keep no skeleton (ESPACE§5.4, ESPACE§5.6): `sql` selects an id,
    /// the id the manifest names, the folder, the state and the archive.
    fn manifest_only(&self, sql: &str, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (owner, stored, state, archive) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            let mut manifest = Manifest::new(self.stamp.to_string(), owner, Vec::new());
            manifest.source_archive = archive;
            self.skeleton_of(&folder, None, manifest, None, out);
        }
        Ok(())
    }

    /// A folder already in the showcase: its skeleton, as it is.
    fn as_is(&self, folder: &Folder, out: &mut Vec<Entry>) {
        for (rel, path) in files_in(&folder.dir) {
            out.push(Entry::file(
                Part::Library,
                zip_name(LIBRARY_PREFIX, &folder.rel, &rel),
                path,
            ));
        }
    }

    /// A complete folder made a skeleton in the zip only — the library is
    /// never touched: what `kind`'s whitelist keeps (nothing without one), the
    /// manifest of the rest, and the frozen image when there is one.
    fn skeleton_of(
        &self,
        folder: &Folder,
        kind: Option<ModKind>,
        manifest: Manifest,
        image: Option<PathBuf>,
        out: &mut Vec<Entry>,
    ) {
        let removed = match kind {
            Some(k) => skeleton::removable_files(k, &folder.dir),
            None => skeleton::all_files(&folder.dir),
        };
        let manifest = Manifest {
            removed_bytes: removed.iter().map(|f| f.size).sum(),
            removed,
            ..manifest
        };
        if let Some(kind) = kind {
            for (rel, path) in files_in(&folder.dir) {
                // A stale manifest or image of a former showcase is not this
                // export's: the ones written below are.
                if skeleton::is_showcase_file(&rel) || !skeleton::is_kept(kind, &rel) {
                    continue;
                }
                let name = zip_name(LIBRARY_PREFIX, &folder.rel, &rel);
                out.push(if skeleton::is_reduced(kind, &rel) {
                    self.reduced(name, &path)
                } else {
                    Entry::file(Part::Library, name, path)
                });
            }
        }
        match serde_json::to_vec_pretty(&manifest) {
            Ok(json) => out.push(Entry::bytes(
                Part::Library,
                zip_name(LIBRARY_PREFIX, &folder.rel, Path::new(skeleton::MANIFEST_NAME)),
                json,
            )),
            Err(e) => log::warn!("transfer: manifest of {} not written: {e}", folder.rel),
        }
        if let Some(src) = image {
            match self.frozen(&src) {
                Ok(frozen) => {
                    let file = frozen.file_name().map(PathBuf::from).unwrap_or_default();
                    out.push(Entry::file(
                        Part::Library,
                        zip_name(LIBRARY_PREFIX, &folder.rel, &file),
                        frozen,
                    ));
                }
                Err(e) => log::warn!("transfer: card image of {} not frozen: {e}", folder.rel),
            }
        }
    }

    /// A track preview, reduced in a scratch copy (ESPACE§3.3).
    fn reduced(&self, name: String, path: &Path) -> Entry {
        let copy = self.scratch_file(path.file_name().map(PathBuf::from).unwrap_or_default());
        let made = std::fs::copy(path, &copy)
            .map_err(|e| e.to_string())
            .and_then(|_| skeleton::reduce_in_place(&copy));
        match made {
            Ok(_) => Entry::file(Part::Library, name, copy),
            Err(e) => {
                log::warn!("transfer: {} not reduced, exported as is: {e}", path.display());
                Entry::file(Part::Library, name, path.to_path_buf())
            }
        }
    }

    fn frozen(&self, src: &Path) -> Result<PathBuf, String> {
        let dir = self.scratch_file(PathBuf::new());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        skeleton::freeze_image(src, &dir)
    }

    /// A fresh place in the scratch folder: every reduced or frozen image
    /// has its own, so two of the same name never overwrite each other.
    fn scratch_file(&self, name: PathBuf) -> PathBuf {
        let dir = self.scratch.join(uuid::Uuid::new_v4().to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("transfer: scratch {} not created: {e}", dir.display());
        }
        dir.join(name)
    }

    /// Mods installed outside Pit Box leave **managed** (EXPORT§4.2): a
    /// version in the showcase is made of their folder in `content/` — its
    /// skeleton, a signature of its real files — under the library path the
    /// import would give them. At arrival they are mods like the others, and
    /// importing their archive recovers them. Returns how many, and their
    /// skeletons. Their game folder is only read.
    pub fn convert_unmanaged(&self) -> Result<(usize, Vec<Entry>), String> {
        let rows: Vec<(String, String, String, String)> = {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT m.id_interne, m.kind, v.id, v.library_path
                     FROM mods m JOIN versions v ON v.id = m.active_version_id
                     WHERE m.is_stock = 1 AND m.is_unmanaged = 1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(|e| e.to_string())?;
            rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
        };
        let mut out = Vec::new();
        let mut converted = 0;
        for (id, kind_col, version_id, content_dir) in rows {
            let dir = PathBuf::from(&content_dir);
            if !dir.is_dir() {
                log::warn!("transfer: unmanaged {id} has no folder at {content_dir}, left as game content");
                continue;
            }
            let kind = ModKind::from_column(&kind_col);
            let rel = format!("{}/{id}/export", kind.content_folder());
            let signature = crate::identity::content_signature(&dir);
            let size = crate::inspect::dir_size_bytes(&dir) as i64;
            let mut manifest = Manifest::new(self.stamp.to_string(), id.clone(), Vec::new());
            manifest.content_signature = Some(signature.clone());
            let image = crate::showcase::own_preview(kind, &dir);
            let folder = Folder {
                rel: rel.clone(),
                dir,
                freed: false,
            };
            self.skeleton_of(&folder, Some(kind), manifest, image, &mut out);
            self.conn
                .execute(
                    "UPDATE versions SET library_path = ?2, content_signature = ?3, size_bytes = ?4 WHERE id = ?1",
                    rusqlite::params![version_id, rel, signature, size],
                )
                .map_err(|e| e.to_string())?;
            self.conn
                .execute(
                    "UPDATE mods SET is_stock = 0, is_unmanaged = 0 WHERE id_interne = ?1",
                    [&id],
                )
                .map_err(|e| e.to_string())?;
            converted += 1;
        }
        Ok((converted, out))
    }
}
