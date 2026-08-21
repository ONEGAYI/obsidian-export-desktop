import { CircleAlertIcon, CircleCheckIcon, MinusCircleIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { SidecarExit } from "@/lib/sidecar";
import { baseName } from "@/lib/sidecar";

export interface ExportResultData {
  total: number;
  done: number;
  skipped: number;
  endSeen: boolean;
  failures: { path: string; message: string }[];
  warnings: { path: string | null; message: string }[];
}

export function ExportResultView({
  progress,
  exit,
  cancelled,
  onRestart,
}: {
  progress: ExportResultData;
  exit: SidecarExit | null;
  cancelled: boolean;
  onRestart: () => void;
}) {
  // No end event means the run never finished cleanly: killed, crashed, or
  // failed before processing (see the sidecar contract).
  const aborted = !progress.endSeen;

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {cancelled
            ? "导出已取消"
            : aborted
              ? "导出异常终止"
              : progress.failures.length > 0
                ? "导出完成（部分失败）"
                : "导出完成"}
        </CardTitle>
        <CardDescription>
          {aborted
            ? "事件流未正常终结，以下为已处理的部分。"
            : `共 ${progress.total} 篇：${progress.done} 成功 · ${progress.skipped} 跳过 · ${progress.failures.length} 失败`}
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="text-muted-foreground flex items-center gap-4 text-xs">
          <span className="flex items-center gap-1">
            <CircleCheckIcon className="size-3.5" />
            {progress.done}
          </span>
          <span className="flex items-center gap-1">
            <MinusCircleIcon className="size-3.5" />
            {progress.skipped}
          </span>
          <span className="text-destructive flex items-center gap-1">
            <CircleAlertIcon className="size-3.5" />
            {progress.failures.length}
          </span>
          {progress.warnings.length > 0 && (
            <span className="flex items-center gap-1 text-yellow-500">
              <CircleAlertIcon className="size-3.5" />
              {progress.warnings.length} 警告
            </span>
          )}
        </div>

        {progress.failures.length > 0 && (
          <div className="flex flex-col gap-2">
            {progress.failures.map((failure) => (
              <details
                key={failure.path}
                className="rounded-md border px-2.5 py-2 text-xs"
              >
                <summary className="text-destructive cursor-pointer select-none font-mono">
                  ✗ {baseName(failure.path)}
                </summary>
                <pre className="text-muted-foreground mt-2 max-h-40 overflow-auto rounded bg-[var(--background-secondary)] p-2 font-mono text-[11px] whitespace-pre-wrap">
                  {failure.message}
                </pre>
              </details>
            ))}
          </div>
        )}

        {aborted && exit?.stderr && (
          <pre className="text-destructive max-h-40 overflow-auto rounded-md border border-destructive/40 bg-[var(--background-secondary)] p-2 font-mono text-[11px] whitespace-pre-wrap">
            {exit.stderr}
          </pre>
        )}

        <div className="flex justify-end">
          <Button onClick={onRestart}>返回</Button>
        </div>
      </CardContent>
    </Card>
  );
}
