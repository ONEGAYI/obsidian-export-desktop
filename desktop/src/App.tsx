import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  FolderOpenIcon,
  MinusIcon,
  MoonIcon,
  SquareIcon,
  SunIcon,
  XIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
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
import { useTheme } from "@/lib/theme";

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

function loadMissingSection(): MissingSectionStrategy {
  const stored = localStorage.getItem(MISSING_SECTION_KEY);
  return MISSING_SECTION_OPTIONS.some((o) => o.value === stored)
    ? (stored as MissingSectionStrategy)
    : "skip";
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
  const [theme, setTheme] = useTheme();
  const [phase, setPhase] = useState<Phase>("setup");
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
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

  const handleStart = useCallback(async () => {
    setConfirmOpen(false);
    localStorage.setItem(MISSING_SECTION_KEY, missingSection);
    setProgress(EMPTY_PROGRESS);
    setExit(null);
    setCancelled(false);
    setPhase("running");
    try {
      await startExport(source, destination, missingSection);
    } catch (err) {
      setExit({ code: null, stderr: String(err) });
      setPhase("result");
    }
  }, [source, destination, missingSection]);

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
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
            aria-label="切换主题"
          >
            {theme === "dark" ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
          </Button>
          <WindowControls />
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-xl flex-1 flex-col gap-4 overflow-y-auto p-4">
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
                onChange={setSource}
              />
              <PathPicker
                label="导出目标"
                placeholder="选择输出文件夹"
                value={destination}
                onChange={setDestination}
              />
              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">
                  更多选项将在后续版本提供
                </span>
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
      </main>

      <ExportDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        missingSection={missingSection}
        onMissingSectionChange={setMissingSection}
        source={source}
        destination={destination}
        onStart={handleStart}
      />
    </div>
  );
}
