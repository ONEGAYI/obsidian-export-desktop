//! CLI contract tests: these lock down the observable behavior of the obsidian-export
//! binary (arguments, exit codes, stdout/stderr usage) which the future desktop app
//! will rely on when driving it as a sidecar process.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// Path to the obsidian-export binary, provided by cargo for integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_obsidian-export");

struct CliOutput {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn run_cli(args: &[&str]) -> CliOutput {
    let output = Command::new(BIN)
        .args(args)
        .output()
        .expect("failed to run CLI");
    CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

fn parse_json_lines(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|err| panic!("invalid JSON line {:?}: {}", line, err))
        })
        .collect()
}

#[test]
fn version_goes_to_stdout_with_zero_exit() {
    let out = run_cli(&["--version"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(
        out.stdout.starts_with("obsidian-export "),
        "stdout should carry the version banner, got: {:?}",
        out.stdout
    );
    assert!(out.stderr.is_empty(), "stderr should be silent");

    let out = run_cli(&["-v"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.starts_with("obsidian-export "));
}

#[test]
fn help_goes_to_stdout_with_zero_exit() {
    let out = run_cli(&["--help"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.contains("Usage:"), "stdout should carry usage");
    assert!(
        out.stdout.contains("--missing-section"),
        "usage should document --missing-section"
    );
    assert!(
        out.stdout.contains("--progress"),
        "usage should document --progress"
    );
    assert!(
        out.stderr.is_empty(),
        "stderr should be silent, got: {:?}",
        out.stderr
    );
}

#[test]
fn argument_errors_go_to_stderr_with_exit_2() {
    let out = run_cli(&[]);
    assert_eq!(out.code, Some(2_i32));
    assert!(
        out.stderr.contains("Error:"),
        "stderr should explain the error"
    );
    assert!(out.stdout.is_empty());

    let out = run_cli(&["--no-such-flag", "a", "b"]);
    assert_eq!(out.code, Some(2_i32));
    assert!(out.stderr.contains("Error:"));
}

#[test]
fn invalid_enum_values_are_argument_errors() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--missing-section",
        "bogus",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(out.code, Some(2_i32));
    assert!(out.stderr.contains("embed-full"));
}

#[test]
fn successful_export_is_silent_with_zero_exit() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&["tests/testdata/input/main-samples", dest]);
    assert_eq!(out.code, Some(0_i32));
    assert!(
        out.stdout.is_empty(),
        "stdout must stay silent without --progress"
    );
    assert!(
        tmp_dir
            .path()
            .join(PathBuf::from("note-without-frontmatter.md"))
            .exists(),
        "notes should have been exported"
    );
}

#[test]
fn progress_json_emits_schema_start_events_and_end() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--progress",
        "json",
        "tests/testdata/input/mixed-health",
        dest,
    ]);
    assert_eq!(
        out.code,
        Some(1_i32),
        "one broken note should fail the export"
    );

    let events = parse_json_lines(&out.stdout);
    assert!(!events.is_empty());

    let schema = events.first().expect("schema event");
    assert_eq!(schema["type"], "schema");
    assert_eq!(schema["version"], 1_i32, "schema version must be pinned");

    let types: Vec<&str> = events
        .iter()
        .map(|event| event["type"].as_str().expect("event type"))
        .collect();
    assert_eq!(types.get(1), Some(&"start"));
    assert!(types.contains(&"file-done"), "healthy files report done");
    assert!(
        types.contains(&"file-failed"),
        "broken frontmatter reports failure"
    );
    assert!(types.contains(&"warning"), "broken link reports a warning");
    assert_eq!(*types.last().expect("at least one event"), "end");

    let end = events.last().expect("end event");
    assert_eq!(
        end["failed"].as_array().map(Vec::len),
        Some(1_usize),
        "end event lists the failed file"
    );
}

#[test]
fn progress_json_all_healthy_export_succeeds() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--progress",
        "json",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(out.code, Some(0_i32));

    let events = parse_json_lines(&out.stdout);
    let types: Vec<&str> = events
        .iter()
        .map(|event| event["type"].as_str().expect("event type"))
        .collect();
    assert_eq!(types.first(), Some(&"schema"));
    assert_eq!(types.get(1), Some(&"start"));
    assert_eq!(*types.last().expect("at least one event"), "end");
    let end = events.last().expect("end event");
    assert_eq!(end["failed"].as_array().map(Vec::len), Some(0_usize));
}

#[test]
fn fail_fast_stops_at_first_failure() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&["--fail-fast", "tests/testdata/input/mixed-health", dest]);
    assert_eq!(out.code, Some(1_i32));
    assert!(
        out.stderr.contains("Error:"),
        "stderr should carry the first error, got: {:?}",
        out.stderr
    );
}
