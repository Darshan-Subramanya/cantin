//! Parallel filesystem scanner.
//!
//! Directories are read with `std::fs::read_dir` and subdirectories are
//! processed in parallel via rayon's work-stealing pool. The result is first
//! built as an owned tree (cheap to assemble in parallel), then flattened into
//! the BFS-ordered [`Tree`] arena.

use crate::categories::{self, Category};
use crate::progress::ScanProgress;
use crate::tree::{flags, CategoryTotal, Node, NodeKind, ScanStats, Tree, NO_PARENT};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Mutex;
use std::time::{Instant, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    /// Do not descend into other mounted filesystems (like `du -x`).
    pub one_filesystem: bool,
    /// Skip kernel pseudo filesystems such as /proc, /sys and /dev.
    pub skip_pseudo: bool,
    /// Absolute paths that should not be entered.
    pub exclude: Vec<PathBuf>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            one_filesystem: true,
            skip_pseudo: true,
            exclude: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("cannot open {path}: {source}")]
    Root {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    #[error("scan cancelled")]
    Cancelled,
}

struct Raw {
    name: String,
    kind: NodeKind,
    size: u64,
    apparent: u64,
    file_count: u64,
    mtime: i64,
    flags: u8,
    children: Vec<Raw>,
}

struct Ctx<'a> {
    opts: &'a ScanOptions,
    progress: &'a ScanProgress,
    allowed_devs: Vec<u64>,
    skip_paths: Vec<PathBuf>,
    inodes: Mutex<HashSet<(u64, u64)>>,
}

pub fn scan(root: &Path, opts: &ScanOptions, progress: &ScanProgress) -> Result<Tree, ScanError> {
    let started = Instant::now();
    let root_path = dunce::canonicalize(root).map_err(|e| ScanError::Root {
        path: root.to_path_buf(),
        source: e,
    })?;
    let md = fs::metadata(&root_path).map_err(|e| ScanError::Root {
        path: root_path.clone(),
        source: e,
    })?;
    if !md.is_dir() {
        return Err(ScanError::NotADirectory(root_path));
    }

    let ctx = Ctx {
        opts,
        progress,
        allowed_devs: platform::allowed_devices(&root_path, &md),
        skip_paths: build_skip_list(&root_path, opts),
        inodes: Mutex::new(HashSet::new()),
    };

    let children = scan_dir(&root_path, &ctx).ok_or_else(|| ScanError::Root {
        path: root_path.clone(),
        source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
    })?;
    if progress.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let mut raw = Raw {
        name: root_path.to_string_lossy().into_owned(),
        kind: NodeKind::Dir,
        size: platform::allocated(&md),
        apparent: 0,
        file_count: 0,
        mtime: mtime(&md),
        flags: 0,
        children,
    };
    aggregate(&mut raw);

    let (nodes, categories) = flatten(raw);
    let root_node = &nodes[0];
    let stats = ScanStats {
        root: root_path.to_string_lossy().into_owned(),
        total_size: root_node.size,
        total_apparent: root_node.apparent,
        files: root_node.file_count,
        dirs: progress.dirs.load(Relaxed),
        errors: progress.errors.load(Relaxed),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    Ok(Tree {
        nodes,
        root_path,
        stats,
        categories,
    })
}

fn build_skip_list(root: &Path, opts: &ScanOptions) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = opts.exclude.clone();
    if opts.skip_pseudo && cfg!(target_os = "linux") {
        for p in ["/proc", "/sys", "/dev", "/run/user"] {
            v.push(PathBuf::from(p));
        }
    }
    if cfg!(target_os = "macos") {
        // The Data volume is reachable through firmlinks (/Users, /Applications ...).
        // Scanning it again via /System/Volumes/Data would double count.
        if root == Path::new("/") {
            v.push(PathBuf::from("/System/Volumes"));
        }
        if opts.skip_pseudo {
            v.push(PathBuf::from("/dev"));
            v.push(PathBuf::from("/Volumes")); // other drives; scan them explicitly
        }
    }
    // Never skip the root itself even if the user excluded a parent.
    v.retain(|p| p != root);
    v
}

