import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ExportOptions } from "@/lib/options";

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

export function checkSidecar(): Promise<string> {
  return invoke<string>("check_sidecar");
}

export function startExport(
  source: string,
  destination: string,
  keepRootFolder: boolean,
  options: ExportOptions,
): Promise<void> {
  return invoke("start_export", {
    source,
    destination,
    keepRootFolder,
    options,
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
