import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Mirrors SidecarEvent in desktop/src-tauri/src/events.rs (schema v1). */
export type SidecarEvent =
  | { type: "schema"; version: number }
  | { type: "start"; total: number }
  | { type: "file-done"; path: string }
  | { type: "file-skipped"; path: string }
  | { type: "file-failed"; path: string; message: string }
  | { type: "warning"; path: string | null; message: string }
  | { type: "end"; failed: string[] };

export interface SidecarExit {
  code: number | null;
  stderr: string;
}

export type MissingSectionStrategy = "skip" | "embed-full" | "fail";

export const MISSING_SECTION_OPTIONS: {
  value: MissingSectionStrategy;
  label: string;
  description: string;
}[] = [
  {
    value: "skip",
    label: "跳过",
    description: "嵌入置空并发警告（默认，贴近 Obsidian 行为）",
  },
  {
    value: "embed-full",
    label: "嵌入整篇",
    description: "找不到章节时嵌入整篇笔记（旧行为）",
  },
  {
    value: "fail",
    label: "报错",
    description: "该笔记导出失败并计入结果",
  },
];

export function checkSidecar(): Promise<string> {
  return invoke<string>("check_sidecar");
}

export function startExport(
  source: string,
  destination: string,
  missingSection: MissingSectionStrategy,
): Promise<void> {
  return invoke("start_export", {
    source,
    destination,
    missingSection,
  });
}

export function cancelExport(): Promise<boolean> {
  return invoke("cancel_export");
}

export function onSidecarEvent(
  cb: (event: SidecarEvent) => void,
): Promise<UnlistenFn> {
  return listen<SidecarEvent>("sidecar-event", (e) => cb(e.payload));
}

export function onSidecarExit(
  cb: (exit: SidecarExit) => void,
): Promise<UnlistenFn> {
  return listen<SidecarExit>("sidecar-exit", (e) => cb(e.payload));
}

export function onSidecarError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("sidecar-error", (e) => cb(e.payload));
}

/** Reduce a full path to its file name for compact log lines. */
export function baseName(path: string): string {
  const normalized = path.split("\\").join("/");
  const idx = normalized.lastIndexOf("/");
  return idx === -1 ? normalized : normalized.slice(idx + 1);
}
