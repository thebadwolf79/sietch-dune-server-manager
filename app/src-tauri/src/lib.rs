mod commands;
mod dto;
mod log_file;
mod logging;
mod state;

use std::sync::Arc;

use tauri::Manager;

use crate::log_file::LogFile;

use crate::commands::{
    check_remote_sudo, detect_remote_ubuntu_servers, get_logs_folder, install_management_service,
    management_service_bundled_version, management_service_status, ms_cluster, ms_cron_preview,
    ms_dump_prune_execute, ms_dump_prune_preview, ms_get_config, ms_health, ms_history,
    ms_list_commands, ms_list_logs, ms_list_runs, ms_list_timezones, ms_player_location,
    ms_publish, ms_search_items, ms_search_journey_nodes, ms_search_players,
    ms_search_skill_modules, ms_search_vehicles, ms_search_xp_event_tags, ms_set_config,
    ms_trigger_run, ms_welcome_grant_retry, ms_welcome_grants, ms_welcome_whisper,
    record_operation_log,
    remote_component_log_tail, remote_server_components, remote_server_status,
    restart_management_service, restart_remote_battlegroup, restart_remote_component,
    server_tunnel_status, start_custom_tunnel, start_remote_battlegroup, start_server_tunnel,
    stop_all_tunnels, stop_remote_battlegroup, stop_server_tunnel, uninstall_management_service,
    update_remote_battlegroup,
};
use crate::state::TunnelRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebKitGTK 4.1 (Fedora 40+, WebKit 2.44+) aborts under GNOME Wayland
    // with "Error 71 dispatching to Wayland display" when the DMABuf
    // renderer is active. Disable it unless the user opted in explicitly.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

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
            start_custom_tunnel,
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
            install_management_service,
            uninstall_management_service,
            management_service_status,
            management_service_bundled_version,
            restart_management_service,
            ms_get_config,
            ms_set_config,
            ms_list_timezones,
            ms_cron_preview,
            ms_dump_prune_preview,
            ms_dump_prune_execute,
            ms_player_location,
            ms_health,
            ms_list_runs,
            ms_list_logs,
            ms_trigger_run,
            ms_list_commands,
            ms_search_items,
            ms_search_vehicles,
            ms_search_players,
            ms_search_skill_modules,
            ms_search_journey_nodes,
            ms_search_xp_event_tags,
            ms_cluster,
            ms_history,
            ms_welcome_grants,
            ms_welcome_grant_retry,
            ms_welcome_whisper,
            ms_publish,
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.state::<TunnelRegistry>().stop_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}
