import type { Category, NodeKind } from "../lib/api";
import { copyPath, reveal, revealLabel } from "../lib/actions";
import { CATEGORY_COLOR, CATEGORY_LABEL, dirName, formatDate, formatSize } from "../lib/format";
import { useStore } from "../state/store";
import { NodeIcon } from "./Icons";

export interface FileRow {
  id: number;
  name: string;
  path: string;
  size: number;
  mtime: number;
  kind: NodeKind;
  category: Category;
}

export function FileTable({ rows, total }: { rows: FileRow[]; total?: number }) {
  const select = useStore((s) => s.select);
  const selectedId = useStore((s) => s.selectedId);
  if (!rows.length) return <div className="empty">Nothing found.</div>;
  const max = Math.max(...rows.map((r) => r.size), 1);
  return (
    <table className="list">
      <thead>
        <tr>
          <th style={{ width: "30%" }}>Name</th>
          <th>Location</th>
          <th style={{ width: 140 }}>Type</th>
          <th className="num" style={{ width: 106 }}>Modified</th>
          <th className="num" style={{ width: 130 }}>Size</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.id} className={r.id === selectedId ? "sel" : ""} onClick={() => select(r.id, { focus: false })}
            onDoubleClick={() => select(r.id)}>
            <td>
              <div className="row" style={{ gap: 6, minWidth: 0 }}>
                <NodeIcon node={r} />
                <span className="ellipsis selectable" title={r.name}>{r.name}</span>
              </div>
            </td>
            <td className="loc" title={r.path}>
              <div className="muted mono ellipsis-start loc-text"><bdi>{dirName(r.path)}</bdi></div>
              <div className="row-actions">
                <button className="small" onClick={(e) => { e.stopPropagation(); reveal(r.path); }}>{revealLabel}</button>
                <button className="small" onClick={(e) => { e.stopPropagation(); copyPath(r.path); }}>Copy path</button>
              </div>
            </td>
            <td className="muted">
              {r.kind === "dir" ? "Folder" : (
                <span className="row ellipsis" style={{ gap: 0 }}><span className="dot" style={{ background: CATEGORY_COLOR[r.category] }} />{CATEGORY_LABEL[r.category]}</span>
              )}
            </td>
            <td className="num muted">{formatDate(r.mtime)}</td>
            <td className="num">
              <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
                <div className="sizebar" style={{ width: 50 }}><div style={{ width: `${(r.size / max) * 100}%` }} /></div>
                {formatSize(r.size)}
              </div>
            </td>
          </tr>
        ))}
      </tbody>
      {total !== undefined && total > rows.length && (
        <tfoot><tr><td colSpan={5} className="muted" style={{ textAlign: "center" }}>Showing {rows.length} of {total.toLocaleString()}</td></tr></tfoot>
      )}
    </table>
  );
}
