import { CheckIcon, PencilIcon } from "lucide-react";

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useI18n } from "@/i18n";
import { summarizeOptionEntries, type ExportOptions } from "@/lib/options";

interface ConfigSummaryProps {
  options: ExportOptions;
  /** Opens the same settings view the header's options button leads to. */
  onEditOptions: () => void;
  className?: string;
}

/** Entries already rendered by a dedicated group above — never list twice. */
const DEDICATED_KEYS = new Set([
  "frontmatter",
  "comments",
  "noRecursiveEmbeds",
  "diagramRenderers",
  "linkCheck",
]);

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-border/60 py-1.5 last:border-b-0">
      <span className="shrink-0 text-sm">{label}</span>
      <span className="min-w-0 text-right text-sm break-words">{value}</span>
    </div>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col">
      <h3 className="mb-1 text-xs font-medium tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      {children}
    </section>
  );
}

/**
 * Read-only config column for the wide layout (>= 1000 CSS px): the real
 * values of the current `ExportOptions`, grouped per spec #37 §4.4. "Saved"
 * refers only to the options being persisted — never to sources, tools, or
 * the output directory being validated.
 */
export function ConfigSummary({
  options,
  onEditOptions,
  className,
}: ConfigSummaryProps) {
  const { t } = useI18n();
  const renderersEnabled = options.diagramRenderers.length > 0;
  const rendererNames = options.diagramRenderers.map(
    (r) => t.options.diagramRendererChoices[r].label,
  );
  const others = summarizeOptionEntries(options, t).filter(
    (entry) => !DEDICATED_KEYS.has(entry.key),
  );

  return (
    <Card className={`flex min-h-0 flex-col ${className ?? ""}`}>
      <CardHeader className="shrink-0 border-b">
        <CardTitle className="flex items-center justify-between text-base">
          {t.configSummary.title}
          <button
            type="button"
            className="flex items-center gap-1 text-xs font-normal text-[var(--interactive-accent)] underline-offset-2 transition-colors hover:underline"
            onClick={onEditOptions}
          >
            <PencilIcon className="size-3.5" />
            {t.configSummary.edit}
          </button>
        </CardTitle>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto">
        <Group title={t.configSummary.groupConversion}>
          <Row
            label={t.configSummary.frontmatter}
            value={t.options.frontmatterChoices[options.frontmatter].label}
          />
          <Row
            label={t.configSummary.comments}
            value={t.options.commentsChoices[options.comments].label}
          />
          <Row
            label={t.configSummary.embeds}
            value={
              options.noRecursiveEmbeds
                ? t.configSummary.embedsFlat
                : t.configSummary.embedsRecursive
            }
          />
        </Group>
        <Group title={t.configSummary.groupDiagrams}>
          <Row
            label={t.configSummary.renderers}
            value={
              renderersEnabled ? rendererNames.join(" · ") : t.configSummary.renderersNone
            }
          />
          {renderersEnabled && (
            <Row
              label={t.configSummary.renderersFormat}
              value={t.options.diagramFormatChoices[options.diagramFormat].label}
            />
          )}
        </Group>
        <Group title={t.configSummary.groupLinkCheck}>
          <Row
            label={t.configSummary.linkCheck}
            value={options.linkCheckEnabled ? t.configSummary.on : t.configSummary.off}
          />
          <Row
            label={t.configSummary.linkCheckTarget}
            value={
              options.linkCheckEnabled
                ? t.options.linkCheckTargetChoices[options.linkCheckTarget].label
                : t.configSummary.notApplicable
            }
          />
        </Group>
        <Group title={t.configSummary.groupOther}>
          {others.length === 0 ? (
            <span className="py-1.5 text-sm text-muted-foreground">
              {t.configSummary.otherNone}
            </span>
          ) : (
            others.map((entry) => (
              <div
                key={entry.key}
                className="border-b border-border/60 py-1.5 text-sm last:border-b-0"
              >
                {entry.text}
              </div>
            ))
          )}
        </Group>
        <p className="mt-auto flex shrink-0 items-center gap-1.5 pt-2 text-xs text-muted-foreground">
          <CheckIcon className="size-3.5" />
          {t.configSummary.saved}
        </p>
      </CardContent>
    </Card>
  );
}
