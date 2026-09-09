import { FolderInputIcon, FolderOpenIcon, FolderOutputIcon, SettingsIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfigSummary } from "@/components/ConfigSummary";
import { KeepRootCapsule, KeepRootExpanded } from "@/components/KeepRootViews";
import { PathPicker } from "@/components/PathPicker";
import { SidecarErrorCard } from "@/components/SidecarErrorCard";
import { useI18n } from "@/i18n";
import type { ExportOptions } from "@/lib/options";
import type { PreviewState } from "@/lib/preview";
import { useLayoutBreakpoints } from "@/lib/layout";

interface HomeViewProps {
  source: string;
  onSourceChange: (value: string) => void;
  destination: string;
  onDestinationChange: (value: string) => void;
  rememberPaths: boolean;
  onRememberPathsChange: (value: boolean) => void;
  keepRootFolder: boolean;
  onKeepRootChange: (value: boolean) => void;
  preview: PreviewState;
  options: ExportOptions;
  canExport: boolean;
  /** Environment-level sidecar failure banner, shown under the header. */
  sidecarError: string | null;
  onOpenOptions: () => void;
  onExport: () => void;
}

/**
 * Responsive home view (spec #37 §4.1–4.4). Width and height are independent
 * conditions: `wide` adds the read-only config column on the right, `tall`
 * swaps the keep-root/output pill for the expanded block. Collapsed regions
 * are removed from the DOM (no CSS-hidden focusables), and the bottom action
 * bar is a fixed-height row — the middle area scrolls when content overflows
 * (tiny window, enlarged fonts, long error text).
 */
export function HomeView({
  source,
  onSourceChange,
  destination,
  onDestinationChange,
  rememberPaths,
  onRememberPathsChange,
  keepRootFolder,
  onKeepRootChange,
  preview,
  options,
  canExport,
  sidecarError,
  onOpenOptions,
  onExport,
}: HomeViewProps) {
  const { t } = useI18n();
  const { wide, tall } = useLayoutBreakpoints();

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex w-full max-w-6xl flex-col gap-4 px-5 py-5 sm:px-6 min-[1000px]:px-8">
          <header className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <h1 className="text-[22px] leading-tight font-bold">
                {t.home.title}
              </h1>
              <p className="mt-1 text-sm text-muted-foreground">
                {t.home.subtitle}
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              className="shrink-0"
              onClick={onOpenOptions}
            >
              <SettingsIcon className="size-4" />
              {t.app.options}
            </Button>
          </header>

          {sidecarError && <SidecarErrorCard error={sidecarError} />}

          <div
            className={`flex min-h-0 gap-4 ${
              wide ? "flex-row items-stretch" : "flex-col"
            }`}
          >
            <div className="flex min-w-0 flex-1 flex-col gap-4">
              <Card className="shrink-0">
                <CardHeader className="pb-3">
                  <CardTitle className="text-base">
                    {t.home.sourceTargetTitle}
                  </CardTitle>
                </CardHeader>
                <CardContent className="flex flex-col gap-4">
                  <PathPicker
                    variant="card"
                    icon={FolderInputIcon}
                    label={t.app.sourceLabel}
                    placeholder={t.app.sourcePlaceholder}
                    namePlaceholder={t.home.sourceNamePlaceholder}
                    value={source}
                    onChange={onSourceChange}
                  />
                  <span aria-hidden className="h-px shrink-0 bg-border/60" />
                  <PathPicker
                    variant="card"
                    icon={FolderOutputIcon}
                    label={t.app.destinationLabel}
                    placeholder={t.app.destinationPlaceholder}
                    namePlaceholder={t.home.destinationNamePlaceholder}
                    value={destination}
                    onChange={onDestinationChange}
                  />
                </CardContent>
              </Card>
              {tall ? (
                <KeepRootExpanded
                  state={preview}
                  keepRootFolder={keepRootFolder}
                  onKeepRootChange={onKeepRootChange}
                />
              ) : (
                <KeepRootCapsule
                  state={preview}
                  keepRootFolder={keepRootFolder}
                  onKeepRootChange={onKeepRootChange}
                />
              )}
            </div>
            {wide && (
              <ConfigSummary
                className="w-[38%] min-w-[300px] max-w-md shrink-0"
                options={options}
                onEditOptions={onOpenOptions}
              />
            )}
          </div>
        </div>
      </div>

      <footer className="shrink-0 border-t bg-card">
        <div className="mx-auto flex w-full max-w-6xl items-center justify-between gap-4 px-5 py-3 sm:px-6 min-[1000px]:px-8">
          <Label className="flex cursor-pointer items-center gap-2 text-xs font-normal text-muted-foreground">
            <Checkbox
              checked={rememberPaths}
              onCheckedChange={(v) => onRememberPathsChange(v === true)}
            />
            {t.app.rememberPaths}
          </Label>
          <Button disabled={!canExport} onClick={onExport} className="shrink-0">
            <FolderOpenIcon className="size-4" />
            {t.app.export}
          </Button>
        </div>
      </footer>
    </div>
  );
}
