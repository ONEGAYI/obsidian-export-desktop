import type { Dict } from "@/i18n/zh";
import { baseName } from "@/lib/sidecar";

/** Mirrors ExportOptions in desktop/src-tauri/src/sidecar.rs (camelCase JSON). */
export type FrontmatterStrategy = "auto" | "always" | "never";
export type MissingSectionStrategy = "skip" | "embed-full" | "fail";
/** Which tree the post-export link check walks; GUI-only, never a CLI flag. */
export type LinkCheckTarget = "source" | "destination";

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
  /** Run the link checker automatically after a successful export. */
  linkCheckEnabled: boolean;
  linkCheckTarget: LinkCheckTarget;
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
  linkCheckEnabled: false,
  linkCheckTarget: "source",
};

// Legal values for the string-enum options; the user-facing labels live in
// the i18n dictionaries (options.frontmatterChoices / missingSectionChoices /
// linkCheckTargetChoices).
export const FRONTMATTER_VALUES: readonly FrontmatterStrategy[] = [
  "auto",
  "always",
  "never",
];

export const MISSING_SECTION_VALUES: readonly MissingSectionStrategy[] = [
  "skip",
  "embed-full",
  "fail",
];

export const LINK_CHECK_TARGET_VALUES: readonly LinkCheckTarget[] = [
  "source",
  "destination",
];

const OPTIONS_KEY = "obsidian-export-options";
const LEGACY_MISSING_SECTION_KEY = "obsidian-export-missing-section";

function oneOf<T extends string>(
  value: unknown,
  choices: readonly T[],
  fallback: T,
): T {
  return choices.includes(value as T) ? (value as T) : fallback;
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
    frontmatter: oneOf(value.frontmatter, FRONTMATTER_VALUES, "auto"),
    ignoreFile: optionalString(value.ignoreFile),
    skipTags: tagList(value.skipTags),
    onlyTags: tagList(value.onlyTags),
    hidden: bool(value.hidden),
    noGit: bool(value.noGit),
    noRecursiveEmbeds: bool(value.noRecursiveEmbeds),
    preserveMtime: bool(value.preserveMtime),
    missingSection: oneOf(value.missingSection, MISSING_SECTION_VALUES, "skip"),
    failFast: bool(value.failFast),
    hardLinebreaks: bool(value.hardLinebreaks),
    linkCheckEnabled: bool(value.linkCheckEnabled),
    linkCheckTarget: oneOf(
      value.linkCheckTarget,
      LINK_CHECK_TARGET_VALUES,
      "source",
    ),
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
    missingSection: oneOf(legacy, MISSING_SECTION_VALUES, "skip"),
  };
  saveOptions(options);
  return options;
}

/**
 * Human-readable summary of every option deviating from the defaults. Shown
 * in the pre-export dialog so a choice made in the settings view stays
 * visible at export time. Derived from the same defaults as the Rust
 * `build_args`, keeping the two in lockstep; wording comes from the active
 * i18n dictionary.
 */
export function summarizeOptions(options: ExportOptions, t: Dict): string[] {
  const items: string[] = [];
  // Whitespace-only values are filtered below the same way build_args does,
  // so the summary never lists an option the CLI won't receive.
  if (options.startAt?.trim()) {
    items.push(
      fmt(t, "startAt", {
        name: baseName(options.startAt) || options.startAt,
      }),
    );
  }
  if (options.frontmatter !== DEFAULT_OPTIONS.frontmatter) {
    items.push(
      fmt(t, "frontmatter", {
        label: t.options.frontmatterChoices[options.frontmatter].label,
      }),
    );
  }
  if (options.ignoreFile?.trim()) {
    items.push(fmt(t, "ignoreFile", { name: options.ignoreFile }));
  }
  if (options.skipTags.length > 0) {
    items.push(fmt(t, "skipTags", { n: options.skipTags.length }));
  }
  if (options.onlyTags.length > 0) {
    items.push(fmt(t, "onlyTags", { n: options.onlyTags.length }));
  }
  if (options.hidden) {
    items.push(t.options.summary.hidden);
  }
  if (options.noGit) {
    items.push(t.options.summary.noGit);
  }
  if (options.noRecursiveEmbeds) {
    items.push(t.options.summary.noRecursiveEmbeds);
  }
  if (options.preserveMtime) {
    items.push(t.options.summary.preserveMtime);
  }
  if (options.missingSection !== DEFAULT_OPTIONS.missingSection) {
    items.push(
      fmt(t, "missingSection", {
        label: t.options.missingSectionChoices[options.missingSection].label,
      }),
    );
  }
  if (options.failFast) {
    items.push(t.options.summary.failFast);
  }
  if (options.hardLinebreaks) {
    items.push(t.options.summary.hardLinebreaks);
  }
  if (options.linkCheckEnabled) {
    items.push(
      fmt(t, "linkCheck", {
        target: t.options.linkCheckTargetChoices[options.linkCheckTarget].label,
      }),
    );
  }
  return items;
}

/** `fmt(t, "startAt", {name})` → the `options.summary.startAt` template. */
function fmt(
  t: Dict,
  key: keyof Dict["options"]["summary"],
  params: Record<string, string | number>,
): string {
  return t.options.summary[key].replace(
    /\{(\w+)\}/g,
    (match, name: string) => (name in params ? String(params[name]) : match),
  );
}
