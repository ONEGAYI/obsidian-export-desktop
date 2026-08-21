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
  source: string;
  destination: string;
  onStart: () => void;
}

/**
 * Pre-export confirmation sheet. The missing-section strategy lives here (not in
 * a global settings panel) per the project decision, and the choice is persisted
 * by the parent so the next export preselects it.
 */
export function ExportDialog({
  open,
  onOpenChange,
  missingSection,
  onMissingSectionChange,
  source,
  destination,
  onStart,
}: ExportDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>导出确认</DialogTitle>
          <DialogDescription className="font-mono text-[11px] leading-relaxed">
            {baseName(source) || source} → {baseName(destination) || destination}
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
