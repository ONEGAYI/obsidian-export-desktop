# Desktop app

A graphical desktop app is available alongside the CLI. It wraps the exact same exporter: the GUI bundles the `obsidian-export` CLI as a sidecar process and shows its progress live, so anything the CLI can export, the desktop app can too.

Features include:

* Obsidian-styled light/dark themes (with a follow-system option) in a frameless window.
* Bilingual UI (Chinese / English) with a language menu: pick either language explicitly or follow the system locale; the choice is remembered across sessions.
* Folder pickers for the vault and destination, with the last-used paths remembered.
* A full conversion options view mirroring every CLI flag: frontmatter strategy, missing-section handling, hard line breaks, recursive embeds, hidden files, git integration, the ignore-file name, skip/only tags, a start-at sub-path, mtime preservation and fail-fast. Options are remembered across sessions, and only non-default values are passed to the sidecar.
* Diagram rendering: dot (Graphviz), Mermaid, WaveDrom and TikZ code blocks can be rendered into image assets through local tools. The settings page shows the enabled renderers at a glance (with a count badge in the navigation) and manages them as pill-style checkboxes; the output format (svg/png, with per-renderer fallback) and per-tool executable paths (blank = PATH lookup) are configurable. Rendering progress ("diagram 3/12") shows in the run view; a missing tool aborts the export before anything is written.
* A pre-export sheet summarizing the effective options (with a shortcut back to the options view), and an option to export into `<destination>/<vault folder name>` so the vault's first-level entries stay contained.
* Live progress, per-file log lines, failure details with full error chains, and cancellation of a running export.
* An optional post-export link check (against the vault source or the exported tree) with a per-link report of broken links, missing sections and blocks.
* An "About & update" page: the app checks GitHub releases on launch (at most once a day, toggleable) and on demand, shows release notes, and can download and launch the new installer.

The CLI remains fully usable on its own; the desktop app is simply another way to invoke it.

For building or running the desktop app from source, see [`docs/BUILD.md`](docs/BUILD.md).
