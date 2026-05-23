use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        EnsureSwitchRequest, HostProvider, StepAction, StepDomain, VmImportRequest, VmPowerState,
        VmProvider,
    },
};

use super::events::{emit_hyperv_event, OperationSink};
use super::models::{HyperVVmSetupRequest, HyperVVmSetupResult};
use super::vm_import::{clear_destination_dir, destination_has_vm_artifacts, single_vmcx};

/// Orchestrates host-side VM import, networking, disk, memory, and startup.
pub struct HyperVVmSetupOrchestrator<H, V> {
    host: H,
    vm: V,
}

impl<H, V> HyperVVmSetupOrchestrator<H, V>
where
    H: HostProvider,
    V: VmProvider,
{
    /// Creates a VM setup orchestrator from host and VM providers.
    pub fn new(host: H, vm: V) -> Self {
        Self { host, vm }
    }

    /// Imports the packaged VM and prepares it for guest bootstrap.
    pub fn import_and_prepare_vm(
        &self,
        request: &HyperVVmSetupRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<HyperVVmSetupResult> {
        request.validate()?;

        emit_hyperv_event(
            sink,
            "host.readiness",
            "Checking host virtualization readiness.",
            StepDomain::Host,
            StepAction::Check,
        );
        let readiness = self.host.readiness()?;
        if !readiness.elevated {
            return Err(failure("Hyper-V setup requires elevated host privileges"));
        }
        if !readiness.hyperv_available {
            return Err(failure("Hyper-V is not available on this host"));
        }
        if !readiness.vmms_running {
            return Err(failure("Hyper-V vmms service is not running"));
        }

        emit_hyperv_event(
            sink,
            "package.locate-vmcx",
            "Locating packaged VM configuration.",
            StepDomain::Files,
            StepAction::Detect,
        );
        let vmcx_path = single_vmcx(&request.install_path)?;

        emit_hyperv_event(
            sink,
            "hyperv.detect-existing-vm",
            "Checking for an existing VM.",
            StepDomain::HyperV,
            StepAction::Detect,
        );
        if let Some(existing) = self.vm.get_vm(&request.vm_name)? {
            if !request.replace_existing_vm {
                return Err(failure(format!(
                    "VM '{}' already exists and replacement was not requested",
                    existing.name
                )));
            }
            if existing.state == VmPowerState::Running {
                emit_hyperv_event(
                    sink,
                    "hyperv.stop-existing-vm",
                    "Stopping existing VM before replacement.",
                    StepDomain::HyperV,
                    StepAction::Stop,
                );
                self.vm.stop_vm(&request.vm_name, true)?;
            }
            emit_hyperv_event(
                sink,
                "hyperv.remove-existing-vm",
                "Removing existing VM registration.",
                StepDomain::HyperV,
                StepAction::Stop,
            );
            self.vm.remove_vm(&request.vm_name)?;
        }

        if destination_has_vm_artifacts(&request.destination_path) {
            if !request.clear_destination {
                return Err(failure(format!(
                    "VM destination already contains VM files: {}",
                    request.destination_path.display()
                )));
            }
            emit_hyperv_event(
                sink,
                "host.clear-vm-destination",
                "Clearing VM destination folder.",
                StepDomain::Files,
                StepAction::Configure,
            );
            clear_destination_dir(&request.destination_path)?;
        }

        let import_request = VmImportRequest {
            vmcx_path: vmcx_path.clone(),
            destination_path: request.destination_path.to_string_lossy().to_string(),
        };

        emit_hyperv_event(
            sink,
            "hyperv.compare-vm",
            "Checking VM import compatibility.",
            StepDomain::HyperV,
            StepAction::Check,
        );
        let compatibility = self.vm.compare_import(&import_request)?;
        if !compatibility.compatible {
            return Err(failure(format!(
                "VM import compatibility failed: {}",
                compatibility.incompatibilities.join("; ")
            )));
        }

        emit_hyperv_event(
            sink,
            "hyperv.import-vm",
            "Importing VM.",
            StepDomain::HyperV,
            StepAction::Import,
        );
        let imported = self.vm.import_vm(&import_request)?;

        emit_hyperv_event(
            sink,
            "hyperv.ensure-switch",
            "Preparing Hyper-V external switch.",
            StepDomain::HyperV,
            StepAction::Create,
        );
        let switch = self.vm.ensure_external_switch(&EnsureSwitchRequest {
            switch_name: request.switch_name.clone(),
            adapter_name: request.adapter_name.clone(),
        })?;

        emit_hyperv_event(
            sink,
            "hyperv.connect-switch",
            "Connecting VM network adapter.",
            StepDomain::HyperV,
            StepAction::Configure,
        );
        self.vm
            .connect_network_adapter(&imported.name, &switch.name)?;

        emit_hyperv_event(
            sink,
            "hyperv.resize-vhd",
            "Sizing VM virtual disk.",
            StepDomain::HyperV,
            StepAction::Configure,
        );
        self.vm
            .resize_first_vhd(&imported.name, request.disk_size_bytes)?;

        emit_hyperv_event(
            sink,
            "hyperv.set-first-boot",
            "Configuring VM boot disk.",
            StepDomain::HyperV,
            StepAction::Configure,
        );
        self.vm.set_first_boot_disk(&imported.name)?;

        emit_hyperv_event(
            sink,
            "hyperv.set-memory",
            "Configuring VM memory.",
            StepDomain::HyperV,
            StepAction::Configure,
        );
        self.vm
            .set_startup_memory(&imported.name, request.memory.bytes())?;

        emit_hyperv_event(
            sink,
            "hyperv.set-processors",
            "Configuring VM processor count.",
            StepDomain::HyperV,
            StepAction::Configure,
        );
        self.vm
            .set_processor_count(&imported.name, request.processor_count)?;

        emit_hyperv_event(
            sink,
            "hyperv.start-vm",
            "Starting VM.",
            StepDomain::HyperV,
            StepAction::Start,
        );
        self.vm.start_vm(&imported.name)?;

        Ok(HyperVVmSetupResult {
            vm_name: imported.name,
            destination_path: request.destination_path.to_string_lossy().to_string(),
            switch_name: switch.name,
            vmcx_path,
        })
    }
}
