import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  CheckCircle2Icon,
  DownloadIcon,
  ExternalLinkIcon,
  PackageCheckIcon,
  RefreshCwIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import { fmt, useI18n } from "@/i18n";
import type { UpdateEvent, UpdateExit, UpdateOutcome } from "@/lib/sidecar";

/**
 * Update-check panel state machine. Owned by App (event subscriptions live
 * there); this module exports the pure fold functions and the panel.
 *
 * `checking`/`downloading` are transitional; `result` carries the verdict of
 * the last check (available / up-to-date / no-release), `ready` means the
 * installer has been downloaded, and `failed` means the sidecar exited
 * without reaching its terminating event (or the invoke itself failed).
 */
export type UpdatePhase =
  | "idle"
  | "checking"
  | "result"
  | "downloading"
  | "ready"
  | "failed";

export interface UpdateState {
  phase: UpdatePhase;
  outcome: UpdateOutcome | null;
  version: string | null;
  htmlUrl: string | null;
  notes: string | null;
  assetName: string | null;
  assetSize: number | null;
  downloadedBytes: number;
  totalBytes: number | null;
  bytesPerSecond: number;
  downloadPath: string | null;
  exit: UpdateExit | null;
  streamErrors: string[];
  invokeError: string | null;
}

export const EMPTY_UPDATE: UpdateState = {
  phase: "idle",
  outcome: null,
  version: null,
  htmlUrl: null,
  notes: null,
  assetName: null,
  assetSize: null,
  downloadedBytes: 0,
  totalBytes: null,
  bytesPerSecond: 0,
  downloadPath: null,
  exit: null,
  streamErrors: [],
  invokeError: null,
};

/** Fold sidecar update events into state. Update events are low-frequency
 * (one verdict plus throttled progress frames), so no rAF buffering. */
export function applyUpdateEvents(
  state: UpdateState,
  events: UpdateEvent[],
): UpdateState {
  let next = state;
  for (const event of events) {
    next = applyOne(next, event);
  }
  return next;
}

function applyOne(state: UpdateState, event: UpdateEvent): UpdateState {
  switch (event.type) {
    case "schema":
      // The version is verified on the Rust side; nothing to fold.
      return state;
    case "update-result": {
      // A fresh verdict invalidates any installer downloaded for a previous
      // release; the download UI resets along with it. During a download
      // (the CLI re-checks before transferring) the phase stays put only
      // when the re-check still resolves to a downloadable asset — the
      // following download-start re-asserts it anyway, and bouncing
      // through "result" would flash the download button for a frame.
      // Any other verdict ends the run with exit 0 right after this
      // event, so folding to "result" keeps that exit from being misread
      // as a failure.
      const downloadContinues =
        state.phase === "downloading" &&
        event.outcome === "available" &&
        event.assetName != null;
      return {
        ...state,
        phase: downloadContinues ? "downloading" : "result",
        outcome: event.outcome,
        version: event.version,
        htmlUrl: event.htmlUrl,
        notes: event.notes,
        assetName: event.assetName,
        assetSize: event.assetSize,
        downloadedBytes: downloadContinues ? state.downloadedBytes : 0,
        totalBytes: downloadContinues ? state.totalBytes : null,
        bytesPerSecond: downloadContinues ? state.bytesPerSecond : 0,
        downloadPath: null,
        exit: null,
        streamErrors: [],
        invokeError: null,
      };
    }
    case "download-start":
      return {
        ...state,
        phase: "downloading",
        // `total` here is the release-metadata size; progress frames carry
        // the observed Content-Length, which wins from the first frame on.
        totalBytes: event.total,
      };
    case "download-progress":
      return {
        ...state,
        phase: "downloading",
        downloadedBytes: event.downloaded,
        totalBytes: event.total,
        bytesPerSecond: event.bytesPerSecond,
      };
    case "download-end":
      return { ...state, phase: "ready", downloadPath: event.path };
  }
}

/** Fold the definitive process exit. Reaching exit while still in a
 * transitional phase means the run never finished (failure, or a kill via
 * the shared cancel command); otherwise the exit is recorded but the
 * result/ready verdict stands. */
export function applyUpdateExit(
  state: UpdateState,
  exit: UpdateExit,
): UpdateState {
  if (state.phase === "checking" || state.phase === "downloading") {
    return { ...state, phase: "failed", exit };
  }
  return { ...state, exit };
}

// ---- 启动节流（localStorage，每天至多自动检查一次） ------------------------

const UPDATE_STATE_KEY = "obsidian-export-update-state";
const DAY_MS = 24 * 60 * 60 * 1000;

