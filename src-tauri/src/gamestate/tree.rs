//! The game folder as it is on the disk (DOSSIER§5.1, first phase): every entry,
//! its type, size and date, and the target of every link — nothing else. What
//! an entry *means* is the classification's job (`classify.rs`).

use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

pub type NodeId = u32;
/// The game folder itself.
pub const ROOT: NodeId = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Dir,
    File,
    /// A reparse point — directory junction or file link. Never walked into
    /// (DOSSIER§4.1): its target lives in the library, and walking it would
    /// count its files twice and show the library inside the game.
    Link,
}

#[derive(Debug, Clone)]
pub struct Node {
    /// `ROOT` for the root itself.
    pub parent: NodeId,
    name: u32,
    pub kind: EntryKind,
    /// False for a path Pit Box expects and the disk does not have (DOSSIER§4.3,
    /// "missing"): added by the classification, never read from the disk.
    pub present: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// A link to a folder (a junction, a livery) rather than to a file.
    pub link_dir: bool,
    pub children: Vec<NodeId>,
}

/// The disk tree, flat. **A parent always has a smaller id than its children**
/// (breadth-first walk, missing paths appended with their ancestors first):
/// walking the ids backwards therefore visits every child before its parent,
/// which is how folder counts are summed in one pass.
#[derive(Debug, Clone)]
pub struct Tree {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    /// Names as on the disk, interned: `ui`, `data`, `preview.jpg` repeat
    /// hundreds of thousands of times across an install.
    names: Vec<Box<str>>,
    /// Accent- and case-folded form of each name, for the search (DOSSIER§7.2).
    folded: Vec<Box<str>>,
    /// Lowercase form id of each name, for the case-insensitive lookup below.
    lower: Vec<u32>,
    interned: HashMap<Box<str>, u32>,
    lower_interned: HashMap<Box<str>, u32>,
    /// `(parent, lowercase name) → child`: the paths the database stores were
    /// written with the casing of the library copy, which the disk need not
    /// share - Windows paths are case-insensitive.
    lookup: HashMap<(NodeId, u32), NodeId>,
    /// Target of every link, as read from the reparse point.
    pub link_targets: HashMap<NodeId, PathBuf>,
}

impl Tree {
    fn new(root: &Path) -> Self {
        let mut t = Tree {
            root: root.to_path_buf(),
            nodes: Vec::new(),
            names: Vec::new(),
            folded: Vec::new(),
            lower: Vec::new(),
            interned: HashMap::new(),
            lower_interned: HashMap::new(),
            lookup: HashMap::new(),
            link_targets: HashMap::new(),
        };
        let name = t.intern("");
        t.nodes.push(Node {
            parent: ROOT,
            name,
            kind: EntryKind::Dir,
            present: true,
            size: 0,
            modified: None,
            link_dir: false,
            children: Vec::new(),
        });
        t
    }

    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.interned.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        let lower = name.to_lowercase();
        let next_lower = self.lower_interned.len() as u32;
        let lower_id = *self.lower_interned.entry(lower.into_boxed_str()).or_insert(next_lower);
        self.names.push(name.into());
        self.folded.push(crate::rules::fold(name).into_boxed_str());
        self.lower.push(lower_id);
        self.interned.insert(name.into(), id);
        id
    }

    fn push(&mut self, parent: NodeId, name: &str, kind: EntryKind, present: bool) -> NodeId {
        let name = self.intern(name);
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            parent,
            name,
            kind,
            present,
            size: 0,
            modified: None,
            link_dir: false,
            children: Vec::new(),
        });
        self.nodes[parent as usize].children.push(id);
        self.lookup.insert((parent, self.lower[name as usize]), id);
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn name(&self, id: NodeId) -> &str {
        &self.names[self.node(id).name as usize]
    }

    pub fn folded_name(&self, id: NodeId) -> &str {
        &self.folded[self.node(id).name as usize]
    }

    /// Child of `parent` called `name`, whatever its case.
    pub fn child(&self, parent: NodeId, name: &str) -> Option<NodeId> {
        let lower = self.lower_interned.get(name.to_lowercase().as_str())?;
        self.lookup.get(&(parent, *lower)).copied()
    }

    /// Node at `rel` (relative to the game folder), whatever its case.
    pub fn find(&self, rel: &Path) -> Option<NodeId> {
        let mut cur = ROOT;
        for part in normal_parts(rel)? {
            cur = self.child(cur, part)?;
        }
        Some(cur)
    }

    /// Node at `abs`, an absolute path the database stored: `None` outside the
    /// game folder. The prefix is compared without regard to case, like the
    /// rest of the path.
    pub fn find_abs(&self, abs: &Path) -> Option<NodeId> {
        self.find(&self.relative(abs)?)
    }

    /// `abs` relative to the game folder, case-insensitively.
    pub fn relative(&self, abs: &Path) -> Option<PathBuf> {
        if let Ok(rel) = abs.strip_prefix(&self.root) {
            return Some(rel.to_path_buf());
        }
        let root: Vec<String> = self.root.components().map(|c| lower(c.as_os_str())).collect();
        let parts: Vec<Component> = abs.components().collect();
        if parts.len() < root.len() || parts.iter().zip(&root).any(|(c, r)| &lower(c.as_os_str()) != r) {
            return None;
        }
        Some(parts[root.len()..].iter().collect())
    }

    /// Path of `id` relative to the game folder.
    pub fn rel_path(&self, id: NodeId) -> PathBuf {
        let mut parts = Vec::new();
        let mut cur = id;
        while cur != ROOT {
            parts.push(self.name(cur));
            cur = self.node(cur).parent;
        }
        parts.iter().rev().collect()
    }

    pub fn abs_path(&self, id: NodeId) -> PathBuf {
        self.root.join(self.rel_path(id))
    }

    /// Chain from the root (excluded) down to `id` (included).
    pub fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut cur = id;
        while cur != ROOT {
            chain.push(cur);
            cur = self.node(cur).parent;
        }
        chain.reverse();
        chain
    }

    /// Depth below the root: 1 for `content`, 3 for `content/cars/<id>`.
    pub fn depth(&self, id: NodeId) -> usize {
        self.ancestry(id).len()
    }

    /// Every node of the subtree of `id`, itself included.
    pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = vec![id];
        let mut i = 0;
        while i < out.len() {
            out.extend_from_slice(&self.node(out[i]).children);
            i += 1;
        }
        out
    }

    /// Node for `rel`, **added as missing** with its missing ancestors when the
    /// disk does not have it (DOSSIER§R2: a path the database believes laid and
    /// the disk lacks still shows, as missing). Stops at a file or a link met on
    /// the way: nothing can live inside those, and inventing a folder there
    /// would lie about the disk.
    pub fn ensure_missing(&mut self, rel: &Path) -> Option<NodeId> {
        let parts: Vec<String> = normal_parts(rel)?.into_iter().map(str::to_string).collect();
        let mut cur = ROOT;
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == parts.len();
            cur = match self.child(cur, part) {
                Some(id) => id,
                None => {
                    if self.node(cur).kind != EntryKind::Dir {
                        return None;
                    }
                    let kind = if last { EntryKind::File } else { EntryKind::Dir };
                    self.push(cur, part, kind, false)
                }
            };
            if !last && self.node(cur).kind != EntryKind::Dir {
                return None;
            }
        }
        Some(cur)
    }
}

