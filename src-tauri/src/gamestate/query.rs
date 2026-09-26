//! What the screen asks of an index: the children of a folder (DOSSIER§8.1),
//! the detail panel (§8.2), the search (§7) and the chain down to a path (§7.5).
//! All read the index in memory - none touches the disk, except the detail
//! panel reading the dates of the library copies it names.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::tree::{EntryKind, NodeId, ROOT};
use super::{Counts, Drift, Filters, Index, Owner, OwnerId, Population, Presence, State};

/// One line of the tree (DOSSIER§8.1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: NodeId,
    pub name: String,
    pub kind: EntryKind,
    pub present: bool,
    pub has_children: bool,
    pub population: Population,
    pub state: State,
    pub drift: Option<Drift>,
    /// Name of the element that provides the path.
    pub provider: Option<String>,
    /// Other mods claiming the path ("+N").
    pub others: u16,
    pub cm_zone: bool,
    /// Folder: its counts under the filters in force.
    pub counts: Counts,
}

/// The folded line standing for the children that are entirely nobody's
/// (DOSSIER§4.5).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRow {
    pub count: usize,
    /// `originCars`, `originTracks` or `nobody`: which label the screen shows.
    pub label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildrenPage {
    pub rows: Vec<Row>,
    /// Rows in total, of which `rows` is the page asked for.
    pub total: usize,
    pub group: Option<GroupRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Mechanism {
    /// Laid from the library (hardlink, or copy when the two are on different
    /// disks).
    Library {
        copy: String,
    },
    Link {
        target: String,
        broken: bool,
    },
    /// The game's original sleeps in the library until no mod claims the path.
    Replaced {
        backup: String,
    },
    /// Expected, not on the disk.
    Missing,
    /// A real file nobody laid.
    Real,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimView {
    pub owner: Owner,
    pub provided: bool,
    pub forced: bool,
    /// Date of its library copy.
    pub copy_date: Option<String>,
    /// Why it does not win: `older`, `sameDate` (the last installed wins),
    /// `waiting` (a foreign file holds the path), or none for the provider.
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftView {
    pub kind: Drift,
    pub expected_size: Option<u64>,
    pub actual_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub owner: Owner,
    pub files: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    pub owner: Owner,
    pub version: Option<String>,
    pub layers: Vec<String>,
}

/// The detail panel (DOSSIER§8.2): everything about one node, in the order the
/// panel shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub id: NodeId,
    pub name: String,
    /// Relative to the game folder, with Windows separators.
    pub rel_path: String,
    pub kind: EntryKind,
    pub present: bool,
    pub population: Population,
    pub state: State,
    pub drift: Option<DriftView>,
    pub cm_zone: bool,
    pub mechanism: Option<Mechanism>,
    /// The element the path belongs to.
    pub owner: Option<Owner>,
    pub claims: Vec<ClaimView>,
    /// Replaces the game: deactivating these brings the original back.
    pub revert: Vec<Owner>,
    /// Folder: every count, not only two.
    pub counts: Counts,
    pub contributors: Vec<Contributor>,
    pub more_contributors: usize,
    pub folder: Option<FolderView>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchLimits {
    pub mods: usize,
    pub dirs: usize,
    pub files: usize,
}

impl Default for SearchLimits {
    /// Fifty lines per group, then "see the N others" (DOSSIER§7.3).
    fn default() -> Self {
        SearchLimits {
            mods: 50,
            dirs: 50,
            files: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModHit {
    pub owner: Owner,
    pub presence: Presence,
    /// Its node in the tree, when it is in the game.
    pub node: Option<NodeId>,
    /// In the game but drifted (DOSSIER§7.4): the gap is the thing to see.
    pub drift: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHit {
    pub row: Row,
    /// Parent folder, relative to the game folder.
    pub parent: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group<T> {
    pub total: usize,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub mods: Group<ModHit>,
    pub dirs: Group<PathHit>,
    pub files: Group<PathHit>,
}

/// Fewer characters than this is not a search yet (DOSSIER§7.2).
const MIN_QUERY: usize = 2;

/// A query, folded: case and accents gone, a backslash read as `/`.
struct Query {
    text: String,
    words: Vec<String>,
    /// A separator in the query: it searches full paths (DOSSIER§7.2).
    path_mode: bool,
}

impl Query {
    /// The first word: a name starting with it ranks first (DOSSIER§7.3).
    fn first(&self) -> &str {
        self.words.first().map(String::as_str).unwrap_or("")
    }
}

impl<T> Group<T> {
    fn empty() -> Self {
        Group {
            total: 0,
            items: Vec::new(),
        }
    }
}

fn iso(t: std::time::SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(t).to_rfc3339()
}

impl Index {
    fn owner_of(&self, o: OwnerId) -> Owner {
        self.owners[o as usize].clone()
    }

    fn row(&self, id: NodeId, counts: &[Counts], filtered: bool) -> Row {
        let n = self.tree.node(id);
        let c = &self.class[id as usize];
        let dir = n.kind == EntryKind::Dir;
        Row {
            id,
            name: self.tree.name(id).to_string(),
            kind: n.kind,
            present: n.present,
            has_children: dir
                && if filtered {
                    counts[id as usize].total() > 0
                } else {
                    !n.children.is_empty()
                },
            population: c.population,
            state: c.state,
            drift: c.drift,
            provider: c.owner.map(|o| self.owners[o as usize].name.clone()),
            others: c.claimants.saturating_sub(1),
            cm_zone: c.cm_zone,
            counts: counts[id as usize],
        }
    }

    /// Is this node shown under `f`? A file by its own class, a folder through
    /// its files.
    fn shown(&self, id: NodeId, f: &Filters, counts: &[Counts], owner: Option<Option<OwnerId>>) -> bool {
        if f.is_empty() {
            return true;
        }
        if self.is_leaf(id) {
            self.passes(id, f, owner)
        } else {
            counts[id as usize].total() > 0
        }
    }

    /// Entirely nobody's: the node and its whole subtree, without drift - to
    /// fold into one line (DOSSIER§4.5). An unmanaged mod keeps its line: it
    /// is the user's, and "do I have this mod" must show it.
    fn nobody_only(&self, id: NodeId, counts: &[Counts]) -> bool {
        let c = &self.class[id as usize];
        if c.population == Population::Unmanaged && self.tree.depth(id) == 3 {
            return false;
        }
        if self.is_leaf(id) {
            c.state == State::Nobody
        } else {
            let k = &counts[id as usize];
            k.total() == k.nobody
        }
    }

    /// Folders first, then files, by name regardless of case.
    fn sort_ids(&self, ids: &mut [NodeId]) {
        ids.sort_by_cached_key(|&id| (self.is_leaf(id), self.tree.name(id).to_lowercase()));
    }

    /// Children of `id` under `f` (DOSSIER§8.1): the rows, a page at a time,
    /// and the folded line standing for what is entirely nobody's. With
    /// `members`, the page is taken from inside that folded line instead.
    pub fn children(&self, id: NodeId, f: &Filters, members: bool, offset: usize, limit: usize) -> ChildrenPage {
        if id as usize >= self.tree.len() {
            return ChildrenPage {
                rows: Vec::new(),
                total: 0,
                group: None,
            };
        }
        let counts = self.view(f);
        let owner = f.owner.as_ref().map(|r| self.owner_id(r));
        // A State chip asks to see what it names, "nobody's" included: no
        // grouping then (DOSSIER§4.5). Nor before the states are known.
        let grouping = self.classified && !f.has_state();
        let (mut main, mut grouped): (Vec<NodeId>, Vec<NodeId>) = self
            .tree
            .node(id)
            .children
            .iter()
            .copied()
            .filter(|&c| self.shown(c, f, &counts, owner))
            .partition(|&c| !(grouping && self.nobody_only(c, &counts)));
        self.sort_ids(&mut main);
        self.sort_ids(&mut grouped);
        let group = (!grouped.is_empty()).then(|| GroupRow {
            count: grouped.len(),
            label: self.group_label(id, &grouped),
        });
        let list = if members { grouped } else { main };
        ChildrenPage {
            total: list.len(),
            rows: list
                .iter()
                .skip(offset)
                .take(limit)
                .map(|&c| self.row(c, &counts, !f.is_empty()))
                .collect(),
            group,
        }
    }

    fn group_label(&self, parent: NodeId, members: &[NodeId]) -> &'static str {
        let origin = members
            .iter()
            .all(|&m| self.class[m as usize].population == Population::Origin);
        let rel = self.tree.rel_path(parent).to_string_lossy().to_lowercase();
        match (origin, rel.as_str()) {
            (true, r"content\cars") => "originCars",
            (true, r"content\tracks") => "originTracks",
            _ => "nobody",
        }
    }

    /// The detail panel of `id` (DOSSIER§8.2).
    pub fn detail(&self, id: NodeId) -> Option<Detail> {
        if id as usize >= self.tree.len() {
            return None;
        }
        let n = self.tree.node(id);
        let c = &self.class[id as usize];
        let leaf = n.kind != EntryKind::Dir;

        let backup = self.backups.get(&id).or_else(|| {
            // A replaced sound is recorded on its `sfx/` folder.
            (c.population == Population::Sound)
                .then(|| {
                    self.tree
                        .ancestry(id)
                        .into_iter()
                        .rev()
                        .find_map(|a| self.backups.get(&a))
                })
                .flatten()
        });
        let mechanism = leaf.then(|| {
            if !n.present {
                Mechanism::Missing
            } else if n.kind == EntryKind::Link {
                let target = self.tree.link_targets.get(&id);
                Mechanism::Link {
                    target: target.map(|t| t.to_string_lossy().into_owned()).unwrap_or_default(),
                    broken: c.drift == Some(Drift::BrokenLink),
                }
            } else if let Some(b) = backup.filter(|_| c.state == State::ReplacesGame) {
                Mechanism::Replaced {
                    backup: b.to_string_lossy().into_owned(),
                }
            } else if let Some(src) = self.sources.get(&id) {
                Mechanism::Library {
                    copy: src.to_string_lossy().into_owned(),
                }
            } else {
                Mechanism::Real
            }
        });

        let drift = c.drift.map(|kind| DriftView {
            kind,
            expected_size: (kind == Drift::Modified)
                .then(|| {
                    self.sources
                        .get(&id)
                        .and_then(|s| std::fs::metadata(s).ok())
                        .map(|m| m.len())
                })
                .flatten(),
            actual_size: (kind == Drift::Modified).then_some(n.size),
        });

        let claims = self.claim_views(id);
        let revert = if c.state == State::ReplacesGame {
            let mut owners: Vec<Owner> = claims.iter().map(|cl| cl.owner.clone()).collect();
            if owners.is_empty() {
                owners.extend(c.owner.map(|o| self.owner_of(o)));
            }
            owners
        } else {
            Vec::new()
        };

        let (contributors, more_contributors) = if leaf { (Vec::new(), 0) } else { self.contributors(id) };
        let folder = self.folders.get(&id).map(|f| FolderView {
            owner: self.owner_of(f.owner),
            version: f.version.clone(),
            layers: f.layers.clone(),
        });
        Some(Detail {
            id,
            name: self.tree.name(id).to_string(),
            rel_path: self.tree.rel_path(id).to_string_lossy().into_owned(),
            kind: n.kind,
            present: n.present,
            population: c.population,
            state: c.state,
            drift,
            cm_zone: c.cm_zone,
            mechanism,
            owner: c.owner.map(|o| self.owner_of(o)),
            claims,
            revert,
            counts: self.counts[id as usize],
            contributors,
            more_contributors,
            folder,
        })
    }

    /// The claimants of a game addition, the provider first, each with the
    /// reason it does not win (DOSSIER§8.2, "Provenance").
    fn claim_views(&self, id: NodeId) -> Vec<ClaimView> {
        let Some(claims) = self.claims.get(&id) else {
            return Vec::new();
        };
        let date = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let provider_date = claims.iter().find(|c| c.provided).and_then(|c| date(&c.copy));
        let mut views: Vec<ClaimView> = claims
            .iter()
            .map(|cl| {
                let d = date(&cl.copy);
                let reason = if cl.provided {
                    None
                } else {
                    match (provider_date, d) {
                        (None, _) => Some("waiting"),
                        (Some(p), Some(mine)) if mine < p => Some("older"),
                        _ => Some("sameDate"),
                    }
                };
                ClaimView {
                    owner: self.owner_of(cl.owner),
                    provided: cl.provided,
                    forced: cl.forced,
                    copy_date: d.map(iso),
                    reason,
                }
            })
            .collect();
        views.sort_by_key(|v| !v.provided);
        views
    }

    /// Who lays the files of a folder, by number of files (DOSSIER§8.2): five,
    /// and how many more.
    fn contributors(&self, id: NodeId) -> (Vec<Contributor>, usize) {
        let mut by: HashMap<OwnerId, u32> = HashMap::new();
        for n in self.tree.subtree(id) {
            if !self.is_leaf(n) {
                continue;
            }
            let c = &self.class[n as usize];
            if c.state == State::Nobody {
                continue;
            }
            if let Some(o) = c.owner {
                *by.entry(o).or_default() += 1;
            }
        }
        let mut list: Vec<(OwnerId, u32)> = by.into_iter().collect();
        list.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| self.owners[a.0 as usize].name.cmp(&self.owners[b.0 as usize].name))
        });
        let more = list.len().saturating_sub(5);
        let top = list
            .into_iter()
            .take(5)
            .map(|(o, files)| Contributor {
                owner: self.owner_of(o),
                files,
            })
            .collect();
        (top, more)
    }

    /// The chain of nodes down to `rel` - what the tree unfolds to reach it
    /// (DOSSIER§7.5). `None` when this scan does not have it.
    pub fn reveal(&self, rel: &str) -> Option<Vec<NodeId>> {
        let rel = rel.replace('/', "\\");
        if rel.is_empty() {
            return Some(Vec::new());
        }
        self.tree.find(Path::new(&rel)).map(|id| self.tree.ancestry(id))
    }

    /// Absolute path of `id`, or of its nearest ancestor on the disk when it
    /// is missing - what "show in Explorer" opens.
    pub fn explorer_target(&self, id: NodeId) -> Option<std::path::PathBuf> {
        if id as usize >= self.tree.len() {
            return None;
        }
        let mut cur = id;
        while !self.tree.node(cur).present && cur != ROOT {
            cur = self.tree.node(cur).parent;
        }
        Some(self.tree.abs_path(cur))
    }

    fn folded_paths(&self) -> &[Box<str>] {
        self.paths.get_or_init(|| {
            let mut out: Vec<Box<str>> = Vec::with_capacity(self.tree.len());
            out.push("".into());
            for id in 1..self.tree.len() as NodeId {
                let parent = &out[self.tree.node(id).parent as usize];
                let name = self.tree.folded_name(id);
                let p = if parent.is_empty() {
                    name.to_string()
                } else {
                    format!("{parent}/{name}")
                };
                out.push(p.into_boxed_str());
            }
            out
        })
    }

    /// The search (DOSSIER§7): the index's paths and the library's elements,
    /// case- and accent-insensitive, every word required. A query with a
    /// separator searches full paths.
    pub fn search(&self, query: &str, f: &Filters, limits: SearchLimits) -> SearchResults {
        let q = crate::rules::fold(query.trim()).replace('\\', "/");
        let q = Query {
            path_mode: q.contains('/'),
            words: q.split_whitespace().map(str::to_string).collect(),
            text: q,
        };
        if q.text.chars().count() < MIN_QUERY {
            return SearchResults {
                mods: Group::empty(),
                dirs: Group::empty(),
                files: Group::empty(),
            };
        }
        let (dirs, files) = self.search_paths(&q, f, limits);
        SearchResults {
            mods: self.search_mods(&q, f, limits.mods),
            dirs,
            files,
        }
    }

    /// Mods: by display name and by id, laid or not (DOSSIER§7.1).
    fn search_mods(&self, q: &Query, f: &Filters, limit: usize) -> Group<ModHit> {
        let owner_filter = f.owner.as_ref().map(|r| self.owner_id(r));
        let mut mods: Vec<&super::LibItem> = self
            .items
            .iter()
            .filter(|it| owner_filter.is_none_or(|o| o == Some(it.owner)))
            .filter(|it| {
                if q.path_mode {
                    it.folded_id.contains(q.text.as_str())
                } else {
                    q.words
                        .iter()
                        .all(|w| it.folded_name.contains(w.as_str()) || it.folded_id.contains(w.as_str()))
                }
            })
            .collect();
        mods.sort_by_cached_key(|it| (!it.folded_name.starts_with(q.first()), it.folded_name.clone()));
        Group {
            total: mods.len(),
            items: mods
                .iter()
                .take(limit)
                .map(|it| ModHit {
                    owner: self.owner_of(it.owner),
                    presence: it.presence,
                    node: it.node,
                    drift: it.node.is_some_and(|n| self.counts[n as usize].drift > 0),
                })
                .collect(),
        }
    }

    /// Does node `id` answer `q`? A plain query: at least one word in its own
    /// name - a file is found by its name, not by its folder's -, the others
    /// anywhere in its path. A path query: the match must end in its own name,
    /// so that `skins/red` finds the livery folder, not each of its files.
    fn path_hit(&self, id: NodeId, q: &Query, paths: Option<&[Box<str>]>) -> bool {
        let name = self.tree.folded_name(id);
        match paths {
            Some(paths) => {
                let p = &paths[id as usize];
                p.rfind(q.text.as_str())
                    .is_some_and(|at| at + q.text.len() > p.len() - name.len())
            }
            None => {
                let missing: Vec<&String> = q.words.iter().filter(|w| !name.contains(w.as_str())).collect();
                if missing.len() == q.words.len() {
                    return false;
                }
                missing.is_empty() || {
                    let p = self.folded_path_of(id);
                    missing.iter().all(|w| p.contains(w.as_str()))
                }
            }
        }
    }

    /// Folders and files of the index answering `q`, under the filters.
    fn search_paths(&self, q: &Query, f: &Filters, limits: SearchLimits) -> (Group<PathHit>, Group<PathHit>) {
        let counts = self.view(f);
        let owner = f.owner.as_ref().map(|r| self.owner_id(r));
        let paths = q.path_mode.then(|| self.folded_paths());
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for id in 1..self.tree.len() as NodeId {
            if !self.path_hit(id, q, paths) || !self.shown(id, f, &counts, owner) {
                continue;
            }
            // A junction is a folder to whoever looks for one - a livery, an
            // app - even though the tree never opens it.
            let n = self.tree.node(id);
            if n.kind == EntryKind::Dir || n.link_dir {
                dirs.push(id);
            } else {
                files.push(id);
            }
        }
        let hits = |mut ids: Vec<NodeId>, limit: usize| {
            ids.sort_by_cached_key(|&id| {
                let name = self.tree.folded_name(id);
                (!name.starts_with(q.first()), name.to_string())
            });
            Group {
                total: ids.len(),
                items: ids
                    .into_iter()
                    .take(limit)
                    .map(|id| PathHit {
                        row: self.row(id, &counts, !f.is_empty()),
                        parent: self
                            .tree
                            .rel_path(self.tree.node(id).parent)
                            .to_string_lossy()
                            .into_owned(),
                    })
                    .collect(),
            }
        };
        (hits(dirs, limits.dirs), hits(files, limits.files))
    }

    /// Folded path of one node, built from its ancestors' folded names.
    fn folded_path_of(&self, id: NodeId) -> String {
        self.tree
            .ancestry(id)
            .iter()
            .map(|&a| self.tree.folded_name(a))
            .collect::<Vec<_>>()
            .join("/")
    }
}
