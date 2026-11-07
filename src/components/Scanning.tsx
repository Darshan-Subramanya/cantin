import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import { formatCount, formatMs, formatSize } from "../lib/format";
import { useStore } from "../state/store";

export function Scanning() {
  const progress = useStore((s) => s.progress);
  const path = useStore((s) => s.scanPath);
  const started = useRef(Date.now());
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="center">
      <div className="card progress-card">
        <div className="title">Scanning <span className="mono ellipsis">{path}</span></div>
        <div className="indeterminate" />
        <div className="row" style={{ gap: 28, marginTop: 6 }}>
          <div><div className="bigstat">{formatSize(progress?.bytes ?? 0)}</div><div className="muted">found</div></div>
          <div><div className="bigstat">{formatCount(progress?.files ?? 0)}</div><div className="muted">files</div></div>
          <div><div className="bigstat">{formatCount(progress?.dirs ?? 0)}</div><div className="muted">folders</div></div>
          <div><div className="bigstat">{formatMs(now - started.current)}</div><div className="muted">elapsed</div></div>
        </div>
        <div className="mono muted ellipsis" title={progress?.current}>{progress?.current || "…"}</div>
        {!!progress?.errors && <div className="muted">{formatCount(progress.errors)} items could not be read</div>}
        <div className="row" style={{ justifyContent: "flex-end" }}>
          <button onClick={() => api.cancelScan()}>Cancel</button>
        </div>
      </div>
    </div>
  );
}
