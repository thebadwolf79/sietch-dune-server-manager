use std::{thread, time::Duration};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{VmPowerState, VmProvider},
};

/// Waits for a running Hyper-V VM to report a non-link-local IPv4 address.
pub fn wait_for_vm_ipv4(
    provider: &impl VmProvider,
    vm_name: &str,
    timeout_seconds: u64,
) -> CommandResult<String> {
    let mut elapsed = 0;
    while elapsed <= timeout_seconds {
        if let Some(vm) = provider.get_vm(vm_name)? {
            if vm.state == VmPowerState::Running {
                if let Some(ip) = vm
                    .ipv4_addresses
                    .iter()
                    .find(|ip| !ip.starts_with("169.254.") && !ip.trim().is_empty())
                {
                    return Ok(ip.clone());
                }
            }
        }
        thread::sleep(Duration::from_secs(2));
        elapsed += 2;
    }
    Err(failure(format!(
        "VM {vm_name} did not report an IPv4 address within {timeout_seconds} seconds"
    )))
}
