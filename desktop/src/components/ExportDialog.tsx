import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  MISSING_SECTION_OPTIONS,
  baseName,
  type MissingSectionStrategy,
} from "@/lib/sidecar";

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  missingSection: MissingSectionStrategy;
  onMissingSectionChange: (value: MissingSectionStrategy) => void;
  keepRootFolder: boolean;
  onKeepRootFolderChange: (value: boolean) => void;
  source: string;
  destination: string;
  onStart: () => void;
}

/**
 * Pre-export confirmation sheet. Export-time choices live here (not in a
 * global settings panel) per the project decision; each choice is persisted
 * by the parent so the next export preselects it.
 */
export function ExportDialog({
  open,
  onOpenChange,
  missingSection,
  onMissingSectionChange,
  keepRootFolder,
  onKeepRootFolderChange,
  source,
  destination,
  onStart,
}: ExportDialogProps) {
  const effectiveDestination = keepRootFolder
    ? `${destination.replace(/[\\/]+$/, "")}/${baseName(source)}`
    : destination;

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

        <div className="flex flex-col gap-2.5">
          <span className="text-sm font-medium">缺失章节的处理方式</span>
          <RadioGroup
            value={missingSection}
            onValueChange={(v) => onMissingSectionChange(v as MissingSectionStrategy)}
            className="gap-2"
          >
            {MISSING_SECTION_OPTIONS.map((option) => (
              <Label
                key={option.value}
                className="flex cursor-pointer items-start gap-2.5 rounded-md border p-2.5 font-normal transition-colors hover:bg-[var(--background-modifier-hover)] [&:has([data-state=checked])]:border-[var(--interactive-accent)]"
              >
                <RadioGroupItem value={option.value} className="mt-0.5" />
                <span className="flex flex-col gap-0.5">
                  <span className="text-sm leading-none font-medium">
                    {option.label}
                  </span>
                  <span className="text-muted-foreground text-xs">
                    {option.description}
                  </span>
                </span>
              </Label>
            ))}
          </RadioGroup>
          <span className="text-[var(--text-faint)] text-xs">
            选择会被记住，下次导出默认沿用。
          </span>
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
