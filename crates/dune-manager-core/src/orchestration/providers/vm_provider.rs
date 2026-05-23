//! VM lifecycle and import provider trait.

use crate::orchestration::providers::shared_types::{
    EnsureSwitchRequest, ExternalSwitch, ImportedVm, VmCompatibilityReport, VmImportRequest,
    VmInventoryRecord,
};
use crate::{errors::failure, models::CommandResult};

/// VM lifecycle and import provider.
pub trait VmProvider {
    /// Lists all VMs known to the provider.
    fn list_vms(&self) -> CommandResult<Vec<VmInventoryRecord>> {
        Err(failure("VM listing is not supported by this provider"))
    }
    /// Returns a VM inventory record by name, or `None` when absent.
    fn get_vm(&self, name: &str) -> CommandResult<Option<VmInventoryRecord>>;
    /// Checks whether a packaged VM import is compatible.
    fn compare_import(&self, request: &VmImportRequest) -> CommandResult<VmCompatibilityReport>;
    /// Imports a packaged VM.
    fn import_vm(&self, request: &VmImportRequest) -> CommandResult<ImportedVm>;
    /// Removes a VM registration.
    fn remove_vm(&self, name: &str) -> CommandResult<()>;
    /// Starts a VM.
    fn start_vm(&self, name: &str) -> CommandResult<()>;
    /// Stops a VM.
    fn stop_vm(&self, name: &str, turn_off: bool) -> CommandResult<()>;
    /// Connects the VM network adapter to a switch.
    fn connect_network_adapter(&self, vm_name: &str, switch_name: &str) -> CommandResult<()>;
    /// Ensures an external switch exists.
    fn ensure_external_switch(
        &self,
        request: &EnsureSwitchRequest,
    ) -> CommandResult<ExternalSwitch>;
    /// Resizes the first VHD attached to a VM.
    fn resize_first_vhd(&self, vm_name: &str, size_bytes: u64) -> CommandResult<()>;
    /// Sets the first disk as boot disk.
    fn set_first_boot_disk(&self, vm_name: &str) -> CommandResult<()>;
    /// Sets startup memory.
    fn set_startup_memory(&self, vm_name: &str, bytes: u64) -> CommandResult<()>;
    /// Sets virtual processor count.
    fn set_processor_count(&self, vm_name: &str, count: u32) -> CommandResult<()>;
}

impl<T> VmProvider for &T
where
    T: VmProvider + ?Sized,
{
    fn list_vms(&self) -> CommandResult<Vec<VmInventoryRecord>> {
        (*self).list_vms()
    }

    fn get_vm(&self, name: &str) -> CommandResult<Option<VmInventoryRecord>> {
        (*self).get_vm(name)
    }

    fn compare_import(&self, request: &VmImportRequest) -> CommandResult<VmCompatibilityReport> {
        (*self).compare_import(request)
    }

    fn import_vm(&self, request: &VmImportRequest) -> CommandResult<ImportedVm> {
        (*self).import_vm(request)
    }

    fn remove_vm(&self, name: &str) -> CommandResult<()> {
        (*self).remove_vm(name)
    }

    fn start_vm(&self, name: &str) -> CommandResult<()> {
        (*self).start_vm(name)
    }

    fn stop_vm(&self, name: &str, turn_off: bool) -> CommandResult<()> {
        (*self).stop_vm(name, turn_off)
    }

    fn connect_network_adapter(&self, vm_name: &str, switch_name: &str) -> CommandResult<()> {
        (*self).connect_network_adapter(vm_name, switch_name)
    }

    fn ensure_external_switch(
        &self,
        request: &EnsureSwitchRequest,
    ) -> CommandResult<ExternalSwitch> {
        (*self).ensure_external_switch(request)
    }

    fn resize_first_vhd(&self, vm_name: &str, size_bytes: u64) -> CommandResult<()> {
        (*self).resize_first_vhd(vm_name, size_bytes)
    }

    fn set_first_boot_disk(&self, vm_name: &str) -> CommandResult<()> {
        (*self).set_first_boot_disk(vm_name)
    }

    fn set_startup_memory(&self, vm_name: &str, bytes: u64) -> CommandResult<()> {
        (*self).set_startup_memory(vm_name, bytes)
    }

    fn set_processor_count(&self, vm_name: &str, count: u32) -> CommandResult<()> {
        (*self).set_processor_count(vm_name, count)
    }
}
