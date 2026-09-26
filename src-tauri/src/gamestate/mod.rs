//! The game folder screen (DOSSIER§1): the Assetto Corsa folder as it is on the
//! disk and, for every path, where it comes from.
//!
//! **Read only, by construction** (DOSSIER§R1). Nothing here writes to the game,
//! the library or the database: the scan reads the disk and a snapshot of the
//! database, and its result lives in memory for the session. The module imports
//! no writing function of the engine — `read_only_imports` below checks it on
//! the source itself, because a single `use` would be enough to break the rule
//! without any test of the engine noticing.
//!
//! **The disk is the truth, the database explains** (DOSSIER§R2): the tree is
//! the disk's, and a path the database believes laid but the disk lacks still
//! appears, as missing. Crossing the two is what produces the information.
//!
//! Every judgement is the engine's own, reused rather than rewritten
//! (DOSSIER§9.1): what a deployed folder should hold is `deploy::planned_files`
//! over `compose::planned_sources`, whether a laid copy is still ours is
//! `extras::is_still_ours`, the CM zones are `acpath`'s, official content is
//! `kunos_dates::is_official`. Rewriting any of them here would make the screen
//! end up saying something else than what the engine does.

mod classify;
mod query;
mod snapshot;
mod tree;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

pub use query::{ChildrenPage, Detail, SearchLimits, SearchResults};
pub use snapshot::Snapshot;
pub use tree::NodeId;
use tree::Tree;

// --- The game-write generation (DOSSIER§5.4) --------------------------------

/// Bumped by every Pit Box operation that writes into the game. The index keeps
/// the value it was built at; a different value means it is out of date. A
/// write by someone else (CM, the user) is not seen: that is what the rescan
/// button and the scan time on screen are for.
static GAME_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Held by every command that may write into the game folder: the generation
/// moves when it starts **and when it ends**, whether it succeeds or not - a
/// half-done operation has written too. The second bump is what matters for a
/// long operation (an import, the general repair): a scan started while it runs
/// would otherwise take a generation that the operation's own later writes do
/// not change.
pub struct GameWrite(());

impl GameWrite {
    pub fn begin() -> GameWrite {
        GAME_GENERATION.fetch_add(1, Ordering::Relaxed);
        GameWrite(())
    }
}

impl Drop for GameWrite {
    fn drop(&mut self) {
        GAME_GENERATION.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn game_generation() -> u64 {
    GAME_GENERATION.load(Ordering::Relaxed)
}

// --- What a path can say (DOSSIER§4) -----------------------------------------

/// What the path is (DOSSIER§4.1). First match wins, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Population {
    /// A livery, an app or an other mod laid by link.
    Link,
    /// Inside a folder carrying `.pitbox-deployed.json`.
    ModFolder,
    /// The `sfx/` of a car whose sound mod is active.
    Sound,
    /// A game addition (`extra_links`).
    Extra,
    /// Official content, intact.
    Origin,
    /// A mod installed outside Pit Box.
    Unmanaged,
    #[default]
    Rest,
}

/// What Pit Box knows of the path (DOSSIER§4.2). One per file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Posed,
    ReplacesGame,
    Waiting,
    Drift,
    #[default]
    Nobody,
}

/// The four kinds of drift (DOSSIER§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Drift {
    Modified,
    Missing,
    Orphan,
    BrokenLink,
}

/// Everything the scan concluded about one node.
#[derive(Debug, Clone, Copy, Default)]
pub struct Class {
    pub population: Population,
    pub state: State,
    pub drift: Option<Drift>,
    pub owner: Option<OwnerId>,
    /// How many mods claim the path (game additions); "shared" beyond one.
    pub claimants: u16,
    /// In a zone Content Manager resynchronises (`acpath`).
    pub cm_zone: bool,
    /// Set once a population is decided, so that the first match wins.
    decided: bool,
}

/// Per-state file counts of a subtree (DOSSIER§4.4), plus the two marks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub posed: u32,
    pub replaces_game: u32,
    pub waiting: u32,
    pub drift: u32,
    pub nobody: u32,
    pub cm_zone: u32,
    pub shared: u32,
}

