//! BattleGroup map instance partition management.
//!
//! The vendor BattleGroup custom resource stores the durable list of map
//! partitions at `spec.database.template.spec.deployment.spec.worldPartitions`.
//! Updating that list, then restarting the BattleGroup, lets the operators and
//! game database converge on additional Survival or Deep Desert instances.
//! Deep Desert instances are distinct partition IDs on dimension zero; Survival
//! instances use distinct dimensions.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{BattlegroupRef, RemoteCommandRunner},
    validation::validate_kube_arg,
};

const SERVER_DISPLAY_NAME_ARGUMENT_PREFIX: &str =
    "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=";

/// Supported map family for user-facing instance count operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceMap {
    /// The primary survival map, stored as `Survival_1`.
    Survival1,
    /// The Deep Desert map, stored as `DeepDesert_1`.
    DeepDesert,
}

impl InstanceMap {
    /// Parses a CLI/user map name.
    pub fn parse(value: &str) -> CommandResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "survival-1" | "survival_1" | "survival" => Ok(Self::Survival1),
            "deep-desert" | "deep_desert" | "deepdesert" | "deepdesert_1" | "deep-desert-1" => {
                Ok(Self::DeepDesert)
            }
            _ => Err(failure(format!(
                "Unsupported instance map {value}; use survival-1 or deep-desert"
            ))),
        }
    }

    /// Returns the Kubernetes/game map name.
    pub fn map_name(self) -> &'static str {
        match self {
            Self::Survival1 => "Survival_1",
            Self::DeepDesert => "DeepDesert_1",
        }
    }
}

/// Request for setting the desired number of map partitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMapInstancesRequest {
    /// BattleGroup namespace and resource name.
    pub battlegroup: BattlegroupRef,
    /// Map family to modify.
    pub map: InstanceMap,
    /// Desired partition count. Must be at least one.
    pub count: usize,
    /// Deep Desert partition IDs that should be marked PvP in user config.
    ///
    /// `None` leaves config files untouched. `Some(Vec::new())` clears the
    /// configured PvP partition list.
    pub pvp_partition_ids: Option<Vec<i64>>,
    /// Number of Deep Desert instances that should be marked PvP.
    ///
    /// When set, the highest selected Deep Desert partition IDs are marked as
    /// PvP and the remaining selected partitions stay PvE. This is the
    /// user-facing setup flow; `pvp_partition_ids` is the lower-level escape
    /// hatch for exact partition control.
    pub pvp_instance_count: Option<usize>,
}

impl SetMapInstancesRequest {
    /// Creates a request without PvP config changes.
    pub fn new(battlegroup: BattlegroupRef, map: InstanceMap, count: usize) -> Self {
        Self {
            battlegroup,
            map,
            count,
            pvp_partition_ids: None,
            pvp_instance_count: None,
        }
    }

    fn validate(&self) -> CommandResult<()> {
        self.battlegroup.validate()?;
        if self.count == 0 || self.count > 64 {
            return Err(failure("--count must be between 1 and 64"));
        }
        if self.map == InstanceMap::DeepDesert && self.count > 1 {
            return Err(failure(
                "Only one Deep Desert instance is supported in this build",
            ));
        }
        if self.pvp_partition_ids.is_some() && self.pvp_instance_count.is_some() {
            return Err(failure(
                "Use either explicit PvP partition IDs or a PvP instance count, not both",
            ));
        }
        if let Some(ids) = &self.pvp_partition_ids {
            for id in ids {
                if *id <= 0 {
                    return Err(failure("PvP partition IDs must be positive"));
                }
            }
            if self.map != InstanceMap::DeepDesert {
                return Err(failure(
                    "PvP partition config is currently supported only for deep-desert",
                ));
            }
        }
        if let Some(count) = self.pvp_instance_count {
            if self.map != InstanceMap::DeepDesert {
                return Err(failure(
                    "PvP instance config is currently supported only for deep-desert",
                ));
            }
            if count > self.count {
                return Err(failure(
                    "PvP instance count cannot exceed total instance count",
                ));
            }
        }
        Ok(())
    }
}

