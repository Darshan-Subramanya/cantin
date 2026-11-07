import { useState } from "react";
import { api } from "../lib/api";
import { backendFor, useStore, type AiEngine } from "../state/store";

const ENGINES: { id: AiEngine; title: string; desc: string }[] = [
  { id: "rules", title: "Built-in rules", desc: "Instant and needs no download. Understands common phrasing: types, sizes, dates, folders." },
  { id: "ollama", title: "Ollama (local model)", desc: "Uses an open-source model running on your machine through Ollama. Recommended: qwen2.5:3b or llama3.2:3b." },
  { id: "openai_compatible", title: "llama.cpp / LM Studio server", desc: "Any local server with an OpenAI-compatible API, e.g. llama.cpp's llama-server." },
];

export function Settings() {
  const settings = useStore((s) => s.settings);
  const update = useStore((s) => s.updateSettings);
  const close = () => useStore.getState().setSettingsOpen(false);
  const [models, setModels] = useState<string[] | null>(null);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const test = async () => {
    const b = backendFor(settings);
    if (!b) return;
    setStatus({ ok: true, msg: "Connecting…" });
    try {
      const m = await api.aiModels(b);
      setModels(m);
      setStatus({ ok: true, msg: m.length ? `Connected · ${m.length} model(s) available` : "Connected, but no models are installed" });
    } catch (e) {
      setModels(null);
      setStatus({ ok: false, msg: String(e) });
    }
  };

  return (
    <div className="modal-bg" onClick={close}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>Settings</h2>

        <h3>Smart search</h3>
        <p className="muted" style={{ marginTop: 0 }}>
          Your request is translated into filters on this computer. File names and contents are never sent anywhere.
        </p>
        {ENGINES.map((e) => (
          <label key={e.id} className={"radio-card" + (settings.aiEngine === e.id ? " on" : "")}>
            <input type="radio" checked={settings.aiEngine === e.id} onChange={() => { update({ aiEngine: e.id }); setStatus(null); setModels(null); }} />
            <div><b>{e.title}</b><small>{e.desc}</small></div>
          </label>
        ))}

        {settings.aiEngine === "ollama" && (
          <>
            <div className="field"><span>Server URL</span>
              <input type="text" value={settings.ollamaUrl} onChange={(e) => update({ ollamaUrl: e.target.value })} /></div>
            <div className="field"><span>Model</span>
              {models?.length ? (
                <select value={settings.ollamaModel} onChange={(e) => update({ ollamaModel: e.target.value })}>
                  {!models.includes(settings.ollamaModel) && <option>{settings.ollamaModel}</option>}
                  {models.map((m) => <option key={m}>{m}</option>)}
                </select>
              ) : (
                <input type="text" value={settings.ollamaModel} onChange={(e) => update({ ollamaModel: e.target.value })} />
              )}
            </div>
            <p className="muted" style={{ fontSize: 12 }}>
              Don't have it? Install Ollama from ollama.com, then run <span className="mono">ollama pull {settings.ollamaModel}</span>.
            </p>
          </>
        )}
        {settings.aiEngine === "openai_compatible" && (
          <>
            <div className="field"><span>Server URL</span>
              <input type="text" value={settings.openaiUrl} onChange={(e) => update({ openaiUrl: e.target.value })} /></div>
            <div className="field"><span>Model name</span>
              <input type="text" value={settings.openaiModel} onChange={(e) => update({ openaiModel: e.target.value })} /></div>
            <p className="muted" style={{ fontSize: 12 }}>
              Example: <span className="mono">llama-server -hf Qwen/Qwen2.5-3B-Instruct-GGUF --port 8080</span>
            </p>
          </>
        )}
        {settings.aiEngine !== "rules" && (
          <div className="row">
            <button onClick={test}>Test connection</button>
            {status && <span className={status.ok ? "muted" : ""} style={status.ok ? {} : { color: "var(--danger)" }}>{status.msg}</span>}
          </div>
        )}

        <h3>Scanning</h3>
        <label className="row" style={{ marginBottom: 6 }}>
          <input type="checkbox" checked={settings.scan.one_filesystem}
            onChange={(e) => update({ scan: { ...settings.scan, one_filesystem: e.target.checked } })} />
          Stay on one drive (don't follow into other mounted drives)
        </label>
        <label className="row">
          <input type="checkbox" checked={settings.scan.skip_pseudo}
            onChange={(e) => update({ scan: { ...settings.scan, skip_pseudo: e.target.checked } })} />
          Skip system pseudo-folders (/proc, /sys, /dev) and external volumes
        </label>

        <div className="row" style={{ justifyContent: "flex-end", marginTop: 18 }}>
          <button className="primary" onClick={close}>Done</button>
        </div>
      </div>
    </div>
  );
}
