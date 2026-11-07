//! Cantin core: fast, cross-platform disk usage scanning and analysis.
//!
//! The crate has no GUI dependencies so it can be shared by the desktop app
//! (`src-tauri`) and the command-line tool (`cantin-cli`).

pub mod ai;
pub mod categories;
pub mod dupes;
pub mod progress;
pub mod query;
pub mod scan;
pub mod tree;
pub mod units;

pub use categories::Category;
pub use dupes::{find_duplicates, DupGroup, DupOptions};
pub use progress::{JobProgress, ProgressSnapshot, ScanProgress};
pub use query::{Query, SearchHit, SortBy};
pub use scan::{scan, ScanError, ScanOptions};
pub use tree::{Node, NodeId, NodeKind, Tree};