fn mtime(md: &Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Returns `None` when the directory cannot be read.
fn scan_dir(path: &Path, ctx: &Ctx) -> Option<Vec<Raw>> {
    if ctx.progress.is_cancelled() {
        return Some(Vec::new());
    }
    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            ctx.progress.errors.fetch_add(1, Relaxed);
            return None;
        }
    };

    let mut out = Vec::new();
    let mut subdirs: Vec<(String, PathBuf, Metadata)> = Vec::new();
    let mut local_files = 0u64;
    let mut local_bytes = 0u64;

    for entry in rd {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                ctx.progress.errors.fetch_add(1, Relaxed);
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        // DirEntry::metadata does not follow symlinks.
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                ctx.progress.errors.fetch_add(1, Relaxed);
                continue;
            }
        };
        let ft = md.file_type();
        let mut raw = Raw {
            name,
            kind: NodeKind::Other,
            size: 0,
            apparent: 0,
            file_count: 0,
            mtime: mtime(&md),
            flags: 0,
            children: Vec::new(),
        };

        if ft.is_symlink() || platform::is_reparse_point(&md) {
            raw.kind = NodeKind::Symlink;
            out.push(raw);
        } else if ft.is_dir() {
            let child_path = entry.path();
            raw.kind = NodeKind::Dir;
            if should_skip_dir(&child_path, &md, ctx) {
                raw.flags |= flags::SKIPPED;
                out.push(raw);
            } else {
                subdirs.push((raw.name, child_path, md));
            }
        } else if ft.is_file() {
            raw.kind = NodeKind::File;
            raw.apparent = md.len();
            raw.file_count = 1;
            if platform::is_counted_hardlink(&md, &ctx.inodes) {
                raw.flags |= flags::HARDLINK_DUP;
            } else {
                raw.size = platform::allocated(&md);
            }
            local_files += 1;
            local_bytes += raw.size;
            out.push(raw);
        } else {
            out.push(raw); // sockets, fifos, devices: zero size
        }
    }

    ctx.progress.files.fetch_add(local_files, Relaxed);
    ctx.progress.bytes.fetch_add(local_bytes, Relaxed);

    let dirs: Vec<Raw> = subdirs
        .into_par_iter()
        .map(|(name, p, md)| {
            ctx.progress.dirs.fetch_add(1, Relaxed);
            ctx.progress.set_current(&p.to_string_lossy());
            let children = scan_dir(&p, ctx);
            let readable = children.is_some();
            let mut raw = Raw {
                name,
                kind: NodeKind::Dir,
                size: platform::allocated(&md),
                apparent: 0,
                file_count: 0,
                mtime: mtime(&md),
                flags: if readable { 0 } else { flags::UNREADABLE },
                children: children.unwrap_or_default(),
            };
            aggregate(&mut raw);
            raw
        })
        .collect();
    out.extend(dirs);
    Some(out)
}

fn should_skip_dir(path: &Path, md: &Metadata, ctx: &Ctx) -> bool {
    if ctx.skip_paths.iter().any(|s| s == path) {
        return true;
    }
    if ctx.opts.one_filesystem {
        if let Some(dev) = platform::device(md) {
            if !ctx.allowed_devs.is_empty() && !ctx.allowed_devs.contains(&dev) {
                return true;
            }
        }
    }
    false
}

/// Sum children into a directory node (children are already aggregated).
fn aggregate(raw: &mut Raw) {
    let mut size = raw.size;
    let mut apparent = 0;
    let mut files = 0;
    let mut newest = raw.mtime;
    for c in &raw.children {
        size += c.size;
        if c.flags & flags::HARDLINK_DUP == 0 {
            apparent += c.apparent;
        }
        files += c.file_count;
        newest = newest.max(c.mtime);
    }
    raw.size = size;
    raw.apparent = apparent;
    raw.file_count = files;
    raw.mtime = newest;
}

