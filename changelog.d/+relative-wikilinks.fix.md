Resolve wikilinks with explicit relative components (`./`, `../`)

Wikilinks such as `![[../assets/diagram.svg]]` were silently dropped (with a warning) because vault lookup only matches path suffixes, which can never contain `.` or `..` components. Obsidian resolves such references against the containing note's directory, and so does the exporter now: the reference is normalized against the note's location before lookup. References that would escape the vault root remain unresolved.
