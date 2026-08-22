import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  FolderOpenIcon,
  MinusIcon,
  MonitorIcon,
  MoonIcon,
  SquareIcon,
  SunIcon,
  XIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ExportDialog } from "@/components/ExportDialog";
import { ExportRunView } from "@/components/ExportRunView";
import { ExportResultView } from "@/components/ExportResultView";
import { PathPicker } from "@/components/PathPicker";
import {
  MISSING_SECTION_OPTIONS,
  type MissingSectionStrategy,
  type SidecarEvent,
  type SidecarExit,
  baseName,
  cancelExport,
  checkSidecar,
  onSidecarError,
  onSidecarEvent,
  onSidecarExit,
  startExport,
} from "@/lib/sidecar";
import { THEME_ORDER, useTheme } from "@/lib/theme";

type Phase = "setup" | "running" | "result";

interface LogLine {
  kind: "done" | "skipped" | "failed" | "warning" | "error";
  text: string;
  detail?: string;
}

interface ExportProgress {
  total: number;
  done: number;
  skipped: number;
  endSeen: boolean;
  failures: { path: string; message: string }[];
  warnings: { path: string | null; message: string }[];
  lines: LogLine[];
}

const EMPTY_PROGRESS: ExportProgress = {
  total: 0,
  done: 0,
  skipped: 0,
  endSeen: false,
  failures: [],
  warnings: [],
  lines: [],
};

const MISSING_SECTION_KEY = "obsidian-export-missing-section";
const REMEMBER_PATHS_KEY = "obsidian-export-remember-paths";
const SOURCE_KEY = "obsidian-export-source";
const DESTINATION_KEY = "obsidian-export-destination";
const KEEP_ROOT_KEY = "obsidian-export-keep-root";

function loadMissingSection(): MissingSectionStrategy {
  const stored = localStorage.getItem(MISSING_SECTION_KEY);
  return MISSING_SECTION_OPTIONS.some((o) => o.value === stored)
    ? (stored as MissingSectionStrategy)
    : "skip";
}

/** Stored booleans default to `fallback` when the key is absent. */
function loadBool(key: string, fallback: boolean): boolean {
  const stored = localStorage.getItem(key);
  return stored === null ? fallback : stored === "true";
}

function foldEvent(progress: ExportProgress, event: SidecarEvent): ExportProgress {
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
            text: event.path ? baseName(event.path) : "警告",
            detail: event.message,
          },
        ],
      };
    case "end":
      return { ...progress, endSeen: true };
  }
}

/** Theme button cycles light → dark → system; icon shows the current mode. */
function ThemeToggle() {
  const [theme, , setTheme] = useTheme();
  const labels: Record<string, string> = {
    light: "浅色",
    dark: "深色",
    system: "跟随系统",
  };
  const next =
    THEME_ORDER[(THEME_ORDER.indexOf(theme) + 1) % THEME_ORDER.length];
  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={() => setTheme(next)}
      aria-label={`主题：${labels[theme]}，点击切换为${labels[next]}`}
      title={`主题：${labels[theme]}（点击切换为${labels[next]}）`}
    >
      {theme === "light" && <SunIcon className="size-4" />}
      {theme === "dark" && <MoonIcon className="size-4" />}
      {theme === "system" && <MonitorIcon className="size-4" />}
    </Button>
  );
}

/** Windows-style window controls for the frameless title bar. */
function WindowControls() {
  const win = getCurrentWindow();
  const control =
    "flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-[var(--background-modifier-hover)]";
  return (
    <div className="flex h-full items-stretch">
      <button
        className={control}
        onClick={() => win.minimize()}
        aria-label="最小化"
      >
        <MinusIcon className="size-3.5" />
      </button>
      <button
        className={control}
        onClick={() => win.toggleMaximize()}
        aria-label="最大化"
      >
        <SquareIcon className="size-3" />
      </button>
      <button
        className="flex h-full w-11 items-center justify-center text-muted-foreground transition-colors hover:bg-[#e81123] hover:text-white"
        onClick={() => win.close()}
        aria-label="关闭"
      >
        <XIcon className="size-4" />
      </button>
    </div>
  );
}

