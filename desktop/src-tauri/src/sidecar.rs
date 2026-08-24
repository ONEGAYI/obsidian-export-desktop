//! Orchestration of the obsidian-export CLI sidecar.
//!
//! The desktop app never implements any conversion logic itself: it spawns the
//! bundled CLI with `--progress json`, forwards the parsed event stream to the
//! frontend, and maps process exit onto UI state (see docs/sidecar-events.md).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::events::{self, ParsedCheckLine, ParsedLine};

/// Tauri event names emitted to the frontend.
pub const EVENT_SIDECAR_EVENT: &str = "sidecar-event";
pub const EVENT_SIDECAR_ERROR: &str = "sidecar-error";
pub const EVENT_SIDECAR_EXIT: &str = "sidecar-exit";
pub const EVENT_CHECK_EVENT: &str = "check-event";
pub const EVENT_CHECK_ERROR: &str = "check-error";
pub const EVENT_CHECK_EXIT: &str = "check-exit";

/// Handle of a running export, if any.
#[derive(Default)]
pub struct ExportState {
    child: Mutex<Option<CommandChild>>,
}

fn take_child(app: &AppHandle) -> Option<CommandChild> {
    let state = app.state::<ExportState>();
    let mut guard = state
        .child
        .lock()
        .expect("export state mutex poisoned");
    guard.take()
}

/// Compute the effective export destination.
///
/// With `keep_root_folder`, exporting a directory source lands in
/// `<destination>/<source folder name>` instead of scattering the vault's
/// first-level entries directly into `destination`. File sources and root
/// paths without a file name are passed through unchanged.
pub fn resolve_destination(
    source: &Path,
    destination: &Path,
    keep_root_folder: bool,
    source_is_dir: bool,
) -> PathBuf {
    match keep_root_folder && source_is_dir {
        true => match source.file_name() {
            Some(name) => destination.join(name),
            None => destination.to_path_buf(),
        },
        false => destination.to_path_buf(),
    }
}

/// Frontmatter strategy selection; mirrors the CLI `--frontmatter` enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrontmatterChoice {
    Always,
    Never,
    #[default]
    Auto,
}

impl FrontmatterChoice {
    fn as_flag(self) -> &'static str {
        match self {
            FrontmatterChoice::Always => "always",
            FrontmatterChoice::Never => "never",
            FrontmatterChoice::Auto => "auto",
        }
    }
}

/// Missing-section strategy selection; mirrors the CLI `--missing-section`
/// enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MissingSectionChoice {
    EmbedFull,
    Fail,
    #[default]
    Skip,
}

impl MissingSectionChoice {
    fn as_flag(self) -> &'static str {
        match self {
            MissingSectionChoice::EmbedFull => "embed-full",
            MissingSectionChoice::Fail => "fail",
            MissingSectionChoice::Skip => "skip",
        }
    }
}

/// Which tree the automatic post-export link check walks.
///
/// `Source` re-checks the vault (catching broken wikilinks before they
/// collapse into italic text during conversion); `Destination` checks the
/// exported output (verifying the emitted markdown links and anchors).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkCheckTarget {
    #[default]
    Source,
    Destination,
}

/// User-configurable export options, one field per CLI flag of the sidecar.
///
/// Defaults mirror the CLI defaults: `build_args` only emits flags for
/// non-default values, so the CLI stays the single source of default
/// behavior and the options summary shown in the UI can be derived from the
/// same comparison.
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportOptions {
    /// Only export notes under this absolute sub-path of the vault.
    pub start_at: Option<String>,
    pub frontmatter: FrontmatterChoice,
    /// Name of the ignore-pattern file; `None` keeps the CLI default
    /// (`.export-ignore`).
    pub ignore_file: Option<String>,
    pub skip_tags: Vec<String>,
    pub only_tags: Vec<String>,
    pub hidden: bool,
    pub no_git: bool,
    pub no_recursive_embeds: bool,
    pub preserve_mtime: bool,
    pub missing_section: MissingSectionChoice,
    pub fail_fast: bool,
    pub hard_linebreaks: bool,
    /// GUI-only preference: run the link checker after a successful export.
    /// Never part of `build_args`; the frontend orchestrates the follow-up
    /// `start_check` invocation.
    pub link_check_enabled: bool,
    /// GUI-only preference: which tree that automatic check walks.
    pub link_check_target: LinkCheckTarget,
}

