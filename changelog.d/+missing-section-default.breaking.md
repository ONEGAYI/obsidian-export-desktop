Missing sections in embeds no longer silently embed the full note

When an embed pointed at a section (heading) that doesn't exist in the target note — including block references like `![[note#^block-id]]` — the entire note used to be embedded silently, containing more content than the reference asked for.

By default such an embed now collapses to nothing and a warning is emitted, matching Obsidian's own "not found" rendering. The previous behavior remains available as `--missing-section embed-full`; `--missing-section fail` turns it into an error instead.