fn flatten(root: Raw) -> (Vec<Node>, Vec<CategoryTotal>) {
    let mut cat_totals: Vec<CategoryTotal> = Category::ALL
        .iter()
        .map(|&c| CategoryTotal {
            category: Some(c),
            label: c.label(),
            size: 0,
            count: 0,
        })
        .collect();

    let mut nodes: Vec<Node> = Vec::with_capacity(1024);
    let Raw {
        name,
        kind,
        size,
        apparent,
        file_count,
        mtime,
        flags,
        children,
    } = root;
    nodes.push(Node {
        name: name.into_boxed_str(),
        parent: NO_PARENT,
        first_child: 0,
        child_count: 0,
        size,
        apparent,
        file_count,
        mtime,
        kind,
        category: Category::Other,
        flags,
    });

    let mut queue: VecDeque<(u32, Vec<Raw>, Option<Category>)> = VecDeque::new();
    queue.push_back((0, children, None));

    while let Some((pid, mut kids, ctx)) = queue.pop_front() {
        kids.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
        let first = nodes.len() as u32;
        nodes[pid as usize].first_child = first;
        nodes[pid as usize].child_count = kids.len() as u32;
        for mut k in kids {
            let idx = nodes.len() as u32;
            let (category, child_ctx) = match k.kind {
                NodeKind::Dir => {
                    let c = if categories::is_bundle_dir(&k.name) && ctx.is_none() {
                        Some(Category::Installer)
                    } else {
                        categories::dir_context(&k.name, ctx)
                    };
                    (c.unwrap_or(Category::Other), c)
                }
                NodeKind::File => (categories::classify_file(&k.name, ctx), None),
                _ => (Category::Other, None),
            };
            if k.kind == NodeKind::File {
                let t = &mut cat_totals[category.index()];
                t.size += k.size;
                t.count += 1;
            }
            let grandkids = std::mem::take(&mut k.children);
            nodes.push(Node {
                name: k.name.into_boxed_str(),
                parent: pid,
                first_child: 0,
                child_count: 0,
                size: k.size,
                apparent: k.apparent,
                file_count: k.file_count,
                mtime: k.mtime,
                kind: k.kind,
                category,
                flags: k.flags,
            });
            if !grandkids.is_empty() {
                queue.push_back((idx, grandkids, child_ctx));
            }
        }
    }

    cat_totals.sort_by_key(|c| std::cmp::Reverse(c.size));
    (nodes, cat_totals)
}

#[cfg(unix)]
mod platform {
    use std::collections::HashSet;
    use std::fs::Metadata;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    use std::sync::Mutex;

    pub fn allocated(md: &Metadata) -> u64 {
        // st_blocks is always in 512-byte units.
        md.blocks() * 512
    }

    pub fn device(md: &Metadata) -> Option<u64> {
        Some(md.dev())
    }

    pub fn allowed_devices(root: &Path, md: &Metadata) -> Vec<u64> {
        let mut v = vec![md.dev()];
        if cfg!(target_os = "macos") && root == Path::new("/") {
            // APFS: system volume + data volume joined by firmlinks.
            if let Ok(d) = std::fs::metadata("/System/Volumes/Data") {
                v.push(d.dev());
            }
        }
        v
    }

    pub fn is_reparse_point(_md: &Metadata) -> bool {
        false
    }

    /// Returns true when this inode was already counted via another hard link.
    pub fn is_counted_hardlink(md: &Metadata, seen: &Mutex<HashSet<(u64, u64)>>) -> bool {
        if md.nlink() <= 1 {
            return false;
        }
        let key = (md.dev(), md.ino());
        let mut set = seen.lock().unwrap();
        !set.insert(key)
    }
}

#[cfg(windows)]
mod platform {
    use std::collections::HashSet;
    use std::fs::Metadata;
    use std::os::windows::fs::MetadataExt;
    use std::path::Path;
    use std::sync::Mutex;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const CLUSTER: u64 = 4096;

    pub fn allocated(md: &Metadata) -> u64 {
        // Approximation: round to the default NTFS cluster size.
        // TODO: GetCompressedFileSizeW for compressed/sparse files.
        md.len().div_ceil(CLUSTER) * CLUSTER
    }

    pub fn device(_md: &Metadata) -> Option<u64> {
        None // volume mount points are reparse points and are skipped anyway
    }

    pub fn allowed_devices(_root: &Path, _md: &Metadata) -> Vec<u64> {
        Vec::new()
    }