/// Result of setting map partitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMapInstancesResult {
    /// Map name that was modified.
    pub map: String,
    /// Partition IDs after the patch.
    pub partition_ids: Vec<i64>,
    /// PvP partition IDs written to config.
    pub pvp_partition_ids: Vec<i64>,
    /// Whether a BattleGroup restart is required for all consumers to see the change.
    pub restart_required: bool,
    /// Whether the BattleGroup resource was patched.
    pub battlegroup_patched: bool,
    /// Whether PvP config files were updated.
    pub pvp_config_updated: bool,
}

/// Request for changing one map dimension's player-facing display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMapDisplayNameRequest {
    /// BattleGroup namespace and resource name.
    pub battlegroup: BattlegroupRef,
    /// Map family to modify.
    pub map: InstanceMap,
    /// Dimension index from the BattleGroup world partition list.
    pub dimension: i64,
    /// New display name. `None` clears the per-partition override.
    pub display_name: Option<String>,
}

impl SetMapDisplayNameRequest {
    /// Creates a request that sets a display-name override.
    pub fn set(
        battlegroup: BattlegroupRef,
        map: InstanceMap,
        dimension: i64,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            battlegroup,
            map,
            dimension,
            display_name: Some(display_name.into()),
        }
    }

    /// Creates a request that removes a display-name override.
    pub fn clear(battlegroup: BattlegroupRef, map: InstanceMap, dimension: i64) -> Self {
        Self {
            battlegroup,
            map,
            dimension,
            display_name: None,
        }
    }

    fn validate(&self) -> CommandResult<()> {
        self.battlegroup.validate()?;
        if self.dimension < 0 {
            return Err(failure("--dimension must be zero or greater"));
        }
        if let Some(display_name) = &self.display_name {
            validate_display_name(display_name)?;
        }
        Ok(())
    }
}

/// Result of changing a map dimension display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMapDisplayNameResult {
    /// Map name that was modified.
    pub map: String,
    /// Dimension index that was modified.
    pub dimension: i64,
    /// Backing partition ID used as the per-partition pod spec index.
    pub partition_id: i64,
    /// Effective display name override after the operation.
    pub display_name: Option<String>,
    /// Whether a BattleGroup restart/reconcile may be required for clients to see the change.
    pub restart_required: bool,
    /// Whether the BattleGroup resource was patched.
    pub battlegroup_patched: bool,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPartitionUpdate {
    partition_ids: Vec<i64>,
    patch_required: bool,
    patch_operations: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisplayNameUpdate {
    partition_id: i64,
    patch_required: bool,
    patch_operations: Vec<Value>,
}

fn build_world_partition_update(
    battlegroup: &Value,
    map: InstanceMap,
    count: usize,
) -> CommandResult<WorldPartitionUpdate> {
    let world_partitions_path = [
        "spec",
        "database",
        "template",
        "spec",
        "deployment",
        "spec",
        "worldPartitions",
    ];
    let world_partitions = descend(battlegroup, &world_partitions_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup worldPartitions is not an array"))?;
    let map_name = map.map_name();
    let map_index = world_partitions
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no worldPartitions entry for {map_name}"
            ))
        })?;
    let entry = &world_partitions[map_index];
    let current = entry["partitions"]
        .as_array()
        .ok_or_else(|| failure(format!("{map_name} partitions is not an array")))?;
    if current.is_empty() {
        return Err(failure(format!(
            "{map_name} has no template partition to clone"
        )));
    }

    let mut desired = current.clone();
    desired.sort_by_key(|item| {
        (
            item["dimension"].as_i64().unwrap_or(i64::MAX),
            item["id"].as_i64().unwrap_or(i64::MAX),
        )
    });

    let used_ids = collect_partition_ids(world_partitions);
    while desired.len() < count {
        let dimension = next_partition_dimension(map, &desired);
        let id = next_free_partition_id(&used_ids, &desired)?;
        let mut next = desired[0].clone();
        next["id"] = json!(id);
        next["dimension"] = json!(dimension);
        next["disable"] = json!(false);
        desired.push(next);
    }
    desired.truncate(count);

    let partition_ids = desired
        .iter()
        .map(|item| {
            item["id"]
                .as_i64()
                .ok_or_else(|| failure("Desired partition is missing id"))
        })
        .collect::<CommandResult<Vec<_>>>()?;

    let patch_required = desired != *current;
    let patch_operations = if patch_required {
        vec![json!({
            "op": "replace",
            "path": format!(
                "/spec/database/template/spec/deployment/spec/worldPartitions/{map_index}/partitions"
            ),
            "value": desired,
        })]
    } else {
        Vec::new()
    };
    let mut patch_operations = patch_operations;

    if map == InstanceMap::Survival1 {
        append_server_group_set_patch(battlegroup, map, &partition_ids, &mut patch_operations)?;
    }

    Ok(WorldPartitionUpdate {
        partition_ids,
        patch_required: !patch_operations.is_empty(),
        patch_operations,
    })
}

