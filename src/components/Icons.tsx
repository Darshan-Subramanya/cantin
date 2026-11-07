import type { NodeDto } from "../lib/api";
import { CATEGORY_COLOR } from "../lib/format";

const s = { fill: "none", stroke: "currentColor", strokeWidth: 1.6, strokeLinecap: "round", strokeLinejoin: "round" } as const;

export const Chevron = () => (
  <svg width="10" height="10" viewBox="0 0 10 10"><path d="M3 1.5 6.5 5 3 8.5" {...s} /></svg>
);

export function NodeIcon({ node }: { node: Pick<NodeDto, "kind" | "category"> }) {
  if (node.kind === "dir") {
    return (
      <svg className="ico" viewBox="0 0 16 16">
        <path d="M1.5 4.2c0-.7.5-1.2 1.2-1.2h3.4l1.5 1.6h5.7c.7 0 1.2.5 1.2 1.2v6.5c0 .7-.5 1.2-1.2 1.2H2.7c-.7 0-1.2-.5-1.2-1.2z"
          fill="#7ea2f2" opacity="0.9" />
      </svg>
    );
  }
  if (node.kind === "symlink") {
    return (
      <svg className="ico" viewBox="0 0 16 16"><path d="M6 10 10 6M7 4.5l1-1a2.5 2.5 0 0 1 3.5 3.5l-1 1M9 11.5l-1 1A2.5 2.5 0 0 1 4.5 9l1-1" {...s} /></svg>
    );
  }
  const c = CATEGORY_COLOR[node.category] ?? "#888";
  return (
    <svg className="ico" viewBox="0 0 16 16">
      <path d="M3.5 1.5h6l3 3v10h-9z" fill={c} opacity="0.25" stroke={c} strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M9.5 1.5v3h3" fill="none" stroke={c} strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

export const IconSearch = () => (
  <svg width="14" height="14" viewBox="0 0 16 16"><circle cx="7" cy="7" r="4.5" {...s} /><path d="m10.5 10.5 3.5 3.5" {...s} /></svg>
);
export const IconGear = () => (
  <svg width="16" height="16" viewBox="0 0 16 16">
    <circle cx="8" cy="8" r="2.2" {...s} />
    <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" {...s} />
  </svg>
);
export const IconUp = () => (
  <svg width="14" height="14" viewBox="0 0 16 16"><path d="M8 13V3M3.5 7.5 8 3l4.5 4.5" {...s} /></svg>
);
export const IconDrive = () => (
  <svg width="18" height="18" viewBox="0 0 16 16"><rect x="1.5" y="4" width="13" height="8" rx="1.8" {...s} /><circle cx="11.5" cy="8" r=".8" fill="currentColor" /></svg>
);
export const IconFolder = () => (
  <svg width="18" height="18" viewBox="0 0 16 16"><path d="M1.5 4.2c0-.7.5-1.2 1.2-1.2h3.4l1.5 1.6h5.7c.7 0 1.2.5 1.2 1.2v6.5c0 .7-.5 1.2-1.2 1.2H2.7c-.7 0-1.2-.5-1.2-1.2z" {...s} /></svg>
);
export const Sparkle = () => (
  <svg width="12" height="12" viewBox="0 0 16 16"><path d="M8 1.5 9.6 6.4 14.5 8 9.6 9.6 8 14.5 6.4 9.6 1.5 8l4.9-1.6z" fill="currentColor" /></svg>
);
