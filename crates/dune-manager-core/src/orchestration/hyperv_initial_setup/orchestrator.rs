use crate::{
    models::CommandResult,
    orchestration::{
        emit_hyperv_event, GuestBootstrapOrchestrator, GuestBootstrapProvider, GuestProvider,
        HyperVVmSetupOrchestrator, OperationSink, StepAction, StepDomain, VmProvider,
    },
};

use super::request::{GuestNetworkPlan, HyperVInitialSetupRequest, HyperVInitialSetupResult};
use super::wait::wait_for_vm_ipv4;

/// Orchestrates full first-time setup across host, VM, guest, and bootstrap providers.
pub struct HyperVInitialSetupOrchestrator<H, V, G, B> {
    host: H,
    vm: V,
    guest: G,
    bootstrap: B,
}

impl<H, V, G, B> HyperVInitialSetupOrchestrator<H, V, G, B>
where
    H: crate::orchestration::HostProvider,
    V: VmProvider,
    G: GuestProvider,
    B: GuestBootstrapProvider,
{
    /// Creates an initial setup orchestrator from its provider boundaries.
    pub fn new(host: H, vm: V, guest: G, bootstrap: B) -> Self {
        Self {
            host,
            vm,
            guest,
            bootstrap,
        }
    }

    /// Imports and starts the VM, waits for SSH, applies network settings, and bootstraps k3s.
    pub fn run(
        &self,
        request: &HyperVInitialSetupRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<HyperVInitialSetupResult> {
        request.validate()?;

        let vm_setup = HyperVVmSetupOrchestrator::new(&self.host, &self.vm);
        let vm = vm_setup.import_and_prepare_vm(&request.vm, sink)?;

        emit_hyperv_event(
            sink,
            "hyperv.wait-for-ip",
            "Waiting for VM IPv4 address.",
            StepDomain::HyperV,
            StepAction::Detect,
        );
        let first_ip = wait_for_vm_ipv4(&self.vm, &vm.vm_name, request.vm_ip_timeout_seconds)?;

        emit_hyperv_event(
            sink,
            "guest.wait-for-ssh",
            "Waiting for guest SSH.",
            StepDomain::Guest,
            StepAction::Check,
        );
        self.guest
            .wait_for_ssh(&first_ip, request.ssh_timeout_seconds)?;

        let guest_ip = match &request.guest_network {
            GuestNetworkPlan::Dhcp => first_ip,
            GuestNetworkPlan::Static(config) => {
                emit_hyperv_event(
                    sink,
                    "guest.apply-static-network",
                    "Applying static guest network.",
                    StepDomain::Guest,
                    StepAction::Configure,
                );
                self.guest.apply_static_network(&first_ip, config)?;
                let static_ip = config
                    .address_cidr
                    .split_once('/')
                    .map(|(ip, _)| ip.to_string())
                    .unwrap_or_else(|| config.address_cidr.clone());
                self.guest
                    .wait_for_ssh(&static_ip, request.ssh_timeout_seconds)?;
                static_ip
            }
        };

        emit_hyperv_event(
            sink,
            "guest.write-player-settings",
            "Writing player-facing server address.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.guest
            .write_player_settings(&guest_ip, &request.guest_bootstrap.player_ip)?;

        let bootstrap =
            GuestBootstrapOrchestrator::new(&self.bootstrap).run(&request.guest_bootstrap, sink)?;

        Ok(HyperVInitialSetupResult {
            vm,
            guest_ip,
            bootstrap,
        })
    }
}
