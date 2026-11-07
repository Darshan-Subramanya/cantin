import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, events, type NodeDetail } from "./lib/api";
import { copyPath, reveal, revealLabel } from "./lib/actions";
import { formatCount, formatMs, formatSize } from "./lib/format";
import { TABS, useStore, type Tab } from "./state/store";
import { Categories } from "./components/Categories";
import { Duplicates, useDupesEvents } from "./components/Duplicates";
import { IconGear, IconSearch, IconUp, Sparkle } from "./components/Icons";
import { Scanning } from "./components/Scanning";
import { SearchView } from "./components/SearchView";
import { Settings } from "./components/Settings";
import { SizeTree } from "./components/SizeTree";
import { Sunburst } from "./components/Sunburst";
import { TopFiles } from "./components/TopFiles";
import { Treemap } from "./components/Treemap";
import { Welcome } from "./components/Welcome";
import icon from "../src-tauri/icons/128x128.png";

function useScanEvents() {
  useEffect(() => {
    const subs = [
      events.scanProgress((progress) => {
        if (useStore.getState().phase === "scanning") useStore.setState({ progress });
      }),
      events.scanDone((stats) => useStore.getState().onScanDone(stats)),
      events.scanError((error) => {
        const s = useStore.getState();
        // a cancelled rescan keeps the previous result on screen
        useStore.setState(s.root ? { phase: "ready", error: null } : { phase: "idle", error: error.includes("cancelled") ? null : error });
      }),
    ];
    return () => subs.forEach((p) => p.then((u) => u()));
  }, []);
}

export default function App() {
  const phase = useStore((s) => s.phase);
  const settingsOpen = useStore((s) => s.settingsOpen);
  useScanEvents();
  useDupesEvents();

  useEffect(() => {
    api.startupPath().then((p) => { if (p) useStore.getState().startScan(p); }).catch(() => {});
  }, []);

  return (
    <div className="app">
      <Header />
      {phase === "ready" ? <Workspace /> : phase === "scanning" ? <Scanning /> : <Welcome />}
      <StatusBar />
      {settingsOpen && <Settings />}
    </div>
  );
}

function Header() {
  const phase = useStore((s) => s.phase);
  const stats = useStore((s) => s.stats);
  const runPrompt = useStore((s) => s.runPrompt);
  const startScan = useStore((s) => s.startScan);
  const [text, setText] = useState("");
  const [smart, setSmart] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement)?.tagName === "INPUT" || (e.target as HTMLElement)?.tagName === "SELECT";
      if ((e.key === "/" && !typing) || ((e.metaKey || e.ctrlKey) && e.key === "f")) {
        e.preventDefault();
        inputRef.current?.focus();
      } else if (!typing && /^[1-6]$/.test(e.key) && useStore.getState().phase === "ready") {
        useStore.getState().setTab(TABS[Number(e.key) - 1].id);
      } else if (e.key === "Backspace" && !typing && useStore.getState().phase === "ready") {
        const st = useStore.getState();
        if (st.focusId !== 0) api.node(st.focusId).then((d) => {
          const parent = d.ancestry[d.ancestry.length - 2];
          if (parent) st.select(parent.id);
        });
      } else if (e.key === "Escape" && useStore.getState().settingsOpen) {
        useStore.getState().setSettingsOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const pick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") startScan(dir);
  };

  return (
    <header className="header">
      <div className="brand"><img src={icon} alt="" />Cantin</div>
      <div className="spacer" />
      {phase === "ready" && (
        <form className="searchbox" onSubmit={(e) => { e.preventDefault(); runPrompt(text, smart); }}>
          <IconSearch />
          <input
            ref={inputRef}
            type="search"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={smart ? "Ask: “videos over 1GB not opened since 2023”" : "Search file names…"}
            spellCheck={false}
          />
          <button type="button" className={"smart-toggle" + (smart ? " on" : "")} onClick={() => setSmart(!smart)}
            title="Smart search understands plain-English requests">
            <Sparkle /> Smart
          </button>
        </form>
      )}
      <div className="spacer" />
      {phase === "ready" && stats && (
        <>
          <button onClick={() => startScan(stats.root)} title="Scan this folder again">Rescan</button>
          <button onClick={pick}>Open…</button>
        </>
      )}
      <button className="ghost" title="Settings" onClick={() => useStore.getState().setSettingsOpen(true)}><IconGear /></button>
    </header>
  );
}

