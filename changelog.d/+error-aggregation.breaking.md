Failing notes no longer abort the export by default

Previously the first note that failed to export (e.g. broken YAML frontmatter) aborted the whole run. Now failures are collected per note, the export continues with the remaining notes, and a summary listing every failing note is printed at the end.

Pass `--fail-fast` to restore the old stop-on-first-failure behavior.
