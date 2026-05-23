use std::{cell::RefCell, fs, rc::Rc};

use crate::orchestration::{VmInventoryRecord, VmPowerState};

use super::super::events::VecOperationSink;
use super::super::models::{HyperVVmSetupRequest, MemoryProfile, DEFAULT_VM_DISK_BYTES};
use super::super::orchestrator::HyperVVmSetupOrchestrator;
use super::mocks::{test_dir, MockHost, MockVm};

#[test]
fn orchestrates_hyperv_vm_import_sequence() {
    let temp = test_dir();
    let install = temp.join("server");
    let vm_dir = install.join("Virtual Machines");
    fs::create_dir_all(&vm_dir).unwrap();
    fs::write(vm_dir.join("server.vmcx"), "").unwrap();
    let destination = temp.join("vm");

    let calls = Rc::new(RefCell::new(Vec::new()));
    let vm = MockVm {
        calls: calls.clone(),
        existing: None,
    };
    let orchestrator = HyperVVmSetupOrchestrator::new(MockHost, vm);
    let mut sink = VecOperationSink::default();
    let result = orchestrator
        .import_and_prepare_vm(
            &HyperVVmSetupRequest {
                install_path: install,
                vm_name: "test-vm".to_string(),
                destination_path: destination,
                switch_name: "switch".to_string(),
                adapter_name: "Ethernet".to_string(),
                memory: MemoryProfile::Sietch20Gb,
                processor_count: 4,
                replace_existing_vm: false,
                clear_destination: false,
                disk_size_bytes: DEFAULT_VM_DISK_BYTES,
            },
            &mut sink,
        )
        .unwrap();

    assert_eq!(result.vm_name, "test-vm");
    assert_eq!(
        calls.borrow().as_slice(),
        &[
            "get_vm",
            "compare_import",
            "import_vm",
            "ensure_external_switch",
            "connect_network_adapter",
            "resize_first_vhd",
            "set_first_boot_disk",
            "set_startup_memory",
            "set_processor_count",
            "start_vm",
        ]
    );
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "hyperv.import-vm"));
}

#[test]
fn refuses_existing_vm_without_replace_flag() {
    let temp = test_dir();
    let install = temp.join("server");
    let vm_dir = install.join("Virtual Machines");
    fs::create_dir_all(&vm_dir).unwrap();
    fs::write(vm_dir.join("server.vmcx"), "").unwrap();

    let vm = MockVm {
        calls: Rc::new(RefCell::new(Vec::new())),
        existing: Some(VmInventoryRecord {
            name: "test-vm".to_string(),
            state: VmPowerState::Off,
            raw_state: "Off".to_string(),
            configuration_location: String::new(),
            path: String::new(),
            memory_assigned_bytes: 0,
            processor_count: 0,
            uptime_seconds: 0,
            ipv4_addresses: vec![],
            hard_disk_paths: vec![],
            disk_size_bytes: 0,
            disk_file_size_bytes: 0,
            switch_names: vec![],
        }),
    };
    let orchestrator = HyperVVmSetupOrchestrator::new(MockHost, vm);
    let mut sink = VecOperationSink::default();
    let err = orchestrator
        .import_and_prepare_vm(
            &HyperVVmSetupRequest {
                install_path: install,
                vm_name: "test-vm".to_string(),
                destination_path: temp.join("vm"),
                switch_name: "switch".to_string(),
                adapter_name: "Ethernet".to_string(),
                memory: MemoryProfile::Sietch20Gb,
                processor_count: 4,
                replace_existing_vm: false,
                clear_destination: false,
                disk_size_bytes: DEFAULT_VM_DISK_BYTES,
            },
            &mut sink,
        )
        .unwrap_err();
    assert!(err.message.contains("already exists"));
}

#[test]
fn allows_existing_destination_folder_without_vm_artifacts() {
    let temp = test_dir();
    let install = temp.join("server");
    let vm_dir = install.join("Virtual Machines");
    fs::create_dir_all(&vm_dir).unwrap();
    fs::write(vm_dir.join("server.vmcx"), "").unwrap();
    let destination = temp.join("vm");
    fs::create_dir_all(&destination).unwrap();

    let calls = Rc::new(RefCell::new(Vec::new()));
    let vm = MockVm {
        calls: calls.clone(),
        existing: None,
    };
    let orchestrator = HyperVVmSetupOrchestrator::new(MockHost, vm);
    let mut sink = VecOperationSink::default();
    orchestrator
        .import_and_prepare_vm(
            &HyperVVmSetupRequest {
                install_path: install,
                vm_name: "test-vm".to_string(),
                destination_path: destination,
                switch_name: "switch".to_string(),
                adapter_name: "Ethernet".to_string(),
                memory: MemoryProfile::Sietch20Gb,
                processor_count: 4,
                replace_existing_vm: false,
                clear_destination: false,
                disk_size_bytes: DEFAULT_VM_DISK_BYTES,
            },
            &mut sink,
        )
        .unwrap();
    assert!(calls.borrow().contains(&"import_vm"));
}

#[test]
fn refuses_destination_folder_with_vm_artifacts() {
    let temp = test_dir();
    let install = temp.join("server");
    let vm_dir = install.join("Virtual Machines");
    fs::create_dir_all(&vm_dir).unwrap();
    fs::write(vm_dir.join("server.vmcx"), "").unwrap();
    let destination = temp.join("vm");
    fs::create_dir_all(destination.join("Virtual Machines")).unwrap();

    let vm = MockVm {
        calls: Rc::new(RefCell::new(Vec::new())),
        existing: None,
    };
    let orchestrator = HyperVVmSetupOrchestrator::new(MockHost, vm);
    let mut sink = VecOperationSink::default();
    let err = orchestrator
        .import_and_prepare_vm(
            &HyperVVmSetupRequest {
                install_path: install,
                vm_name: "test-vm".to_string(),
                destination_path: destination,
                switch_name: "switch".to_string(),
                adapter_name: "Ethernet".to_string(),
                memory: MemoryProfile::Sietch20Gb,
                processor_count: 4,
                replace_existing_vm: false,
                clear_destination: false,
                disk_size_bytes: DEFAULT_VM_DISK_BYTES,
            },
            &mut sink,
        )
        .unwrap_err();
    assert!(err.message.contains("contains VM files"));
}
