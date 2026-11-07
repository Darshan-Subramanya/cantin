import type { Category } from "./api";

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatSize(bytes: number): string {
  if (!isFinite(bytes) || bytes < 0) return "—";
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  if (i === 0) return `${bytes} B`;
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${UNITS[i]}`;
}

export function parseSize(s: string): number | null {
  const m = s.trim().toLowerCase().match(/^([\d.]+)\s*(b|k|kb|kib|m|mb|mib|g|gb|gib|t|tb|tib)?$/);
  if (!m) return null;
  const n = parseFloat(m[1]);
  const mult: Record<string, number> = { b: 1, k: 1024, m: 1024 ** 2, g: 1024 ** 3, t: 1024 ** 4 };
  return Math.round(n * mult[(m[2] ?? "b")[0]]);
}

export function formatCount(n: number): string {
  return n.toLocaleString();
}

export function formatDate(unix: number): string {
  if (!unix) return "—";
  return new Date(unix * 1000).toISOString().slice(0, 10);
}

export function dateToUnix(s: string): number | null {
  if (!s) return null;
  const t = Date.parse(s + "T00:00:00Z");
  return isNaN(t) ? null : Math.floor(t / 1000);
}

export function pct(part: number, whole: number): string {
  if (!whole) return "0%";
  const p = (part * 100) / whole;
  return p >= 10 ? `${p.toFixed(0)}%` : `${p.toFixed(1)}%`;
}

export function formatMs(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

export const CATEGORY_LABEL: Record<Category, string> = {
  video: "Videos",
  image: "Images",
  audio: "Audio",
  document: "Documents",
  archive: "Archives",
  code: "Code",
  installer: "Apps & installers",
  disk_image: "Disk & VM images",
  dev_artifacts: "Dev dependencies",
  cache: "Caches & temp",
  log: "Logs",
  other: "Other",
};

export const CATEGORY_COLOR: Record<Category, string> = {
  video: "#e8607a",
  image: "#f2a33a",
  audio: "#b57ef0",
  document: "#4f9cf0",
  archive: "#c98a5a",
  code: "#3fc1a5",
  installer: "#7f8cf5",
  disk_image: "#e56fd0",
  dev_artifacts: "#8bc34a",
  cache: "#9aa5b8",
  log: "#d6c34a",
  other: "#6b7a99",
};

export const ALL_CATEGORIES = Object.keys(CATEGORY_LABEL) as Category[];

export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return i > 0 ? path.slice(0, i) : path;
}
