import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CheckIcon,
  LanguagesIcon,
  MinusIcon,
  MonitorIcon,
  MoonIcon,
  SquareIcon,
  SunIcon,
  XIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ExportDialog } from "@/components/ExportDialog";
import { ExportRunView, type LogLine } from "@/components/ExportRunView";
import { ExportResultView } from "@/components/ExportResultView";
import { HomeView } from "@/components/HomeView";
import { SidecarErrorCard } from "@/components/SidecarErrorCard";
import {
  EMPTY_LINK_CHECK,
  LinkCheckPanel,
  applyCheckEvents,
  applyCheckExit,
  type LinkCheckState,
} from "@/components/LinkCheckPanel";
import { OptionsView, type UpdateHandlers } from "@/components/OptionsView";
import {
  EMPTY_UPDATE,
  applyUpdateEvents,
  applyUpdateExit,
  dueUpdateCheck,
  loadUpdateProxy,
  markUpdateChecked,
  normalizeUpdateProxy,
  saveUpdateProxy,
  updateProxyArg,
  type UpdateProxyConfig,
  type UpdateState,
} from "@/components/UpdatePanel";
import { fmt, LANGUAGE_ORDER, useI18n } from "@/i18n";
import type { LanguagePreference } from "@/i18n";
import {
  loadOptions,
  saveOptions,
  type ExportOptions,
} from "@/lib/options";
import { useDestinationPreview } from "@/lib/preview";
import {
  type CheckEvent,
  type SidecarEvent,
  type SidecarExit,
  baseName,
  cancelExport,
  checkSidecar,
  onCheckError,
  onCheckEvent,
  onCheckExit,
  onSidecarError,
  onSidecarEvent,
  onSidecarExit,
  onUpdateError,
  onUpdateEvent,
  onUpdateExit,
  runInstaller,
  startCheck,
  startExport,
  startUpdate,
} from "@/lib/sidecar";
import { useLayoutBreakpoints } from "@/lib/layout";
import { THEME_ORDER, useTheme, type ThemePreference } from "@/lib/theme";

type Phase = "setup" | "running" | "result";

interface ExportProgress {
  total: number;
  done: number;
  skipped: number;
  endSeen: boolean;
  failures: { path: string; message: string }[];
  warnings: { path: string | null; message: string }[];
  lines: LogLine[];
  /** Current diagram rendering slot (index/total), null when not rendering. */
  diagram: { index: number; total: number; language: string } | null;
}

export const EMPTY_PROGRESS: ExportProgress = {
  total: 0,
  done: 0,
  skipped: 0,
  endSeen: false,
  failures: [],
  warnings: [],
  lines: [],
  diagram: null,
};

const REMEMBER_PATHS_KEY = "obsidian-export-remember-paths";
const SOURCE_KEY = "obsidian-export-source";
const DESTINATION_KEY = "obsidian-export-destination";
const KEEP_ROOT_KEY = "obsidian-export-keep-root";

/** Stored booleans default to `fallback` when the key is absent. */
function loadBool(key: string, fallback: boolean): boolean {
  const stored = localStorage.getItem(key);
  return stored === null ? fallback : stored === "true";
}

export function foldEvent(
  progress: ExportProgress,
  event: SidecarEvent,
  warningLabel: string,
): ExportProgress {
  switch (event.type) {
    case "schema":
      return progress;
    case "start":
      return { ...progress, total: event.total };
    case "file-done":
      return {
        ...progress,
        done: progress.done + 1,
        lines: [...progress.lines, { kind: "done", text: baseName(event.path) }],
      };
    case "file-skipped":
      return {
        ...progress,
        skipped: progress.skipped + 1,
        lines: [...progress.lines, { kind: "skipped", text: baseName(event.path) }],
      };
    case "file-failed":
      return {
        ...progress,
        failures: [...progress.failures, { path: event.path, message: event.message }],
        lines: [
          ...progress.lines,
          { kind: "failed", text: baseName(event.path), detail: event.message },
        ],
      };
    case "warning":
      return {
        ...progress,
        warnings: [
          ...progress.warnings,
          { path: event.path, message: event.message },
        ],
        lines: [
          ...progress.lines,
          {
            kind: "warning",
            text: event.path ? baseName(event.path) : warningLabel,
            detail: event.message,
          },
        ],
      };
    case "diagram-render":
      return {
        ...progress,
        diagram: {
          index: event.index,
          total: event.total,
          language: event.language,
        },
      };
    case "end":
      return { ...progress, endSeen: true, diagram: null };
  }
}

