Keep non-ASCII characters verbatim in link destinations

Link destinations percent-encoded every non-ASCII character (e.g. `图.svg` became `%E5%9B%BE.svg`), making exported notes hard to read and diff. Only characters that would break a Markdown inline link destination or URL semantics (controls, spaces, parentheses, `%`, `?`, `#`) are escaped now; filenames in Chinese or other non-ASCII scripts stay readable, matching what Obsidian itself writes.
