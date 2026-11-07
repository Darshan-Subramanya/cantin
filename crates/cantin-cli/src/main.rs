//! `cantin` — command-line front end to cantin-core (handy for scripting and benchmarks).

use cantin_core::ai::{self, Backend};
use cantin_core::units::format_size;
use cantin_core::{find_duplicates, scan, DupOptions, JobProgress, Query, ScanOptions, ScanProgress, Tree};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "cantin", version, about = "Fast cross-platform storage analyzer")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan a folder and print the largest entries.
    Scan {
        path: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Number of entries / top files to show.
        #[arg(short, long, default_value_t = 15)]
        top: usize,
        /// Cross filesystem boundaries.
        #[arg(long)]
        cross_fs: bool,
    },
    /// Find duplicate files.
    Dupes {
        path: PathBuf,
        /// Minimum file size, e.g. 1MB.
        #[arg(long, default_value = "1MB")]
        min_size: String,
        #[arg(long)]
        json: bool,
    },
    /// Search with a natural-language request, e.g. "videos over 1GB".
    Ask {
        path: PathBuf,
        prompt: String,
        /// Use a local Ollama model (e.g. qwen2.5:3b). Without it, the built-in rule parser is used.
        #[arg(long)]
        ollama: Option<String>,
        /// Use an OpenAI-compatible local server URL (llama.cpp llama-server, LM Studio).
        #[arg(long)]
        openai_url: Option<String>,
        #[arg(long, default_value = "local")]
        model: String,
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
}

fn run_scan(path: &std::path::Path, opts: ScanOptions, quiet: bool) -> Tree {
    let progress = Arc::new(ScanProgress::new());
    let p2 = progress.clone();
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let d2 = done.clone();
    let reporter = std::thread::spawn(move || {
        while !d2.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(200));
            if !quiet {
                let s = p2.snapshot();
                eprint!("\r\x1b[2Kscanning… {} files, {}", s.files, format_size(s.bytes));
            }
        }
    });
    let tree = scan(path, &opts, &progress);
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    reporter.join().ok();
    if !quiet {
        eprint!("\r\x1b[2K");
    }
    match tree {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Scan { path, json, top, cross_fs } => {
            let opts = ScanOptions { one_filesystem: !cross_fs, ..Default::default() };
            let tree = run_scan(&path, opts, json);
            if json {
                let children: Vec<_> = tree
                    .children(Tree::ROOT)
                    .iter()
                    .take(top)
                    .map(|n| serde_json::json!({"name": &*n.name, "size": n.size, "files": n.file_count, "kind": n.kind}))
                    .collect();
                let top_files: Vec<_> = tree
                    .top_files(top, None)
                    .into_iter()
                    .map(|id| serde_json::json!({"path": tree.path(id), "size": tree.nodes[id as usize].size}))
                    .collect();
                let out = serde_json::json!({
                    "stats": tree.stats, "children": children,
                    "top_files": top_files, "categories": tree.categories,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                return;
            }
            let s = &tree.stats;
            println!(
                "{}  {} on disk ({} apparent), {} files, {} dirs, {} errors, {} ms",
                s.root, format_size(s.total_size), format_size(s.total_apparent), s.files, s.dirs, s.errors, s.elapsed_ms
            );
            let total = s.total_size.max(1);
            println!("\nLargest entries:");
            for n in tree.children(Tree::ROOT).iter().take(top) {
                let pct = n.size as f64 * 100.0 / total as f64;
                let bar = "█".repeat((pct / 4.0).round() as usize);
                let slash = if n.is_dir() { "/" } else { "" };
                println!("{:>10} {:5.1}% {:<25} {}{}", format_size(n.size), pct, bar, n.name, slash);
            }
            println!("\nLargest files:");
            for id in tree.top_files(top, None) {
                println!("{:>10}  {}", format_size(tree.nodes[id as usize].size), tree.path(id).display());
            }
            println!("\nBy category:");
            for c in tree.categories.iter().filter(|c| c.count > 0) {
                println!("{:>10}  {:<28} {} files", format_size(c.size), c.label, c.count);
            }
        }
        Cmd::Dupes { path, min_size, json } => {
            let tree = run_scan(&path, ScanOptions::default(), json);
            let min_size = cantin_core::units::parse_size(&min_size).unwrap_or(1 << 20);
            let groups = find_duplicates(&tree, &DupOptions { min_size, under: None }, &JobProgress::new());
            if json {
                let v: Vec<_> = groups
                    .iter()
                    .map(|g| serde_json::json!({"size": g.size, "wasted": g.wasted,
                        "paths": g.ids.iter().map(|&i| tree.path(i)).collect::<Vec<_>>()}))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
                return;
            }
            let wasted: u64 = groups.iter().map(|g| g.wasted).sum();
            println!("{} duplicate groups, {} reclaimable", groups.len(), format_size(wasted));
            for g in groups.iter().take(30) {
                println!("\n{} × {}", g.ids.len(), format_size(g.size));
                for &id in &g.ids {
                    println!("    {}", tree.path(id).display());
                }
            }
        }
        Cmd::Ask { path, prompt, ollama, openai_url, model, limit } => {
            let backend = match (ollama, openai_url) {
                (Some(m), _) => Some(Backend::Ollama { url: "http://localhost:11434".into(), model: m }),
                (None, Some(url)) => Some(Backend::OpenaiCompatible { url, model }),
                _ => None,
            };
            let mut q: Query = match &backend {
                Some(b) => ai::translate(b, &prompt).unwrap_or_else(|e| {
                    eprintln!("AI unavailable ({e}); using built-in parser");
                    ai::heuristic(&prompt, cantin_core::units::now_unix())
                }),
                None => ai::heuristic(&prompt, cantin_core::units::now_unix()),
            };
            q.limit = limit;
            eprintln!("filter: {}", serde_json::to_string(&q).unwrap());
            let tree = run_scan(&path, ScanOptions::default(), false);
            match q.run(&tree) {
                Ok(r) => {
                    println!("{} matches, {} total", r.total, format_size(r.total_size));
                    for h in r.hits {
                        println!("{:>10}  {}  {}", format_size(h.size), cantin_core::units::format_date(h.mtime), h.path);
                    }
                }
                Err(e) => eprintln!("error: {e}"),
            }
        }
    }
}
