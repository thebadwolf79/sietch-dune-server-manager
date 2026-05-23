use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    models::CommandResult,
    orchestration::{
        DriveCandidate, EnsureSwitchRequest, ExternalSwitch, HostProvider, HostReadiness,
        NetworkAdapterCandidate, VmCompatibilityReport, VmImportRequest, VmInventoryRecord,
        VmProvider,
    },
};

#[derive(Default)]
pub(super) struct MockHost;

impl HostProvider for MockHost {
    fn readiness(&self) -> CommandResult<HostReadiness> {
        Ok(HostReadiness {
            elevated: true,
            hyperv_available: true,
            vmms_running: true,
            virtualization_firmware_enabled: Some(true),
            total_physical_memory_bytes: 64 * 1024 * 1024 * 1024,
            available_physical_memory_bytes: 48 * 1024 * 1024 * 1024,
            logical_processor_count: 16,
        })
    }

    fn drives_with_minimum_free_space(
        &self,
        _minimum_free_bytes: u64,
    ) -> CommandResult<Vec<DriveCandidate>> {
        Ok(vec![])
    }

    fn active_physical_adapters(&self) -> CommandResult<Vec<NetworkAdapterCandidate>> {
        Ok(vec![])
    }
}

#[derive(Default)]
pub(super) struct MockVm {
    pub(super) calls: Rc<RefCell<Vec<&'static str>>>,
    pub(super) existing: Option<VmInventoryRecord>,
}

impl VmProvider for MockVm {
    fn get_vm(&self, _name: &str) -> CommandResult<Option<VmInventoryRecord>> {
        self.calls.borrow_mut().push("get_vm");
        Ok(self.existing.clone())
    }

    fn compare_import(&self, _request: &VmImportRequest) -> CommandResult<VmCompatibilityReport> {
        self.calls.borrow_mut().push("compare_import");
        Ok(VmCompatibilityReport {
            compatible: true,
            incompatibilities: vec![],
        })
    }

    fn import_vm(
        &self,
        _request: &VmImportRequest,
    ) -> CommandResult<crate::orchestration::ImportedVm> {
        self.calls.borrow_mut().push("import_vm");
        Ok(crate::orchestration::ImportedVm {
            name: "test-vm".to_string(),
            configuration_location: "dest".to_string(),
        })
    }

    fn remove_vm(&self, _name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("remove_vm");
        Ok(())
    }

    fn start_vm(&self, _name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("start_vm");
        Ok(())
    }

    fn stop_vm(&self, _name: &str, _turn_off: bool) -> CommandResult<()> {
        self.calls.borrow_mut().push("stop_vm");
        Ok(())
    }

    fn connect_network_adapter(&self, _vm_name: &str, _switch_name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("connect_network_adapter");
        Ok(())
    }

    fn ensure_external_switch(
        &self,
        _request: &EnsureSwitchRequest,
    ) -> CommandResult<ExternalSwitch> {
        self.calls.borrow_mut().push("ensure_external_switch");
        Ok(ExternalSwitch {
            name: "switch".to_string(),
            net_adapter_interface_description: "adapter".to_string(),
        })
    }

    fn resize_first_vhd(&self, _vm_name: &str, _size_bytes: u64) -> CommandResult<()> {
        self.calls.borrow_mut().push("resize_first_vhd");
        Ok(())
    }

    fn set_first_boot_disk(&self, _vm_name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("set_first_boot_disk");
        Ok(())
    }

    fn set_startup_memory(&self, _vm_name: &str, _bytes: u64) -> CommandResult<()> {
        self.calls.borrow_mut().push("set_startup_memory");
        Ok(())
    }

    fn set_processor_count(&self, _vm_name: &str, _count: u32) -> CommandResult<()> {
        self.calls.borrow_mut().push("set_processor_count");
        Ok(())
    }
}

pub(super) fn test_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("dune-manager-orchestration-test-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}
