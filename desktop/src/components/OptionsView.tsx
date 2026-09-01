import { useState } from "react";
import {
  ArrowLeftIcon,
  ArrowRightLeftIcon,
  FileCogIcon,
  FilterIcon,
  InfoIcon,
  Link2Icon,
  RotateCcwIcon,
  ShapesIcon,
} from "lucide-react";

import { PathPicker } from "@/components/PathPicker";
import { TagInput } from "@/components/TagInput";
import { UpdatePanel, type UpdateState } from "@/components/UpdatePanel";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Switch } from "@/components/ui/switch";
import { fmt, useI18n } from "@/i18n";
import {
  DEFAULT_OPTIONS,
  DIAGRAM_FORMAT_VALUES,
  DIAGRAM_RENDERER_VALUES,
  DIAGRAM_TOOL_VALUES,
  FRONTMATTER_VALUES,
  LINK_CHECK_TARGET_VALUES,
  MISSING_SECTION_VALUES,
  type DiagramFormat,
  type ExportOptions,
  type FrontmatterStrategy,
  type LinkCheckTarget,
  type MissingSectionStrategy,
} from "@/lib/options";

/** Update actions handed down from App (the sidecar slots live there). */
export interface UpdateHandlers {
  state: UpdateState;
  onCheckNow: () => void;
  onDownload: () => void;
  onInstall: () => void;
  onCancelDownload: () => void;
}

interface OptionsViewProps {
  options: ExportOptions;
  onOptionsChange: (options: ExportOptions) => void;
  onBack: () => void;
  update: UpdateHandlers;
}

/** Settings pages: each maps to one option group in the side-nav. */
type Page =
  | "conversion"
  | "filtering"
  | "process"
  | "diagrams"
  | "linkCheck"
  | "about";

/**
 * Radio list for a string-enum option: one card with one row per choice
 * (separated by hairlines, selected row highlighted), so the choices read as
 * a single group clearly distinct from the switch rows below.
 */
function EnumChoice<T extends string>({
  value,
  choices,
  groupLabel,
  onChange,
}: {
  value: T;
  choices: { value: T; label: string; description: string }[];
  /** Accessible name for the radio group (the visible FieldLabel text). */
  groupLabel: string;
  onChange: (value: T) => void;
}) {
  return (
    <RadioGroup
      value={value}
      onValueChange={(v) => onChange(v as T)}
      aria-label={groupLabel}
      className="gap-0 overflow-hidden rounded-md border bg-[var(--background-primary)]"
    >
      {choices.map((choice) => (
        <Label
          key={choice.value}
          className="flex cursor-pointer items-start gap-2.5 border-b border-b-[var(--background-modifier-border)] p-2.5 font-normal transition-colors last:border-b-0 hover:bg-[var(--background-modifier-hover)] [&:has([data-state=checked])]:bg-[var(--background-modifier-hover)]"
        >
          <RadioGroupItem value={choice.value} className="mt-0.5" />
          <span className="flex flex-col gap-0.5">
            <span className="text-sm leading-none font-medium">
              {choice.label}
            </span>
            <span className="text-muted-foreground text-xs">
              {choice.description}
            </span>
          </span>
        </Label>
      ))}
    </RadioGroup>
  );
}

