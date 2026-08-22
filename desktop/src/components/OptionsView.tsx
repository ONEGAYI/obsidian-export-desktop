import { ArrowLeftIcon, RotateCcwIcon } from "lucide-react";

import { PathPicker } from "@/components/PathPicker";
import { TagInput } from "@/components/TagInput";
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
import {
  DEFAULT_OPTIONS,
  FRONTMATTER_OPTIONS,
  MISSING_SECTION_OPTIONS,
  type ExportOptions,
} from "@/lib/options";

interface OptionsViewProps {
  options: ExportOptions;
  onOptionsChange: (options: ExportOptions) => void;
  onBack: () => void;
}

/** Radio card group for a string-enum option (same look as the export dialog). */
function EnumChoice<T extends string>({
  value,
  choices,
  onChange,
}: {
  value: T;
  choices: { value: T; label: string; description: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <RadioGroup
      value={value}
      onValueChange={(v) => onChange(v as T)}
      className="gap-2"
    >
      {choices.map((choice) => (
        <Label
          key={choice.value}
          className="flex cursor-pointer items-start gap-2.5 rounded-md border p-2.5 font-normal transition-colors hover:bg-[var(--background-modifier-hover)] [&:has([data-state=checked])]:border-[var(--interactive-accent)]"
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
  return (
    <div className="flex items-start justify-between gap-3 rounded-md border p-2.5">
      <span className="flex flex-col gap-0.5">
        <span className="text-sm leading-none font-medium">{title}</span>
        <span className="text-muted-foreground text-xs">{description}</span>
      </span>
      <Switch checked={checked} onCheckedChange={onCheckedChange} className="mt-0.5" />
    </div>
  );
}

function FieldLabel({ children }: { children: React.ReactNode }) {
  return <span className="text-muted-foreground text-xs">{children}</span>;
}

/**
 * Full options panel mirroring the CLI flags of the sidecar. All choices are
 * persisted by the parent as they are made; only non-default values are
 * forwarded to the CLI (see build_args in src-tauri/src/sidecar.rs).
 */
export function OptionsView({
  options,
  onOptionsChange,
  onBack,
}: OptionsViewProps) {
  const patch = (partial: Partial<ExportOptions>) =>
    onOptionsChange({ ...options, ...partial });

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-1.5">
          <Button variant="ghost" size="icon" onClick={onBack} aria-label="返回">
            <ArrowLeftIcon className="size-4" />
          </Button>
          <CardTitle>转换选项</CardTitle>
        </div>
        <CardDescription>
          与 obsidian-export CLI 选项一一对应，改动即时保存；保持默认的选项不会传给边车。
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-6">
        <section className="flex flex-col gap-2.5 rounded-lg border p-3">
          <h3 className="text-sm font-semibold">转换行为</h3>
          <div className="flex flex-col gap-2.5">
            <FieldLabel>Frontmatter 处理</FieldLabel>
            <EnumChoice
              value={options.frontmatter}
              choices={FRONTMATTER_OPTIONS}
              onChange={(frontmatter) => patch({ frontmatter })}
            />
            <FieldLabel>缺失章节的处理方式</FieldLabel>
            <EnumChoice
              value={options.missingSection}
              choices={MISSING_SECTION_OPTIONS}
              onChange={(missingSection) => patch({ missingSection })}
            />
            <SwitchRow
              title="硬换行"
              description="软换行转为硬换行，贴近 Obsidian「严格换行」设置"
              checked={options.hardLinebreaks}
              onCheckedChange={(hardLinebreaks) => patch({ hardLinebreaks })}
            />
            <SwitchRow
              title="非递归嵌入"
              description="不展开嵌入中的嵌套嵌入，可打断笔记间的循环引用"
              checked={options.noRecursiveEmbeds}
              onCheckedChange={(noRecursiveEmbeds) =>
                patch({ noRecursiveEmbeds })
              }
            />
          </div>
        </section>

        <section className="flex flex-col gap-2.5 rounded-lg border p-3">
          <h3 className="text-sm font-semibold">内容过滤</h3>
          <div className="flex flex-col gap-2.5">
            <SwitchRow
              title="包含隐藏文件"
              description="导出以 . 开头的隐藏文件（默认跳过）"
              checked={options.hidden}
              onCheckedChange={(hidden) => patch({ hidden })}
            />
            <SwitchRow
              title="禁用 git 集成"
              description="不读取 .gitignore 忽略规则（默认读取）"
              checked={options.noGit}
              onCheckedChange={(noGit) => patch({ noGit })}
            />
            <div className="flex flex-col gap-1.5">
              <FieldLabel>忽略规则文件名</FieldLabel>
              <Input
                value={options.ignoreFile ?? ""}
                placeholder=".export-ignore（默认）"
                onChange={(e) =>
                  patch({
                    ignoreFile: e.target.value.trim() === "" ? null : e.target.value.trim(),
                  })
                }
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <FieldLabel>跳过标签</FieldLabel>
              <TagInput
                value={options.skipTags}
                onChange={(skipTags) => patch({ skipTags })}
                placeholder="含任一标签的笔记不导出"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <FieldLabel>仅导出标签</FieldLabel>
              <TagInput
                value={options.onlyTags}
                onChange={(onlyTags) => patch({ onlyTags })}
                placeholder="只导出含任一标签的笔记"
              />
            </div>
            <PathPicker
              label="仅导出子路径（可选）"
              placeholder="选择 vault 内的子文件夹，留空导出全部"
              value={options.startAt ?? ""}
              onChange={(v) =>
                patch({ startAt: v.trim() === "" ? null : v.trim() })
              }
              hint="需位于 vault 根目录之下，越界会在导出时报错"
            />
          </div>
        </section>

        <section className="flex flex-col gap-2.5 rounded-lg border p-3">
          <h3 className="text-sm font-semibold">文件与过程</h3>
          <div className="flex flex-col gap-2.5">
            <SwitchRow
              title="保留修改时间"
              description="导出文件保持与源笔记相同的修改时间"
              checked={options.preserveMtime}
              onCheckedChange={(preserveMtime) => patch({ preserveMtime })}
            />
            <SwitchRow
              title="快速失败"
              description="遇到第一个失败文件立即停止，而非继续并在末尾汇总"
              checked={options.failFast}
              onCheckedChange={(failFast) => patch({ failFast })}
            />
          </div>
        </section>

        <div className="flex items-center justify-between border-t pt-3">
          <span className="text-[var(--text-faint)] text-xs">
            全部选项均已记住，下次启动自动沿用。
          </span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onOptionsChange(DEFAULT_OPTIONS)}
          >
            <RotateCcwIcon className="size-3.5" />
            恢复默认
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
