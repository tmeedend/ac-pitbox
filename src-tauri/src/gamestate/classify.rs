//! The second phase of the scan (DOSSIER§4, DOSSIER§5.1): crossing the disk tree with
//! the database snapshot. Each step decides a **population** for what no
//! earlier step claimed — first match wins, in the order of DOSSIER§4.1 — and
//! the state that goes with it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::snapshot::Snapshot;
use super::tree::{EntryKind, NodeId, Tree};
use super::{
    ClaimInfo, Class, Drift, FolderInfo, Index, LibItem, Owner, OwnerId, OwnerKind, Population, Presence, State,
};
use crate::extras::OwnerKind as ExtraOwner;
use crate::modscan::ModKind;

struct Classifier<'a> {
    snap: &'a Snapshot,
    tree: Tree,
    class: Vec<Class>,
    owners: Vec<Owner>,
    owner_ids: HashMap<(OwnerKind, String), OwnerId>,
    sources: HashMap<NodeId, PathBuf>,
    claims: HashMap<NodeId, Vec<ClaimInfo>>,
    backups: HashMap<NodeId, PathBuf>,
    folders: HashMap<NodeId, FolderInfo>,
    /// Lowercase id → owner, for the ids read off folder names: the disk keeps
    /// the case the user or the archive gave, the database the case of import.
    mods_ci: HashMap<String, OwnerId>,
    apps_ci: HashMap<String, OwnerId>,
    /// `(sub type, lowercase parent, lowercase folder)` → livery owner.
    subs_ci: HashMap<(String, String, String), OwnerId>,
    /// Lowercase absolute path → the active other mod that laid a link there.
    other_links: HashMap<String, OwnerId>,
}

pub fn classify(tree: Tree, snap: &Snapshot) -> Index {
    let n = tree.len();
    let mut c = Classifier {
        snap,
        tree,
        class: vec![Class::default(); n],
        owners: Vec::new(),
        owner_ids: HashMap::new(),
        sources: HashMap::new(),
        claims: HashMap::new(),
        backups: HashMap::new(),
        folders: HashMap::new(),
        mods_ci: HashMap::new(),
        apps_ci: HashMap::new(),
        subs_ci: HashMap::new(),
        other_links: HashMap::new(),
    };
    c.register_owners();
    c.links();
    c.mod_folders();
    c.sounds();
    c.extras();
    c.game_backups();
    c.stored_copies();
    c.content_folders();
    c.cm_zones();
    let items = c.items();

    let mut idx = Index::unclassified(c.tree, snap.generation);
    idx.class = c.class;
    idx.owners = c.owners;
    idx.items = items;
    idx.sources = c.sources;
    idx.claims = c.claims;
    idx.backups = c.backups;
    idx.folders = c.folders;
    idx.classified = true;
    idx.counts = std::sync::Arc::new(idx.compute_counts(&super::Filters::default()));
    idx
}

