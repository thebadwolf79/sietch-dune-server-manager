use dune_manager_core::orchestration::{
    is_started_state, BattlegroupManagementOrchestrator, BattlegroupRef, RusshRunner,
    StructuredKubectl, VendorBattlegroupWrapper,
};

use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::commands::status_data::read_remote_server_status;
use crate::dto::{RemoteServerActionRequest, RemoteServerStatus};
use crate::logging::TauriOperationSink;

type Manager = BattlegroupManagementOrchestrator<
    StructuredKubectl<RusshRunner>,
    VendorBattlegroupWrapper<RusshRunner>,
>;

fn manager_from_runner(runner: &RusshRunner) -> Manager {
    let kubernetes = StructuredKubectl::new(runner.clone());
    let wrapper = VendorBattlegroupWrapper::new(runner.clone());
    BattlegroupManagementOrchestrator::new(kubernetes, wrapper)
}

#[tauri::command]
pub async fn start_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    run_remote_battlegroup_action(app, request, false).await
}

#[tauri::command]
pub async fn stop_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    run_remote_battlegroup_action(app, request, true).await
}

#[tauri::command]
pub async fn restart_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(worker_app);
        sink.info("bg.restart", "Restarting remote battlegroup.");
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        let battlegroup = BattlegroupRef {
            namespace: request.namespace,
            name: request.battlegroup_name,
        };
        let manager = manager_from_runner(&runner);
        manager
            .restart_and_wait_director(&battlegroup, 240, &mut sink)
            .map_err(command_error_message)?;
        sink.info("bg.restart", "Refreshing battlegroup state.");
        read_remote_server_status(&runner, &battlegroup.namespace, &battlegroup.name)
            .map_err(command_error_message)
    })
    .await
    .map_err(|err| format!("Remote battlegroup restart worker failed: {err}"))?
}

#[tauri::command]
pub async fn update_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(worker_app);
        sink.info("bg.update", "Running vendor wrapper update.");
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        run_battlegroup_update_with_runner(
            &runner,
            &mut sink,
            request.namespace,
            request.battlegroup_name,
        )
    })
    .await
    .map_err(|err| format!("Remote battlegroup update worker failed: {err}"))?
}

pub async fn run_remote_battlegroup_action(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink::new(worker_app);
        sink.info("bg.check", "Checking remote battlegroup state.");
        let runner = runner_for_remote_kind(
            request.server_type.as_deref(),
            request.host,
            request.user,
            request.key_path,
        )?;
        run_battlegroup_action_with_runner(
            &runner,
            &mut sink,
            request.namespace,
            request.battlegroup_name,
            stop,
        )
    })
    .await
    .map_err(|err| format!("Remote battlegroup action worker failed: {err}"))?
}

fn run_battlegroup_action_with_runner(
    runner: &RusshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let manager = manager_from_runner(runner);
    let before = manager
        .status(&battlegroup)
        .map_err(command_error_message)?;
    let before_started = is_started_state(&before);
    if stop && !before_started {
        return Err(format!(
            "Battlegroup is not running (status={}, stop={}, database={}, gateway={}, director={}).",
            before.phase,
            before.stop,
            before.database_phase,
            before.server_group_phase,
            before.director_phase
        ));
    }
    if !stop && before_started {
        return Err("Battlegroup is already started.".to_string());
    }
    if stop {
        manager
            .stop(&battlegroup, sink)
            .map_err(command_error_message)?;
    } else {
        manager
            .start_and_wait_director(&battlegroup, 180, sink)
            .map_err(command_error_message)?;
    }
    sink.info("bg.check", "Refreshing battlegroup state.");
    read_remote_server_status(runner, &battlegroup.namespace, &battlegroup.name)
        .map_err(command_error_message)
}

fn run_battlegroup_update_with_runner(
    runner: &RusshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
) -> Result<RemoteServerStatus, String> {
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let manager = manager_from_runner(runner);
    sink.warn(
        "bg.update",
        "Running vendor `battlegroup update` (steamcmd + operators + maps + images).",
    );
    let stdout = manager
        .update(&battlegroup, sink)
        .map_err(command_error_message)?;
    if !stdout.trim().is_empty() {
        sink.info("bg.update", stdout.trim().to_string());
    }
    sink.info("bg.update", "Refreshing battlegroup state.");
    read_remote_server_status(runner, &battlegroup.namespace, &battlegroup.name)
        .map_err(command_error_message)
}
