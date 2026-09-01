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

use crate::events::{self, ParsedCheckLine, ParsedLine, ParsedUpdateLine};

/// Tauri event names emitted to the frontend.
pub const EVENT_SIDECAR_EVENT: &str = "sidecar-event";
pub const EVENT_SIDECAR_ERROR: &str = "sidecar-error";
pub const EVENT_SIDECAR_EXIT: &str = "sidecar-exit";
pub const EVENT_CHECK_EVENT: &str = "check-event";
pub const EVENT_CHECK_ERROR: &str = "check-error";
pub const EVENT_CHECK_EXIT: &str = "check-exit";
pub const EVENT_UPDATE_EVENT: &str = "update-event";
pub const EVENT_UPDATE_ERROR: &str = "update-error";
pub const EVENT_UPDATE_EXIT: &str = "update-exit";

/// Which sidecar flavor currently occupies the child slot; surfaces in the
/// "already running" error so the UI can tell the user *what* to wait for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OccupiedBy {
    #[default]
    None,
    Export,
    Check,
    UpdateCheck,
    UpdateDownload,
}

impl OccupiedBy {
    fn describe(self) -> &'static str {
        match self {
            Self::None => "nothing",
            Self::Export => "an export",
            Self::Check => "a link check",
            Self::UpdateCheck => "an update check",
            Self::UpdateDownload => "an update download",
        }
    }
}

/// The sidecar child slot: the running process (if any) and who claimed it.
/// Locked as one unit so that claiming is atomic with the occupancy check —
/// a claim taken before spawn must block every other claim until the child
/// is stored or the claim is rolled back.
#[derive(Default)]
struct SlotState {
    child: Option<CommandChild>,
    occupant: OccupiedBy,
}

/// Handle of a running export, if any.
#[derive(Default)]
pub struct ExportState {
    slot: Mutex<SlotState>,
}

fn take_child(app: &AppHandle) -> Option<CommandChild> {
    let state = app.state::<ExportState>();
    let mut guard = state.slot.lock().expect("export state mutex poisoned");
    // Only a stored child releases the claim: a cancel landing inside the
    // claim→spawn window must not clear a claim that hasn't been fulfilled
    // yet (the spawning start_* would then store into a slot another start
    // could have re-claimed meanwhile).
    match guard.child.take() {
        Some(child) => {
            guard.occupant = OccupiedBy::None;
            Some(child)
        }
        None => None,
    }
}

/// Claim the child slot for `who`; errors with a message naming the current
/// occupant when already taken.
///
/// The claim outlives this function's lock: between claiming and storing the
/// spawned child, `occupant` alone blocks concurrent claims (the child is
/// still `None` there). Every failure on that stretch must roll back via
/// [`release_claim`] or the slot stays claimed forever.
fn claim_slot(state: &State<'_, ExportState>, who: OccupiedBy) -> Result<(), String> {
    let mut guard = state.slot.lock().map_err(|_| "export state poisoned")?;
    if guard.child.is_some() || guard.occupant != OccupiedBy::None {
        return Err(format!(
            "cannot start: {} is already running",
            slot_description(&guard)
        ));
    }
    guard.occupant = who;
    Ok(())
}

/// Store a successfully spawned child, making the claim permanent until the
/// process terminates.
fn store_child(state: &State<'_, ExportState>, child: CommandChild) -> Result<(), String> {
    let mut guard = state.slot.lock().map_err(|_| "export state poisoned")?;
    guard.child = Some(child);
    Ok(())
}

/// Roll back a claim by `who` when the spawned child was never stored.
/// Guarded on the claimant so a late rollback cannot clobber a successor
/// that has already re-claimed the idle slot.
fn release_claim(state: &State<'_, ExportState>, who: OccupiedBy) {
    if let Ok(mut guard) = state.slot.lock() {
        if guard.child.is_none() && guard.occupant == who {
            guard.occupant = OccupiedBy::None;
        }
    }
}