/// Build the sidecar argv. `--progress json` is always passed (the desktop
/// app consumes the JSON Lines event stream); user options are only passed
/// when they deviate from the CLI defaults.
fn build_args(options: &ExportOptions, source: &str, target: &str) -> Vec<String> {
    let mut args = vec!["--progress".to_owned(), "json".to_owned()];
    // Blank strings count as unset: the frontend already maps them to null,
    // but the invoke boundary must not rely on that.
    if let Some(start_at) = options
        .start_at
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        args.extend(["--start-at".to_owned(), start_at.to_owned()]);
    }
    if options.frontmatter != FrontmatterChoice::Auto {
        args.extend([
            "--frontmatter".to_owned(),
            options.frontmatter.as_flag().to_owned(),
        ]);
    }
    if let Some(ignore_file) = options
        .ignore_file
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        args.extend(["--ignore-file".to_owned(), ignore_file.to_owned()]);
    }
    for tag in &options.skip_tags {
        if !tag.trim().is_empty() {
            args.extend(["--skip-tags".to_owned(), tag.clone()]);
        }
    }
    for tag in &options.only_tags {
        if !tag.trim().is_empty() {
            args.extend(["--only-tags".to_owned(), tag.clone()]);
        }
    }
    if options.hidden {
        args.push("--hidden".to_owned());
    }
    if options.no_git {
        args.push("--no-git".to_owned());
    }
    if options.no_recursive_embeds {
        args.push("--no-recursive-embeds".to_owned());
    }
    if options.preserve_mtime {
        args.push("--preserve-mtime".to_owned());
    }
    if options.missing_section != MissingSectionChoice::Skip {
        args.extend([
            "--missing-section".to_owned(),
            options.missing_section.as_flag().to_owned(),
        ]);
    }
    if options.fail_fast {
        args.push("--fail-fast".to_owned());
    }
    if options.hard_linebreaks {
        args.push("--hard-linebreaks".to_owned());
    }
    args.push(source.to_owned());
    args.push(target.to_owned());
    args
}

/// Build the sidecar argv for `obsidian-export check`.
///
/// Checking the vault source forwards the non-default filter flags
/// (`--start-at`, `--ignore-file`, `--hidden`, `--no-git`) so the checked
/// walk set matches the exported one (tag post-processing is export-only and
/// deliberately not part of check).
///
/// Checking the exported output re-applies no vault filters (the tree is
/// already filtered), but the CLI's *defaults* are themselves filters:
/// `honor_gitignore` would silently exclude an output folder living inside
/// a git repository, and `ignore_hidden` would miss dot-files that the
/// export produced under `--hidden`. So `--no-git` is always passed and
/// `--hidden` mirrors the export's setting.
fn build_check_args(
    options: &ExportOptions,
    target: LinkCheckTarget,
    source: &str,
) -> Vec<String> {
    let mut args = vec![
        "check".to_owned(),
        "--progress".to_owned(),
        "json".to_owned(),
    ];
    match target {
        LinkCheckTarget::Source => {
            if let Some(start_at) = options
                .start_at
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                args.extend(["--start-at".to_owned(), start_at.to_owned()]);
            }
            if let Some(ignore_file) = options
                .ignore_file
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                args.extend(["--ignore-file".to_owned(), ignore_file.to_owned()]);
            }
            if options.hidden {
                args.push("--hidden".to_owned());
            }
            if options.no_git {
                args.push("--no-git".to_owned());
            }
        }
        LinkCheckTarget::Destination => {
            if options.hidden {
                args.push("--hidden".to_owned());
            }
            args.push("--no-git".to_owned());
        }
    }
    args.push(source.to_owned());
    args
}