fn build_display_name_update(
    battlegroup: &Value,
    request: &SetMapDisplayNameRequest,
) -> CommandResult<DisplayNameUpdate> {
    let partition_id = partition_id_for_dimension(battlegroup, request.map, request.dimension)?;
    let sets_path = ["spec", "serverGroup", "template", "spec", "sets"];
    let sets = descend(battlegroup, &sets_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup serverGroup sets is not an array"))?;
    let map_name = request.map.map_name();
    let set_index = sets
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no serverGroup set entry for {map_name}"
            ))
        })?;
    let set = &sets[set_index];
    let desired_arg = request
        .display_name
        .as_ref()
        .map(|name| format!("{SERVER_DISPLAY_NAME_ARGUMENT_PREFIX}{name}"));
    let patch_operations =
        display_name_patch_operations(set, set_index, partition_id, desired_arg)?;

    Ok(DisplayNameUpdate {
        partition_id,
        patch_required: !patch_operations.is_empty(),
        patch_operations,
    })
}

fn partition_id_for_dimension(
    battlegroup: &Value,
    map: InstanceMap,
    dimension: i64,
) -> CommandResult<i64> {
    let world_partitions_path = [
        "spec",
        "database",
        "template",
        "spec",
        "deployment",
        "spec",
        "worldPartitions",
    ];
    let world_partitions = descend(battlegroup, &world_partitions_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup worldPartitions is not an array"))?;
    let map_name = map.map_name();
    let entry = world_partitions
        .iter()
        .find(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no worldPartitions entry for {map_name}"
            ))
        })?;
    let partitions = entry["partitions"]
        .as_array()
        .ok_or_else(|| failure(format!("{map_name} partitions is not an array")))?;
    let partition = partitions
        .iter()
        .find(|item| item["dimension"].as_i64() == Some(dimension))
        .ok_or_else(|| {
            failure(format!(
                "{map_name} has no partition for dimension {dimension}"
            ))
        })?;
    partition["id"]
        .as_i64()
        .ok_or_else(|| failure(format!("{map_name} dimension {dimension} is missing id")))
}

