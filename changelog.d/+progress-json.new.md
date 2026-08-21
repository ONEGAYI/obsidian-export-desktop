Machine-readable progress events with `--progress json`

Passing `--progress json` emits progress events on stdout as JSON Lines (one JSON object per line), intended for programs driving obsidian-export as a child process. The stream starts with a schema-version line, followed by per-file progress, warnings (with the originating file), and a terminating end event listing failed files. Without the flag, stdout stays silent as before.
