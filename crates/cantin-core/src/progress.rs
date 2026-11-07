//! Lock-free progress counters shared between a worker and the UI.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

/// Live counters for a running scan. Cheap to update from many threads.
#[derive(Default, Debug)]
pub struct ScanProgress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub current: Mutex<String>,
    cancel: AtomicBool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub current: String,
}

impl ScanProgress {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Relaxed)
    }
    pub fn set_current(&self, s: &str) {
        // try_lock: never block scanner threads for a cosmetic update.
        if let Ok(mut c) = self.current.try_lock() {
            c.clear();
            c.push_str(s);
        }
    }
    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files: self.files.load(Relaxed),
            dirs: self.dirs.load(Relaxed),
            bytes: self.bytes.load(Relaxed),
            errors: self.errors.load(Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }
}

/// Generic progress for secondary jobs (duplicate detection, etc).
#[derive(Default, Debug)]
pub struct JobProgress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub bytes: AtomicU64,
    cancel: AtomicBool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSnapshot {
    pub done: u64,
    pub total: u64,
    pub bytes: u64,
}

impl JobProgress {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Relaxed)
    }
    pub fn snapshot(&self) -> JobSnapshot {
        JobSnapshot {
            done: self.done.load(Relaxed),
            total: self.total.load(Relaxed),
            bytes: self.bytes.load(Relaxed),
        }
    }
}