fn display_name_patch_operations(
    set: &Value,
    set_index: usize,
    partition_id: i64,
    desired_arg: Option<String>,
) -> CommandResult<Vec<Value>> {
    let pod_specs = set.get("podSpecs");
    let Some(pod_specs) = pod_specs else {
        return Ok(desired_arg
            .map(|arg| {
                vec![json!({
                    "op": "add",
                    "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/podSpecs"),
                    "value": [{
                        "index": partition_id,
                        "arguments": [arg],
                    }],
                })]
            })
            .unwrap_or_default());
    };
    let pod_specs = pod_specs
        .as_array()
        .ok_or_else(|| failure("BattleGroup serverGroup podSpecs is not an array"))?;
    let Some(pod_spec_index) = pod_specs
        .iter()
        .position(|item| item["index"].as_i64() == Some(partition_id))
    else {
        return Ok(desired_arg
            .map(|arg| {
                vec![json!({
                    "op": "add",
                    "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/podSpecs/-"),
                    "value": {
                        "index": partition_id,
                        "arguments": [arg],
                    },
                })]
            })
            .unwrap_or_default());
    };

    let pod_spec = &pod_specs[pod_spec_index];
    let arguments = pod_spec.get("arguments");
    let arguments_path = format!(
        "/spec/serverGroup/template/spec/sets/{set_index}/podSpecs/{pod_spec_index}/arguments"
    );
    let Some(arguments) = arguments else {
        return Ok(desired_arg
            .map(|arg| {
                vec![json!({
                    "op": "add",
                    "path": arguments_path,
                    "value": [arg],
                })]
            })
            .unwrap_or_default());
    };
    let arguments = arguments
        .as_array()
        .ok_or_else(|| failure("BattleGroup serverGroup podSpec arguments is not an array"))?;
    let current_index = arguments.iter().position(|item| {
        item.as_str()
            .is_some_and(|arg| arg.starts_with(SERVER_DISPLAY_NAME_ARGUMENT_PREFIX))
    });

    match (desired_arg, current_index) {
        (Some(desired), Some(arg_index)) if arguments[arg_index].as_str() == Some(&desired) => {
            Ok(Vec::new())
        }
        (Some(desired), Some(arg_index)) => Ok(vec![json!({
            "op": "replace",
            "path": format!("{arguments_path}/{arg_index}"),
            "value": desired,
        })]),
        (Some(desired), None) => Ok(vec![json!({
            "op": "add",
            "path": format!("{arguments_path}/-"),
            "value": desired,
        })]),
        (None, Some(arg_index)) => Ok(vec![json!({
            "op": "remove",
            "path": format!("{arguments_path}/{arg_index}"),
        })]),
        (None, None) => Ok(Vec::new()),
    }
}

fn append_server_group_set_patch(
    battlegroup: &Value,
    map: InstanceMap,
    partition_ids: &[i64],
    patch_operations: &mut Vec<Value>,
) -> CommandResult<()> {
    let sets_path = ["spec", "serverGroup", "template", "spec", "sets"];
    let sets = descend(battlegroup, &sets_path)?
        .as_array()
        .ok_or_else(|| failure("BattleGroup serverGroup sets is not an array"))?;
    let map_name = map.map_name();
    let set_index = sets
        .iter()
        .position(|item| item["map"].as_str() == Some(map_name))
        .ok_or_else(|| {
            failure(format!(
                "BattleGroup has no serverGroup set entry for {map_name}"
            ))
        })?;
    let set = &sets[set_index];
    let desired_replicas = partition_ids.len() as u64;
    let current_replicas = set["replicas"].as_u64();
    if current_replicas != Some(desired_replicas) {
        patch_operations.push(json!({
            "op": if set.get("replicas").is_some() { "replace" } else { "add" },
            "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/replicas"),
            "value": desired_replicas,
        }));
    }

    let desired_partitions = partition_ids.iter().map(|id| json!(id)).collect::<Vec<_>>();
    let current_partitions = set.get("partitions").and_then(Value::as_array);
    if current_partitions != Some(&desired_partitions) {
        patch_operations.push(json!({
            "op": if set.get("partitions").is_some() { "replace" } else { "add" },
            "path": format!("/spec/serverGroup/template/spec/sets/{set_index}/partitions"),
            "value": desired_partitions,
        }));
    }
    Ok(())
}

fn next_partition_dimension(map: InstanceMap, desired: &[Value]) -> i64 {
    match map {
        InstanceMap::DeepDesert => 0,
        InstanceMap::Survival1 => {
            desired
                .iter()
                .filter_map(|item| item["dimension"].as_i64())
                .max()
                .unwrap_or(-1)
                + 1
        }
    }
}

