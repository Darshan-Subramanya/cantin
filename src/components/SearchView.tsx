import { useEffect, useState } from "react";
import type { Category, KindFilter, Query, SortBy } from "../lib/api";
import {
  ALL_CATEGORIES, CATEGORY_COLOR, CATEGORY_LABEL, dateToUnix, formatDate, formatSize, parseSize,
} from "../lib/format";
import { useStore } from "../state/store";
import { FileTable } from "./FileTable";
import { Sparkle } from "./Icons";

type Chip = { key: string; label: string; remove: (q: Query) => Query };

/** Human-readable, removable chips for every active filter. */
function chipsFor(q: Query): Chip[] {
  const c: Chip[] = [];
  const without = (k: keyof Query) => (x: Query) => ({ ...x, [k]: null });
  if (q.name_contains) c.push({ key: "name", label: `name has “${q.name_contains}”`, remove: without("name_contains") });
  if (q.glob) c.push({ key: "glob", label: `name matches ${q.glob}`, remove: without("glob") });
  if (q.regex) c.push({ key: "regex", label: `regex /${q.regex}/`, remove: without("regex") });
  for (const e of q.extensions ?? [])
    c.push({ key: "ext" + e, label: `.${e}`, remove: (x) => ({ ...x, extensions: (x.extensions ?? []).filter((y) => y !== e) }) });
  for (const cat of q.categories ?? [])
    c.push({ key: "cat" + cat, label: CATEGORY_LABEL[cat], remove: (x) => ({ ...x, categories: (x.categories ?? []).filter((y) => y !== cat) }) });
  if (q.kind && q.kind !== "any") c.push({ key: "kind", label: q.kind === "dir" ? "folders only" : "files only", remove: (x) => ({ ...x, kind: "any" }) });
  if (q.min_size != null) c.push({ key: "min", label: `> ${formatSize(q.min_size)}`, remove: without("min_size") });
  if (q.max_size != null) c.push({ key: "max", label: `< ${formatSize(q.max_size)}`, remove: without("max_size") });
  if (q.modified_after != null) c.push({ key: "after", label: `modified after ${formatDate(q.modified_after)}`, remove: without("modified_after") });
  if (q.modified_before != null) c.push({ key: "before", label: `not modified since ${formatDate(q.modified_before)}`, remove: without("modified_before") });
  if (q.path_contains) c.push({ key: "path", label: `in “${q.path_contains}”`, remove: without("path_contains") });
  return c;
}

export function SearchView() {
  const search = useStore((s) => s.search);
  const runQuery = useStore((s) => s.runQuery);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const [editing, setEditing] = useState(false);
  const q = search.query;

  if (!q && !search.loading) {
    return (
      <div className="view scroll">
        <div className="section">
          <div className="notice">
            <b><Sparkle /> Smart search</b> — type a request such as <i>“videos over 1GB I haven't touched since 2023”</i>,
            <i> “pdfs in Downloads”</i> or <i>“node_modules folders”</i> in the search box above. Cantin turns it into
            filters you can see and edit. It runs entirely on your computer:
            either with built-in rules, or with a local open-source model through Ollama or llama.cpp
            (<a href="#" onClick={(e) => { e.preventDefault(); setSettingsOpen(true); }}>settings</a>).
          </div>
        </div>
      </div>
    );
  }

  const chips = q ? chipsFor(q) : [];
  return (
    <div className="view scroll">
      <div className="section">
        {search.engine && search.engine !== "filter" && (
          <div className="muted" style={{ fontSize: 12 }}>
            <Sparkle /> Understood by {search.engine === "llm" ? "local AI model" : "built-in rules"}
            {search.prompt && <> from “{search.prompt}”</>}
          </div>
        )}
        {search.warning && (
          <div className="notice warn">
            Local model unavailable, used built-in rules instead. <span className="muted">{search.warning}</span>
          </div>
        )}
        {search.error && <div className="notice err">{search.error}</div>}
        <div className="chips">
          {chips.length === 0 && q && <span className="muted">No filters — showing everything</span>}
          {chips.map((c) => (
            <span key={c.key} className="chip">
              {c.label}
              <button title="Remove filter" onClick={() => q && runQuery(c.remove(q))}>×</button>
            </span>
          ))}
          <button className="small ghost" onClick={() => setEditing((e) => !e)}>{editing ? "Hide filters" : "Edit filters"}</button>
          {search.result && (
            <span className="muted" style={{ marginLeft: "auto" }}>
              {search.result.total.toLocaleString()} matches · {formatSize(search.result.total_size)}
            </span>
          )}
        </div>
        {editing && q && <FilterForm query={q} onChange={runQuery} />}
      </div>
      {search.loading ? <div className="empty">Searching…</div> : search.result && <FileTable rows={search.result.hits} total={search.result.total} />}
    </div>
  );
}

