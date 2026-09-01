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
  | { type: "diagram-render"; language: string; index: number; total: number }
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

/** Mirrors the `update` dialect of the same schema (UpdateEvent in events.rs). */
export type UpdateEvent =
  | { type: "schema"; version: number }
  | {
      type: "update-result";
      outcome: UpdateOutcome;
      version: string | null;
      htmlUrl: string | null;
      notes: string | null;
      assetName: string | null;
      assetSize: number | null;
    }
  | { type: "download-start"; total: number }
  | {
      type: "download-progress";
      downloaded: number;
      total: number | null;
      bytesPerSecond: number;
    }
  | { type: "download-end"; path: string };

export type UpdateOutcome =
  | "available"
  | "up-to-date"
  | "no-release"
  | "unknown";

/** Same shape as SidecarExit, delivered on the `update-exit` channel. */
export type UpdateExit = SidecarExit;

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

/**
 * Start an update action on the sidecar. `check` only queries the latest
 * release; `download` additionally saves the NSIS installer into the
 * downloads dir (created by the backend) and resolves to that directory.
 */
export function startUpdate(action: "check" | "download"): Promise<string> {
  return invoke<string>("start_update", { action });
}

/** Launch the downloaded installer; the app exits right after (see Rust side). */
export function runInstaller(path: string): Promise<void> {
  return invoke("run_installer", { path });
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

export function onUpdateEvent(
  cb: (event: UpdateEvent) => void,
): Promise<UnlistenFn> {
  return listen<UpdateEvent>("update-event", (e) => cb(e.payload));
}

export function onUpdateExit(cb: (exit: UpdateExit) => void): Promise<UnlistenFn> {
  return listen<UpdateExit>("update-exit", (e) => cb(e.payload));
}

/** Parse/IO errors of the update stream (the update dialect's error channel). */
export function onUpdateError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("update-error", (e) => cb(e.payload));
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
