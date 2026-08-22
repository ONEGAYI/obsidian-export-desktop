# Desktop app

A graphical desktop app is available alongside the CLI. It wraps the exact same exporter: the GUI bundles the `obsidian-export` CLI as a sidecar process and shows its progress live, so anything the CLI can export, the desktop app can too.

Features include:

* Obsidian-styled light/dark themes (with a follow-system option) in a frameless window.
* Folder pickers for the vault and destination, with the last-used paths remembered.
* A pre-export sheet to pick the missing-section strategy, and an option to export into `<destination>/<vault folder name>` so the vault's first-level entries stay contained.
* Live progress, per-file log lines, failure details with full error chains, and cancellation of a running export.

The CLI remains fully usable on its own; the desktop app is simply another way to invoke it.

For building or running the desktop app from source, see [`docs/BUILD.md`](docs/BUILD.md).
