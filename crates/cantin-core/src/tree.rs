//! Arena-based scan result. Children of every directory are stored contiguously
//! and pre-sorted by size (largest first), so "list children" is a slice.

use crate::categories::Category;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Dir,
    Symlink,
    Other,
}

pub mod flags {
    /// Another hard link to this inode was already counted; size is 0.
    pub const HARDLINK_DUP: u8 = 1;
    /// Directory could not be read (permissions etc).
    pub const UNREADABLE: u8 = 2;
    /// Directory deliberately not entered (other filesystem, pseudo fs, excluded).
    pub const SKIPPED: u8 = 4;
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Box<str>,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub child_count: u32,
    /// Allocated bytes on disk (aggregated for directories).
    pub size: u64,
    /// Logical file length (aggregated for directories).
    pub apparent: u64,
    /// Number of files below (1 for a file).
    pub file_count: u64,
    /// Modification time, unix seconds (newest below, for directories).
    pub mtime: i64,
    pub kind: NodeKind,
    pub category: Category,
    pub flags: u8,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.kind == NodeKind::Dir
    }
    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
    pub fn extension(&self) -> Option<&str> {
        match self.name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && self.kind == NodeKind::File => Some(ext),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CategoryTotal {
    pub category: Option<Category>,
    pub label: &'static str,
    pub size: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanStats {
    pub root: String,
    pub total_size: u64,
    pub total_apparent: u64,
    pub files: u64,
    pub dirs: u64,
    pub errors: u64,
    pub elapsed_ms: u64,
}

pub struct Tree {
    pub nodes: Vec<Node>,
    pub root_path: PathBuf,
    pub stats: ScanStats,
    pub categories: Vec<CategoryTotal>,
}

impl Tree {
    pub const ROOT: NodeId = 0;

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize)
    }
    pub fn root(&self) -> &Node {
        &self.nodes[0]
    }

    pub fn children(&self, id: NodeId) -> &[Node] {
        match self.node(id) {
            Some(n) if n.child_count > 0 => {
                let s = n.first_child as usize;
                &self.nodes[s..s + n.child_count as usize]
            }
            _ => &[],
        }
    }

    pub fn child_ids(&self, id: NodeId) -> std::ops::Range<NodeId> {
        match self.node(id) {
            Some(n) if n.child_count > 0 => n.first_child..n.first_child + n.child_count,
            _ => 0..0,
        }
    }

    /// Ancestors from the root down to (and including) `id`.
    pub fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut cur = id;
        while (cur as usize) < self.nodes.len() {
            out.push(cur);
            let p = self.nodes[cur as usize].parent;
            if p == NO_PARENT {
                break;
            }
            cur = p;
        }
        out.reverse();
        out
    }

    pub fn path(&self, id: NodeId) -> PathBuf {
        let chain = self.ancestry(id);
        let mut p = self.root_path.clone();
        for &n in chain.iter().skip(1) {
            p.push(&*self.nodes[n as usize].name);
        }
        p
    }

    /// Resolve an absolute path inside the scanned tree to a node id.
    pub fn find_path(&self, path: &Path) -> Option<NodeId> {
        let rel = path.strip_prefix(&self.root_path).ok()?;
        let mut cur = Self::ROOT;
        for comp in rel.components() {
            let name = comp.as_os_str().to_string_lossy();
            cur = self
                .child_ids(cur)
                .find(|&c| *self.nodes[c as usize].name == *name)?;
        }
        Some(cur)
    }

    /// The `n` largest files (by allocated size) under `under` (default root).
    pub fn top_files(&self, n: usize, under: Option<NodeId>) -> Vec<NodeId> {
        let range = match under {
            Some(id) if id != Self::ROOT => self.subtree_range(id),
            _ => 0..self.nodes.len() as NodeId,
        };
        let mut heap: BinaryHeap<std::cmp::Reverse<(u64, NodeId)>> = BinaryHeap::new();
        for id in range {
            let node = &self.nodes[id as usize];
            if node.kind != NodeKind::File {
                continue;
            }
            if under.is_some() && !self.is_descendant(id, under.unwrap()) {
                continue;
            }
            heap.push(std::cmp::Reverse((node.size, id)));
            if heap.len() > n {
                heap.pop();
            }
        }
        let mut v: Vec<_> = heap.into_iter().map(|r| r.0).collect();
        v.sort_unstable_by(|a, b| b.cmp(a));
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// BFS layout means all descendants have ids greater than the node; this
    /// returns a conservative id range that contains every descendant.
    pub fn subtree_range(&self, id: NodeId) -> std::ops::Range<NodeId> {
        id..self.nodes.len() as NodeId
    }

    pub fn is_descendant(&self, mut id: NodeId, ancestor: NodeId) -> bool {
        if ancestor == Self::ROOT {
            return true;
        }
        while id != NO_PARENT {
            if id == ancestor {
                return true;
            }
            if id < ancestor {
                return false; // BFS: ancestors always have smaller ids
            }
            id = self.nodes[id as usize].parent;
        }
        false
    }
}
