//! Display-name patch construction for BattleGroup server-group pod specs.

use serde_json::{json, Value};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::instance_management::{
        display_name_models::SetMapDisplayNameRequest, instance_map::InstanceMap, shell::descend,
    },
};

pub(super) const SERVER_DISPLAY_NAME_ARGUMENT_PREFIX: &str =
    "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DisplayNameUpdate {
    pub(super) partition_id: i64,
    pub(super) patch_required: bool,
    pub(super) patch_operations: Vec<Value>,
}

pub(super) fn build_display_name_update(
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
