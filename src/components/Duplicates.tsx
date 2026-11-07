import { useEffect, useState } from "react";
import { api, events } from "../lib/api";
import { copyPath, reveal, revealLabel } from "../lib/actions";
import { formatCount, formatDate, formatSize } from "../lib/format";
import { useStore } from "../state/store";
import { Chevron } from "./Icons";

const SIZES = [
  { label: "≥ 100 KB", v: 100 * 1024 },
  { label: "≥ 1 MB", v: 1024 ** 2 },
  { label: "≥ 10 MB", v: 10 * 1024 ** 2 },
  { label: "≥ 100 MB", v: 100 * 1024 ** 2 },
];

/** Event wiring lives at app level so results arrive even when the tab is hidden. */
export function useDupesEvents() {
  useEffect(() => {
    const subs = [
      events.dupesProgress((progress) =>
        useStore.setState((s) => ({ dupes: { ...s.dupes, progress } }))),
      events.dupesDone((groups) =>
        useStore.setState((s) => ({ dupes: { ...s.dupes, groups, running: false } }))),
      events.dupesCancelled(() =>
        useStore.setState((s) => ({ dupes: { ...s.dupes, running: false, progress: null } }))),
    ];
    return () => subs.forEach((p) => p.then((u) => u()));
  }, []);
}

export function Duplicates() {
  const dupes = useStore((s) => s.dupes);
  const focusId = useStore((s) => s.focusId);
  const select = useStore((s) => s.select);
  const [open, setOpen] = useState<Record<number, boolean>>({});
  const [scoped, setScoped] = useState(false);

  const start = async () => {
    useStore.setState((s) => ({ dupes: { ...s.dupes, running: true, groups: null, progress: null } }));
    setOpen({});
    await api.startDupes(dupes.minSize, scoped ? focusId : null);
  };

  const wasted = dupes.groups?.reduce((a, g) => a + g.wasted, 0) ?? 0;
  const p = dupes.progress;
  return (
    <div className="view scroll">
      <div className="section">
        <div className="row" style={{ flexWrap: "wrap" }}>
          <span className="muted">Files</span>
          <select
            value={dupes.minSize}
            onChange={(e) => useStore.setState((s) => ({ dupes: { ...s.dupes, minSize: Number(e.target.value) } }))}
            disabled={dupes.running}
          >
            {SIZES.map((s) => <option key={s.v} value={s.v}>{s.label}</option>)}
          </select>
          <label className="row muted"><input type="checkbox" checked={scoped} onChange={(e) => setScoped(e.target.checked)} disabled={dupes.running} />only in current folder</label>
          <span style={{ flex: 1 }} />
          {dupes.running ? (
            <button onClick={() => api.cancelDupes()}>Cancel</button>
          ) : (
            <button className="primary" onClick={start}>{dupes.groups ? "Search again" : "Find duplicates"}</button>
          )}
        </div>
        {dupes.running && (
          <div>
            <div className="bar"><div style={{ width: p && p.total ? `${(p.done / p.total) * 100}%` : "2%" }} /></div>
            <div className="muted" style={{ marginTop: 4 }}>
              Comparing {p ? `${formatCount(p.done)} / ${formatCount(p.total)}` : "…"} candidate files (same size → quick hash → full hash)
            </div>
          </div>
        )}
        {!dupes.running && !dupes.groups && (
          <div className="notice">
            Cantin finds files with identical content. It compares sizes first, then a quick 128 KB fingerprint, and only fully reads files that still match. Nothing is deleted.
          </div>
        )}
        {dupes.groups && (
          <div className="row">
            <b>{formatCount(dupes.groups.length)}</b> groups of duplicates ·
            <b>{formatSize(wasted)}</b> could be freed by keeping one copy of each
          </div>
        )}
        {dupes.groups?.map((g, i) => (
          <div key={i} className="dup-group">
            <div className="dup-head" onClick={() => setOpen((o) => ({ ...o, [i]: !o[i] }))}>
              <span className={"chev" + (open[i] ? " open" : "")}><Chevron /></span>
              <b className="ellipsis" style={{ flex: 1 }}>{g.files[0].path.split(/[\\/]/).pop()}</b>
              <span className="muted">{g.files.length} copies × {formatSize(g.size)}</span>
              <span className="num" style={{ width: 90 }}>{formatSize(g.wasted)}</span>
            </div>
            {open[i] && (
              <div className="dup-files">
                {g.files.map((f) => (
                  <div key={f.id} className="dup-file">
                    <span className="mono ellipsis-start selectable" style={{ flex: 1 }} title={f.path}
                      onDoubleClick={() => select(f.id)}><bdi>{f.path}</bdi></span>
                    <span className="muted num">{formatDate(f.mtime)}</span>
                    <button className="small" onClick={() => reveal(f.path)}>{revealLabel}</button>
                    <button className="small" onClick={() => copyPath(f.path)}>Copy</button>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
        {dupes.groups && dupes.groups.length === 0 && <div className="empty">No duplicates found 🎉</div>}
      </div>
    </div>
  );
}
