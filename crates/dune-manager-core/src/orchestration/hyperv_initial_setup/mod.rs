//! Hyper-V initial setup orchestration: imports the VM, waits for guest connectivity,
//! optionally applies static networking, and runs guest bootstrap.

mod orchestrator;
mod player_address;
mod request;
mod wait;

#[cfg(test)]
mod tests;

pub use orchestrator::HyperVInitialSetupOrchestrator;
pub use player_address::detect_player_address_candidates;
pub use request::{
    GuestNetworkPlan, HyperVInitialSetupRequest, HyperVInitialSetupResult, PlayerAddressCandidates,
};
pub use wait::wait_for_vm_ipv4;
