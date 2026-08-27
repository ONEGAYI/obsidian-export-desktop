//! CLI contract tests: these lock down the observable behavior of the obsidian-export
//! binary (arguments, exit codes, stdout/stderr usage) which the future desktop app
//! will rely on when driving it as a sidecar process.

// serde_json's `Value` indexing is panic-free (out-of-shape access yields Null)
// and integer literals compared against `Value` pick their type from the
// comparison; both are intended here.
#![allow(
    clippy::indexing_slicing,
    clippy::default_numeric_fallback,
    clippy::uninlined_format_args
)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
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

/// Index one parsed event with a clear panic message (events are asserted
/// to exist; a missing one should fail loudly, not with an opaque slice panic).
fn event_at(events: &[Value], index: usize) -> &Value {
    events
        .get(index)
        .unwrap_or_else(|| panic!("event {} missing: {:?}", index, events))
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

    let out_short = run_cli(&["-v"]);
    assert_eq!(out_short.code, Some(0_i32));
    assert!(out_short.stdout.starts_with("obsidian-export "));
}

#[test]
fn version_flag_in_options_position_is_handled_by_the_parser() {
    // --version after other options is a regular flag once the required free arguments
    // are present, and still prints the version instead of exporting.
    let out = run_cli(&["--fail-fast", "--version", "a", "b"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.starts_with("obsidian-export "));
}

#[test]
fn version_flag_as_option_value_is_not_special() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    // "-v" consumed as the value of --ignore-file must not trigger version output
    // (it is only special in first position).
    let out = run_cli(&[
        "--ignore-file",
        "-v",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(
        out.code,
        Some(0_i32),
        "export should proceed, stderr: {:?}",
        out.stderr
    );
    assert!(
        !out.stdout.starts_with("obsidian-export "),
        "no version banner expected, got: {:?}",
        out.stdout
    );
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

    let out_unknown = run_cli(&["--no-such-flag", "a", "b"]);
    assert_eq!(out_unknown.code, Some(2_i32));
    assert!(out_unknown.stderr.contains("Error:"));
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

    let out_progress = run_cli(&[
        "--progress",
        "bogus",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(out_progress.code, Some(2_i32));
    assert!(out_progress.stderr.contains("none"));

    let out_frontmatter = run_cli(&[
        "--frontmatter",
        "bogus",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(out_frontmatter.code, Some(2_i32));
    assert!(out_frontmatter.stderr.contains("auto"));
}

#[test]
fn successful_export_is_silent_with_zero_exit() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    // chinese-anchor contains no dead links or missing embeds; main-samples does
    // (embeds.md), whose warnings legitimately go to stderr in default mode.
    let out = run_cli(&["tests/testdata/input/chinese-anchor", dest]);
    assert_eq!(out.code, Some(0_i32));
    assert!(
        out.stdout.is_empty(),
        "stdout must stay silent without --progress"
    );
    assert!(
        out.stderr.is_empty(),
        "stderr must stay silent on a vault without warnings, got: {:?}",
        out.stderr
    );
    assert!(
        tmp_dir.path().join(PathBuf::from("note.md")).exists(),
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

    // The message is the GUI's only structured source for error details: it must
    // carry the full chain (outer context: inner context: root cause), not just
    // the outermost message.
    let failure = events
        .iter()
        .find(|event| event["type"] == "file-failed")
        .expect("file-failed event");
    let message = failure["message"].as_str().expect("message string");
    assert!(
        message.contains("Failed to export"),
        "outer context present, got: {:?}",
        message
    );
    assert!(
        message.contains("Failed to decode YAML frontmatter"),
        "inner context present, got: {:?}",
        message
    );
    assert!(
        message.matches(':').count() >= 2,
        "chain joined with colons down to the root cause, got: {:?}",
        message
    );

    // Warnings carry the originating file so consumers can locate them without
    // parsing the human-readable message.
    let warning = events
        .iter()
        .find(|event| event["type"] == "warning")
        .expect("warning event");
    assert!(
        warning["path"]
            .as_str()
            .expect("warning carries a path")
            .contains("broken-link.md"),
        "warning should point at the file that emitted it, got: {}",
        warning
    );

    // json mode routes warnings through the event stream; stderr must only carry the
    // final error report.
    assert!(
        !out.stderr.contains("Warning:"),
        "json mode must not leak warnings to stderr, got: {:?}",
        out.stderr
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

#[test]
fn fail_fast_json_stream_still_ends_with_end_event() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--progress",
        "json",
        "--fail-fast",
        "tests/testdata/input/mixed-health",
        dest,
    ]);
    assert_eq!(out.code, Some(1_i32));

    // A fail-fast abort must still terminate the event stream: consumers treat a
    // missing end event as a hard crash of the sidecar process itself.
    let events = parse_json_lines(&out.stdout);
    let types = event_types(&events);
    assert_eq!(types.first(), Some(&"schema"));
    assert_eq!(
        *types.last().expect("at least schema and start"),
        "end",
        "fail-fast abort must emit end, got: {types:?}"
    );
    let end = events.last().expect("end event");
    assert!(
        !end["failed"]
            .as_array()
            .expect("end carries a failed array")
            .is_empty(),
        "end after a fail-fast abort lists the failed file, got: {}",
        end
    );
}

#[test]
fn single_file_failure_json_stream_emits_end_event() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().join("out.md");
    let dest = dest.to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--progress",
        "json",
        "tests/testdata/input/mixed-health/bad-frontmatter.md",
        dest,
    ]);
    assert_eq!(out.code, Some(1_i32));

    // The single-file code path (source is a file, not a directory) must follow the
    // same stream contract as directory exports.
    let events = parse_json_lines(&out.stdout);
    let types = event_types(&events);
    assert_eq!(types.get(1), Some(&"start"));
    assert!(
        types.contains(&"file-failed"),
        "broken note reports failure, got: {:?}",
        types
    );
    assert_eq!(
        *types.last().expect("at least schema and start"),
        "end",
        "single-file failure must emit end, got: {types:?}"
    );
    let end = events.last().expect("end event");
    assert_eq!(end["failed"].as_array().map(Vec::len), Some(1_usize));
}

fn event_types(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["type"].as_str().expect("event type"))
        .collect()
}

#[test]
fn progress_json_reports_skipped_files() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--progress",
        "json",
        "--skip-tags",
        "private",
        "tests/testdata/input/filter-by-tags",
        dest,
    ]);
    assert_eq!(
        out.code,
        Some(0_i32),
        "skipping notes is not a failure, stderr: {:?}",
        out.stderr
    );

    // Notes dropped by a postprocessor surface as file-skipped events.
    let events = parse_json_lines(&out.stdout);
    let types = event_types(&events);
    assert!(
        types.contains(&"file-skipped"),
        "tag-filtered notes report skipped, got: {:?}",
        types
    );
    let end = events.last().expect("end event");
    assert_eq!(
        end["failed"].as_array().map(Vec::len),
        Some(0_usize),
        "skips are not failures, got: {end}"
    );
}

