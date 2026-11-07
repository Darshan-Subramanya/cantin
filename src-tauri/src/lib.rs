//! Tauri command layer: owns the scan result and exposes small, paginated
//! views of it to the web UI (millions of nodes never cross IPC at once).

use cantin_core::ai::{self, Backend};
use cantin_core::query::SearchResult;
use cantin_core::tree::{CategoryTotal, ScanStats};
use cantin_core::{
    find_duplicates, scan, Category, DupOptions, JobProgress, NodeId, NodeKind, Query, ScanOptions,
    ScanProgress, Tree,
};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct AppState {
    tree: RwLock<Option<Arc<Tree>>>,
    scan: Mutex<Option<Arc<ScanProgress>>>,
    dupes: Mutex<Option<Arc<JobProgress>>>,
}

type S<'a> = State<'a, AppState>;
type R<T> = Result<T, String>;

fn tree(state: &S) -> R<Arc<Tree>> {
    state
        .tree
        .read()
        .unwrap()
        .clone()
        .ok_or_else(|| "No scan loaded yet".to_string())
}

// ---------- DTOs ----------

#[derive(Serialize)]
struct NodeDto {
    id: NodeId,
    name: String,
    size: u64,
    apparent: u64,
    files: u64,
    mtime: i64,
    kind: NodeKind,
    category: Category,
    flags: u8,
    children: u32,
}

fn dto(t: &Tree, id: NodeId) -> NodeDto {
    let n = &t.nodes[id as usize];
    NodeDto {
        id,
        name: n.name.to_string(),
        size: n.size,
        apparent: n.apparent,
        files: n.file_count,
        mtime: n.mtime,
        kind: n.kind,
        category: n.category,
        flags: n.flags,
        children: n.child_count,
    }
}

#[derive(Serialize)]
struct NodeDetail {
    node: NodeDto,
    path: String,
    ancestry: Vec<NodeDto>,
}

#[derive(Serialize)]
struct Drive {
    name: String,
    mount: String,
    total: u64,
    free: u64,
    removable: bool,
}

#[derive(Serialize, Clone)]
struct LayoutNode {
    /// `None` for the synthetic "smaller items" bucket.
    id: Option<NodeId>,
    name: String,
    size: u64,
    kind: NodeKind,
    category: Category,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<LayoutNode>,
}

#[derive(Serialize, Clone)]
struct DupGroupDto {
    size: u64,
    wasted: u64,
    files: Vec<DupFile>,
}

#[derive(Serialize, Clone)]
struct DupFile {
    id: NodeId,
    path: String,
    mtime: i64,
}

#[derive(Serialize)]
struct AiResult {
    query: Query,
    /// "llm" or "rules"
    engine: &'static str,
    warning: Option<String>,
    result: SearchResult,
}

// ---------- commands ----------

#[tauri::command]
fn list_drives() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut out: Vec<Drive> = disks
        .list()
        .iter()
        .filter(|d| {
            let m = d.mount_point().to_string_lossy();
            // hide pseudo/system mounts that aren't useful to scan
            !(m.starts_with("/boot") || m.starts_with("/snap") || m.starts_with("/System/Volumes/")
                || m.starts_with("/run") || m.starts_with("/dev") || m.starts_with("/sys")
                || m.starts_with("/proc") || d.total_space() == 0)
        })
        .map(|d| Drive {
            name: d.name().to_string_lossy().into_owned(),
            mount: d.mount_point().to_string_lossy().into_owned(),
            total: d.total_space(),
            free: d.available_space(),
            removable: d.is_removable(),
        })
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

/// Path to scan on launch: `cantin <path>` or `CANTIN_SCAN=<path>`.
#[tauri::command]
fn startup_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|a| !a.starts_with('-'))
        .or_else(|| std::env::var("CANTIN_SCAN").ok())
        .filter(|p| !p.is_empty())
}

