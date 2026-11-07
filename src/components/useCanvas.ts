import { useEffect, useRef, useState } from "react";

/** A HiDPI canvas that tracks its container size. */
export function useCanvas() {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => {
      const { width, height } = e.contentRect;
      setSize({ w: Math.floor(width), h: Math.floor(height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const getCtx = () => {
    const c = canvasRef.current;
    if (!c || !size.w || !size.h) return null;
    const dpr = window.devicePixelRatio || 1;
    if (c.width !== size.w * dpr || c.height !== size.h * dpr) {
      c.width = size.w * dpr;
      c.height = size.h * dpr;
    }
    const ctx = c.getContext("2d");
    if (!ctx) return null;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, size.w, size.h);
    return ctx;
  };

  return { wrapRef, canvasRef, size, getCtx };
}

export function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Mix a hex colour towards white/black by `amt` (-1..1). */
export function shade(hex: string, amt: number): string {
  const n = parseInt(hex.slice(1), 16);
  let r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  const t = amt < 0 ? 0 : 255;
  const p = Math.abs(amt);
  r = Math.round((t - r) * p + r);
  g = Math.round((t - g) * p + g);
  b = Math.round((t - b) * p + b);
  return `rgb(${r},${g},${b})`;
}

export function fitText(ctx: CanvasRenderingContext2D, text: string, max: number): string | null {
  if (max < 14) return null;
  if (ctx.measureText(text).width <= max) return text;
  let lo = 0, hi = text.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (ctx.measureText(text.slice(0, mid) + "…").width <= max) lo = mid;
    else hi = mid - 1;
  }
  return lo >= 2 ? text.slice(0, lo) + "…" : null;
}
