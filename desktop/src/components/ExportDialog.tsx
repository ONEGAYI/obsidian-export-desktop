import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { summarizeOptions, type ExportOptions } from "@/lib/options";
import { baseName } from "@/lib/sidecar";

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  keepRootFolder: boolean;
  onKeepRootFolderChange: (value: boolean) => void;
  source: string;
  destination: string;
  options: ExportOptions;
  onEditOptions: () => void;
  onStart: () => void;
}

/**
 * Pre-export confirmation sheet. Export-time choices live here (not in a
 * global settings panel) per the project decision; each choice is persisted
 * by the parent so the next export preselects it. Conversion options are
 * configured in the settings view; this dialog only summarizes them so they
 * stay visible at export time.
 */
export function ExportDialog({
  open,
  onOpenChange,
  keepRootFolder,
  onKeepRootFolderChange,
  source,
  destination,
  options,
  onEditOptions,
  onStart,
}: ExportDialogProps) {
  const effectiveDestination = keepRootFolder
    ? `${destination.replace(/[\\/]+$/, "")}/${baseName(source)}`
    : destination;
  const summary = summarizeOptions(options);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>导出确认</DialogTitle>
          <DialogDescription className="font-mono text-[11px] leading-relaxed break-all">
            {source}
            <br />
            → {effectiveDestination}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-1.5 rounded-md border p-2.5">
          <div className="flex items-center justify-between">
            <span className="text-sm leading-none font-medium">生效选项</span>
            <button
              type="button"
              className="text-xs text-muted-foreground underline underline-offset-2 transition-colors hover:text-[var(--text-normal)]"
              onClick={onEditOptions}
            >
              修改
            </button>
          </div>
          {summary.length === 0 ? (
            <span className="text-muted-foreground text-xs">全部保持默认</span>
          ) : (
            <span className="text-xs leading-relaxed break-words">
              {summary.join(" · ")}
            </span>
          )}
        </div>

        <Label
          className="flex cursor-pointer items-start gap-2.5 rounded-md border p-2.5 font-normal transition-colors hover:bg-[var(--background-modifier-hover)] [&:has([data-state=checked])]:border-[var(--interactive-accent)]"
        >
          <Checkbox
            checked={keepRootFolder}
            onCheckedChange={onKeepRootFolderChange}
            className="mt-0.5"
          />
          <span className="flex flex-col gap-0.5">
            <span className="text-sm leading-none font-medium">
              在目标下保留根文件夹
            </span>
            <span className="text-muted-foreground text-xs">
              导出文件夹时写入「目标/{baseName(source) || "来源文件夹名"}」，
              避免内部第一层文件散落在目标位置（仅文件夹来源生效）。
            </span>
          </span>
        </Label>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button onClick={onStart}>开始导出</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
