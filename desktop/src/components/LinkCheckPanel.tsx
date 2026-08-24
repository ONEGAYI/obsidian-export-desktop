import { useState } from "react";
import { Link2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { fmt, useI18n } from "@/i18n";
import type { Dict } from "@/i18n/zh";
import type { CheckEvent, CheckStatus, LinkKind } from "@/lib/sidecar";

/** One checked link as kept in the app state (a flattened link-report). */
export interface LinkReportEntry {
  source: string;
  line: number;
  raw: string;
  kind: LinkKind;
  status: CheckStatus;
}

export interface LinkCheckSummary {
  filesChecked: number;
  totalLinks: number;
  broken: number;
  skipped: number;
}

export type LinkCheckPhase = "idle" | "running" | "done" | "failed";

export interface LinkCheckState {
  phase: LinkCheckPhase;
  /** Link reports received so far (the panel filters by verdict client-side). */
  reports: LinkReportEntry[];
  end: LinkCheckSummary | null;
  exit: { code: number | null; stderr: string } | null;
  /** Failure of the start_check invoke itself (channel errors still go to the log). */
  invokeError: string | null;
}

export const EMPTY_LINK_CHECK: LinkCheckState = {
  phase: "idle",
  reports: [],
  end: null,
  exit: null,
  invokeError: null,
};

/** Fold one check event into the panel state; schema/check-start carry nothing. */
export function foldCheckEvent(
  state: LinkCheckState,
  event: CheckEvent,
): LinkCheckState {
  switch (event.type) {
    case "schema":
    case "check-start":
      return state;
    case "link-report":
      return {
        ...state,
        reports: [
          ...state.reports,
          {
            source: event.source,
            line: event.line,
            raw: event.raw,
            kind: event.kind,
            status: event.status,
          },
        ],
      };
    case "check-end":
      return {
        ...state,
        end: {
          filesChecked: event.filesChecked,
          totalLinks: event.totalLinks,
          broken: event.broken,
          skipped: event.skipped,
        },
      };
  }
}

/** Broken = neither healthy nor an intentionally skipped external URL. */
export function isBroken(status: CheckStatus): boolean {
  return status.type !== "ok" && status.type !== "external-skipped";
}

/** Render cap: a big vault can produce tens of thousands of reports; the
 * list stays interactive while the counts above always reflect everything. */
const LIST_LIMIT = 300;

type Filter = "broken" | "all" | "skipped";

/** Localize a structured verdict; paths and raw link text stay as-is. */
function statusText(status: CheckStatus, t: Dict): string {
  switch (status.type) {
    case "ok":
      return t.linkCheck.statusOk;
    case "missing-file":
      return fmt(t.linkCheck.statusMissingFile, { target: status.target });
    case "out-of-bounds":
      return fmt(t.linkCheck.statusOutOfBounds, { target: status.target });
    case "missing-section":
      return fmt(t.linkCheck.statusMissingSection, {
        target: status.target,
        section: status.section,
      });
    case "missing-block":
      return fmt(t.linkCheck.statusMissingBlock, {
        target: status.target,
        block: status.block,
      });
    case "file-unreadable":
      return fmt(t.linkCheck.statusUnreadable, { message: status.message });
    case "external-skipped":
      return fmt(t.linkCheck.statusExternal, { url: status.url });
    case "unknown":
      return t.linkCheck.statusUnknown;
  }
}

function kindLabel(kind: LinkKind, t: Dict): string {
  const labels = {
    "wiki-link": t.linkCheck.kinds.wikiLink,
    "wiki-embed": t.linkCheck.kinds.wikiEmbed,
    "markdown-link": t.linkCheck.kinds.markdownLink,
    "markdown-image": t.linkCheck.kinds.markdownImage,
    unknown: t.linkCheck.kinds.unknown,
  } as const;
  return labels[kind];
}

export function LinkCheckPanel({ state }: { state: LinkCheckState }) {
  const { t } = useI18n();
  const [filter, setFilter] = useState<Filter>("broken");

  const brokenCount = state.reports.filter((r) => isBroken(r.status)).length;
  const skippedCount = state.reports.filter(
    (r) => r.status.type === "external-skipped",
  ).length;

  const title =
    state.phase === "running"
      ? t.linkCheck.runningTitle
      : state.phase === "failed"
        ? t.linkCheck.titleFailed
        : state.end && state.end.broken > 0
          ? fmt(t.linkCheck.titleBroken, { n: state.end.broken })
          : t.linkCheck.titleClean;

  const description =
    state.phase === "running"
      ? fmt(t.linkCheck.runningProgress, { n: state.reports.length })
      : state.phase === "failed"
        ? t.linkCheck.failedHint
        : state.end
          ? fmt(t.linkCheck.summary, {
              files: state.end.filesChecked,
              links: state.end.totalLinks,
              broken: state.end.broken,
              skipped: state.end.skipped,
            })
          : "";

  // Counts come from the check-end summary once it lands (authoritative even
  // if rendering caps the list); before that, running counts from the reports.
  const showBroken = state.end ? state.end.broken : brokenCount;
  const showSkipped = state.end ? state.end.skipped : skippedCount;

  const visible =
    filter === "broken"
      ? state.reports.filter((r) => isBroken(r.status))
      : filter === "skipped"
        ? state.reports.filter((r) => r.status.type === "external-skipped")
        : state.reports;
  const shown = visible.slice(0, LIST_LIMIT);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Link2Icon className="size-4" />
          {title}
        </CardTitle>
        <CardDescription>{description}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {state.phase === "failed" && (
          <>
            {state.invokeError && (
              <pre className="text-destructive max-h-24 overflow-auto rounded-md border border-destructive/40 bg-[var(--background-secondary)] p-2 font-mono text-[11px] whitespace-pre-wrap">
                {state.invokeError}
              </pre>
            )}
            {state.exit?.stderr && (
              <pre className="text-destructive max-h-40 overflow-auto rounded-md border border-destructive/40 bg-[var(--background-secondary)] p-2 font-mono text-[11px] whitespace-pre-wrap">
                {state.exit.stderr}
              </pre>
            )}
          </>
        )}

        {state.phase === "done" && (
          <>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex gap-1">
                {(["broken", "all", "skipped"] as const).map((f) => (
                  <Button
                    key={f}
                    variant={filter === f ? "secondary" : "ghost"}
                    size="sm"
                    onClick={() => setFilter(f)}
                  >
                    {t.linkCheck.filter[f]}
                    <span className="text-muted-foreground">
                      ({f === "broken" ? showBroken : f === "skipped" ? showSkipped : state.reports.length})
                    </span>
                  </Button>
                ))}
              </div>
            </div>

            {shown.length === 0 ? (
              <p className="text-muted-foreground text-xs">
                {t.linkCheck.emptyList}
              </p>
            ) : (
              <div className="flex max-h-64 flex-col gap-1.5 overflow-y-auto">
                {shown.map((entry, i) => {
                  const broken = isBroken(entry.status);
                  return (
                    <div
                      key={`${entry.source}:${entry.line}:${i}`}
                      className="rounded-md border px-2.5 py-1.5 text-xs"
                    >
                      <div className="flex items-baseline justify-between gap-2">
                        <span className="font-mono">
                          {entry.source}:{entry.line}
                        </span>
                        <span
                          className={
                            broken
                              ? "text-destructive"
                              : entry.status.type === "external-skipped"
                                ? "text-[var(--text-faint)]"
                                : "text-[var(--text-success)]"
                          }
                        >
                          {statusText(entry.status, t)}
                        </span>
                      </div>
                      <div className="mt-1 flex items-baseline justify-between gap-2">
                        <code className="text-muted-foreground font-mono break-all">
                          {entry.raw}
                        </code>
                        <span className="text-[var(--text-faint)] shrink-0">
                          {kindLabel(entry.kind, t)}
                        </span>
                      </div>
                    </div>
                  );
                })}
                {visible.length > shown.length && (
                  <p className="text-muted-foreground text-xs">
                    {fmt(t.linkCheck.truncated, {
                      shown: shown.length,
                      total: visible.length,
                    })}
                  </p>
                )}
              </div>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}