#[test]
fn start_at_restricts_export_and_outside_root_fails() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&[
        "--start-at",
        "tests/testdata/input/start-at/subdir",
        "tests/testdata/input/start-at",
        dest,
    ]);
    assert_eq!(
        out.code,
        Some(0_i32),
        "subdir export succeeds, stderr: {:?}",
        out.stderr
    );
    assert!(
        tmp_dir.path().join("Note B.md").exists(),
        "notes under --start-at are exported"
    );
    assert!(
        !tmp_dir.path().join("Note A.md").exists(),
        "notes outside --start-at are not exported"
    );

    // A start_at outside the root is rejected instead of silently exporting nothing.
    let out_outside = run_cli(&[
        "--start-at",
        "tests/testdata",
        "tests/testdata/input/main-samples",
        dest,
    ]);
    assert_eq!(out_outside.code, Some(1_i32));
    assert!(
        out_outside.stderr.contains("start-at"),
        "error mentions start-at, got: {:?}",
        out_outside.stderr
    );
}

#[test]
fn aggregation_summary_goes_to_stderr() {
    let tmp_dir = TempDir::new().expect("failed to make tempdir");
    let dest = tmp_dir.path().to_str().expect("non-unicode tmpdir");
    let out = run_cli(&["tests/testdata/input/mixed-health", dest]);
    assert_eq!(out.code, Some(1_i32));
    assert!(
        out.stderr.contains("1 failing file(s)"),
        "summary counts failures, got: {:?}",
        out.stderr
    );
    assert!(
        out.stderr.contains("bad-frontmatter.md"),
        "summary lists the failing file, got: {:?}",
        out.stderr
    );
    assert!(
        out.stderr.contains("Hint:"),
        "summary carries the hint, got: {:?}",
        out.stderr
    );
}