/** Whether the automatic on-launch check is due (≥24h since the last one). */
export function dueUpdateCheck(now = Date.now()): boolean {
  try {
    const raw = localStorage.getItem(UPDATE_STATE_KEY);
    const last = raw ? (JSON.parse(raw) as { lastCheck?: unknown }).lastCheck : null;
    if (typeof last !== "number" || !Number.isFinite(last)) {
      return true;
    }
    return now - last >= DAY_MS;
  } catch {
    // Corrupted payload: treat as never checked.
    return true;
  }
}

/** Record a check timestamp (called for both automatic and manual checks). */
export function markUpdateChecked(now = Date.now()): void {
  try {
    localStorage.setItem(UPDATE_STATE_KEY, JSON.stringify({ lastCheck: now }));
  } catch {
    // Storage unavailable (private mode &c.): throttling degrades to
    // checking on every launch, which is acceptable.
  }
}

// ---- 面板 -------------------------------------------------------------------

interface UpdatePanelProps {
  state: UpdateState;
  autoCheckEnabled: boolean;
  onAutoCheckChange: (enabled: boolean) => void;
  onCheckNow: () => void;
  onDownload: () => void;
  onInstall: () => void;
  onCancelDownload: () => void;
}

/** "About & update" page of the settings view. Pure presentation: all state
 * and side effects are owned by App. */
