//! Diagram rendering integration tests: these drive the CLI binary in a
//! subprocess with mock renderer executables injected through the
//! debug-only `OBSIDIAN_EXPORT_DIAGRAM_BIN_<TOOL>` environment variables, so
//! the tests run on any machine and in CI without graphviz/npm/TeX
//! installed. On Windows the mocks are `.cmd` scripts, which additionally
//! exercises the `cmd.exe` wrapping path used for real npm shims.
//!
//! The injection hook is compiled out of release builds, so the whole file
//! is debug-only: under `cargo test --release` these tests vanish instead
//! of failing on missing tools.

#![cfg(debug_assertions)]
#![allow(
    clippy::indexing_slicing,
    clippy::default_numeric_fallback,
    clippy::uninlined_format_args,
    clippy::case_sensitive_file_extension_comparisons
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// Path to the obsidian-export binary, provided by cargo for integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_obsidian-export");

/// The fixed bytes every mock renderer produces.
const MOCK_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\" fill=\"green\"/></svg>\n";

const MOCK_OUTPUT_FILE: &str = "mock-output.svg";

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

/// Mock renderers installed into a temp directory, plus the env overrides
/// pointing the CLI at them.
struct MockEnv {
    #[allow(dead_code)] // kept alive for the script directory's lifetime
    dir: TempDir,
    envs: Vec<(String, String)>,
}

/// Install mock executables for the given tools (`"dot"`, `"mmdc"`,
/// `"wavedrom"`, `"latex"`, `"dvisvgm"`), behaving normally. Each invocation
/// is appended to `<tool>.calls` in the same directory for the idempotency
/// test.
fn install_mock_tools(tools: &[&str]) -> MockEnv {
    install_mock_variants(
        &tools
            .iter()
            .map(|tool| (*tool, MockVariant::Normal))
            .collect::<Vec<_>>(),
    )
}

/// Install mocks with per-tool behavior variants.
fn install_mock_variants(variants: &[(&str, MockVariant)]) -> MockEnv {
    let dir = TempDir::new().expect("failed to make tempdir");
    fs::write(dir.path().join(MOCK_OUTPUT_FILE), MOCK_SVG).expect("write mock output");

    let mut envs = Vec::new();
    for (tool, variant) in variants {
        let script = write_mock_script(dir.path(), tool, *variant);
        let key = format!(
            "OBSIDIAN_EXPORT_DIAGRAM_BIN_{}",
            tool.to_uppercase().replace('-', "_")
        );
        envs.push((key, script.to_string_lossy().into_owned()));
    }
    MockEnv { dir, envs }
}

/// Selectable mock misbehavior for failure-mode coverage.
#[derive(Clone, Copy)]
enum MockVariant {
    /// Faithful mock: produces the fixture output.
    Normal,
    /// Exits 0 but produces nothing at all (wavedrom: empty stdout; latex:
    /// no dvi file).
    NoOutput,
}