/// Handshake: run `--version` and return the banner (e.g. "obsidian-export 25.9.0").
///
/// Called on startup so a stale or mismatched sidecar is reported clearly instead
/// of failing later with cryptic event-stream errors.
#[tauri::command]
pub async fn check_sidecar(app: AppHandle) -> Result<String, String> {
    let output = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(["--version"])
        .output()
        .await
        .map_err(|err| format!("failed to run sidecar: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "sidecar --version exited with {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let banner = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if banner.is_empty() {
        return Err("sidecar --version printed nothing".to_string());
    }
    Ok(banner)
}

/// Start an export. Emits `sidecar-event` per parsed JSON Lines event and a
/// final `sidecar-exit` with the process exit code (0/1/2 per the contract).
///
/// `options` carries the user-configurable CLI flags (only non-default values
/// are passed through, see [`build_args`]). `keep_root_folder` is a
/// desktop-only concept routing a directory source into
/// `<destination>/<source folder name>` (created if missing) so the vault's
/// first-level entries don't scatter directly in the destination.
///
/// Returns the actual export destination (after `keep_root_folder`
/// resolution) so the frontend can point later actions — like the post-export
/// link check — at the tree that was written.
#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    destination: String,
    options: ExportOptions,
    keep_root_folder: Option<bool>,
) -> Result<String, String> {
    let mut slot = state.child.lock().map_err(|_| "export state poisoned")?;
    if slot.is_some() {
        return Err("a sidecar process is already running".to_string());
    }

    // Resolve picked paths against the GUI process's working directory up
    // front, so the sidecar contract holds even for manually typed relative
    // paths: the events echo `path` back in the same absolute shape they
    // were given (docs/sidecar-events.md).
    let source_path = std::path::absolute(&source)
        .map_err(|err| format!("invalid source path '{source}': {err}"))?;
    let destination_path = std::path::absolute(&destination)
        .map_err(|err| format!("invalid destination path '{destination}': {err}"))?;
    let target = resolve_destination(
        &source_path,
        &destination_path,
        keep_root_folder.unwrap_or(false),
        source_path.is_dir(),
    );
    // The user-picked destination always exists; only the appended subfolder
    // needs creating. A mistyped manual destination is left for the CLI to
    // report rather than being silently created here.
    if target != destination_path {
        std::fs::create_dir_all(&target).map_err(|err| {
            format!("failed to create destination '{}': {err}", target.display())
        })?;
    }
    let source_arg = source_path.to_string_lossy().into_owned();
    let target_arg = target.to_string_lossy().into_owned();

    let args = build_args(&options, &source_arg, &target_arg);
    let (rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    *slot = Some(child);
    drop(slot);

    tauri::async_runtime::spawn(pump_sidecar(app.clone(), rx, StreamDialect::Export));

    Ok(target_arg)
}

/// Which JSON Lines dialect a spawned sidecar speaks; decides the parse
/// function and the frontend channels used by [`pump_sidecar`].
#[derive(Clone, Copy)]
enum StreamDialect {
    Export,
    Check,
}

impl StreamDialect {
    fn event_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_EVENT,
            Self::Check => EVENT_CHECK_EVENT,
        }
    }

    fn exit_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_EXIT,
            Self::Check => EVENT_CHECK_EXIT,
        }
    }

    /// Parse errors and sidecar IO errors go to the dialect's own channel so
    /// the frontend can surface them next to the stream they belong to
    /// (check runs while the export log view is gone).
    fn error_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_ERROR,
            Self::Check => EVENT_CHECK_ERROR,
        }
    }

    /// Parse one stdout line into a forwardable event; `None` for blank or
    /// unknown event types (a newer sidecar may add kinds; skipping them
    /// keeps older app builds usable), an error for malformed lines or an
    /// unsupported schema version.
    fn parse(self, line: &str) -> Result<Option<serde_json::Value>, String> {
        match self {
            Self::Export => match events::parse_line(line) {
                Ok(ParsedLine::Event(event)) => Ok(Some(event_value(&event))),
                Ok(ParsedLine::Ignored) => Ok(None),
                Err(err) => Err(err),
            },
            Self::Check => match events::parse_check_line(line) {
                Ok(ParsedCheckLine::Event(event)) => Ok(Some(event_value(&event))),
                Ok(ParsedCheckLine::Ignored) => Ok(None),
                Err(err) => Err(err),
            },
        }
    }
}

/// Serialize a parsed event for the frontend. These are plain data types, so
/// serialization cannot fail in practice.
fn event_value(event: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(event).expect("event serializes to JSON")
}

