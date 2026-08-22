import { baseName } from "@/lib/sidecar";

/** Mirrors ExportOptions in desktop/src-tauri/src/sidecar.rs (camelCase JSON). */
export type FrontmatterStrategy = "auto" | "always" | "never";
export type MissingSectionStrategy = "skip" | "embed-full" | "fail";

export interface ExportOptions {
  /** Absolute sub-path of the vault; `null` exports everything. */
  startAt: string | null;
  frontmatter: FrontmatterStrategy;
  /** Ignore-pattern file name; `null` keeps the CLI default. */
  ignoreFile: string | null;
  skipTags: string[];
  onlyTags: string[];
  hidden: boolean;
  noGit: boolean;
  noRecursiveEmbeds: boolean;
  preserveMtime: boolean;
  missingSection: MissingSectionStrategy;
  failFast: boolean;
  hardLinebreaks: boolean;
}

export const DEFAULT_OPTIONS: ExportOptions = {
  startAt: null,
  frontmatter: "auto",
  ignoreFile: null,
  skipTags: [],
  onlyTags: [],
  hidden: false,
  noGit: false,
  noRecursiveEmbeds: false,
  preserveMtime: false,
  missingSection: "skip",
  failFast: false,
  hardLinebreaks: false,
};

export const FRONTMATTER_OPTIONS: {
  value: FrontmatterStrategy;
  label: string;
  description: string;
}[] = [
  {
    value: "auto",
    label: "自动",
    description: "笔记自带 frontmatter 时原样保留（默认）",
  },
  {
    value: "always",
    label: "始终添加",
    description: "没有 frontmatter 的笔记也补一个空的 frontmatter 块",
  },
  {
    value: "never",
    label: "全部移除",
    description: "导出结果不包含任何 frontmatter",
  },
];

export const MISSING_SECTION_OPTIONS: {
  value: MissingSectionStrategy;
  label: string;
  description: string;
}[] = [
  {
    value: "skip",
    label: "跳过",
    description: "嵌入置空并发警告（默认，贴近 Obsidian 行为）",
  },
  {
    value: "embed-full",
    label: "嵌入整篇",
    description: "找不到章节时嵌入整篇笔记（旧行为）",
  },
  {
    value: "fail",
    label: "报错",
    description: "该笔记导出失败并计入结果",
  },
];

const OPTIONS_KEY = "obsidian-export-options";
const LEGACY_MISSING_SECTION_KEY = "obsidian-export-missing-section";

function oneOf<T extends string>(
  value: unknown,
  choices: readonly { value: T }[],
  fallback: T,
): T {
  return choices.some((c) => c.value === value) ? (value as T) : fallback;
}

function optionalString(value: unknown): string | null {
  // Whitespace-only counts as unset, mirroring the Rust build_args filter
  // so the summary never shows an option the CLI won't receive.
  return typeof value === "string" && value.trim() !== "" ? value : null;
}

function tagList(value: unknown): string[] {
  // Whitespace-only entries are dropped (same rule as the Rust side);
  // deduplicated so a hand-edited payload can't produce duplicate chips
  // (React key collisions, × removing all same-named tags at once).
  if (!Array.isArray(value)) {
    return [];
  }
  return [
    ...new Set(
      value.filter(
        (tag): tag is string => typeof tag === "string" && tag.trim() !== "",
      ),
    ),
  ];
}

function bool(value: unknown): boolean {
  return value === true;
}

/** Field-by-field validation so a corrupted payload degrades to defaults. */
function sanitizeOptions(raw: unknown): ExportOptions {
  const value = (raw ?? {}) as Record<string, unknown>;
  return {
    startAt: optionalString(value.startAt),
    frontmatter: oneOf(value.frontmatter, FRONTMATTER_OPTIONS, "auto"),
    ignoreFile: optionalString(value.ignoreFile),
    skipTags: tagList(value.skipTags),
    onlyTags: tagList(value.onlyTags),
    hidden: bool(value.hidden),
    noGit: bool(value.noGit),
    noRecursiveEmbeds: bool(value.noRecursiveEmbeds),
    preserveMtime: bool(value.preserveMtime),
    missingSection: oneOf(value.missingSection, MISSING_SECTION_OPTIONS, "skip"),
    failFast: bool(value.failFast),
    hardLinebreaks: bool(value.hardLinebreaks),
  };
}

export function saveOptions(options: ExportOptions): void {
  localStorage.setItem(OPTIONS_KEY, JSON.stringify(options));
}

export function loadOptions(): ExportOptions {
  const stored = localStorage.getItem(OPTIONS_KEY);
  if (stored !== null) {
    try {
      return sanitizeOptions(JSON.parse(stored));
    } catch {
      // Corrupted payload: fall through to defaults (+ legacy migration).
    }
  }
  return migrateLegacyOptions();
}

/**
 * One-time migration of the old per-option key
 * (`obsidian-export-missing-section`) into the consolidated options object.
 */
function migrateLegacyOptions(): ExportOptions {
  const legacy = localStorage.getItem(LEGACY_MISSING_SECTION_KEY);
  if (legacy === null) {
    return DEFAULT_OPTIONS;
  }
  localStorage.removeItem(LEGACY_MISSING_SECTION_KEY);
  const options: ExportOptions = {
    ...DEFAULT_OPTIONS,
    missingSection: oneOf(legacy, MISSING_SECTION_OPTIONS, "skip"),
  };
  saveOptions(options);
  return options;
}

function frontmatterLabel(value: FrontmatterStrategy): string {
  return FRONTMATTER_OPTIONS.find((o) => o.value === value)?.label ?? "";
}

function missingSectionLabel(value: MissingSectionStrategy): string {
  return MISSING_SECTION_OPTIONS.find((o) => o.value === value)?.label ?? "";
}

/**
 * Human-readable summary of every option deviating from the defaults. Shown
 * in the pre-export dialog so a choice made in the settings view stays
 * visible at export time. Derived from the same defaults as the Rust
 * `build_args`, keeping the two in lockstep.
 */
export function summarizeOptions(options: ExportOptions): string[] {
  const items: string[] = [];
  if (options.startAt) {
    items.push(`仅导出 ${baseName(options.startAt) || options.startAt}`);
  }
  if (options.frontmatter !== DEFAULT_OPTIONS.frontmatter) {
    items.push(`Frontmatter：${frontmatterLabel(options.frontmatter)}`);
  }
  if (options.ignoreFile) {
    items.push(`忽略文件：${options.ignoreFile}`);
  }
  if (options.skipTags.length > 0) {
    items.push(`跳过标签 ×${options.skipTags.length}`);
  }
  if (options.onlyTags.length > 0) {
    items.push(`仅导出标签 ×${options.onlyTags.length}`);
  }
  if (options.hidden) {
    items.push("含隐藏文件");
  }
  if (options.noGit) {
    items.push("禁用 git");
  }
  if (options.noRecursiveEmbeds) {
    items.push("非递归嵌入");
  }
  if (options.preserveMtime) {
    items.push("保留修改时间");
  }
  if (options.missingSection !== DEFAULT_OPTIONS.missingSection) {
    items.push(`缺失章节：${missingSectionLabel(options.missingSection)}`);
  }
  if (options.failFast) {
    items.push("快速失败");
  }
  if (options.hardLinebreaks) {
    items.push("硬换行");
  }
  return items;
}
