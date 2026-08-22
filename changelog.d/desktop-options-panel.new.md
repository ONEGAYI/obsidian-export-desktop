Desktop: added a full conversion options view exposing every CLI flag of
the sidecar (frontmatter strategy, missing-section handling, hard line
breaks, recursive embeds, hidden files, git integration, ignore file name,
skip/only tags, start-at sub-path, preserve mtime, fail-fast). Options are
persisted across sessions and only non-default values are forwarded to the
CLI. The pre-export dialog now summarizes the effective options with a
shortcut to edit them, and picked paths are normalized to absolute form
per the sidecar contract.
