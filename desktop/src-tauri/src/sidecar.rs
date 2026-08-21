//! Orchestration of the obsidian-export CLI sidecar.
//!
//! The desktop app never implements any conversion logic itself: it spawns the
//! bundled CLI with `--progress json`, forwards the parsed event stream to the
//! frontend, and maps process exit onto UI state (see docs/sidecar-events.md).

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
#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, ExportState>,
    source: String,
    destination: String,
    missing_section: String,
) -> Result<(), String> {
    let mut slot = state.child.lock().map_err(|_| "export state poisoned")?;
    if slot.is_some() {
        return Err("an export is already running".to_string());
    }

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
            &destination,
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
