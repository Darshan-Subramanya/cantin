import type { LayoutNode } from "../lib/api";
import { CATEGORY_LABEL, formatSize } from "../lib/format";

export interface Tip {
  x: number;
  y: number;
  node: LayoutNode;
  share: string;
}

export function Tooltip({ tip }: { tip: Tip | null }) {
  if (!tip) return null;
  const { node } = tip;
  const left = Math.min(tip.x + 14, window.innerWidth - 300);
  const top = Math.min(tip.y + 14, window.innerHeight - 80);
  return (
    <div className="tooltip" style={{ left, top }}>
      <div className="t-name">{node.name}</div>
      <div>
        <b>{formatSize(node.size)}</b> <span className="muted">· {tip.share} of view</span>
      </div>
      <div className="muted">
        {node.id === null ? "Grouped small items" : node.kind === "dir" ? "Folder — click to open" : CATEGORY_LABEL[node.category]}
      </div>
    </div>
  );
}
