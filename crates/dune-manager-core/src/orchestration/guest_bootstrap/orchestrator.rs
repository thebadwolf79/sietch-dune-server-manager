//! Guest bootstrap orchestrator that drives the provider through the setup sequence.

use serde::Serialize;

use crate::{
    models::CommandResult,
    orchestration::{
        GuestBootstrapProvider, OperationSink, OrchestrationEvent, ProviderKind, StepAction,
        StepDomain, WorldManifestRequest,
    },
};

use super::plan::GuestBootstrapPlan;

/// Identifies the world resources created by guest bootstrap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestBootstrapResult {
    /// Kubernetes namespace created for the BattleGroup.
    pub namespace: String,
    /// BattleGroup resource name.
    pub battlegroup_name: String,
    /// Vendor unique world name used for namespace and resource creation.
    pub world_unique_name: String,
}

/// Runs the native replacement for the vendor guest setup script.
pub struct GuestBootstrapOrchestrator<P> {
    provider: P,
}

impl<P> GuestBootstrapOrchestrator<P>
where
    P: GuestBootstrapProvider,
{
    /// Creates a guest bootstrap orchestrator around a provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Executes disk, payload, k3s, operator, world, image, and defaults setup.
    pub fn run(
        &self,
        plan: &GuestBootstrapPlan,
        sink: &mut impl OperationSink,
    ) -> CommandResult<GuestBootstrapResult> {
        plan.validate()?;

        emit(
            sink,
            "guest-settings",
            "Writing player-facing server address.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        // The existing guest provider split still owns the actual settings file write.
        // This bootstrap provider starts at the vendor bootstrap/setup boundary.

        emit(
            sink,
            "guest-disk",
            "Checking guest disk capacity.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.provider.validate_and_resize_root_disk()?;

        emit(
            sink,
            "guest-download",
            "Ensuring guest server payload is installed.",
            StepDomain::Steam,
            StepAction::Download,
        );
        self.provider.ensure_server_payload()?;

        emit(
            sink,
            "guest-k3s.start",
            "Starting k3s.",
            StepDomain::Guest,
            StepAction::Start,
        );
        self.provider.start_k3s_and_wait()?;

        emit(
            sink,
            "guest-k3s.import-core-images",
            "Importing k3s prerequisite images.",
            StepDomain::Guest,
            StepAction::Import,
        );
        self.provider.import_core_images()?;

        emit(
            sink,
            "guest-k3s.scale-core",
            "Starting k3s core deployments.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.provider.scale_core_deployments()?;

        emit(
            sink,
            "guest-operators.update-crds",
            "Updating operator resources.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.provider.update_operator_crds()?;

        emit(
            sink,
            "guest-operators.patch-images",
            "Updating operator images.",
            StepDomain::Kubernetes,
            StepAction::Patch,
        );
        self.provider.patch_operator_images()?;

        emit(
            sink,
            "guest-operators.scale",
            "Starting operator deployments.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.provider.scale_operator_deployments()?;

        emit(
            sink,
            "guest-system.install-helper",
            "Installing guest battlegroup helper.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.provider.install_battlegroup_helper()?;

        let world_unique_name = plan.world_unique_name();
        emit(
            sink,
            "guest-world.create",
            "Creating battlegroup world resources.",
            StepDomain::Kubernetes,
            StepAction::Create,
        );
        let world = self.provider.create_world(&WorldManifestRequest {
            world_name: plan.world_name.clone(),
            world_region: plan.world_region.clone(),
            player_ip: plan.player_ip.clone(),
            world_unique_name: world_unique_name.clone(),
            self_host_token: plan.self_host_token.clone(),
        })?;

        emit(
            sink,
            "guest-images.import",
            "Importing battlegroup images.",
            StepDomain::Guest,
            StepAction::Import,
        );
        self.provider.import_battlegroup_images()?;

        emit(
            sink,
            "guest-images.patch",
            "Patching battlegroup image revisions.",
            StepDomain::Kubernetes,
            StepAction::Patch,
        );
        self.provider
            .patch_battlegroup_images(&world.namespace, &world.battlegroup_name)?;

        emit(
            sink,
            "guest-defaults.apply",
            "Applying default user settings.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.provider
            .apply_default_user_settings(&world.namespace, &world.battlegroup_name)?;

        Ok(GuestBootstrapResult {
            namespace: world.namespace,
            battlegroup_name: world.battlegroup_name,
            world_unique_name,
        })
    }
}

fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: impl Into<String>,
    domain: StepDomain,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain,
        action,
        provider: ProviderKind::HyperV,
    });
}

#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;
