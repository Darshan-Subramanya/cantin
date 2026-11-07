import { create } from "zustand";
import {
  api, type AiResult, type Backend, type DupGroup, type JobProgress, type NodeDto, type Query,
  type ScanOptions, type ScanProgress, type ScanStats, type SearchResult,
} from "../lib/api";

export type Tab = "treemap" | "sunburst" | "files" | "categories" | "duplicates" | "search";
export const TABS: { id: Tab; label: string }[] = [
  { id: "treemap", label: "Treemap" },
  { id: "sunburst", label: "Sunburst" },
  { id: "files", label: "Largest files" },
  { id: "categories", label: "Categories" },
  { id: "duplicates", label: "Duplicates" },
  { id: "search", label: "Search" },
];

export type AiEngine = "rules" | "ollama" | "openai_compatible";

export interface Settings {
  aiEngine: AiEngine;
  ollamaUrl: string;
  ollamaModel: string;
  openaiUrl: string;
  openaiModel: string;
  scan: ScanOptions;
}

const DEFAULT_SETTINGS: Settings = {
  aiEngine: "rules",
  ollamaUrl: "http://localhost:11434",
  ollamaModel: "qwen2.5:3b",
  openaiUrl: "http://localhost:8080",
  openaiModel: "local",
  scan: { one_filesystem: true, skip_pseudo: true, exclude: [] },
};

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem("cantin.settings");
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return DEFAULT_SETTINGS;
}

export function backendFor(s: Settings): Backend | null {
  if (s.aiEngine === "ollama") return { kind: "ollama", url: s.ollamaUrl, model: s.ollamaModel };
  if (s.aiEngine === "openai_compatible")
    return { kind: "openai_compatible", url: s.openaiUrl, model: s.openaiModel };
  return null;
}

interface SearchState {
  prompt: string;
  query: Query | null;
  result: SearchResult | null;
  engine: AiResult["engine"] | "filter" | null;
  warning: string | null;
  loading: boolean;
  error: string | null;
}

interface DupState {
  running: boolean;
  progress: JobProgress | null;
  groups: DupGroup[] | null;
  minSize: number;
}

interface Store {
  phase: "idle" | "scanning" | "ready" | "error";
  scanPath: string | null;
  progress: ScanProgress | null;
  stats: ScanStats | null;
  error: string | null;
  scanGen: number; // bumps on every completed scan to invalidate caches

  root: NodeDto | null;
  focusId: number; // view root for visualisations
  selectedId: number | null;
  children: Record<number, NodeDto[]>;
  expanded: Record<number, boolean>;
  tab: Tab;
  search: SearchState;
  dupes: DupState;
  settings: Settings;
  settingsOpen: boolean;

  startScan: (path: string) => Promise<void>;
  onScanDone: (s: ScanStats) => Promise<void>;
  setTab: (t: Tab) => void;
  loadChildren: (id: number) => Promise<NodeDto[]>;
  toggle: (id: number) => Promise<void>;
  select: (id: number, opts?: { focus?: boolean }) => Promise<void>;
  setFocus: (id: number) => void;
  runQuery: (q: Query) => Promise<void>;
  runPrompt: (prompt: string, smart: boolean) => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => void;
  setSettingsOpen: (o: boolean) => void;
  reset: () => void;
}

const emptySearch: SearchState = {
  prompt: "", query: null, result: null, engine: null, warning: null, loading: false, error: null,
};

export const useStore = create<Store>((set, get) => ({
  phase: "idle",
  scanPath: null,
  progress: null,
  stats: null,
  error: null,
  scanGen: 0,
  root: null,
  focusId: 0,
  selectedId: null,
  children: {},
  expanded: {},
  tab: "treemap",
  search: emptySearch,
  dupes: { running: false, progress: null, groups: null, minSize: 1024 * 1024 },
  settings: loadSettings(),
  settingsOpen: false,

  startScan: async (path) => {
    set({ phase: "scanning", scanPath: path, progress: null, error: null });
    try {
      await api.startScan(path, get().settings.scan);
    } catch (e) {
      set({ phase: "error", error: String(e) });
    }
  },

  onScanDone: async (stats) => {
    const root = (await api.node(0)).node;
    set((s) => ({
      phase: "ready",
      stats,
      root,
      focusId: 0,
      selectedId: 0,
      children: {},
      expanded: { 0: true },
      search: emptySearch,
      dupes: { ...s.dupes, running: false, progress: null, groups: null },
      scanGen: s.scanGen + 1,
    }));
    await get().loadChildren(0);
  },

  setTab: (tab) => set({ tab }),

  loadChildren: async (id) => {
    const cached = get().children[id];
    if (cached) return cached;
    const kids = await api.children(id);
    set((s) => ({ children: { ...s.children, [id]: kids } }));
    return kids;
  },

  toggle: async (id) => {
    const open = !get().expanded[id];
    if (open) await get().loadChildren(id);
    set((s) => ({ expanded: { ...s.expanded, [id]: open } }));
  },

  // Select a node, expanding the tree down to it. Directories also become the
  // visualisation focus; files focus their parent folder.
  select: async (id, opts = {}) => {
    const detail = await api.node(id);
    const chain = detail.ancestry;
    await Promise.all(chain.slice(0, -1).map((a) => get().loadChildren(a.id)));
    const expanded = { ...get().expanded };
    chain.slice(0, -1).forEach((a) => (expanded[a.id] = true));
    const focusWanted = opts.focus ?? true;
    const focusId = !focusWanted
      ? get().focusId
      : detail.node.kind === "dir"
        ? id
        : chain.length > 1
          ? chain[chain.length - 2].id
          : 0;
    set({ selectedId: id, expanded, focusId });
  },

  setFocus: (id) => set({ focusId: id }),

  runQuery: async (q) => {
    set((s) => ({ tab: "search", search: { ...s.search, loading: true, error: null, query: q } }));
    try {
      const result = await api.search({ limit: 500, ...q });
      set((s) => ({ search: { ...s.search, result, loading: false, engine: s.search.engine ?? "filter" } }));
    } catch (e) {
      set((s) => ({ search: { ...s.search, loading: false, error: String(e) } }));
    }
  },

  runPrompt: async (prompt, smart) => {
    if (!prompt.trim()) return;
    if (!smart) {
      set((s) => ({ search: { ...s.search, prompt, engine: "filter", warning: null } }));
      return get().runQuery({ name_contains: prompt.trim() });
    }
    set((s) => ({ tab: "search", search: { ...s.search, prompt, loading: true, error: null, warning: null } }));
    try {
      const r = await api.aiSearch(prompt, backendFor(get().settings));
      set({
        search: {
          prompt, query: r.query, result: r.result, engine: r.engine,
          warning: r.warning, loading: false, error: null,
        },
      });
    } catch (e) {
      set((s) => ({ search: { ...s.search, loading: false, error: String(e) } }));
    }
  },

  updateSettings: (patch) => {
    const settings = { ...get().settings, ...patch };
    try {
      localStorage.setItem("cantin.settings", JSON.stringify(settings));
    } catch {
      /* ignore */
    }
    set({ settings });
  },

  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  reset: () => set({ phase: "idle", stats: null, root: null, progress: null, error: null }),
}));
