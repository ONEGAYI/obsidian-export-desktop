//! Excalidraw conversion integration tests: these drive the CLI binary in a
//! subprocess with a mock `excalidraw-export` executable injected through
//! the debug-only `OBSIDIAN_EXPORT_DIAGRAM_BIN_EXCALIDRAW_EXPORT`
//! environment variable, so the tests run on any machine and in CI without
//! node or the real tool installed. On Windows the mock is a `.cmd` script,
//! which additionally exercises the `cmd.exe` wrapping path used for real
//! npm shims.
//!
//! The injection hook is compiled out of release builds, so these tests
//! require a dev-profile binary. Instead of silently vanishing under
//! `cargo test --release`, the assertion below fails the build loudly.

#![allow(
    clippy::indexing_slicing,
    clippy::default_numeric_fallback,
    clippy::uninlined_format_args,
    clippy::case_sensitive_file_extension_comparisons
)]

// Must not be a `#![cfg(debug_assertions)]` crate gate: that would compile the
// file to an empty test binary (0 tests, silently green) under --release.
const _: () = assert!(
    cfg!(debug_assertions),
    "excalidraw CLI tests need the debug-only mock injection hook; run cargo test without --release"
);

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// Path to the obsidian-export binary, provided by cargo for integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_obsidian-export");

/// The fixed bytes the mock converter produces.
const MOCK_IMAGE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\" fill=\"green\"/></svg>\n";

const MOCK_OUTPUT_FILE: &str = "mock-output.svg";

const INPUT: &str = "tests/testdata/input";

struct CliOutput {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn run_cli_env(args: &[&str], envs: &[(String, String)]) -> CliOutput {
    let output = Command::new(BIN)
        .args(args)
        .envs(envs.iter().cloned())
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

fn events_of_type<'a>(events: &'a [Value], kind: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event.get("type").and_then(Value::as_str) == Some(kind))
        .collect()
}

/// Mock tool installed into a temp directory, plus the env override pointing
/// the CLI at it.
struct MockEnv {
    #[allow(dead_code)] // kept alive for the script directory's lifetime
    dir: TempDir,
    envs: Vec<(String, String)>,
}

/// Selectable mock misbehavior for failure-mode coverage.
#[derive(Clone, Copy)]
enum MockVariant {
    /// Faithful mock: produces the fixture output.
    Normal,
    /// Exits 0 but produces no output file at all.
    NoOutput,
}

/// Install the mock `excalidraw-export` tool. The invocation layout is
/// `excalidraw-export IN --svg -o OUT` for SVG and `excalidraw-export IN -o
/// OUT` for PNG (the tool's default), so the output argument sits at a
/// different position per format; the mock picks the branch by argument
/// presence. Exits 1 when the input scene contains the marker `FAIL`.
fn install_mock(variant: MockVariant) -> MockEnv {
    let dir = TempDir::new().expect("failed to make tempdir");
    fs::write(dir.path().join(MOCK_OUTPUT_FILE), MOCK_IMAGE).expect("write mock output");

    let fixture = dir.path().join(MOCK_OUTPUT_FILE);
    #[cfg(windows)]
    let (path, body) = {
        let path = dir.path().join("excalidraw-export.cmd");
        let body = match variant {
            MockVariant::NoOutput => String::from("@echo off\r\nexit /b 0\r\n"),
            MockVariant::Normal => format!(
                // NB: `copy ... ""` exits 0 on cmd, so the layout cannot be
                // probed with an `||` fallback; branch on argument presence.
                "@echo off\r\nfindstr /C:\"FAIL\" \"%~1\" >nul 2>&1\r\nif %errorlevel%==0 exit /b 1\r\nif not \"%~4\"==\"\" (copy /Y \"{fix}\" \"%~4\" >nul) else (copy /Y \"{fix}\" \"%~3\" >nul)\r\nexit /b 0\r\n",
                fix = fixture.display(),
            ),
        };
        (path, body)
    };
    #[cfg(not(windows))]
    let path = {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.path().join("excalidraw-export");
        let body = match variant {
            MockVariant::NoOutput => String::from("#!/bin/sh\nexit 0\n"),
            MockVariant::Normal => format!(
                "#!/bin/sh\nin=\"$1\"\nif grep -q \"FAIL\" \"$in\"; then exit 1; fi\nif [ -n \"$4\" ]; then cp \"{fix}\" \"$4\"; else cp \"{fix}\" \"$3\"; fi\nexit 0\n",
                fix = fixture.display(),
            ),
        };
        fs::write(&path, &body).expect("write mock script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod mock script");
        path
    };
    #[cfg(windows)]
    fs::write(&path, body).expect("write mock script");

    let envs = vec![(
        String::from("OBSIDIAN_EXPORT_DIAGRAM_BIN_EXCALIDRAW_EXPORT"),
        path.to_string_lossy().into_owned(),
    )];
    MockEnv { dir, envs }
}

