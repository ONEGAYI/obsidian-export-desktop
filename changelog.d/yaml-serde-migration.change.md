Migrated the YAML dependency from the archived `serde_yaml 0.9.34` to
`yaml_serde 0.10` (maintained by the YAML organization) via a Cargo package
rename. The public `obsidian_export::serde_yaml` path, the `Frontmatter`
type alias and all parsing/emitting behavior are unchanged. The minimum
supported Rust version was bumped from 1.80 to 1.82 as required by
yaml_serde.
