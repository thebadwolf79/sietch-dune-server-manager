//! Provider traits and shared data models for host, VM, guest, and Kubernetes operations.

mod guest_bootstrap_provider;
mod guest_provider;
mod host_provider;
mod kubernetes_provider;
mod shared_types;
mod vm_provider;

pub use guest_bootstrap_provider::GuestBootstrapProvider;
pub use guest_provider::GuestProvider;
pub use host_provider::HostProvider;
pub use kubernetes_provider::KubernetesProvider;
pub use shared_types::{
    packaged_vmcx_candidates, BattlegroupState, CreatedWorld, DriveCandidate, EnsureSwitchRequest,
    ExternalSwitch, GuestNetworkConfig, HostReadiness, ImportedVm, NetworkAdapterCandidate,
    VmCompatibilityReport, VmImportRequest, VmInventoryRecord, VmPowerState, WorldManifestRequest,
};
pub use vm_provider::VmProvider;
