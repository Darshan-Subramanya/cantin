import { useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { FLAG_HARDLINK_DUP, FLAG_SKIPPED, FLAG_UNREADABLE, type NodeDto } from "../lib/api";
import { formatCount, formatDate, formatSize, pct } from "../lib/format";
import { useStore } from "../state/store";
import { Chevron, NodeIcon } from "./Icons";

interface Row {
  node: NodeDto;
  depth: number;
  parentSize: number;
}

const ROW_H = 28;

export function SizeTree() {
  const root = useStore((s) => s.root);
  const children = useStore((s) => s.children);
  const expanded = useStore((s) => s.expanded);
  const selectedId = useStore((s) => s.selectedId);
  const toggle = useStore((s) => s.toggle);
  const select = useStore((s) => s.select);
  const scrollRef = useRef<HTMLDivElement>(null);

  const rows = useMemo(() => {
    const out: Row[] = [];
    if (!root) return out;
    const walk = (id: number, depth: number, parentSize: number) => {
      for (const n of children[id] ?? []) {
        out.push({ node: n, depth, parentSize });
        if (n.kind === "dir" && expanded[n.id]) walk(n.id, depth + 1, n.size);
      }
    };
    out.push({ node: root, depth: 0, parentSize: root.size });
    if (expanded[root.id]) walk(root.id, 1, root.size);
    return out;
  }, [root, children, expanded]);

  const virt = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: 20,
  });

  // keep the selected row visible when it changes from another view
  const selIndex = rows.findIndex((r) => r.node.id === selectedId);
  useEffect(() => {
    if (selIndex >= 0) virt.scrollToIndex(selIndex, { align: "auto" });
  }, [selIndex]); // eslint-disable-line react-hooks/exhaustive-deps

  const onKey = (e: React.KeyboardEvent) => {
    if (selIndex < 0) return;
    const row = rows[selIndex];
    const go = (i: number) => {
      const r = rows[Math.max(0, Math.min(rows.length - 1, i))];
      if (r) select(r.node.id, { focus: false });
    };
    if (e.key === "ArrowDown") go(selIndex + 1);
    else if (e.key === "ArrowUp") go(selIndex - 1);
    else if (e.key === "ArrowRight" && row.node.kind === "dir" && !expanded[row.node.id]) toggle(row.node.id);
    else if (e.key === "ArrowLeft" && row.node.kind === "dir" && expanded[row.node.id]) toggle(row.node.id);
    else if (e.key === "Enter") select(row.node.id, { focus: true });
    else return;
    e.preventDefault();
  };

  if (!root) return null;
  return (
    <>
      <div className="tree-head">
        <span>Name</span><span>Share</span><span className="num">Size</span><span className="num">Files</span>
      </div>
      <div className="tree" ref={scrollRef} tabIndex={0} onKeyDown={onKey}>
        <div style={{ height: virt.getTotalSize(), position: "relative" }}>
          {virt.getVirtualItems().map((v) => {
            const { node, depth, parentSize } = rows[v.index];
            const isDir = node.kind === "dir" && node.children > 0;
            const open = !!expanded[node.id];
            const frac = parentSize ? node.size / parentSize : 0;
            const flags = node.flags;
            return (
              <div
                key={node.id}
                className={"tree-row" + (node.id === selectedId ? " sel" : "")}
                style={{ transform: `translateY(${v.start}px)` }}
                onClick={() => select(node.id)}
                onDoubleClick={() => isDir && toggle(node.id)}
                title={`${node.name}\n${formatSize(node.size)} on disk · ${formatSize(node.apparent)} apparent\nModified ${formatDate(node.mtime)}`}
              >
                <div className="tree-name" style={{ paddingLeft: 6 + depth * 16 }}>
                  <span
                    className={"chev" + (open ? " open" : "") + (isDir ? " clickable" : "")}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (isDir) toggle(node.id);
                    }}
                  >
                    {isDir && <Chevron />}
                  </span>
                  <NodeIcon node={node} />
                  <span className="ellipsis">{depth === 0 ? node.name : node.name}</span>
                  {flags & FLAG_UNREADABLE ? <span className="badge" title="Permission denied">locked</span> : null}
                  {flags & FLAG_SKIPPED ? <span className="badge" title="Not scanned (other filesystem or excluded)">skipped</span> : null}
                  {flags & FLAG_HARDLINK_DUP ? <span className="badge" title="Hard link: counted once elsewhere">link</span> : null}
                </div>
                <div className="row" style={{ gap: 6 }}>
                  <div className="sizebar" style={{ flex: 1 }}><div style={{ width: `${Math.max(frac * 100, frac > 0 ? 1 : 0)}%` }} /></div>
                  <span className="muted num" style={{ width: 38, fontSize: 11 }}>{depth === 0 ? "" : pct(node.size, parentSize)}</span>
                </div>
                <span className="num">{formatSize(node.size)}</span>
                <span className="num muted">{node.kind === "dir" ? formatCount(node.files) : ""}</span>
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