fn dest_dir() -> (TempDir, String) {
    let dir = TempDir::new().expect("failed to make tempdir");
    let path = dir.path().to_string_lossy().into_owned();
    (dir, path)
}

/// Sorted list of file names directly under `dir`.
fn list_dir(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("read {}: {}", dir.display(), err))
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn converts_drawings_and_rewrites_references() {
    let mocks = install_mock(MockVariant::Normal);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "excalidraw",
            &format!("{INPUT}/excalidraw"),
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    // Assets replace the source files, plugin-style naming
    // (`x.excalidraw.md` → `x.excalidraw.svg`, bare `.excalidraw` → `.svg`).
    for asset in ["glitch.excalidraw.svg", "drawing.svg", "legacy.svg"] {
        let content = fs::read_to_string(dest.path().join(asset))
            .unwrap_or_else(|err| panic!("read {}: {}", asset, err));
        assert_eq!(content, MOCK_IMAGE);
    }
    // The drawing files themselves are gone.
    let names = list_dir(dest.path());
    assert!(
        !names.contains(&String::from("glitch.excalidraw.md")),
        "{:?}",
        names
    );
    assert!(!names.contains(&String::from("drawing.md")), "{:?}", names);
    assert!(
        !names.contains(&String::from("legacy.excalidraw")),
        "{:?}",
        names
    );

    // Embeds become image references; aliases survive; numeric size labels
    // drop; section references embed the whole drawing (no anchor).
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(
        note.contains("![glitch.excalidraw.md](glitch.excalidraw.svg)"),
        "{:?}",
        note
    );
    assert!(
        note.contains("![glitch.excalidraw](glitch.excalidraw.svg)"),
        "{:?}",
        note
    );
    assert!(note.contains("![drawing](drawing.svg)"), "{:?}", note);
    assert!(
        note.contains("![legacy.excalidraw](legacy.svg)"),
        "{:?}",
        note
    );
    assert!(
        !note.contains(".svg#"),
        "no anchors on drawing references:\n{:?}",
        note
    );
    assert!(
        !note.contains("300"),
        "numeric size label dropped:\n{:?}",
        note
    );
    // Plain links point at the converted asset.
    assert!(
        note.contains("[the drawing](glitch.excalidraw.svg)"),
        "{:?}",
        note
    );
    // And the raw compressed payload never leaks into the host note.
    assert!(!note.contains("compressed-json"), "{:?}", note);

    // Event stream: three conversions plus skips for the source files.
    let events = parse_json_lines(&out.stdout);
    let renders = events_of_type(&events, "diagram-render");
    assert_eq!(renders.len(), 3, "{:?}", events);
    assert!(renders
        .iter()
        .all(|event| { event.get("language").and_then(Value::as_str) == Some("excalidraw") }));
    assert_eq!(
        events_of_type(&events, "file-skipped").len(),
        3,
        "{:?}",
        events
    );
    // The section reference on one embed warns.
    let warnings = events_of_type(&events, "warning");
    assert_eq!(warnings.len(), 1, "{:?}", events);
    assert!(
        warnings[0]
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("no image equivalent")),
        "{:?}",
        warnings
    );
}

