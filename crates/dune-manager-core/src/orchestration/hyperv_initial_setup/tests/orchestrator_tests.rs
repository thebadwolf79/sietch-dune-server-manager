use std::{
    cell::RefCell,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::orchestration::{
    GuestBootstrapPlan, HyperVVmSetupRequest, MemoryProfile, VecOperationSink,
    DEFAULT_VM_DISK_BYTES,
};

use super::super::{
    detect_player_address_candidates, GuestNetworkPlan, HyperVInitialSetupOrchestrator,
    HyperVInitialSetupRequest,
};
use super::mock_providers::{MockBootstrap, MockGuest, MockHost, MockVm};

#[test]
fn orchestrates_hyperv_initial_setup_across_provider_boundaries() {
    let temp = test_dir();
    let install = temp.join("server");
    let vm_dir = install.join("Virtual Machines");
    fs::create_dir_all(&vm_dir).unwrap();
    fs::write(vm_dir.join("server.vmcx"), "").unwrap();

    let guest_calls = Rc::new(RefCell::new(Vec::new()));
    let bootstrap_calls = Rc::new(RefCell::new(Vec::new()));
    let orchestrator = HyperVInitialSetupOrchestrator::new(
        MockHost,
        MockVm::default(),
        MockGuest {
            calls: guest_calls.clone(),
        },
        MockBootstrap {
            calls: bootstrap_calls.clone(),
        },
    );
    let mut sink = VecOperationSink::default();
    let result = orchestrator
        .run(
            &HyperVInitialSetupRequest {
                vm: HyperVVmSetupRequest {
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
                    convert_to_fixed_disk: false,
                    disable_dynamic_memory: false,
                },
                guest_network: GuestNetworkPlan::Dhcp,
                guest_bootstrap: GuestBootstrapPlan {
                    player_ip: "10.0.0.4".to_string(),
                    world_name: "Adain".to_string(),
                    world_region: "Europe".to_string(),
                    self_host_token: "token".to_string(),
                    host_id: "host123".to_string(),
                    world_suffix: "abcdef".to_string(),
                },
                vm_ip_timeout_seconds: 2,
                ssh_timeout_seconds: 2,
            },
            &mut sink,
        )
        .unwrap();

    assert_eq!(result.guest_ip, "10.0.0.4");
    assert_eq!(
        guest_calls.borrow().as_slice(),
        &["wait_for_ssh", "write_player_settings"]
    );
    assert_eq!(
        bootstrap_calls.borrow().as_slice(),
        &[
            "disk",
            "payload",
            "k3s",
            "core_images",
            "core_scale",
            "operator_crds",
            "operator_images",
            "operator_scale",
            "helper",
            "world",
            "bg_images",
            "bg_patch",
            "defaults",
        ]
    );
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "guest.write-player-settings"));
}

#[test]
fn detects_player_address_candidates_as_a_distinct_setup_step() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let guest = MockGuest {
        calls: calls.clone(),
    };
    let mut sink = VecOperationSink::default();
    let candidates = detect_player_address_candidates(&guest, "10.0.0.4", &mut sink).unwrap();

    assert_eq!(candidates.guest_lan_ip, "10.0.0.4");
    assert_eq!(candidates.public_ip, Some("203.0.113.10".to_string()));
    assert_eq!(calls.borrow().as_slice(), &["detect_public_ip"]);
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "guest.detect-public-ip"));
}

fn test_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("dune-manager-initial-setup-test-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}
