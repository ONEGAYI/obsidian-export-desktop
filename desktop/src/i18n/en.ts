import type { Dict } from "./zh";

/**
 * English UI dictionary. Typed as `Dict` (derived from the zh dictionary) so
 * a missing, extra, or structurally drifted key fails the build.
 */
export const en: Dict = {
  common: {
    close: "Close",
    browse: "Browse",
    // Accessible-name templates for stateful controls (switches etc.):
    // AX-tree walkers (CUA and the like) don't render ToggleState, so the
    // state goes into the name to stay observable after a click; screen
    // readers announce it twice — an accepted trade-off for walk-throughs.
    statefulControl: {
      nameOn: "{title} (on)",
      nameOff: "{title} (off)",
    },
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
    sectionDiagrams: "Diagram Rendering",
    sectionLinkCheck: "Link Check",
    sectionAbout: "About & Updates",
    updateCurrentVersion: "Current version",
    updateIdle: "No update check has run yet.",
    updateUnknown: "The update status could not be determined.",
    updateChecking: "Checking for updates…",
    updateCheckNowBtn: "Check now",
    updateUpToDate: "You are up to date.",
    updateNoRelease: "No releases have been published yet.",
    updateAvailable: "Version {version} is available",
    updateNotesTitle: "Release notes",
    updateNoAsset:
      "No installer matches this platform; download one from the release page.",
    updateOpenReleasePage: "Open release page",
    updateDownload: "Download installer",
    updateDownloading: "Downloading…",
    updateCancelDownload: "Cancel download",
    updateReady: "Installer ready",
    updateSavedTo: "Saved to {path}",
    updateInstall: "Install update",
    updateInstallHint:
      "Launches the install wizard and exits the app; reopen it once the installer finishes.",
    updateFailed: "The check or download did not complete.",
    updateCancelled: "The check or download was cancelled.",
    updateAutoCheckTitle: "Check for updates automatically",
    updateAutoCheckHint:
      "Check on launch (at most once a day)",
    frontmatterLabel: "Frontmatter handling",
    missingSectionLabel: "Missing section handling",
    commentsLabel: "Obsidian comment handling (%% fences)",
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
    linkCheckEnable: {
      title: "Link check after export",
      description:
        "Run the link checker automatically after a successful export and report every broken link",
    },
    linkCheckTargetLabel: "Check target",
    linkCheckTargetChoices: {
      source: {
        label: "Vault source",
        description:
          "Check the vault itself; catches dead links (wikilinks, embeds, plain links) before conversion",
      },
      destination: {
        label: "Export output",
        description:
          "Check the exported tree and verify the generated Markdown links and anchors; broken wikilinks have already collapsed to plain text there",
      },
    },
    footer: "All options are remembered and reused on the next launch.",
    resetDefaults: "Reset to defaults",
    back: "Back",
    diagramsDescription:
      "Renders special code blocks (dot, Mermaid, …) into images through local tools during export and embeds them as regular Markdown images; Excalidraw drawing files are converted whole, with references to them rewritten. Tools are looked up on PATH automatically; a missing tool aborts the export before any file is written.",
    diagramRenderersLabel: "Enabled renderers",
    diagramRendererChoices: {
      dot: {
        label: "dot",
        description: "Graphviz DOT diagrams (dot / graphviz blocks); requires dot",
      },
      mermaid: {
        label: "Mermaid",
        description: "Mermaid diagrams (mermaid blocks); requires mmdc (mermaid-cli)",
      },
      wavedrom: {
        label: "WaveDrom",
        description: "Digital timing diagrams (wavedrom blocks); requires wavedrom",
      },
      tikz: {
        label: "TikZ",
        description: "TikZ drawings (tikz blocks); requires latex and dvisvgm (a TeX distribution); CJK text inside may render poorly",
      },
      excalidraw: {
        label: "Excalidraw",
        description:
          "Converts Excalidraw drawing files (.excalidraw(.md) or .md with the excalidraw frontmatter) into images and rewrites embeds and links; requires excalidraw-export; complex drawings may lose fidelity",
      },
    },
    diagramFormatLabel: "Output format",
    diagramFormatChoices: {
      svg: {
        label: "SVG",
        description: "Vector format, scales without quality loss (default)",
      },
      png: {
        label: "PNG",
        description: "Raster format, best compatibility",
      },
    },
    diagramFormatFallbackNote:
      "Renderers without the chosen format fall back to SVG with a warning (WaveDrom and TikZ have no PNG output).",
    diagramBinsTitle: "Advanced: executable paths",
    diagramBinsHint:
      "Defaults to a PATH lookup. Only fill in when a tool is not on PATH or a specific version is needed; an invalid path fails before the export starts.",
    diagramBinsPlaceholder: "Leave blank to look up on PATH",
    diagramToolNames: {
      dot: "dot (Graphviz)",
      mmdc: "mmdc (mermaid-cli)",
      wavedrom: "wavedrom",
      latex: "latex (TeX)",
      dvisvgm: "dvisvgm (TeX)",
      "excalidraw-export": "excalidraw-export (Excalidraw conversion)",
    },
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
    commentsChoices: {
      keep: {
        label: "Keep as-is",
        description: "Leave %% comments verbatim (default)",
      },
      convert: {
        label: "Convert to HTML comments",
        description: "Rewrite to <!-- -->, visible in source but never rendered",
      },
      strip: {
        label: "Remove entirely",
        description: "Drop comments from the exported output",
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
      comments: "Comments: {label}",
      linkCheck: "Link check after export ({target})",
      diagramRenderers: "Diagram rendering ×{n} ({format})",
      diagramBins: "Custom tool paths ×{n}",
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
    diagramProgress: "Rendering diagram {index}/{total} ({language})",
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
  linkCheck: {
    runningTitle: "Checking links…",
    runningProgress: "{n} links reported",
    titleClean: "Link check: all good",
    titleBroken: "Link check: {n} broken",
    titleFailed: "Link check could not finish",
    cancelledTitle: "Link check cancelled",
    failedHint: "The event stream ended abnormally.",
    cancelledHint: "The check was cancelled; results are incomplete.",
    exitCode: "Exit code {code}",
    cancel: "Cancel Check",
    summary:
      "{files} files · {links} links · {broken} broken · {skipped} skipped (external)",
    filter: {
      broken: "Broken only",
      all: "All",
      skipped: "Skipped",
    },
    truncated: "Showing the first {shown} of {total} entries",
    emptyList: "Nothing under this filter",
    statusOk: "OK",
    statusMissingFile: "Target not found: {target}",
    statusOutOfBounds: "Escapes the checked root: {target}",
    statusMissingSection: "Section \u201c{section}\u201d not found in {target}",
    statusMissingBlock: "Block ^{block} not found in {target}",
    statusUnreadable: "File unreadable: {message}",
    statusExternal: "External link, skipped: {url}",
    statusUnknown: "Unknown status",
    kinds: {
      wikiLink: "Wikilink",
      wikiEmbed: "Embed",
      markdownLink: "Markdown link",
      markdownImage: "Markdown image",
      unknown: "Unknown kind",
    },
  },
  tagInput: {
    removeTag: "Remove tag {tag}",
    placeholder: "Type and press Enter to add",
  },
};
