import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type Drive } from "../lib/api";
import { formatSize } from "../lib/format";
import { useStore } from "../state/store";
import { IconDrive, IconFolder } from "./Icons";

export function Welcome() {
  const startScan = useStore((s) => s.startScan);
  const error = useStore((s) => s.error);
  const [drives, setDrives] = useState<Drive[]>([]);
  const [home, setHome] = useState<string | null>(null);

  useEffect(() => {
    api.listDrives().then(setDrives).catch(() => setDrives([]));
    api.homeDir().then(setHome).catch(() => setHome(null));
  }, []);

  const pick = async () => {
    const dir = await open({ directory: true, multiple: false, title: "Choose a folder to analyze" });
    if (typeof dir === "string") startScan(dir);
  };

  return (
    <div className="center">
      <div className="welcome">
        <h1>What's using your disk?</h1>
        <p className="lead">Pick a drive or folder. Cantin scans it in parallel and shows where the space went.</p>
        {error && <div className="notice err" style={{ marginBottom: 16 }}>{error}</div>}
        <div className="cards">
          <button className="card" onClick={pick}>
            <div className="title"><IconFolder /> Choose folder…</div>
            <div className="muted">Scan any folder on this computer</div>
          </button>
          {home && (
            <button className="card" onClick={() => startScan(home)}>
              <div className="title"><IconFolder /> Home</div>
              <div className="muted ellipsis">{home}</div>
            </button>
          )}
          {drives.map((d) => {
            const used = d.total - d.free;
            const frac = d.total ? used / d.total : 0;
            return (
              <button key={d.mount} className="card" onClick={() => startScan(d.mount)}>
                <div className="title"><IconDrive /> <span className="ellipsis">{d.mount}</span></div>
                <div className={"usage" + (frac > 0.9 ? " warn" : "")}><div style={{ width: `${frac * 100}%` }} /></div>
                <div className="muted">{formatSize(d.free)} free of {formatSize(d.total)}</div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
