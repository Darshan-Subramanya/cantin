//! Duplicate file detection: size buckets → partial hash → full hash.

use crate::progress::JobProgress;
use crate::tree::{flags, NodeId, NodeKind, Tree};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;

const PARTIAL: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DupOptions {
    /// Ignore files smaller than this many bytes.
    pub min_size: u64,
    /// Restrict to files below this node.
    pub under: Option<NodeId>,
}

impl Default for DupOptions {
    fn default() -> Self {
        Self {
            min_size: 1024 * 1024,
            under: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DupGroup {
    pub size: u64,
    pub hash: String,
    pub ids: Vec<NodeId>,
    /// Bytes reclaimable by keeping one copy.
    pub wasted: u64,
}

pub fn find_duplicates(tree: &Tree, opts: &DupOptions, progress: &JobProgress) -> Vec<DupGroup> {
    // 1. bucket by exact length
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for (i, n) in tree.nodes.iter().enumerate() {
        let id = i as NodeId;
        if n.kind != NodeKind::File || n.has(flags::HARDLINK_DUP) || n.apparent == 0 {
            continue;
        }
        if n.apparent < opts.min_size {
            continue;
        }
        if let Some(u) = opts.under {
            if !tree.is_descendant(id, u) {
                continue;
            }
        }
        by_size.entry(n.apparent).or_default().push(id);
    }
    let candidates: Vec<(u64, Vec<NodeId>)> =
        by_size.into_iter().filter(|(_, v)| v.len() > 1).collect();

    let total: u64 = candidates.iter().map(|(_, v)| v.len() as u64).sum();
    progress.total.store(total, Relaxed);

    // 2. partial hash (head + tail) per bucket, 3. full hash where needed
    let mut groups: Vec<DupGroup> = candidates
        .into_par_iter()
        .flat_map_iter(|(size, ids)| {
            if progress.is_cancelled() {
                return Vec::new();
            }
            let partial = hash_bucket(tree, &ids, |p| partial_hash(p, size), progress);
            let mut out = Vec::new();
            for (h, same) in partial.into_iter().filter(|(_, v)| v.len() > 1) {
                if size <= 2 * PARTIAL {
                    // partial hash already covered the whole file
                    out.push(group(size, format!("{h:032x}"), same));
                    continue;
                }
                let full = hash_bucket(tree, &same, full_hash, progress);
                for (fh, s) in full.into_iter().filter(|(_, v)| v.len() > 1) {
                    out.push(group(size, fh, s));
                }
            }
            progress.done.fetch_add(ids.len() as u64, Relaxed);
            out
        })
        .collect();

    groups.sort_by(|a, b| b.wasted.cmp(&a.wasted));
    groups
}

fn group(size: u64, hash: String, mut ids: Vec<NodeId>) -> DupGroup {
    ids.sort_unstable();
    DupGroup {
        size,
        hash,
        wasted: size * (ids.len() as u64 - 1),
        ids,
    }
}

fn hash_bucket<H: Eq + std::hash::Hash>(
    tree: &Tree,
    ids: &[NodeId],
    f: impl Fn(&Path) -> io::Result<H>,
    progress: &JobProgress,
) -> HashMap<H, Vec<NodeId>> {
    let mut m: HashMap<H, Vec<NodeId>> = HashMap::new();
    for &id in ids {
        if progress.is_cancelled() {
            break;
        }
        if let Ok(h) = f(&tree.path(id)) {
            m.entry(h).or_default().push(id);
        }
    }
    m
}

fn partial_hash(path: &Path, size: u64) -> io::Result<u128> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; PARTIAL as usize];
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    let n = read_full(&mut f, &mut buf)?;
    h.update(&buf[..n]);
    if size > 2 * PARTIAL {
        f.seek(SeekFrom::End(-(PARTIAL as i64)))?;
        let n = read_full(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    } else if size > PARTIAL {
        let n = read_full(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(h.digest128())
}

fn full_hash(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex().to_string())
}

fn read_full(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = f.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{scan, ScanOptions, ScanProgress};
    use std::fs;

    #[test]
    fn finds_duplicates_and_ignores_near_misses() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir(r.join("x")).unwrap();
        let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(r.join("one.bin"), &big).unwrap();
        fs::write(r.join("x/two.bin"), &big).unwrap();
        // same size, same head & tail, different middle → must not match
        let mut near = big.clone();
        near[150_000] ^= 0xff;
        fs::write(r.join("near.bin"), &near).unwrap();
        // small duplicates
        fs::write(r.join("s1.txt"), b"hello world").unwrap();
        fs::write(r.join("s2.txt"), b"hello world").unwrap();
        fs::write(r.join("s3.txt"), b"hello there").unwrap();

        let tree = scan(r, &ScanOptions::default(), &ScanProgress::new()).unwrap();
        let groups = find_duplicates(
            &tree,
            &DupOptions { min_size: 1, under: None },
            &JobProgress::new(),
        );
        assert_eq!(groups.len(), 2, "{groups:?}");
        assert_eq!(groups[0].size, 300_000);
        assert_eq!(groups[0].ids.len(), 2);
        assert_eq!(groups[0].wasted, 300_000);
        let names: Vec<_> = groups[1].ids.iter().map(|&i| &*tree.nodes[i as usize].name).collect();
        assert!(names.contains(&"s1.txt") && names.contains(&"s2.txt"));

        let only_big = find_duplicates(&tree, &DupOptions::default(), &JobProgress::new());
        assert!(only_big.is_empty(), "files under 1 MiB are ignored by default");
    }
}