function SwitchRow({
  title,
  description,
  checked,
  onCheckedChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  const { t } = useI18n();
  return (
    <div className="flex items-start justify-between gap-3 rounded-md border bg-[var(--background-primary)] p-2.5">
      <span className="flex flex-col gap-0.5">
        <span className="text-sm leading-none font-medium">{title}</span>
        <span className="text-muted-foreground text-xs">{description}</span>
      </span>
      {/* The name carries the state: AX-tree walkers don't render
          ToggleState, so without it a toggle leaves the tree unchanged. */}
      <Switch
        checked={checked}
        onCheckedChange={onCheckedChange}
        aria-label={fmt(
          checked
            ? t.common.statefulControl.nameOn
            : t.common.statefulControl.nameOff,
          { title },
        )}
        className="mt-0.5"
      />
    </div>
  );
}

function FieldLabel({ children }: { children: React.ReactNode }) {
  return <span className="text-muted-foreground text-xs">{children}</span>;
}

/**
 * Pill-shaped checkbox (semantic checkbox, visual pill): selected pills fill
 * with the accent color. Used for the diagram renderer multi-select.
 */
function PillCheckbox({
  label,
  hint,
  checked,
  onCheckedChange,
}: {
  label: string;
  /** Extra description surfaced as a tooltip. */
  hint: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      title={hint}
      aria-label={fmt(
        checked
          ? t.common.statefulControl.nameOn
          : t.common.statefulControl.nameOff,
        { title: label },
      )}
      onClick={() => onCheckedChange(!checked)}
      className={`rounded-full border px-3 py-1.5 text-sm whitespace-nowrap transition-colors ${
        checked
          ? "border-transparent bg-[var(--interactive-accent)] text-[var(--text-on-accent)]"
          : "bg-[var(--background-primary)] text-muted-foreground hover:bg-[var(--background-modifier-hover)]"
      }`}
    >
      {label}
    </button>
  );
}

/**
 * Full options panel mirroring the CLI flags of the sidecar, paginated into
 * a side-nav (page selection) and a content area. All choices are persisted
 * by the parent as they are made; only non-default values are forwarded to
 * the CLI (see build_args in src-tauri/src/sidecar.rs).
 */
export function OptionsView({
  options,
  onOptionsChange,
  onBack,
  update,
}: OptionsViewProps) {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>("conversion");
  const patch = (partial: Partial<ExportOptions>) =>
    onOptionsChange({ ...options, ...partial });

  // Choice labels live in the i18n dictionaries; the legal values come from
  // the options module (also used to sanitize the persisted payload).
  const frontmatterChoices = FRONTMATTER_VALUES.map((value) => ({
    value,
    ...t.options.frontmatterChoices[value],
  })) satisfies { value: FrontmatterStrategy; label: string; description: string }[];
  const missingSectionChoices = MISSING_SECTION_VALUES.map((value) => ({
    value,
    ...t.options.missingSectionChoices[value],
  })) satisfies { value: MissingSectionStrategy; label: string; description: string }[];
  const linkCheckTargetChoices = LINK_CHECK_TARGET_VALUES.map((value) => ({
    value,
    ...t.options.linkCheckTargetChoices[value],
  })) satisfies { value: LinkCheckTarget; label: string; description: string }[];
  const diagramFormatChoices = DIAGRAM_FORMAT_VALUES.map((value) => ({
    value,
    ...t.options.diagramFormatChoices[value],
  })) satisfies { value: DiagramFormat; label: string; description: string }[];

  const navItems: {
    id: Page;
    label: string;
    icon: React.ReactNode;
  }[] = [
    { id: "conversion", label: t.options.sectionConversion, icon: <ArrowRightLeftIcon className="size-4 shrink-0" /> },
    { id: "filtering", label: t.options.sectionFiltering, icon: <FilterIcon className="size-4 shrink-0" /> },
    { id: "process", label: t.options.sectionProcess, icon: <FileCogIcon className="size-4 shrink-0" /> },
    { id: "diagrams", label: t.options.sectionDiagrams, icon: <ShapesIcon className="size-4 shrink-0" /> },
    { id: "linkCheck", label: t.options.sectionLinkCheck, icon: <Link2Icon className="size-4 shrink-0" /> },
    { id: "about", label: t.options.sectionAbout, icon: <InfoIcon className="size-4 shrink-0" /> },
  ];

  return (
    <Card className="flex max-h-[calc(100vh-5rem)] flex-col">
      <CardHeader className="shrink-0">
        <div className="flex items-center justify-between gap-1.5">
          <div className="flex items-center gap-1.5">
            <Button
              variant="ghost"
              size="icon"
              onClick={onBack}
              aria-label={t.options.back}
            >
              <ArrowLeftIcon className="size-4" />
            </Button>
            <CardTitle>{t.options.title}</CardTitle>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onOptionsChange(DEFAULT_OPTIONS)}
          >
            <RotateCcwIcon className="size-3.5" />
            {t.options.resetDefaults}
          </Button>
        </div>
        <CardDescription>{t.options.description}</CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 p-0">
        {/* The card is capped at the viewport (minus title bar and page
            padding); every level down to the panel must allow shrinking so
            the overflow lands on the option panel, not the whole page. */}
        <div className="grid h-full grid-rows-[auto_minmax(0,1fr)] sm:grid-cols-[150px_minmax(0,1fr)] sm:grid-rows-[minmax(0,1fr)]">
          {/* Side-nav: soft background with the selected page floating on it
              (macOS System Settings style). Collapses to a horizontal tab
              strip on narrow windows. */}
          <nav
            role="tablist"
            aria-label={t.options.title}
            className="flex flex-row gap-1 overflow-x-auto border-b bg-[var(--background-secondary)] p-2 sm:flex-col sm:overflow-y-auto sm:border-r sm:border-b-0"
          >
            {navItems.map((item) => (
              <button
                key={item.id}
                type="button"
                role="tab"
                id={`settings-tab-${item.id}`}
                aria-selected={page === item.id}
                aria-controls="settings-tabpanel"
                onClick={() => setPage(item.id)}
                className={`flex items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm whitespace-nowrap transition-colors sm:justify-start ${
                  page === item.id
                    ? "bg-[var(--background-primary)] font-semibold text-[var(--interactive-accent)]"
                    : "text-muted-foreground hover:bg-[var(--background-modifier-hover)]"
                }`}
              >
                {item.icon}
                {item.label}
                {/* First-level brief for the diagram page: how many renderers
                    are on, visible without opening the page. */}
                {item.id === "diagrams" &&
                  options.diagramRenderers.length > 0 && (
                    <span className="ml-auto rounded-full bg-[var(--interactive-accent)] px-1.5 py-0.5 text-[10px] leading-none font-semibold text-[var(--text-on-accent)]">
                      {options.diagramRenderers.length}/
                      {DIAGRAM_RENDERER_VALUES.length}
                    </span>
                  )}
              </button>
            ))}
          </nav>

          <div
            role="tabpanel"
            id="settings-tabpanel"
            aria-labelledby={`settings-tab-${page}`}
            className="flex min-w-0 flex-col gap-4 overflow-y-auto p-4"
          >
            {page === "conversion" && (
              <section className="flex flex-col gap-2.5">
                <h3 className="text-sm font-semibold">{t.options.sectionConversion}</h3>
                <div className="flex max-w-lg flex-col gap-2.5">
                  <div className="flex flex-col gap-1.5 pb-2.5">
                    <FieldLabel>{t.options.frontmatterLabel}</FieldLabel>
                    <EnumChoice
                      value={options.frontmatter}
                      choices={frontmatterChoices}
                      groupLabel={t.options.frontmatterLabel}
                      onChange={(frontmatter) => patch({ frontmatter })}
                    />
                  </div>
                  <div className="flex flex-col gap-1.5 pb-2.5">
                    <FieldLabel>{t.options.missingSectionLabel}</FieldLabel>
                    <EnumChoice
                      value={options.missingSection}
                      choices={missingSectionChoices}
                      groupLabel={t.options.missingSectionLabel}
                      onChange={(missingSection) => patch({ missingSection })}
                    />
                  </div>
                  <SwitchRow
                    title={t.options.hardLinebreaks.title}
                    description={t.options.hardLinebreaks.description}
                    checked={options.hardLinebreaks}
                    onCheckedChange={(hardLinebreaks) => patch({ hardLinebreaks })}
                  />
                  <SwitchRow
                    title={t.options.noRecursiveEmbeds.title}
                    description={t.options.noRecursiveEmbeds.description}
                    checked={options.noRecursiveEmbeds}
                    onCheckedChange={(noRecursiveEmbeds) =>
                      patch({ noRecursiveEmbeds })
                    }
                  />
                </div>
              </section>
            )}

            {page === "filtering" && (
              <section className="flex flex-col gap-2.5">
                <h3 className="text-sm font-semibold">{t.options.sectionFiltering}</h3>
                <div className="flex max-w-lg flex-col gap-2.5">
                  <SwitchRow
                    title={t.options.hidden.title}
                    description={t.options.hidden.description}
                    checked={options.hidden}
                    onCheckedChange={(hidden) => patch({ hidden })}
                  />
                  <SwitchRow
                    title={t.options.noGit.title}
                    description={t.options.noGit.description}
                    checked={options.noGit}
                    onCheckedChange={(noGit) => patch({ noGit })}
                  />
                  <div className="flex flex-col gap-1.5">
                    <FieldLabel>{t.options.ignoreFileLabel}</FieldLabel>
                    <Input
                      value={options.ignoreFile ?? ""}
                      placeholder={t.options.ignoreFilePlaceholder}
                      onChange={(e) =>
                        patch({
                          // No trimming here: spaces must be typeable (file names
                          // may contain them). Blank handling lives in the Rust
                          // build_args filter.
                          ignoreFile: e.target.value === "" ? null : e.target.value,
                        })
                      }
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <FieldLabel>{t.options.skipTagsLabel}</FieldLabel>
                    <TagInput
                      value={options.skipTags}
                      onChange={(skipTags) => patch({ skipTags })}
                      placeholder={t.options.skipTagsPlaceholder}
                    />
                  </div>
                  <div className="flex flex-col gap-1.5">
                    <FieldLabel>{t.options.onlyTagsLabel}</FieldLabel>
                    <TagInput
                      value={options.onlyTags}
                      onChange={(onlyTags) => patch({ onlyTags })}
                      placeholder={t.options.onlyTagsPlaceholder}
                    />
                  </div>
                  <PathPicker
                    label={t.options.startAtLabel}
                    placeholder={t.options.startAtPlaceholder}
                    value={options.startAt ?? ""}
                    onChange={(v) =>
                      patch({ startAt: v === "" ? null : v })
                    }
                    hint={t.options.startAtHint}
                  />
                </div>
              </section>
            )}

            {page === "process" && (
              <section className="flex flex-col gap-2.5">
                <h3 className="text-sm font-semibold">{t.options.sectionProcess}</h3>
                <div className="flex max-w-lg flex-col gap-2.5">
                  <SwitchRow
                    title={t.options.preserveMtime.title}
                    description={t.options.preserveMtime.description}
                    checked={options.preserveMtime}
                    onCheckedChange={(preserveMtime) => patch({ preserveMtime })}
                  />
                  <SwitchRow
                    title={t.options.failFast.title}
                    description={t.options.failFast.description}
                    checked={options.failFast}
                    onCheckedChange={(failFast) => patch({ failFast })}
                  />
                </div>
              </section>
            )}

            {page === "diagrams" && (
              <section className="flex flex-col gap-2.5">
                <h3 className="text-sm font-semibold">{t.options.sectionDiagrams}</h3>
                <div className="flex max-w-lg flex-col gap-2.5">
                  <p className="text-muted-foreground text-xs">
                    {t.options.diagramsDescription}
                  </p>
                  <div className="flex flex-col gap-1.5">
                    <FieldLabel>{t.options.diagramRenderersLabel}</FieldLabel>
                    <div
                      role="group"
                      aria-label={t.options.diagramRenderersLabel}
                      className="flex flex-wrap gap-2"
                    >
                      {DIAGRAM_RENDERER_VALUES.map((renderer) => (
                        <PillCheckbox
                          key={renderer}
                          label={t.options.diagramRendererChoices[renderer].label}
                          hint={t.options.diagramRendererChoices[renderer].description}
                          checked={options.diagramRenderers.includes(renderer)}
                          onCheckedChange={(checked) =>
                            patch({
                              diagramRenderers: checked
                                ? [...options.diagramRenderers, renderer]
                                : options.diagramRenderers.filter(
                                    (r) => r !== renderer,
                                  ),
                            })
                          }
                        />
                      ))}
                    </div>
                  </div>
                  <div className="flex flex-col gap-1.5 pb-2.5">
                    <FieldLabel>{t.options.diagramFormatLabel}</FieldLabel>
                    <EnumChoice
                      value={options.diagramFormat}
                      choices={diagramFormatChoices}
                      groupLabel={t.options.diagramFormatLabel}
                      onChange={(diagramFormat) => patch({ diagramFormat })}
                    />
                    <p className="text-[var(--text-faint)] text-xs">
                      {t.options.diagramFormatFallbackNote}
                    </p>
                  </div>
                  <details className="rounded-md border bg-[var(--background-primary)] p-2.5">
                    <summary className="cursor-pointer text-sm font-medium">
                      {t.options.diagramBinsTitle}
                    </summary>
                    <p className="text-muted-foreground mt-1.5 text-xs">
                      {t.options.diagramBinsHint}
                    </p>
                    <div className="mt-2 flex flex-col gap-2">
                      {DIAGRAM_TOOL_VALUES.map((tool) => (
                        <div key={tool} className="flex flex-col gap-1">
                          <FieldLabel>
                            {t.options.diagramToolNames[tool]}
                          </FieldLabel>
                          <Input
                            value={options.diagramBins[tool] ?? ""}
                            placeholder={t.options.diagramBinsPlaceholder}
                            onChange={(e) => {
                              // Blank means PATH lookup; storing it as absent
                              // keeps the payload and the summary in sync.
                              const bins = { ...options.diagramBins };
                              if (e.target.value === "") {
                                delete bins[tool];
                              } else {
                                bins[tool] = e.target.value;
                              }
                              patch({ diagramBins: bins });
                            }}
                          />
                        </div>
                      ))}
                    </div>
                  </details>
                </div>
              </section>
            )}

            {page === "linkCheck" && (
              <section className="flex flex-col gap-2.5">
                <h3 className="text-sm font-semibold">{t.options.sectionLinkCheck}</h3>
                <div className="flex max-w-lg flex-col gap-2.5">
                  <SwitchRow
                    title={t.options.linkCheckEnable.title}
                    description={t.options.linkCheckEnable.description}
                    checked={options.linkCheckEnabled}
                    onCheckedChange={(linkCheckEnabled) =>
                      patch({ linkCheckEnabled })
                    }
                  />
                  <div className="flex flex-col gap-1.5">
                    <FieldLabel>{t.options.linkCheckTargetLabel}</FieldLabel>
                    <EnumChoice
                      value={options.linkCheckTarget}
                      choices={linkCheckTargetChoices}
                      groupLabel={t.options.linkCheckTargetLabel}
                      onChange={(linkCheckTarget) => patch({ linkCheckTarget })}
                    />
                  </div>
                </div>
              </section>
            )}

            {page === "about" && (
              <UpdatePanel
                state={update.state}
                autoCheckEnabled={options.autoCheckUpdates}
                onAutoCheckChange={(autoCheckUpdates) => patch({ autoCheckUpdates })}
                onCheckNow={update.onCheckNow}
                onDownload={update.onDownload}
                onInstall={update.onInstall}
                onCancelDownload={update.onCancelDownload}
              />
            )}

            <div className="mt-auto border-t pt-3">
              <span className="text-[var(--text-faint)] text-xs">
                {t.options.footer}
              </span>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
