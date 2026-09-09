import { useEffect, useRef } from "react";
import {
  CircleAlertIcon,
  CircleCheckIcon,
  Loader2Icon,
  MinusCircleIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { fmt, useI18n } from "@/i18n";

interface LogLine {
  kind: "done" | "skipped" | "failed" | "warning" | "error";
  text: string;
  detail?: string;
}

export interface ExportProgressData {
  total: number;
  done: number;
  skipped: number;
  lines: LogLine[];
  /** Current diagram rendering slot, null while no diagram is rendering. */
  diagram: { index: number; total: number; language: string } | null;
}

const LINE_COLOR: Record<LogLine["kind"], string> = {
  done: "text-[var(--text-muted)]",
  skipped: "text-[var(--text-faint)]",
  failed: "text-destructive",
  warning: "text-yellow-500",
  error: "text-destructive",
};

const LINE_PREFIX: Record<LogLine["kind"], string> = {
  done: "✓",
  skipped: "–",
  failed: "✗",
  warning: "⚠",
  error: "!",
};

export function ExportRunView({
  progress,
  onCancel,
}: {
  progress: ExportProgressData;
  onCancel: () => void;
}) {
  const { t } = useI18n();
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [progress.lines.length]);

  const processed = progress.done + progress.skipped;
  const percent =
    progress.total > 0 ? Math.round((processed / progress.total) * 100) : 0;

  return (
    <Card className="flex h-full min-h-0 flex-col">
      <CardHeader className="shrink-0">
        <CardTitle>{t.run.title}</CardTitle>
        <CardDescription>
          {fmt(t.run.progressCount, {
            processed,
            total: progress.total,
          })}
        </CardDescription>
      </CardHeader>
      {/* Wide viewports put the progress column beside the log (which then
       * takes the remaining height); narrow ones stack, log keeps its fixed
       * height and the outer scroll container copes with overflow. The
       * cancel button sits at the end of the progress column so it stays
       * visible even while the log scrolls. */}
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4 min-[1000px]:flex-row">
        <div className="flex shrink-0 flex-col gap-3 min-[1000px]:w-72">
          <Progress value={percent} />
          <div className="text-muted-foreground flex flex-wrap items-center gap-4 text-xs">
            <span className="flex items-center gap-1">
              <CircleCheckIcon className="size-3.5" />
              {fmt(t.run.doneCount, { n: progress.done })}
            </span>
            <span className="flex items-center gap-1">
              <MinusCircleIcon className="size-3.5" />
              {fmt(t.run.skippedCount, { n: progress.skipped })}
            </span>
            <span className="flex items-center gap-1 text-destructive">
              <CircleAlertIcon className="size-3.5" />
              {fmt(t.run.failedCount, {
                n: progress.lines.filter((l) => l.kind === "failed").length,
              })}
            </span>
            {progress.diagram && (
              <span className="flex items-center gap-1">
                <Loader2Icon className="size-3.5 animate-spin" />
                {fmt(t.run.diagramProgress, {
                  index: progress.diagram.index,
                  total: progress.diagram.total,
                  language: progress.diagram.language,
                })}
              </span>
            )}
          </div>
          <div className="mt-auto flex justify-end pt-3">
            <Button variant="outline" onClick={onCancel}>
              {t.run.cancel}
            </Button>
          </div>
        </div>
        <div
          ref={logRef}
          className="h-52 min-h-0 min-[1000px]:h-auto min-[1000px]:flex-1 overflow-y-auto rounded-md border bg-[var(--background-secondary)] p-2 font-mono text-xs leading-5"
        >
          {progress.lines.length === 0 && (
            <span className="text-[var(--text-faint)]">{t.run.waiting}</span>
          )}
          {progress.lines.map((line, i) => (
            <div
              key={i}
              className={LINE_COLOR[line.kind]}
              title={line.detail ?? line.text}
            >
              {LINE_PREFIX[line.kind]} {line.text}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