#[test]
#[cfg(unix)]
fn non_utf8_arguments_exit_cleanly_instead_of_panicking() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let bad_arg = OsStr::from_bytes(b"\xff\xfeinvalid").to_os_string();
    let output = Command::new(BIN)
        .arg(&bad_arg)
        .arg("some-dest")
        .output()
        .expect("failed to run CLI");
    // Arguments that aren't valid UTF-8 undergo lossy conversion and report a
    // (nonexistent) path error, rather than panicking with exit code 101.
    assert_eq!(output.status.code(), Some(1_i32));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
}

#[test]
fn check_reports_per_link_verdicts_and_exit_code() {
    // main-samples contains deliberately broken links (a missing note, a
    // missing block id, a missing markdown target): the per-link report
    // must name each one, and any broken link exits 1.
    let out = run_cli(&["check", "tests/testdata/input/main-samples"]);
    assert_eq!(out.code, Some(1_i32));
    assert!(
        out.stdout
            .contains("embeds.md:7: broken: file not found 'non-existing note'"),
        "missing-note verdict reported per link, got: {:?}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("broken: block 'abc' not found in 'foo.md'"),
        "missing block verdict reported per link, got: {:?}",
        out.stdout
    );
    assert!(
        out.stdout.contains("35 link(s) found, 6 broken"),
        "summary counts, got: {:?}",
        out.stdout
    );
}

