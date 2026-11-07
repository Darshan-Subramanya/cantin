import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useStore } from "../state/store";
import { FileTable, type FileRow } from "./FileTable";

export function TopFiles() {
  const focusId = useStore((s) => s.focusId);
  const scanGen = useStore((s) => s.scanGen);
  const [scoped, setScoped] = useState(true);
  const [rows, setRows] = useState<FileRow[] | null>(null);

  useEffect(() => {
    let alive = true;
    api.topFiles(300, scoped ? focusId : null).then((r) => {
      if (alive) setRows(r.map(([n, path]) => ({ ...n, path })));
    });
    return () => {
      alive = false;
    };
  }, [focusId, scoped, scanGen]);

  return (
    <div className="view scroll">
      <div className="section" style={{ paddingBottom: 0 }}>
        <label className="row muted">
          <input type="checkbox" checked={scoped} onChange={(e) => setScoped(e.target.checked)} />
          Only inside the current folder
        </label>
      </div>
      {rows ? <FileTable rows={rows} /> : <div className="empty">Loading…</div>}
    </div>
  );
}