function Workspace() {
  const tab = useStore((s) => s.tab);
  const setTab = useStore((s) => s.setTab);
  const selectedId = useStore((s) => s.selectedId);
  const [detail, setDetail] = useState<NodeDetail | null>(null);
  useEffect(() => {
    if (selectedId != null) api.node(selectedId).then(setDetail).catch(() => setDetail(null));
  }, [selectedId]);

  const views: Record<Tab, JSX.Element> = {
    treemap: <Treemap />,
    sunburst: <Sunburst />,
    files: <TopFiles />,
    categories: <Categories />,
    duplicates: <Duplicates />,
    search: <SearchView />,
  };
  return (
    <div className="main">
      <section className="pane">
        <div className="pane-head">
          {detail ? (
            <>
              <span className="ellipsis-start mono selectable" style={{ flex: 1 }} title={detail.path}><bdi>{detail.path}</bdi></span>
              <button className="small" onClick={() => reveal(detail.path)}>{revealLabel}</button>
              <button className="small" onClick={() => copyPath(detail.path)}>Copy path</button>
            </>
          ) : <span className="muted">Select an item</span>}
        </div>
        <SizeTree />
      </section>
      <section className="pane">
        <div className="tabs">
          {TABS.map((t, i) => (
            <button key={t.id} className={"tab" + (tab === t.id ? " active" : "")} onClick={() => setTab(t.id)}>
              {t.label}<kbd>{i + 1}</kbd>
            </button>
          ))}
        </div>
        {(tab === "treemap" || tab === "sunburst" || tab === "files") && <Breadcrumbs />}
        {views[tab]}
      </section>
    </div>
  );
}

function Breadcrumbs() {
  const focusId = useStore((s) => s.focusId);
  const select = useStore((s) => s.select);
  const [chain, setChain] = useState<NodeDetail | null>(null);
  useEffect(() => {
    api.node(focusId).then(setChain).catch(() => setChain(null));
  }, [focusId]);
  if (!chain) return <div className="pane-head" />;
  const anc = chain.ancestry;
  const parent = anc.length > 1 ? anc[anc.length - 2] : null;
  return (
    <div className="pane-head" style={{ background: "var(--panel)" }}>
      <button className="small" disabled={!parent} onClick={() => parent && select(parent.id)} title="Up one level (Backspace)">
        <IconUp />
      </button>
      <div className="crumbs">
        {anc.map((a, i) => (
          <span key={a.id} className="row" style={{ gap: 2 }}>
            {i > 0 && <span className="sep">›</span>}
            <button className="ellipsis" onClick={() => select(a.id)} title={a.name}>{a.name}</button>
          </span>
        ))}
      </div>
      <span className="muted num">{formatSize(chain.node.size)} · {formatCount(chain.node.files)} files</span>
    </div>
  );
}

function StatusBar() {
  const stats = useStore((s) => s.stats);
  const phase = useStore((s) => s.phase);
  if (!stats || phase !== "ready") {
    return <footer className="statusbar"><span>Ready</span><span style={{ marginLeft: "auto" }}>Cantin 0.1.0 · open source</span></footer>;
  }
  return (
    <footer className="statusbar">
      <span><b>{formatSize(stats.total_size)}</b> on disk</span>
      <span>{formatSize(stats.total_apparent)} apparent</span>
      <span><b>{formatCount(stats.files)}</b> files · {formatCount(stats.dirs)} folders</span>
      <span>scanned in {formatMs(stats.elapsed_ms)}</span>
      {stats.errors > 0 && <span title="Folders that could not be read (permissions)">{formatCount(stats.errors)} unreadable</span>}
      <span style={{ marginLeft: "auto" }}>Press <b>/</b> to search · <b>1–6</b> switch views</span>
    </footer>
  );
}
