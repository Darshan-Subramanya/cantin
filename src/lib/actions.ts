import { revealItemInDir } from "@tauri-apps/plugin-opener";

export async function reveal(path: string) {
  try {
    await revealItemInDir(path);
  } catch (e) {
    console.error("reveal failed", e);
  }
}

export async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
  } catch (e) {
    console.error("copy failed", e);
  }
}

export const revealLabel = navigator.userAgent.includes("Mac")
  ? "Reveal in Finder"
  : navigator.userAgent.includes("Windows")
    ? "Show in Explorer"
    : "Show in folder";
