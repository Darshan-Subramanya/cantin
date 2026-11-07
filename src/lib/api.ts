// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type NodeKind = "file" | "dir" | "symlink" | "other";
export type Category =
  | "video" | "image" | "audio" | "document" | "archive" | "code" | "installer"
  | "disk_image" | "dev_artifacts" | "cache" | "log" | "other";

export const FLAG_HARDLINK_DUP = 1;
export const FLAG_UNREADABLE = 2;
export const FLAG_SKIPPED = 4;

export interface NodeDto {
  id: number;
  name: string;
  size: number;
  apparent: number;
  files: number;
  mtime: number;
  kind: NodeKind;
  category: Category;
  flags: number;
  children: number;
}

export interface NodeDetail {
  node: NodeDto;
  path: string;
  ancestry: NodeDto[];
}

export interface LayoutNode {
  id: number | null;
  name: string;
  size: number;
  kind: NodeKind;
  category: Category;
  children?: LayoutNode[];
}

export interface ScanStats {
  root: string;
  total_size: number;
  total_apparent: number;
  files: number;
  dirs: number;
  errors: number;
  elapsed_ms: number;
}

export interface ScanProgress {
  files: number;
  dirs: number;
  bytes: number;
  errors: number;
  current: string;
}

export interface ScanOptions {
  one_filesystem: boolean;
  skip_pseudo: boolean;
  exclude: string[];
}

export interface Drive {
  name: string;
  mount: string;
  total: number;
  free: number;
  removable: boolean;
}

export interface CategoryTotal {
  category: Category | null;
  label: string;
  size: number;
  count: number;
}

export type SortBy = "size_desc" | "size_asc" | "newest" | "oldest" | "name";
export type KindFilter = "any" | "file" | "dir";

export interface Query {
  name_contains?: string | null;
  glob?: string | null;
  regex?: string | null;
  extensions?: string[];
  categories?: Category[];
  kind?: KindFilter;
  min_size?: number | null;
  max_size?: number | null;
  modified_after?: number | null;
  modified_before?: number | null;
  path_contains?: string | null;
  under?: number | null;
  sort?: SortBy;
  limit?: number;
}

export interface SearchHit {
  id: number;
  name: string;
  path: string;
  size: number;
  mtime: number;
  kind: NodeKind;
  category: Category;
}

export interface SearchResult {
  hits: SearchHit[];
  total: number;
  total_size: number;
}

export type Backend =
  | { kind: "ollama"; url: string; model: string }
  | { kind: "openai_compatible"; url: string; model: string };

export interface AiResult {
  query: Query;
  engine: "llm" | "rules";
  warning: string | null;
  result: SearchResult;
}

export interface DupGroup {
  size: number;
  wasted: number;
  files: { id: number; path: string; mtime: number }[];
}

export interface JobProgress {
  done: number;
  total: number;
  bytes: number;
}

export const api = {
  listDrives: () => invoke<Drive[]>("list_drives"),
  homeDir: () => invoke<string | null>("home_dir"),
  startupPath: () => invoke<string | null>("startup_path"),
  startScan: (path: string, options: ScanOptions) => invoke<void>("start_scan", { path, options }),
  cancelScan: () => invoke<void>("cancel_scan"),
  children: (id: number, limit?: number) => invoke<NodeDto[]>("get_children", { id, limit }),
  node: (id: number) => invoke<NodeDetail>("get_node", { id }),
  layout: (id: number, depth: number, maxNodes?: number) =>
    invoke<LayoutNode>("get_layout", { id, depth, maxNodes }),
  topFiles: (limit: number, under?: number | null) =>
    invoke<[NodeDto, string][]>("top_files", { limit, under: under ?? null }),
  categories: () => invoke<CategoryTotal[]>("get_categories"),
  search: (query: Query) => invoke<SearchResult>("search", { query }),
  aiSearch: (prompt: string, backend: Backend | null, limit?: number) =>
    invoke<AiResult>("ai_search", { prompt, backend, limit }),
  aiModels: (backend: Backend) => invoke<string[]>("ai_models", { backend }),
  startDupes: (minSize: number, under?: number | null) =>
    invoke<void>("start_dupes", { minSize, under: under ?? null }),
  cancelDupes: () => invoke<void>("cancel_dupes"),
  nodePath: (id: number) => invoke<string>("node_path", { id }),
};

export const events = {
  scanProgress: (cb: (p: ScanProgress) => void) => listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
  scanDone: (cb: (s: ScanStats) => void) => listen<ScanStats>("scan-done", (e) => cb(e.payload)),
  scanError: (cb: (msg: string) => void) => listen<string>("scan-error", (e) => cb(e.payload)),
  dupesProgress: (cb: (p: JobProgress) => void) => listen<JobProgress>("dupes-progress", (e) => cb(e.payload)),
  dupesDone: (cb: (g: DupGroup[]) => void) => listen<DupGroup[]>("dupes-done", (e) => cb(e.payload)),
  dupesCancelled: (cb: () => void) => listen("dupes-cancelled", () => cb()),
};

export type { UnlistenFn };