/// The segments of a relative path, lowercase, for matching a layout
/// (`content/cars/<id>/skins/<name>`).
fn lower_parts(rel: &Path) -> Vec<String> {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// A path as a lookup key: lowercase, backslashes. What the database stores
/// was built by `Path::join`, which keeps whichever separator it was given.
fn key(p: &str) -> String {
    p.to_lowercase().replace('/', "\\")
}

fn starts_with_ci(path: &Path, prefix: &Path) -> bool {
    let p = key(&path.to_string_lossy());
    let pre = key(&prefix.to_string_lossy());
    p == pre || p.starts_with(&format!("{}\\", pre.trim_end_matches('\\')))
}

impl Classifier<'_> {
    // --- Owners -------------------------------------------------------------

    fn owner(&mut self, kind: OwnerKind, id: &str, name: &str, parent: Option<&str>) -> OwnerId {
        let key = (kind, id.to_string());
        if let Some(&o) = self.owner_ids.get(&key) {
            return o;
        }
        let o = self.owners.len() as OwnerId;
        self.owners.push(Owner {
            kind,
            id: id.to_string(),
            name: name.to_string(),
            parent: parent.map(str::to_string),
        });
        self.owner_ids.insert(key, o);
        o
    }

    fn known(&self, kind: OwnerKind, id: &str) -> Option<OwnerId> {
        self.owner_ids.get(&(kind, id.to_string())).copied()
    }

    /// A car or a track, whatever the case of `id`.
    fn mod_owner(&self, id: &str) -> Option<OwnerId> {
        self.mods_ci.get(&id.to_lowercase()).copied()
    }

    /// Every element of the snapshot becomes an owner up front: the search
    /// needs them all, laid in the game or not (DOSSIER§7.1).
    fn register_owners(&mut self) {
        let snap = self.snap;
        let mut mods: Vec<_> = snap.mods.iter().collect();
        mods.sort_by(|a, b| a.0.cmp(b.0));
        for (id, m) in mods {
            let kind = match m.kind {
                ModKind::Car => OwnerKind::Car,
                ModKind::Track => OwnerKind::Track,
            };
            let o = self.owner(kind, id, &m.name, None);
            self.mods_ci.insert(id.to_lowercase(), o);
        }
        let mut apps: Vec<_> = snap.apps.iter().collect();
        apps.sort_by(|a, b| a.0.cmp(b.0));
        for (id, app) in apps {
            let o = self.owner(OwnerKind::App, id, &app.name, None);
            self.apps_ci.insert(id.to_lowercase(), o);
        }
        for s in &snap.subs {
            let kind = match s.sub_type.as_str() {
                "SKIN" => OwnerKind::Skin,
                "TRACK_SKIN" => OwnerKind::TrackSkin,
                "SOUND" => OwnerKind::Sound,
                _ => continue,
            };
            let o = self.owner(kind, &s.id, &s.name, Some(&s.parent_id));
            self.subs_ci.insert(
                (s.sub_type.clone(), s.parent_id.to_lowercase(), s.folder.to_lowercase()),
                o,
            );
        }
        for other in &snap.others {
            let o = self.owner(OwnerKind::Other, &other.id, &other.name, None);
            if other.is_active {
                for l in &other.links {
                    self.other_links.insert(key(l), o);
                }
            }
        }
    }

    // --- Deciding -------------------------------------------------------------

    fn decided(&self, id: NodeId) -> bool {
        self.class[id as usize].decided
    }

    fn decide(
        &mut self,
        id: NodeId,
        population: Population,
        state: State,
        drift: Option<Drift>,
        owner: Option<OwnerId>,
    ) {
        let c = &mut self.class[id as usize];
        if c.decided {
            return;
        }
        c.decided = true;
        c.population = population;
        c.state = if drift.is_some() { State::Drift } else { state };
        c.drift = drift;
        c.owner = owner;
    }

    /// Decides every node of the subtree that no earlier step claimed.
    fn decide_subtree(
        &mut self,
        root: NodeId,
        population: Population,
        state: State,
        drift: Option<Drift>,
        owner: Option<OwnerId>,
    ) {
        for id in self.tree.subtree(root) {
            let leaf = self.tree.node(id).kind != EntryKind::Dir;
            let (st, dr) = if leaf { (state, drift) } else { (State::Nobody, None) };
            self.decide(id, population, st, dr, owner);
        }
    }

    /// Adds a path the database expects and the disk lacks.
    fn missing(&mut self, rel: &Path) -> Option<NodeId> {
        let id = self.tree.ensure_missing(rel)?;
        self.class.resize(self.tree.len(), Class::default());
        Some(id)
    }

    fn in_library(&self, p: &Path) -> bool {
        self.snap.library.as_deref().is_some_and(|lib| starts_with_ci(p, lib))
    }

    // --- 1. Links (DOSSIER§4.1, population 1) -------------------------------

    /// Who laid the link at `rel`, from where it sits: the layouts Pit Box
    /// uses (`submods::skins::project_skin`, `apps::app_link`, the legacy mod
    /// junction), then the links an other mod recorded at its activation.
    fn link_owner(&self, rel: &Path, abs: &Path) -> Option<OwnerId> {
        let parts = lower_parts(rel);
        let p: Vec<&str> = parts.iter().map(String::as_str).collect();
        let sub = |ty: &str, parent: &str, folder: &str| {
            self.subs_ci
                .get(&(ty.to_string(), parent.to_string(), folder.to_string()))
                .copied()
        };
        let found = match p.as_slice() {
            ["content", "cars" | "tracks", id] => self.mod_owner(id),
            ["content", "cars", car, "skins", folder] => sub("SKIN", car, folder),
            ["content", "tracks", track, "skins", "cm_skins", folder] => sub("TRACK_SKIN", track, folder),
            ["apps", "python" | "lua", id] => self.apps_ci.get(*id).copied(),
            _ => None,
        };
        found.or_else(|| self.other_links.get(&key(&abs.to_string_lossy())).copied())
    }

    fn links(&mut self) {
        let links: Vec<(NodeId, Option<PathBuf>)> = (0..self.tree.len() as NodeId)
            .filter(|&id| self.tree.node(id).kind == EntryKind::Link)
            .map(|id| (id, self.tree.link_targets.get(&id).cloned()))
            .collect();
        for (id, target) in links {
            let rel = self.tree.rel_path(id);
            let abs = self.tree.root.join(&rel);
            // A target that cannot be read, or is gone: the link is broken.
            let broken = target.as_ref().is_none_or(|t| std::fs::metadata(t).is_err());
            let owner = self.link_owner(&rel, &abs);
            let ours = owner.is_some() || target.as_ref().is_some_and(|t| self.in_library(t));
            if !ours {
                // Someone else's link: left to the later steps, like any entry.
                continue;
            }
            let drift = if broken {
                Some(Drift::BrokenLink)
            } else if owner.is_none() {
                // Points into the library, but nothing projects it any more.
                Some(Drift::Orphan)
            } else {
                None
            };
            self.decide(id, Population::Link, State::Posed, drift, owner);
            if let Some(t) = target {
                self.sources.insert(id, t);
            }
        }
    }

    // --- 2. Deployed mod folders (population 2) -----------------------------

    fn mod_folders(&mut self) {
        let marked: Vec<NodeId> = (0..self.tree.len() as NodeId)
            .filter(|&id| {
                self.tree.node(id).kind == EntryKind::Dir
                    && !self.decided(id)
                    && self.tree.child(id, crate::deploy::MARKER_FILE).is_some()
            })
            .collect();
        for dir in marked {
            let abs = self.tree.abs_path(dir);
            let marker = crate::deploy::read_marker(&abs);
            let owner = marker.as_ref().and_then(|(id, kind)| {
                if kind.eq_ignore_ascii_case("app") {
                    self.known(OwnerKind::App, id)
                } else {
                    self.mod_owner(id)
                }
            });
            let Some(owner) = owner else {
                // A marker whose mod is gone (app killed mid-way, database
                // restored): nothing of Pit Box claims this folder any more.
                self.decide_subtree(dir, Population::ModFolder, State::Posed, Some(Drift::Orphan), None);
                continue;
            };
            let host = marker.map(|(id, _)| id).unwrap_or_default();
            let info = self.snap.mods.get(&host);
            self.folders.insert(
                dir,
                FolderInfo {
                    owner,
                    version: info.and_then(|m| m.version_label.clone()),
                    layers: info.map(|m| m.layers.clone()).unwrap_or_default(),
                },
            );
            self.laid_folder(dir, owner, self.snap.plans.get(&host));
        }
    }

    /// A deployed folder, measured against its plan: each file is laid and
    /// identical, laid and changed, or not in the plan at all; each planned
    /// file the disk lacks is missing.
    fn laid_folder(&mut self, dir: NodeId, owner: OwnerId, plan: Option<&(PathBuf, Vec<PathBuf>)>) {
        // Keyed by lowercase path: the disk and the library need not agree on
        // case, and a lookup per file keeps a 2 000-file car linear.
        let mut plan: HashMap<String, (PathBuf, PathBuf)> = plan
            .map(|(base, layers)| crate::deploy::planned_files(base, layers))
            .unwrap_or_default()
            .into_iter()
            .map(|(inner, src)| (inner.to_string_lossy().to_lowercase(), (inner, src)))
            .collect();
        let dir_rel = self.tree.rel_path(dir);
        for id in self.tree.subtree(dir) {
            if self.decided(id) {
                continue;
            }
            if self.tree.node(id).kind == EntryKind::Dir {
                self.decide(id, Population::ModFolder, State::Nobody, None, Some(owner));
                continue;
            }
            let rel = self.tree.rel_path(id);
            let inner = rel.strip_prefix(&dir_rel).unwrap_or(&rel).to_path_buf();
            if inner.as_os_str().eq_ignore_ascii_case(crate::deploy::MARKER_FILE) {
                self.decide(id, Population::ModFolder, State::Posed, None, Some(owner));
                continue;
            }
            match plan.remove(&inner.to_string_lossy().to_lowercase()) {
                Some((_, src)) => {
                    let drift =
                        (!crate::extras::is_still_ours(&src, &self.tree.root.join(&rel))).then_some(Drift::Modified);
                    self.decide(id, Population::ModFolder, State::Posed, drift, Some(owner));
                    self.sources.insert(id, src);
                }
                // Added after the deployment - a showroom preview CM
                // regenerated, typically. Nobody's, in a laid folder: making it
                // a drift would flag every preview (DOSSIER§4.3).
                None => self.decide(id, Population::ModFolder, State::Nobody, None, Some(owner)),
            }
        }
        let mut rest: Vec<_> = plan.into_values().collect();
        rest.sort();
        for (inner, src) in rest {
            if let Some(id) = self.missing(&dir_rel.join(&inner)) {
                self.decide(
                    id,
                    Population::ModFolder,
                    State::Posed,
                    Some(Drift::Missing),
                    Some(owner),
                );
                self.sources.insert(id, src);
            }
        }
    }

    // --- 3. Replaced car sounds (population 3) ------------------------------

    fn sounds(&mut self) {
        let snap = self.snap;
        for s in snap.subs.iter().filter(|s| s.sub_type == "SOUND" && s.is_active) {
            let Some(sfx) = self
                .tree
                .find(&Path::new("content").join("cars").join(&s.parent_id).join("sfx"))
            else {
                continue;
            };
            if self.decided(sfx) {
                // Inside a deployed mod folder: the sound replaced a mod file,
                // not a game file, and the folder's plan already covers it.
                continue;
            }
            let owner = self.known(OwnerKind::Sound, &s.id);
            self.decide_subtree(sfx, Population::Sound, State::ReplacesGame, None, owner);
            if let Some(b) = &s.sound_backup {
                self.backups.insert(sfx, b.clone());
            }
        }
    }

    // --- 4. Game additions (population 4) ------------------------------------

    fn extra_owner(&mut self, kind: &str, id: &str) -> Option<OwnerId> {
        match ExtraOwner::parse(kind)? {
            ExtraOwner::Car | ExtraOwner::Track => self.mod_owner(id),
            ExtraOwner::App => self.known(OwnerKind::App, id),
            // A pack is not an entity of its own in the database: it becomes
            // an owner the first time one of its additions is met.
            ExtraOwner::Pack => Some(self.owner(OwnerKind::Pack, id, id, None)),
        }
    }

    fn extras(&mut self) {
        let snap = self.snap;
        let mut by_path: HashMap<String, Vec<&crate::overlay::ExtraLinkRow>> = HashMap::new();
        for row in snap.extra_links.iter().filter(|r| !r.is_dir) {
            by_path.entry(key(&row.ac_path)).or_default().push(row);
        }
        let mut paths: Vec<_> = by_path.into_iter().collect();
        paths.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, rows) in paths {
            let abs = PathBuf::from(&rows[0].ac_path);
            let Some(rel) = self.tree.relative(&abs) else { continue };
            let mut claims = Vec::new();
            for row in &rows {
                let (Some(owner), Some(kind), Some(library)) = (
                    self.extra_owner(&row.kind, &row.mod_id),
                    ExtraOwner::parse(&row.kind),
                    snap.library.as_deref(),
                ) else {
                    continue;
                };
                claims.push(ClaimInfo {
                    owner,
                    provided: row.provided,
                    forced: snap.forced.contains(&(row.mod_id.clone(), key(&row.ac_path))),
                    copy: crate::extras::library_copy(library, kind, &row.mod_id, &rel),
                });
            }
            if claims.is_empty() {
                continue;
            }
            let provider = claims.iter().find(|c| c.provided);
            let owner = provider.or(claims.first()).map(|c| c.owner);
            let node = self
                .tree
                .find(&rel)
                .filter(|&id| self.tree.node(id).kind != EntryKind::Dir && self.tree.node(id).present);
            // Already decided - an other mod's file link on the same path: the
            // claims are attached, the rest stays what the link step said.
            let taken = node.is_some_and(|id| self.decided(id));
            let id = match node {
                Some(id) => {
                    let abs = self.tree.root.join(&rel);
                    let (state, drift) = match provider {
                        Some(p) if crate::extras::is_still_ours(&p.copy, &abs) => (State::Posed, None),
                        Some(_) => (State::Posed, Some(Drift::Modified)),
                        // Claimed, provided by nobody, and yet occupied: a
                        // foreign file holds the place (§4.5.4).
                        None => (State::Waiting, None),
                    };
                    self.decide(id, Population::Extra, state, drift, owner);
                    id
                }
                None => match self.missing(&rel) {
                    Some(id) => {
                        self.decide(id, Population::Extra, State::Posed, Some(Drift::Missing), owner);
                        id
                    }
                    None => continue,
                },
            };
            if let Some(p) = provider.filter(|_| !taken) {
                self.sources.insert(id, p.copy.clone());
            }
            self.class[id as usize].claimants = claims.len().min(u16::MAX as usize) as u16;
            self.claims.insert(id, claims);
        }
    }

    // --- Replaced game files (DOSSIER§4.2, "replaces the game") -------------

    /// "Replaces the game" wins over "laid" (DOSSIER§4.2): it is the one state
    /// that touches every session. A drift keeps its drift - it is graver.
    fn game_backups(&mut self) {
        for (ac_path, backup) in &self.snap.backups {
            let Some(id) = self.tree.find_abs(Path::new(ac_path)) else {
                continue;
            };
            if !self.tree.node(id).present || self.tree.node(id).kind == EntryKind::Dir {
                continue;
            }
            self.decide(id, Population::Rest, State::ReplacesGame, None, None);
            let c = &mut self.class[id as usize];
            if c.state != State::Drift {
                c.state = State::ReplacesGame;
            }
            self.backups.insert(id, PathBuf::from(backup));
        }
    }

    // --- Stored copies nobody claims (DOSSIER§4.2 "waiting", DOSSIER§4.3 "orphan") --

    /// Is the owner of a stored additions tree deployed? The engine's own
    /// answer, taken in the snapshot.
    fn owner_active(&self, kind: ExtraOwner, id: &str) -> bool {
        match kind {
            ExtraOwner::Car | ExtraOwner::Track => self.snap.mods.get(id).is_some_and(|m| m.active),
            ExtraOwner::App => self.snap.apps.get(id).is_some_and(|a| a.active),
            ExtraOwner::Pack => self.snap.active_packs.contains(id),
        }
    }

    /// The library copies no claim covers, crossed with the disk. Two cases,
    /// the ones `extras::list` already tells apart on a mod's sheet:
    ///
    /// - the owner is deployed and a file occupies the path: its copy **waits**.
    ///   A foreign file holds the place and won the date arbitration (§4.5.4),
    ///   so the deployment recorded no claim;
    /// - the owner is not deployed and the file on the disk is identical to its
    ///   copy (relative path, size, date): laid by Pit Box, then left behind -
    ///   an **orphan**.
    fn stored_copies(&mut self) {
        let Some(library) = self.snap.library.clone() else {
            return;
        };
        for (kind, owner_id) in crate::extras::stored_owners(&library) {
            let active = self.owner_active(kind, &owner_id);
            let root = crate::extras::library_copy(&library, kind, &owner_id, Path::new(""));
            for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Ok(rel) = entry.path().strip_prefix(&root) else {
                    continue;
                };
                if !crate::acpath::is_ac_relative(rel) {
                    continue;
                }
                let Some(id) = self.tree.find(rel) else { continue };
                if self.decided(id) || self.tree.node(id).kind != EntryKind::File {
                    continue;
                }
                let owner = self.extra_owner(kind.category(), &owner_id);
                if active {
                    self.decide(id, Population::Extra, State::Waiting, None, owner);
                    // Its copy is the claim that waits: the panel says so.
                    if let Some(owner) = owner {
                        self.class[id as usize].claimants = 1;
                        self.claims.entry(id).or_default().push(ClaimInfo {
                            owner,
                            provided: false,
                            forced: false,
                            copy: entry.path().to_path_buf(),
                        });
                    }
                } else if crate::extras::is_still_ours(entry.path(), &self.tree.root.join(rel)) {
                    self.decide(id, Population::Rest, State::Posed, Some(Drift::Orphan), owner);
                    self.sources.insert(id, entry.path().to_path_buf());
                }
            }
        }
    }

    // --- 5 and 6. Official content, unmanaged mods ---------------------------

    fn content_folders(&mut self) {
        for kind in [ModKind::Car, ModKind::Track] {
            let Some(folder) = self.tree.find(&Path::new("content").join(kind.content_folder())) else {
                continue;
            };
            for child in self.tree.node(folder).children.clone() {
                if self.decided(child) {
                    continue;
                }
                let name = self.tree.name(child).to_string();
                let owner = self.mod_owner(&name);
                let entity = owner.and_then(|o| self.snap.mods.get(&self.owners[o as usize].id));
                let official = crate::kunos_dates::is_official(kind, &name)
                    || entity.is_some_and(|m| m.is_stock && !m.is_unmanaged);
                // A first-level folder nobody manages and that is not official
                // content is a mod of the user's, indexed or not yet: it keeps
                // its line (DOSSIER§4.5).
                let population = if official {
                    Population::Origin
                } else {
                    Population::Unmanaged
                };
                self.decide_subtree(child, population, State::Nobody, None, owner);
            }
        }
    }

    // --- Content Manager zones (DOSSIER§4.2, mark) ---------------------------

    fn cm_zones(&mut self) {
        let roots: Vec<NodeId> = (1..self.tree.len() as NodeId)
            .filter(|&id| self.tree.node(id).kind == EntryKind::Dir && self.tree.depth(id) <= 4)
            .filter(|&id| crate::acpath::is_externally_managed_root(&self.tree.rel_path(id)))
            .collect();
        for root in roots {
            for id in self.tree.subtree(root) {
                self.class[id as usize].cm_zone = true;
            }
        }
    }

    // --- The library, for the search (DOSSIER§7.1, DOSSIER§7.4) ---------------------

    fn items(&self) -> Vec<LibItem> {
        let present = |rel: PathBuf| self.tree.find(&rel).filter(|&id| self.tree.node(id).present);
        let mut items = Vec::new();
        for (i, owner) in self.owners.iter().enumerate() {
            let (presence, node) = match owner.kind {
                OwnerKind::Car | OwnerKind::Track => {
                    let folder = if owner.kind == OwnerKind::Car { "cars" } else { "tracks" };
                    let node = present(Path::new("content").join(folder).join(&owner.id));
                    let m = self.snap.mods.get(&owner.id);
                    let presence = match m {
                        Some(m) if m.is_stock && m.is_unmanaged => Presence::Unmanaged,
                        Some(m) if m.is_stock => Presence::Origin,
                        _ if node.is_some() => Presence::InGame,
                        _ => Presence::Disabled,
                    };
                    (presence, node)
                }
                OwnerKind::App => {
                    let node = ["python", "lua"]
                        .iter()
                        .find_map(|lang| present(Path::new("apps").join(lang).join(&owner.id)));
                    (
                        if node.is_some() {
                            Presence::InGame
                        } else {
                            Presence::Disabled
                        },
                        node,
                    )
                }
                OwnerKind::Skin | OwnerKind::TrackSkin | OwnerKind::Sound => {
                    let sub = self.snap.subs.iter().find(|s| s.id == owner.id);
                    let parent = owner.parent.clone().unwrap_or_default();
                    let folder = sub.map(|s| s.folder.clone()).unwrap_or_default();
                    let node = match owner.kind {
                        OwnerKind::Skin => present(Path::new("content/cars").join(&parent).join("skins").join(&folder)),
                        OwnerKind::TrackSkin => present(
                            Path::new("content/tracks")
                                .join(&parent)
                                .join("skins")
                                .join("cm_skins")
                                .join(&folder),
                        ),
                        _ => sub
                            .filter(|s| s.is_active)
                            .and_then(|_| present(Path::new("content/cars").join(&parent).join("sfx"))),
                    };
                    (
                        if node.is_some() {
                            Presence::InGame
                        } else {
                            Presence::Disabled
                        },
                        node,
                    )
                }
                OwnerKind::Other => {
                    let o = self.snap.others.iter().find(|o| o.id == owner.id);
                    let node = o
                        .filter(|o| o.is_active)
                        .and_then(|o| o.links.iter().find_map(|l| self.tree.find_abs(Path::new(l))));
                    (
                        if node.is_some() {
                            Presence::InGame
                        } else {
                            Presence::Disabled
                        },
                        node,
                    )
                }
                // A pack is found through its members, not on its own.
                OwnerKind::Pack => continue,
            };
            items.push(LibItem {
                owner: i as OwnerId,
                presence,
                node,
                folded_name: crate::rules::fold(&owner.name),
                folded_id: crate::rules::fold(&owner.id),
            });
        }
        items
    }
}