function FilterForm({ query, onChange }: { query: Query; onChange: (q: Query) => void }) {
  const [draft, setDraft] = useState<Query>(query);
  useEffect(() => setDraft(query), [query]);
  const apply = (patch: Partial<Query>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    onChange(next);
  };
  const toggleCat = (c: Category) => {
    const cur = draft.categories ?? [];
    apply({ categories: cur.includes(c) ? cur.filter((x) => x !== c) : [...cur, c] });
  };
  const sizeField = (label: string, key: "min_size" | "max_size") => (
    <label>
      {label}
      <input
        type="text"
        placeholder="e.g. 500MB"
        defaultValue={draft[key] != null ? formatSize(draft[key]!) : ""}
        key={String(draft[key])}
        onBlur={(e) => apply({ [key]: e.target.value ? parseSize(e.target.value) : null })}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
    </label>
  );
  const dateField = (label: string, key: "modified_after" | "modified_before") => (
    <label>
      {label}
      <input type="date" value={draft[key] != null ? formatDate(draft[key]!) : ""}
        onChange={(e) => apply({ [key]: dateToUnix(e.target.value) })} />
    </label>
  );
  const textField = (label: string, key: "name_contains" | "path_contains" | "glob", ph: string) => (
    <label>
      {label}
      <input type="text" placeholder={ph} defaultValue={draft[key] ?? ""} key={draft[key] ?? ""}
        onBlur={(e) => apply({ [key]: e.target.value || null })}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
    </label>
  );
  return (
    <div className="filter-form">
      <div className="cat-toggles">
        {ALL_CATEGORIES.map((c) => {
          const on = draft.categories?.includes(c);
          return (
            <button key={c} className={on ? "on" : ""} style={on ? { background: CATEGORY_COLOR[c] } : {}} onClick={() => toggleCat(c)}>
              {CATEGORY_LABEL[c]}
            </button>
          );
        })}
      </div>
      {textField("Name contains", "name_contains", "invoice")}
      {textField("Name pattern", "glob", "*.mp4")}
      {textField("Path contains", "path_contains", "Downloads")}
      <label>
        Extensions
        <input type="text" placeholder="pdf, docx" defaultValue={(draft.extensions ?? []).join(", ")}
          key={(draft.extensions ?? []).join(",")}
          onBlur={(e) => apply({ extensions: e.target.value.split(/[\s,]+/).filter(Boolean) })}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
      </label>
      {sizeField("Larger than", "min_size")}
      {sizeField("Smaller than", "max_size")}
      {dateField("Modified after", "modified_after")}
      {dateField("Not modified since", "modified_before")}
      <label>
        Type
        <select value={draft.kind ?? "any"} onChange={(e) => apply({ kind: e.target.value as KindFilter })}>
          <option value="any">Files & folders</option>
          <option value="file">Files</option>
          <option value="dir">Folders</option>
        </select>
      </label>
      <label>
        Sort
        <select value={draft.sort ?? "size_desc"} onChange={(e) => apply({ sort: e.target.value as SortBy })}>
          <option value="size_desc">Largest first</option>
          <option value="size_asc">Smallest first</option>
          <option value="newest">Newest first</option>
          <option value="oldest">Oldest first</option>
          <option value="name">Name</option>
        </select>
      </label>
    </div>
  );
}