#[test]
fn check_exits_zero_when_nothing_is_broken() {
    let out = run_cli(&["check", "tests/testdata/input/chinese-anchor"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(
        out.stdout.contains("0 broken"),
        "healthy vault reports zero broken links, got: {:?}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("note.md:5: ok [target#总纲：三份形态，两个断口]"),
        "ok verdicts are listed per link too, got: {:?}",
        out.stdout
    );
}

#[test]
fn check_without_source_is_a_usage_error() {
    let out = run_cli(&["check"]);
    assert_eq!(out.code, Some(2_i32));
    assert!(
        out.stderr.contains("Error:"),
        "usage error goes to stderr with the documented exit code, got: {:?}",
        out.stderr
    );
}

#[test]
fn check_with_unknown_flag_is_a_usage_error() {
    let out = run_cli(&["check", "--bogus-flag", "."]);
    assert_eq!(out.code, Some(2_i32));
    assert!(out.stderr.contains("Error:"));
}

#[test]
fn check_accepts_dot_relative_source_spellings() {
    // `./`-prefixed and redundant-component roots must behave like their
    // canonical spelling: no in-bounds link may be flagged as an escape.
    let out = run_cli(&["check", "./tests/testdata/input/chinese-anchor"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.contains("0 broken"), "got: {:?}", out.stdout);
}

#[test]
fn check_version_flag_works_inside_subcommand() {
    let out = run_cli(&["check", "--version"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.contains("obsidian-export"));
}

#[test]
fn check_keyword_shadowing_a_directory_prints_a_warning() {
    use std::process::Command;
    use tempfile::TempDir;

    // A folder named "check" in the working directory shadows the old
    // export spelling; the CLI must say so instead of failing cryptically.
    let dir = TempDir::new().expect("tempdir");
    std::fs::create_dir(dir.path().join("check")).expect("mkdir check");
    let output = Command::new(BIN)
        .current_dir(dir.path())
        .args(["check", "some-dest"])
        .output()
        .expect("run CLI");
    assert_eq!(output.status.code(), Some(1_i32));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Warning:"),
        "shadowing warning on stderr, got: {:?}",
        stderr
    );
    assert!(stderr.contains("./check"), "hint mentions ./check");
}

#[test]
fn check_progress_json_emits_event_stream() {
    // The desktop app drives check through the same JSON Lines contract as
    // exports: a schema header first, one event per link with structured
    // payloads, and a summary end event last. The verdict counts must match
    // the human-readable format (see check_reports_per_link_verdicts_...).
    let out = run_cli(&[
        "check",
        "--progress",
        "json",
        "tests/testdata/input/main-samples",
    ]);
    assert_eq!(out.code, Some(1_i32), "broken links keep the exit contract");

    let events = parse_json_lines(&out.stdout);
    assert_eq!(
        event_at(&events, 0)["type"],
        "schema",
        "schema line comes first"
    );
    assert_eq!(event_at(&events, 0)["version"], 1);
    assert_eq!(event_at(&events, 1)["type"], "check-start");
    assert_eq!(
        event_at(&events, 1)["files"],
        19,
        "file count, got: {:?}",
        event_at(&events, 1)
    );

    let end = events.last().expect("non-empty output");
    assert_eq!(end["type"], "check-end", "check-end comes last");
    assert_eq!(end["filesChecked"], 19);
    assert_eq!(end["totalLinks"], 35);
    assert_eq!(end["broken"], 6);

    let reports: Vec<&Value> = events
        .iter()
        .filter(|event| event["type"] == "link-report")
        .collect();
    assert_eq!(reports.len(), 35, "one report event per link");

    // A broken embed carries its verdict as structured data instead of the
    // formatted text line, so consumers never parse English prose. `raw` is
    // the reference text without its surrounding syntax.
    assert!(
        reports
            .iter()
            .any(|report| report["status"]["type"] == "missing-file"
                && report["source"] == "embeds.md"
                && report["line"] == 7
                && report["kind"] == "wiki-embed"
                && report["raw"] == "non-existing note"
                && report["status"]["target"] == "non-existing note"),
        "missing-file verdict with structured payload, got: {:?}",
        reports
    );

    // Every report carries the full field set, whatever the verdict.
    for report in &reports {
        assert!(report["source"].is_string(), "source, got: {}", report);
        assert!(report["line"].is_u64(), "line, got: {}", report);
        assert!(report["raw"].is_string(), "raw, got: {}", report);
        assert!(report["kind"].is_string(), "kind, got: {}", report);
        assert!(
            report["status"]["type"].is_string(),
            "status type, got: {}",
            report
        );
    }
}

#[test]
fn check_progress_json_healthy_vault_exits_zero() {
    let out = run_cli(&[
        "check",
        "--progress",
        "json",
        "tests/testdata/input/chinese-anchor",
    ]);
    assert_eq!(out.code, Some(0_i32));
    let events = parse_json_lines(&out.stdout);
    assert_eq!(event_at(&events, 0)["type"], "schema");
    let end = events.last().expect("non-empty output");
    assert_eq!(end["type"], "check-end");
    assert_eq!(end["broken"], 0);

    // Ok verdicts are reported too, with unicode anchors intact.
    assert!(
        events.iter().any(|event| event["type"] == "link-report"
            && event["status"]["type"] == "ok"
            && event["source"] == "note.md"
            && event["raw"] == "target#总纲：三份形态，两个断口"),
        "ok verdict with the unicode anchor, got: {:?}",
        events
    );
}

#[test]
fn check_progress_json_failure_reports_on_stderr_without_end() {
    // Same termination protocol as exports: a run that fails after the
    // schema line emits no check-end, and the reason stays on stderr.
    let out = run_cli(&["check", "--progress", "json", "no-such-vault"]);
    assert_eq!(out.code, Some(1_i32));
    assert!(
        out.stderr.contains("Error:"),
        "run error stays human-readable on stderr, got: {:?}",
        out.stderr
    );
    let events = parse_json_lines(&out.stdout);
    assert_eq!(
        events.len(),
        1,
        "only the schema line, no check-end on failure, got: {:?}",
        events
    );
    assert_eq!(event_at(&events, 0)["type"], "schema");
}

// ---- update 子命令（本地 HTTP mock 服务覆盖检测/下载闭环） -------------------
//
// 集成测试的 BIN 是 dev profile 构建（debug_assertions 开启），因此可以通过
// OBSIDIAN_EXPORT_UPDATE_API_BASE 环境变量把 GitHub API 指到本地 mock；
// release 二进制不含该读取路径（见 `src/update.rs` 的 `releases_latest_url`）。

/// 一条 mock 路由：请求路径含 `path` 片段即应答 (`status`, `content_type`, `body`)。
struct MockRoute {
    path: &'static str,
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

/// 起一个单线程 HTTP mock 服务，循环 accept 直到 `max_requests` 个请求
/// 处理完毕后自动退出（检测与下载走不同连接，各算一个请求）。
fn spawn_update_mock(
    build_routes: impl FnOnce(&str) -> Vec<MockRoute>,
    max_requests: usize,
) -> (SocketAddr, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("mock server addr");
    let routes = build_routes(&format!("http://{addr}"));
    let handle = std::thread::spawn(move || {
        for _ in 0..max_requests {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            let request_line = String::from_utf8_lossy(&request);
            let route = routes
                .iter()
                .find(|r| request_line.contains(r.path))
                .unwrap_or_else(|| panic!("no mock route matches request: {}", request_line));
            let reason = match route.status {
                200 => "OK",
                403 => "Forbidden",
                404 => "Not Found",
                _ => "Status",
            };
            let header = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                route.status,
                reason,
                route.content_type,
                route.body.len()
            );
            stream.write_all(header.as_bytes()).expect("write header");
            stream.write_all(&route.body).expect("write body");
        }
    });
    (addr, handle)
}

fn run_update_cli(args: &[&str], api_base: &str) -> CliOutput {
    let output = Command::new(BIN)
        .args(args)
        .env("OBSIDIAN_EXPORT_UPDATE_API_BASE", api_base)
        .output()
        .expect("failed to run CLI");
    CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

/// 含两意图资产与 sha256 副产物的 release JSON（tag 为远大于当前版本）。
/// `base` 是 mock 服务地址：浏览器下载 URL 必须指回本地，否则下载测试
/// 会去解析不存在的主机。
fn release_json(tag: &str, base: &str) -> String {
    format!(
        r#"{{"tag_name":"{tag}","html_url":"https://github.com/ONEGAYI/obsidian-export-desktop/releases/{tag}","body":"release notes","assets":[
            {{"name":"obsidian-export-x86_64-pc-windows-msvc.zip","browser_download_url":"{base}/cli.zip","size":4}},
            {{"name":"obsidian-export-x86_64-pc-windows-msvc.zip.sha256","browser_download_url":"{base}/cli.zip.sha256","size":2}},
            {{"name":"Obsidian.Export_99.0.0_x64-setup.exe","browser_download_url":"{base}/setup.exe","size":6}},
            {{"name":"Obsidian.Export_99.0.0_x64_en-US.msi","browser_download_url":"{base}/app.msi","size":8}}
        ]}}"#
    )
}

#[test]
fn update_help_exits_zero() {
    let out = run_cli(&["update", "--help"]);
    assert_eq!(out.code, Some(0_i32));
    assert!(
        out.stdout
            .contains("Usage: obsidian-export update [OPTIONS]"),
        "stdout: {:?}",
        out.stdout
    );
    assert!(out.stdout.contains("--asset"), "asset 选项应出现在帮助中");
}

#[test]
fn update_bad_option_exits_two() {
    let out = run_cli(&["update", "--no-such-flag"]);
    assert_eq!(out.code, Some(2_i32));
    assert!(out.stderr.contains("Error"), "stderr: {:?}", out.stderr);
}

#[test]
fn update_bad_asset_value_exits_two() {
    let out = run_cli(&["update", "--asset", "phone"]);
    assert_eq!(out.code, Some(2_i32));
    assert!(out.stderr.contains("cli, desktop"));
}

#[test]
fn main_usage_lists_all_three_invocations() {
    let out = run_cli(&["--help"]);
    assert_eq!(out.code, Some(0_i32));
    for usage_line in [
        "obsidian-export [OPTIONS] SOURCE DESTINATION",
        "obsidian-export check [OPTIONS] SOURCE",
        "obsidian-export update [OPTIONS]",
    ] {
        assert!(out.stdout.contains(usage_line), "missing: {}", usage_line);
    }
}

#[test]
fn update_available_json_event_contract() {
    let (addr, server) = spawn_update_mock(
        |base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: release_json("v99.0.0", base).into_bytes(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--progress", "json"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");

    assert_eq!(out.code, Some(0_i32), "「有更新」不是失败：{}", out.stderr);
    let events = parse_json_lines(&out.stdout);
    assert_eq!(events.len(), 2, "schema + update-result：{events:?}");
    assert_eq!(event_at(&events, 0)["type"], "schema");
    assert_eq!(
        event_at(&events, 0)["version"],
        1,
        "与导出/check 共享 schema v1"
    );
    assert_eq!(event_at(&events, 1)["type"], "update-result");
    assert_eq!(event_at(&events, 1)["outcome"], "available");
    assert_eq!(
        event_at(&events, 1)["version"],
        "99.0.0",
        "版本号去掉 v 前缀"
    );
    assert_eq!(
        event_at(&events, 1)["htmlUrl"],
        "https://github.com/ONEGAYI/obsidian-export-desktop/releases/v99.0.0"
    );
    assert_eq!(event_at(&events, 1)["notes"], "release notes");
    // Windows 测试环境：cli 意图应挑本平台 zip 而非 sha256 副产物
    assert_eq!(
        event_at(&events, 1)["assetName"],
        "obsidian-export-x86_64-pc-windows-msvc.zip"
    );
    assert_eq!(event_at(&events, 1)["assetSize"], 4);
}

#[test]
fn update_available_text_output_mentions_asset_and_url() {
    let (addr, server) = spawn_update_mock(
        |base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: release_json("v99.0.0", base).into_bytes(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");

    assert_eq!(out.code, Some(0_i32));
    assert!(out.stdout.contains("99.0.0"), "stdout: {:?}", out.stdout);
    assert!(out.stdout.contains("releases/v99.0.0"));
    assert!(out.stdout.contains("--download"), "应提示下载方式");
    assert!(out
        .stdout
        .contains("obsidian-export-x86_64-pc-windows-msvc.zip"));
}

#[test]
fn update_desktop_asset_picks_setup_exe() {
    let (addr, server) = spawn_update_mock(
        |base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: release_json("v99.0.0", base).into_bytes(),
            }]
        },
        1,
    );
    let out = run_update_cli(
        &["update", "--asset", "desktop", "--progress", "json"],
        &format!("http://{addr}"),
    );
    server.join().expect("mock server panicked");

    assert_eq!(out.code, Some(0_i32));
    let events = parse_json_lines(&out.stdout);
    assert_eq!(
        event_at(&events, 1)["assetName"],
        "Obsidian.Export_99.0.0_x64-setup.exe"
    );
}

#[test]
fn update_up_to_date_json() {
    let current = env!("CARGO_PKG_VERSION");
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: format!(r#"{{"tag_name":"v{current}","assets":[]}}"#).into_bytes(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--progress", "json"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(0_i32));
    let events = parse_json_lines(&out.stdout);
    assert_eq!(events.len(), 2);
    assert_eq!(event_at(&events, 1)["type"], "update-result");
    assert_eq!(event_at(&events, 1)["outcome"], "up-to-date");
}

#[test]
fn update_no_release_json() {
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 404,
                content_type: "application/json",
                body: Vec::new(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--progress", "json"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(0_i32));
    let events = parse_json_lines(&out.stdout);
    assert_eq!(events.len(), 2);
    assert_eq!(event_at(&events, 1)["outcome"], "no-release");
}

#[test]
fn update_check_failure_exits_one_without_result_event() {
    // 限流 403：stderr 报错（含响应体 message），stdout 只有 schema 行
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 403,
                content_type: "application/json",
                body: br#"{"message":"API rate limit exceeded for 1.2.3.4."}"#.to_vec(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--progress", "json"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");

    assert_eq!(out.code, Some(1_i32));
    let events = parse_json_lines(&out.stdout);
    assert_eq!(events.len(), 1, "失败路径只有 schema 行：{events:?}");
    assert!(out.stderr.contains("HTTP 403"), "stderr: {:?}", out.stderr);
    assert!(
        out.stderr.contains("API rate limit exceeded"),
        "限流详情应透出：{:?}",
        out.stderr
    );
}

#[test]
fn update_bad_release_json_is_deterministic_failure() {
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: b"not json".to_vec(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(1_i32));
    assert!(out.stderr.contains("parse"), "stderr: {:?}", out.stderr);
}

#[test]
fn update_download_json_stream_and_saved_bytes() {
    let payload: &[u8] = b"ZIPDATA";
    let (addr, server) = spawn_update_mock(
        |base| {
            vec![
                MockRoute {
                    path: "/repos/",
                    status: 200,
                    content_type: "application/json",
                    body: release_json("v99.0.0", base).into_bytes(),
                },
                MockRoute {
                    path: "/cli.zip",
                    status: 200,
                    content_type: "application/octet-stream",
                    body: payload.to_vec(),
                },
            ]
        },
        2,
    );
    let dir = TempDir::new().expect("tempdir");
    let out = run_update_cli(
        &[
            "update",
            "--download",
            "--progress",
            "json",
            "--output",
            dir.path().to_str().expect("non-unicode tmpdir"),
        ],
        &format!("http://{addr}"),
    );
    server.join().expect("mock server panicked");

    assert_eq!(out.code, Some(0_i32), "stderr: {:?}", out.stderr);
    let events = parse_json_lines(&out.stdout);
    let types: Vec<&str> = events.iter().filter_map(|e| e["type"].as_str()).collect();
    // download-progress 至少两帧（首帧 0 字节 + 终态帧），期间可有节流帧。
    assert_eq!(types.first(), Some(&"schema"));
    assert_eq!(types[1], "update-result");
    assert_eq!(types[2], "download-start");
    assert_eq!(types.last(), Some(&"download-end"));
    let progress_count = types.iter().filter(|t| **t == "download-progress").count();
    assert!(progress_count >= 2, "事件序列：{:?}", events);

    let start = &event_at(&events, 2);
    // start 的 total 来自 release 元数据的 size（此处故意与实际字节不同，
    // 锁定两者来源：元数据预告 vs Content-Length 实测）
    assert_eq!(start["total"], 4, "download-start 携带 release 资产大小");
    let last_progress = &events[events.len() - 2];
    assert_eq!(last_progress["downloaded"], 7);
    assert_eq!(last_progress["total"], 7);
    let end = events.last().expect("download-end");
    let saved_path = end["path"].as_str().expect("download-end.path");
    assert!(saved_path.ends_with("obsidian-export-x86_64-pc-windows-msvc.zip"));
    assert_eq!(
        std::fs::read(
            dir.path()
                .join("obsidian-export-x86_64-pc-windows-msvc.zip")
        )
        .expect("下载文件应落盘"),
        payload,
        "字节原样"
    );
}

#[test]
fn update_download_rejects_path_shaped_asset_name() {
    // 恶意 release 把资产名写成「能通过前缀挑选、但含路径分隔符」的形
    // 态：落盘前必须拒绝（exit 1），不得逃出输出目录。
    let evil = r#"{"tag_name":"v99.0.0","html_url":"u","assets":[{"name":"obsidian-export-x86_64-pc-windows-msvc.zip/../../evil.exe","browser_download_url":"http://MOCKHOST/evil","size":1}]}"#;
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: evil.as_bytes().to_vec(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--download"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(1_i32));
    assert!(out.stderr.contains("refusing"), "stderr: {:?}", out.stderr);
    assert!(
        !std::path::Path::new("evil.exe").exists(),
        "不得逃出输出目录"
    );
}

#[test]
fn update_download_missing_output_dir_exits_one() {
    let (addr, server) = spawn_update_mock(
        |base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: release_json("v99.0.0", base).into_bytes(),
            }]
        },
        1,
    );
    let out = run_update_cli(
        &["update", "--download", "--output", "Z:/definitely/not/here"],
        &format!("http://{addr}"),
    );
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(1_i32));
    assert!(
        out.stderr.contains("does not exist"),
        "stderr: {:?}",
        out.stderr
    );
}

#[test]
fn update_without_matching_asset_still_exits_zero() {
    let no_asset = r#"{"tag_name":"v99.0.0","html_url":"https://x","assets":[]}"#;
    let (addr, server) = spawn_update_mock(
        |_base| {
            vec![MockRoute {
                path: "/repos/",
                status: 200,
                content_type: "application/json",
                body: no_asset.as_bytes().to_vec(),
            }]
        },
        1,
    );
    let out = run_update_cli(&["update", "--download"], &format!("http://{addr}"));
    server.join().expect("mock server panicked");
    assert_eq!(out.code, Some(0_i32), "无资产引导手动下载，不是失败");
    assert!(out.stdout.contains("manually"), "stdout: {:?}", out.stdout);
}
