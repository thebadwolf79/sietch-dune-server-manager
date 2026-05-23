mod commands;
mod dto;
mod logging;
mod state;

use tauri::Manager;

use crate::commands::{
    detect_remote_ubuntu_servers, remote_component_log_tail, remote_server_components,
    remote_server_status, restart_remote_component, server_tunnel_status, start_remote_battlegroup,
    start_server_tunnel, stop_all_tunnels, stop_remote_battlegroup, stop_server_tunnel,
    update_remote_battlegroup,
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
            update_remote_battlegroup,
            detect_remote_ubuntu_servers,
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.state::<TunnelRegistry>().stop_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