/** Theme button cycles light → dark → system; icon shows the current mode. */
function ThemeToggle() {
  const [theme, , setTheme] = useTheme();
  const { t } = useI18n();
  const labels: Record<ThemePreference, string> = {
    light: t.theme.light,
    dark: t.theme.dark,
    system: t.theme.system,
  };
  const next =
    THEME_ORDER[(THEME_ORDER.indexOf(theme) + 1) % THEME_ORDER.length];
  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={() => setTheme(next)}
      aria-label={fmt(t.theme.toggleLabel, {
        current: labels[theme],
        next: labels[next],
      })}
      title={fmt(t.theme.toggleTitle, {
        current: labels[theme],
        next: labels[next],
      })}
    >
      {theme === "light" && <SunIcon className="size-4" />}
      {theme === "dark" && <MoonIcon className="size-4" />}
      {theme === "system" && <MonitorIcon className="size-4" />}
    </Button>
  );
}

/** Language dropdown: any of zh / en / follow-system can be picked freely. */
function LanguageMenu() {
  const { preference, setPreference, t } = useI18n();
  const labels: Record<LanguagePreference, string> = {
    zh: t.language.zh,
    en: t.language.en,
    system: t.language.system,
  };
  const current = labels[preference];
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={fmt(t.language.menuLabel, { current })}
          title={fmt(t.language.menuLabel, { current })}
        >
          <LanguagesIcon className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {LANGUAGE_ORDER.map((lang) => (
          <DropdownMenuItem key={lang} onClick={() => setPreference(lang)}>
            {/* Fixed-width slot keeps labels aligned with and without the check. */}
            <span className="flex w-4 shrink-0 justify-center">
              {lang === preference && <CheckIcon className="size-4" />}
            </span>
            {labels[lang]}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Windows-style window controls for the frameless title bar. */
function WindowControls() {
  const win = getCurrentWindow();
  const { t } = useI18n();
  const control =
    "flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-[var(--background-modifier-hover)]";
  return (
    <div className="flex h-full items-stretch">
      <button
        className={control}
        onClick={() => win.minimize()}
        aria-label={t.window.minimize}
      >
        <MinusIcon className="size-3.5" />
      </button>
      <button
        className={control}
        onClick={() => win.toggleMaximize()}
        aria-label={t.window.maximize}
      >
        <SquareIcon className="size-3" />
      </button>
      <button
        className="flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-[#e81123] hover:text-white"
        onClick={() => win.close()}
        aria-label={t.window.close}
      >
        <XIcon className="size-4" />
      </button>
    </div>
  );
}

export default function App() {
  const { t } = useI18n();
  const { wide } = useLayoutBreakpoints();
  const [phase, setPhase] = useState<Phase>("setup");
  const [source, setSource] = useState(
    () => localStorage.getItem(SOURCE_KEY) ?? "",
  );
  const [destination, setDestination] = useState(
    () => localStorage.getItem(DESTINATION_KEY) ?? "",
  );
  const [rememberPaths, setRememberPaths] = useState(() =>
    loadBool(REMEMBER_PATHS_KEY, true),
  );
  const [keepRootFolder, setKeepRootFolder] = useState(() =>
    loadBool(KEEP_ROOT_KEY, true),
  );
  // Landing-path preview shared by the home view and the confirm dialog;
  // debounced, stale-result-proof (see lib/preview).
  const preview = useDestinationPreview(source, destination, keepRootFolder);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [view, setView] = useState<"main" | "options">("main");
  const [options, setOptions] = useState<ExportOptions>(loadOptions);
  const [progress, setProgress] = useState<ExportProgress>(EMPTY_PROGRESS);
  const [exit, setExit] = useState<SidecarExit | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [sidecarBanner, setSidecarBanner] = useState<string | null>(null);
  const [sidecarError, setSidecarError] = useState<string | null>(null);
  const [check, setCheck] = useState<LinkCheckState>(EMPTY_LINK_CHECK);
  const [update, setUpdate] = useState<UpdateState>(EMPTY_UPDATE);
  const [updateProxy, setUpdateProxy] = useState<UpdateProxyConfig>(() =>
    loadUpdateProxy(),
  );
  // The sidecar subscriptions below are subscribed exactly once for the
  // app's lifetime, so the trigger data they need (latest options, last
  // run's paths) travels through refs instead of stale closures.
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const updateProxyRef = useRef(updateProxy);
  updateProxyRef.current = updateProxy;
  const lastRunRef = useRef<{ source: string; target: string } | null>(null);

  useEffect(() => {
    checkSidecar()
      .then(setSidecarBanner)
      .catch((err) => setSidecarError(String(err)));
  }, []);

  // Automatic update check on launch: at most once per day, gated by the
  // preference, silent on failure (the verdict surfaces the next time the
  // "About" page is opened; the panel folds whatever events arrived).
  useEffect(() => {
    if (!optionsRef.current.autoCheckUpdates || !dueUpdateCheck()) {
      return;
    }
    // Delay past the user's first interactions: the check shares the sidecar
    // child slot with exports, and a silent background claim right at launch
    // would turn an immediate first export into a confusing failure.
    const timer = window.setTimeout(() => {
      markUpdateChecked();
      setUpdate((s) => (s.phase === "idle" ? { ...s, phase: "checking" } : s));
      startUpdate("check", updateProxyArg(updateProxyRef.current)).catch(() => {
        setUpdate((s) => (s.phase === "checking" ? EMPTY_UPDATE : s));
      });
    }, 2500);
    return () => window.clearTimeout(timer);
  }, []);

  // All sidecar subscriptions live in a single effect with no dependencies.
  // The one per-language value they need (the export log's warning label)
  // travels through a ref instead of the closure: re-subscribing on
  // language change opened a microtask window where the old handler's
  // unlisten had not run yet, so sidecar-event was folded by both handlers
  // (double done counts, duplicated log lines) — and re-subscribing at all
  // could miss sidecar-exit (the auto-check trigger) or check-exit mid-flight.
  const warningLogRef = useRef(t.app.warningLog);
  useEffect(() => {
    warningLogRef.current = t.app.warningLog;
  }, [t]);

  useEffect(() => {
    // The CLI bursts every link report in one go after checking finishes;
    // folding each event into state individually is quadratic on big vaults.
    // Events are buffered and folded once per animation frame instead. The
    // frame callback never fires while the WebView is minimized or hidden
    // (rAF is paused there), so a short timer rides along as the fallback:
    // background timers get throttled to roughly one per second but still
    // flush, keeping the progress current instead of buffering in memory.
    const FALLBACK_FLUSH_MS = 250;
    let buffer: CheckEvent[] = [];
    let frame: number | null = null;
    let timer: number | null = null;
    const flush = () => {
      if (frame !== null) {
        cancelAnimationFrame(frame);
        frame = null;
      }
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      if (buffer.length === 0) return;
      const events = buffer;
      buffer = [];
      setCheck((s) => applyCheckEvents(s, events));
    };

    const unlisteners: Promise<() => void>[] = [
      // The warning label comes from the ref so this subscription never
      // needs re-creating on a language switch (see the comment above).
      onSidecarEvent((event) =>
        setProgress((p) => foldEvent(p, event, warningLogRef.current)),
      ),
      onSidecarExit((payload) => {
        setExit(payload);
        setPhase("result");
        // A healthy export kicks off the automatic link check configured in
        // the options. The check runs against the vault source (pre-export,
        // wikilinks intact) or the exported tree (post-export markdown).
        const current = optionsRef.current;
        if (payload.code === 0 && current.linkCheckEnabled) {
          const run = lastRunRef.current;
          if (run) {
            const root =
              current.linkCheckTarget === "destination"
                ? run.target
                : run.source;
            setCheck({ ...EMPTY_LINK_CHECK, phase: "running" });
            startCheck(root, current, current.linkCheckTarget).catch((err) =>
              setCheck((s) => ({
                ...s,
                phase: "failed",
                invokeError: String(err),
              })),
            );
          }
        }
      }),
      onCheckEvent((event) => {
        buffer.push(event);
        if (frame === null) {
          frame = requestAnimationFrame(flush);
        }
        if (timer === null) {
          timer = window.setTimeout(flush, FALLBACK_FLUSH_MS);
        }
      }),
      // Exit is the definitive end of the stream: flush pending reports
      // first so the end summary (and the done/failed verdict) sees them.
      onCheckExit((payload) => {
        flush();
        // Exit 1 covers both "broken links found" (a completed run, the end
        // event is present) and "the check itself failed" (no end event);
        // the two are told apart by the end summary, not the code. A user
        // cancel folds into the cancelled verdict (see applyCheckExit).
        setCheck((s) => applyCheckExit(s, payload));
      }),
      onCheckError((message) =>
        // Keep the last few stream errors for the failed-state diagnosis;
        // the export log view is gone while checking, so they can't go there.
        setCheck((s) => ({
          ...s,
          streamErrors: [...s.streamErrors.slice(-4), message],
        })),
      ),
      onSidecarError((message) =>
        setProgress((p) => ({
          ...p,
          lines: [...p.lines, { kind: "error", text: message }],
        })),
      ),
      // Update events are low-frequency (one verdict plus throttled progress
      // frames), so they fold directly without rAF buffering.
      onUpdateEvent((event) =>
        setUpdate((s) => applyUpdateEvents(s, [event])),
      ),
      // Exit is definitive for the update stream too: a transitional phase
      // at exit means the run failed (or was cancelled via the shared kill).
      onUpdateExit((payload) => setUpdate((s) => applyUpdateExit(s, payload))),
      onUpdateError((message) =>
        setUpdate((s) => ({
          ...s,
          streamErrors: [...s.streamErrors.slice(-4), message],
        })),
      ),
    ];
    return () => {
      if (frame !== null) cancelAnimationFrame(frame);
      if (timer !== null) clearTimeout(timer);
      for (const p of unlisteners) {
        p.then((unlisten) => unlisten());
      }
    };
  }, []);

  /** Persist a picked path immediately so it survives a restart. */
  const rememberPath = useCallback(
    (key: string, value: string) => {
      if (rememberPaths && value) {
        localStorage.setItem(key, value);
      }
    },
    [rememberPaths],
  );

  const handleSourceChange = useCallback(
    (value: string) => {
      setSource(value);
      rememberPath(SOURCE_KEY, value);
    },
    [rememberPath],
  );

  const handleDestinationChange = useCallback(
    (value: string) => {
      setDestination(value);
      rememberPath(DESTINATION_KEY, value);
    },
    [rememberPath],
  );

  const handleRememberPathsChange = useCallback(
    (value: boolean) => {
      setRememberPaths(value);
      localStorage.setItem(REMEMBER_PATHS_KEY, String(value));
      if (value) {
        if (source) localStorage.setItem(SOURCE_KEY, source);
        if (destination) localStorage.setItem(DESTINATION_KEY, destination);
      } else {
        localStorage.removeItem(SOURCE_KEY);
        localStorage.removeItem(DESTINATION_KEY);
      }
    },
    [source, destination],
  );

  const handleKeepRootChange = useCallback((value: boolean) => {
    setKeepRootFolder(value);
    localStorage.setItem(KEEP_ROOT_KEY, String(value));
  }, []);

  /** Options are persisted as they are made; no separate save step. */
  const handleOptionsChange = useCallback((next: ExportOptions) => {
    setOptions(next);
    saveOptions(next);
  }, []);

  const handleEditOptions = useCallback(() => {
    setConfirmOpen(false);
    setView("options");
  }, []);

  // ---- Update actions (sidecar slots live here, mirroring export/check) ---

  /** Normalize (pure function in UpdatePanel, contract-tested there) and
   * persist the proxy fields as they are typed: blank host means 127.0.0.1
   * (stored as null), blank/invalid port means direct (stored as null).
   * Anything structural the CLI rejects visibly. */
  const handleUpdateProxyChange = useCallback((host: string, port: string) => {
    const next = normalizeUpdateProxy(host, port);
    setUpdateProxy(next);
    saveUpdateProxy(next);
  }, []);

  const handleCheckNow = useCallback(() => {
    markUpdateChecked();
    setUpdate({ ...EMPTY_UPDATE, phase: "checking" });
    startUpdate("check", updateProxyArg(updateProxy)).catch((err) =>
      setUpdate((s) => ({
        ...s,
        phase: "failed",
        invokeError: String(err),
      })),
    );
  }, [updateProxy]);

  const handleDownload = useCallback(() => {
    setUpdate((s) => ({
      ...s,
      phase: "downloading",
      cancelled: false,
      downloadedBytes: 0,
      totalBytes: null,
      bytesPerSecond: 0,
      downloadPath: null,
    }));
    startUpdate("download", updateProxyArg(updateProxy)).catch((err) =>
      setUpdate((s) => ({
        ...s,
        phase: "failed",
        invokeError: String(err),
      })),
    );
  }, [updateProxy]);

  const handleInstall = useCallback(() => {
    const path = update.downloadPath;
    if (path === null) {
      return;
    }
    // On success the app exits before the promise resolves; the catch only
    // fires when launching failed (e.g. the file is locked by antivirus).
    runInstaller(path).catch((err) =>
      setUpdate((s) => ({
        ...s,
        phase: "failed",
        invokeError: String(err),
      })),
    );
  }, [update.downloadPath]);

  const handleCancelDownload = useCallback(() => {
    // The update download shares the sidecar child slot with exports; the
    // generic kill covers it. The flag makes update-exit fold the state
    // into cancelled rather than failed (mirrors the export side).
    setUpdate((s) =>
      s.phase === "checking" || s.phase === "downloading"
        ? { ...s, cancelled: true }
        : s,
    );
    cancelExport().catch(() => undefined);
  }, []);

  const updateHandlers: UpdateHandlers = {
    state: update,
    proxyHost: updateProxy.host ?? "",
    proxyPort: updateProxy.port === null ? "" : String(updateProxy.port),
    onProxyChange: handleUpdateProxyChange,
    onCheckNow: handleCheckNow,
    onDownload: handleDownload,
    onInstall: handleInstall,
    onCancelDownload: handleCancelDownload,
  };

  const handleStart = useCallback(async () => {
    setConfirmOpen(false);
    setProgress(EMPTY_PROGRESS);
    setExit(null);
    setCancelled(false);
    setCheck(EMPTY_LINK_CHECK);
    setPhase("running");
    try {
      const target = await startExport(source, destination, keepRootFolder, options);
      lastRunRef.current = { source, target };
    } catch (err) {
      setExit({ code: null, stderr: String(err) });
      setPhase("result");
    }
  }, [source, destination, keepRootFolder, options]);

  const handleCancel = useCallback(async () => {
    setCancelled(true);
    await cancelExport();
  }, []);

  const handleReset = useCallback(() => {
    // No-ops when nothing runs; kills a still-running link check otherwise.
    void cancelExport();
    setPhase("setup");
    setProgress(EMPTY_PROGRESS);
    setExit(null);
    setCancelled(false);
    setCheck(EMPTY_LINK_CHECK);
  }, []);

  const canExport = source !== "" && destination !== "" && !sidecarError;

  return (
    <div className="flex h-screen flex-col bg-[var(--background-secondary)]">
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center justify-between border-b bg-[var(--background-primary)] pr-0 pl-4"
      >
        <div data-tauri-drag-region className="flex items-center gap-2.5">
          <span
            data-tauri-drag-region
            className="size-2.5 rounded-full bg-[var(--interactive-accent)]"
          />
          <span data-tauri-drag-region className="font-semibold">
            Obsidian Export
          </span>
          {sidecarBanner && (
            <span
              data-tauri-drag-region
              className="font-mono text-xs text-muted-foreground"
            >
              {sidecarBanner}
            </span>
          )}
          {sidecarError && (
            <span
              data-tauri-drag-region
              className="text-xs text-destructive"
              title={sidecarError}
            >
              {t.app.sidecarUnavailable}
            </span>
          )}
        </div>
        <div className="flex h-full items-center">
          <LanguageMenu />
          <ThemeToggle />
          <WindowControls />
        </div>
      </header>

      <main className="flex min-h-0 w-full flex-1 flex-col overflow-hidden">
        {phase === "setup" && view === "options" && (
          <div className="flex-1 overflow-y-auto">
            <div className="mx-auto flex w-full max-w-4xl flex-col gap-4 p-4">
              {sidecarError && <SidecarErrorCard error={sidecarError} />}
              <OptionsView
                options={options}
                onOptionsChange={handleOptionsChange}
                onBack={() => setView("main")}
                update={updateHandlers}
              />
            </div>
          </div>
        )}

        {phase === "setup" && view === "main" && (
          <HomeView
            source={source}
            onSourceChange={handleSourceChange}
            destination={destination}
            onDestinationChange={handleDestinationChange}
            rememberPaths={rememberPaths}
            onRememberPathsChange={handleRememberPathsChange}
            keepRootFolder={keepRootFolder}
            onKeepRootChange={handleKeepRootChange}
            preview={preview}
            options={options}
            canExport={canExport}
            sidecarError={sidecarError}
            onOpenOptions={() => setView("options")}
            onExport={() => setConfirmOpen(true)}
          />
        )}

        {phase === "running" && (
          <div className="flex-1 overflow-y-auto">
            <div className="mx-auto flex h-full w-full max-w-6xl flex-col gap-4 p-4">
              {sidecarError && (
                <div className="shrink-0">
                  <SidecarErrorCard error={sidecarError} />
                </div>
              )}
              <ExportRunView progress={progress} onCancel={handleCancel} />
            </div>
          </div>
        )}

        {phase === "result" && (
          <div className="flex-1 overflow-y-auto">
            {/* Container width and row direction both derive from the JS
             * `wide` flag: innerWidth includes the classic scrollbar while
             * min-[1000px:] media queries do not, so a CSS-driven flex-row
             * could disagree with the JS-driven max-w in a ~17px window. */}
            <div
              className={`mx-auto flex w-full gap-4 p-4 ${
                wide && check.phase !== "idle"
                  ? "max-w-6xl flex-row items-start [&>*]:min-w-0 [&>*]:flex-1"
                  : "max-w-3xl flex-col"
              }`}
            >
              {sidecarError && <SidecarErrorCard error={sidecarError} />}
              <ExportResultView
                progress={progress}
                exit={exit}
                cancelled={cancelled}
                onRestart={handleReset}
              />
              {check.phase !== "idle" && (
                <LinkCheckPanel
                  state={check}
                  onCancel={() => {
                    // The check shares the export's child slot; the generic
                    // kill covers it. The flag makes check-exit fold the
                    // state into cancelled rather than failed.
                    setCheck((s) =>
                      s.phase === "running" ? { ...s, cancelled: true } : s,
                    );
                    void cancelExport();
                  }}
                />
              )}
            </div>
          </div>
        )}
      </main>

      <ExportDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        keepRootFolder={keepRootFolder}
        source={source}
        destination={destination}
        preview={preview}
        options={options}
        onEditOptions={handleEditOptions}
        onStart={handleStart}
      />
    </div>
  );
}
