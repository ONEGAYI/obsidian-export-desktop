import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { fmt, useI18n } from "@/i18n";
import { summarizeOptions, type ExportOptions } from "@/lib/options";
import type { PreviewState } from "@/lib/preview";
import { baseName } from "@/lib/sidecar";

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Read-only restatement: the option is edited on the home view. */
  keepRootFolder: boolean;
  source: string;
  destination: string;
  /** Shared preview state (same resolver as the spawn); see lib/preview. */
  preview: PreviewState;
  options: ExportOptions;
  onEditOptions: () => void;
  onStart: () => void;
}

/**
 * Pre-export confirmation sheet. Export-time choices are edited on the home
 * view (the keep-root checkbox) and in the settings view (conversion
 * options); this dialog only restates them read-only so what will run stays
 * visible at export time. The landing path comes from the shared preview;
 * when the preview is unusable the raw destination is shown and labeled as
 * such — the backend verdict at start time remains the authority.
 */
export function ExportDialog({
  open,
  onOpenChange,
  keepRootFolder,
  source,
  destination,
  preview,
  options,
  onEditOptions,
  onStart,
}: ExportDialogProps) {
  const { t } = useI18n();
  const summary = summarizeOptions(options, t);
  const rootName = baseName(source) || t.app.keepRootFallbackName;
  const landingPath =
    preview.phase === "ready" && preview.sourceKind !== "other"
      ? preview.target
      : destination;
  const previewUnusable =
    preview.phase !== "ready" || preview.sourceKind === "other";
  const fileNote =
    preview.phase === "ready" && preview.sourceKind === "file" && keepRootFolder;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t.dialog.title}</DialogTitle>
          <DialogDescription className="font-mono text-[11px] leading-relaxed break-all">
            {source}
            <br />
            → {landingPath}
          </DialogDescription>
          {preview.phase === "pending" && (
            <DialogDescription>{t.preview.resolving}</DialogDescription>
          )}
          {previewUnusable && preview.phase !== "pending" && (
            <DialogDescription className="text-xs text-destructive">
              {preview.phase === "ready"
                ? t.preview.sourceMissing
                : t.dialog.previewUnavailable}
            </DialogDescription>
          )}
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

        <div className="flex flex-col gap-0.5 rounded-md border p-2.5">
          <span className="text-sm leading-none font-medium">
            {keepRootFolder
              ? t.dialog.keepRootSummaryOn
              : t.dialog.keepRootSummaryOff}
          </span>
          <span className="text-muted-foreground text-xs">
            {fmt(t.app.keepRootDescription, { name: rootName })}
          </span>
          {fileNote && (
            <span className="text-[var(--text-faint)] text-xs">
              {t.preview.fileKeepRootNote}
            </span>
          )}
        </div>

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
