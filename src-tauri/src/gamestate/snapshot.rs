//! What the database says Pit Box laid in the game (DOSSIER§5.1), read in one
//! go under the lock and then released: the disk walk and the classification
//! that follow take seconds, and must not hold every other command waiting.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::{self, ExtraLinkRow};

#[derive(Debug, Clone)]
pub struct ModInfo {
    pub kind: ModKind,
    pub name: String,
    pub is_stock: bool,
    pub is_unmanaged: bool,
    pub version_label: Option<String>,
    /// Names of the active layers, highest priority last.
    pub layers: Vec<String>,
    /// Deployed, as the engine itself decides it (`activation::is_mod_active`).
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    /// Deployed, as `apps::is_app_active` decides it.
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct SubInfo {
    pub id: String,
    pub sub_type: String,
    pub parent_id: String,
    /// Folder name - the name of the junction in `skins/`.
    pub folder: String,
    pub name: String,
    pub is_active: bool,
    /// A sound: where the car's original `sfx/` is kept (`submods`).
    pub sound_backup: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct OtherInfo {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    /// Absolute paths of the links laid at its last activation.
    pub links: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub ac: PathBuf,
    pub library: Option<PathBuf>,
    pub mods: HashMap<String, ModInfo>,
    pub apps: HashMap<String, AppInfo>,
    /// Packs with at least one member deployed: their game additions are laid
    /// (`extras::sync_pack`).
    pub active_packs: HashSet<String>,
    pub subs: Vec<SubInfo>,
    pub others: Vec<OtherInfo>,
    pub extra_links: Vec<ExtraLinkRow>,
    /// `(ac_path, backup_path)`.
    pub backups: Vec<(String, String)>,
    /// `(mod_id, ac_path)`, the path lowercase with backslashes.
    pub forced: HashSet<(String, String)>,
    /// What `compose::recompose` would lay down for each host deployed from the
    /// library (`compose::planned_sources`): base, then active layers.
    pub plans: HashMap<String, (PathBuf, Vec<PathBuf>)>,
    /// The game-write generation this snapshot was taken at (DOSSIER§5.4).
    pub generation: u64,
}

impl Snapshot {
    pub fn load(conn: &Connection, cfg: &AppConfig, generation: u64) -> Result<Snapshot, String> {
        let ac = cfg.ac_install_path.clone().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
        let err = |e: rusqlite::Error| e.to_string();

        let mut mods = HashMap::new();
        let mut plans = HashMap::new();
        let mut active_packs = HashSet::new();
        for m in overlay::list_mods(conn).map_err(err)? {
            let Some(kind) = ModKind::from_kind(&m.kind) else {
                continue;
            };
            let layers = overlay::active_layers(conn, &m.id_interne, kind.into())
                .map_err(err)?
                .into_iter()
                .map(|l| l.display_name_user.or(l.source_archive).unwrap_or(l.name))
                .collect();
            if let Some(plan) = crate::compose::planned_sources(conn, cfg, &m.id_interne) {
                plans.insert(m.id_interne.clone(), plan);
            }
            let active = crate::activation::is_mod_active(cfg, kind, &m.id_interne);
            if active {
                active_packs.extend(m.source_pack.clone());
            }
            mods.insert(
                m.id_interne.clone(),
                ModInfo {
                    kind,
                    name: m.display_name.unwrap_or_else(|| m.id_interne.clone()),
                    is_stock: m.is_stock,
                    is_unmanaged: m.is_unmanaged,
                    version_label: m.active_version_label,
                    layers,
                    active,
                },
            );
        }

        let mut apps = HashMap::new();
        for a in overlay::list_apps(conn).map_err(err)? {
            if let Some(plan) = crate::compose::planned_sources(conn, cfg, &a.id) {
                plans.insert(a.id.clone(), plan);
            }
            apps.insert(
                a.id.clone(),
                AppInfo {
                    active: crate::apps::is_app_active(cfg, &a.id),
                    name: a.display_name_user.unwrap_or(a.id),
                },
            );
        }

        let subs = overlay::list_all_subs(conn)
            .map_err(err)?
            .into_iter()
            .map(|s| SubInfo {
                sound_backup: (s.sub_type == "SOUND")
                    .then(|| crate::submods::sound_backup_dir(cfg, &s.parent_id).ok())
                    .flatten(),
                name: s.display_name_user.unwrap_or_else(|| s.name.clone()),
                id: s.id,
                sub_type: s.sub_type,
                parent_id: s.parent_id,
                folder: s.name,
                is_active: s.is_active,
            })
            .collect();

        let others = overlay::list_other_mods(conn)
            .map_err(err)?
            .into_iter()
            .map(|o| OtherInfo {
                name: o.display_name_user.unwrap_or_else(|| o.id.clone()),
                id: o.id,
                is_active: o.is_active,
                links: o.junctions,
            })
            .collect();

        let forced = overlay::list_forced_extras(conn)
            .map_err(err)?
            .into_iter()
            .map(|(m, p)| (m, p.to_lowercase().replace('/', "\\")))
            .collect();

        Ok(Snapshot {
            ac,
            library: cfg.library_path.clone(),
            mods,
            apps,
            active_packs,
            subs,
            others,
            extra_links: overlay::list_all_extra_links(conn).map_err(err)?,
            backups: overlay::list_game_backups(conn).map_err(err)?,
            forced,
            plans,
            generation,
        })
    }
}
