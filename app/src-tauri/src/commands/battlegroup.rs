use dune_manager_core::models::CommandResult;
use dune_manager_core::orchestration::{
    is_started_state, BattlegroupManagementOrchestrator, BattlegroupRef,
    BattlegroupUpdateOrchestrator, KubernetesProvider, OpenSshRunner, SshGuestBootstrapProvider,
    StructuredKubectl, UbuntuSshPrepareRequest, UbuntuSshSetup,
};

use crate::commands::shared::{command_error_message, runner_for_remote_kind};
use crate::commands::status_data::read_remote_server_status;
use crate::dto::{RemoteServerActionRequest, RemoteServerStatus};
use crate::logging::TauriOperationSink;

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
pub async fn update_remote_battlegroup(
    app: tauri::AppHandle,
    request: RemoteServerActionRequest,
) -> Result<RemoteServerStatus, String> {
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut sink = TauriOperationSink { app: worker_app };
        sink.info("bg.update", "Checking remote battlegroup update.");
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
        let mut sink = TauriOperationSink { app: worker_app };
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
    runner: &OpenSshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
    stop: bool,
) -> Result<RemoteServerStatus, String> {
    let kubernetes = StructuredKubectl::new(runner.clone());
    let before = kubernetes
        .battlegroup_state(&namespace, &battlegroup_name)
        .map_err(command_error_message)?;
    let before_started = is_started_state(&before);
    if stop && !before_started {
        return Err(format!(
            "Battlegroup is not running (phase={}, stop={}, serverGroup={}, director={}).",
            before.phase, before.stop, before.server_group_phase, before.director_phase
        ));
    }
    if !stop && before_started {
        return Err("Battlegroup is already started.".to_string());
    }
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let manager = BattlegroupManagementOrchestrator::new(kubernetes);
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

fn wait_for_battlegroup_fully_stopped(
    kubernetes: &StructuredKubectl<OpenSshRunner>,
    battlegroup: &BattlegroupRef,
    timeout_seconds: u64,
    sink: &mut TauriOperationSink,
) -> CommandResult<()> {
    sink.info("bg.update", "Verifying BattleGroup is fully stopped.");
    let mut elapsed = 0;
    let mut last = None;
    while elapsed <= timeout_seconds {
        let state = kubernetes.battlegroup_state(&battlegroup.namespace, &battlegroup.name)?;
        if is_fully_stopped_state(&state) {
            return Ok(());
        }
        last = Some(state);
        std::thread::sleep(std::time::Duration::from_secs(5));
        elapsed += 5;
    }
    let detail = last
        .map(|state| {
            format!(
                "last phase={}, stop={}, serverGroup={}, director={}",
                state.phase, state.stop, state.server_group_phase, state.director_phase
            )
        })
        .unwrap_or_else(|| "no BattleGroup state was read".to_string());
    Err(dune_manager_core::errors::failure(format!(
        "BattleGroup did not fully stop within {timeout_seconds}s ({detail})"
    )))
}

fn is_fully_stopped_state(state: &dune_manager_core::orchestration::BattlegroupState) -> bool {
    state.stop
        && stoppedish_phase(&state.phase)
        && stoppedish_phase(&state.server_group_phase)
        && !director_running_phase(&state.director_phase)
}

fn stoppedish_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "stopped" | "suspended" | "notready" | "not_ready" | "unknown"
        )
}

fn director_running_phase(phase: &str) -> bool {
    let normalized = phase.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "running" | "ready" | "healthy" | "available" | "reconciling"
    )
}

fn run_battlegroup_update_with_runner(
    runner: &OpenSshRunner,
    sink: &mut TauriOperationSink,
    namespace: String,
    battlegroup_name: String,
) -> Result<RemoteServerStatus, String> {
    let battlegroup = BattlegroupRef {
        namespace,
        name: battlegroup_name,
    };
    let kubernetes = StructuredKubectl::new(runner.clone());
    let manager = BattlegroupManagementOrchestrator::new(kubernetes);
    sink.warn(
        "bg.update",
        "Stopping BattleGroup before applying the server update.",
    );
    manager
        .stop(&battlegroup, sink)
        .map_err(command_error_message)?;
    let verifier = StructuredKubectl::new(runner.clone());
    wait_for_battlegroup_fully_stopped(&verifier, &battlegroup, 600, sink)
        .map_err(command_error_message)?;
    let provider = SshGuestBootstrapProvider::new(runner.clone());
    let ubuntu = UbuntuSshSetup::new(runner.clone());
    let prepare = UbuntuSshPrepareRequest::default();
    ubuntu
        .install_server_payload(&prepare, sink)
        .map_err(command_error_message)?;
    BattlegroupUpdateOrchestrator::new(provider)
        .update_from_downloads(&battlegroup, sink)
        .map_err(command_error_message)?;
    sink.warn("bg.update", "Starting BattleGroup after update.");
    manager
        .start_and_wait_director(&battlegroup, 600, sink)
        .map_err(command_error_message)?;
    sink.info("bg.update", "Refreshing battlegroup state.");
    read_remote_server_status(runner, &battlegroup.namespace, &battlegroup.name)
        .map_err(command_error_message)
}