    pub fn is_reparse_point(md: &Metadata) -> bool {
        md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    pub fn is_counted_hardlink(_md: &Metadata, _seen: &Mutex<HashSet<(u64, u64)>>) -> bool {
        // TODO: number_of_links/file_index are unstable in std; use windows-sys.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: usize) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![7u8; bytes]).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("a/b")).unwrap();
        fs::create_dir_all(r.join("node_modules/pkg")).unwrap();
        write(&r.join("a/big.mp4"), 200_000);
        write(&r.join("a/b/small.txt"), 10);
        write(&r.join("node_modules/pkg/index.js"), 50_000);
        write(&r.join("top.zip"), 100_000);
        t
    }

    #[test]
    fn scans_and_aggregates() {
        let t = fixture();
        let tree = scan(t.path(), &ScanOptions::default(), &ScanProgress::new()).unwrap();
        let root = tree.root();
        assert_eq!(root.file_count, 4);
        assert_eq!(root.apparent, 350_010);
        assert!(root.size >= 350_010);
        // children sorted by size, desc
        let kids = tree.children(Tree::ROOT);
        assert!(kids.windows(2).all(|w| w[0].size >= w[1].size));
        assert_eq!(&*kids[0].name, "a");
        // path reconstruction + lookup
        let id = tree.find_path(&t.path().canonicalize().unwrap().join("a/b/small.txt")).unwrap();
        assert_eq!(tree.path(id).file_name().unwrap(), "small.txt");
        // categories
        let nm = tree.find_path(&tree.root_path.join("node_modules/pkg/index.js")).unwrap();
        assert_eq!(tree.nodes[nm as usize].category, Category::DevArtifacts);
        let video = tree.categories.iter().find(|c| c.category == Some(Category::Video)).unwrap();
        assert_eq!(video.count, 1);
        // top files
        let top = tree.top_files(2, None);
        assert_eq!(&*tree.nodes[top[0] as usize].name, "big.mp4");
        assert_eq!(&*tree.nodes[top[1] as usize].name, "top.zip");
        let under_a = tree.top_files(10, tree.find_path(&tree.root_path.join("a")));
        assert_eq!(under_a.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn hardlinks_counted_once_and_symlink_loops_ignored() {
        let t = fixture();
        let r = t.path();
        fs::hard_link(r.join("a/big.mp4"), r.join("big-link.mp4")).unwrap();
        std::os::unix::fs::symlink(r, r.join("a/loop")).unwrap();
        let tree = scan(r, &ScanOptions::default(), &ScanProgress::new()).unwrap();
        assert_eq!(tree.root().apparent, 350_010, "hard link must not double count");
        assert_eq!(tree.root().file_count, 5);
        let dups = tree.nodes.iter().filter(|n| n.has(flags::HARDLINK_DUP)).count();
        assert_eq!(dups, 1);
        let link = tree.find_path(&tree.root_path.join("a/loop")).unwrap();
        assert_eq!(tree.nodes[link as usize].kind, NodeKind::Symlink);
    }

    #[cfg(unix)]
    #[test]
    fn sparse_file_uses_allocated_size() {
        let t = tempfile::tempdir().unwrap();
        let f = fs::File::create(t.path().join("sparse.img")).unwrap();
        f.set_len(1 << 30).unwrap(); // 1 GiB logical, ~0 allocated
        let tree = scan(t.path(), &ScanOptions::default(), &ScanProgress::new()).unwrap();
        assert_eq!(tree.root().apparent, 1 << 30);
        assert!(tree.root().size < 1 << 20, "allocated {}", tree.root().size);
    }

    #[test]
    fn excludes_and_cancel() {
        let t = fixture();
        let root = t.path().canonicalize().unwrap();
        let opts = ScanOptions {
            exclude: vec![root.join("a")],
            ..Default::default()
        };
        let tree = scan(&root, &opts, &ScanProgress::new()).unwrap();
        let a = tree.find_path(&root.join("a")).unwrap();
        assert!(tree.nodes[a as usize].has(flags::SKIPPED));
        assert_eq!(tree.root().file_count, 2);

        let p = ScanProgress::new();
        p.cancel();
        assert!(matches!(scan(&root, &ScanOptions::default(), &p), Err(ScanError::Cancelled)));
    }

    #[test]
    fn root_errors() {
        let p = ScanProgress::new();
        assert!(matches!(
            scan(Path::new("/definitely/not/here"), &ScanOptions::default(), &p),
            Err(ScanError::Root { .. })
        ));
    }
}
