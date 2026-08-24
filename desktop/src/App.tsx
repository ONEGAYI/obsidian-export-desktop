import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CheckIcon,
  FolderOpenIcon,
  LanguagesIcon,
  MinusIcon,
  MonitorIcon,
  MoonIcon,
  SettingsIcon,
  SquareIcon,
  SunIcon,
  XIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
import {
  EMPTY_LINK_CHECK,
  LinkCheckPanel,
  foldCheckEvent,
  type LinkCheckState,
} from "@/components/LinkCheckPanel";
import { OptionsView } from "@/components/OptionsView";
import { PathPicker } from "@/components/PathPicker";
import { fmt, LANGUAGE_ORDER, useI18n } from "@/i18n";
import type { LanguagePreference } from "@/i18n";
import {
  loadOptions,
  saveOptions,
  type ExportOptions,
} from "@/lib/options";
import {
  type SidecarEvent,
  type SidecarExit,
  baseName,
  cancelExport,
  checkSidecar,
  onCheckEvent,
  onCheckExit,
  onSidecarError,
  onSidecarEvent,
  onSidecarExit,
  startCheck,
  startExport,
} from "@/lib/sidecar";
import { THEME_ORDER, useTheme, type ThemePreference } from "@/lib/theme";

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

const REMEMBER_PATHS_KEY = "obsidian-export-remember-paths";
const SOURCE_KEY = "obsidian-export-source";
const DESTINATION_KEY = "obsidian-export-destination";
const KEEP_ROOT_KEY = "obsidian-export-keep-root";

/** Stored booleans default to `fallback` when the key is absent. */
function loadBool(key: string, fallback: boolean): boolean {
  const stored = localStorage.getItem(key);
  return stored === null ? fallback : stored === "true";
}

function foldEvent(
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
    case "end":
      return { ...progress, endSeen: true };
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
  const [view, setView] = useState<"main" | "options">("main");
  const [options, setOptions] = useState<ExportOptions>(loadOptions);
  const [progress, setProgress] = useState<ExportProgress>(EMPTY_PROGRESS);
  const [exit, setExit] = useState<SidecarExit | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [sidecarBanner, setSidecarBanner] = useState<string | null>(null);
  const [sidecarError, setSidecarError] = useState<string | null>(null);
  const [check, setCheck] = useState<LinkCheckState>(EMPTY_LINK_CHECK);
  // The sidecar-exit listener below is subscribed once per language change,
  // so the trigger data it needs (latest options, last run's paths) travels
  // through refs instead of stale closures.
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const lastRunRef = useRef<{ source: string; target: string } | null>(null);

  useEffect(() => {
    checkSidecar()
      .then(setSidecarBanner)
      .catch((err) => setSidecarError(String(err)));
  }, []);

  // Re-subscribed when the language changes so log placeholders follow the
  // active dictionary.
  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [
      onSidecarEvent((event) =>
        setProgress((p) => foldEvent(p, event, t.app.warningLog)),
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
      onCheckEvent((event) => setCheck((s) => foldCheckEvent(s, event))),
      onCheckExit((payload) =>
        // Exit 1 covers "broken links found" (a completed run, the end event
        // is present) and "the check itself failed" (no end event); the two
        // are told apart by the end summary, not the code.
        setCheck((s) =>
          s.phase === "running"
            ? { ...s, exit: payload, phase: s.end ? "done" : "failed" }
            : s,
        ),
      ),
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
  }, [t]);

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

      <main
        className={`mx-auto flex w-full flex-1 flex-col overflow-y-auto p-4 ${
          phase === "setup" && view === "options" ? "max-w-2xl" : "max-w-xl"
        }`}
      >
        <div className="m-auto flex w-full flex-col gap-4">
            {sidecarError && (
            <Card className="border-destructive">
              <CardHeader>
                <CardTitle className="text-destructive">
                  {t.app.sidecarErrorTitle}
                </CardTitle>
                <CardDescription>
                  {t.app.sidecarErrorHint.pre}{" "}
                  <code>{t.app.sidecarErrorHint.code}</code>{" "}
                  {t.app.sidecarErrorHint.post}
                </CardDescription>
              </CardHeader>
              <CardContent>
                <pre className="max-h-24 overflow-auto rounded-md bg-[var(--background-secondary)] p-2 font-mono text-xs whitespace-pre-wrap">
                  {sidecarError}
                </pre>
              </CardContent>
            </Card>
          )}

          {phase === "setup" && view === "options" && (
            <OptionsView
              options={options}
              onOptionsChange={handleOptionsChange}
              onBack={() => setView("main")}
            />
          )}

          {phase === "setup" && view === "main" && (
            <Card>
              <CardHeader>
                <CardTitle>{t.app.exportTitle}</CardTitle>
                <CardDescription>{t.app.exportDescription}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                <PathPicker
                  label={t.app.sourceLabel}
                  placeholder={t.app.sourcePlaceholder}
                  value={source}
                  onChange={handleSourceChange}
                />
                <PathPicker
                  label={t.app.destinationLabel}
                  placeholder={t.app.destinationPlaceholder}
                  value={destination}
                  onChange={handleDestinationChange}
                />
                <div className="flex items-center justify-between">
                  <Label className="flex cursor-pointer items-center gap-2 text-xs font-normal text-muted-foreground">
                    <Checkbox
                      checked={rememberPaths}
                      onCheckedChange={handleRememberPathsChange}
                    />
                    {t.app.rememberPaths}
                  </Label>
                  <div className="flex items-center gap-2">
                    <Button variant="secondary" onClick={() => setView("options")}>
                      <SettingsIcon className="size-4" />
                      {t.app.options}
                    </Button>
                    <Button disabled={!canExport} onClick={() => setConfirmOpen(true)}>
                      <FolderOpenIcon className="size-4" />
                      {t.app.export}
                    </Button>
                  </div>
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
            <>
              <ExportResultView
                progress={progress}
                exit={exit}
                cancelled={cancelled}
                onRestart={handleReset}
              />
              {check.phase !== "idle" && <LinkCheckPanel state={check} />}
            </>
          )}
        </div>
      </main>

      <ExportDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        keepRootFolder={keepRootFolder}
        onKeepRootFolderChange={handleKeepRootChange}
        source={source}
        destination={destination}
        options={options}
        onEditOptions={handleEditOptions}
        onStart={handleStart}
      />
    </div>
  );
}
