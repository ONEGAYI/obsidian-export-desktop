mod events;
mod sidecar;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(sidecar::ExportState::default())
        .invoke_handler(tauri::generate_handler![
            sidecar::check_sidecar,
            sidecar::start_export,
            sidecar::cancel_export,
            sidecar::export_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
