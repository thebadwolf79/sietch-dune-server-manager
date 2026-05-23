use std::{cell::RefCell, rc::Rc};

use crate::{
    models::CommandResult,
    orchestration::{
        CreatedWorld, DriveCandidate, ExternalSwitch, GuestBootstrapProvider, GuestNetworkConfig,
        GuestProvider, HostProvider, HostReadiness, ImportedVm, NetworkAdapterCandidate,
        VmCompatibilityReport, VmImportRequest, VmInventoryRecord, VmPowerState, VmProvider,
        WorldManifestRequest,
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
    pub calls: Rc<RefCell<Vec<&'static str>>>,
    pub get_vm_calls: RefCell<usize>,
}

impl VmProvider for MockVm {
    fn get_vm(&self, _name: &str) -> CommandResult<Option<VmInventoryRecord>> {
        self.calls.borrow_mut().push("get_vm");
        let mut get_vm_calls = self.get_vm_calls.borrow_mut();
        *get_vm_calls += 1;
        if *get_vm_calls == 1 {
            return Ok(None);
        }
        Ok(Some(VmInventoryRecord {
            name: "test-vm".to_string(),
            state: VmPowerState::Running,
            raw_state: "Running".to_string(),
            configuration_location: String::new(),
            path: String::new(),
            memory_assigned_bytes: 0,
            processor_count: 0,
            uptime_seconds: 0,
            ipv4_addresses: vec!["10.0.0.4".to_string()],
            hard_disk_paths: vec![],
            disk_size_bytes: 0,
            disk_file_size_bytes: 0,
            switch_names: vec![],
        }))
    }

    fn compare_import(&self, _request: &VmImportRequest) -> CommandResult<VmCompatibilityReport> {
        self.calls.borrow_mut().push("compare_import");
        Ok(VmCompatibilityReport {
            compatible: true,
            incompatibilities: vec![],
        })
    }

    fn import_vm(&self, _request: &VmImportRequest) -> CommandResult<ImportedVm> {
        self.calls.borrow_mut().push("import_vm");
        Ok(ImportedVm {
            name: "test-vm".to_string(),
            configuration_location: String::new(),
        })
    }

    fn remove_vm(&self, _name: &str) -> CommandResult<()> {
        Ok(())
    }

    fn start_vm(&self, _name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("start_vm");
        Ok(())
    }

    fn stop_vm(&self, _name: &str, _turn_off: bool) -> CommandResult<()> {
        Ok(())
    }

    fn connect_network_adapter(&self, _vm_name: &str, _switch_name: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("connect_network_adapter");
        Ok(())
    }

    fn ensure_external_switch(
        &self,
        _request: &crate::orchestration::EnsureSwitchRequest,
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

#[derive(Default)]
pub(super) struct MockGuest {
    pub calls: Rc<RefCell<Vec<&'static str>>>,
}

impl GuestProvider for MockGuest {
    fn wait_for_ssh(&self, _ip: &str, _timeout_seconds: u64) -> CommandResult<()> {
        self.calls.borrow_mut().push("wait_for_ssh");
        Ok(())
    }

    fn upload_bytes(
        &self,
        _ip: &str,
        _remote_path: &str,
        _bytes: &[u8],
        _mode: u32,
    ) -> CommandResult<()> {
        Ok(())
    }

    fn write_player_settings(&self, _ip: &str, _player_ip: &str) -> CommandResult<()> {
        self.calls.borrow_mut().push("write_player_settings");
        Ok(())
    }

    fn apply_static_network(
        &self,
        _ip: &str,
        _config: &GuestNetworkConfig,
    ) -> CommandResult<()> {
        self.calls.borrow_mut().push("apply_static_network");
        Ok(())
    }

    fn detect_public_ip(&self, _ip: &str) -> CommandResult<Option<String>> {
        self.calls.borrow_mut().push("detect_public_ip");
        Ok(Some("203.0.113.10".to_string()))
    }
}

#[derive(Default)]
pub(super) struct MockBootstrap {
    pub calls: Rc<RefCell<Vec<&'static str>>>,
}

impl GuestBootstrapProvider for MockBootstrap {
    fn validate_and_resize_root_disk(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("disk");
        Ok(())
    }
    fn ensure_server_payload(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("payload");
        Ok(())
    }
    fn start_k3s_and_wait(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("k3s");
        Ok(())
    }
    fn import_core_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("core_images");
        Ok(())
    }
    fn scale_core_deployments(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("core_scale");
        Ok(())
    }
    fn update_operator_crds(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_crds");
        Ok(())
    }
    fn patch_operator_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_images");
        Ok(())
    }
    fn scale_operator_deployments(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_scale");
        Ok(())
    }
    fn install_battlegroup_helper(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("helper");
        Ok(())
    }
    fn create_world(&self, request: &WorldManifestRequest) -> CommandResult<CreatedWorld> {
        self.calls.borrow_mut().push("world");
        Ok(CreatedWorld {
            namespace: format!("funcom-seabass-{}", request.world_unique_name),
            battlegroup_name: request.world_unique_name.clone(),
        })
    }
    fn import_battlegroup_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("bg_images");
        Ok(())
    }
    fn patch_battlegroup_images(
        &self,
        _namespace: &str,
        _battlegroup_name: &str,
    ) -> CommandResult<()> {
        self.calls.borrow_mut().push("bg_patch");
        Ok(())
    }
    fn apply_default_user_settings(
        &self,
        _namespace: &str,
        _battlegroup_name: &str,
    ) -> CommandResult<()> {
        self.calls.borrow_mut().push("defaults");
        Ok(())
    }
}