fn deep_desert_pvp_ids(partition_ids: &[i64], pvp_instance_count: usize) -> Vec<i64> {
    if pvp_instance_count == 0 {
        return Vec::new();
    }
    partition_ids
        .iter()
        .rev()
        .take(pvp_instance_count)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn descend<'a>(value: &'a Value, path: &[&str]) -> CommandResult<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| failure(format!("BattleGroup is missing {segment}")))?;
    }
    Ok(current)
}

fn collect_partition_ids(world_partitions: &[Value]) -> Vec<i64> {
    let mut ids = Vec::new();
    for entry in world_partitions {
        for partition in entry["partitions"].as_array().into_iter().flatten() {
            if let Some(id) = partition["id"].as_i64() {
                ids.push(id);
            }
        }
    }
    ids
}

fn next_free_partition_id(existing: &[i64], desired: &[Value]) -> CommandResult<i64> {
    let mut used = existing.to_vec();
    used.extend(desired.iter().filter_map(|item| item["id"].as_i64()));
    let max = used.into_iter().max().unwrap_or(0);
    max.checked_add(1)
        .ok_or_else(|| failure("No free partition ID is available"))
}

fn validate_display_name(value: &str) -> CommandResult<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(failure(
            "Display name must be a non-empty single-line value",
        ));
    }
    if value.chars().count() > 128 {
        return Err(failure("Display name must be 128 characters or fewer"));
    }
    Ok(())
}