fn slot_description(slot: &SlotState) -> String {
    if slot.occupant == OccupiedBy::None {
        "another sidecar process".to_owned()
    } else {
        slot.occupant.describe().to_owned()
    }
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

/// Output format for rendered diagrams; mirrors the CLI `--diagram-format`
/// enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagramFormatChoice {
    #[default]
    Svg,
    Png,
}

impl DiagramFormatChoice {
    fn as_flag(self) -> &'static str {
        match self {
            DiagramFormatChoice::Svg => "svg",
            DiagramFormatChoice::Png => "png",
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
    /// GUI-only preference: run the link checker after a successful export.
    /// Never part of `build_args`; the frontend orchestrates the follow-up
    /// `start_check` invocation.
    pub link_check_enabled: bool,
    /// GUI-only preference: which tree that automatic check walks.
    pub link_check_target: LinkCheckTarget,
    /// Diagram renderers to enable (a subset of dot/mermaid/wavedrom/tikz);
    /// empty keeps the CLI default of no diagram rendering.
    pub diagram_renderers: Vec<String>,
    /// Output format for rendered diagrams (svg default; renderers without
    /// raster output fall back to svg with a warning).
    pub diagram_format: DiagramFormatChoice,
    /// Explicit executable paths overriding the PATH lookup, keyed by tool
    /// name (dot/mmdc/wavedrom/latex/dvisvgm). Blank values are treated as
    /// unset — a GUI-side convenience the CLI does not share: it rejects an
    /// empty `--diagram-bin` path as a usage error instead of ignoring it.
    pub diagram_bins: std::collections::BTreeMap<String, String>,
}

/// Build the sidecar argv. `--progress json` is always passed (the desktop
/// app consumes the JSON Lines event stream); user options are only passed
/// when they deviate from the CLI defaults.
fn build_args(options: &ExportOptions, source: &str, target: &str) -> Vec<String> {
    let mut args = vec!["--progress".to_owned(), "json".to_owned()];
    // Blank strings count as unset: the frontend already maps them to null,
    // but the invoke boundary must not rely on that.
    if let Some(start_at) = options.start_at.as_deref().filter(|s| !s.trim().is_empty()) {
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
    if !options.diagram_renderers.is_empty() {
        args.push("--render-diagrams".to_owned());
        args.push(options.diagram_renderers.join(","));
    }
    if options.diagram_format != DiagramFormatChoice::Svg {
        args.extend([
            "--diagram-format".to_owned(),
            options.diagram_format.as_flag().to_owned(),
        ]);
    }
    for (tool, path) in &options.diagram_bins {
        // Blank strings count as unset, same rule as the scalar string
        // options above.
        if !path.trim().is_empty() {
            args.extend([
                "--diagram-bin".to_owned(),
                format!("{tool}={path}"),
            ]);
        }
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
fn build_check_args(options: &ExportOptions, target: LinkCheckTarget, source: &str) -> Vec<String> {
    let mut args = vec![
        "check".to_owned(),
        "--progress".to_owned(),
        "json".to_owned(),
    ];
    match target {
        LinkCheckTarget::Source => {
            if let Some(start_at) = options.start_at.as_deref().filter(|s| !s.trim().is_empty()) {
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
/// Resolve inputs and spawn the export sidecar. Every `?` in here runs
/// under a live claim; the caller rolls the claim back on any error.
#[allow(clippy::type_complexity)]
fn spawn_export_sidecar(
    app: &AppHandle,
    source: &str,
    destination: &str,
    options: &ExportOptions,
    keep_root_folder: bool,
) -> Result<
    (
        tauri::async_runtime::Receiver<CommandEvent>,
        CommandChild,
        String,
    ),
    String,
> {
    // Resolve picked paths against the GUI process's working directory up
    // front, so the sidecar contract holds even for manually typed relative
    // paths: the events echo `path` back in the same absolute shape they
    // were given (docs/sidecar-events.md).
    let source_path = std::path::absolute(source)
        .map_err(|err| format!("invalid source path '{source}': {err}"))?;
    let destination_path = std::path::absolute(destination)
        .map_err(|err| format!("invalid destination path '{destination}': {err}"))?;
    let target = resolve_destination(
        &source_path,
        &destination_path,
        keep_root_folder,
        source_path.is_dir(),
    );
    // The user-picked destination always exists; only the appended subfolder
    // needs creating. A mistyped manual destination is left for the CLI to
    // report rather than being silently created here.
    if target != destination_path {
        std::fs::create_dir_all(&target)
            .map_err(|err| format!("failed to create destination '{}': {err}", target.display()))?;
    }
    let source_arg = source_path.to_string_lossy().into_owned();
    let target_arg = target.to_string_lossy().into_owned();

    let args = build_args(options, &source_arg, &target_arg);
    let (rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    Ok((rx, child, target_arg))
}

#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    destination: String,
    options: ExportOptions,
    keep_root_folder: Option<bool>,
) -> Result<String, String> {
    claim_slot(&state, OccupiedBy::Export)?;

    let spawned = spawn_export_sidecar(
        &app,
        &source,
        &destination,
        &options,
        keep_root_folder.unwrap_or(false),
    );
    let (rx, child, target_arg) = match spawned {
        Ok(spawned) => spawned,
        Err(err) => {
            release_claim(&state, OccupiedBy::Export);
            return Err(err);
        }
    };
    store_child(&state, child)?;

    tauri::async_runtime::spawn(pump_sidecar(app.clone(), rx, StreamDialect::Export));

    Ok(target_arg)
}

/// Which JSON Lines dialect a spawned sidecar speaks; decides the parse
/// function and the frontend channels used by [`pump_sidecar`].
#[derive(Clone, Copy)]
enum StreamDialect {
    Export,
    Check,
    Update,
}

impl StreamDialect {
    fn event_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_EVENT,
            Self::Check => EVENT_CHECK_EVENT,
            Self::Update => EVENT_UPDATE_EVENT,
        }
    }

    fn exit_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_EXIT,
            Self::Check => EVENT_CHECK_EXIT,
            Self::Update => EVENT_UPDATE_EXIT,
        }
    }

    /// Parse errors and sidecar IO errors go to the dialect's own channel so
    /// the frontend can surface them next to the stream they belong to
    /// (check runs while the export log view is gone).
    fn error_channel(self) -> &'static str {
        match self {
            Self::Export => EVENT_SIDECAR_ERROR,
            Self::Check => EVENT_CHECK_ERROR,
            Self::Update => EVENT_UPDATE_ERROR,
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
            Self::Update => match events::parse_update_line(line) {
                Ok(ParsedUpdateLine::Event(event)) => Ok(Some(event_value(&event))),
                Ok(ParsedUpdateLine::Ignored) => Ok(None),
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
                let _ = handle.emit(dialect.error_channel(), format!("sidecar IO error: {err}"));
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
/// Resolve the source and spawn the check sidecar. Runs under a live
/// claim; the caller rolls the claim back on any error.
fn spawn_check_sidecar(
    app: &AppHandle,
    source: &str,
    options: &ExportOptions,
    target: LinkCheckTarget,
) -> Result<(tauri::async_runtime::Receiver<CommandEvent>, CommandChild), String> {
    let source_path = std::path::absolute(source)
        .map_err(|err| format!("invalid source path '{source}': {err}"))?;
    let source_arg = source_path.to_string_lossy().into_owned();

    let args = build_check_args(options, target, &source_arg);
    let (rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    Ok((rx, child))
}

#[tauri::command]
pub async fn start_check(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    options: ExportOptions,
    target: LinkCheckTarget,
) -> Result<(), String> {
    claim_slot(&state, OccupiedBy::Check)?;

    let spawned = spawn_check_sidecar(&app, &source, &options, target);
    let (rx, child) = match spawned {
        Ok(spawned) => spawned,
        Err(err) => {
            release_claim(&state, OccupiedBy::Check);
            return Err(err);
        }
    };
    store_child(&state, child)?;

    tauri::async_runtime::spawn(pump_sidecar(app.clone(), rx, StreamDialect::Check));

    Ok(())
}

/// Cancel the running sidecar process (an export, a link check, or an update
/// download) by killing it.
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
    let guard = state.slot.lock().map_err(|_| "export state poisoned")?;
    Ok(guard.child.is_some())
}

// ---- 更新检查 / 下载 / 安装（边车 update 子命令的编排） ---------------------

/// Which update action the sidecar should perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateAction {
    /// `obsidian-export update --asset desktop`：仅检测，报告最新 release。
    Check,
    /// `… --download --output <downloads dir>`：检测并把 NSIS 安装包下载
    /// 到系统临时目录。
    Download,
}

/// Installer download directory: `<temp>/obsidian-export/Downloads`. The
/// temp-dir semantics mean one-shot installers get reclaimed by the OS and
/// a lost file is simply re-downloaded.
pub fn update_downloads_dir() -> PathBuf {
    std::env::temp_dir()
        .join("obsidian-export")
        .join("Downloads")
}

/// Arguments for the sidecar's `update` subcommand. The desktop always
/// selects the `desktop` asset target (NSIS setup exe); the CLI picks its
/// own platform archive when run manually.
fn build_update_args(action: UpdateAction, output_dir: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "update".to_owned(),
        "--progress".to_owned(),
        "json".to_owned(),
        "--asset".to_owned(),
        "desktop".to_owned(),
    ];
    if let UpdateAction::Download = action {
        args.push("--download".to_owned());
        if let Some(dir) = output_dir {
            args.extend(["--output".to_owned(), dir.to_owned()]);
        }
    }
    args
}

/// Start an update action (`obsidian-export update`). Emits `update-event`
/// per parsed JSON Lines event and a final `update-exit` with the process
/// exit code. Exit 1 means the check/download itself failed (a *found*
/// update is still exit 0 — the frontend reads the verdict from the
/// `update-result` event, not the exit code).
///
/// The download action returns the absolute directory the installer will be
/// saved into (created here; the CLI requires it to exist).
///
/// Shares the export's child slot: only one sidecar process may run at a
/// time, and `cancel_export` covers all kinds.
/// Prepare the download directory and spawn the update sidecar. Runs under
/// a live claim; the caller rolls the claim back on any error.
fn spawn_update_sidecar(
    app: &AppHandle,
    action: UpdateAction,
) -> Result<
    (
        tauri::async_runtime::Receiver<CommandEvent>,
        CommandChild,
        String,
    ),
    String,
> {
    let output_dir = match action {
        UpdateAction::Download => {
            let dir = update_downloads_dir();
            // 纵深防御：%TEMP% 是用户态任意进程可写区，目录链上若有指
            // 向他处的 symlink/junction（junction 无需特权且 is_symlink
            // 探不到），写入会跟随逃逸。创建后以 canonicalize 复核：
            // 规范化路径必须仍位于系统临时目录之下，否则拒绝。
            std::fs::create_dir_all(&dir).map_err(|err| {
                format!(
                    "failed to create download directory '{}': {err}",
                    dir.display()
                )
            })?;
            let canonical = std::fs::canonicalize(&dir).map_err(|err| {
                format!(
                    "failed to resolve download directory '{}': {err}",
                    dir.display()
                )
            })?;
            let temp_root = std::fs::canonicalize(std::env::temp_dir())
                .map_err(|err| format!("failed to resolve the system temp directory: {err}"))?;
            if !canonical.starts_with(&temp_root) {
                return Err(format!(
                    "refusing to use download directory (it escapes the temp root): {}",
                    dir.display()
                ));
            }
            // 传给边车的是原（非 `\\?\` 规范化）形态：download-end 回报的
            // path 与 run_installer 的 parent 比对都按此形态进行。
            dir.to_string_lossy().into_owned()
        }
        UpdateAction::Check => String::new(),
    };

    let args = build_update_args(
        action,
        if output_dir.is_empty() {
            None
        } else {
            Some(&output_dir)
        },
    );
    let (rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args(&args)
        .spawn()
        .map_err(|err| format!("failed to spawn sidecar: {err}"))?;
    Ok((rx, child, output_dir))
}

#[tauri::command]
pub async fn start_update(
    app: AppHandle,
    state: State<'_, ExportState>,
    action: UpdateAction,
) -> Result<String, String> {
    let who = match action {
        UpdateAction::Check => OccupiedBy::UpdateCheck,
        UpdateAction::Download => OccupiedBy::UpdateDownload,
    };
    claim_slot(&state, who)?;

    let spawned = spawn_update_sidecar(&app, action);
    let (rx, child, output_dir) = match spawned {
        Ok(spawned) => spawned,
        Err(err) => {
            release_claim(&state, who);
            return Err(err);
        }
    };
    store_child(&state, child)?;

    tauri::async_runtime::spawn(pump_sidecar(app.clone(), rx, StreamDialect::Update));

    Ok(output_dir)
}

/// Path defense for [`run_installer`]: a plain `.exe` directly inside
/// [`update_downloads_dir`], nothing else. The CLI already rejects
/// path-shaped asset names before saving; this re-checks the value the
/// frontend echoes back before it is ever executed.
fn validate_installer_path(path: &Path) -> bool {
    path.parent() == Some(update_downloads_dir().as_path())
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

/// Run a downloaded installer and exit the app.
///
/// The NSIS wizard handles its own UAC elevation; the app exits right
/// after spawning it because the installer cannot overwrite the app's own
/// files while they are locked by a running process.
#[tauri::command]
pub fn run_installer(app: AppHandle, path: String) -> Result<(), String> {
    let installer = PathBuf::from(&path);
    if !validate_installer_path(&installer) {
        return Err(format!(
            "refusing to run installer outside the download directory: {path}"
        ));
    }
    if !installer.is_file() {
        return Err(format!("installer file is missing: {path}"));
    }
    std::process::Command::new(&installer)
        .spawn()
        .map_err(|err| format!("failed to launch installer: {err}"))?;
    // The response may not reach the frontend before the process is gone;
    // that's fine — exiting is the point.
    app.exit(0);
    Ok(())
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
    fn diagram_options_map_to_flags() {
        let options = ExportOptions {
            diagram_renderers: vec!["dot".into(), "mermaid".into()],
            diagram_format: DiagramFormatChoice::Png,
            diagram_bins: std::collections::BTreeMap::from([
                ("mmdc".into(), r"C:\Tools\mmdc.cmd".into()),
                // Blank paths are unset and must not reach the CLI.
                ("latex".into(), "  ".into()),
            ]),
            ..ExportOptions::default()
        };
        let args = build_args(&options, "SRC", "DST");
        assert_eq!(
            args,
            vec![
                "--progress",
                "json",
                "--render-diagrams",
                "dot,mermaid",
                "--diagram-format",
                "png",
                "--diagram-bin",
                "mmdc=C:\\Tools\\mmdc.cmd",
                "SRC",
                "DST",
            ]
        );
    }

    #[test]
    fn diagram_defaults_pass_no_flags() {
        // Explicit defaults (svg format, empty renderer list) stay silent,
        // keeping the CLI the single source of default behavior.
        let options = ExportOptions {
            diagram_format: DiagramFormatChoice::Svg,
            ..ExportOptions::default()
        };
        let args = build_args(&options, "SRC", "DST");
        assert_eq!(args, vec!["--progress", "json", "SRC", "DST"]);
    }

    #[test]
    fn nondefault_enum_and_bool_flags_are_passed() {
        let options = ExportOptions {
            frontmatter: FrontmatterChoice::Always,
            missing_section: MissingSectionChoice::Fail,
            hidden: true,
            no_git: true,
            no_recursive_embeds: true,
            preserve_mtime: true,
            fail_fast: true,
            hard_linebreaks: true,
            ..ExportOptions::default()
        };
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
        let options = ExportOptions::default();
        let args = build_args(&options, "S", "D");
        assert!(!args.iter().any(|a| a == "--frontmatter"));
        assert!(!args.iter().any(|a| a == "--missing-section"));
    }

    #[test]
    fn value_options_and_repeated_tag_flags() {
        let options = ExportOptions {
            start_at: Some(r"D:\vaults\lib".to_owned()),
            ignore_file: Some(".custom-ignore".to_owned()),
            skip_tags: vec!["draft".to_owned(), "private".to_owned()],
            only_tags: vec!["published".to_owned()],
            ..ExportOptions::default()
        };
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
        let options: ExportOptions = serde_json::from_str(payload).expect("valid frontend payload");
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
        let payload =
            r#"{ "startAt": "", "ignoreFile": "   ", "skipTags": ["", "  "], "onlyTags": ["\t"] }"#;
        let options: ExportOptions = serde_json::from_str(payload).expect("valid frontend payload");
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
        let options = ExportOptions {
            start_at: Some(r"D:\vaults\lib".to_owned()),
            ignore_file: Some(".custom-ignore".to_owned()),
            hidden: true,
            no_git: true,
            frontmatter: FrontmatterChoice::Always,
            fail_fast: true,
            ..ExportOptions::default()
        };
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
        let options = ExportOptions {
            start_at: Some("sub".to_owned()),
            ignore_file: Some(".custom-ignore".to_owned()),
            hidden: true,
            ..ExportOptions::default()
        };
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
        let args = build_check_args(
            &ExportOptions::default(),
            LinkCheckTarget::Destination,
            "OUT",
        );
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

    // ---- update 编排 ----

    #[test]
    fn build_update_args_check_and_download() {
        assert_eq!(
            build_update_args(UpdateAction::Check, None),
            vec![
                "update".to_owned(),
                "--progress".to_owned(),
                "json".to_owned(),
                "--asset".to_owned(),
                "desktop".to_owned(),
            ]
        );
        assert_eq!(
            build_update_args(UpdateAction::Download, Some(r"C:\tmp\dl")),
            vec![
                "update".to_owned(),
                "--progress".to_owned(),
                "json".to_owned(),
                "--asset".to_owned(),
                "desktop".to_owned(),
                "--download".to_owned(),
                "--output".to_owned(),
                r"C:\tmp\dl".to_owned(),
            ]
        );
    }

    #[test]
    fn downloads_dir_is_temp_scoped() {
        assert_eq!(
            update_downloads_dir(),
            std::env::temp_dir()
                .join("obsidian-export")
                .join("Downloads")
        );
    }

    #[test]
    fn validate_installer_path_contract() {
        let dir = update_downloads_dir();
        assert!(validate_installer_path(
            &dir.join("Obsidian.Export_26.9.0_x64-setup.exe")
        ));
        assert!(
            validate_installer_path(&dir.join("setup.EXE")),
            "扩展名大小写不敏感"
        );
        assert!(!validate_installer_path(&dir.join("app.msi")), "非 exe");
        assert!(
            !validate_installer_path(&dir.join("payload.zip")),
            "zip 不是安装器"
        );
        assert!(
            !validate_installer_path(&dir.join("sub").join("setup.exe")),
            "嵌套子目录不放行"
        );
        assert!(
            !validate_installer_path(&dir.join("..").join("evil.exe")),
            "越出下载目录不放行"
        );
        assert!(
            !validate_installer_path(Path::new(r"C:\Windows\notepad.exe")),
            "任意路径不放行"
        );
    }
}
