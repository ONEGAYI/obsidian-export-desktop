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

use crate::events::{self, ParsedLine};

/// Tauri event names emitted to the frontend.
pub const EVENT_SIDECAR_EVENT: &str = "sidecar-event";
pub const EVENT_SIDECAR_ERROR: &str = "sidecar-error";
pub const EVENT_SIDECAR_EXIT: &str = "sidecar-exit";

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
#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    destination: String,
    options: ExportOptions,
    keep_root_folder: Option<bool>,
) -> Result<(), String> {
    let mut slot = state.child.lock().map_err(|_| "export state poisoned")?;
    if slot.is_some() {
        return Err("an export is already running".to_string());
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
    let (mut rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    *slot = Some(child);
    drop(slot);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut stdout_buffer = String::new();
        let mut stderr_text = String::new();
        while let Some(message) = rx.recv().await {
            match message {
                CommandEvent::Stdout(bytes) => {
                    stdout_buffer.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = stdout_buffer.find('\n') {
                        let line: String = stdout_buffer.drain(..=pos).collect();
                        match events::parse_line(&line) {
                            Ok(ParsedLine::Event(event)) => {
                                let _ = handle.emit(EVENT_SIDECAR_EVENT, &event);
                            }
                            Ok(ParsedLine::Ignored) => (),
                            Err(err) => {
                                let _ = handle.emit(EVENT_SIDECAR_ERROR, err);
                            }
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => {
                    stderr_text.push_str(&String::from_utf8_lossy(&bytes));
                }
                CommandEvent::Error(err) => {
                    let _ = handle.emit(EVENT_SIDECAR_ERROR, format!("sidecar IO error: {err}"));
                }
                CommandEvent::Terminated(status) => {
                    // The child handle is dead now; release it so a new export
                    // can start. Reaching termination without an `end` event
                    // means the run never reached processing (see the contract).
                    take_child(&handle);
                    let _ = handle.emit(
                        EVENT_SIDECAR_EXIT,
                        serde_json::json!({
                            "code": status.code,
                            "stderr": stderr_text.trim(),
                        }),
                    );
                }
                _ => (),
            }
        }
    });

    Ok(())
}

/// Cancel a running export by killing the sidecar process.
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
}
