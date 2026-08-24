New `check` subcommand and library API for vault link integrity

`obsidian-export check SOURCE` (and the new `Exporter::check()` library
method) walks the same files an export would process and verifies every
link without writing anything:

- Obsidian references (`[[note]]`, `[[note#section]]`, `[[note#^block]]`,
  embeds included) resolve exactly the way the exporter resolves them;
- standard Markdown links/images must point to a file inside the checked
  root — the root is the export boundary, so links that escape it
  (`../sibling`, absolute paths, other drives) are reported as broken even
  when the file exists on disk;
- section anchors are validated per target (Obsidian-style matching for
  wikilinks, GitHub-style slugs for markdown fragments), block ids reuse
  the exporter's block-locating rules;
- external URLs (`https://…`) are skipped and counted separately.

Output is one line per link, `{source}:{line}: {status} [{raw}]`, plus a
summary; the exit code stays within the documented 0/1/2 contract (any
broken link exits 1). The desktop app can later run this automatically
after an export (configuration point pending).
