import { useI18n } from "@/i18n";
import type { PreviewState } from "@/lib/preview";

interface OutputPreviewProps {
  state: PreviewState;
  keepRootFolder: boolean;
}

/**
 * Landing-path preview for the home view: renders the shared preview state
 * (resolved by the same backend rule as the export spawn). Shows the resolved
 * path when the source checks out, an honest per-state message otherwise —
 * never a stale path from an older input.
 */
export function OutputPreview({ state, keepRootFolder }: OutputPreviewProps) {
  const { t } = useI18n();
  // A single-note source keeps the user's keep-root preference but the rule
  // never applies to it; say so instead of silently ignoring the checkbox.
  const fileNote =
    state.phase === "ready" && state.sourceKind === "file" && keepRootFolder;

  return (
    <div className="flex flex-col gap-1 rounded-md border bg-[var(--background-secondary)] p-2.5">
      <span className="text-muted-foreground text-xs">{t.preview.label}</span>
      {state.phase === "idle" && (
        <span className="text-[var(--text-faint)] text-xs">
          {t.preview.waiting}
        </span>
      )}
      {state.phase === "pending" && (
        <span className="text-muted-foreground text-xs animate-pulse">
          {t.preview.resolving}
        </span>
      )}
      {state.phase === "ready" && state.sourceKind === "other" && (
        <span className="text-xs text-destructive">
          {t.preview.sourceMissing}
        </span>
      )}
      {state.phase === "ready" && state.sourceKind !== "other" && (
        <span
          className="font-mono text-xs break-all"
          title={state.target}
        >
          {state.target}
        </span>
      )}
      {state.phase === "failed" && (
        <div className="flex flex-col gap-1">
          <span className="text-xs text-destructive">{t.preview.failed}</span>
          <details>
            <summary className="cursor-pointer text-[11px] text-muted-foreground">
              {t.preview.failedDetail}
            </summary>
            <span className="font-mono text-[11px] break-all">
              {state.message}
            </span>
          </details>
        </div>
      )}
      {fileNote && (
        <span className="text-[var(--text-faint)] text-[11px]">
          {t.preview.fileKeepRootNote}
        </span>
      )}
    </div>
  );
}