/// Shared stdout/stderr pump for a spawned sidecar: parses each JSON Lines
/// event with the stream's dialect, forwards it to the dialect's event
/// channel, and maps process termination onto the dialect's exit event
/// (`{code, stderr}`). Releasing the child slot on termination lets the
/// next export or check start.
async fn pump_sidecar(
    handle: AppHandle,
    mut rx: tauri::async_runtime::Receiver<CommandEvent>,
    dialect: StreamDialect,
) {
    // Bytes are buffered until a full line arrives and only then decoded:
    // chunks may split a multi-byte UTF-8 character in the middle, and
    // per-chunk lossy decoding would corrupt it into U+FFFD. The CLI always
    // emits valid UTF-8, so per-line lossy decoding is safe.
    let mut stdout_buffer: Vec<u8> = Vec::new();
    let mut stderr_text = String::new();
    while let Some(message) = rx.recv().await {
        match message {
            CommandEvent::Stdout(bytes) => {
                stdout_buffer.extend_from_slice(&bytes);
                while let Some(pos) = stdout_buffer.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = stdout_buffer.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&line_bytes);
                    match dialect.parse(&line) {
                        Ok(Some(event)) => {
                            let _ = handle.emit(dialect.event_channel(), &event);
                        }
                        Ok(None) => (),
                        Err(err) => {
                            let _ = handle.emit(dialect.error_channel(), err);
                        }
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                stderr_text.push_str(&String::from_utf8_lossy(&bytes));
            }
            CommandEvent::Error(err) => {
                let _ = handle.emit(
                    dialect.error_channel(),
                    format!("sidecar IO error: {err}"),
                );
            }
            CommandEvent::Terminated(status) => {
                // The child handle is dead now; release it so a new export
                // or check can start. Reaching termination without an `end`
                // (or `check-end`) event means the run never reached
                // processing (see docs/sidecar-events.md).
                take_child(&handle);
                let _ = handle.emit(
                    dialect.exit_channel(),
                    serde_json::json!({
                        "code": status.code,
                        "stderr": stderr_text.trim(),
                    }),
                );
            }
            _ => (),
        }
    }
}

