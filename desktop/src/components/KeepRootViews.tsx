import { FolderOutputIcon } from "lucide-react";

import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { useI18n } from "@/i18n";
import {
  isFileSource,
  isPreviewUsable,
  type PreviewState,
} from "@/lib/preview";

interface KeepRootAreaProps {
  state: PreviewState;
  keepRootFolder: boolean;
  onKeepRootChange: (value: boolean) => void;
}

/** One rendered preview line shared by both layouts (text + tone + extras). */
interface PreviewLine {
  text: string;
  tone: "faint" | "muted" | "error" | "path";
  /** Full output path when ready (drives the copyable popover), else null. */
  path: string | null;
  /** Technical error message when the preview failed, else null. */
  detail: string | null;
}

/** Preview state → the one line both layouts render. */
function previewLine(state: PreviewState, waiting: string, resolving: string, missing: string, failed: string): PreviewLine {
  switch (state.phase) {
    case "idle":
      return { text: waiting, tone: "faint", path: null, detail: null };
    case "pending":
      return { text: resolving, tone: "muted", path: null, detail: null };
    case "ready":
      return isPreviewUsable(state)
        ? { text: state.target, tone: "path", path: state.target, detail: null }
        : { text: missing, tone: "error", path: null, detail: null };
    case "failed":
      return { text: failed, tone: "error", path: null, detail: state.message };
  }
}

const TONE_CLASS: Record<"faint" | "muted" | "error" | "path", string> = {
  faint: "text-[var(--text-faint)]",
  muted: "text-muted-foreground",
  error: "text-destructive",
  path: "",
};

/**
 * Short-height layout (viewport < 640 CSS px): keep-root checkbox + output
 * path compressed into one pill. Always a single line: the checkbox group
 * never shrinks, the path truncates at the tail (the full text is reachable
 * on hover and keyboard focus via the popover, selectable and copyable — a
 * bare `title` alone would leave keyboard users stranded). Tail truncation
 * can hide the final folder name — a deliberate trade-off against the spec's
 * "prefer keeping the last segment": middle truncation needs extra machinery,
 * and the popover carries the full text either way.
 */
export function KeepRootCapsule({
  state,
  keepRootFolder,
  onKeepRootChange,
}: KeepRootAreaProps) {
  const { t } = useI18n();
  const line = previewLine(
    state,
    t.preview.waiting,
    t.preview.resolving,
    t.preview.sourceMissing,
    t.preview.failed,
  );
  return (
    <div className="flex h-10 shrink-0 items-center gap-2.5 rounded-full border border-pill-border bg-pill px-3.5">
      <Checkbox
        checked={keepRootFolder}
        onCheckedChange={(v) => onKeepRootChange(v === true)}
        aria-label={t.app.keepRootTitle}
        className="shrink-0"
      />
      <span className="shrink-0 text-xs font-medium whitespace-nowrap">
        {t.keepRoot.label}
      </span>
      <span aria-hidden className="h-4 w-px shrink-0 bg-border" />
      <FolderOutputIcon
        aria-hidden
        className="size-3.5 shrink-0 text-muted-foreground"
      />
      <span className="shrink-0 text-[11px] text-muted-foreground">
        {t.keepRoot.outputTag}
      </span>
      <span aria-hidden className="h-4 w-px shrink-0 bg-border" />
      <div className="group relative min-w-0 flex-1">
        {/* No native `title` here: hover and keyboard focus both surface the
         * self-drawn popover below (a second native tooltip would stack on
         * top of it); screen readers get the full text via aria-label. */}
        <span
          tabIndex={0}
          aria-label={
            line.path ?? (line.detail !== null ? `${line.text} (${line.detail})` : line.text)
          }
          className={`block truncate font-mono text-xs ${TONE_CLASS[line.tone]}`}
        >
          {line.text}
        </span>
        {(line.path !== null || line.detail !== null) && (
          <div className="invisible absolute inset-x-0 top-full z-20 pt-1 group-hover:visible group-focus-within:visible">
            <div className="rounded-md border bg-popover p-2 font-mono text-xs shadow-md break-all select-text">
              {line.path ?? line.detail}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Enough-height layout (viewport >= 640 CSS px): the same keep-root state as
 * the pill, with a short explanation and the full output path on its own
 * line (wraps instead of truncating; selectable text).
 */
export function KeepRootExpanded({
  state,
  keepRootFolder,
  onKeepRootChange,
}: KeepRootAreaProps) {
  const { t } = useI18n();
  const line = previewLine(
    state,
    t.preview.waiting,
    t.preview.resolving,
    t.preview.sourceMissing,
    t.preview.failed,
  );
  // A single-note source keeps the preference but the rule never applies to
  // it — say so instead of silently ignoring the checkbox.
  const fileNote = isFileSource(state) && keepRootFolder;
  return (
    <div className="flex shrink-0 flex-col gap-2.5 rounded-xl border bg-card p-4">
      <Label className="flex cursor-pointer items-center gap-2.5 font-normal">
        <Checkbox
          checked={keepRootFolder}
          onCheckedChange={(v) => onKeepRootChange(v === true)}
        />
        <span className="text-sm leading-none font-medium">
          {t.app.keepRootTitle}
        </span>
      </Label>
      <span className="text-xs text-muted-foreground">{t.keepRoot.hint}</span>
      <div className="flex flex-col gap-1 rounded-lg border border-pill-border bg-pill px-3 py-2.5">
        <span className="text-[11px] text-muted-foreground">
          {t.preview.label}
        </span>
        <span
          className={`text-xs break-all select-text ${
            line.tone === "path" ? "font-mono" : TONE_CLASS[line.tone]
          }`}
        >
          {line.text}
        </span>
        {state.phase === "failed" && (
          <details>
            <summary className="cursor-pointer text-[11px] text-muted-foreground">
              {t.preview.failedDetail}
            </summary>
            <span className="font-mono text-[11px] break-all select-text">
              {state.message}
            </span>
          </details>
        )}
        {fileNote && (
          <span className="text-[11px] text-[var(--text-faint)]">
            {t.preview.fileKeepRootNote}
          </span>
        )}
      </div>
    </div>
  );
}