impl Counts {
    pub fn total(&self) -> u32 {
        self.posed + self.replaces_game + self.waiting + self.drift + self.nobody
    }

    fn add_class(&mut self, c: &Class) {
        match c.state {
            State::Posed => self.posed += 1,
            State::ReplacesGame => self.replaces_game += 1,
            State::Waiting => self.waiting += 1,
            State::Drift => self.drift += 1,
            State::Nobody => self.nobody += 1,
        }
        if c.cm_zone {
            self.cm_zone += 1;
        }
        if c.claimants > 1 {
            self.shared += 1;
        }
    }

    fn add(&mut self, o: &Counts) {
        self.posed += o.posed;
        self.replaces_game += o.replaces_game;
        self.waiting += o.waiting;
        self.drift += o.drift;
        self.nobody += o.nobody;
        self.cm_zone += o.cm_zone;
        self.shared += o.shared;
    }
}

// --- Who laid it --------------------------------------------------------------

pub type OwnerId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OwnerKind {
    Car,
    Track,
    App,
    Skin,
    TrackSkin,
    Sound,
    Other,
    Pack,
}

/// Something that claims or provides paths: a mod, an app, a livery, a sound,
/// an other mod, a pack.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Owner {
    pub kind: OwnerKind,
    pub id: String,
    pub name: String,
    /// The car or track a livery or a sound belongs to.
    pub parent: Option<String>,
}

/// An owner as the screen names it in a filter: stable across scans, where an
/// `OwnerId` is not.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRef {
    pub kind: OwnerKind,
    pub id: String,
}

/// One claim on a game addition (§4.5.3).
#[derive(Debug, Clone)]
pub struct ClaimInfo {
    pub owner: OwnerId,
    pub provided: bool,
    /// Authorised explicitly (§4.6ter): the date arbitration does not apply.
    pub forced: bool,
    /// The library copy this claimant would lay.
    pub copy: PathBuf,
}

/// A deployed mod folder: whose, which version, which layers.
#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub owner: OwnerId,
    pub version: Option<String>,
    pub layers: Vec<String>,
}

/// Where a library element is, for "do I have this mod" (DOSSIER§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Presence {
    InGame,
    Disabled,
    Unmanaged,
    Origin,
}

/// A library element the search can find, laid in the game or not.
#[derive(Debug, Clone)]
pub struct LibItem {
    pub owner: OwnerId,
    pub presence: Presence,
    /// The node that shows it in the tree, when it is there.
    pub node: Option<NodeId>,
    folded_name: String,
    folded_id: String,
}

// --- Filters (DOSSIER§6) ------------------------------------------------------

/// The values of the State filter: the five states and the two marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StateValue {
    Posed,
    ReplacesGame,
    Waiting,
    Drift,
    Nobody,
    CmZone,
    Shared,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filters {
    /// Several values: OR (DOSSIER§6.1).
    pub states: Vec<StateValue>,
    /// Values posed with a minus: excluded.
    pub excluded: Vec<StateValue>,
    /// Provenance (DOSSIER§6.2): a single element.
    pub owner: Option<OwnerRef>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.states.is_empty() && self.excluded.is_empty() && self.owner.is_none()
    }

    /// A State chip is posed: grouping is off (DOSSIER§4.5).
    fn has_state(&self) -> bool {
        !self.states.is_empty() || !self.excluded.is_empty()
    }
}

// --- The index (DOSSIER§5.3) ------------------------------------------------

/// The result of a scan, in memory for the session. Nothing of it is persisted.
pub struct Index {
    tree: Tree,
    class: Vec<Class>,
    /// Per-node counts, unfiltered, computed once at the scan.
    counts: Arc<Vec<Counts>>,
    owners: Vec<Owner>,
    items: Vec<LibItem>,
    /// Library copy a laid file comes from.
    sources: HashMap<NodeId, PathBuf>,
    claims: HashMap<NodeId, Vec<ClaimInfo>>,
    /// Where the original of a replaced game file sleeps.
    backups: HashMap<NodeId, PathBuf>,
    /// Root node of each deployed mod folder.
    folders: HashMap<NodeId, FolderInfo>,
    /// False while the states are not filled in yet (DOSSIER§5.2, progressive
    /// display): the tree is usable, its pills are neutral.
    pub classified: bool,
    pub generation: u64,
    pub scanned_at: String,
    /// Counts under the last filter asked for: the tree asks for the children
    /// of many folders under the same filter.
    view: Mutex<Option<(Filters, Arc<Vec<Counts>>)>>,
    /// Folded full paths, built on the first path search only.
    paths: std::sync::OnceLock<Vec<Box<str>>>,
}

