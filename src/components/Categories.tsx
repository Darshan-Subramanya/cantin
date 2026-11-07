import { useEffect, useState } from "react";
import { api, type CategoryTotal } from "../lib/api";
import { CATEGORY_COLOR, formatCount, formatSize, pct } from "../lib/format";
import { useStore } from "../state/store";

export function Categories() {
  const scanGen = useStore((s) => s.scanGen);
  const runQuery = useStore((s) => s.runQuery);
  const [cats, setCats] = useState<CategoryTotal[]>([]);

  useEffect(() => {
    api.categories().then(setCats);
  }, [scanGen]);

  const total = cats.reduce((a, c) => a + c.size, 0);
  const max = Math.max(...cats.map((c) => c.size), 1);
  return (
    <div className="view scroll">
      <div className="cat-list">
        <div className="muted" style={{ padding: "0 10px 8px" }}>
          Files grouped by type. Folders like <span className="mono">node_modules</span> and caches are counted as a whole. Click a row to list its files.
        </div>
        {cats.filter((c) => c.count > 0).map((c) => (
          <button
            key={c.label}
            className="cat-row"
            onClick={() => {
              useStore.setState((s) => ({ search: { ...s.search, prompt: c.label, engine: "filter", warning: null } }));
              runQuery({ categories: c.category ? [c.category] : [], kind: "file" });
            }}
          >
            <span className="row"><span className="dot" style={{ background: CATEGORY_COLOR[c.category ?? "other"] }} /><b>{c.label}</b></span>
            <div className="cat-bar"><div style={{ width: `${(c.size / max) * 100}%`, background: CATEGORY_COLOR[c.category ?? "other"] }} /></div>
            <span className="num">{formatSize(c.size)} <span className="muted">{pct(c.size, total)}</span></span>
            <span className="num muted">{formatCount(c.count)} {c.count === 1 ? "file" : "files"}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