#[test]
fn tool_failure_degrades_embeds_to_plain_links() {
    // The fixture scene contains the FAIL marker, so the mock exits 1.
    let mocks = install_mock(MockVariant::Normal);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "excalidraw",
            &format!("{INPUT}/excalidraw-fail"),
            &dest_str,
        ],
        &mocks.envs,
    );
    // Conversion failure is non-fatal: the export still completes.
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    // Neither the drawing nor any asset is exported.
    let names = list_dir(dest.path());
    assert_eq!(names, vec![String::from("note.md")], "{:?}", names);

    // Embeds degrade to a plain link to the original vault path plus an
    // italic notice; plain links keep pointing at the original file.
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(
        note.contains("[bad.excalidraw.md](bad.excalidraw.md)"),
        "{:?}",
        note
    );
    assert!(
        note.contains(
            "*Excalidraw drawing not rendered; the link points at the original vault file*"
        ),
        "{:?}",
        note
    );

    let events = parse_json_lines(&out.stdout);
    let warnings = events_of_type(&events, "warning");
    assert_eq!(warnings.len(), 1, "{:?}", events);
    assert!(
        warnings[0]
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("failed to render Excalidraw drawing")),
        "{:?}",
        warnings
    );
    assert_eq!(
        events_of_type(&events, "file-skipped").len(),
        1,
        "{:?}",
        events
    );
}

#[test]
fn no_output_failure_degrades_like_a_tool_error() {
    let mocks = install_mock(MockVariant::NoOutput);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "excalidraw",
            &format!("{INPUT}/excalidraw-fail"),
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);
    let names = list_dir(dest.path());
    assert_eq!(names, vec![String::from("note.md")], "{:?}", names);
}

#[test]
fn without_the_renderer_behavior_is_unchanged() {
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(&[&format!("{INPUT}/excalidraw"), &dest_str], &[]);
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    // Pre-feature behavior: drawings export as ordinary notes/assets and
    // embeds transclude the raw file (compressed payload included).
    for file in ["glitch.excalidraw.md", "drawing.md", "legacy.excalidraw"] {
        assert!(dest.path().join(file).is_file(), "missing {:?}", file);
    }
    assert!(!dest.path().join("glitch.excalidraw.svg").exists());
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("compressed-json"), "{:?}", note);
}

#[test]
fn png_format_writes_png_assets() {
    let mocks = install_mock(MockVariant::Normal);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "excalidraw",
            "--diagram-format",
            "png",
            &format!("{INPUT}/excalidraw"),
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);
    for asset in ["glitch.excalidraw.png", "drawing.png", "legacy.png"] {
        assert!(dest.path().join(asset).is_file(), "missing {:?}", asset);
    }
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("(glitch.excalidraw.png)"), "{:?}", note);
}

#[test]
fn out_of_scope_drawing_degrades_embed() {
    let mocks = install_mock(MockVariant::Normal);
    let (dest, dest_str) = dest_dir();
    // start-at narrows the export to sub/; the drawing at the vault root is
    // outside the scope and must not convert (or leak its payload).
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "excalidraw",
            "--start-at",
            &format!("{INPUT}/excalidraw-scope/sub"),
            &format!("{INPUT}/excalidraw-scope"),
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let host = fs::read_to_string(dest.path().join("host.md")).expect("read exported note");
    assert!(
        host.contains("[outside.excalidraw.md](../outside.excalidraw.md)"),
        "{:?}",
        host
    );
    assert!(
        host.contains(
            "*Excalidraw drawing not rendered; the link points at the original vault file*"
        ),
        "{:?}",
        host
    );

    let events = parse_json_lines(&out.stdout);
    assert!(
        events_of_type(&events, "diagram-render").is_empty(),
        "out-of-scope drawings must not convert: {:?}",
        events
    );
    let warnings = events_of_type(&events, "warning");
    assert_eq!(warnings.len(), 1, "{:?}", events);
    assert!(
        warnings[0]
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.contains("outside the export scope")),
        "{:?}",
        warnings
    );
}

#[test]
fn missing_tool_fails_atomically() {
    let (dest, dest_str) = dest_dir();
    // An explicit --diagram-bin path that does not exist fails the run
    // before any output file is written, regardless of what is on PATH.
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "excalidraw",
            "--diagram-bin",
            "excalidraw-export=C:/definitely/not/installed.exe",
            &format!("{INPUT}/excalidraw"),
            &dest_str,
        ],
        &[],
    );
    assert_ne!(out.code, Some(0));
    assert!(
        out.stderr.contains("excalidraw-export"),
        "stderr should name the tool: {}",
        out.stderr
    );
    assert!(
        !dest.path().join("note.md").exists(),
        "atomic failure must leave the destination untouched"
    );
}
