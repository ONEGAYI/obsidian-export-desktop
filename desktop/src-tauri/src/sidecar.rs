//! Orchestration of the obsidian-export CLI sidecar.
//!
//! The desktop app never implements any conversion logic itself: it spawns the
//! bundled CLI with `--progress json`, forwards the parsed event stream to the
//! frontend, and maps process exit onto UI state (see docs/sidecar-events.md).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
/// `keep_root_folder` routes a directory source into
/// `<destination>/<source folder name>` (created if missing) so the vault's
/// first-level entries don't scatter directly in the destination.
#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    destination: String,
    missing_section: String,
    keep_root_folder: Option<bool>,
) -> Result<(), String> {
    let mut slot = state.child.lock().map_err(|_| "export state poisoned")?;
    if slot.is_some() {
        return Err("an export is already running".to_string());
    }

    let source_path = PathBuf::from(&source);
    let destination_path = PathBuf::from(&destination);
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
    let target = target.to_string_lossy().into_owned();

    let (mut rx, child) = app
        .shell()
        .sidecar("obsidian-export")
        .map_err(|err| format!("sidecar binary not found: {err}"))?
        .args([
            "--progress",
            "json",
            "--missing-section",
            &missing_section,
            &source,
            &target,
        ])
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
        // A drive root has no file name; wrapping must not panic or produce
        // a degenerate path.
        let source = Path::new(r"D:\");
        let destination = Path::new(r"E:\out");
        let resolved = resolve_destination(source, destination, true, true);
        assert_eq!(resolved, PathBuf::from(r"E:\out"));
    }
}