fn lower(s: &std::ffi::OsStr) -> String {
    s.to_string_lossy().to_lowercase()
}

/// The plain segments of a relative path, `None` if it climbs out (`..`) or is
/// absolute.
fn normal_parts(rel: &Path) -> Option<Vec<&str>> {
    rel.components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            Component::CurDir => Some(""),
            _ => None,
        })
        .filter(|s| s != &Some(""))
        .collect()
}

/// How often the walk reports how far it got.
const PROGRESS_EVERY: u64 = 2_000;

/// Reads the whole game folder. **Links are never followed** (`file_type` of a
/// directory entry comes from the directory listing, it does not resolve the
/// reparse point): their target is read and kept, their content is not walked.
/// A folder that cannot be read is logged and left empty — one unreadable
/// folder must not cost the rest of the install.
pub fn walk(root: &Path, progress: &dyn Fn(u64)) -> Tree {
    let mut tree = Tree::new(root);
    let mut queue: VecDeque<(NodeId, PathBuf)> = VecDeque::from([(ROOT, root.to_path_buf())]);
    let mut seen: u64 = 0;
    while let Some((parent, dir)) = queue.pop_front() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("gamestate: cannot read {}: {e}", dir.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if ft.is_symlink() {
                let id = tree.push(parent, &name, EntryKind::Link, true);
                #[cfg(windows)]
                {
                    use std::os::windows::fs::FileTypeExt;
                    tree.nodes[id as usize].link_dir = ft.is_symlink_dir();
                }
                match std::fs::read_link(&path) {
                    Ok(target) => {
                        tree.link_targets.insert(id, strip_verbatim(target));
                    }
                    Err(e) => log::warn!("gamestate: cannot read link {}: {e}", path.display()),
                }
            } else if ft.is_dir() {
                let id = tree.push(parent, &name, EntryKind::Dir, true);
                queue.push_back((id, path));
            } else {
                let id = tree.push(parent, &name, EntryKind::File, true);
                // On Windows the directory listing already carries size and
                // date: no extra system call per file.
                if let Ok(meta) = entry.metadata() {
                    let n = &mut tree.nodes[id as usize];
                    n.size = meta.len();
                    n.modified = meta.modified().ok();
                }
            }
            seen += 1;
            if seen.is_multiple_of(PROGRESS_EVERY) {
                progress(seen);
            }
        }
    }
    progress(seen);
    tree
}

/// `read_link` hands back `\\?\D:\…` for a junction: the verbatim prefix would
/// make every comparison with a library path fail.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p,
    }
}
