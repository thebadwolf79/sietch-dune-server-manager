//! Orchestrates durable BattleGroup map instance updates.

use serde_json::Value;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        instance_management::{
            count_models::{SetMapInstancesRequest, SetMapInstancesResult},
            display_name_helpers::build_display_name_update,
            display_name_models::{SetMapDisplayNameRequest, SetMapDisplayNameResult},
            orchestrator_helpers::{
                build_world_partition_update, deep_desert_pvp_ids, write_pvp_config_script,
            },
            shell::sh_single_quoted,
        },
        BattlegroupRef, RemoteCommandRunner,
    },
    validation::validate_kube_arg,
};

/// Orchestrates durable BattleGroup map instance updates.
#[derive(Debug, Clone)]
pub struct MapInstanceOrchestrator<R> {
    runner: R,
}

impl<R> MapInstanceOrchestrator<R>
where
    R: RemoteCommandRunner,
{
    /// Creates a map instance orchestrator around a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Sets the desired partition count in the BattleGroup resource.
    pub fn set_instances(
        &self,
        request: &SetMapInstancesRequest,
    ) -> CommandResult<SetMapInstancesResult> {
        request.validate()?;

        let battlegroup = self.battlegroup(&request.battlegroup)?;
        let update = build_world_partition_update(&battlegroup, request.map, request.count)?;
        let mut battlegroup_patched = false;

        if update.patch_required {
            let patch = serde_json::to_string(&update.patch_operations)
                .map_err(|err| failure(format!("Failed to serialize instance patch: {err}")))?;
            let command = format!(
                "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o json",
                sh_single_quoted(&request.battlegroup.name),
                sh_single_quoted(&request.battlegroup.namespace),
                sh_single_quoted(&patch),
            );
            self.runner
                .run_json(&command, "map instance battlegroup patch")?;
            battlegroup_patched = true;
        }

        let pvp_partition_ids = request.pvp_partition_ids.clone().or_else(|| {
            request
                .pvp_instance_count
                .map(|count| deep_desert_pvp_ids(&update.partition_ids, count))
        });

        let mut pvp_config_updated = false;
        if let Some(ids) = &pvp_partition_ids {
            self.write_deep_desert_pvp_config(&request.battlegroup.namespace, ids)?;
            pvp_config_updated = true;
        }

        Ok(SetMapInstancesResult {
            map: request.map.map_name().to_string(),
            partition_ids: update.partition_ids,
            pvp_partition_ids: pvp_partition_ids.unwrap_or_default(),
            restart_required: battlegroup_patched || pvp_config_updated,
            battlegroup_patched,
            pvp_config_updated,
        })
    }

    /// Sets or clears the display-name override for a single map dimension.
    pub fn set_display_name(
        &self,
        request: &SetMapDisplayNameRequest,
    ) -> CommandResult<SetMapDisplayNameResult> {
        request.validate()?;

        let battlegroup = self.battlegroup(&request.battlegroup)?;
        let update = build_display_name_update(&battlegroup, request)?;
        if update.patch_required {
            let patch = serde_json::to_string(&update.patch_operations)
                .map_err(|err| failure(format!("Failed to serialize display-name patch: {err}")))?;
            let command = format!(
                "sudo kubectl patch battlegroup {} -n {} --type=json -p {} -o json",
                sh_single_quoted(&request.battlegroup.name),
                sh_single_quoted(&request.battlegroup.namespace),
                sh_single_quoted(&patch),
            );
            self.runner
                .run_json(&command, "map display-name battlegroup patch")?;
        }

        Ok(SetMapDisplayNameResult {
            map: request.map.map_name().to_string(),
            dimension: request.dimension,
            partition_id: update.partition_id,
            display_name: request.display_name.clone(),
            restart_required: update.patch_required,
            battlegroup_patched: update.patch_required,
        })
    }

    fn battlegroup(&self, battlegroup: &BattlegroupRef) -> CommandResult<Value> {
        battlegroup.validate()?;
        let command = format!(
            "sudo kubectl get battlegroup {} -n {} -o json",
            sh_single_quoted(&battlegroup.name),
            sh_single_quoted(&battlegroup.namespace),
        );
        self.runner.run_json(&command, "map instance battlegroup")
    }

    fn write_deep_desert_pvp_config(
        &self,
        namespace: &str,
        pvp_partition_ids: &[i64],
    ) -> CommandResult<()> {
        validate_kube_arg(namespace, "namespace")?;
        let list = pvp_partition_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        self.runner
            .run_script(&write_pvp_config_script(namespace, &list))?;
        Ok(())
    }
}
