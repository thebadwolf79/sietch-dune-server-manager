use serde_json::json;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        parse_single_json_document, OperationSink, OrchestrationEvent, ProviderKind,
        RemoteCommandRunner, StepAction, StepDomain,
    },
};

use super::kubernetes_bootstrap::bootstrap_kubernetes_script;
use super::kubernetes_scripts::{K3S_INSTALL_SCRIPT, PREFLIGHT_SCRIPT};
use super::models::{
    sh_single_quoted, UbuntuServerPayload, UbuntuSshPreflight, UbuntuSshPrepareRequest,
    UbuntuSshPreparedHost, UbuntuSwapRequest, UbuntuSwapResult,
};
use super::scripts::{install_payload_script, prepare_host_script};
use super::swap_script::ubuntu_swap_script;

/// SSH-backed Ubuntu setup phases for remote or bare-metal servers.
#[derive(Debug, Clone)]
pub struct UbuntuSshSetup<R> {
    runner: R,
}

impl<R> UbuntuSshSetup<R>
where
    R: RemoteCommandRunner,
{
    /// Creates an Ubuntu SSH setup provider from a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Performs read-only OS, resource, and tool detection.
    pub fn preflight(&self) -> CommandResult<UbuntuSshPreflight> {
        let output = self.runner.run_script(PREFLIGHT_SCRIPT)?;
        let result: UbuntuSshPreflight = parse_single_json_document(&output, "ubuntu preflight")?;
        if result.os_id != "ubuntu" {
            return Err(failure(format!(
                "Remote host is {}, expected Ubuntu",
                result.os_pretty_name
            )));
        }
        Ok(result)
    }

    /// Installs base packages, creates the service user, and installs SteamCMD.
    pub fn prepare_host(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<UbuntuSshPreparedHost> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.prepare.packages",
            "Installing Ubuntu prerequisites.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        let output = self
            .runner
            .run_script(&prepare_host_script(request, false))?;
        parse_single_json_document(&output, "ubuntu prepare host")
    }

    /// Installs or starts k3s using systemd.
    pub fn install_k3s(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.k3s.install",
            "Installing or validating k3s.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.runner.run_script(K3S_INSTALL_SCRIPT)?;
        Ok(())
    }

    /// Creates and enables a native Ubuntu swapfile for low-memory hosts.
    pub fn configure_swap(
        &self,
        request: &UbuntuSwapRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<UbuntuSwapResult> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.swap.native",
            "Creating or validating Ubuntu swapfile.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        let output = self
            .runner
            .run_script(&ubuntu_swap_script(request.swap_size_gib))?;
        parse_single_json_document(&output, "ubuntu swap")
    }

    /// Bootstraps cert-manager and the initial Funcom operator deployments on fresh Ubuntu.
    pub fn bootstrap_kubernetes(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.k3s.bootstrap",
            "Bootstrapping Kubernetes images and operators.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.runner
            .run_script(&bootstrap_kubernetes_script(request))?;
        Ok(())
    }

    /// Downloads the Dune server package through SteamCMD on the Ubuntu host.
    pub fn install_server_payload(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<UbuntuServerPayload> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.steam.download",
            "Installing or validating the Dune server payload.",
            StepDomain::Steam,
            StepAction::Download,
        );
        let output = self.runner.run_script(&install_payload_script(request))?;
        parse_single_json_document(&output, "ubuntu server payload")
    }

    /// Writes the player-facing address consumed by the vendor world creation scripts.
    pub fn write_player_settings(
        &self,
        player_ip: &str,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        if player_ip.trim().is_empty() || player_ip.contains('\n') || player_ip.contains('\r') {
            return Err(failure("Player-facing IP is required"));
        }
        emit(
            sink,
            "ubuntu.settings.player-ip",
            "Writing player-facing server address.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        let script = format!(
            "set -eu\nmkdir -p /home/dune/.dune\nprintf '\\n\\n\\n%s\\n' {} > /home/dune/.dune/settings.conf\nchown -R dune:dune /home/dune/.dune\n",
            sh_single_quoted(player_ip)
        );
        self.runner.run_script(&script)?;
        Ok(())
    }

    /// Removes vendor scheduler references so fresh Ubuntu hosts can use the default scheduler.
    pub fn use_default_scheduler(
        &self,
        namespace: &str,
        battlegroup_name: &str,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        emit(
            sink,
            "ubuntu.scheduler.default",
            "Using the default Kubernetes scheduler for Ubuntu.",
            StepDomain::Kubernetes,
            StepAction::Patch,
        );
        let command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(battlegroup_name),
            sh_single_quoted(namespace),
        );
        let value = self
            .runner
            .run_json(&command, "ubuntu battlegroup scheduler patch source")?;
        let sets = value["spec"]["serverGroup"]["template"]["spec"]["sets"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let operations = sets
            .iter()
            .enumerate()
            .filter(|(_, item)| item.get("schedulerName").is_some())
            .map(|(index, _)| {
                json!({
                    "op": "remove",
                    "path": format!("/spec/serverGroup/template/spec/sets/{index}/schedulerName"),
                })
            })
            .collect::<Vec<_>>();
        if operations.is_empty() {
            return Ok(());
        }
        let patch = serde_json::to_string(&operations)
            .map_err(|err| failure(format!("Failed to serialize scheduler patch: {err}")))?;
        let patch_command = format!(
            "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o name",
            sh_single_quoted(battlegroup_name),
            sh_single_quoted(namespace),
            sh_single_quoted(&patch),
        );
        self.runner.run(&patch_command)?;
        Ok(())
    }
}

fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: &str,
    domain: StepDomain,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.to_string(),
        domain,
        action,
        provider: ProviderKind::Ssh,
    });
}