export default function App() {
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
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [missingSection, setMissingSection] = useState<MissingSectionStrategy>(
    loadMissingSection,
  );
  const [progress, setProgress] = useState<ExportProgress>(EMPTY_PROGRESS);
  const [exit, setExit] = useState<SidecarExit | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [sidecarBanner, setSidecarBanner] = useState<string | null>(null);
  const [sidecarError, setSidecarError] = useState<string | null>(null);

  useEffect(() => {
    checkSidecar()
      .then(setSidecarBanner)
      .catch((err) => setSidecarError(String(err)));
  }, []);

  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [
      onSidecarEvent((event) => setProgress((p) => foldEvent(p, event))),
      onSidecarExit((payload) => {
        setExit(payload);
        setPhase("result");
      }),
      onSidecarError((message) =>
        setProgress((p) => ({
          ...p,
          lines: [...p.lines, { kind: "error", text: message }],
        })),
      ),
    ];
    return () => {
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

  const handleStart = useCallback(async () => {
    setConfirmOpen(false);
    localStorage.setItem(MISSING_SECTION_KEY, missingSection);
    setProgress(EMPTY_PROGRESS);
    setExit(null);
    setCancelled(false);
    setPhase("running");
    try {
      await startExport(source, destination, missingSection, keepRootFolder);
    } catch (err) {
      setExit({ code: null, stderr: String(err) });
      setPhase("result");
    }
  }, [source, destination, missingSection, keepRootFolder]);

  const handleCancel = useCallback(async () => {
    setCancelled(true);
    await cancelExport();
  }, []);

  const handleReset = useCallback(() => {
    setPhase("setup");
    setProgress(EMPTY_PROGRESS);
    setExit(null);
    setCancelled(false);
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
              边车不可用
            </span>
          )}
        </div>
        <div className="flex h-full items-center">
          <ThemeToggle />
          <WindowControls />
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-xl flex-1 flex-col overflow-y-auto p-4">
        <div className="m-auto flex w-full flex-col gap-4">
            {sidecarError && (
            <Card className="border-destructive">
              <CardHeader>
                <CardTitle className="text-destructive">边车进程不可用</CardTitle>
                <CardDescription>
                  运行 <code>just desktop-sync-sidecar</code> 后重启应用。
                </CardDescription>
              </CardHeader>
              <CardContent>
                <pre className="max-h-24 overflow-auto rounded-md bg-[var(--background-secondary)] p-2 font-mono text-xs whitespace-pre-wrap">
                  {sidecarError}
                </pre>
              </CardContent>
            </Card>
          )}

          {phase === "setup" && (
            <Card>
              <CardHeader>
                <CardTitle>导出 Obsidian Vault</CardTitle>
                <CardDescription>
                  将 Obsidian 方言 Markdown 转换为通用 Markdown，转换由内置的
                  obsidian-export 边车进程完成。
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <PathPicker
                  label="Vault 来源"
                  placeholder="选择 Obsidian vault 文件夹或单篇笔记"
                  value={source}
                  onChange={handleSourceChange}
                />
                <PathPicker
                  label="导出目标"
                  placeholder="选择输出文件夹"
                  value={destination}
                  onChange={handleDestinationChange}
                />
                <div className="flex items-center justify-between">
                  <Label className="flex cursor-pointer items-center gap-2 text-xs font-normal text-muted-foreground">
                    <Checkbox
                      checked={rememberPaths}
                      onCheckedChange={handleRememberPathsChange}
                    />
                    记住上次路径
                  </Label>
                  <Button disabled={!canExport} onClick={() => setConfirmOpen(true)}>
                    <FolderOpenIcon className="size-4" />
                    导出
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}

          {phase === "running" && (
            <ExportRunView
              progress={progress}
              onCancel={handleCancel}
            />
          )}

          {phase === "result" && (
            <ExportResultView
              progress={progress}
              exit={exit}
              cancelled={cancelled}
              onRestart={handleReset}
            />
          )}
        </div>
      </main>

      <ExportDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        missingSection={missingSection}
        onMissingSectionChange={setMissingSection}
        keepRootFolder={keepRootFolder}
        onKeepRootFolderChange={handleKeepRootChange}
        source={source}
        destination={destination}
        onStart={handleStart}
      />
    </div>
  );
}