/// Write the mock script for one tool, matching the argument layout the
/// render pipeline actually invokes:
///
/// - `dot`:     `dot -Tsvg|-Tpng IN -o OUT` (the format flag is recorded in
///   the calls log so tests can pin the `-Tpng`/`-Tsvg` pass-through)
/// - `mmdc`:    `mmdc -i IN -o OUT`
/// - `wavedrom`: `wavedrom --input IN`, SVG on stdout
/// - `latex`:   `latex -interaction=nonstopmode -halt-on-error -output-directory DIR IN`
/// - `dvisvgm`: `dvisvgm --no-fonts --exact -o OUT IN`
///
/// All mocks exit 1 when the input contains the marker `FAIL`. Input is
/// always a file: a piped stdin is unreliable across the cmd.exe wrapper
/// (cmd pre-reads a chunk of the pipe).
#[allow(clippy::too_many_lines)]
fn write_mock_script(dir: &Path, tool: &str, variant: MockVariant) -> PathBuf {
    let fixture = dir.join(MOCK_OUTPUT_FILE);
    let calls = dir.join(format!("{tool}.calls"));

    #[cfg(windows)]
    let (path, script) = {
        let path = dir.join(format!("{tool}.cmd"));
        let body = match (tool, variant) {
            (_, MockVariant::NoOutput) => String::from("@echo off\r\nexit /b 0\r\n"),
            ("dot", MockVariant::Normal) => format!(
                "@echo off\r\nfindstr /C:\"FAIL\" \"%~2\" >nul 2>&1\r\nif %errorlevel%==0 exit /b 1\r\ncopy /Y \"{fix}\" \"%~4\" >nul\r\necho %~1>> \"{cnt}\" 2>nul\r\nexit /b 0\r\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("mmdc", MockVariant::Normal) => format!(
                "@echo off\r\nfindstr /C:\"FAIL\" \"%~2\" >nul 2>&1\r\nif %errorlevel%==0 exit /b 1\r\ncopy /Y \"{fix}\" \"%~4\" >nul\r\necho x>> \"{cnt}\" 2>nul\r\nexit /b 0\r\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("wavedrom", MockVariant::Normal) => format!(
                "@echo off\r\nfindstr /C:\"FAIL\" \"%~2\" >nul 2>&1\r\nif %errorlevel%==0 exit /b 1\r\ntype \"{fix}\"\r\necho x>> \"{cnt}\" 2>nul\r\nexit /b 0\r\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("latex", MockVariant::Normal) => format!(
                "@echo off\r\nfindstr /C:\"FAIL\" \"%~5\" >nul 2>&1\r\nif %errorlevel%==0 (\r\n  echo mock latex failure > \"%~4\\diagram.log\"\r\n  exit /b 1\r\n)\r\necho mock-dvi > \"%~4\\diagram.dvi\"\r\necho x>> \"{cnt}\" 2>nul\r\nexit /b 0\r\n",
                cnt = calls.display(),
            ),
            ("dvisvgm", MockVariant::Normal) => format!(
                "@echo off\r\ncopy /Y \"{fix}\" \"%~4\" >nul\r\necho x>> \"{cnt}\" 2>nul\r\nexit /b 0\r\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            (other, _) => panic!("unknown mock tool {}", other),
        };
        (path, body)
    };

    #[cfg(not(windows))]
    let (path, script) = {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(tool);
        let body = match (tool, variant) {
            (_, MockVariant::NoOutput) => String::from("#!/bin/sh\nexit 0\n"),
            ("dot", MockVariant::Normal) => format!(
                "#!/bin/sh\nin=\"$2\"; out=\"$4\"\nif grep -q \"FAIL\" \"$in\"; then exit 1; fi\ncp \"{fix}\" \"$out\"\necho \"$1\" >> \"{cnt}\" 2>/dev/null\nexit 0\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("mmdc", MockVariant::Normal) => format!(
                "#!/bin/sh\nin=\"$2\"; out=\"$4\"\nif grep -q \"FAIL\" \"$in\"; then exit 1; fi\ncp \"{fix}\" \"$out\"\necho x >> \"{cnt}\" 2>/dev/null\nexit 0\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("wavedrom", MockVariant::Normal) => format!(
                "#!/bin/sh\nin=\"$2\"\nif grep -q \"FAIL\" \"$in\"; then exit 1; fi\ncat \"{fix}\"\necho x >> \"{cnt}\" 2>/dev/null\nexit 0\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            ("latex", MockVariant::Normal) => format!(
                "#!/bin/sh\ndir=\"$4\"; tex=\"$5\"\nif grep -q \"FAIL\" \"$tex\"; then\n  echo mock latex failure > \"$dir/diagram.log\"\n  exit 1\nfi\necho mock-dvi > \"$dir/diagram.dvi\"\necho x >> \"{cnt}\" 2>/dev/null\nexit 0\n",
                cnt = calls.display(),
            ),
            ("dvisvgm", MockVariant::Normal) => format!(
                "#!/bin/sh\nout=\"$4\"\ncp \"{fix}\" \"$out\"\necho x >> \"{cnt}\" 2>/dev/null\nexit 0\n",
                fix = fixture.display(),
                cnt = calls.display(),
            ),
            (other, _) => panic!("unknown mock tool {}", other),
        };
        fs::write(&path, body).expect("write mock script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod mock script");
        (path, body)
    };

    #[cfg(windows)]
    fs::write(&path, script).expect("write mock script");

    path
}

fn dest_dir() -> (TempDir, String) {
    let dir = TempDir::new().expect("failed to make tempdir");
    let path = dir.path().to_string_lossy().into_owned();
    (dir, path)
}

fn list_assets(dest: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dest.join("assets"))
        .expect("assets dir should exist")
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

// Each test installs its own mock directory: a shared one would have
// parallel test processes appending to the same `.calls` files, and cmd's
// `>>` open does not allow concurrent writers on Windows.

#[test]
fn renders_all_four_diagram_languages_to_svg_assets() {
    let mocks = install_mock_tools(&["dot", "mmdc", "wavedrom", "latex", "dvisvgm"]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot,mermaid,wavedrom,tikz",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let events = parse_json_lines(&out.stdout);
    for event in &events {
        if event.get("type").and_then(Value::as_str) == Some("warning") {
            eprintln!("[diag] warning: {}", event);
        }
    }
    assert_eq!(
        events[0].get("type").and_then(Value::as_str),
        Some("schema"),
        "first line must be the schema header"
    );

    let renders = events_of_type(&events, "diagram-render");
    assert_eq!(renders.len(), 4, "one event per renderable block");
    let languages: Vec<&str> = renders
        .iter()
        .filter_map(|event| event.get("language").and_then(Value::as_str))
        .collect();
    let mut sorted = languages.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec!["dot", "mermaid", "tikz", "wavedrom"]);
    for event in &renders {
        assert_eq!(event.get("total").and_then(Value::as_u64), Some(4));
    }
    let indexes: std::collections::BTreeSet<u64> = renders
        .iter()
        .filter_map(|event| event.get("index").and_then(Value::as_u64))
        .collect();
    assert_eq!(indexes, std::collections::BTreeSet::from([1, 2, 3, 4]));

    // The note itself: four image references, no diagram fences left, the
    // untouched rust block survives.
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    for language in ["dot", "mermaid", "wavedrom", "tikz"] {
        assert!(
            note.contains(&format!("![diagram ({language})](assets/note-")),
            "missing image reference for {} in:\n{}",
            language,
            note
        );
        assert!(
            !note.contains(&format!("```{language}")),
            "{} fence should be gone",
            language
        );
    }
    assert!(note.contains("```rust"), "plain code block must survive");

    // Assets: four content-addressed SVGs with the mock bytes.
    let assets = list_assets(dest.path());
    assert_eq!(assets.len(), 4, "assets: {:?}", assets);
    for name in &assets {
        assert!(
            name.starts_with("note-") && name.ends_with(".svg"),
            "{}",
            name
        );
        let content =
            fs::read_to_string(dest.path().join("assets").join(name)).expect("read asset");
        assert_eq!(content, MOCK_SVG);
    }
}

#[test]
fn png_format_produces_png_where_supported_and_falls_back_to_svg() {
    let mocks = install_mock_tools(&["dot", "mmdc", "wavedrom", "latex", "dvisvgm"]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot,wavedrom",
            "--diagram-format",
            "png",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let events = parse_json_lines(&out.stdout);
    let warnings: Vec<&str> = events_of_type(&events, "warning")
        .iter()
        .filter_map(|event| event.get("message").and_then(Value::as_str))
        .collect();
    assert!(
        warnings
            .iter()
            .any(|message| message.contains("fell back to svg")),
        "expected a fallback warning, got: {:?}",
        warnings
    );

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(
        note.contains("![diagram (dot)](assets/note-"),
        "dot image reference missing, note:\n{}",
        note
    );
    assert!(note.contains(".png)"));
    // wavedrom fell back to svg
    assert!(note.contains("![diagram (wavedrom)](assets/note-"));
    assert!(note.contains(".svg)"));
    // Not enabled: mermaid/tikz blocks stay untouched.
    assert!(note.contains("```mermaid"));
    assert!(note.contains("```tikz"));

    let assets = list_assets(dest.path());
    let pngs = assets.iter().filter(|name| name.ends_with(".png")).count();
    let svgs = assets.iter().filter(|name| name.ends_with(".svg")).count();
    assert_eq!(pngs, 1, "assets: {:?}", assets);
    assert_eq!(svgs, 1, "assets: {:?}", assets);

    // The dot mock records the format flag it was invoked with: pin the
    // actual `-Tpng` pass-through (a mock that ignored its arguments would
    // render "png" files with SVG bytes unnoticed).
    let dot_calls = fs::read_to_string(mocks.dir.path().join("dot.calls")).expect("read dot calls");
    assert_eq!(
        dot_calls.trim(),
        "-Tpng",
        "dot must be asked for PNG output"
    );
}

#[test]
fn renderer_failure_keeps_code_block_and_warns() {
    let mocks = install_mock_tools(&["dot", "mmdc", "wavedrom", "latex", "dvisvgm"]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot",
            "tests/testdata/input/diagrams-fail",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(
        out.code,
        Some(0),
        "a failing block is non-fatal, stderr: {}",
        out.stderr
    );

    let events = parse_json_lines(&out.stdout);
    // The progress event fires before the renderer runs, so a failing block
    // still announced itself first.
    let renders = events_of_type(&events, "diagram-render");
    assert_eq!(renders.len(), 1, "render progress must precede the attempt");
    assert_eq!(
        renders[0].get("language").and_then(Value::as_str),
        Some("dot")
    );
    let warnings: Vec<&str> = events_of_type(&events, "warning")
        .iter()
        .filter_map(|event| event.get("message").and_then(Value::as_str))
        .collect();
    assert!(
        warnings
            .iter()
            .any(|message| message.contains("failed to render dot")),
        "expected a render failure warning, got: {:?}",
        warnings
    );

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(
        note.contains("```dot"),
        "the original code block must be kept"
    );
    assert!(
        !note.contains("](assets/"),
        "no image reference expected on failure"
    );
    // No asset files anywhere in the destination.
    let svgs = walk_for_extension(dest.path(), "svg");
    assert!(svgs.is_empty(), "leftover assets: {:?}", svgs);
}

#[test]
fn missing_tool_fails_atomically_without_writing_output() {
    let (dest, dest_str) = dest_dir();
    // An explicit path that does not exist fails tool resolution during the
    // prescan — before any output file is written. No env hooks are set, so
    // the explicit path is what resolution sees (the debug env override
    // would otherwise take precedence over --diagram-bin).
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "tikz",
            "--diagram-bin",
            "latex=C:\\definitely\\not\\installed\\latex.exe",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &[],
    );
    assert_eq!(out.code, Some(1));
    assert!(
        out.stderr.contains("latex"),
        "error should name the missing tool, got: {}",
        out.stderr
    );
    assert!(
        out.stderr.contains("tikz") || out.stderr.contains("renderer"),
        "error should mention the renderer, got: {}",
        out.stderr
    );
    // Atomicity: the destination stays empty (no note, no assets).
    let entries: Vec<_> = fs::read_dir(dest.path())
        .expect("dest dir readable")
        .collect();
    assert!(
        entries.is_empty(),
        "destination must stay untouched, got {:?}",
        entries.len()
    );
    // The event stream still opened with the schema line, never announced
    // a start, and reported no file-done before dying.
    let events = parse_json_lines(&out.stdout);
    assert_eq!(
        events[0].get("type").and_then(Value::as_str),
        Some("schema")
    );
    assert!(
        events_of_type(&events, "start").is_empty(),
        "tool resolution must fail before the start event"
    );
    assert!(
        events_of_type(&events, "file-done").is_empty(),
        "no file should have been exported"
    );
}

#[test]
fn second_export_reuses_cached_assets_without_new_renders() {
    let mocks = install_mock_tools(&["wavedrom"]);
    let (dest, dest_str) = dest_dir();
    let args = [
        "--progress",
        "json",
        "--render-diagrams",
        "wavedrom",
        "tests/testdata/input/diagrams-png",
        "",
    ];

    let mut first = args;
    first[5] = dest_str.as_str();
    let out = run_cli_env(&first, &mocks.envs);
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let calls_first = fs::read_to_string(mocks.dir.path().join("wavedrom.calls"))
        .expect("read calls")
        .lines()
        .count();
    assert_eq!(calls_first, 1);

    let out_second = run_cli_env(&first, &mocks.envs);
    assert_eq!(out_second.code, Some(0), "stderr: {}", out_second.stderr);
    // Cache hits still emit their progress event: the GUI must see every
    // block pass by even when nothing is re-rendered.
    let events_second = parse_json_lines(&out_second.stdout);
    assert_eq!(
        events_of_type(&events_second, "diagram-render").len(),
        1,
        "cached renders still report progress"
    );

    let calls_second = fs::read_to_string(mocks.dir.path().join("wavedrom.calls"))
        .expect("read calls")
        .lines()
        .count();
    assert_eq!(
        calls_second, calls_first,
        "cached asset must not invoke the renderer again"
    );

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("![diagram (wavedrom)](assets/note-"));
}

#[test]
fn explicit_diagram_bin_overrides_path_lookup() {
    // Point --diagram-bin at the mock while leaving the env hooks unset: on
    // machines with a real `dot` on PATH, only the explicit path can produce
    // the mock bytes.
    let mocks = install_mock_tools(&["dot"]);
    let mock_dot = mocks
        .dir
        .path()
        .join(if cfg!(windows) { "dot.cmd" } else { "dot" });
    let (dest, dest_str) = dest_dir();
    let bin_arg = format!("dot={}", mock_dot.display());
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot",
            "--diagram-bin",
            &bin_arg,
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &[],
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let assets = list_assets(dest.path());
    assert_eq!(assets.len(), 1, "assets: {:?}", assets);
    let content =
        fs::read_to_string(dest.path().join("assets").join(&assets[0])).expect("read asset");
    assert_eq!(
        content, MOCK_SVG,
        "explicit --diagram-bin must win over any PATH lookup"
    );
}

#[test]
fn prescan_only_requires_tools_for_languages_actually_present() {
    // The vault holds a wavedrom block only; the dot tool's explicit path is
    // broken, but dot is never required, so the export must succeed.
    let mocks = install_mock_tools(&["wavedrom"]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot,wavedrom",
            "--diagram-bin",
            "dot=/definitely/not/installed/dot",
            "tests/testdata/input/diagrams-png",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(
        out.code,
        Some(0),
        "absent languages must not require their tools, stderr: {}",
        out.stderr
    );
    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("![diagram (wavedrom)](assets/note-"));
}

#[test]
fn invalid_render_diagrams_value_is_rejected() {
    let (_dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "dot,mermaidcharts",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &[],
    );
    assert_eq!(out.code, Some(2), "bad renderer names are usage errors");
    assert!(
        out.stderr.contains("mermaidcharts") || out.stderr.contains("renderer"),
        "error should name the bad value, got: {}",
        out.stderr
    );

    let out_bad_tool = run_cli_env(
        &[
            "--render-diagrams",
            "dot",
            "--diagram-bin",
            "not-a-tool=/bin/true",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &[],
    );
    assert_eq!(
        out_bad_tool.code,
        Some(2),
        "bad tool names are usage errors"
    );
    assert!(
        out_bad_tool.stderr.contains("not-a-tool") || out_bad_tool.stderr.contains("tool"),
        "error should name the bad tool, got: {}",
        out_bad_tool.stderr
    );
}

/// Run the CLI with `PATH` removed from its environment, so tool
/// resolution cannot find anything and must take the "not on PATH" path.
fn run_cli_stripped_path(args: &[&str]) -> CliOutput {
    let output = Command::new(BIN)
        .args(args)
        .env_remove("PATH")
        .output()
        .expect("failed to run CLI");
    CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code(),
    }
}

#[test]
fn empty_tool_output_degrades_to_warning() {
    // wavedrom exits 0 but prints nothing: the pipeline must detect the
    // missing output and fall back to the code block, not embed an empty
    // file.
    let mocks = install_mock_variants(&[("wavedrom", MockVariant::NoOutput)]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "wavedrom",
            "tests/testdata/input/diagrams-png",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let events = parse_json_lines(&out.stdout);
    let warnings: Vec<&str> = events_of_type(&events, "warning")
        .iter()
        .filter_map(|event| event.get("message").and_then(Value::as_str))
        .collect();
    assert!(
        warnings
            .iter()
            .any(|message| message.contains("produced no")),
        "expected an output-missing warning, got: {:?}",
        warnings
    );

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("```wavedrom"), "code block must be kept");
    let svgs = walk_for_extension(dest.path(), "svg");
    assert!(svgs.is_empty(), "no asset expected, got {:?}", svgs);
}

#[test]
fn latex_without_dvi_degrades_to_warning() {
    // latex exits 0 but never writes the dvi: same missing-output contract
    // on the tikz pipeline.
    let mocks = install_mock_variants(&[
        ("latex", MockVariant::NoOutput),
        ("dvisvgm", MockVariant::Normal),
    ]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "tikz",
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let events = parse_json_lines(&out.stdout);
    let warnings: Vec<&str> = events_of_type(&events, "warning")
        .iter()
        .filter_map(|event| event.get("message").and_then(Value::as_str))
        .collect();
    assert!(
        warnings
            .iter()
            .any(|message| message.contains("produced no")),
        "expected an output-missing warning, got: {:?}",
        warnings
    );

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(note.contains("```tikz"), "code block must be kept");
}

#[test]
fn debug_env_hook_takes_precedence_over_explicit_bin() {
    // Resolution priority is env hook > --diagram-bin > PATH. Point the env
    // hook at a working mock and --diagram-bin at a script that always
    // fails: only the env hook winning produces a rendered asset.
    let mocks = install_mock_tools(&["dot"]);

    let fail_dir = TempDir::new().expect("failed to make tempdir");
    #[cfg(windows)]
    let fail_dot = fail_dir.path().join("dot.cmd");
    #[cfg(not(windows))]
    let fail_dot = fail_dir.path().join("dot");
    #[cfg(windows)]
    fs::write(&fail_dot, "@echo off\r\nexit /b 1\r\n").expect("write failing mock");
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(&fail_dot, "#!/bin/sh\nexit 1\n").expect("write failing mock");
        fs::set_permissions(&fail_dot, fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    let (dest, dest_str) = dest_dir();
    let bin_arg = format!("dot={}", fail_dot.display());
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "dot",
            "--diagram-bin",
            &bin_arg,
            "tests/testdata/input/diagrams",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(
        out.code,
        Some(0),
        "the env hook must win and render fine, stderr: {}",
        out.stderr
    );
    let assets = list_assets(dest.path());
    assert_eq!(assets.len(), 1, "assets: {:?}", assets);
}

#[test]
fn missing_path_tool_reports_install_hint() {
    // With PATH stripped and no explicit path, resolution takes the
    // NotFoundOnPath branch whose hint says how to install the tool (the
    // missing_tool test above only covers the explicit-path branch).
    let (dest, dest_str) = dest_dir();
    let out = run_cli_stripped_path(&[
        "--progress",
        "json",
        "--render-diagrams",
        "dot",
        "tests/testdata/input/diagrams",
        &dest_str,
    ]);
    assert_eq!(out.code, Some(1));
    assert!(
        out.stderr.contains("dot"),
        "error should name the tool, got: {}",
        out.stderr
    );
    assert!(
        out.stderr.to_lowercase().contains("install"),
        "error should carry an install hint, got: {}",
        out.stderr
    );
    let entries: Vec<_> = fs::read_dir(dest.path())
        .expect("dest dir readable")
        .collect();
    assert!(
        entries.is_empty(),
        "destination must stay untouched, got {:?}",
        entries.len()
    );
}

#[test]
fn single_file_export_renders_into_sibling_assets_dir() {
    // Single-file mode: the source is one .md file and the destination a
    // filename; assets land in an `assets/` directory next to the output
    // file, named after the output file's stem.
    let mocks = install_mock_tools(&["dot"]);
    let (dest, dest_str) = dest_dir();
    let out_path = format!("{dest_str}\\out.md");
    let out = run_cli_env(
        &[
            "--render-diagrams",
            "dot",
            "tests/testdata/input/diagrams/note.md",
            &out_path,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let note = fs::read_to_string(dest.path().join("out.md")).expect("read exported note");
    assert!(
        note.contains("![diagram (dot)](assets/out-"),
        "image reference must target the sibling assets dir, note:\n{}",
        note
    );
    assert!(
        note.contains("```mermaid"),
        "other languages stay untouched"
    );

    let assets = list_assets(dest.path());
    assert_eq!(assets.len(), 1, "assets: {:?}", assets);
    assert!(
        assets[0].starts_with("out-") && assets[0].ends_with(".svg"),
        "{}",
        assets[0]
    );
}

#[test]
fn language_aliases_render_and_report_verbatim() {
    // `graphviz` maps to the dot renderer and `mmd` to mermaid; the
    // progress event reports the fence's own first word, not the canonical
    // renderer name.
    let mocks = install_mock_tools(&["dot", "mmdc"]);
    let (dest, dest_str) = dest_dir();
    let out = run_cli_env(
        &[
            "--progress",
            "json",
            "--render-diagrams",
            "dot,mermaid",
            "tests/testdata/input/diagrams-alias",
            &dest_str,
        ],
        &mocks.envs,
    );
    assert_eq!(out.code, Some(0), "stderr: {}", out.stderr);

    let events = parse_json_lines(&out.stdout);
    let mut languages: Vec<&str> = events_of_type(&events, "diagram-render")
        .iter()
        .filter_map(|event| event.get("language").and_then(Value::as_str))
        .collect();
    languages.sort_unstable();
    assert_eq!(languages, vec!["graphviz", "mmd"]);

    let note = fs::read_to_string(dest.path().join("note.md")).expect("read exported note");
    assert!(
        note.contains("![diagram (graphviz)](assets/note-"),
        "alias blocks render, note:\n{}",
        note
    );
    assert!(note.contains("![diagram (mmd)](assets/note-"));
    let assets = list_assets(dest.path());
    assert_eq!(assets.len(), 2, "assets: {:?}", assets);
}

fn walk_for_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
                found.push(path);
            } else {
                // Other files are not relevant here.
            }
        }
    }
    found
}
