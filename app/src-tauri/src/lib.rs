mod commands;
mod dto;
mod log_file;
mod logging;
mod state;

use std::sync::Arc;

use tauri::Manager;

use crate::log_file::LogFile;

use crate::commands::{
    check_remote_sudo, detect_remote_ubuntu_servers, get_logs_folder, record_operation_log,
    remote_component_log_tail, remote_server_components, remote_server_status,
    restart_remote_battlegroup, restart_remote_component, server_tunnel_status,
    start_remote_battlegroup, start_server_tunnel, stop_all_tunnels, stop_remote_battlegroup,
    stop_server_tunnel, update_remote_battlegroup,
};
use crate::state::TunnelRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(TunnelRegistry::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            match LogFile::new(&app.handle()) {
                Ok(file) => {
                    app.manage(Arc::new(file));
                }
                Err(err) => {
                    eprintln!("Failed to initialize operation log file: {err}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            remote_server_status,
            remote_server_components,
            start_server_tunnel,
            stop_server_tunnel,
            server_tunnel_status,
            stop_all_tunnels,
            remote_component_log_tail,
            restart_remote_component,
            start_remote_battlegroup,
            stop_remote_battlegroup,
            restart_remote_battlegroup,
            update_remote_battlegroup,
            detect_remote_ubuntu_servers,
            check_remote_sudo,
            record_operation_log,
            get_logs_folder,
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.state::<TunnelRegistry>().stop_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