export function UpdatePanel({
  state,
  autoCheckEnabled,
  onAutoCheckChange,
  onCheckNow,
  onDownload,
  onInstall,
  onCancelDownload,
}: UpdatePanelProps) {
  const { t } = useI18n();
  const [appVersion, setAppVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(null));
  }, []);

  const busy = state.phase === "checking" || state.phase === "downloading";
  const available = state.phase !== "idle" && state.outcome === "available";

  // One-line verdict for the non-available, non-transitional outcomes; the
  // available verdict renders the detail card below instead. An `unknown`
  // outcome (a future dialect value) has its own line.
  const verdictLine: string | null = (() => {
    if (busy || state.phase === "failed") {
      return null;
    }
    switch (state.outcome) {
      case "up-to-date":
        return t.options.updateUpToDate;
      case "no-release":
        return t.options.updateNoRelease;
      case "available":
        return null;
      case "unknown":
        return t.options.updateUnknown;
      default:
        return t.options.updateIdle;
    }
  })();

  return (
    <section className="flex flex-col gap-2.5">
      <h3 className="text-sm font-semibold">{t.options.sectionAbout}</h3>
      <div className="flex max-w-lg flex-col gap-2.5">
        <p className="text-muted-foreground text-sm">
          {t.options.updateCurrentVersion}
          <span className="text-foreground ml-1.5 font-mono">
            {appVersion ?? "…"}
          </span>
        </p>

        {/* Verdict card */}
        <div className="flex flex-col gap-2 rounded-md border bg-[var(--background-primary)] p-3">
          {state.phase === "checking" && (
            <p className="text-muted-foreground flex items-center gap-2 text-sm">
              <RefreshCwIcon className="size-4 animate-spin" />
              {t.options.updateChecking}
            </p>
          )}

          {verdictLine !== null && state.outcome === "up-to-date" && (
            <p className="flex items-center gap-2 text-sm">
              <CheckCircle2Icon className="size-4 text-[var(--text-success)]" />
              {verdictLine}
            </p>
          )}
          {verdictLine !== null && state.outcome !== "up-to-date" && (
            <p className="text-muted-foreground text-sm">{verdictLine}</p>
          )}

          {available && (
            <div className="flex flex-col gap-2">
              <p className="text-sm font-semibold">
                {fmt(t.options.updateAvailable, {
                  version: state.version ?? "?",
                })}
              </p>
              {state.notes && (
                <details className="text-muted-foreground text-xs">
                  <summary className="cursor-pointer select-none hover:text-foreground">
                    {t.options.updateNotesTitle}
                  </summary>
                  <pre className="mt-1.5 max-h-40 overflow-y-auto whitespace-pre-wrap font-sans">
                    {state.notes}
                  </pre>
                </details>
              )}
              {state.assetName ? (
                <p className="text-muted-foreground text-xs">
                  {state.assetName}
                  {state.assetSize !== null && ` · ${formatBytes(state.assetSize)}`}
                </p>
              ) : (
                <p className="text-muted-foreground text-xs">
                  {t.options.updateNoAsset}
                </p>
              )}
              {state.htmlUrl && (
                <Button
                  variant="outline"
                  size="sm"
                  className="self-start"
                  onClick={() => {
                    if (state.htmlUrl) {
                      void openUrl(state.htmlUrl);
                    }
                  }}
                >
                  <ExternalLinkIcon className="size-3.5" />
                  {t.options.updateOpenReleasePage}
                </Button>
              )}
            </div>
          )}

          {state.phase === "downloading" && (
            <div className="flex flex-col gap-2">
              <p className="flex items-center gap-2 text-sm">
                <DownloadIcon className="size-4 animate-bounce" />
                {t.options.updateDownloading}
              </p>
              {state.totalBytes !== null && state.totalBytes > 0 ? (
                <>
                  <Progress
                    value={Math.min(
                      100,
                      (state.downloadedBytes / state.totalBytes) * 100,
                    )}
                    aria-label={t.options.updateDownloading}
                  />
                  <p className="text-muted-foreground text-xs">
                    {formatBytes(state.downloadedBytes)} / {formatBytes(state.totalBytes)}
                    {state.bytesPerSecond > 0 && ` · ${formatBytes(state.bytesPerSecond)}/s`}
                  </p>
                </>
              ) : (
                <p className="text-muted-foreground text-xs">
                  {formatBytes(state.downloadedBytes)}
                  {state.bytesPerSecond > 0 && ` · ${formatBytes(state.bytesPerSecond)}/s`}
                </p>
              )}
            </div>
          )}

          {state.phase === "ready" && state.downloadPath && (
            <div className="flex flex-col gap-2">
              <p className="flex items-center gap-2 text-sm">
                <PackageCheckIcon className="size-4 text-[var(--text-success)]" />
                {t.options.updateReady}
              </p>
              <p className="text-muted-foreground text-xs break-all">
                {fmt(t.options.updateSavedTo, { path: state.downloadPath })}
              </p>
            </div>
          )}

          {state.phase === "failed" && (
            <div className="flex flex-col gap-1.5">
              <p className="text-sm text-[var(--text-error)]">
                {t.options.updateFailed}
              </p>
              {state.invokeError && (
                <pre className="text-muted-foreground max-h-24 overflow-y-auto whitespace-pre-wrap text-xs">
                  {state.invokeError}
                </pre>
              )}
              {state.streamErrors.length > 0 && (
                <pre className="text-muted-foreground max-h-24 overflow-y-auto whitespace-pre-wrap text-xs">
                  {state.streamErrors.join("\n")}
                </pre>
              )}
              {state.exit?.stderr && (
                <pre className="text-muted-foreground max-h-24 overflow-y-auto whitespace-pre-wrap text-xs">
                  {state.exit.stderr}
                </pre>
              )}
            </div>
          )}

          {/* Primary action row */}
          <div className="flex flex-wrap gap-1.5">
            <Button
              size="sm"
              disabled={busy}
              onClick={onCheckNow}
            >
              {state.phase === "checking" ? (
                <RefreshCwIcon className="size-3.5 animate-spin" />
              ) : (
                <RefreshCwIcon className="size-3.5" />
              )}
              {t.options.updateCheckNowBtn}
            </Button>
            {available && state.assetName && (state.phase === "result" || state.phase === "failed") && (
              <Button size="sm" variant="outline" disabled={busy} onClick={onDownload}>
                <DownloadIcon className="size-3.5" />
                {t.options.updateDownload}
              </Button>
            )}
            {state.phase === "downloading" && (
              <Button size="sm" variant="ghost" onClick={onCancelDownload}>
                {t.options.updateCancelDownload}
              </Button>
            )}
            {state.phase === "ready" && state.downloadPath && (
              <Button size="sm" onClick={onInstall}>
                <PackageCheckIcon className="size-3.5" />
                {t.options.updateInstall}
              </Button>
            )}
          </div>
          {state.phase === "ready" && (
            <p className="text-muted-foreground text-xs">
              {t.options.updateInstallHint}
            </p>
          )}
        </div>

        {/* Auto-check preference (same shape as the options view's SwitchRow) */}
        <div className="flex items-start justify-between gap-3 rounded-md border bg-[var(--background-primary)] p-2.5">
          <span className="flex flex-col gap-0.5">
            <span className="text-sm leading-none font-medium">
              {t.options.updateAutoCheckTitle}
            </span>
            <span className="text-muted-foreground text-xs">
              {t.options.updateAutoCheckHint}
            </span>
          </span>
          {/* The name carries the state: AX-tree walkers don't render
              ToggleState, so without it a toggle leaves the tree unchanged. */}
          <Switch
            checked={autoCheckEnabled}
            onCheckedChange={onAutoCheckChange}
            aria-label={fmt(
              autoCheckEnabled
                ? t.common.statefulControl.nameOn
                : t.common.statefulControl.nameOff,
              { title: t.options.updateAutoCheckTitle },
            )}
            className="mt-0.5"
          />
        </div>
      </div>
    </section>
  );
}

/** Format a byte count as B/KB/MB/GB with one decimal for scaled units. */
function formatBytes(value: number): string {
  const units = ["B", "KB", "MB", "GB"] as const;
  let scaled = value;
  let unit = 0;
  while (scaled >= 1024 && unit < units.length - 1) {
    scaled /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${scaled} B` : `${scaled.toFixed(1)} ${units[unit]}`;
}
