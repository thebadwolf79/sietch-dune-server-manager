//! Guest bootstrap provider trait covering disk, payload, k3s, operators, and world setup.

use crate::models::CommandResult;
use crate::orchestration::providers::shared_types::{CreatedWorld, WorldManifestRequest};

/// Provider for the guest bootstrap phases.
pub trait GuestBootstrapProvider {
    /// Validates and expands guest root disk if needed.
    fn validate_and_resize_root_disk(&self) -> CommandResult<()>;
    /// Ensures the server payload is downloaded inside the guest.
    fn ensure_server_payload(&self) -> CommandResult<()>;
    /// Starts k3s and waits until it is reachable.
    fn start_k3s_and_wait(&self) -> CommandResult<()>;
    /// Imports prerequisite k3s images.
    fn import_core_images(&self) -> CommandResult<()>;
    /// Starts core k3s deployments.
    fn scale_core_deployments(&self) -> CommandResult<()>;
    /// Updates operator CRDs and RBAC.
    fn update_operator_crds(&self) -> CommandResult<()>;
    /// Patches operator deployment images.
    fn patch_operator_images(&self) -> CommandResult<()>;
    /// Starts operator deployments.
    fn scale_operator_deployments(&self) -> CommandResult<()>;
    /// Installs the guest battlegroup helper script.
    fn install_battlegroup_helper(&self) -> CommandResult<()>;
    /// Creates the world namespace, secrets, and battlegroup resource.
    fn create_world(&self, request: &WorldManifestRequest) -> CommandResult<CreatedWorld>;
    /// Imports battlegroup container images.
    fn import_battlegroup_images(&self) -> CommandResult<()>;
    /// Patches battlegroup image tags to the downloaded version.
    fn patch_battlegroup_images(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()>;
    /// Applies default user settings files through the file browser pod.
    fn apply_default_user_settings(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()>;
}

impl<T> GuestBootstrapProvider for &T
where
    T: GuestBootstrapProvider + ?Sized,
{
    fn validate_and_resize_root_disk(&self) -> CommandResult<()> {
        (*self).validate_and_resize_root_disk()
    }

    fn ensure_server_payload(&self) -> CommandResult<()> {
        (*self).ensure_server_payload()
    }

    fn start_k3s_and_wait(&self) -> CommandResult<()> {
        (*self).start_k3s_and_wait()
    }

    fn import_core_images(&self) -> CommandResult<()> {
        (*self).import_core_images()
    }

    fn scale_core_deployments(&self) -> CommandResult<()> {
        (*self).scale_core_deployments()
    }

    fn update_operator_crds(&self) -> CommandResult<()> {
        (*self).update_operator_crds()
    }

    fn patch_operator_images(&self) -> CommandResult<()> {
        (*self).patch_operator_images()
    }

    fn scale_operator_deployments(&self) -> CommandResult<()> {
        (*self).scale_operator_deployments()
    }

    fn install_battlegroup_helper(&self) -> CommandResult<()> {
        (*self).install_battlegroup_helper()
    }

    fn create_world(&self, request: &WorldManifestRequest) -> CommandResult<CreatedWorld> {
        (*self).create_world(request)
    }

    fn import_battlegroup_images(&self) -> CommandResult<()> {
        (*self).import_battlegroup_images()
    }

    fn patch_battlegroup_images(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()> {
        (*self).patch_battlegroup_images(namespace, battlegroup_name)
    }

    fn apply_default_user_settings(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()> {
        (*self).apply_default_user_settings(namespace, battlegroup_name)
    }
}
