//! Parsing of the sidecar's JSON Lines event stream (schema v1).
//!
//! The contract lives in `docs/sidecar-events.md` at the repository root; this
//! module must stay in sync with `JSON_EVENT_SCHEMA_VERSION` in the CLI.

use serde::{Deserialize, Serialize};

/// The schema version this desktop app understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// One event from the sidecar's `--progress json` stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SidecarEvent {
    Schema { version: u32 },
    Start { total: u32 },
    FileDone { path: String },
    FileSkipped { path: String },
    FileFailed { path: String, message: String },
    Warning {
        path: Option<String>,
        message: String,
    },
    End { failed: Vec<String> },
}

/// Outcome of parsing one stdout line.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    /// A known event, parsed into its structured form.
    Event(SidecarEvent),
    /// Blank line, or an event type unknown to schema v1 (a newer sidecar may
    /// emit additional event kinds; skipping them keeps older app builds usable).
    Ignored,
}

/// Intermediate probe to inspect the `type` tag before full deserialization.
#[derive(Deserialize)]
struct TypeTag {
    #[serde(rename = "type")]
    tag: String,
}

/// Parse a single line of the sidecar's stdout.
///
/// Returns an error for lines that are neither valid JSON nor blank, or that
/// carry an unsupported schema version — both indicate a broken sidecar pairing
/// and should surface to the user instead of being silently swallowed.
pub fn parse_line(line: &str) -> Result<ParsedLine, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(ParsedLine::Ignored);
    }

    let tag: TypeTag =
        serde_json::from_str(trimmed).map_err(|err| format!("invalid JSON line: {err}"))?;

    let event: SidecarEvent = match tag.tag.as_str() {
        "schema" | "start" | "file-done" | "file-skipped" | "file-failed" | "warning" | "end" => {
            serde_json::from_str(trimmed)
                .map_err(|err| format!("malformed {} event: {err}", tag.tag))?
        }
        _ => return Ok(ParsedLine::Ignored),
    };

    if let SidecarEvent::Schema { version } = &event {
        if *version != SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "sidecar speaks event schema v{version}, this app supports v{SUPPORTED_SCHEMA_VERSION}"
            ));
        }
    }

    Ok(ParsedLine::Event(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(line: &str) -> SidecarEvent {
        match parse_line(line) {
            Ok(ParsedLine::Event(event)) => event,
            other => panic!("expected event, got {other:?} for line {line:?}"),
        }
    }

    #[test]
    fn parses_all_seven_event_types() {
        assert_eq!(
            parse_ok(r#"{"type":"schema","version":1}"#),
            SidecarEvent::Schema { version: 1 }
        );
        assert_eq!(
            parse_ok(r#"{"type":"start","total":3}"#),
            SidecarEvent::Start { total: 3 }
        );
        assert_eq!(
            parse_ok(r#"{"type":"file-done","path":"a.md"}"#),
            SidecarEvent::FileDone { path: "a.md".into() }
        );
        assert_eq!(
            parse_ok(r#"{"type":"file-skipped","path":"b.md"}"#),
            SidecarEvent::FileSkipped { path: "b.md".into() }
        );
        assert_eq!(
            parse_ok(r#"{"type":"file-failed","path":"c.md","message":"Failed to export 'c.md': No such file or directory: x"}"#),
            SidecarEvent::FileFailed {
                path: "c.md".into(),
                message: "Failed to export 'c.md': No such file or directory: x".into(),
            }
        );
        assert_eq!(
            parse_ok(r#"{"type":"warning","path":"d.md","message":"Unable to find referenced note"}"#),
            SidecarEvent::Warning {
                path: Some("d.md".into()),
                message: "Unable to find referenced note".into(),
            }
        );
        assert_eq!(
            parse_ok(r#"{"type":"end","failed":["c.md"]}"#),
            SidecarEvent::End {
                failed: vec!["c.md".into()]
            }
        );
    }

    #[test]
    fn warning_path_may_be_null() {
        assert_eq!(
            parse_ok(r#"{"type":"warning","path":null,"message":"m"}"#),
            SidecarEvent::Warning {
                path: None,
                message: "m".into(),
            }
        );
    }

    /// A real line captured from mixed-health with Windows path separators and
    /// a full three-level error chain.
    #[test]
    fn parses_real_world_file_failed_line() {
        let line = r#"{"message":"Failed to export 'tests\\bad.md': Failed to decode YAML frontmatter in 'tests\\bad.md': did not find expected ',' or ']'^","path":"tests\\bad.md","type":"file-failed"}"#;
        assert_eq!(
            parse_ok(line),
            SidecarEvent::FileFailed {
                path: "tests\\bad.md".into(),
                message:
                    "Failed to export 'tests\\bad.md': Failed to decode YAML frontmatter in 'tests\\bad.md': did not find expected ',' or ']'^"
                        .into(),
            }
        );
    }

    #[test]
    fn blank_and_unknown_lines_are_ignored() {
        assert_eq!(parse_line("").unwrap(), ParsedLine::Ignored);
        assert_eq!(parse_line("   ").unwrap(), ParsedLine::Ignored);
        assert_eq!(
            parse_line(r#"{"type":"future-event","x":1}"#).unwrap(),
            ParsedLine::Ignored
        );
    }

    #[test]
    fn invalid_json_and_unknown_schema_are_errors() {
        assert!(parse_line("not json").is_err());
        // Valid JSON but not an event object shape.
        assert!(parse_line(r#"[1,2]"#).is_err());
        assert!(parse_line(r#"{"type":"schema","version":2}"#).is_err());
    }
}
