Fix CLI error handling and argument edge cases

- `--help` output now goes to stdout (exit code 0) instead of stderr; argument errors exit with code 2 while runtime errors exit with code 1.
- Non-UTF-8 command-line arguments no longer panic the process.
- A `--start-at` path outside the export root now fails with a clear error instead of silently exporting zero files.
- Unicode heading anchors are preserved as-is instead of being transliterated (e.g. Chinese headings used to become pinyin, producing anchors no renderer matches), and underscores in anchors are now kept, matching GitHub's slug rules.
- Degenerate wikilinks like `[[note|]]` or `[[#]]` no longer panic the export.
- Resolution of bare-name references to same-named files is now deterministic (fewest path components, then lexicographic order) instead of depending on directory traversal order.
- `filter_by_tags` now also accepts scalar and comma-separated string values for `tags` in frontmatter.
- Closing the stdout pipe (e.g. a consumer that stopped reading `--progress json` output) exits quietly with code 1 instead of panicking with code 101.
