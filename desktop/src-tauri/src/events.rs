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

/// Which link syntax a check verdict is about; mirrors the CLI's
/// `link-report` kind field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkKind {
    WikiLink,
    WikiEmbed,
    MarkdownLink,
    MarkdownImage,
    /// A kind this app version doesn't know (the CLI degrades future core
    /// variants to this opaque marker instead of dropping the report).
    Unknown,
}

/// Verdict for one checked link; mirrors the CLI's `link-report` status
/// payload, with the target/section names as structured fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CheckStatus {
    Ok,
    MissingFile { target: String },
    OutOfBounds { target: String },
    MissingSection { target: String, section: String },
    MissingBlock { target: String, block: String },
    FileUnreadable { message: String },
    ExternalSkipped { url: String },
    /// A status this app version doesn't know (see [`LinkKind::Unknown`]).
    Unknown,
}

/// One event from the sidecar's `check --progress json` stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CheckEvent {
    Schema { version: u32 },
    CheckStart { files: u32 },
    LinkReport {
        source: String,
        line: u32,
        raw: String,
        kind: LinkKind,
        status: CheckStatus,
    },
    #[serde(rename_all = "camelCase")]
    CheckEnd {
        files_checked: u32,
        total_links: u32,
        broken: u32,
        skipped: u32,
    },
}

/// Outcome of parsing one stdout line of the check event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedCheckLine {
    Event(CheckEvent),
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

/// Parse a single line of the sidecar's stdout while running `check`.
///
/// Semantics mirror [`parse_line`] for the check dialect: the schema event
/// shares the export stream's version constant, unknown event types are
/// ignored for forward compatibility, and anything malformed is an error.
pub fn parse_check_line(line: &str) -> Result<ParsedCheckLine, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(ParsedCheckLine::Ignored);
    }

    let tag: TypeTag =
        serde_json::from_str(trimmed).map_err(|err| format!("invalid JSON line: {err}"))?;

    let event: CheckEvent = match tag.tag.as_str() {
        "schema" | "check-start" | "link-report" | "check-end" => {
            serde_json::from_str(trimmed)
                .map_err(|err| format!("malformed {} event: {err}", tag.tag))?
        }
        _ => return Ok(ParsedCheckLine::Ignored),
    };

    if let CheckEvent::Schema { version } = &event {
        if *version != SUPPORTED_SCHEMA_VERSION {
            return Err(format!(
                "sidecar speaks event schema v{version}, this app supports v{SUPPORTED_SCHEMA_VERSION}"
            ));
        }
    }

    Ok(ParsedCheckLine::Event(event))
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

    fn parse_check_ok(line: &str) -> CheckEvent {
        match parse_check_line(line) {
            Ok(ParsedCheckLine::Event(event)) => event,
            other => panic!("expected event, got {other:?} for line {line:?}"),
        }
    }

    #[test]
    fn parses_all_four_check_event_types() {
        assert_eq!(
            parse_check_ok(r#"{"type":"schema","version":1}"#),
            CheckEvent::Schema { version: 1 }
        );
        assert_eq!(
            parse_check_ok(r#"{"type":"check-start","files":19}"#),
            CheckEvent::CheckStart { files: 19 }
        );
        assert_eq!(
            parse_check_ok(
                // A real line shape from `check --progress json`: `raw` is the
                // reference text between the brackets, without its ![[…]]
                // syntax wrapper (same as the CLI contract fixture).
                r#"{"type":"link-report","source":"embeds.md","line":7,"raw":"non-existing note","kind":"wiki-embed","status":{"type":"missing-file","target":"non-existing note"}}"#
            ),
            CheckEvent::LinkReport {
                source: "embeds.md".into(),
                line: 7,
                raw: "non-existing note".into(),
                kind: LinkKind::WikiEmbed,
                status: CheckStatus::MissingFile {
                    target: "non-existing note".into()
                },
            }
        );
        assert_eq!(
            parse_check_ok(
                r#"{"type":"check-end","filesChecked":19,"totalLinks":35,"broken":6,"skipped":2}"#
            ),
            CheckEvent::CheckEnd {
                files_checked: 19,
                total_links: 35,
                broken: 6,
                skipped: 2,
            }
        );
    }

    #[test]
    fn parses_every_check_status_variant() {
        fn status_of(status_json: &str) -> CheckStatus {
            let line = format!(
                r#"{{"type":"link-report","source":"a.md","line":1,"raw":"r","kind":"wiki-link","status":{status_json}}}"#
            );
            match parse_check_ok(&line) {
                CheckEvent::LinkReport { status, .. } => status,
                other => panic!("expected link-report, got {other:?} for line {line:?}"),
            }
        }
        assert_eq!(status_of(r#"{"type":"ok"}"#), CheckStatus::Ok);
        assert_eq!(
            status_of(r#"{"type":"missing-file","target":"x.md"}"#),
            CheckStatus::MissingFile { target: "x.md".into() }
        );
        assert_eq!(
            status_of(r#"{"type":"out-of-bounds","target":"../x.md"}"#),
            CheckStatus::OutOfBounds {
                target: "../x.md".into()
            }
        );
        assert_eq!(
            status_of(r#"{"type":"missing-section","target":"t.md","section":"Head"}"#),
            CheckStatus::MissingSection {
                target: "t.md".into(),
                section: "Head".into()
            }
        );
        assert_eq!(
            status_of(r#"{"type":"missing-block","target":"t.md","block":"abc"}"#),
            CheckStatus::MissingBlock {
                target: "t.md".into(),
                block: "abc".into()
            }
        );
        assert_eq!(
            status_of(r#"{"type":"file-unreadable","message":"boom"}"#),
            CheckStatus::FileUnreadable { message: "boom".into() }
        );
        assert_eq!(
            status_of(r#"{"type":"external-skipped","url":"https://x"}"#),
            CheckStatus::ExternalSkipped {
                url: "https://x".into()
            }
        );
        // The CLI degrades future core variants to an opaque marker; it must
        // parse here instead of failing the whole line.
        assert_eq!(status_of(r#"{"type":"unknown"}"#), CheckStatus::Unknown);
    }

    #[test]
    fn check_stream_ignores_export_events_and_vice_versa() {
        // The two dialects share one stdout contract but disjoint event
        // sets: cross-stream lines are unknown types, not errors.
        assert_eq!(
            parse_check_line(r#"{"type":"file-done","path":"a.md"}"#).unwrap(),
            ParsedCheckLine::Ignored
        );
        assert_eq!(
            parse_line(r#"{"type":"check-start","files":2}"#).unwrap(),
            ParsedLine::Ignored
        );
        assert_eq!(
            parse_check_line("").unwrap(),
            ParsedCheckLine::Ignored
        );
    }

    #[test]
    fn check_stream_invalid_json_and_unknown_schema_are_errors() {
        assert!(parse_check_line("not json").is_err());
        assert!(parse_check_line(r#"{"type":"schema","version":2}"#).is_err());
    }
}
