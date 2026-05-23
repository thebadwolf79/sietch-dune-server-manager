use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        parse_single_json_document, CreatedWorld, GuestBootstrapProvider, RemoteCommandRunner,
        WorldManifestRequest,
    },
    validation::validate_kube_arg,
};

use super::image_patching::battlegroup_image_patch_operations;
use super::scripts::{
    download_script, shell_value, sh_single_quoted, with_guest_path, CONTAINER_IMAGE_HELPERS,
    DISK_SCRIPT, IMPORT_CORE_IMAGES_SCRIPT, INSTALL_HELPER_SCRIPT, KUBECTL_HELPERS,
    SCALE_CORE_SCRIPT, START_K3S_SCRIPT,
};
use super::scripts_kubernetes::{
    APPLY_DEFAULT_SETTINGS_SCRIPT, IMPORT_BATTLEGROUP_IMAGES_SCRIPT,
    PATCH_DATABASE_OPERATOR_SCRIPT, PATCH_OPERATOR_IMAGES_SCRIPT,
    READ_BATTLEGROUP_VERSION_SCRIPT, SCALE_OPERATOR_SCRIPT,
    SYNC_POSTGRES_SUPERUSER_PASSWORD_SCRIPT, UPDATE_OPERATOR_CRDS_SCRIPT,
};
use super::world_creation::{create_world_script, validate_world_manifest_request, CreateWorldOutput};

/// SSH-backed implementation of the guest bootstrap phases.
#[derive(Debug, Clone)]
pub struct SshGuestBootstrapProvider<R> {
    runner: R,
}

impl<R> SshGuestBootstrapProvider<R>
where
    R: RemoteCommandRunner,
{
    /// Creates a bootstrap provider around a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub(super) fn run_phase(&self, body: &str) -> CommandResult<String> {
        self.runner.run_script(&with_guest_path(body))
    }
}

impl<R> GuestBootstrapProvider for SshGuestBootstrapProvider<R>
where
    R: RemoteCommandRunner,
{
    fn validate_and_resize_root_disk(&self) -> CommandResult<()> {
        self.run_phase(DISK_SCRIPT)?;
        Ok(())
    }

    fn ensure_server_payload(&self) -> CommandResult<()> {
        self.run_phase(&download_script())?;
        Ok(())
    }

    fn start_k3s_and_wait(&self) -> CommandResult<()> {
        self.run_phase(START_K3S_SCRIPT)?;
        Ok(())
    }

    fn import_core_images(&self) -> CommandResult<()> {
        self.run_phase(&format!(
            "{}\n{}",
            CONTAINER_IMAGE_HELPERS, IMPORT_CORE_IMAGES_SCRIPT
        ))?;
        Ok(())
    }

    fn scale_core_deployments(&self) -> CommandResult<()> {
        self.run_phase(&format!("{}\n{}", KUBECTL_HELPERS, SCALE_CORE_SCRIPT))?;
        Ok(())
    }

    fn update_operator_crds(&self) -> CommandResult<()> {
        self.run_phase(&format!(
            "{}\n{}",
            KUBECTL_HELPERS, UPDATE_OPERATOR_CRDS_SCRIPT
        ))?;
        Ok(())
    }

    fn patch_operator_images(&self) -> CommandResult<()> {
        self.run_phase(&format!(
            "{}\n{}\n{}\n{}",
            KUBECTL_HELPERS,
            CONTAINER_IMAGE_HELPERS,
            PATCH_DATABASE_OPERATOR_SCRIPT,
            PATCH_OPERATOR_IMAGES_SCRIPT
        ))?;
        Ok(())
    }

    fn scale_operator_deployments(&self) -> CommandResult<()> {
        self.run_phase(&format!("{}\n{}", KUBECTL_HELPERS, SCALE_OPERATOR_SCRIPT))?;
        Ok(())
    }

    fn install_battlegroup_helper(&self) -> CommandResult<()> {
        self.run_phase(INSTALL_HELPER_SCRIPT)?;
        Ok(())
    }

    fn create_world(&self, request: &WorldManifestRequest) -> CommandResult<CreatedWorld> {
        validate_world_manifest_request(request)?;
        let script = create_world_script(request);
        let output = self.run_phase(&script)?;
        let result: CreateWorldOutput = parse_single_json_document(&output, "create world")?;
        Ok(CreatedWorld {
            namespace: result.namespace,
            battlegroup_name: result.battlegroup_name,
        })
    }

    fn import_battlegroup_images(&self) -> CommandResult<()> {
        self.run_phase(&format!(
            "{}\n{}",
            CONTAINER_IMAGE_HELPERS, IMPORT_BATTLEGROUP_IMAGES_SCRIPT
        ))?;
        Ok(())
    }

    fn patch_battlegroup_images(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()> {
        validate_kube_arg(namespace, "namespace")?;
        validate_kube_arg(battlegroup_name, "battlegroup name")?;
        let new_version = self
            .run_phase(READ_BATTLEGROUP_VERSION_SCRIPT)?
            .trim()
            .to_string();
        if new_version.is_empty() {
            return Err(failure("Battlegroup image version file was empty"));
        }

        self.sync_existing_postgres_credentials(namespace, battlegroup_name)?;

        let command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(battlegroup_name),
            sh_single_quoted(namespace),
        );
        let battlegroup_json = self
            .runner
            .run_json(&command, "battlegroup image patch source")?;
        let operations = battlegroup_image_patch_operations(&battlegroup_json, &new_version)?;
        let patch_command = format!(
            "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o json",
            sh_single_quoted(battlegroup_name),
            sh_single_quoted(namespace),
            sh_single_quoted(&serde_json::to_string(&operations).map_err(|err| {
                failure(format!(
                    "Failed to serialize battlegroup image patch: {err}"
                ))
            })?),
        );
        self.runner.run(&patch_command)?;
        Ok(())
    }

    fn apply_default_user_settings(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()> {
        validate_kube_arg(namespace, "namespace")?;
        validate_kube_arg(battlegroup_name, "battlegroup name")?;
        let mut script = String::new();
        script.push_str("set -eu\n");
        script.push_str(&shell_value("NS", namespace));
        script.push_str(APPLY_DEFAULT_SETTINGS_SCRIPT);
        self.run_phase(&script)?;
        Ok(())
    }
}

impl<R> SshGuestBootstrapProvider<R>
where
    R: RemoteCommandRunner,
{
    fn sync_existing_postgres_credentials(
        &self,
        namespace: &str,
        battlegroup_name: &str,
    ) -> CommandResult<()> {
        validate_kube_arg(namespace, "namespace")?;
        validate_kube_arg(battlegroup_name, "battlegroup name")?;

        let mut script = String::new();
        script.push_str("set -eu\n");
        script.push_str(&shell_value("NS", namespace));
        script.push_str(&shell_value("BG", battlegroup_name));
        script.push_str(SYNC_POSTGRES_SUPERUSER_PASSWORD_SCRIPT);
        self.run_phase(&script)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
