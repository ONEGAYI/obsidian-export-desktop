import { useEffect, useRef } from "react";
import { CircleAlertIcon, CircleCheckIcon, MinusCircleIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";

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
  const logRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [progress.lines.length]);

  const processed = progress.done + progress.skipped;
  const percent =
    progress.total > 0 ? Math.round((processed / progress.total) * 100) : 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>正在导出…</CardTitle>
        <CardDescription>
          {processed} / {progress.total} 篇笔记
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <Progress value={percent} />
        <div className="text-muted-foreground flex items-center gap-4 text-xs">
          <span className="flex items-center gap-1">
            <CircleCheckIcon className="size-3.5" />
            {progress.done} 成功
          </span>
          <span className="flex items-center gap-1">
            <MinusCircleIcon className="size-3.5" />
            {progress.skipped} 跳过
          </span>
          <span className="flex items-center gap-1 text-destructive">
            <CircleAlertIcon className="size-3.5" />
            {progress.lines.filter((l) => l.kind === "failed").length} 失败
          </span>
        </div>
        <div
          ref={logRef}
          className="h-52 overflow-y-auto rounded-md border bg-[var(--background-secondary)] p-2 font-mono text-xs leading-5"
        >
          {progress.lines.length === 0 && (
            <span className="text-[var(--text-faint)]">等待边车事件…</span>
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
        <div className="flex justify-end">
          <Button variant="outline" onClick={onCancel}>
            取消导出
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
