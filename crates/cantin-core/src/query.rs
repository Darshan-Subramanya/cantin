//! Structured search over a scanned [`Tree`]. Both the filter UI and the AI
//! search produce a [`Query`]; this module is the single place that runs it.

use crate::categories::Category;
use crate::tree::{NodeId, NodeKind, Tree};
use globset::{GlobBuilder, GlobMatcher};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindFilter {
    #[default]
    Any,
    File,
    Dir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    SizeDesc,
    SizeAsc,
    Newest,
    Oldest,
    Name,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    /// Case-insensitive substring of the file name.
    pub name_contains: Option<String>,
    /// Glob on the file name, e.g. `*.mp4` or `IMG_*`.
    pub glob: Option<String>,
    /// Regex on the file name.
    pub regex: Option<String>,
    /// Extensions without dot, case-insensitive.
    pub extensions: Vec<String>,
    pub categories: Vec<Category>,
    pub kind: KindFilter,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Unix seconds, inclusive.
    pub modified_after: Option<i64>,
    /// Unix seconds, exclusive.
    pub modified_before: Option<i64>,
    /// Case-insensitive substring that must appear in the full path.
    pub path_contains: Option<String>,
    /// Restrict to the subtree of this node.
    pub under: Option<NodeId>,
    pub sort: SortBy,
    /// Max results (0 = default of 500).
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub mtime: i64,
    pub kind: NodeKind,
    pub category: Category,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    /// Total matches before `limit`.
    pub total: usize,
    pub total_size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("invalid glob: {0}")]
    Glob(#[from] globset::Error),
    #[error("invalid regex: {0}")]
    Regex(#[from] regex::Error),
}

fn nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

struct Compiled {
    name: Option<String>,
    path: Option<String>,
    glob: Option<GlobMatcher>,
    regex: Option<Regex>,
    exts: Vec<String>,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        let d = Query {
            sort: self.sort,
            limit: self.limit,
            under: self.under,
            ..Default::default()
        };
        *self == d
    }

    fn compile(&self) -> Result<Compiled, QueryError> {
        Ok(Compiled {
            name: nonempty(&self.name_contains).map(str::to_lowercase),
            path: nonempty(&self.path_contains).map(str::to_lowercase),
            glob: match nonempty(&self.glob) {
                Some(g) => Some(GlobBuilder::new(g).case_insensitive(true).build()?.compile_matcher()),
                None => None,
            },
            regex: match nonempty(&self.regex) {
                Some(r) => Some(RegexBuilder::new(r).case_insensitive(true).size_limit(1 << 20).build()?),
                None => None,
            },
            exts: self
                .extensions
                .iter()
                .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        })
    }

    pub fn run(&self, tree: &Tree) -> Result<SearchResult, QueryError> {
        let c = self.compile()?;
        let mut ids: Vec<NodeId> = Vec::new();
        for (i, n) in tree.nodes.iter().enumerate().skip(1) {
            let id = i as NodeId;
            match self.kind {
                KindFilter::File if n.kind != NodeKind::File => continue,
                KindFilter::Dir if n.kind != NodeKind::Dir => continue,
                _ => {}
            }
            if self.min_size.is_some_and(|m| n.size < m) || self.max_size.is_some_and(|m| n.size > m) {
                continue;
            }
            if self.modified_after.is_some_and(|t| n.mtime < t)
                || self.modified_before.is_some_and(|t| n.mtime >= t)
            {
                continue;
            }
            if !self.categories.is_empty() && !self.categories.contains(&n.category) {
                continue;
            }
            if !c.exts.is_empty() {
                match n.extension() {
                    Some(e) if c.exts.iter().any(|x| x.eq_ignore_ascii_case(e)) => {}
                    _ => continue,
                }
            }
            if let Some(s) = &c.name {
                if !n.name.to_lowercase().contains(s) {
                    continue;
                }
            }
            if let Some(g) = &c.glob {
                if !g.is_match(&*n.name) {
                    continue;
                }
            }
            if let Some(r) = &c.regex {
                if !r.is_match(&n.name) {
                    continue;
                }
            }
            if let Some(u) = self.under {
                if !tree.is_descendant(id, u) {
                    continue;
                }
            }
            if let Some(p) = &c.path {
                if !tree.path(id).to_string_lossy().to_lowercase().contains(p) {
                    continue;
                }
            }
            ids.push(id);
        }

        let total = ids.len();
        let total_size = ids.iter().map(|&i| tree.nodes[i as usize].size).sum();
        let node = |i: &NodeId| &tree.nodes[*i as usize];
        match self.sort {
            SortBy::SizeDesc => ids.sort_by(|a, b| node(b).size.cmp(&node(a).size)),
            SortBy::SizeAsc => ids.sort_by(|a, b| node(a).size.cmp(&node(b).size)),
            SortBy::Newest => ids.sort_by(|a, b| node(b).mtime.cmp(&node(a).mtime)),
            SortBy::Oldest => ids.sort_by(|a, b| node(a).mtime.cmp(&node(b).mtime)),
            SortBy::Name => ids.sort_by(|a, b| node(a).name.to_lowercase().cmp(&node(b).name.to_lowercase())),
        }
        let limit = if self.limit == 0 { 500 } else { self.limit };
        let hits = ids
            .into_iter()
            .take(limit)
            .map(|id| {
                let n = &tree.nodes[id as usize];
                SearchHit {
                    id,
                    name: n.name.to_string(),
                    path: tree.path(id).to_string_lossy().into_owned(),
                    size: n.size,
                    mtime: n.mtime,
                    kind: n.kind,
                    category: n.category,
                }
            })
            .collect();
        Ok(SearchResult { hits, total, total_size })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{scan, ScanOptions, ScanProgress};
    use std::fs;

    fn tree() -> (tempfile::TempDir, Tree) {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        fs::create_dir_all(r.join("Movies/old")).unwrap();
        fs::write(r.join("Movies/trip.MP4"), vec![1u8; 300_000]).unwrap();
        fs::write(r.join("Movies/old/clip.mkv"), vec![1u8; 50_000]).unwrap();
        fs::write(r.join("IMG_001.jpg"), vec![1u8; 20_000]).unwrap();
        fs::write(r.join("notes.txt"), b"hi").unwrap();
        let tree = scan(r, &ScanOptions::default(), &ScanProgress::new()).unwrap();
        (t, tree)
    }

    fn names(r: &SearchResult) -> Vec<&str> {
        r.hits.iter().map(|h| h.name.as_str()).collect()
    }

    #[test]
    fn filters() {
        let (_t, tree) = tree();
        let q = Query { categories: vec![Category::Video], ..Default::default() };
        assert_eq!(names(&q.run(&tree).unwrap()), ["trip.MP4", "clip.mkv"]);

        let q = Query { categories: vec![Category::Video], min_size: Some(100_000), ..Default::default() };
        assert_eq!(names(&q.run(&tree).unwrap()), ["trip.MP4"]);

        let q = Query { glob: Some("img_*".into()), ..Default::default() };
        assert_eq!(names(&q.run(&tree).unwrap()), ["IMG_001.jpg"]);

        let q = Query { extensions: vec![".mp4".into()], ..Default::default() };
        assert_eq!(names(&q.run(&tree).unwrap()), ["trip.MP4"]);

        let q = Query { path_contains: Some(format!("{0}old{0}", std::path::MAIN_SEPARATOR)), kind: KindFilter::File, ..Default::default() };
        assert_eq!(names(&q.run(&tree).unwrap()), ["clip.mkv"]);

        let q = Query { kind: KindFilter::Dir, name_contains: Some("movie".into()), ..Default::default() };
        let r = q.run(&tree).unwrap();
        assert_eq!(names(&r), ["Movies"]);

        let q = Query { modified_before: Some(1), ..Default::default() };
        assert_eq!(q.run(&tree).unwrap().total, 0);

        let q = Query { kind: KindFilter::File, sort: SortBy::Name, limit: 2, ..Default::default() };
        let r = q.run(&tree).unwrap();
        assert_eq!(r.total, 4);
        assert_eq!(names(&r), ["clip.mkv", "IMG_001.jpg"]);
    }

    #[test]
    fn bad_regex_is_an_error() {
        let (_t, tree) = tree();
        let q = Query { regex: Some("(".into()), ..Default::default() };
        assert!(q.run(&tree).is_err());
    }

    #[test]
    fn serde_defaults() {
        let q: Query = serde_json::from_str(r#"{"categories":["video"],"min_size":10}"#).unwrap();
        assert_eq!(q.categories, vec![Category::Video]);
        assert_eq!(q.min_size, Some(10));
        assert_eq!(q.sort, SortBy::SizeDesc);
    }
}