#[tauri::command]
fn home_dir() -> Option<String> {
    dirs::home_dir().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn start_scan(app: AppHandle, state: S, path: String, options: Option<ScanOptions>) -> R<()> {
    let progress = Arc::new(ScanProgress::new());
    {
        let mut slot = state.scan.lock().unwrap();
        if let Some(old) = slot.as_ref() {
            old.cancel();
        }
        *slot = Some(progress.clone());
    }
    let opts = options.unwrap_or_default();
    let done = Arc::new(AtomicBool::new(false));

    // progress ticker
    {
        let (app, progress, done) = (app.clone(), progress.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Relaxed) {
                let _ = app.emit("scan-progress", progress.snapshot());
                std::thread::sleep(Duration::from_millis(120));
            }
        });
    }

    std::thread::spawn(move || {
        let result = scan(std::path::Path::new(&path), &opts, &progress);
        done.store(true, Relaxed);
        let state = app.state::<AppState>();
        match result {
            Ok(t) => {
                let stats = t.stats.clone();
                *state.tree.write().unwrap() = Some(Arc::new(t));
                let _ = app.emit("scan-done", stats);
            }
            Err(e) => {
                let _ = app.emit("scan-error", e.to_string());
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: S) {
    if let Some(p) = state.scan.lock().unwrap().as_ref() {
        p.cancel();
    }
}

#[tauri::command]
fn scan_stats(state: S) -> Option<ScanStats> {
    state.tree.read().unwrap().as_ref().map(|t| t.stats.clone())
}

#[tauri::command]
fn get_children(state: S, id: NodeId, limit: Option<usize>) -> R<Vec<NodeDto>> {
    let t = tree(&state)?;
    let limit = limit.unwrap_or(2000);
    Ok(t.child_ids(id).take(limit).map(|c| dto(&t, c)).collect())
}

#[tauri::command]
fn get_node(state: S, id: NodeId) -> R<NodeDetail> {
    let t = tree(&state)?;
    if t.node(id).is_none() {
        return Err(format!("no node {id}"));
    }
    Ok(NodeDetail {
        node: dto(&t, id),
        path: t.path(id).to_string_lossy().into_owned(),
        ancestry: t.ancestry(id).into_iter().map(|a| dto(&t, a)).collect(),
    })
}

/// Pruned nested tree for treemap / sunburst rendering.
#[tauri::command]
fn get_layout(state: S, id: NodeId, depth: Option<u32>, max_nodes: Option<usize>) -> R<LayoutNode> {
    let t = tree(&state)?;
    if t.node(id).is_none() {
        return Err(format!("no node {id}"));
    }
    let depth = depth.unwrap_or(4).min(8);
    let mut budget = max_nodes.unwrap_or(4000);
    let root_size = t.nodes[id as usize].size.max(1);
    // items smaller than this fraction of the view are folded into a bucket
    let min = root_size / 2500;
    Ok(build_layout(&t, id, depth, min, &mut budget))
}

fn build_layout(t: &Tree, id: NodeId, depth: u32, min: u64, budget: &mut usize) -> LayoutNode {
    let n = &t.nodes[id as usize];
    let mut node = LayoutNode {
        id: Some(id),
        name: n.name.to_string(),
        size: n.size,
        kind: n.kind,
        category: n.category,
        children: Vec::new(),
    };
    if depth == 0 || n.child_count == 0 || *budget == 0 {
        return node;
    }
    let mut rest = 0u64;
    let mut rest_count = 0u32;
    for c in t.child_ids(id) {
        let cn = &t.nodes[c as usize];
        if cn.size == 0 {
            continue;
        }
        if cn.size < min || *budget == 0 {
            rest += cn.size;
            rest_count += 1;
            continue;
        }
        *budget -= 1;
        node.children.push(build_layout(t, c, depth - 1, min, budget));
    }
    // a directory's own inode size + anything folded keeps totals exact
    let shown: u64 = node.children.iter().map(|c| c.size).sum();
    let remainder = n.size.saturating_sub(shown);
    if rest_count > 0 && remainder > 0 {
        node.children.push(LayoutNode {
            id: None,
            name: format!("{rest_count} smaller items"),
            size: remainder.max(rest),
            kind: NodeKind::Other,
            category: Category::Other,
            children: Vec::new(),
        });
    }
    node
}

#[tauri::command]
fn top_files(state: S, limit: Option<usize>, under: Option<NodeId>) -> R<Vec<(NodeDto, String)>> {
    let t = tree(&state)?;
    Ok(t
        .top_files(limit.unwrap_or(200), under)
        .into_iter()
        .map(|id| (dto(&t, id), t.path(id).to_string_lossy().into_owned()))
        .collect())
}

#[tauri::command]
fn get_categories(state: S) -> R<Vec<CategoryTotal>> {
    Ok(tree(&state)?.categories.clone())
}

#[tauri::command]
async fn search(app: AppHandle, query: Query) -> R<SearchResult> {
    let state = app.state::<AppState>();
    let t = tree(&state)?;
    tauri::async_runtime::spawn_blocking(move || query.run(&t).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn ai_search(app: AppHandle, prompt: String, backend: Option<Backend>, limit: Option<usize>) -> R<AiResult> {
    let state = app.state::<AppState>();
    let t = tree(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let now = cantin_core::units::now_unix();
        let (mut query, engine, warning) = match &backend {
            Some(b) => match ai::translate(b, &prompt) {
                Ok(q) => (q, "llm", None),
                Err(e) => (ai::heuristic(&prompt, now), "rules", Some(e.to_string())),
            },
            None => (ai::heuristic(&prompt, now), "rules", None),
        };
        query.limit = limit.unwrap_or(500);
        let result = query.run(&t).map_err(|e| e.to_string())?;
        Ok(AiResult { query, engine, warning, result })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn ai_models(backend: Backend) -> R<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || ai::list_models(&backend).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn start_dupes(app: AppHandle, state: S, min_size: Option<u64>, under: Option<NodeId>) -> R<()> {
    let t = tree(&state)?;
    let progress = Arc::new(JobProgress::new());
    {
        let mut slot = state.dupes.lock().unwrap();
        if let Some(old) = slot.as_ref() {
            old.cancel();
        }
        *slot = Some(progress.clone());
    }
    let done = Arc::new(AtomicBool::new(false));
    {
        let (app, progress, done) = (app.clone(), progress.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Relaxed) {
                let _ = app.emit("dupes-progress", progress.snapshot());
                std::thread::sleep(Duration::from_millis(150));
            }
        });
    }
    std::thread::spawn(move || {
        let opts = DupOptions { min_size: min_size.unwrap_or(1 << 20), under };
        let groups = find_duplicates(&t, &opts, &progress);
        done.store(true, Relaxed);
        if progress.is_cancelled() {
            let _ = app.emit("dupes-cancelled", ());
            return;
        }
        let dto: Vec<DupGroupDto> = groups
            .into_iter()
            .take(2000)
            .map(|g| DupGroupDto {
                size: g.size,
                wasted: g.wasted,
                files: g
                    .ids
                    .iter()
                    .map(|&id| DupFile {
                        id,
                        path: t.path(id).to_string_lossy().into_owned(),
                        mtime: t.nodes[id as usize].mtime,
                    })
                    .collect(),
            })
            .collect();
        let _ = app.emit("dupes-done", dto);
    });
    Ok(())
}

#[tauri::command]
fn cancel_dupes(state: S) {
    if let Some(p) = state.dupes.lock().unwrap().as_ref() {
        p.cancel();
    }
}

#[tauri::command]
fn node_path(state: S, id: NodeId) -> R<String> {
    Ok(tree(&state)?.path(id).to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_drives,
            home_dir,
            startup_path,
            start_scan,
            cancel_scan,
            scan_stats,
            get_children,
            get_node,
            get_layout,
            top_files,
            get_categories,
            search,
            ai_search,
            ai_models,
            start_dupes,
            cancel_dupes,
            node_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cantin");
}
