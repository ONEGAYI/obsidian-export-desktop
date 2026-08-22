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
import { fmt, useI18n } from "@/i18n";
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
  const { t } = useI18n();
  const effectiveDestination = keepRootFolder
    ? `${destination.replace(/[\\/]+$/, "")}/${baseName(source)}`
    : destination;
  const summary = summarizeOptions(options, t);
  const rootName = baseName(source) || t.dialog.keepRootFallbackName;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t.dialog.title}</DialogTitle>
          <DialogDescription className="font-mono text-[11px] leading-relaxed break-all">
            {source}
            <br />
            → {effectiveDestination}
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-1.5 rounded-md border p-2.5">
          <div className="flex items-center justify-between">
            <span className="text-sm leading-none font-medium">
              {t.dialog.activeOptions}
            </span>
            <button
              type="button"
              className="text-xs text-muted-foreground underline underline-offset-2 transition-colors hover:text-[var(--text-normal)]"
              onClick={onEditOptions}
            >
              {t.dialog.modify}
            </button>
          </div>
          {summary.length === 0 ? (
            <span className="text-muted-foreground text-xs">
              {t.dialog.allDefault}
            </span>
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
              {t.dialog.keepRootTitle}
            </span>
            <span className="text-muted-foreground text-xs">
              {fmt(t.dialog.keepRootDescription, { name: rootName })}
            </span>
          </span>
        </Label>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t.dialog.cancel}
          </Button>
          <Button onClick={onStart}>{t.dialog.start}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
