import { useEffect, useMemo, useRef, useState } from "react";
import { hierarchy, treemap, treemapSquarify, type HierarchyRectangularNode } from "d3-hierarchy";
import { api, type LayoutNode } from "../lib/api";
import { CATEGORY_COLOR, CATEGORY_LABEL, formatSize, pct } from "../lib/format";
import { useStore } from "../state/store";
import { cssVar, fitText, shade, useCanvas } from "./useCanvas";
import { Tooltip, type Tip } from "./Tooltip";

type R = HierarchyRectangularNode<LayoutNode>;
const DIR_COLOR = "#6d84b8";

export function useLayout(focusId: number, depth: number) {
  const scanGen = useStore((s) => s.scanGen);
  const [data, setData] = useState<LayoutNode | null>(null);
  useEffect(() => {
    let alive = true;
    api.layout(focusId, depth).then((d) => alive && setData(d)).catch(() => alive && setData(null));
    return () => {
      alive = false;
    };
  }, [focusId, depth, scanGen]);
  return data;
}

export function Treemap() {
  const focusId = useStore((s) => s.focusId);
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const data = useLayout(focusId, 3);
  const { wrapRef, canvasRef, size, getCtx } = useCanvas();
  const [hover, setHover] = useState<R | null>(null);
  const [tip, setTip] = useState<Tip | null>(null);
  const leavesRef = useRef<R[]>([]);

  const root = useMemo(() => {
    if (!data || !size.w || !size.h) return null;
    const h = hierarchy(data)
      .sum((d) => (d.children && d.children.length ? 0 : Math.max(d.size, 0)))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    return treemap<LayoutNode>()
      .tile(treemapSquarify.ratio(1.3))
      .size([size.w, size.h])
      .paddingOuter(3)
      .paddingInner(1)
      .paddingTop((n) => (n.depth === 0 ? 3 : n.x1 - n.x0 > 60 && n.y1 - n.y0 > 36 ? 18 : 3))
      .round(true)(h);
  }, [data, size.w, size.h]);

  useEffect(() => {
    const ctx = getCtx();
    if (!ctx || !root) return;
    const text = cssVar("--text");
    const bg = cssVar("--panel-2");
    ctx.font = "11px " + cssVar("--font");
    ctx.textBaseline = "middle";
    const nodes = root.descendants();
    leavesRef.current = [];
    for (const n of nodes) {
      const w = n.x1 - n.x0, h = n.y1 - n.y0;
      if (w < 1 || h < 1) continue;
      const d = n.data;
      const isGroup = !!n.children?.length;
      if (n.depth === 0) {
        ctx.fillStyle = bg;
        ctx.fillRect(n.x0, n.y0, w, h);
        continue;
      }
      if (isGroup) {
        ctx.fillStyle = shade(DIR_COLOR, -0.35 + n.depth * 0.12);
        ctx.fillRect(n.x0, n.y0, w, h);
        if (w > 60 && h > 36) {
          ctx.fillStyle = "#fff";
          const label = fitText(ctx, `${d.name}  ${formatSize(d.size)}`, w - 8);
          if (label) ctx.fillText(label, n.x0 + 4, n.y0 + 9);
        }
      } else {
        leavesRef.current.push(n);
        const base = d.id === null ? "#8a8f9c" : d.kind === "dir" ? DIR_COLOR : CATEGORY_COLOR[d.category];
        const g = ctx.createLinearGradient(n.x0, n.y0, n.x1, n.y1);
        g.addColorStop(0, shade(base, 0.18));
        g.addColorStop(1, shade(base, -0.12));
        ctx.fillStyle = g;
        ctx.fillRect(n.x0, n.y0, w, h);
        if (w > 44 && h > 18) {
          ctx.fillStyle = "rgba(0,0,0,0.75)";
          const label = fitText(ctx, d.name, w - 8);
          if (label) ctx.fillText(label, n.x0 + 4, n.y0 + 10);
          if (h > 32) {
            ctx.fillStyle = "rgba(0,0,0,0.55)";
            const sz = fitText(ctx, formatSize(d.size), w - 8);
            if (sz) ctx.fillText(sz, n.x0 + 4, n.y0 + 24);
          }
        }
      }
    }
    // highlight selected + hovered
    const outline = (n: R | undefined | null, color: string, lw: number) => {
      if (!n) return;
      ctx.strokeStyle = color;
      ctx.lineWidth = lw;
      ctx.strokeRect(n.x0 + lw / 2, n.y0 + lw / 2, n.x1 - n.x0 - lw, n.y1 - n.y0 - lw);
    };
    outline(nodes.find((n) => n.depth > 0 && n.data.id === selectedId), text, 2);
    outline(hover, "#ffffff", 2);
  }, [root, hover, selectedId, getCtx]);

  const hit = (e: React.MouseEvent): R | null => {
    const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
    const x = e.clientX - rect.left, y = e.clientY - rect.top;
    const leaf = leavesRef.current.find((n) => x >= n.x0 && x < n.x1 && y >= n.y0 && y < n.y1);
    if (leaf) return leaf;
    // a group header
    if (!root) return null;
    let found: R | null = null;
    for (const n of root.descendants()) {
      if (n.depth > 0 && x >= n.x0 && x < n.x1 && y >= n.y0 && y < n.y1) found = n;
    }
    return found;
  };

  const total = data?.size ?? 0;
  return (
    <>
      <div className="view">
        <div className="canvas-wrap" ref={wrapRef}>
          <canvas
            ref={canvasRef}
            style={{ cursor: hover?.data.id != null ? "pointer" : "default" }}
            onMouseMove={(e) => {
              const n = hit(e);
              setHover(n);
              setTip(n ? { x: e.clientX, y: e.clientY, node: n.data, share: pct(n.data.size, total) } : null);
            }}
            onMouseLeave={() => {
              setHover(null);
              setTip(null);
            }}
            onClick={(e) => {
              const n = hit(e);
              if (n?.data.id != null) select(n.data.id);
            }}
          />
        </div>
        {!data && <div className="empty">Loading…</div>}
      </div>
      <Legend />
      <Tooltip tip={tip} />
    </>
  );
}

export function Legend() {
  const cats = ["video", "image", "audio", "document", "archive", "code", "installer", "disk_image", "dev_artifacts", "cache", "log", "other"] as const;
  return (
    <div className="legend">
      <span><i style={{ background: DIR_COLOR }} />Folder</span>
      {cats.map((c) => (
        <span key={c}><i style={{ background: CATEGORY_COLOR[c] }} />{CATEGORY_LABEL[c]}</span>
      ))}
    </div>
  );
}
