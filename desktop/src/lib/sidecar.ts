import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ExportOptions, LinkCheckTarget } from "@/lib/options";

/** Mirrors SidecarEvent in desktop/src-tauri/src/events.rs (schema v1). */
export type SidecarEvent =
  | { type: "schema"; version: number }
  | { type: "start"; total: number }
  | { type: "file-done"; path: string }
  | { type: "file-skipped"; path: string }
  | { type: "file-failed"; path: string; message: string }
  | { type: "warning"; path: string | null; message: string }
  | { type: "end"; failed: string[] };

/** Mirrors the `check` dialect of the same schema (CheckEvent in events.rs). */
export type CheckEvent =
  | { type: "schema"; version: number }
  | { type: "check-start"; files: number }
  | {
      type: "link-report";
      source: string;
      line: number;
      raw: string;
      kind: LinkKind;
      status: CheckStatus;
    }
  | {
      type: "check-end";
      filesChecked: number;
      totalLinks: number;
      broken: number;
      skipped: number;
    };

export type LinkKind =
  | "wiki-link"
  | "wiki-embed"
  | "markdown-link"
  | "markdown-image"
  | "unknown";

export type CheckStatus =
  | { type: "ok" }
  | { type: "missing-file"; target: string }
  | { type: "out-of-bounds"; target: string }
  | { type: "missing-section"; target: string; section: string }
  | { type: "missing-block"; target: string; block: string }
  | { type: "file-unreadable"; message: string }
  | { type: "external-skipped"; url: string }
  | { type: "unknown" };

export interface SidecarExit {
  code: number | null;
  stderr: string;
}

/** Same shape as SidecarExit, delivered on the `check-exit` channel. */
export type CheckExit = SidecarExit;

export function checkSidecar(): Promise<string> {
  return invoke<string>("check_sidecar");
}

/** Resolves to the actual export destination (after keep-root resolution). */
export function startExport(
  source: string,
  destination: string,
  keepRootFolder: boolean,
  options: ExportOptions,
): Promise<string> {
  return invoke<string>("start_export", {
    source,
    destination,
    keepRootFolder,
    options,
  });
}

export function startCheck(
  source: string,
  options: ExportOptions,
  target: LinkCheckTarget,
): Promise<void> {
  return invoke("start_check", { source, options, target });
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

export function onCheckEvent(
  cb: (event: CheckEvent) => void,
): Promise<UnlistenFn> {
  return listen<CheckEvent>("check-event", (e) => cb(e.payload));
}

export function onCheckExit(
  cb: (exit: CheckExit) => void,
): Promise<UnlistenFn> {
  return listen<CheckExit>("check-exit", (e) => cb(e.payload));
}

/** Parse/IO errors of the check stream (the check dialect's error channel). */
export function onCheckError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("check-error", (e) => cb(e.payload));
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
