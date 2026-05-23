//! Host-level discovery provider trait.

use crate::models::CommandResult;
use crate::orchestration::providers::shared_types::{
    DriveCandidate, HostReadiness, NetworkAdapterCandidate,
};

/// Host-level discovery provider.
pub trait HostProvider {
    /// Returns host readiness information.
    fn readiness(&self) -> CommandResult<HostReadiness>;
    /// Lists drives with at least the requested free space.
    fn drives_with_minimum_free_space(
        &self,
        minimum_free_bytes: u64,
    ) -> CommandResult<Vec<DriveCandidate>>;
    /// Lists active physical IPv4 adapters suitable for setup.
    fn active_physical_adapters(&self) -> CommandResult<Vec<NetworkAdapterCandidate>>;
}

impl<T> HostProvider for &T
where
    T: HostProvider + ?Sized,
{
    fn readiness(&self) -> CommandResult<HostReadiness> {
        (*self).readiness()
    }

    fn drives_with_minimum_free_space(
        &self,
        minimum_free_bytes: u64,
    ) -> CommandResult<Vec<DriveCandidate>> {
        (*self).drives_with_minimum_free_space(minimum_free_bytes)
    }

    fn active_physical_adapters(&self) -> CommandResult<Vec<NetworkAdapterCandidate>> {
        (*self).active_physical_adapters()
    }
}