impl Index {
    /// An index of the disk alone, before classification.
    fn unclassified(tree: Tree, generation: u64) -> Index {
        let n = tree.len();
        let mut idx = Index {
            tree,
            class: vec![Class::default(); n],
            counts: Arc::new(Vec::new()),
            owners: Vec::new(),
            items: Vec::new(),
            sources: HashMap::new(),
            claims: HashMap::new(),
            backups: HashMap::new(),
            folders: HashMap::new(),
            classified: false,
            generation,
            scanned_at: chrono::Local::now().to_rfc3339(),
            view: Mutex::new(None),
            paths: std::sync::OnceLock::new(),
        };
        idx.counts = Arc::new(idx.compute_counts(&Filters::default()));
        idx
    }

    /// Entries read on the disk (missing paths not counted).
    pub fn entries(&self) -> usize {
        self.tree.nodes.iter().skip(1).filter(|n| n.present).count()
    }

    pub fn root(&self) -> &std::path::Path {
        &self.tree.root
    }

    /// The whole folder's counts - the summary band (DOSSIER§3.3).
    pub fn summary(&self) -> Counts {
        self.counts[tree::ROOT as usize]
    }

    fn owner_id(&self, r: &OwnerRef) -> Option<OwnerId> {
        self.owners
            .iter()
            .position(|o| o.kind == r.kind && o.id == r.id)
            .map(|i| i as OwnerId)
    }

    fn is_leaf(&self, id: NodeId) -> bool {
        self.tree.node(id).kind != tree::EntryKind::Dir
    }

    /// Does this file pass the filters? Folders pass through their files.
    fn passes(&self, id: NodeId, f: &Filters, owner: Option<Option<OwnerId>>) -> bool {
        let c = &self.class[id as usize];
        let is = |v: &StateValue| match v {
            StateValue::Posed => c.state == State::Posed,
            StateValue::ReplacesGame => c.state == State::ReplacesGame,
            StateValue::Waiting => c.state == State::Waiting,
            StateValue::Drift => c.state == State::Drift,
            StateValue::Nobody => c.state == State::Nobody,
            StateValue::CmZone => c.cm_zone,
            StateValue::Shared => c.claimants > 1,
        };
        if !f.states.is_empty() && !f.states.iter().any(is) {
            return false;
        }
        if f.excluded.iter().any(is) {
            return false;
        }
        match owner {
            None => true,
            // A provenance chip naming something this scan does not know:
            // nothing of it is in the game.
            Some(None) => false,
            Some(Some(o)) => self.belongs_to(id, o),
        }
    }

    /// What an element claims or provides (DOSSIER§6.2): what it owns, what it
    /// claims without providing, and for a car or a track, what its liveries
    /// and its sound lay.
    fn belongs_to(&self, id: NodeId, o: OwnerId) -> bool {
        let c = &self.class[id as usize];
        if c.owner == Some(o) || self.claims.get(&id).is_some_and(|v| v.iter().any(|cl| cl.owner == o)) {
            return true;
        }
        let target = &self.owners[o as usize];
        matches!(target.kind, OwnerKind::Car | OwnerKind::Track)
            && c.owner.is_some_and(|x| {
                let x = &self.owners[x as usize];
                x.parent.as_deref() == Some(target.id.as_str())
            })
    }

    fn compute_counts(&self, f: &Filters) -> Vec<Counts> {
        let owner = f.owner.as_ref().map(|r| self.owner_id(r));
        let mut counts = vec![Counts::default(); self.tree.len()];
        for id in (0..self.tree.len() as NodeId).rev() {
            if self.is_leaf(id) && self.passes(id, f, owner) {
                counts[id as usize].add_class(&self.class[id as usize]);
            }
            if id != tree::ROOT {
                let parent = self.tree.node(id).parent as usize;
                let child = counts[id as usize];
                counts[parent].add(&child);
            }
        }
        counts
    }

