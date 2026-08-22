import type { Dict } from "./zh";

/**
 * English UI dictionary. Typed as `Dict` (derived from the zh dictionary) so
 * a missing, extra, or structurally drifted key fails the build.
 */
export const en: Dict = {
  common: {
    close: "Close",
    browse: "Browse",
  },
  window: {
    minimize: "Minimize",
    maximize: "Maximize",
    close: "Close",
  },
  theme: {
    light: "Light",
    dark: "Dark",
    system: "Follow system",
    toggleLabel: "Theme: {current}, click to switch to {next}",
    toggleTitle: "Theme: {current} (click to switch to {next})",
  },
  language: {
    zh: "中文",
    en: "English",
    system: "Follow system",
    menuLabel: "Language: {current}",
  },
  app: {
    warningLog: "Warning",
    sidecarUnavailable: "Sidecar unavailable",
    sidecarErrorTitle: "Sidecar process unavailable",
    sidecarErrorHint: {
      pre: "Run",
      code: "just desktop-sync-sidecar",
      post: "and restart the app.",
    },
    exportTitle: "Export an Obsidian Vault",
    exportDescription:
      "Convert Obsidian-flavored Markdown to plain Markdown. Conversion is done by the bundled obsidian-export sidecar.",
    sourceLabel: "Vault source",
    sourcePlaceholder: "Choose an Obsidian vault folder or a single note",
    destinationLabel: "Destination",
    destinationPlaceholder: "Choose an output folder",
    rememberPaths: "Remember last paths",
    options: "Options",
    export: "Export",
  },
  options: {
    title: "Conversion Options",
    description:
      "Mirrors the obsidian-export CLI options; changes save instantly. Options left at their defaults are never passed to the sidecar.",
    sectionConversion: "Conversion",
    sectionFiltering: "Content Filtering",
    sectionProcess: "Files & Process",
    frontmatterLabel: "Frontmatter handling",
    missingSectionLabel: "Missing section handling",
    hardLinebreaks: {
      title: "Hard line breaks",
      description:
        'Turn soft line breaks into hard breaks, mirroring Obsidian\u2019s "Strict line breaks" setting',
    },
    noRecursiveEmbeds: {
      title: "Non-recursive embeds",
      description:
        "Don't expand embeds inside embeds; this breaks circular references between notes",
    },
    hidden: {
      title: "Include hidden files",
      description: "Export dot-prefixed hidden files (skipped by default)",
    },
    noGit: {
      title: "Disable git integration",
      description: "Don't read .gitignore rules (read by default)",
    },
    ignoreFileLabel: "Ignore-rules file name",
    ignoreFilePlaceholder: ".export-ignore (default)",
    skipTagsLabel: "Skip tags",
    skipTagsPlaceholder: "Notes with any of these tags are skipped",
    onlyTagsLabel: "Only tags",
    onlyTagsPlaceholder: "Export only notes with any of these tags",
    startAtLabel: "Start at sub-path (optional)",
    startAtPlaceholder:
      "Pick a sub-folder inside the vault; leave empty to export everything",
    startAtHint:
      "Must live under the vault root; an out-of-bounds path fails the export",
    preserveMtime: {
      title: "Preserve modification time",
      description: "Exported files keep the source note's modification time",
    },
    failFast: {
      title: "Fail fast",
      description:
        "Stop at the first failed file instead of continuing and summarizing at the end",
    },
    footer: "All options are remembered and reused on the next launch.",
    resetDefaults: "Reset to defaults",
    back: "Back",
    frontmatterChoices: {
      auto: {
        label: "Auto",
        description: "Keep frontmatter when the note has one (default)",
      },
      always: {
        label: "Always add",
        description: "Add an empty frontmatter block to notes without one",
      },
      never: {
        label: "Remove all",
        description: "Strip all frontmatter from the output",
      },
    },
    missingSectionChoices: {
      skip: {
        label: "Skip",
        description: "Embed nothing and warn (default, matches Obsidian)",
      },
      "embed-full": {
        label: "Embed full note",
        description:
          "Embed the whole note when the section is missing (legacy behavior)",
      },
      fail: {
        label: "Fail",
        description: "Fail the note and count it in the results",
      },
    },
    summary: {
      startAt: "Only exporting {name}",
      frontmatter: "Frontmatter: {label}",
      ignoreFile: "Ignore file: {name}",
      skipTags: "Skip tags ×{n}",
      onlyTags: "Only tags ×{n}",
      hidden: "Hidden files included",
      noGit: "Git disabled",
      noRecursiveEmbeds: "Non-recursive embeds",
      preserveMtime: "Mtime preserved",
      missingSection: "Missing section: {label}",
      failFast: "Fail fast",
      hardLinebreaks: "Hard line breaks",
    },
  },
  dialog: {
    title: "Confirm Export",
    activeOptions: "Active options",
    modify: "Modify",
    allDefault: "All defaults",
    keepRootTitle: "Keep root folder under destination",
    keepRootDescription:
      'When exporting a folder, writes to "destination/{name}" so the top-level files don\u2019t spill into the destination itself (folder sources only).',
    keepRootFallbackName: "source folder name",
    cancel: "Cancel",
    start: "Start Export",
  },
  run: {
    title: "Exporting…",
    progressCount: "{processed} / {total} notes",
    doneCount: "{n} done",
    skippedCount: "{n} skipped",
    failedCount: "{n} failed",
    waiting: "Waiting for sidecar events…",
    cancel: "Cancel Export",
  },
  result: {
    cancelled: "Export cancelled",
    aborted: "Export aborted",
    partial: "Export finished (with failures)",
    completed: "Export finished",
    abortedDetail:
      "The event stream ended abnormally; results below are partial.",
    summary: "{total} notes: {done} done · {skipped} skipped · {failures} failed",
    warnings: "{n} warnings",
    back: "Back",
  },
  tagInput: {
    removeTag: "Remove tag {tag}",
    placeholder: "Type and press Enter to add",
  },
};
