import { useEffect, useMemo, useRef, useState } from "react";
import { hierarchy, partition, type HierarchyRectangularNode } from "d3-hierarchy";
import { api, type LayoutNode } from "../lib/api";
import { CATEGORY_COLOR, formatSize, pct } from "../lib/format";
import { useStore } from "../state/store";
import { Legend, useLayout } from "./Treemap";
import { cssVar, fitText, shade, useCanvas } from "./useCanvas";
import { Tooltip, type Tip } from "./Tooltip";

type R = HierarchyRectangularNode<LayoutNode>;
const RINGS = 5;
const PALETTE = ["#5b8def", "#34c7a0", "#f5a623", "#ed5f74", "#9b7bf0", "#4fc3e8", "#e9c84a", "#e57bd0", "#7fc75a", "#f08a5d"];

export function Sunburst() {
  const focusId = useStore((s) => s.focusId);
  const root = useStore((s) => s.root);
  const select = useStore((s) => s.select);
  const selectedId = useStore((s) => s.selectedId);
  const data = useLayout(focusId, RINGS);
  const { wrapRef, canvasRef, size, getCtx } = useCanvas();
  const [hover, setHover] = useState<R | null>(null);
  const [tip, setTip] = useState<Tip | null>(null);
  const geo = useRef({ cx: 0, cy: 0, ring: 0, inner: 0 });

  const parts = useMemo(() => {
    if (!data) return null;
    const h = hierarchy(data)
      .sum((d) => (d.children && d.children.length ? 0 : Math.max(d.size, 0)))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    return partition<LayoutNode>().size([2 * Math.PI, h.height + 1])(h);
  }, [data]);

  // colour: each top-level child gets a hue; descendants get lighter
  const colorOf = (n: R): string => {
    if (n.data.id === null) return "#8a8f9c";
    let top = n;
    while (top.depth > 1 && top.parent) top = top.parent;
    const idx = top.parent ? top.parent.children!.indexOf(top) : 0;
    const base = n.data.kind === "file" ? CATEGORY_COLOR[n.data.category] : PALETTE[idx % PALETTE.length];
    return shade(base, Math.min(0.5, (n.depth - 1) * 0.12));
  };

  useEffect(() => {
    const ctx = getCtx();
    if (!ctx || !parts) return;
    const radius = Math.min(size.w, size.h) / 2 - 12;
    const ring = radius / (Math.min(parts.height, RINGS) + 1);
    const cx = size.w / 2, cy = size.h / 2;
    geo.current = { cx, cy, ring, inner: ring };
    const panel = cssVar("--panel");
    for (const n of parts.descendants()) {
      if (n.depth === 0 || n.y0 > RINGS) continue;
      const a0 = n.x0 - Math.PI / 2, a1 = n.x1 - Math.PI / 2;
      if (a1 - a0 < 0.002) continue;
      const r0 = n.y0 * ring, r1 = n.y1 * ring - 1;
      ctx.beginPath();
      ctx.arc(cx, cy, r1, a0, a1);
      ctx.arc(cx, cy, r0, a1, a0, true);
      ctx.closePath();
      const sel = n.data.id != null && n.data.id === selectedId;
      ctx.fillStyle = colorOf(n);
      ctx.fill();
      if (n === hover) {
        ctx.fillStyle = "rgba(255,255,255,0.35)";
        ctx.fill();
      }
      ctx.strokeStyle = sel ? cssVar("--text") : panel;
      ctx.lineWidth = sel ? 2.5 : 1;
      ctx.stroke();
    }
    // centre
    ctx.beginPath();
    ctx.arc(cx, cy, ring - 2, 0, Math.PI * 2);
    ctx.fillStyle = cssVar("--panel");
    ctx.fill();
    ctx.fillStyle = cssVar("--text");
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.font = "600 13px " + cssVar("--font");
    const name = fitText(ctx, parts.data.name.split(/[\\/]/).filter(Boolean).pop() || parts.data.name, ring * 1.7);
    if (name) ctx.fillText(name, cx, cy - 9);
    ctx.font = "12px " + cssVar("--font");
    ctx.fillStyle = cssVar("--muted");
    ctx.fillText(formatSize(parts.data.size), cx, cy + 9);
    ctx.textAlign = "start";
  }, [parts, size, hover, selectedId, getCtx]); // eslint-disable-line react-hooks/exhaustive-deps

  const hit = (e: React.MouseEvent): R | "center" | null => {
    if (!parts) return null;
    const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
    const { cx, cy, ring } = geo.current;
    const dx = e.clientX - rect.left - cx, dy = e.clientY - rect.top - cy;
    const r = Math.hypot(dx, dy) / ring;
    if (r < 1) return "center";
    let a = Math.atan2(dy, dx) + Math.PI / 2;
    if (a < 0) a += 2 * Math.PI;
    return parts.descendants().find((n) => n.depth > 0 && r >= n.y0 && r < n.y1 && a >= n.x0 && a < n.x1) ?? null;
  };

  const goUp = async () => {
    if (focusId === 0 || !root) return;
    const d = await api.node(focusId);
    const parent = d.ancestry[d.ancestry.length - 2];
    if (parent) select(parent.id);
  };

  return (
    <>
      <div className="view">
        <div className="canvas-wrap" ref={wrapRef}>
          <canvas
            ref={canvasRef}
            onMouseMove={(e) => {
              const n = hit(e);
              if (n && n !== "center") {
                setHover(n);
                setTip({ x: e.clientX, y: e.clientY, node: n.data, share: pct(n.data.size, parts?.data.size ?? 0) });
              } else {
                setHover(null);
                setTip(null);
              }
            }}
            onMouseLeave={() => {
              setHover(null);
              setTip(null);
            }}
            onClick={(e) => {
              const n = hit(e);
              if (n === "center") goUp();
              else if (n?.data.id != null) select(n.data.id);
            }}
            style={{ cursor: hover ? "pointer" : "default" }}
          />
        </div>
      </div>
      <Legend />
      <Tooltip tip={tip} />
    </>
  );
}
