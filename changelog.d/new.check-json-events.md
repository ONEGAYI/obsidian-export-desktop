`check --progress json`: machine-readable link-check event stream

The `check` subcommand now accepts `--progress json`, emitting the same
JSON Lines dialect family as exports: a `schema` header (shared version
constant), `check-start`, one `link-report` per link with fully structured
payloads (`source`, `line`, `raw`, `kind`, and a `status` object whose
variants carry the target/section/block names), and a `check-end` summary
(`filesChecked`, `totalLinks`, `broken`, `skipped`). Consumers no longer
parse the human-readable verdict lines. The termination protocol mirrors
exports (a run that fails after the schema line emits no `check-end`; the
reason stays on stderr), exit codes stay 0/1/2, and the plain-text mode is
unchanged.