    /// Counts under `f`, cached for the next call with the same filters.
    fn view(&self, f: &Filters) -> Arc<Vec<Counts>> {
        if f.is_empty() {
            return self.counts.clone();
        }
        let mut cache = self.view.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((key, counts)) = cache.as_ref() {
            if key == f {
                return counts.clone();
            }
        }
        let counts = Arc::new(self.compute_counts(f));
        *cache = Some((f.clone(), counts.clone()));
        counts
    }
}

// --- The scan, and where its result lives -----------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Reading the disk.
    Disk,
    /// Crossing it with the database.
    Classify,
}

/// Progress of a running scan: entries read, never a percentage - the total is
/// not known in advance (DOSSIER§5.2).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub entries: u64,
}

#[derive(Default)]
struct StoreState {
    index: Option<Arc<Index>>,
    running: Option<Progress>,
    error: Option<String>,
}

/// The index of the session, and the scan running if any. Leaving the screen
/// does not cancel a scan; coming back finds its result (DOSSIER§5.2).
#[derive(Default)]
pub struct Store(Mutex<StoreState>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub running: Option<Progress>,
    pub error: Option<String>,
    pub index: Option<IndexInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub root: String,
    pub entries: usize,
    pub scanned_at: String,
    pub classified: bool,
    /// A Pit Box operation wrote into the game since (DOSSIER§5.4).
    pub stale: bool,
    pub summary: Counts,
}

impl Store {
    fn lock(&self) -> std::sync::MutexGuard<'_, StoreState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The current index, if a scan produced one.
    pub fn index(&self) -> Option<Arc<Index>> {
        self.lock().index.clone()
    }

    pub fn status(&self) -> Status {
        let st = self.lock();
        Status {
            running: st.running,
            error: st.error.clone(),
            index: st.index.as_ref().map(|i| IndexInfo {
                root: i.root().to_string_lossy().into_owned(),
                entries: i.entries(),
                scanned_at: i.scanned_at.clone(),
                classified: i.classified,
                stale: i.generation != game_generation(),
                summary: i.summary(),
            }),
        }
    }

    /// Claims the scan slot: `false` if a scan is already running, which the
    /// caller then simply waits for instead of starting a second one.
    pub fn begin(&self) -> bool {
        let mut st = self.lock();
        if st.running.is_some() {
            return false;
        }
        st.running = Some(Progress {
            phase: Phase::Disk,
            entries: 0,
        });
        st.error = None;
        true
    }

    fn set_progress(&self, p: Progress) {
        self.lock().running = Some(p);
    }

    fn publish(&self, index: Index) {
        self.lock().index = Some(Arc::new(index));
    }

    /// Releases the scan slot, keeping the error if the scan failed.
    pub fn end(&self, error: Option<String>) {
        let mut st = self.lock();
        st.running = None;
        st.error = error;
    }
}

/// Runs a whole scan (DOSSIER§5): the disk first, published as soon as it is
/// read so the tree is usable, then the classification, published in its place.
/// `report` is called as it goes - the facade turns it into events. The caller
/// has claimed the slot with [`Store::begin`] and releases it with
/// [`Store::end`].
pub fn run_scan(store: &Store, snap: Snapshot, report: &dyn Fn(Progress)) {
    let progress = |phase: Phase, entries: u64| {
        let p = Progress { phase, entries };
        store.set_progress(p);
        report(p);
    };
    let tree = tree::walk(&snap.ac, &|n| progress(Phase::Disk, n));
    let entries = tree.len() as u64 - 1;
    store.publish(Index::unclassified(tree.clone(), snap.generation));
    progress(Phase::Classify, entries);
    store.publish(classify::classify(tree, &snap));
}

// The end-to-end tests of the scan need a whole game folder of fixtures: they
// live in their own file of this module rather than at the end of this one.
#[cfg(test)]
mod tests;