/// Start a link check (`obsidian-export check`). Emits `check-event` per
/// parsed JSON Lines event and a final `check-exit` with the process exit
/// code. Exit 1 covers both "broken links found" and "the check itself
/// failed"; the frontend distinguishes the two by the presence of a
/// `check-end` event.
///
/// The check shares the export's child slot: only one sidecar process may
/// run at a time, and `cancel_export` covers both kinds.
#[tauri::command]
pub async fn start_check(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    options: ExportOptions,
    target: LinkCheckTarget,
) -> Result<(), String> {
    let mut slot = state.child.lock().map_err(|_| "export state poisoned")?;
    if slot.is_some() {
        return Err("a sidecar process is already running".to_string());
    }

    let source_path = std::path::absolute(&source)
        .map_err(|err| format!("invalid source path '{source}': {err}"))?;
    let source_arg = source_path.to_string_lossy().into_owned();

    let args = build_check_args(&options, target, &source_arg);
    let (rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    *slot = Some(child);
    drop(slot);

    tauri::async_runtime::spawn(pump_sidecar(app.clone(), rx, StreamDialect::Check));

    Ok(())
}

/// Cancel the running sidecar process (an export or a link check) by
/// killing it.
#[tauri::command]
pub fn cancel_export(app: AppHandle) -> Result<bool, String> {
    match take_child(&app) {
        Some(child) => {
            child
                .kill()
                .map_err(|err| format!("failed to kill sidecar: {err}"))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Whether an export is currently running (used to restore UI state).
#[tauri::command]
pub fn export_running(app: AppHandle) -> Result<bool, String> {
    let state = app.state::<ExportState>();
    let guard = state.child.lock().map_err(|_| "export state poisoned")?;
    Ok(guard.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_root_appends_source_folder_name() {
        let source = Path::new(r"D:\vaults\我的库");
        let destination = Path::new(r"E:\out");
        let resolved = resolve_destination(source, destination, true, true);
        assert_eq!(resolved, PathBuf::from(r"E:\out\我的库"));
    }

    #[test]
    fn keep_root_off_passes_destination_through() {
        let source = Path::new(r"D:\vaults\我的库");
        let destination = Path::new(r"E:\out");
        let resolved = resolve_destination(source, destination, false, true);
        assert_eq!(resolved, PathBuf::from(r"E:\out"));
    }

    #[test]
    fn file_sources_are_never_wrapped() {
        let source = Path::new(r"D:\vaults\note.md");
        let destination = Path::new(r"E:\out");
        let resolved = resolve_destination(source, destination, true, false);
        assert_eq!(resolved, PathBuf::from(r"E:\out"));
    }

    #[test]
    fn rootless_source_falls_back_to_destination() {
        // A drive root has no file name; wrapping must not panic or produce a
        // degenerate path.
        let source = Path::new(r"D:\");
        let destination = Path::new(r"E:\out");
        let resolved = resolve_destination(source, destination, true, true);
        assert_eq!(resolved, PathBuf::from(r"E:\out"));
    }

    #[test]
    fn default_options_pass_no_extra_flags() {
        let args = build_args(&ExportOptions::default(), "SRC", "DST");
        assert_eq!(args, vec!["--progress", "json", "SRC", "DST"]);
    }

    #[test]
    fn nondefault_enum_and_bool_flags_are_passed() {
        let mut options = ExportOptions::default();
        options.frontmatter = FrontmatterChoice::Always;
        options.missing_section = MissingSectionChoice::Fail;
        options.hidden = true;
        options.no_git = true;
        options.no_recursive_embeds = true;
        options.preserve_mtime = true;
        options.fail_fast = true;
        options.hard_linebreaks = true;
        let args = build_args(&options, "SRC", "DST");
        assert_eq!(
            args,
            vec![
                "--progress",
                "json",
                "--frontmatter",
                "always",
                "--hidden",
                "--no-git",
                "--no-recursive-embeds",
                "--preserve-mtime",
                "--missing-section",
                "fail",
                "--fail-fast",
                "--hard-linebreaks",
                "SRC",
                "DST",
            ]
        );
    }

    #[test]
    fn explicit_default_enums_are_omitted() {
        // frontmatter=auto / missing-section=skip match the CLI defaults; even
        // when the frontend sends them explicitly they must not reach the argv.
        let mut options = ExportOptions::default();
        options.frontmatter = FrontmatterChoice::Auto;
        options.missing_section = MissingSectionChoice::Skip;
        let args = build_args(&options, "S", "D");
        assert!(!args.iter().any(|a| a == "--frontmatter"));
        assert!(!args.iter().any(|a| a == "--missing-section"));
    }

    #[test]
    fn value_options_and_repeated_tag_flags() {
        let mut options = ExportOptions::default();
        options.start_at = Some(r"D:\vaults\lib".to_owned());
        options.ignore_file = Some(".custom-ignore".to_owned());
        options.skip_tags = vec!["draft".to_owned(), "private".to_owned()];
        options.only_tags = vec!["published".to_owned()];
        let args = build_args(&options, "S", "D");
        assert_eq!(
            args,
            vec![
                "--progress",
                "json",
                "--start-at",
                r"D:\vaults\lib",
                "--ignore-file",
                ".custom-ignore",
                "--skip-tags",
                "draft",
                "--skip-tags",
                "private",
                "--only-tags",
                "published",
                "S",
                "D",
            ]
        );
    }

    #[test]
    fn options_deserialize_from_camelcase_frontend_payload() {
        let payload = r#"{
            "startAt": "D:/vaults/sub",
            "frontmatter": "never",
            "ignoreFile": ".ignore",
            "skipTags": ["a"],
            "onlyTags": [],
            "hidden": false,
            "noGit": true,
            "noRecursiveEmbeds": false,
            "preserveMtime": false,
            "missingSection": "embed-full",
            "failFast": false,
            "hardLinebreaks": true
        }"#;
        let options: ExportOptions =
            serde_json::from_str(payload).expect("valid frontend payload");
        assert_eq!(options.start_at.as_deref(), Some("D:/vaults/sub"));
        assert_eq!(options.frontmatter, FrontmatterChoice::Never);
        assert_eq!(options.ignore_file.as_deref(), Some(".ignore"));
        assert_eq!(options.skip_tags, vec!["a".to_owned()]);
        assert!(options.no_git);
        assert_eq!(options.missing_section, MissingSectionChoice::EmbedFull);
        assert!(options.hard_linebreaks);
    }

    #[test]
    fn empty_string_value_options_are_omitted() {
        // A blank start-at/ignore-file must be treated as unset: the CLI
        // should not receive `--start-at ""` (nonexistent-path error) or an
        // ignore-file name that can never match anything. Blank tags are
        // dropped for the same reason.
        let payload = r#"{ "startAt": "", "ignoreFile": "   ", "skipTags": ["", "  "], "onlyTags": ["\t"] }"#;
        let options: ExportOptions =
            serde_json::from_str(payload).expect("valid frontend payload");
        let args = build_args(&options, "S", "D");
        assert!(!args.iter().any(|a| a == "--start-at"));
        assert!(!args.iter().any(|a| a == "--ignore-file"));
        assert!(!args.iter().any(|a| a == "--skip-tags"));
        assert!(!args.iter().any(|a| a == "--only-tags"));
    }

    #[test]
    fn unknown_enum_value_fails_deserialization() {
        // Rejects at the invoke boundary instead of silently exporting with
        // defaults; the error is surfaced by the frontend's start catch.
        let payload = r#"{ "frontmatter": "sometimes" }"#;
        assert!(serde_json::from_str::<ExportOptions>(payload).is_err());
    }

    #[test]
    fn wrong_type_fails_deserialization() {
        let payload = r#"{ "hidden": "yes" }"#;
        assert!(serde_json::from_str::<ExportOptions>(payload).is_err());
    }

    #[test]
    fn check_args_default_only_pass_progress_and_source() {
        let args = build_check_args(&ExportOptions::default(), LinkCheckTarget::Source, "SRC");
        assert_eq!(args, vec!["check", "--progress", "json", "SRC"]);
    }

    #[test]
    fn check_args_source_target_inherits_export_filters() {
        // Checking the vault source must walk the same file set as the
        // export: every non-default filter flag is forwarded.
        let mut options = ExportOptions::default();
        options.start_at = Some(r"D:\vaults\lib".to_owned());
        options.ignore_file = Some(".custom-ignore".to_owned());
        options.hidden = true;
        options.no_git = true;
        // Export-only flags must not leak into the check argv.
        options.frontmatter = FrontmatterChoice::Always;
        options.fail_fast = true;
        let args = build_check_args(&options, LinkCheckTarget::Source, "SRC");
        assert_eq!(
            args,
            vec![
                "check",
                "--progress",
                "json",
                "--start-at",
                r"D:\vaults\lib",
                "--ignore-file",
                ".custom-ignore",
                "--hidden",
                "--no-git",
                "SRC",
            ]
        );
    }

    #[test]
    fn check_args_destination_target_never_applies_default_filters() {
        // The output tree is already filtered, but the CLI defaults are
        // themselves filters: gitignore rules would silently exclude an
        // output folder inside a git repository, and dot-files produced
        // under `--hidden` would be skipped. Vault-only filters (start-at,
        // ignore-file) stay off.
        let mut options = ExportOptions::default();
        options.start_at = Some("sub".to_owned());
        options.ignore_file = Some(".custom-ignore".to_owned());
        options.hidden = true;
        let args = build_check_args(&options, LinkCheckTarget::Destination, "OUT");
        assert_eq!(
            args,
            vec!["check", "--progress", "json", "--hidden", "--no-git", "OUT"]
        );
    }

    #[test]
    fn check_args_destination_default_options_still_pass_no_git() {
        // Even with every option at its default, checking the output must
        // not honor gitignore (the false-negative trap).
        let args = build_check_args(&ExportOptions::default(), LinkCheckTarget::Destination, "OUT");
        assert_eq!(args, vec!["check", "--progress", "json", "--no-git", "OUT"]);
    }

    #[test]
    fn link_check_preferences_deserialize_from_camelcase() {
        let payload = r#"{ "linkCheckEnabled": true, "linkCheckTarget": "destination" }"#;
        let options: ExportOptions = serde_json::from_str(payload).expect("valid payload");
        assert!(options.link_check_enabled);
        assert_eq!(options.link_check_target, LinkCheckTarget::Destination);
        // Omitted preferences keep the defaults (check off, vault source).
        let options: ExportOptions = serde_json::from_str("{}").expect("valid payload");
        assert!(!options.link_check_enabled);
        assert_eq!(options.link_check_target, LinkCheckTarget::Source);
    }
}