fn write_pvp_config_script(namespace: &str, pvp_ids: &str) -> String {
    format!(
        r#"
set -eu
ns={namespace}
pvp_ids={pvp_ids}
pvc=$(sudo kubectl get pvc -n "$ns" --no-headers | awk '$1 !~ /-db-pvc$/ && $1 ~ /-pvc$/ {{ print $1; exit }}')
if [ -z "$pvc" ]; then
  echo "No shared battlegroup PVC found in $ns" >&2
  exit 1
fi
pv=$(sudo kubectl get pvc "$pvc" -n "$ns" -o jsonpath='{{.spec.volumeName}}')
pv_path=$(sudo kubectl get pv "$pv" -o jsonpath='{{.spec.local.path}}{{.spec.hostPath.path}}')
if [ -z "$pv_path" ]; then
  echo "No host path found for PVC $pvc" >&2
  exit 1
fi

update_ini() {{
  file="$1"
  sudo mkdir -p "$(dirname "$file")"
  sudo touch "$file"
  backup="$file.manager-backup-$(date +%Y%m%d%H%M%S)"
  sudo cp "$file" "$backup"
  tmp=$(mktemp)
  sudo awk -v ids="$pvp_ids" '
  BEGIN {{ section="[/Script/DuneSandbox.PvpPveSettings]"; insec=0; wrote=0 }}
  function write_block(    n, parts, i) {{
    if (!wrote) {{
      print section
      print "; Managed by Dune Dedicated Server Manager"
      print "m_bIsInitialized=True"
      print "m_bShouldForceEnablePvpOnAllPartitions=False"
      print "!m_PvpEnabledPartitions=ClearArray"
      n=split(ids, parts, " ")
      for (i=1; i<=n; i++) if (parts[i] != "") print "+m_PvpEnabledPartitions=" parts[i]
      print "!m_EffectivePvpEnabledPartitions=ClearArray"
      for (i=1; i<=n; i++) if (parts[i] != "") print "+m_EffectivePvpEnabledPartitions=(UID=" parts[i] ")"
      wrote=1
    }}
  }}
  $0 == section {{ insec=1; next }}
  /^\[/ {{
    if (insec) {{ write_block(); insec=0 }}
    print
    next
  }}
  insec {{ next }}
  {{ print }}
  END {{ if (insec || !wrote) write_block() }}
  ' "$file" > "$tmp"
  sudo cp "$tmp" "$file"
  rm -f "$tmp"
}}

update_ini "$pv_path/Saved/UserSettings/UserGame.ini"
update_ini "$pv_path/Saved/Config/LinuxServer/Game.ini"
"#,
        namespace = sh_single_quoted(namespace),
        pvp_ids = sh_single_quoted(pvp_ids)
    )
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_battlegroup() -> Value {
        json!({
            "spec": {
                "database": {
                    "template": {
                        "spec": {
                            "deployment": {
                                "spec": {
                                    "worldPartitions": [
                                        {"map":"Survival_1","partitions":[{"id":1,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}]},
                                        {"map":"Other","partitions":[{"id":2,"dimension":0,"disable":false}]},
                                        {"map":"DeepDesert_1","partitions":[{"id":8,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}]}
                                    ]
                                }
                            }
                        }
                    }
                },
                "serverGroup": {
                    "template": {
                        "spec": {
                            "sets": [
                                {"map":"Survival_1","replicas":1,"partitions":[1]},
                                {"map":"Overmap","replicas":1,"partitions":[2]},
                                {"map":"DeepDesert_1","replicas":0}
                            ]
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn preserves_single_deep_desert_partition_on_dimension_zero() {
        let update =
            build_world_partition_update(&sample_battlegroup(), InstanceMap::DeepDesert, 1)
                .unwrap();

        assert_eq!(update.partition_ids, vec![8]);
        assert!(!update.patch_required);
        assert!(update.patch_operations.is_empty());
    }

    #[test]
    fn rejects_multiple_deep_desert_world_partitions() {
        let request = SetMapInstancesRequest::new(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::DeepDesert,
            2,
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn derives_deep_desert_pvp_ids_from_instance_count() {
        assert_eq!(deep_desert_pvp_ids(&[8], 0), Vec::<i64>::new());
        assert_eq!(deep_desert_pvp_ids(&[8], 1), vec![8]);
    }

    #[test]
    fn rejects_pvp_instance_count_for_survival() {
        let mut request = SetMapInstancesRequest::new(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            2,
        );
        request.pvp_instance_count = Some(1);

        assert!(request.validate().is_err());
    }

    #[test]
    fn rejects_pvp_instance_count_above_deep_desert_total() {
        let mut request = SetMapInstancesRequest::new(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::DeepDesert,
            1,
        );
        request.pvp_instance_count = Some(2);

        assert!(request.validate().is_err());
    }

    #[test]
    fn shrinks_survival_partitions_by_dimension_order() {
        let mut bg = sample_battlegroup();
        bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
            ["partitions"] = json!([
            {"id":1,"dimension":0,"disable":false},
            {"id":30,"dimension":2,"disable":false},
            {"id":29,"dimension":1,"disable":false}
        ]);

        let update = build_world_partition_update(&bg, InstanceMap::Survival1, 2).unwrap();

        assert_eq!(update.partition_ids, vec![1, 29]);
        assert!(update.patch_required);
        assert_eq!(
            update.patch_operations[1],
            json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/replicas","value":2})
        );
        assert_eq!(
            update.patch_operations[2],
            json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1,29]})
        );
    }

    #[test]
    fn adds_survival_partitions_with_new_dimensions() {
        let update =
            build_world_partition_update(&sample_battlegroup(), InstanceMap::Survival1, 3).unwrap();

        assert_eq!(update.partition_ids, vec![1, 9, 10]);
        assert_eq!(
            update.patch_operations[0]["value"],
            json!([
                {"id":1,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1},
                {"id":9,"dimension":1,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1},
                {"id":10,"dimension":2,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}
            ])
        );
        assert_eq!(
            update.patch_operations[1],
            json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/replicas","value":3})
        );
        assert_eq!(
            update.patch_operations[2],
            json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1,9,10]})
        );
    }

    #[test]
    fn leaves_survival_server_group_when_count_is_current() {
        let update =
            build_world_partition_update(&sample_battlegroup(), InstanceMap::Survival1, 1).unwrap();

        assert_eq!(update.partition_ids, vec![1]);
        assert!(!update.patch_required);
        assert!(update.patch_operations.is_empty());
    }

    #[test]
    fn adds_survival_partitions_field_when_missing() {
        let mut bg = sample_battlegroup();
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0] =
            json!({"map":"Survival_1","replicas":1});

        let update = build_world_partition_update(&bg, InstanceMap::Survival1, 1).unwrap();

        assert_eq!(
            update.patch_operations[0],
            json!({"op":"add","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1]})
        );
    }

    #[test]
    fn display_name_adds_per_partition_pod_specs_for_dimension() {
        let mut bg = sample_battlegroup();
        bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
            ["partitions"] = json!([
            {"id":1,"dimension":0,"disable":false},
            {"id":29,"dimension":1,"disable":false}
        ]);
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["partitions"] = json!([1, 29]);
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["replicas"] = json!(2);
        let request = SetMapDisplayNameRequest::set(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            0,
            "Bob",
        );

        let update = build_display_name_update(&bg, &request).unwrap();

        assert_eq!(update.partition_id, 1);
        assert!(update.patch_required);
        assert_eq!(
            update.patch_operations,
            vec![json!({
                "op": "add",
                "path": "/spec/serverGroup/template/spec/sets/0/podSpecs",
                "value": [{
                    "index": 1,
                    "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
                }]
            })]
        );
    }

    #[test]
    fn display_name_adds_new_pod_spec_without_touching_other_dimensions() {
        let mut bg = sample_battlegroup();
        bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
            ["partitions"] = json!([
            {"id":1,"dimension":0,"disable":false},
            {"id":29,"dimension":1,"disable":false}
        ]);
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] =
            json!([{"index":29,"arguments":["-SomeOtherArg=value"]}]);
        let request = SetMapDisplayNameRequest::set(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            0,
            "Bob",
        );

        let update = build_display_name_update(&bg, &request).unwrap();

        assert_eq!(
            update.patch_operations,
            vec![json!({
                "op": "add",
                "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/-",
                "value": {
                    "index": 1,
                    "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
                }
            })]
        );
    }

    #[test]
    fn display_name_replaces_existing_override() {
        let mut bg = sample_battlegroup();
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
            "index": 1,
            "arguments": [
                "-Other=value",
                "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Alice"
            ]
        }]);
        let request = SetMapDisplayNameRequest::set(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            0,
            "Bob",
        );

        let update = build_display_name_update(&bg, &request).unwrap();

        assert_eq!(
            update.patch_operations,
            vec![json!({
                "op": "replace",
                "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/0/arguments/1",
                "value": "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"
            })]
        );
    }

    #[test]
    fn display_name_clear_removes_only_override_argument() {
        let mut bg = sample_battlegroup();
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
            "index": 1,
            "arguments": [
                "-Other=value",
                "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Alice"
            ]
        }]);
        let request = SetMapDisplayNameRequest::clear(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            0,
        );

        let update = build_display_name_update(&bg, &request).unwrap();

        assert_eq!(
            update.patch_operations,
            vec![json!({
                "op": "remove",
                "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/0/arguments/1"
            })]
        );
    }

    #[test]
    fn display_name_is_noop_when_value_is_current() {
        let mut bg = sample_battlegroup();
        bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
            "index": 1,
            "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
        }]);
        let request = SetMapDisplayNameRequest::set(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            0,
            "Bob",
        );

        let update = build_display_name_update(&bg, &request).unwrap();

        assert!(!update.patch_required);
        assert!(update.patch_operations.is_empty());
    }

    #[test]
    fn display_name_rejects_missing_dimension() {
        let request = SetMapDisplayNameRequest::set(
            BattlegroupRef {
                namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                name: "sh-host-abcdef".to_string(),
            },
            InstanceMap::Survival1,
            9,
            "Bob",
        );

        let err = build_display_name_update(&sample_battlegroup(), &request).unwrap_err();

        assert!(err.message.contains("dimension 9"));
    }
}
